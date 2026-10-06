use data_recovery_core::{BlockSource, FileBlockSource};
use data_recovery_fs_ntfs::{read_boot_sector, read_mft_records};
use data_recovery_safe_writer::{
    StorageIdentity, resolve_image_storage_identities, validate_storage_isolation,
    verify_open_source_storage, write_recovered_file_with_storage_identities,
};
use std::fmt;
use std::io::Write;
use std::path::{Path, PathBuf};

// ── Error ─────────────────────────────────────────────────────────────────────

#[derive(Debug)]
pub enum PlanError {
    Io(std::io::Error),
    Parse(String),
    Ntfs(String),
    ExFat(String),
}

impl fmt::Display for PlanError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(e) => write!(f, "I/O error: {e}"),
            Self::Parse(s) => write!(f, "plan parse error: {s}"),
            Self::Ntfs(s) => write!(f, "NTFS error: {s}"),
            Self::ExFat(s) => write!(f, "exFAT error: {s}"),
        }
    }
}

impl std::error::Error for PlanError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(e) => Some(e),
            _ => None,
        }
    }
}

impl From<std::io::Error> for PlanError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}

// ── Plan types ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecoveryPlanItem {
    pub record_index: usize,
    pub output_filename: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecoveryPlan {
    pub source: String,
    pub volume_offset: u64,
    pub items: Vec<RecoveryPlanItem>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatusFilter {
    Deleted,
    Allocated,
    All,
}

impl RecoveryPlan {
    pub fn from_json(json: &str) -> Result<Self, PlanError> {
        parse_plan(json)
    }

    pub fn from_file(path: impl AsRef<Path>) -> Result<Self, PlanError> {
        let json = std::fs::read_to_string(path)?;
        Self::from_json(&json)
    }

    /// Build a plan from `ntfs-scan-vfs --format json` output.
    /// Selects File entries matching `filter`, deduplicates output filenames.
    pub fn from_vfs_scan_json(json: &str, filter: StatusFilter) -> Result<Self, PlanError> {
        parse_plan_from_vfs_scan(json, filter)
    }

    pub fn to_json(&self) -> String {
        let mut out = String::new();
        out.push_str("{\n");
        out.push_str(&format!(
            "  \"source\": \"{}\",\n",
            json_escape(&self.source)
        ));
        out.push_str(&format!("  \"volume_offset\": {},\n", self.volume_offset));
        out.push_str("  \"items\": [\n");
        for (i, item) in self.items.iter().enumerate() {
            let comma = if i + 1 == self.items.len() { "" } else { "," };
            out.push_str(&format!(
                "    {{\"record_index\":{},\"output_filename\":\"{}\"}}{}",
                item.record_index,
                json_escape(&item.output_filename),
                comma
            ));
            out.push('\n');
        }
        out.push_str("  ]\n");
        out.push('}');
        out
    }
}

// ── Manifest types ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BatchItemResult {
    Ok {
        output: PathBuf,
        sha256: String,
        bytes_written: usize,
    },
    Err(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BatchRecoveryItem {
    pub record_index: usize,
    pub output_filename: String,
    pub result: BatchItemResult,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BatchRecoveryManifest {
    pub source: String,
    pub output_dir: PathBuf,
    pub total: usize,
    pub succeeded: usize,
    pub failed: usize,
    pub items: Vec<BatchRecoveryItem>,
    pub manifest_path: PathBuf,
}

impl BatchRecoveryManifest {
    pub fn to_json(&self) -> String {
        let mut out = String::new();
        out.push_str("{\n");
        out.push_str(&format!(
            "  \"source\": \"{}\",\n",
            json_escape(&self.source)
        ));
        out.push_str(&format!(
            "  \"output_dir\": \"{}\",\n",
            json_escape(&self.output_dir.to_string_lossy())
        ));
        out.push_str(&format!("  \"total\": {},\n", self.total));
        out.push_str(&format!("  \"succeeded\": {},\n", self.succeeded));
        out.push_str(&format!("  \"failed\": {},\n", self.failed));
        out.push_str("  \"items\": [\n");
        for (i, item) in self.items.iter().enumerate() {
            let comma = if i + 1 == self.items.len() { "" } else { "," };
            match &item.result {
                BatchItemResult::Ok {
                    output,
                    sha256,
                    bytes_written,
                } => {
                    out.push_str(&format!(
                        "    {{\"record_index\":{},\"output_filename\":\"{}\",\"status\":\"ok\",\"output\":\"{}\",\"sha256\":\"{}\",\"bytes_written\":{}}}{}",
                        item.record_index,
                        json_escape(&item.output_filename),
                        json_escape(&output.to_string_lossy()),
                        sha256,
                        bytes_written,
                        comma
                    ));
                }
                BatchItemResult::Err(msg) => {
                    out.push_str(&format!(
                        "    {{\"record_index\":{},\"output_filename\":\"{}\",\"status\":\"error\",\"error\":\"{}\"}}{}",
                        item.record_index,
                        json_escape(&item.output_filename),
                        json_escape(msg),
                        comma
                    ));
                }
            }
            out.push('\n');
        }
        out.push_str("  ]\n");
        out.push('}');
        out
    }

    pub fn to_text(&self) -> String {
        let mut out = String::new();
        out.push_str(&format!("source: {}\n", self.source));
        out.push_str(&format!("output_dir: {}\n", self.output_dir.display()));
        out.push_str(&format!("total: {}\n", self.total));
        out.push_str(&format!("succeeded: {}\n", self.succeeded));
        out.push_str(&format!("failed: {}\n", self.failed));
        out.push_str(&format!("manifest: {}\n", self.manifest_path.display()));
        for (i, item) in self.items.iter().enumerate() {
            match &item.result {
                BatchItemResult::Ok {
                    output,
                    sha256,
                    bytes_written,
                } => {
                    out.push_str(&format!(
                        "[{i}] record={} -> {} ({} bytes, sha256={})\n",
                        item.record_index,
                        output.display(),
                        bytes_written,
                        sha256
                    ));
                }
                BatchItemResult::Err(msg) => {
                    out.push_str(&format!(
                        "[{i}] record={} -> {} (ERROR: {msg})\n",
                        item.record_index, item.output_filename
                    ));
                }
            }
        }
        out
    }
}

// ── Execute ───────────────────────────────────────────────────────────────────

pub fn execute_plan(
    plan: &RecoveryPlan,
    output_dir: impl AsRef<Path>,
) -> Result<BatchRecoveryManifest, PlanError> {
    let output_dir = output_dir.as_ref();
    let identity_probe = output_dir.join(".storage-identity-preflight");
    let (source_identity, output_identity) =
        resolve_image_storage_identities(&plan.source, &identity_probe)
            .map_err(|error| PlanError::Io(std::io::Error::other(error.to_string())))?;
    execute_plan_with_storage_identities(plan, output_dir, &source_identity, &output_identity)
}

pub fn execute_plan_with_storage_identities(
    plan: &RecoveryPlan,
    output_dir: impl AsRef<Path>,
    source_identity: &StorageIdentity,
    output_identity: &StorageIdentity,
) -> Result<BatchRecoveryManifest, PlanError> {
    let output_dir = output_dir.as_ref();
    validate_storage_isolation(source_identity, output_identity)
        .map_err(|error| PlanError::Io(std::io::Error::other(error.to_string())))?;
    std::fs::create_dir_all(output_dir)?;

    let source = FileBlockSource::open(PathBuf::from(&plan.source))
        .map_err(|e| PlanError::Io(std::io::Error::other(e.to_string())))?;
    let verified_source = verify_open_source_storage(&source, source_identity)
        .map_err(|e| PlanError::Io(std::io::Error::other(e.to_string())))?;

    let mut boot_bytes = [0_u8; 512];
    source
        .read_at(plan.volume_offset, &mut boot_bytes)
        .map_err(|e| {
            PlanError::Io(std::io::Error::new(
                std::io::ErrorKind::Other,
                e.to_string(),
            ))
        })?;

    let is_exfat = &boot_bytes[3..11] == b"EXFAT   ";

    let mut items = Vec::new();
    let mut succeeded = 0usize;
    let mut failed = 0usize;

    if is_exfat {
        let boot = data_recovery_fs_exfat::ExFatBootSector::parse(&boot_bytes)
            .map_err(|e| PlanError::ExFat(e.to_string()))?;
        let entries = data_recovery_fs_exfat::scan_exfat_vfs(&source, &boot, plan.volume_offset)
            .map_err(|e| PlanError::ExFat(e.to_string()))?;

        for plan_item in &plan.items {
            let output_path = output_dir.join(&plan_item.output_filename);

            let item_result: Result<BatchItemResult, String> = (|| {
                let entry = entries
                    .iter()
                    .find(|e| e.file_offset == plan_item.record_index as u64)
                    .ok_or_else(|| {
                        format!("exFAT entry offset {} not found", plan_item.record_index)
                    })?;

                let data = data_recovery_fs_exfat::read_entry_data(
                    &source,
                    &boot,
                    plan.volume_offset,
                    entry.first_cluster,
                    entry.size,
                    entry.no_fat_chain,
                )
                .map_err(|e| e.to_string())?;

                let manifest = write_recovered_file_with_storage_identities(
                    &plan.source,
                    &output_path,
                    &data,
                    Some(plan_item.record_index as u64),
                    &verified_source,
                    output_identity,
                )
                .map_err(|e| e.to_string())?;

                Ok(BatchItemResult::Ok {
                    output: manifest.output,
                    sha256: manifest.sha256,
                    bytes_written: manifest.bytes_written,
                })
            })();

            match item_result {
                Ok(result) => {
                    succeeded += 1;
                    items.push(BatchRecoveryItem {
                        record_index: plan_item.record_index,
                        output_filename: plan_item.output_filename.clone(),
                        result,
                    });
                }
                Err(e) => {
                    failed += 1;
                    items.push(BatchRecoveryItem {
                        record_index: plan_item.record_index,
                        output_filename: plan_item.output_filename.clone(),
                        result: BatchItemResult::Err(e),
                    });
                }
            }
        }
    } else {
        // NTFS
        let boot = read_boot_sector(&source, plan.volume_offset)
            .map_err(|e| PlanError::Ntfs(e.to_string()))?;

        let max_record = plan
            .items
            .iter()
            .map(|item| (item.record_index as u64 & 0x0000_FFFF_FFFF_FFFF) as usize)
            .max()
            .map(|m| m + 1)
            .unwrap_or(0);

        let records = read_mft_records(&source, &boot, plan.volume_offset, max_record)
            .map_err(|e| PlanError::Ntfs(e.to_string()))?;

        for plan_item in &plan.items {
            let output_path = output_dir.join(&plan_item.output_filename);

            let item_result: Result<BatchItemResult, String> = (|| {
                let composite_id = plan_item.record_index as u64;
                let record_index = (composite_id & 0x0000_FFFF_FFFF_FFFF) as usize;
                let stream_index = (composite_id >> 48) as usize;

                let record = records
                    .get(record_index)
                    .ok_or_else(|| format!("record {} not available", record_index))?;

                let data = if stream_index == 0 {
                    // Use partial extraction to tolerate bad sectors in batch recovery
                    let partial = data_recovery_fs_ntfs::extract_file_data_partial(
                        &source,
                        &boot,
                        plan.volume_offset,
                        record,
                    );
                    partial.data
                } else {
                    let streams = data_recovery_fs_ntfs::list_data_streams(record);
                    let mut named_count = 0;
                    let mut stream_name = None;
                    for stream in streams {
                        if stream.name.is_some() {
                            named_count += 1;
                            if named_count == stream_index {
                                stream_name = stream.name;
                                break;
                            }
                        }
                    }
                    let Some(s_name) = stream_name else {
                        return Err(format!(
                            "stream index {} not found in record {}",
                            stream_index, record_index
                        ));
                    };
                    data_recovery_fs_ntfs::extract_named_stream(
                        &source,
                        &boot,
                        plan.volume_offset,
                        record,
                        Some(&s_name),
                    )
                    .map_err(|e| e.to_string())?
                };

                let manifest = write_recovered_file_with_storage_identities(
                    &plan.source,
                    &output_path,
                    &data,
                    Some(composite_id),
                    &verified_source,
                    output_identity,
                )
                .map_err(|e| e.to_string())?;
                Ok(BatchItemResult::Ok {
                    output: manifest.output,
                    sha256: manifest.sha256,
                    bytes_written: manifest.bytes_written,
                })
            })();

            match item_result {
                Ok(result) => {
                    succeeded += 1;
                    items.push(BatchRecoveryItem {
                        record_index: plan_item.record_index,
                        output_filename: plan_item.output_filename.clone(),
                        result,
                    });
                }
                Err(e) => {
                    failed += 1;
                    items.push(BatchRecoveryItem {
                        record_index: plan_item.record_index,
                        output_filename: plan_item.output_filename.clone(),
                        result: BatchItemResult::Err(e),
                    });
                }
            }
        }
    }

    let manifest_path = output_dir.join("batch_manifest.json");
    let manifest = BatchRecoveryManifest {
        source: plan.source.clone(),
        output_dir: output_dir.to_path_buf(),
        total: plan.items.len(),
        succeeded,
        failed,
        items,
        manifest_path: manifest_path.clone(),
    };

    let mut file = std::fs::File::create(&manifest_path)?;
    file.write_all(manifest.to_json().as_bytes())?;
    file.flush()?;

    let session_manifest_path = output_dir.join("recovery-session.manifest.txt");
    let mut session_file = std::fs::File::create(&session_manifest_path)?;
    session_file.write_all(manifest.to_text().as_bytes())?;
    session_file.flush()?;

    Ok(manifest)
}

// ── JSON escape ───────────────────────────────────────────────────────────────

fn json_escape(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for ch in value.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c.is_control() => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out
}

// ── Minimal JSON parser ───────────────────────────────────────────────────────

#[derive(Debug)]
enum JsonVal {
    Str(String),
    Num(f64),
    #[allow(dead_code)]
    Bool(bool),
    #[allow(dead_code)]
    Null,
    Array(Vec<JsonVal>),
    Object(Vec<(String, JsonVal)>),
}

struct Parser {
    chars: Vec<char>,
    pos: usize,
}

impl Parser {
    fn new(input: &str) -> Self {
        Self {
            chars: input.chars().collect(),
            pos: 0,
        }
    }

    fn skip_ws(&mut self) {
        while self.pos < self.chars.len() && self.chars[self.pos].is_ascii_whitespace() {
            self.pos += 1;
        }
    }

    fn peek(&self) -> Option<char> {
        self.chars.get(self.pos).copied()
    }

    fn eat(&mut self, expected: char) -> Result<(), PlanError> {
        self.skip_ws();
        match self.chars.get(self.pos) {
            Some(&c) if c == expected => {
                self.pos += 1;
                Ok(())
            }
            other => Err(PlanError::Parse(format!(
                "expected '{}', got {:?} at pos {}",
                expected, other, self.pos
            ))),
        }
    }

    fn parse_string(&mut self) -> Result<String, PlanError> {
        self.eat('"')?;
        let mut s = String::new();
        loop {
            match self.chars.get(self.pos) {
                None => return Err(PlanError::Parse("unterminated string".into())),
                Some('"') => {
                    self.pos += 1;
                    break;
                }
                Some('\\') => {
                    self.pos += 1;
                    match self.chars.get(self.pos) {
                        Some('"') => {
                            s.push('"');
                            self.pos += 1;
                        }
                        Some('\\') => {
                            s.push('\\');
                            self.pos += 1;
                        }
                        Some('/') => {
                            s.push('/');
                            self.pos += 1;
                        }
                        Some('n') => {
                            s.push('\n');
                            self.pos += 1;
                        }
                        Some('r') => {
                            s.push('\r');
                            self.pos += 1;
                        }
                        Some('t') => {
                            s.push('\t');
                            self.pos += 1;
                        }
                        Some('u') => {
                            self.pos += 1;
                            let hex: String = self.chars[self.pos..].iter().take(4).collect();
                            if hex.len() < 4 {
                                return Err(PlanError::Parse("short \\u escape".into()));
                            }
                            let codepoint = u32::from_str_radix(&hex, 16)
                                .map_err(|_| PlanError::Parse(format!("invalid \\u{hex}")))?;
                            let ch = char::from_u32(codepoint).ok_or_else(|| {
                                PlanError::Parse(format!("invalid codepoint {codepoint}"))
                            })?;
                            s.push(ch);
                            self.pos += 4;
                        }
                        other => {
                            return Err(PlanError::Parse(format!("invalid escape: {:?}", other)));
                        }
                    }
                }
                Some(&c) => {
                    s.push(c);
                    self.pos += 1;
                }
            }
        }
        Ok(s)
    }

    fn parse_number(&mut self) -> Result<f64, PlanError> {
        let start = self.pos;
        if self.chars.get(self.pos) == Some(&'-') {
            self.pos += 1;
        }
        while self.pos < self.chars.len() && self.chars[self.pos].is_ascii_digit() {
            self.pos += 1;
        }
        if self.chars.get(self.pos) == Some(&'.') {
            self.pos += 1;
            while self.pos < self.chars.len() && self.chars[self.pos].is_ascii_digit() {
                self.pos += 1;
            }
        }
        if matches!(self.chars.get(self.pos), Some('e') | Some('E')) {
            self.pos += 1;
            if matches!(self.chars.get(self.pos), Some('+') | Some('-')) {
                self.pos += 1;
            }
            while self.pos < self.chars.len() && self.chars[self.pos].is_ascii_digit() {
                self.pos += 1;
            }
        }
        let s: String = self.chars[start..self.pos].iter().collect();
        s.parse::<f64>()
            .map_err(|e| PlanError::Parse(format!("invalid number {s:?}: {e}")))
    }

    fn parse_value(&mut self) -> Result<JsonVal, PlanError> {
        self.skip_ws();
        match self.peek() {
            Some('"') => Ok(JsonVal::Str(self.parse_string()?)),
            Some('{') => Ok(JsonVal::Object(self.parse_object()?)),
            Some('[') => Ok(JsonVal::Array(self.parse_array()?)),
            Some('t') => {
                self.pos += 4;
                Ok(JsonVal::Bool(true))
            }
            Some('f') => {
                self.pos += 5;
                Ok(JsonVal::Bool(false))
            }
            Some('n') => {
                self.pos += 4;
                Ok(JsonVal::Null)
            }
            Some(c) if c == '-' || c.is_ascii_digit() => Ok(JsonVal::Num(self.parse_number()?)),
            other => Err(PlanError::Parse(format!(
                "unexpected token {:?} at pos {}",
                other, self.pos
            ))),
        }
    }

    fn parse_object(&mut self) -> Result<Vec<(String, JsonVal)>, PlanError> {
        self.eat('{')?;
        let mut pairs = Vec::new();
        self.skip_ws();
        if self.peek() == Some('}') {
            self.pos += 1;
            return Ok(pairs);
        }
        loop {
            self.skip_ws();
            let key = self.parse_string()?;
            self.eat(':')?;
            let val = self.parse_value()?;
            pairs.push((key, val));
            self.skip_ws();
            match self.peek() {
                Some(',') => {
                    self.pos += 1;
                }
                Some('}') => {
                    self.pos += 1;
                    break;
                }
                other => {
                    return Err(PlanError::Parse(format!(
                        "expected ',' or '}}', got {:?} at pos {}",
                        other, self.pos
                    )));
                }
            }
        }
        Ok(pairs)
    }

    fn parse_array(&mut self) -> Result<Vec<JsonVal>, PlanError> {
        self.eat('[')?;
        let mut vals = Vec::new();
        self.skip_ws();
        if self.peek() == Some(']') {
            self.pos += 1;
            return Ok(vals);
        }
        loop {
            vals.push(self.parse_value()?);
            self.skip_ws();
            match self.peek() {
                Some(',') => {
                    self.pos += 1;
                }
                Some(']') => {
                    self.pos += 1;
                    break;
                }
                other => {
                    return Err(PlanError::Parse(format!(
                        "expected ',' or ']', got {:?} at pos {}",
                        other, self.pos
                    )));
                }
            }
        }
        Ok(vals)
    }
}

// ── Plan parsing ──────────────────────────────────────────────────────────────

fn parse_plan(input: &str) -> Result<RecoveryPlan, PlanError> {
    let mut p = Parser::new(input);
    let root = p.parse_value()?;
    let pairs = match root {
        JsonVal::Object(pairs) => pairs,
        _ => return Err(PlanError::Parse("plan must be a JSON object".into())),
    };

    let source = get_str(&pairs, "source")
        .ok_or_else(|| PlanError::Parse("missing \"source\" field".into()))?;

    let volume_offset = get_u64(&pairs, "volume_offset").unwrap_or(0);

    let items_val = pairs.iter().find(|(k, _)| k == "items").map(|(_, v)| v);

    let items = match items_val {
        Some(JsonVal::Array(arr)) => arr
            .iter()
            .enumerate()
            .map(|(i, item)| {
                let item_pairs = match item {
                    JsonVal::Object(pairs) => pairs,
                    _ => return Err(PlanError::Parse(format!("items[{i}] must be an object"))),
                };
                let record_index = get_usize(item_pairs, "record_index").ok_or_else(|| {
                    PlanError::Parse(format!("items[{i}] missing \"record_index\""))
                })?;
                let output_filename = get_str(item_pairs, "output_filename").ok_or_else(|| {
                    PlanError::Parse(format!("items[{i}] missing \"output_filename\""))
                })?;
                Ok(RecoveryPlanItem {
                    record_index,
                    output_filename,
                })
            })
            .collect::<Result<Vec<_>, PlanError>>()?,
        Some(_) => return Err(PlanError::Parse("\"items\" must be an array".into())),
        None => return Err(PlanError::Parse("missing \"items\" field".into())),
    };

    Ok(RecoveryPlan {
        source,
        volume_offset,
        items,
    })
}

fn parse_plan_from_vfs_scan(input: &str, filter: StatusFilter) -> Result<RecoveryPlan, PlanError> {
    let mut p = Parser::new(input);
    let root = p.parse_value()?;
    let pairs = match root {
        JsonVal::Object(pairs) => pairs,
        _ => return Err(PlanError::Parse("scan JSON must be an object".into())),
    };

    let source = get_str(&pairs, "source")
        .ok_or_else(|| PlanError::Parse("missing \"source\" field".into()))?;
    let volume_offset = get_u64(&pairs, "volume_offset").unwrap_or(0);

    let entries = match pairs.iter().find(|(k, _)| k == "entries").map(|(_, v)| v) {
        Some(JsonVal::Array(arr)) => arr,
        _ => {
            return Err(PlanError::Parse(
                "missing or invalid \"entries\" array".into(),
            ));
        }
    };

    // First pass: collect matching File candidates
    struct Candidate {
        id: usize,
        filename: String,
    }

    let mut candidates: Vec<Candidate> = Vec::new();
    for entry in entries {
        let ep = match entry {
            JsonVal::Object(pairs) => pairs,
            _ => continue,
        };
        if get_str(ep, "kind").as_deref() != Some("File") {
            continue;
        }
        let status = get_str(ep, "status").unwrap_or_default();
        let include = match filter {
            StatusFilter::Deleted => status == "Deleted",
            StatusFilter::Allocated => status == "Allocated",
            StatusFilter::All => true,
        };
        if !include {
            continue;
        }
        let id =
            get_usize(ep, "id").ok_or_else(|| PlanError::Parse("entry missing \"id\"".into()))?;
        let path = get_str(ep, "path").unwrap_or_default();
        let filename = path
            .rsplit('/')
            .next()
            .filter(|s| !s.is_empty())
            .unwrap_or("recovered")
            .to_string();
        candidates.push(Candidate { id, filename });
    }

    // Count duplicate filenames to know which need a disambiguating id suffix
    let mut name_counts: std::collections::HashMap<String, usize> =
        std::collections::HashMap::new();
    for c in &candidates {
        *name_counts.entry(c.filename.clone()).or_insert(0) += 1;
    }

    let items = candidates
        .into_iter()
        .map(|c| {
            let output_filename = if name_counts[&c.filename] > 1 {
                let (stem, ext) = split_stem_ext(&c.filename);
                format!("{stem}_{}{ext}", c.id)
            } else {
                c.filename
            };
            RecoveryPlanItem {
                record_index: c.id,
                output_filename,
            }
        })
        .collect();

    Ok(RecoveryPlan {
        source,
        volume_offset,
        items,
    })
}

fn split_stem_ext(filename: &str) -> (&str, &str) {
    if let Some(pos) = filename.rfind('.') {
        if pos > 0 {
            return (&filename[..pos], &filename[pos..]);
        }
    }
    (filename, "")
}

fn get_str(pairs: &[(String, JsonVal)], key: &str) -> Option<String> {
    pairs.iter().find(|(k, _)| k == key).and_then(|(_, v)| {
        if let JsonVal::Str(s) = v {
            Some(s.clone())
        } else {
            None
        }
    })
}

fn get_u64(pairs: &[(String, JsonVal)], key: &str) -> Option<u64> {
    pairs.iter().find(|(k, _)| k == key).and_then(|(_, v)| {
        if let JsonVal::Num(n) = v {
            Some(*n as u64)
        } else {
            None
        }
    })
}

fn get_usize(pairs: &[(String, JsonVal)], key: &str) -> Option<usize> {
    pairs.iter().find(|(k, _)| k == key).and_then(|(_, v)| {
        if let JsonVal::Num(n) = v {
            Some(*n as usize)
        } else {
            None
        }
    })
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    fn temp_dir() -> PathBuf {
        static NEXT_ID: AtomicU64 = AtomicU64::new(0);
        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../target/test-recovery-plan")
            .join(format!("dr_recovery_plan_{}_{id}", std::process::id()))
    }

    #[test]
    fn parses_plan_json() {
        let json = r#"{
            "source": "fixtures/images/ntfs_volume_extract.bin",
            "volume_offset": 0,
            "items": [
                { "record_index": 0, "output_filename": "recovered_0.txt" },
                { "record_index": 2, "output_filename": "another.doc" }
            ]
        }"#;

        let plan = RecoveryPlan::from_json(json).unwrap();
        assert_eq!(plan.source, "fixtures/images/ntfs_volume_extract.bin");
        assert_eq!(plan.volume_offset, 0);
        assert_eq!(plan.items.len(), 2);
        assert_eq!(plan.items[0].record_index, 0);
        assert_eq!(plan.items[0].output_filename, "recovered_0.txt");
        assert_eq!(plan.items[1].record_index, 2);
        assert_eq!(plan.items[1].output_filename, "another.doc");
    }

    #[test]
    fn parses_plan_without_volume_offset() {
        let json =
            r#"{"source":"test.bin","items":[{"record_index":1,"output_filename":"out.txt"}]}"#;
        let plan = RecoveryPlan::from_json(json).unwrap();
        assert_eq!(plan.volume_offset, 0);
        assert_eq!(plan.items.len(), 1);
    }

    #[test]
    fn rejects_missing_source() {
        let json = r#"{"items":[]}"#;
        assert!(matches!(
            RecoveryPlan::from_json(json),
            Err(PlanError::Parse(_))
        ));
    }

    #[test]
    fn rejects_missing_items() {
        let json = r#"{"source":"test.bin"}"#;
        assert!(matches!(
            RecoveryPlan::from_json(json),
            Err(PlanError::Parse(_))
        ));
    }

    #[test]
    fn rejects_missing_record_index() {
        let json = r#"{"source":"test.bin","items":[{"output_filename":"out.txt"}]}"#;
        assert!(matches!(
            RecoveryPlan::from_json(json),
            Err(PlanError::Parse(_))
        ));
    }

    #[test]
    fn executes_plan_with_fixture() {
        let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
        let fixture =
            PathBuf::from(&manifest_dir).join("../../fixtures/images/ntfs_volume_extract.bin");
        if !fixture.exists() {
            return;
        }

        let out_root = temp_dir();
        let source_dir = out_root.join("source");
        let output_dir = out_root.join("output");
        std::fs::create_dir_all(&source_dir).unwrap();
        std::fs::create_dir_all(&output_dir).unwrap();

        let source_path = source_dir.join("ntfs_volume_extract.bin");
        std::fs::copy(&fixture, &source_path).unwrap();

        let plan = RecoveryPlan {
            source: source_path.to_string_lossy().into_owned(),
            volume_offset: 0,
            items: vec![RecoveryPlanItem {
                record_index: 0,
                output_filename: "recovered_0.txt".to_string(),
            }],
        };

        let source_identity = StorageIdentity::debug_fixture("fixture", ["fixture-source"]);
        let output_identity = StorageIdentity::debug_fixture("output", ["fixture-output"]);
        let manifest = execute_plan_with_storage_identities(
            &plan,
            &output_dir,
            &source_identity,
            &output_identity,
        )
        .unwrap();
        assert_eq!(manifest.total, 1);
        assert_eq!(manifest.succeeded, 1);
        assert_eq!(manifest.failed, 0);

        if let BatchItemResult::Ok {
            output,
            bytes_written,
            ..
        } = &manifest.items[0].result
        {
            assert_eq!(std::fs::read(output).unwrap(), b"hello recovered data");
            assert_eq!(*bytes_written, 20);
        } else {
            panic!("expected Ok result");
        }

        assert!(manifest.manifest_path.exists());
        let manifest_json = std::fs::read_to_string(&manifest.manifest_path).unwrap();
        assert!(manifest_json.contains("\"succeeded\": 1"));
        assert!(manifest_json.contains("\"status\":\"ok\""));

        std::fs::remove_dir_all(out_root).unwrap();
    }

    #[test]
    fn from_vfs_scan_filters_deleted_files() {
        let json = r#"{
            "source": "volume.bin",
            "volume_offset": 0,
            "mft_offset": 4096,
            "file_record_size": 1024,
            "records_read": 4,
            "entries": [
                {"id":5,"path":"/root","kind":"Directory","status":"Allocated","size":null},
                {"id":10,"path":"/root/report.txt","kind":"File","status":"Deleted","size":100},
                {"id":11,"path":"/root/notes.txt","kind":"File","status":"Allocated","size":200},
                {"id":12,"path":"/root/data.bin","kind":"File","status":"Deleted","size":50}
            ]
        }"#;

        let plan = RecoveryPlan::from_vfs_scan_json(json, StatusFilter::Deleted).unwrap();
        assert_eq!(plan.source, "volume.bin");
        assert_eq!(plan.volume_offset, 0);
        assert_eq!(plan.items.len(), 2);
        assert_eq!(plan.items[0].record_index, 10);
        assert_eq!(plan.items[0].output_filename, "report.txt");
        assert_eq!(plan.items[1].record_index, 12);
        assert_eq!(plan.items[1].output_filename, "data.bin");
    }

    #[test]
    fn from_vfs_scan_skips_directories() {
        let json = r#"{
            "source": "volume.bin",
            "volume_offset": 0,
            "mft_offset": 4096,
            "file_record_size": 1024,
            "records_read": 2,
            "entries": [
                {"id":5,"path":"/root","kind":"Directory","status":"Deleted","size":null},
                {"id":10,"path":"/root/file.txt","kind":"File","status":"Deleted","size":10}
            ]
        }"#;

        let plan = RecoveryPlan::from_vfs_scan_json(json, StatusFilter::Deleted).unwrap();
        assert_eq!(plan.items.len(), 1);
        assert_eq!(plan.items[0].record_index, 10);
    }

    #[test]
    fn from_vfs_scan_deduplicates_filenames() {
        let json = r#"{
            "source": "volume.bin",
            "volume_offset": 0,
            "mft_offset": 4096,
            "file_record_size": 1024,
            "records_read": 2,
            "entries": [
                {"id":10,"path":"/dir_a/report.txt","kind":"File","status":"Deleted","size":10},
                {"id":20,"path":"/dir_b/report.txt","kind":"File","status":"Deleted","size":20}
            ]
        }"#;

        let plan = RecoveryPlan::from_vfs_scan_json(json, StatusFilter::Deleted).unwrap();
        assert_eq!(plan.items.len(), 2);
        assert_eq!(plan.items[0].output_filename, "report_10.txt");
        assert_eq!(plan.items[1].output_filename, "report_20.txt");
    }

    #[test]
    fn plan_roundtrips_through_json() {
        let original = RecoveryPlan {
            source: "volume.bin".to_string(),
            volume_offset: 512,
            items: vec![
                RecoveryPlanItem {
                    record_index: 5,
                    output_filename: "file.txt".to_string(),
                },
                RecoveryPlanItem {
                    record_index: 10,
                    output_filename: "data.bin".to_string(),
                },
            ],
        };

        let json = original.to_json();
        let parsed = RecoveryPlan::from_json(&json).unwrap();
        assert_eq!(parsed, original);
    }
}
