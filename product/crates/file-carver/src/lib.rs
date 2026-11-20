use data_recovery_core::{BlockSource, ReadError, Sha256, sha256_hex};
use memchr::memmem;
use std::ops::ControlFlow;
use std::sync::Mutex;

#[derive(Debug, Clone, PartialEq)]
pub struct CarvedFile {
    pub file_type: String,
    pub offset: u64,
    pub length: u64,
    pub sha256: String,
    pub confidence: f32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnreadableRange {
    pub offset: u64,
    pub length: u64,
    pub errno: i32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CarveReport {
    pub carved_files: Vec<CarvedFile>,
    pub unreadable_ranges: Vec<UnreadableRange>,
    pub cancelled: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CarveProgress {
    pub scanned_bytes: u64,
    pub total_bytes: u64,
    pub files_found: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FormatValidation {
    pub ok: bool,
    pub extension: &'static str,
    pub confidence: f32,
    pub label: &'static str,
}

pub struct CarverConfig {
    pub chunk_size: usize,
    pub overlap_size: usize,
}

impl Default for CarverConfig {
    fn default() -> Self {
        Self {
            chunk_size: 256 * 1024, // 256KB chunks
            overlap_size: 4096,     // 4KB overlap
        }
    }
}

pub fn carve_source<S: BlockSource + ?Sized>(
    source: &S,
    config: &CarverConfig,
) -> Result<Vec<CarvedFile>, ReadError> {
    Ok(carve_source_with_report(source, config)?.carved_files)
}

pub fn carve_source_with_report<S: BlockSource + ?Sized>(
    source: &S,
    config: &CarverConfig,
) -> Result<CarveReport, ReadError> {
    let tracked = TrackingSource::new(source);
    let carved_files = carve_source_impl(&tracked, config, None)?.0;
    Ok(CarveReport {
        carved_files,
        unreadable_ranges: tracked.unreadable_ranges(),
        cancelled: false,
    })
}

pub fn carve_source_with_callback<S, F>(
    source: &S,
    config: &CarverConfig,
    mut callback: F,
) -> Result<CarveReport, ReadError>
where
    S: BlockSource + ?Sized,
    F: FnMut(CarveProgress) -> ControlFlow<()>,
{
    let tracked = TrackingSource::new(source);
    let (carved_files, cancelled) = carve_source_impl(&tracked, config, Some(&mut callback))?;
    Ok(CarveReport {
        carved_files,
        unreadable_ranges: tracked.unreadable_ranges(),
        cancelled,
    })
}

fn carve_source_impl<S: BlockSource + ?Sized>(
    source: &S,
    config: &CarverConfig,
    mut callback: Option<&mut dyn FnMut(CarveProgress) -> ControlFlow<()>>,
) -> Result<(Vec<CarvedFile>, bool), ReadError> {
    let mut files = Vec::new();
    let mut next_scan_offset = 0_u64;
    let size_bytes = source.size_bytes();
    let mut offset = 0u64;

    while offset < size_bytes {
        let bytes_to_read = config.chunk_size.min((size_bytes - offset) as usize);
        if bytes_to_read == 0 {
            break;
        }

        let mut buf = vec![0_u8; bytes_to_read];
        let read_res = match source.read_at(offset, &mut buf) {
            Ok(read_res) => read_res,
            Err(ReadError::Io(_)) => {
                let next = offset.saturating_add(bytes_to_read as u64).min(size_bytes);
                if next <= offset {
                    break;
                }
                offset = next;
                continue;
            }
            Err(error) => return Err(error),
        };
        if read_res.bytes_read == 0 {
            break;
        }
        let buf = &buf[..read_res.bytes_read];

        if let Some(callback) = callback.as_mut() {
            if callback(CarveProgress {
                scanned_bytes: offset.saturating_add(read_res.bytes_read as u64),
                total_bytes: size_bytes,
                files_found: files.len(),
            })
            .is_break()
            {
                return Ok((finalize_carved_files(files), true));
            }
        }

        // Scan only the next candidate signature; memmem avoids six
        // per-byte marker comparisons over every chunk.
        let mut i = 0;
        while i < buf.len() {
            let Some(candidate) = next_signature_offset(buf, i) else {
                break;
            };
            i = candidate;
            let abs_pos = offset + i as u64;
            if abs_pos < next_scan_offset {
                i += 1;
                continue;
            }

            // 1. PNG
            if i + 8 <= buf.len() && &buf[i..i + 8] == b"\x89PNG\r\n\x1a\n" {
                if let Some(len) = find_png_length(source, abs_pos, 20 * 1024 * 1024) {
                    if let Ok(file_data) = read_file_data(source, abs_pos, len) {
                        let validation = validate_carved_bytes("PNG", &file_data);
                        if !is_mostly_padding(&file_data) && validation.ok {
                            record_carved_file(
                                &mut files,
                                &mut next_scan_offset,
                                CarvedFile {
                                    file_type: "PNG".to_string(),
                                    offset: abs_pos,
                                    length: len,
                                    sha256: sha256_hex(&file_data),
                                    confidence: validation.confidence,
                                },
                            );
                            i += len as usize;
                            continue;
                        }
                    }
                }
            }

            // 2. JPEG — strict marker walk (SOF + SOS + EOI), not just first FF D9.
            if i + 3 <= buf.len() && &buf[i..i + 3] == b"\xFF\xD8\xFF" {
                if let Some((len, conf)) = find_jpeg_length(source, abs_pos, 25 * 1024 * 1024) {
                    if let Ok(file_data) = read_file_data(source, abs_pos, len) {
                        let validation = validate_carved_bytes("JPEG", &file_data);
                        if !is_mostly_padding(&file_data) && validation.ok {
                            record_carved_file(
                                &mut files,
                                &mut next_scan_offset,
                                CarvedFile {
                                    file_type: "JPEG".to_string(),
                                    offset: abs_pos,
                                    length: len,
                                    sha256: sha256_hex(&file_data),
                                    confidence: conf.min(validation.confidence),
                                },
                            );
                            i += len as usize;
                            continue;
                        }
                    }
                }
            }

            // 3. PDF — require version header, startxref, and EOF near the end.
            if i + 8 <= buf.len() && &buf[i..i + 5] == b"%PDF-" {
                if let Some((len, conf)) = find_pdf_length(source, abs_pos, 80 * 1024 * 1024) {
                    if let Ok(file_data) = read_file_data(source, abs_pos, len) {
                        let validation = validate_carved_bytes("PDF", &file_data);
                        if !is_mostly_padding(&file_data) && validation.ok {
                            record_carved_file(
                                &mut files,
                                &mut next_scan_offset,
                                CarvedFile {
                                    file_type: "PDF".to_string(),
                                    offset: abs_pos,
                                    length: len,
                                    sha256: sha256_hex(&file_data),
                                    confidence: conf.min(validation.confidence),
                                },
                            );
                            i += len as usize;
                            continue;
                        }
                    }
                }
            }

            // 4. ZIP / Office Open XML — require a coherent EOCD and central directory.
            if i + 4 <= buf.len() && &buf[i..i + 4] == b"PK\x03\x04" {
                if let Some(candidate) = read_zip_candidate(source, abs_pos, 120 * 1024 * 1024) {
                    let validation = validate_carved_bytes("ZIP", &candidate.bytes);
                    if !is_mostly_padding(&candidate.bytes) && validation.ok {
                        record_carved_file(
                            &mut files,
                            &mut next_scan_offset,
                            CarvedFile {
                                file_type: "ZIP".to_string(),
                                offset: abs_pos,
                                length: candidate.bytes.len() as u64,
                                sha256: candidate.sha256,
                                confidence: validation.confidence,
                            },
                        );
                        i += candidate.bytes.len();
                        continue;
                    }
                }
            }

            // 5. MP4 / MOV — require ftyp + real media boxes, not any random "ftyp" string.
            if i + 12 <= buf.len() && &buf[i + 4..i + 8] == b"ftyp" {
                if let Some(candidate) = read_mp4_candidate(source, abs_pos, 250 * 1024 * 1024) {
                    let validation = validate_carved_bytes("MP4", &candidate.bytes);
                    if !is_mostly_padding(&candidate.bytes) && validation.ok {
                        let file_type =
                            if validation.extension == "heic" || validation.extension == "heif" {
                                "HEIC"
                            } else {
                                "MP4"
                            };
                        record_carved_file(
                            &mut files,
                            &mut next_scan_offset,
                            CarvedFile {
                                file_type: file_type.to_string(),
                                offset: abs_pos,
                                length: candidate.bytes.len() as u64,
                                sha256: candidate.sha256,
                                confidence: validation.confidence,
                            },
                        );
                        i += candidate.bytes.len();
                        continue;
                    }
                }
            }

            // 6. OLE (Microsoft Office legacy formats: .doc, .xls, .ppt)
            if i + 8 <= buf.len() && &buf[i..i + 8] == b"\xD0\xCF\x11\xE0\xA1\xB1\x1A\xE1" {
                if let Some(len) = find_ole_length(source, abs_pos, 30 * 1024 * 1024) {
                    if let Ok(file_data) = read_file_data(source, abs_pos, len) {
                        let validation = validate_carved_bytes("OLE", &file_data);
                        if !is_mostly_padding(&file_data) && validation.ok {
                            record_carved_file(
                                &mut files,
                                &mut next_scan_offset,
                                CarvedFile {
                                    file_type: "OLE".to_string(),
                                    offset: abs_pos,
                                    length: len,
                                    sha256: sha256_hex(&file_data),
                                    confidence: validation.confidence,
                                },
                            );
                            i += len as usize;
                            continue;
                        }
                    }
                }
            }

            i += 1;
        }

        offset += (bytes_to_read as u64).saturating_sub(config.overlap_size as u64);
        if bytes_to_read < config.chunk_size {
            break;
        }
    }

    Ok((finalize_carved_files(files), false))
}

fn next_signature_offset(buffer: &[u8], start: usize) -> Option<usize> {
    let patterns = [
        b"\x89PNG\r\n\x1a\n".as_slice(),
        b"\xff\xd8\xff".as_slice(),
        b"%PDF-".as_slice(),
        b"PK\x03\x04".as_slice(),
        b"\xd0\xcf\x11\xe0\xa1\xb1\x1a\xe1".as_slice(),
    ];
    let mut best = patterns
        .iter()
        .filter_map(|pattern| memmem::find(&buffer[start..], pattern).map(|index| start + index))
        .min();

    let ftyp_search_start = start.saturating_add(4);
    if ftyp_search_start < buffer.len() {
        if let Some(index) = memmem::find(&buffer[ftyp_search_start..], b"ftyp") {
            let candidate = ftyp_search_start + index - 4;
            best = Some(best.map_or(candidate, |current| current.min(candidate)));
        }
    }
    best
}

fn record_carved_file(files: &mut Vec<CarvedFile>, next_scan_offset: &mut u64, file: CarvedFile) {
    if let Some(end) = file.offset.checked_add(file.length) {
        *next_scan_offset = (*next_scan_offset).max(end);
    }
    files.push(file);
}

fn finalize_carved_files(mut files: Vec<CarvedFile>) -> Vec<CarvedFile> {
    files.sort_by(|left, right| {
        left.offset
            .cmp(&right.offset)
            .then_with(|| right.length.cmp(&left.length))
    });

    let mut next_end = 0_u64;
    let mut seen_hashes = std::collections::HashSet::new();
    let mut unique_files = Vec::with_capacity(files.len());
    for file in files {
        let Some(end) = file.offset.checked_add(file.length) else {
            continue;
        };
        if file.offset < next_end || !seen_hashes.insert(file.sha256.clone()) {
            continue;
        }
        next_end = end;
        unique_files.push(file);
    }
    unique_files
}

struct TrackingSource<'a, S: BlockSource + ?Sized> {
    source: &'a S,
    unreadable_ranges: Mutex<Vec<UnreadableRange>>,
}

impl<'a, S: BlockSource + ?Sized> TrackingSource<'a, S> {
    fn new(source: &'a S) -> Self {
        Self {
            source,
            unreadable_ranges: Mutex::new(Vec::new()),
        }
    }

    fn unreadable_ranges(&self) -> Vec<UnreadableRange> {
        let mut ranges = self
            .unreadable_ranges
            .lock()
            .expect("tracking source lock must not be poisoned")
            .clone();
        ranges.sort_by_key(|range| (range.offset, range.errno));

        let mut merged: Vec<UnreadableRange> = Vec::with_capacity(ranges.len());
        for range in ranges {
            let mut merged_into_previous = false;
            if let Some(previous) = merged.last_mut() {
                if previous.errno == range.errno
                    && previous.offset.saturating_add(previous.length) >= range.offset
                {
                    let previous_end = previous.offset.saturating_add(previous.length);
                    let range_end = range.offset.saturating_add(range.length);
                    let end = previous_end.max(range_end);
                    previous.length = end.saturating_sub(previous.offset);
                    merged_into_previous = true;
                }
            }
            if !merged_into_previous {
                merged.push(range);
            }
        }
        merged
    }

    fn record_error(&self, offset: u64, length: usize, error: &ReadError) {
        let sector_size = u64::from(self.sector_size().max(1));
        let start = (offset / sector_size) * sector_size;
        let requested_end = offset
            .saturating_add(length.max(1) as u64)
            .min(self.size_bytes());
        let end = requested_end
            .saturating_add(sector_size.saturating_sub(1))
            .checked_div(sector_size)
            .unwrap_or(0)
            .saturating_mul(sector_size)
            .min(self.size_bytes());
        let length = end
            .saturating_sub(start)
            .max(sector_size.min(self.size_bytes().saturating_sub(start)));
        if length == 0 {
            return;
        }
        let errno = match error {
            ReadError::Io(error) => error.raw_os_error().unwrap_or(-1),
            _ => -1,
        };
        self.unreadable_ranges
            .lock()
            .expect("tracking source lock must not be poisoned")
            .push(UnreadableRange {
                offset: start,
                length,
                errno,
            });
    }
}

impl<S: BlockSource + ?Sized> BlockSource for TrackingSource<'_, S> {
    fn device(&self) -> &data_recovery_core::PhysicalDevice {
        self.source.device()
    }

    fn read_at(
        &self,
        offset: u64,
        buffer: &mut [u8],
    ) -> Result<data_recovery_core::ReadResult, ReadError> {
        match self.source.read_at(offset, buffer) {
            Ok(result) => Ok(result),
            Err(error) => {
                if matches!(error, ReadError::Io(_)) {
                    self.record_error(offset, buffer.len(), &error);
                }
                Err(error)
            }
        }
    }
}

fn find_png_length<S: BlockSource + ?Sized>(
    source: &S,
    start_offset: u64,
    max_size: u64,
) -> Option<u64> {
    let mut offset = start_offset;
    let limit = source.size_bytes().min(start_offset + max_size);
    let mut sig = [0_u8; 8];
    source.read_at(offset, &mut sig).ok()?;
    if &sig != b"\x89PNG\r\n\x1a\n" {
        return None;
    }
    offset += 8;

    let mut seen_ihdr = false;
    let mut seen_idat = false;
    while offset + 12 <= limit {
        let mut header = [0_u8; 8];
        let read = source.read_at(offset, &mut header).ok()?;
        if read.bytes_read < header.len() {
            return None;
        }
        let chunk_len = u32::from_be_bytes(header[0..4].try_into().ok()?) as u64;
        let chunk_type = &header[4..8];
        if !chunk_type.iter().all(|b| b.is_ascii_alphabetic()) {
            return None;
        }

        if !seen_ihdr {
            if chunk_type != b"IHDR" || chunk_len != 13 {
                return None;
            }
            seen_ihdr = true;
        } else if chunk_type == b"IHDR" {
            return None;
        }

        let chunk_total = 12_u64.checked_add(chunk_len)?;
        let next = offset.checked_add(chunk_total)?;
        if next > limit {
            return None;
        }
        if chunk_type == b"IDAT" {
            seen_idat = true;
        }
        if chunk_type == b"IEND" {
            return if chunk_len == 0 && seen_ihdr && seen_idat {
                Some(next - start_offset)
            } else {
                None
            };
        }
        offset = next;
    }
    None
}

fn read_window<S: BlockSource + ?Sized>(
    source: &S,
    start_offset: u64,
    max_size: u64,
) -> Option<Vec<u8>> {
    let available = source.size_bytes().checked_sub(start_offset)?;
    let len = available.min(max_size) as usize;
    if len == 0 {
        return None;
    }
    let mut buf = vec![0_u8; len];
    let read = source.read_at(start_offset, &mut buf).ok()?;
    if read.bytes_read == 0 {
        return None;
    }
    buf.truncate(read.bytes_read);
    Some(buf)
}

fn find_subsequence(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() || haystack.len() < needle.len() {
        return None;
    }
    haystack.windows(needle.len()).position(|w| w == needle)
}

fn rfind_subsequence(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() || haystack.len() < needle.len() {
        return None;
    }
    haystack.windows(needle.len()).rposition(|w| w == needle)
}

fn data_contains(haystack: &[u8], needle: &[u8]) -> bool {
    find_subsequence(haystack, needle).is_some()
}

fn is_mostly_padding(data: &[u8]) -> bool {
    if data.is_empty() {
        return true;
    }
    let sample_len = data.len().min(64 * 1024);
    let sample = &data[..sample_len];
    let padding = sample.iter().filter(|&&b| b == 0x00 || b == 0xff).count();
    padding * 100 >= sample_len * 98
}

pub fn classify_carved_extension(file_type: &str, data: &[u8]) -> &'static str {
    validate_carved_bytes(file_type, data).extension
}

pub fn validate_carved_bytes(file_type: &str, data: &[u8]) -> FormatValidation {
    if data.is_empty() || is_mostly_padding(data) {
        return FormatValidation {
            ok: false,
            extension: "bin",
            confidence: 0.0,
            label: "空檔或填充值，略過",
        };
    }

    match file_type {
        "PNG" => {
            let ok = valid_png_crc_bytes(data);
            FormatValidation {
                ok,
                extension: "png",
                confidence: if ok { 0.98 } else { 0.25 },
                label: if ok {
                    "PNG chunk 與 CRC 驗證通過"
                } else {
                    "PNG chunk/CRC 不完整或不一致"
                },
            }
        }
        "JPEG" => {
            let ok = valid_jpeg_bytes_strict(data);
            FormatValidation {
                ok,
                extension: "jpg",
                confidence: if ok { 0.88 } else { 0.20 },
                label: if ok {
                    "JPEG quantization/Huffman、尺寸、SOS/entropy/EOI 驗證通過"
                } else {
                    "JPEG marker 結構不完整"
                },
            }
        }
        "PDF" => {
            let score = pdf_validation_score(data);
            FormatValidation {
                ok: score >= 0.65,
                extension: "pdf",
                confidence: score,
                label: if score >= 0.90 {
                    "PDF startxref 指向有效 xref / xref stream"
                } else if score >= 0.65 {
                    "PDF EOF 與 startxref 存在，但交叉參照需修復"
                } else {
                    "PDF xref/EOF 結構不足"
                },
            }
        }
        "ZIP" => {
            let inspection = inspect_zip(data);
            let ok = inspection
                .as_ref()
                .is_some_and(|z| z.valid_local_headers > 0 && z.score >= 0.70);
            let extension = classify_zip_extension(data);
            let score = inspection.as_ref().map(|z| z.score).unwrap_or(0.20);
            FormatValidation {
                ok,
                extension,
                confidence: score,
                label: if ok {
                    "ZIP/Office 中央目錄與本機檔頭驗證通過"
                } else {
                    "ZIP 中央目錄或本機檔頭不完整"
                },
            }
        }
        "MP4" | "HEIC" => {
            let inspection = inspect_iso_bmff(data);
            let ok = inspection.as_ref().is_some_and(|m| m.score >= 0.70);
            let extension = classify_iso_bmff_extension(data);
            let score = inspection.as_ref().map(|m| m.score).unwrap_or(0.20);
            FormatValidation {
                ok,
                extension,
                confidence: score,
                label: if extension == "heic" || extension == "heif" {
                    if ok {
                        "HEIC/HEIF ftyp/meta box 結構通過"
                    } else {
                        "HEIC/HEIF box 結構不足"
                    }
                } else if ok {
                    "MP4/MOV ftyp/moov/mdat box 結構通過"
                } else {
                    "MP4/MOV box 結構不足"
                },
            }
        }
        "OLE" => {
            let ok = valid_ole_bytes_strict(data);
            FormatValidation {
                ok,
                extension: classify_ole_extension(data),
                confidence: if ok { 0.82 } else { 0.35 },
                label: if ok {
                    "OLE header、sector 與目錄線索通過"
                } else {
                    "OLE sector 或目錄線索不足"
                },
            }
        }
        _ => FormatValidation {
            ok: !is_mostly_padding(data),
            extension: "bin",
            confidence: 0.50,
            label: "僅通過基本填充值檢查",
        },
    }
}

fn crc32_update(mut crc: u32, bytes: &[u8]) -> u32 {
    for &b in bytes {
        crc ^= b as u32;
        for _ in 0..8 {
            let mask = (crc & 1).wrapping_neg();
            crc = (crc >> 1) ^ (0xEDB88320 & mask);
        }
    }
    crc
}

fn crc32_concat(a: &[u8], b: &[u8]) -> u32 {
    let crc = 0xFFFF_FFFFu32;
    let crc = crc32_update(crc, a);
    !crc32_update(crc, b)
}

fn valid_png_crc_bytes(data: &[u8]) -> bool {
    if data.len() < 45 || !data.starts_with(b"\x89PNG\r\n\x1a\n") {
        return false;
    }
    let mut pos = 8usize;
    let mut seen_ihdr = false;
    let mut seen_idat = false;
    while pos + 12 <= data.len() {
        let chunk_len = u32::from_be_bytes(match data[pos..pos + 4].try_into() {
            Ok(v) => v,
            Err(_) => return false,
        }) as usize;
        let chunk_type = &data[pos + 4..pos + 8];
        if !chunk_type.iter().all(|b| b.is_ascii_alphabetic()) {
            return false;
        }
        let data_start = pos + 8;
        let data_end = match data_start.checked_add(chunk_len) {
            Some(v) => v,
            None => return false,
        };
        let crc_end = match data_end.checked_add(4) {
            Some(v) => v,
            None => return false,
        };
        if crc_end > data.len() {
            return false;
        }
        let stored_crc = u32::from_be_bytes(match data[data_end..crc_end].try_into() {
            Ok(v) => v,
            Err(_) => return false,
        });
        let calc_crc = crc32_concat(chunk_type, &data[data_start..data_end]);
        if stored_crc != calc_crc {
            return false;
        }
        if !seen_ihdr {
            if chunk_type != b"IHDR" || chunk_len != 13 {
                return false;
            }
            let width = u32::from_be_bytes(match data[data_start..data_start + 4].try_into() {
                Ok(v) => v,
                Err(_) => return false,
            });
            let height =
                u32::from_be_bytes(match data[data_start + 4..data_start + 8].try_into() {
                    Ok(v) => v,
                    Err(_) => return false,
                });
            if width == 0 || height == 0 {
                return false;
            }
            seen_ihdr = true;
        } else if chunk_type == b"IHDR" {
            return false;
        }
        if chunk_type == b"IDAT" {
            seen_idat = true;
        }
        if chunk_type == b"IEND" {
            return chunk_len == 0 && seen_ihdr && seen_idat && crc_end == data.len();
        }
        pos = crc_end;
    }
    false
}

#[derive(Debug)]
struct JpegFrame {
    marker: u8,
    components: Vec<(u8, u8, u8)>,
}

fn jpeg_segment(data: &[u8], length_pos: usize) -> Option<&[u8]> {
    let length = u16::from_be_bytes([*data.get(length_pos)?, *data.get(length_pos + 1)?]) as usize;
    if length < 2 {
        return None;
    }
    let end = length_pos.checked_add(length)?;
    data.get(length_pos + 2..end)
}

fn valid_jpeg_quantization(payload: &[u8]) -> bool {
    let mut pos = 0usize;
    let mut tables = 0usize;
    while pos < payload.len() {
        let info = payload[pos];
        let precision = info >> 4;
        let table_id = info & 0x0F;
        if table_id > 3 || precision > 1 {
            return false;
        }
        let value_bytes = if precision == 0 { 64 } else { 128 };
        let end = match pos.checked_add(1 + value_bytes) {
            Some(end) => end,
            None => return false,
        };
        let values = match payload.get(pos + 1..end) {
            Some(values) => values,
            None => return false,
        };
        if precision == 0 {
            if values.contains(&0) {
                return false;
            }
        } else if values
            .chunks_exact(2)
            .any(|value| u16::from_be_bytes([value[0], value[1]]) == 0)
        {
            return false;
        }
        tables += 1;
        pos = end;
    }
    tables > 0
}

fn valid_jpeg_huffman(payload: &[u8]) -> bool {
    let mut pos = 0usize;
    let mut tables = 0usize;
    while pos < payload.len() {
        let info = payload[pos];
        let table_class = info >> 4;
        let table_id = info & 0x0F;
        if table_class > 1 || table_id > 3 || pos + 17 > payload.len() {
            return false;
        }
        let counts = &payload[pos + 1..pos + 17];
        let symbol_count = counts.iter().map(|&count| count as usize).sum::<usize>();
        if symbol_count == 0 || symbol_count > 256 {
            return false;
        }
        let end = match pos.checked_add(17 + symbol_count) {
            Some(end) => end,
            None => return false,
        };
        if end > payload.len() {
            return false;
        }
        tables += 1;
        pos = end;
    }
    tables > 0
}

fn valid_jpeg_bytes_strict(data: &[u8]) -> bool {
    if data.len() < 64 || !data.starts_with(b"\xff\xd8") || !data.ends_with(b"\xff\xd9") {
        return false;
    }

    let mut pos = 2usize;
    let mut segments = 0usize;
    let mut seen_dqt = false;
    let mut seen_dht = false;
    let mut frame: Option<JpegFrame> = None;

    while pos < data.len() && segments < 1024 {
        if data[pos] != 0xFF {
            return false;
        }
        while pos < data.len() && data[pos] == 0xFF {
            pos += 1;
        }
        let Some(&marker) = data.get(pos) else {
            return false;
        };
        pos += 1;
        segments += 1;

        match marker {
            0xDA => {
                let Some(payload) = jpeg_segment(data, pos) else {
                    return false;
                };
                let Some(current_frame) = frame.as_ref() else {
                    return false;
                };
                let Some(&component_count_byte) = payload.first() else {
                    return false;
                };
                let component_count = component_count_byte as usize;
                if !(1..=4).contains(&component_count) || payload.len() != 4 + component_count * 2 {
                    return false;
                }
                for component in payload[1..1 + component_count * 2].chunks_exact(2) {
                    let id = component[0];
                    let selectors = component[1];
                    if !current_frame.components.iter().any(|item| item.0 == id)
                        || selectors >> 4 > 3
                        || selectors & 0x0F > 3
                    {
                        return false;
                    }
                }
                let spectral_start = payload[1 + component_count * 2];
                let spectral_end = payload[2 + component_count * 2];
                let successive = payload[3 + component_count * 2];
                let baseline = matches!(current_frame.marker, 0xC0 | 0xC1);
                if baseline {
                    if spectral_start != 0 || spectral_end != 63 || successive != 0 {
                        return false;
                    }
                } else if spectral_start > spectral_end
                    || spectral_end > 63
                    || successive >> 4 > 13
                    || successive & 0x0F > 13
                {
                    return false;
                }

                let mut entropy = pos + payload.len() + 2;
                while entropy + 1 < data.len() {
                    if data[entropy] != 0xFF {
                        entropy += 1;
                        continue;
                    }
                    entropy += 1;
                    while entropy < data.len() && data[entropy] == 0xFF {
                        entropy += 1;
                    }
                    let Some(&entropy_marker) = data.get(entropy) else {
                        return false;
                    };
                    match entropy_marker {
                        0x00 | 0xD0..=0xD7 => entropy += 1,
                        0xD9 => {
                            return entropy + 1 == data.len() && seen_dqt && (seen_dht || baseline);
                        }
                        _ => return false,
                    }
                }
                return false;
            }
            0xD8 | 0xD9 | 0x01 | 0xD0..=0xD7 => return false,
            _ => {
                let Some(payload) = jpeg_segment(data, pos) else {
                    return false;
                };
                if marker == 0xDB {
                    if !valid_jpeg_quantization(payload) {
                        return false;
                    }
                    seen_dqt = true;
                } else if marker == 0xC4 {
                    if !valid_jpeg_huffman(payload) {
                        return false;
                    }
                    seen_dht = true;
                } else if is_jpeg_sof(marker) {
                    if frame.is_some() || payload.len() < 6 {
                        return false;
                    }
                    let precision = payload[0];
                    let height = u16::from_be_bytes([payload[1], payload[2]]);
                    let width = u16::from_be_bytes([payload[3], payload[4]]);
                    let component_count = payload[5] as usize;
                    if !matches!(precision, 8 | 12)
                        || width == 0
                        || height == 0
                        || !matches!(component_count, 1 | 3 | 4)
                        || payload.len() != 6 + component_count * 3
                    {
                        return false;
                    }
                    let mut components = Vec::with_capacity(component_count);
                    for component in payload[6..].chunks_exact(3) {
                        let id = component[0];
                        let sampling = component[1];
                        let h = sampling >> 4;
                        let v = sampling & 0x0F;
                        if id == 0
                            || components.iter().any(|item: &(u8, u8, u8)| item.0 == id)
                            || !(1..=4).contains(&h)
                            || !(1..=4).contains(&v)
                            || component[2] > 3
                        {
                            return false;
                        }
                        components.push((id, h, v));
                    }
                    frame = Some(JpegFrame { marker, components });
                }
                pos += payload.len() + 2;
            }
        }
    }
    false
}

fn parse_startxref_value(data: &[u8], startxref_pos: usize) -> Option<usize> {
    let mut p = startxref_pos + b"startxref".len();
    while p < data.len() && matches!(data[p], b'\r' | b'\n' | b' ' | b'\t') {
        p += 1;
    }
    let begin = p;
    while p < data.len() && data[p].is_ascii_digit() {
        p += 1;
    }
    if p == begin {
        return None;
    }
    std::str::from_utf8(&data[begin..p])
        .ok()?
        .parse::<usize>()
        .ok()
}

fn pdf_validation_score(data: &[u8]) -> f32 {
    if data.len() < 128
        || !data.starts_with(b"%PDF-1.")
        || !matches!(data.get(7), Some(b) if b.is_ascii_digit())
    {
        return 0.0;
    }
    let Some(eof) = rfind_subsequence(data, b"%%EOF") else {
        return 0.35;
    };
    if data.len().saturating_sub(eof) > 4096 {
        return 0.45;
    }
    let tail_start = eof.saturating_sub(8192);
    let Some(rel_startxref) = rfind_subsequence(&data[tail_start..eof], b"startxref") else {
        return 0.55;
    };
    let startxref = tail_start + rel_startxref;
    let Some(xref_pos) = parse_startxref_value(data, startxref) else {
        return 0.62;
    };
    if xref_pos >= data.len() {
        return 0.62;
    }
    if data.get(xref_pos..xref_pos + 4) == Some(&b"xref"[..]) {
        0.96
    } else {
        let end = (xref_pos + 1024).min(data.len());
        let slice = &data[xref_pos..end];
        if data_contains(slice, b" obj") && data_contains(slice, b"/XRef") {
            0.92
        } else {
            0.70
        }
    }
}

#[derive(Debug, Clone)]
struct ZipInspection {
    valid_local_headers: usize,
    score: f32,
}

fn inspect_zip(data: &[u8]) -> Option<ZipInspection> {
    if data.len() < 64 || !data.starts_with(b"PK\x03\x04") {
        return None;
    }
    let eocd = data.windows(4).rposition(|w| w == b"PK\x05\x06")?;
    if eocd + 22 > data.len() {
        return None;
    }
    let entries = u16::from_le_bytes([data[eocd + 10], data[eocd + 11]]) as usize;
    let cd_size = u32::from_le_bytes([
        data[eocd + 12],
        data[eocd + 13],
        data[eocd + 14],
        data[eocd + 15],
    ]) as usize;
    let cd_offset = u32::from_le_bytes([
        data[eocd + 16],
        data[eocd + 17],
        data[eocd + 18],
        data[eocd + 19],
    ]) as usize;
    let comment_len = u16::from_le_bytes([data[eocd + 20], data[eocd + 21]]) as usize;
    let end = eocd + 22 + comment_len;
    if entries == 0
        || entries > 100_000
        || end > data.len()
        || cd_offset >= eocd
        || cd_offset.saturating_add(cd_size) > eocd
    {
        return None;
    }
    let mut pos = cd_offset;
    let cd_end = cd_offset + cd_size;
    let mut parsed = 0usize;
    let mut local_ok = 0usize;
    while pos + 46 <= cd_end && parsed < entries {
        if data.get(pos..pos + 4) != Some(&b"PK\x01\x02"[..]) {
            break;
        }
        let name_len = u16::from_le_bytes([data[pos + 28], data[pos + 29]]) as usize;
        let extra_len = u16::from_le_bytes([data[pos + 30], data[pos + 31]]) as usize;
        let comment_len = u16::from_le_bytes([data[pos + 32], data[pos + 33]]) as usize;
        let local_offset = u32::from_le_bytes([
            data[pos + 42],
            data[pos + 43],
            data[pos + 44],
            data[pos + 45],
        ]) as usize;
        let next = pos
            .checked_add(46)?
            .checked_add(name_len)?
            .checked_add(extra_len)?
            .checked_add(comment_len)?;
        if next > cd_end {
            break;
        }
        if local_offset + 30 <= data.len()
            && data.get(local_offset..local_offset + 4) == Some(&b"PK\x03\x04"[..])
        {
            let local_name_len =
                u16::from_le_bytes([data[local_offset + 26], data[local_offset + 27]]) as usize;
            let local_extra_len =
                u16::from_le_bytes([data[local_offset + 28], data[local_offset + 29]]) as usize;
            if local_offset + 30 + local_name_len + local_extra_len <= data.len() {
                local_ok += 1;
            }
        }
        parsed += 1;
        pos = next;
    }
    if parsed == 0 {
        return None;
    }
    let ratio = (local_ok as f32 / entries.max(1) as f32).min(1.0);
    let parsed_ratio = (parsed as f32 / entries.max(1) as f32).min(1.0);
    let score = 0.45 + ratio * 0.40 + parsed_ratio * 0.15;
    Some(ZipInspection {
        valid_local_headers: local_ok,
        score,
    })
}

fn classify_zip_extension(data: &[u8]) -> &'static str {
    if data_contains(data, b"word/document.xml")
        || data_contains(
            data,
            b"application/vnd.openxmlformats-officedocument.wordprocessingml",
        )
    {
        "docx"
    } else if data_contains(data, b"xl/workbook.xml")
        || data_contains(
            data,
            b"application/vnd.openxmlformats-officedocument.spreadsheetml",
        )
    {
        "xlsx"
    } else if data_contains(data, b"ppt/presentation.xml")
        || data_contains(
            data,
            b"application/vnd.openxmlformats-officedocument.presentationml",
        )
    {
        "pptx"
    } else if data_contains(data, b"mimetypeapplication/vnd.apple.pages")
        || data_contains(data, b"Index/Document.iwa")
    {
        "pages"
    } else if data_contains(data, b"mimetypeapplication/vnd.apple.numbers")
        || data_contains(data, b"Index/Tables/DataList.iwa")
    {
        "numbers"
    } else if data_contains(data, b"mimetypeapplication/vnd.apple.keynote")
        || data_contains(data, b"Index/Slide.iwa")
    {
        "key"
    } else {
        "zip"
    }
}

#[derive(Debug, Clone)]
struct IsoBmffInspection {
    score: f32,
}

fn iso_major_brand(data: &[u8]) -> Option<&[u8]> {
    if data.len() >= 12 && data.get(4..8) == Some(&b"ftyp"[..]) {
        Some(&data[8..12])
    } else {
        None
    }
}

fn is_heif_brand(brand: &[u8]) -> bool {
    matches!(
        brand,
        b"heic" | b"heix" | b"hevc" | b"hevx" | b"heis" | b"heim" | b"heif" | b"mif1" | b"msf1"
    )
}

fn classify_iso_bmff_extension(data: &[u8]) -> &'static str {
    let brand = iso_major_brand(data).unwrap_or(b"");
    if matches!(
        brand,
        b"heic" | b"heix" | b"hevc" | b"hevx" | b"heis" | b"heim"
    ) {
        "heic"
    } else if matches!(brand, b"mif1" | b"msf1" | b"heif") {
        "heif"
    } else if brand == b"qt  " {
        "mov"
    } else if matches!(brand, b"M4V " | b"m4v ") {
        "m4v"
    } else {
        "mp4"
    }
}

fn inspect_iso_bmff(data: &[u8]) -> Option<IsoBmffInspection> {
    if data.len() < 24 || data.get(4..8) != Some(&b"ftyp"[..]) {
        return None;
    }
    let brand = iso_major_brand(data)?;
    let heif_like = is_heif_brand(brand);
    let mut offset = 0usize;
    let mut boxes = 0usize;
    let mut saw_ftyp = false;
    let mut saw_moov = false;
    let mut saw_mdat = false;
    let mut saw_meta = false;
    while offset + 8 <= data.len() && boxes < 4096 {
        let mut header_len = 8usize;
        let mut size = u32::from_be_bytes(match data[offset..offset + 4].try_into() {
            Ok(v) => v,
            Err(_) => return None,
        }) as u64;
        let box_type = &data[offset + 4..offset + 8];
        if size == 1 {
            if offset + 16 > data.len() {
                break;
            }
            size = u64::from_be_bytes(match data[offset + 8..offset + 16].try_into() {
                Ok(v) => v,
                Err(_) => return None,
            });
            header_len = 16;
        } else if size == 0 {
            break;
        }
        if size < header_len as u64 {
            break;
        }
        let end = offset.checked_add(size as usize)?;
        if end > data.len() {
            break;
        }
        if !box_type
            .iter()
            .all(|&b| b.is_ascii_alphanumeric() || b == b' ')
        {
            break;
        }
        if boxes == 0 && box_type != b"ftyp" {
            return None;
        }
        if box_type == b"ftyp" {
            saw_ftyp = true;
        }
        if box_type == b"moov" {
            saw_moov = true;
        }
        if box_type == b"mdat" {
            saw_mdat = true;
        }
        if box_type == b"meta" {
            saw_meta = true;
        }
        boxes += 1;
        offset = end;
    }
    let score = if heif_like {
        if saw_ftyp && saw_meta {
            0.88
        } else if saw_ftyp && (data_contains(data, b"iloc") || data_contains(data, b"iinf")) {
            0.76
        } else {
            0.45
        }
    } else if saw_ftyp && saw_moov && saw_mdat {
        0.95
    } else if saw_ftyp && (saw_moov || saw_mdat) {
        0.68
    } else {
        0.35
    };
    Some(IsoBmffInspection { score })
}

fn classify_ole_extension(data: &[u8]) -> &'static str {
    if data_contains(data, b"WordDocument") {
        "doc"
    } else if data_contains(data, b"Workbook") || data_contains(data, b"Book") {
        "xls"
    } else if data_contains(data, b"PowerPoint Document") || data_contains(data, b"Pictures") {
        "ppt"
    } else {
        "ole"
    }
}

fn valid_ole_bytes_strict(data: &[u8]) -> bool {
    data.len() >= 1536
        && data.starts_with(b"\xD0\xCF\x11\xE0\xA1\xB1\x1A\xE1")
        && data.get(28) == Some(&0xFE)
        && data.get(29) == Some(&0xFF)
        && matches!(data.get(30), Some(&9 | &12))
        && (data_contains(data, b"WordDocument")
            || data_contains(data, b"Workbook")
            || data_contains(data, b"PowerPoint Document")
            || data_contains(data, b"Root Entry"))
}

fn is_jpeg_sof(marker: u8) -> bool {
    matches!(
        marker,
        0xC0 | 0xC1 | 0xC2 | 0xC3 | 0xC5 | 0xC6 | 0xC7 | 0xC9 | 0xCA | 0xCB | 0xCD | 0xCE | 0xCF
    )
}

fn find_jpeg_length<S: BlockSource + ?Sized>(
    source: &S,
    start_offset: u64,
    max_size: u64,
) -> Option<(u64, f32)> {
    let buf = read_window(source, start_offset, max_size)?;
    if buf.len() < 16 || !buf.starts_with(b"\xff\xd8\xff") {
        return None;
    }

    let mut pos = 2usize;
    let mut seen_sof = false;
    let seen_sos = false;
    let mut segments = 0usize;

    while pos + 1 < buf.len() && segments < 512 {
        while pos < buf.len() && buf[pos] != 0xff {
            pos += 1;
        }
        if pos + 1 >= buf.len() {
            return None;
        }
        while pos + 1 < buf.len() && buf[pos + 1] == 0xff {
            pos += 1;
        }
        if pos + 1 >= buf.len() {
            return None;
        }
        let marker = buf[pos + 1];
        segments += 1;

        match marker {
            0xD8 | 0x01 | 0xD0..=0xD7 => {
                pos += 2;
            }
            0xD9 => {
                if seen_sof && seen_sos && pos + 2 >= 64 {
                    return Some(((pos + 2) as u64, 1.0));
                }
                return None;
            }
            0xDA => {
                if pos + 4 > buf.len() {
                    return None;
                }
                let seg_len = u16::from_be_bytes([buf[pos + 2], buf[pos + 3]]) as usize;
                if seg_len < 2 || pos + 2 + seg_len > buf.len() {
                    return None;
                }
                let mut e = pos + 2 + seg_len;
                while e + 1 < buf.len() {
                    if buf[e] == 0xff {
                        let n = buf[e + 1];
                        if n == 0x00 || (0xD0..=0xD7).contains(&n) {
                            e += 2;
                            continue;
                        }
                        if n == 0xD9 {
                            if seen_sof && e + 2 >= 64 {
                                return Some(((e + 2) as u64, 1.0));
                            }
                            return None;
                        }
                    }
                    e += 1;
                }
                return None;
            }
            _ => {
                if pos + 4 > buf.len() {
                    return None;
                }
                let seg_len = u16::from_be_bytes([buf[pos + 2], buf[pos + 3]]) as usize;
                if seg_len < 2 || pos + 2 + seg_len > buf.len() {
                    return None;
                }
                if is_jpeg_sof(marker) {
                    seen_sof = true;
                }
                pos += 2 + seg_len;
            }
        }
    }
    None
}

fn find_pdf_length<S: BlockSource + ?Sized>(
    source: &S,
    start_offset: u64,
    max_size: u64,
) -> Option<(u64, f32)> {
    let buf = read_window(source, start_offset, max_size)?;
    if buf.len() < 128 || !buf.starts_with(b"%PDF-1.") {
        return None;
    }
    match buf.get(7) {
        Some(b) if b.is_ascii_digit() => {}
        _ => return None,
    }
    let eof = rfind_subsequence(&buf, b"%%EOF")?;
    let tail = &buf[eof.saturating_sub(8192)..eof];
    find_subsequence(tail, b"startxref")?;
    let mut end = eof + 5;
    while end < buf.len() && matches!(buf[end], b'\r' | b'\n' | b' ' | b'\t') {
        end += 1;
    }
    if end < 128 {
        return None;
    }
    let score = pdf_validation_score(&buf[..end]);
    if score >= 0.65 {
        Some((end as u64, score))
    } else {
        None
    }
}

struct CandidateData {
    bytes: Vec<u8>,
    sha256: String,
}

fn append_source_range<S: BlockSource + ?Sized>(
    source: &S,
    start_offset: u64,
    length: u64,
    output: &mut Vec<u8>,
    hasher: &mut Sha256,
) -> Option<()> {
    let mut offset = start_offset;
    let end = start_offset.checked_add(length)?;
    let mut chunk = vec![0_u8; 64 * 1024];
    while offset < end {
        let take = (end - offset).min(chunk.len() as u64) as usize;
        let read = source.read_at(offset, &mut chunk[..take]).ok()?;
        if read.bytes_read == 0 || read.bytes_read > take {
            return None;
        }
        output.extend_from_slice(&chunk[..read.bytes_read]);
        hasher.update(&chunk[..read.bytes_read]);
        offset = offset.checked_add(read.bytes_read as u64)?;
    }
    Some(())
}

fn read_zip_candidate<S: BlockSource + ?Sized>(
    source: &S,
    start_offset: u64,
    max_size: u64,
) -> Option<CandidateData> {
    let mut local_header = [0_u8; 30];
    let local_read = source.read_at(start_offset, &mut local_header).ok()?;
    if local_read.bytes_read < local_header.len() || !local_header.starts_with(b"PK\x03\x04") {
        return None;
    }
    let version_needed = u16::from_le_bytes([local_header[4], local_header[5]]);
    let flags = u16::from_le_bytes([local_header[6], local_header[7]]);
    let method = u16::from_le_bytes([local_header[8], local_header[9]]);
    let name_len = u16::from_le_bytes([local_header[26], local_header[27]]) as u64;
    let extra_len = u16::from_le_bytes([local_header[28], local_header[29]]) as u64;
    if !(10..=63).contains(&version_needed)
        || flags & 0xE000 != 0
        || method > 99
        || name_len == 0
        || name_len > 4096
        || 30_u64.saturating_add(name_len).saturating_add(extra_len)
            > source.size_bytes().saturating_sub(start_offset)
    {
        return None;
    }

    let available = source.size_bytes().checked_sub(start_offset)?.min(max_size);
    let mut bytes = Vec::with_capacity(available.min(64 * 1024) as usize);
    let mut hasher = Sha256::new();
    let mut search_from = 0usize;
    while bytes.len() < available as usize {
        let old_len = bytes.len();
        let next = (available as usize).min(old_len.saturating_add(64 * 1024));
        append_source_range(
            source,
            start_offset + old_len as u64,
            (next - old_len) as u64,
            &mut bytes,
            &mut hasher,
        )?;
        while let Some(rel) = memmem::find(&bytes[search_from..], b"PK\x05\x06") {
            let pos = search_from + rel;
            if pos + 22 <= bytes.len() {
                let entries = u16::from_le_bytes([bytes[pos + 10], bytes[pos + 11]]) as u32;
                let cd_size = u32::from_le_bytes([
                    bytes[pos + 12],
                    bytes[pos + 13],
                    bytes[pos + 14],
                    bytes[pos + 15],
                ]) as usize;
                let cd_offset = u32::from_le_bytes([
                    bytes[pos + 16],
                    bytes[pos + 17],
                    bytes[pos + 18],
                    bytes[pos + 19],
                ]) as usize;
                let comment_len = u16::from_le_bytes([bytes[pos + 20], bytes[pos + 21]]) as usize;
                let end = pos.checked_add(22)?.checked_add(comment_len)?;
                let cd_ok =
                    entries > 0 && cd_offset <= pos && cd_offset.saturating_add(cd_size) <= pos;
                if end <= bytes.len() && cd_ok {
                    let inspection = inspect_zip(&bytes[..end])?;
                    if inspection.score >= 0.70 {
                        bytes.truncate(end);
                        return Some(CandidateData {
                            sha256: sha256_hex(&bytes),
                            bytes,
                        });
                    }
                }
            }
            search_from = pos.saturating_add(4);
        }
        if next == old_len {
            break;
        }
    }
    None
}

fn read_mp4_candidate<S: BlockSource + ?Sized>(
    source: &S,
    start_offset: u64,
    max_size: u64,
) -> Option<CandidateData> {
    let available = source.size_bytes().checked_sub(start_offset)?.min(max_size);
    if available < 24 {
        return None;
    }
    let mut bytes = Vec::with_capacity(64 * 1024);
    let mut hasher = Sha256::new();
    let mut offset = 0_u64;
    let mut boxes = 0usize;
    let mut saw_moov = false;
    let mut saw_mdat = false;
    let mut saw_meta = false;
    let mut heif_like = false;

    while offset + 8 <= available && boxes < 4096 {
        let mut header = [0_u8; 16];
        let header_read = source.read_at(start_offset + offset, &mut header).ok()?;
        if header_read.bytes_read < 8 {
            return None;
        }
        let mut header_len = 8_u64;
        let mut box_size = u32::from_be_bytes(header[..4].try_into().ok()?) as u64;
        if box_size == 1 {
            if header_read.bytes_read < 16 {
                return None;
            }
            box_size = u64::from_be_bytes(header[8..16].try_into().ok()?);
            header_len = 16;
        } else if box_size == 0 {
            return None;
        }
        let box_type = &header[4..8];
        if box_size < header_len
            || !box_type
                .iter()
                .all(|&b| b.is_ascii_alphanumeric() || b == b' ')
        {
            return None;
        }
        let end = offset.checked_add(box_size)?;
        if end > available {
            return None;
        }
        if boxes == 0 {
            if box_type != b"ftyp" || box_size < 16 {
                return None;
            }
            let brand = &header[8..12];
            if !brand
                .iter()
                .all(|b| b.is_ascii_alphanumeric() || *b == b' ')
            {
                return None;
            }
            heif_like = is_heif_brand(brand);
        }
        append_source_range(
            source,
            start_offset + offset,
            box_size,
            &mut bytes,
            &mut hasher,
        )?;
        if box_type == b"moov" {
            saw_moov = true;
        }
        if box_type == b"mdat" {
            saw_mdat = true;
        }
        if box_type == b"meta" {
            saw_meta = true;
        }
        boxes += 1;
        offset = end;
        if (heif_like && saw_meta) || (!heif_like && saw_moov && saw_mdat) {
            let inspection = inspect_iso_bmff(&bytes)?;
            if inspection.score >= 0.70 {
                return Some(CandidateData {
                    bytes,
                    sha256: hasher.finalize_hex(),
                });
            }
            return None;
        }
    }
    None
}

fn find_ole_length<S: BlockSource + ?Sized>(
    source: &S,
    start_offset: u64,
    max_size: u64,
) -> Option<u64> {
    let mut header = [0_u8; 512];
    if source.read_at(start_offset, &mut header).is_err() {
        return None;
    }

    if &header[0..8] != b"\xD0\xCF\x11\xE0\xA1\xB1\x1A\xE1" {
        return None;
    }

    let sector_shift = u16::from_le_bytes(header[30..32].try_into().ok()?);
    if !(9..=12).contains(&sector_shift) {
        return None;
    }
    let sector_size = 1u64 << sector_shift;

    let num_fat_sectors = u32::from_le_bytes(header[44..48].try_into().ok()?) as usize;
    let first_difat_sector = u32::from_le_bytes(header[68..72].try_into().ok()?);
    let num_difat_sectors = u32::from_le_bytes(header[72..76].try_into().ok()?) as usize;

    if num_fat_sectors == 0 || num_fat_sectors > 1_000_000 {
        return None;
    }

    let mut fat_sectors = Vec::new();

    // 1. Read first 109 FAT sector locations from header
    for k in 0..109 {
        let offset = 76 + k * 4;
        let sec = u32::from_le_bytes(header[offset..offset + 4].try_into().ok()?);
        if sec != 0xFFFFFFFF && sec != 0xFFFFFFFE {
            fat_sectors.push(sec);
        }
    }

    // 2. Read remaining FAT sector locations from DIFAT sectors if any
    let mut current_difat_sec = first_difat_sector;
    let mut difat_read = 0;
    while current_difat_sec != 0xFFFFFFFF
        && current_difat_sec != 0xFFFFFFFE
        && difat_read < num_difat_sectors
    {
        let sec_offset = start_offset + 512 + current_difat_sec as u64 * sector_size;
        let mut difat_buf = vec![0_u8; sector_size as usize];
        if source.read_at(sec_offset, &mut difat_buf).is_err() {
            break;
        }

        let entries_per_sector = (sector_size / 4) as usize;
        for k in 0..(entries_per_sector - 1) {
            let offset = k * 4;
            if offset + 4 > difat_buf.len() {
                break;
            }
            let sec = u32::from_le_bytes(difat_buf[offset..offset + 4].try_into().ok()?);
            if sec != 0xFFFFFFFF && sec != 0xFFFFFFFE {
                fat_sectors.push(sec);
            }
        }

        let next_offset = (entries_per_sector - 1) * 4;
        if next_offset + 4 <= difat_buf.len() {
            current_difat_sec =
                u32::from_le_bytes(difat_buf[next_offset..next_offset + 4].try_into().ok()?);
        } else {
            break;
        }
        difat_read += 1;
    }

    let limit_fat_sectors = num_fat_sectors.min(fat_sectors.len());
    let fat_sectors = &fat_sectors[..limit_fat_sectors];

    let mut max_sector = 0u32;
    for &sec in fat_sectors {
        if sec != 0xFFFFFFFF && sec != 0xFFFFFFFE {
            max_sector = max_sector.max(sec);
        }
    }

    // Include directory and mini-fat headers
    let first_dir_sector = u32::from_le_bytes(header[48..52].try_into().ok()?);
    if first_dir_sector < 0xFFFFFFFA {
        max_sector = max_sector.max(first_dir_sector);
    }
    let first_minifat_sector = u32::from_le_bytes(header[60..64].try_into().ok()?);
    if first_minifat_sector < 0xFFFFFFFA {
        max_sector = max_sector.max(first_minifat_sector);
    }

    let mut current_difat_sec = first_difat_sector;
    let mut difat_read = 0;
    while current_difat_sec != 0xFFFFFFFF
        && current_difat_sec != 0xFFFFFFFE
        && difat_read < num_difat_sectors
    {
        max_sector = max_sector.max(current_difat_sec);
        let sec_offset = start_offset + 512 + current_difat_sec as u64 * sector_size;
        let mut next_buf = [0_u8; 4];
        let next_offset = sector_size - 4;
        if source
            .read_at(sec_offset + next_offset, &mut next_buf)
            .is_ok()
        {
            current_difat_sec = u32::from_le_bytes(next_buf);
        } else {
            break;
        }
        difat_read += 1;
    }

    // 3. Scan the FAT sectors
    for (fat_sec_idx, &fat_sec) in fat_sectors.iter().enumerate() {
        let sec_offset = start_offset + 512 + fat_sec as u64 * sector_size;
        let mut fat_buf = vec![0_u8; sector_size as usize];
        if source.read_at(sec_offset, &mut fat_buf).is_err() {
            continue;
        }

        let entries_in_sector = (sector_size / 4) as usize;
        for k in 0..entries_in_sector {
            let offset = k * 4;
            if offset + 4 > fat_buf.len() {
                break;
            }
            let val = u32::from_le_bytes(fat_buf[offset..offset + 4].try_into().ok()?);
            if val != 0xFFFFFFFF {
                let sector_index = (fat_sec_idx * entries_in_sector + k) as u32;
                max_sector = max_sector.max(sector_index);
                if val < 0xFFFFFFFA {
                    max_sector = max_sector.max(val);
                }
            }
        }
    }

    let total_length = 512 + (max_sector as u64 + 1) * sector_size;
    if total_length <= max_size {
        Some(total_length)
    } else {
        Some(max_size)
    }
}

fn read_file_data<S: BlockSource + ?Sized>(
    source: &S,
    offset: u64,
    len: u64,
) -> Result<Vec<u8>, ReadError> {
    let mut buf = vec![0_u8; len as usize];
    let res = source.read_at(offset, &mut buf)?;
    if res.bytes_read < len as usize {
        return Err(ReadError::Io(std::io::Error::new(
            std::io::ErrorKind::UnexpectedEof,
            "read shorter than expected",
        )));
    }
    Ok(buf)
}

#[cfg(test)]
mod tests {
    use super::*;
    use data_recovery_core::PhysicalDevice;

    struct MemBlockSource {
        device: PhysicalDevice,
        data: Vec<u8>,
    }

    impl MemBlockSource {
        fn new(data: Vec<u8>) -> Self {
            Self {
                device: PhysicalDevice {
                    id: "mem".into(),
                    display_name: "mem".into(),
                    platform_path: "mem".into(),
                    size_bytes: data.len() as u64,
                    logical_sector_size: 512,
                    physical_sector_size: 512,
                    is_removable: false,
                    is_system_disk: false,
                    bus_type: data_recovery_core::BusType::Virtual,
                    trim_supported: None,
                },
                data,
            }
        }
    }

    impl BlockSource for MemBlockSource {
        fn device(&self) -> &PhysicalDevice {
            &self.device
        }

        fn read_at(
            &self,
            offset: u64,
            buffer: &mut [u8],
        ) -> Result<data_recovery_core::ReadResult, ReadError> {
            let offset = offset as usize;
            if offset >= self.data.len() {
                return Ok(data_recovery_core::ReadResult {
                    offset: offset as u64,
                    bytes_read: 0,
                });
            }
            let bytes_to_read = buffer.len().min(self.data.len() - offset);
            buffer[..bytes_to_read].copy_from_slice(&self.data[offset..offset + bytes_to_read]);
            Ok(data_recovery_core::ReadResult {
                offset: offset as u64,
                bytes_read: bytes_to_read,
            })
        }
    }

    struct EioBlockSource {
        inner: MemBlockSource,
        bad_offset: u64,
        bad_length: u64,
    }

    impl BlockSource for EioBlockSource {
        fn device(&self) -> &PhysicalDevice {
            self.inner.device()
        }

        fn read_at(
            &self,
            offset: u64,
            buffer: &mut [u8],
        ) -> Result<data_recovery_core::ReadResult, ReadError> {
            if offset >= self.bad_offset && offset < self.bad_offset.saturating_add(self.bad_length)
            {
                return Err(ReadError::Io(std::io::Error::from_raw_os_error(5)));
            }
            self.inner.read_at(offset, buffer)
        }
    }

    fn write_png_chunk(raw: &mut [u8], pos: usize, chunk_type: &[u8; 4], payload: &[u8]) -> usize {
        raw[pos..pos + 4].copy_from_slice(&(payload.len() as u32).to_be_bytes());
        raw[pos + 4..pos + 8].copy_from_slice(chunk_type);
        raw[pos + 8..pos + 8 + payload.len()].copy_from_slice(payload);
        let crc = crc32_concat(chunk_type, payload);
        let crc_pos = pos + 8 + payload.len();
        raw[crc_pos..crc_pos + 4].copy_from_slice(&crc.to_be_bytes());
        crc_pos + 4
    }

    #[test]
    fn carves_png_file() {
        let mut raw_data = vec![0_u8; 1000];
        // Offset 100: PNG header
        raw_data[100..108].copy_from_slice(b"\x89PNG\r\n\x1a\n");
        let mut pos = 108;
        // IHDR chunk: length 13, type IHDR, real CRC
        pos = write_png_chunk(
            &mut raw_data,
            pos,
            b"IHDR",
            &[0, 0, 0, 1, 0, 0, 0, 1, 8, 6, 0, 0, 0],
        );
        // IDAT chunk: one byte payload, real CRC
        pos = write_png_chunk(&mut raw_data, pos, b"IDAT", &[0x78]);
        // IEND chunk: zero length, real CRC
        pos = write_png_chunk(&mut raw_data, pos, b"IEND", &[]);

        let source = MemBlockSource::new(raw_data);
        let config = CarverConfig::default();
        let carved = carve_source(&source, &config).unwrap();

        assert_eq!(carved.len(), 1);
        assert_eq!(carved[0].file_type, "PNG");
        assert_eq!(carved[0].offset, 100);
        assert_eq!(carved[0].length, (pos - 100) as u64);
    }

    #[test]
    fn callback_can_cancel_after_first_chunk() {
        let source = MemBlockSource::new(vec![0_u8; 4096]);
        let config = CarverConfig {
            chunk_size: 512,
            overlap_size: 32,
        };
        let mut progress_events = 0;
        let report = carve_source_with_callback(&source, &config, |progress| {
            progress_events += 1;
            assert!(progress.scanned_bytes <= progress.total_bytes);
            ControlFlow::Break(())
        })
        .unwrap();

        assert!(report.cancelled);
        assert_eq!(progress_events, 1);
        assert!(report.carved_files.is_empty());
    }

    #[test]
    fn rejects_false_png_signature_without_ihdr() {
        let mut raw_data = vec![0_u8; 1000];
        raw_data[100..108].copy_from_slice(b"\x89PNG\r\n\x1a\n");
        raw_data[108..200].fill(0xAA);
        raw_data[200..208].copy_from_slice(b"IEND\xaeB`\x82");

        let source = MemBlockSource::new(raw_data);
        let config = CarverConfig::default();
        let carved = carve_source(&source, &config).unwrap();

        assert!(carved.is_empty());
    }

    #[test]
    fn continues_after_unreadable_sector_and_reports_eio() {
        let mut raw_data = vec![0_u8; 4096];
        raw_data[512..520].copy_from_slice(b"\x89PNG\r\n\x1a\n");
        let mut pos = 2000 + 8;
        raw_data[2000..2008].copy_from_slice(b"\x89PNG\r\n\x1a\n");
        pos = write_png_chunk(
            &mut raw_data,
            pos,
            b"IHDR",
            &[0, 0, 0, 1, 0, 0, 0, 1, 8, 6, 0, 0, 0],
        );
        pos = write_png_chunk(&mut raw_data, pos, b"IDAT", &[0x78]);
        write_png_chunk(&mut raw_data, pos, b"IEND", &[]);

        let source = EioBlockSource {
            inner: MemBlockSource::new(raw_data),
            bad_offset: 512,
            bad_length: 512,
        };
        let report = carve_source_with_report(&source, &CarverConfig::default()).unwrap();

        assert_eq!(report.carved_files.len(), 1);
        assert_eq!(report.carved_files[0].file_type, "PNG");
        assert_eq!(
            report.unreadable_ranges,
            vec![UnreadableRange {
                offset: 512,
                length: 512,
                errno: 5,
            }]
        );
    }

    #[test]
    fn carves_jpeg_file() {
        let mut raw_data = vec![0_u8; 1000];
        let start = 50usize;
        let mut pos = start;
        raw_data[pos..pos + 2].copy_from_slice(b"\xFF\xD8");
        pos += 2;
        // APP0 segment
        raw_data[pos..pos + 2].copy_from_slice(b"\xFF\xE0");
        raw_data[pos + 2..pos + 4].copy_from_slice(&16_u16.to_be_bytes());
        raw_data[pos + 4..pos + 18].copy_from_slice(b"JFIF\0\x01\x01\0\0\x01\0\x01\0\0");
        pos += 18;
        // DQT segment, needed for strict validation confidence.
        raw_data[pos..pos + 2].copy_from_slice(b"\xFF\xDB");
        raw_data[pos + 2..pos + 4].copy_from_slice(&67_u16.to_be_bytes());
        raw_data[pos + 4] = 0;
        for q in 0..64 {
            raw_data[pos + 5 + q] = 1 + (q as u8 % 8);
        }
        pos += 69;
        // SOF0 segment
        raw_data[pos..pos + 2].copy_from_slice(b"\xFF\xC0");
        raw_data[pos + 2..pos + 4].copy_from_slice(&17_u16.to_be_bytes());
        raw_data[pos + 4..pos + 19]
            .copy_from_slice(&[8, 0, 1, 0, 1, 3, 1, 0x11, 0, 2, 0x11, 0, 3, 0x11, 0]);
        pos += 19;
        // SOS segment + a tiny entropy payload + EOI
        raw_data[pos..pos + 2].copy_from_slice(b"\xFF\xDA");
        raw_data[pos + 2..pos + 4].copy_from_slice(&12_u16.to_be_bytes());
        raw_data[pos + 4..pos + 14].copy_from_slice(&[3, 1, 0, 2, 0, 3, 0, 0, 63, 0]);
        pos += 14;
        raw_data[pos..pos + 6].copy_from_slice(b"ABCDEF");
        pos += 6;
        raw_data[pos..pos + 2].copy_from_slice(b"\xFF\xD9");
        pos += 2;

        let source = MemBlockSource::new(raw_data);
        let config = CarverConfig::default();
        let carved = carve_source(&source, &config).unwrap();

        assert_eq!(carved.len(), 1);
        assert_eq!(carved[0].file_type, "JPEG");
        assert_eq!(carved[0].offset, start as u64);
        assert_eq!(carved[0].length, (pos - start) as u64);
    }

    #[test]
    fn rejects_jpeg_signature_in_random_bytes() {
        let mut data = vec![0xA5; 256];
        data[..3].copy_from_slice(b"\xFF\xD8\xFF");
        data[254..].copy_from_slice(b"\xFF\xD9");

        assert!(!valid_jpeg_bytes_strict(&data));
    }

    #[test]
    fn carves_zip_with_eocd_after_64k_boundary() {
        let name = b"large.txt";
        let payload = vec![b'Z'; 70 * 1024];
        let crc = crc32_concat(&[], &payload);
        let mut zip = Vec::new();
        zip.extend_from_slice(b"PK\x03\x04");
        zip.extend_from_slice(&20_u16.to_le_bytes());
        zip.extend_from_slice(&0_u16.to_le_bytes());
        zip.extend_from_slice(&0_u16.to_le_bytes());
        zip.extend_from_slice(&[0; 4]);
        zip.extend_from_slice(&crc.to_le_bytes());
        zip.extend_from_slice(&(payload.len() as u32).to_le_bytes());
        zip.extend_from_slice(&(payload.len() as u32).to_le_bytes());
        zip.extend_from_slice(&(name.len() as u16).to_le_bytes());
        zip.extend_from_slice(&0_u16.to_le_bytes());
        zip.extend_from_slice(name);
        zip.extend_from_slice(&payload);
        let central_offset = zip.len() as u32;
        zip.extend_from_slice(b"PK\x01\x02");
        zip.extend_from_slice(&20_u16.to_le_bytes());
        zip.extend_from_slice(&20_u16.to_le_bytes());
        zip.extend_from_slice(&0_u16.to_le_bytes());
        zip.extend_from_slice(&0_u16.to_le_bytes());
        zip.extend_from_slice(&[0; 4]);
        zip.extend_from_slice(&crc.to_le_bytes());
        zip.extend_from_slice(&(payload.len() as u32).to_le_bytes());
        zip.extend_from_slice(&(payload.len() as u32).to_le_bytes());
        zip.extend_from_slice(&(name.len() as u16).to_le_bytes());
        zip.extend_from_slice(&[0; 6]);
        zip.extend_from_slice(&0_u16.to_le_bytes());
        zip.extend_from_slice(&0_u32.to_le_bytes());
        zip.extend_from_slice(&0_u32.to_le_bytes());
        zip.extend_from_slice(name);
        let central_size = zip.len() as u32 - central_offset;
        zip.extend_from_slice(b"PK\x05\x06");
        zip.extend_from_slice(&[0; 4]);
        zip.extend_from_slice(&1_u16.to_le_bytes());
        zip.extend_from_slice(&1_u16.to_le_bytes());
        zip.extend_from_slice(&central_size.to_le_bytes());
        zip.extend_from_slice(&central_offset.to_le_bytes());
        zip.extend_from_slice(&0_u16.to_le_bytes());

        assert!(inspect_zip(&zip).is_some(), "fixture ZIP should validate");
        let carved = carve_source(&MemBlockSource::new(zip), &CarverConfig::default()).unwrap();
        assert_eq!(carved.len(), 1);
        assert_eq!(carved[0].file_type, "ZIP");
        assert!(carved[0].length > 64 * 1024);
    }

    #[test]
    fn carves_mp4_with_mdat_after_64k_boundary() {
        fn box_bytes(kind: &[u8; 4], payload: &[u8]) -> Vec<u8> {
            let mut result = Vec::with_capacity(payload.len() + 8);
            result.extend_from_slice(&((payload.len() + 8) as u32).to_be_bytes());
            result.extend_from_slice(kind);
            result.extend_from_slice(payload);
            result
        }

        let mut movie = box_bytes(b"ftyp", b"isom\0\0\x02\0isomiso2mp41");
        movie.extend_from_slice(&box_bytes(b"moov", &box_bytes(b"mvhd", &[0; 96])));
        movie.extend_from_slice(&box_bytes(b"mdat", &[b'M'; 70 * 1024]));

        let carved = carve_source(&MemBlockSource::new(movie), &CarverConfig::default()).unwrap();
        assert_eq!(carved.len(), 1);
        assert_eq!(carved[0].file_type, "MP4");
        assert!(carved[0].length > 64 * 1024);
    }

    #[test]
    fn discards_overlapping_carved_ranges() {
        let files = vec![
            CarvedFile {
                file_type: "short-overlap".to_string(),
                offset: 200,
                length: 100,
                sha256: "short".to_string(),
                confidence: 0.8,
            },
            CarvedFile {
                file_type: "first".to_string(),
                offset: 100,
                length: 300,
                sha256: "first".to_string(),
                confidence: 0.9,
            },
            CarvedFile {
                file_type: "second".to_string(),
                offset: 500,
                length: 20,
                sha256: "second".to_string(),
                confidence: 0.9,
            },
        ];
        let finalized = finalize_carved_files(files);

        assert_eq!(finalized.len(), 2);
        assert_eq!(finalized[0].file_type, "first");
        assert_eq!(finalized[1].file_type, "second");
        assert!(
            finalized
                .windows(2)
                .all(|pair| { pair[0].offset + pair[0].length <= pair[1].offset })
        );
    }

    #[test]
    fn carves_ole_file() {
        let sector_size = 512usize;
        let mut raw_data = vec![0_u8; 2000];
        let start_pos = 100usize;

        // OLE Magic at start_pos
        raw_data[start_pos..start_pos + 8].copy_from_slice(b"\xD0\xCF\x11\xE0\xA1\xB1\x1A\xE1");
        // OLE major version + byte order + sector shift
        raw_data[start_pos + 26..start_pos + 28].copy_from_slice(&3_u16.to_le_bytes());
        raw_data[start_pos + 28..start_pos + 30].copy_from_slice(&0xFFFE_u16.to_le_bytes());
        raw_data[start_pos + 30..start_pos + 32].copy_from_slice(&9_u16.to_le_bytes());
        // Num FAT sectors = 1 at offset 44
        raw_data[start_pos + 44..start_pos + 48].copy_from_slice(&1_u32.to_le_bytes());
        // First directory sector = 1 at offset 48
        raw_data[start_pos + 48..start_pos + 52].copy_from_slice(&1_u32.to_le_bytes());
        // First DIFAT sector = 0xFFFFFFFF at offset 68
        raw_data[start_pos + 68..start_pos + 72].copy_from_slice(&0xFFFFFFFF_u32.to_le_bytes());
        // First FAT sector location = 0 at offset 76
        raw_data[start_pos + 76..start_pos + 80].copy_from_slice(&0_u32.to_le_bytes());

        // Directory sector: add a recognizable OLE directory string so strict validation accepts it.
        let dir_pos = start_pos + 512 + sector_size;
        raw_data[dir_pos..dir_pos + 64].fill(0x41);
        raw_data[dir_pos..dir_pos + 10].copy_from_slice(b"Root Entry");

        // FAT sector (at start_pos + 512)
        let fat_pos = start_pos + 512;
        // Entry 0 (FAT sector self): 0xFFFFFFFD
        raw_data[fat_pos..fat_pos + 4].copy_from_slice(&0xFFFFFFFD_u32.to_le_bytes());
        // Entry 1 (Directory sector): 0xFFFFFFFE
        raw_data[fat_pos + 4..fat_pos + 8].copy_from_slice(&0xFFFFFFFE_u32.to_le_bytes());
        // The rest are Free: 0xFFFFFFFF
        for k in 2..(sector_size / 4) {
            let offset = fat_pos + k * 4;
            raw_data[offset..offset + 4].copy_from_slice(&0xFFFFFFFF_u32.to_le_bytes());
        }

        let source = MemBlockSource::new(raw_data);
        let config = CarverConfig::default();
        let carved = carve_source(&source, &config).unwrap();

        assert_eq!(carved.len(), 1);
        assert_eq!(carved[0].file_type, "OLE");
        assert_eq!(carved[0].offset, start_pos as u64);
        assert_eq!(carved[0].length, 512 + 2 * sector_size as u64); // 1536 bytes
    }
}
