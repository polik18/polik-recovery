use data_recovery_core::{BlockSource, FileBlockSource};
use std::collections::{HashMap, VecDeque};
use std::ops::ControlFlow;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use tauri::Emitter;

const NTFS_QUICK_SCAN_COUNT: usize = 10_000;
const NTFS_STANDARD_FALLBACK_SCAN_COUNT: usize = 50_000;
const NTFS_MAX_SCAN_COUNT: usize = 1_000_000;
const NTFS_ADS_ID_MASK: u64 = 0x0000_FFFF_FFFF_FFFF;
const APFS_MAX_EXTRACT_BYTES: u64 = 2 * 1024 * 1024 * 1024;

const THUMBNAIL_CACHE_CAPACITY: usize = 128;

struct ActiveOperation {
    job_id: String,
    cancelled: Arc<AtomicBool>,
}

struct ThumbnailCache {
    values: HashMap<String, String>,
    lru: VecDeque<String>,
}

static THUMBNAIL_CACHE: OnceLock<Mutex<ThumbnailCache>> = OnceLock::new();
static SCAN_RESULT_CACHE: OnceLock<Mutex<HashMap<String, Vec<ScanEntryPayload>>>> = OnceLock::new();
static ACTIVE_OPERATION: OnceLock<Mutex<Option<ActiveOperation>>> = OnceLock::new();
static SCAN_SEQUENCE: AtomicU64 = AtomicU64::new(1);
const SCAN_PAGE_SIZE: usize = 500;
const SCAN_RESULT_CACHE_CAPACITY: usize = 4;

fn scan_result_cache() -> &'static Mutex<HashMap<String, Vec<ScanEntryPayload>>> {
    SCAN_RESULT_CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn thumbnail_cache() -> &'static Mutex<ThumbnailCache> {
    THUMBNAIL_CACHE.get_or_init(|| {
        Mutex::new(ThumbnailCache {
            values: HashMap::new(),
            lru: VecDeque::new(),
        })
    })
}

fn cached_thumbnail(key: &str) -> Option<String> {
    let mut cache = thumbnail_cache().lock().ok()?;
    let value = cache.values.get(key).cloned()?;
    if let Some(position) = cache.lru.iter().position(|item| item == key) {
        cache.lru.remove(position);
    }
    cache.lru.push_back(key.to_string());
    Some(value)
}

fn cache_thumbnail(key: String, value: String) {
    if let Ok(mut cache) = thumbnail_cache().lock() {
        cache.values.insert(key.clone(), value);
        if let Some(position) = cache.lru.iter().position(|item| item == &key) {
            cache.lru.remove(position);
        }
        cache.lru.push_back(key);
        while cache.lru.len() > THUMBNAIL_CACHE_CAPACITY {
            if let Some(evicted) = cache.lru.pop_front() {
                cache.values.remove(&evicted);
            }
        }
    }
}

fn active_operation() -> &'static Mutex<Option<ActiveOperation>> {
    ACTIVE_OPERATION.get_or_init(|| Mutex::new(None))
}

fn begin_operation(job_id: &str) -> Result<Arc<AtomicBool>, String> {
    if job_id.trim().is_empty() {
        return Err("缺少作業 jobId，已拒絕啟動重作業。".to_string());
    }
    let mut active = active_operation()
        .lock()
        .map_err(|_| "作業狀態目前無法使用。".to_string())?;
    if active.is_some() {
        return Err("已有另一個掃描／還原作業進行中；請先取消或等待它完成。".to_string());
    }
    let cancelled = Arc::new(AtomicBool::new(false));
    *active = Some(ActiveOperation {
        job_id: job_id.to_string(),
        cancelled: Arc::clone(&cancelled),
    });
    Ok(cancelled)
}

fn finish_operation(job_id: &str) {
    if let Ok(mut active) = active_operation().lock() {
        if active
            .as_ref()
            .is_some_and(|operation| operation.job_id == job_id)
        {
            *active = None;
        }
    }
}

fn request_cancellation(job_id: &str) -> Result<(), String> {
    let active = active_operation()
        .lock()
        .map_err(|_| "作業狀態目前無法使用。".to_string())?;
    let Some(operation) = active.as_ref() else {
        return Err("目前沒有可取消的重作業。".to_string());
    };
    if operation.job_id != job_id {
        return Err("jobId 與目前作業不符，未送出取消要求。".to_string());
    }
    operation.cancelled.store(true, Ordering::SeqCst);
    Ok(())
}

fn operation_cancelled() -> bool {
    active_operation()
        .lock()
        .ok()
        .and_then(|active| {
            active
                .as_ref()
                .map(|operation| Arc::clone(&operation.cancelled))
        })
        .is_some_and(|cancelled| cancelled.load(Ordering::SeqCst))
}

fn check_cancelled() -> Result<(), String> {
    if operation_cancelled() {
        Err("使用者已中止目前作業。已停止後續掃描/還原；若是實體磁碟讀取，權限工作站也會被要求關閉。".to_string())
    } else {
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RecoveryConfidenceTier {
    High,
    Medium,
    Low,
}

impl RecoveryConfidenceTier {
    fn as_str(self) -> &'static str {
        match self {
            RecoveryConfidenceTier::High => "high",
            RecoveryConfidenceTier::Medium => "medium",
            RecoveryConfidenceTier::Low => "low",
        }
    }

    fn zh_label(self) -> &'static str {
        match self {
            RecoveryConfidenceTier::High => "高機率可用",
            RecoveryConfidenceTier::Medium => "可能部分損毀",
            RecoveryConfidenceTier::Low => "低信心碎片",
        }
    }
}

// ── Response / helper types ───────────────────────────────────────────────────

#[derive(serde::Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct ScanProgressPayload {
    pct: Option<u8>,
    scanned: usize,
    total: usize,
    files_found: usize,
    msg: String,
}

#[derive(serde::Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct CarvedFileProgressPayload {
    file_type: String,
    offset: u64,
    length: u64,
    sha256: String,
    confidence: f32,
}

#[derive(serde::Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct ExtractProgressPayload {
    pct: u8,
    done: usize,
    total: usize,
    current_file: String,
}

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct ScanResponse {
    report: String,
    vfs: String,
    entries: Vec<ScanEntryPayload>,
    scan_id: String,
    total_entries: usize,
    next_cursor: Option<usize>,
}

#[derive(serde::Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct ScanPageResponse {
    scan_id: String,
    entries: Vec<ScanEntryPayload>,
    total_entries: usize,
    next_cursor: Option<usize>,
}

#[derive(serde::Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct StructuredErrorPayload {
    code: String,
    message_zh: String,
    detail: String,
}

#[derive(serde::Serialize, serde::Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct ScanEntryPayload {
    path: String,
    kind: String,
    status: String,
    size: String,
    id: String,
    confidence: String,
    quality: String,
    tier: String,
    recovery_mode: String,
    bytes_lost: String,
    bytes_read: String,
    reason: String,
}

#[derive(serde::Serialize, serde::Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct BatchExtractItem {
    selected_id: String,
    output_path: String,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct BatchExtractRequest {
    device: String,
    mode: String,
    offset: u64,
    items: Vec<BatchExtractItem>,
}

#[derive(serde::Serialize, serde::Deserialize)]

struct HelperBatchExtractOut {
    messages: Vec<String>,
    error: Option<String>,
}

// ── Disk listing ──────────────────────────────────────────────────────────────

#[derive(serde::Serialize, Clone)]
struct DiskInfo {
    path: String,
    label: String,
    size: String,
    removable: bool,
    fs_type: String,
    solid_state: bool,
    encrypted: bool,
}

#[derive(serde::Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct OutputIsolationCheck {
    ok: bool,
    message: String,
    source_stores: Vec<String>,
    output_stores: Vec<String>,
}

#[derive(serde::Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct PartitionInfo {
    index: u32,
    offset: u64,
    length: u64,
    filesystem: Option<String>,
    name: Option<String>,
    bootable: bool,
}

#[derive(serde::Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct PartitionDetection {
    table: String,
    used_backup_header: bool,
    partitions: Vec<PartitionInfo>,
}

#[derive(serde::Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct RuntimeDiagnosticCheck {
    id: String,
    title: String,
    status: String,
    detail: String,
    action: String,
}

#[derive(serde::Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct RuntimeDiagnostics {
    app_version: String,
    platform: String,
    app_path: String,
    checks: Vec<RuntimeDiagnosticCheck>,
}

#[derive(serde::Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct SessionWorkflowPlan {
    recommended: bool,
    case_id: String,
    session_root: String,
    session_dir: String,
    create_command: String,
    index_command: String,
    doctor_command: String,
    image_manifest_target: String,
    scan_report_target: String,
    recovery_output_dir: String,
    handoff_dir: String,
    next_step: String,
    warnings: Vec<String>,
    steps: Vec<String>,
}

#[derive(serde::Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct DesktopSessionState {
    active: bool,
    created: bool,
    case_id: String,
    session_dir: String,
    manifest_path: String,
    index_path: String,
    source: String,
    mode: String,
    recovery_output_dir: String,
    recovered_dir: String,
    suspect_dir: String,
    image_dir: String,
    scan_reports_dir: String,
    quality_reports_dir: String,
    logs_dir: String,
    handoff_dir: String,
    timeline_dir: String,
    timeline_count: usize,
    dashboard_dir: String,
    dashboard_html: String,
    dashboard_json: String,
    dashboard_score: usize,
    reports_dir: String,
    customer_report_html: String,
    engineer_report_html: String,
    artifact_count: usize,
    warnings: Vec<String>,
    next_step: String,
}

#[derive(serde::Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct DesktopSessionCheck {
    id: String,
    ok: bool,
    detail: String,
}

#[derive(serde::Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct DesktopSessionDoctorReport {
    ok: bool,
    session_dir: String,
    checks: Vec<DesktopSessionCheck>,
    warnings: Vec<String>,
}

#[derive(serde::Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct DesktopSessionArtifactImportResult {
    ok: bool,
    session_dir: String,
    artifact_kind: String,
    source_path: String,
    destination_path: String,
    target_folder: String,
    copied: bool,
    message: String,
}

#[derive(serde::Serialize, Clone)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
struct DesktopSessionArtifactSummary {
    artifact_kind: String,
    destination_path: String,
    imported_at_unix: String,
}

#[derive(serde::Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct DesktopSessionTimelineResult {
    ok: bool,
    session_dir: String,
    event_id: String,
    timeline_html: String,
    timeline_json: String,
    message: String,
}

#[derive(serde::Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct DesktopSessionDashboardResult {
    ok: bool,
    session_dir: String,
    dashboard_html: String,
    dashboard_json: String,
    score: usize,
    label: String,
    message: String,
}

#[derive(serde::Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct DesktopSessionHandoffReportResult {
    ok: bool,
    session_dir: String,
    report_dir: String,
    customer_html: String,
    engineer_html: String,
    message: String,
}

#[derive(serde::Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct ImagingWorkflowPlan {
    recommended: bool,
    source: String,
    output_dir: String,
    image_path: String,
    manifest_path: String,
    html_report_path: String,
    create_command: String,
    verify_command: String,
    map_report_command: String,
    resume_command: String,
    retry_command: String,
    retry_reverse_command: String,
    scan_next_step: String,
    warnings: Vec<String>,
    steps: Vec<String>,
}

mod session;
pub(crate) use session::*;
mod disks;
pub(crate) use disks::*;
mod scan;
pub(crate) use scan::*;
mod extract;
pub(crate) use extract::*;
mod errors;
pub(crate) use errors::*;
#[tauri::command]
async fn get_imaging_workflow_plan(
    source_path: String,
    output_dir: String,
) -> Result<ImagingWorkflowPlan, String> {
    let source = source_path.trim().to_string();
    if source.is_empty() {
        return Err("請先選擇來源磁碟或映像檔。".to_string());
    }
    let out = if output_dir.trim().is_empty() {
        std::env::var("HOME")
            .map(|h| format!("{}/Desktop/Polik-Source-Images", h))
            .unwrap_or_else(|_| "/Users/Shared/Polik-Source-Images".to_string())
    } else {
        output_dir.trim().trim_end_matches('/').to_string()
    };
    if out.starts_with("/dev/") {
        return Err("映像檔輸出位置不能是 /dev/* 裝置；請選另一顆磁碟上的資料夾。".to_string());
    }
    let slug = safe_image_slug(&source);
    let prefix = format!("polik-image-{}-{}", slug, imaging_stamp());
    let image_path = format!("{}/{}.img", out, prefix);
    let manifest_path = format!("{}/{}.manifest.json", out, prefix);
    let html_report_path = format!("{}/{}.manifest.html", out, prefix);
    let create_command = format!(
        "python3 tools/macos/create_source_image.py --source {} --out-dir {}",
        shell_quote(&source),
        shell_quote(&out)
    );
    let verify_command = format!(
        "python3 tools/macos/verify_source_image.py --manifest {}",
        shell_quote(&manifest_path)
    );
    let map_report_command = format!(
        "python3 tools/macos/ddrescue_map_report.py --manifest {}",
        shell_quote(&manifest_path)
    );
    let resume_command = format!(
        "python3 tools/macos/create_source_image.py --resume-manifest {} --method ddrescue",
        shell_quote(&manifest_path)
    );
    let retry_command = format!(
        "python3 tools/macos/create_source_image.py --resume-manifest {} --method ddrescue --retry-bad --retries 3",
        shell_quote(&manifest_path)
    );
    let retry_reverse_command = format!(
        "python3 tools/macos/create_source_image.py --resume-manifest {} --method ddrescue --retry-bad --reverse --retries 3",
        shell_quote(&manifest_path)
    );
    let recommended = source.starts_with("/dev/");
    let mut warnings = Vec::new();
    if recommended {
        warnings.push(
            "來源是實體磁碟；建議先建立映像檔，再掃描 .img，避免反覆讀取快故障的原碟。".to_string(),
        );
    } else {
        warnings.push(
            "目前來源看起來已是映像檔/一般檔案，可以直接掃描；仍建議保留原始檔不要覆寫。"
                .to_string(),
        );
    }
    if out.contains("/tmp") {
        warnings.push(
            "輸出位置看起來在暫存區；磁碟映像檔可能很大，請改用外接硬碟或空間充足的位置。"
                .to_string(),
        );
    }
    Ok(ImagingWorkflowPlan {
        recommended,
        source,
        output_dir: out.clone(),
        image_path: image_path.clone(),
        manifest_path: manifest_path.clone(),
        html_report_path,
        create_command,
        verify_command,
        map_report_command,
        resume_command,
        retry_command,
        retry_reverse_command,
        scan_next_step: format!("建立並驗證完成後，請把來源改成 {} 再掃描。", image_path),
        warnings,
        steps: vec![
            "停止使用來源磁碟，不要再複製、下載或安裝檔案到來源磁碟。".to_string(),
            "將映像檔輸出到另一顆磁碟或空間充足的安全資料夾。".to_string(),
            "必須使用 ddrescue/gddrescue 並保留 mapfile；工具不存在時停止，不退回 dd。".to_string(),
            "驗證 manifest / sample hash 後，只掃描 .img 映像檔。".to_string(),
        ],
    })
}

mod diagnostics;
#[tauri::command]
async fn get_recovery_default_dir() -> String {
    std::env::var("HOME")
        .map(|h| format!("{}/Desktop/已救援", h))
        .unwrap_or_else(|_| "/Users/Shared/已救援".to_string())
}

#[tauri::command]
async fn open_report_file(path: String) -> Result<(), String> {
    let target = validate_report_open_target(std::path::Path::new(&path))?;
    open_path_with_system(&target).map_err(|e| format!("無法開啟報告：{}", e))
}

fn validate_image_source_path(file_path: &str) -> Result<(), String> {
    if file_path.starts_with("/dev/") || file_path.starts_with(r"\\.\") {
        return Err(
            "RAW_DEVICE_UNSUPPORTED: 請先建立唯讀映像檔，再將 .img 作為來源；桌面 App 不會開啟實體磁碟。"
                .to_string(),
        );
    }
    if !PathBuf::from(file_path).exists() {
        return Err(format!("找不到指定的來源映像檔：{}", file_path));
    }
    Ok(())
}

#[tauri::command]
async fn run_recovery_scan(
    app: tauri::AppHandle,
    file_path: String,
    mode: String,
    offset: u64,
    count: usize,
    job_id: String,
) -> Result<ScanResponse, String> {
    validate_image_source_path(&file_path)?;
    let operation_token = begin_operation(&job_id)?;
    emit_scan_progress(Some(&app), 1, 0, 0, 0, "正在讀取磁碟資料...");
    let scan_app = app.clone();
    let scan_file_path = file_path.clone();
    let scan_mode = mode.clone();
    let scan_token = Arc::clone(&operation_token);
    let scan_result = tauri::async_runtime::spawn_blocking(move || {
        perform_scan(
            &scan_file_path,
            &scan_mode,
            offset,
            count,
            Some(&scan_app),
            &scan_token,
        )
    })
    .await
    .map_err(|error| format!("掃描工作執行緒失敗：{}", error));
    finish_operation(&job_id);
    let scan_result = scan_result?;
    match scan_result {
        Ok(r) => {
            app.emit(
                "scan-progress",
                ScanProgressPayload {
                    pct: Some(100),
                    scanned: 0,
                    total: 0,
                    files_found: 0,
                    msg: "掃描完成".to_string(),
                },
            )
            .ok();
            Ok(prepare_scan_response(r))
        }
        Err(e) => Err(friendly_runtime_error("掃描", &file_path, &e)),
    }
}

#[tauri::command]
async fn get_scan_page(
    scan_id: String,
    cursor: usize,
    limit: usize,
) -> Result<ScanPageResponse, String> {
    let cache = scan_result_cache()
        .lock()
        .map_err(|_| "掃描結果快取目前無法使用。".to_string())?;
    let entries = cache
        .get(&scan_id)
        .ok_or_else(|| "掃描結果已過期，請重新執行掃描。".to_string())?;
    let safe_limit = limit.clamp(1, SCAN_PAGE_SIZE);
    let start = cursor.min(entries.len());
    let end = start.saturating_add(safe_limit).min(entries.len());
    Ok(ScanPageResponse {
        scan_id,
        entries: entries[start..end].to_vec(),
        total_entries: entries.len(),
        next_cursor: (end < entries.len()).then_some(end),
    })
}

#[tauri::command]
async fn run_recovery_extract(
    app: tauri::AppHandle,
    file_path: String,
    mode: String,
    offset: u64,
    selected_id: String,
    output_path: String,
    job_id: String,
) -> Result<String, String> {
    validate_image_source_path(&file_path)?;
    let operation_token = begin_operation(&job_id)?;
    app.emit("extract-progress", "正在讀取來源磁碟...").ok();
    let extract_file_path = file_path.clone();
    let extract_mode = mode.clone();
    let extract_selected_id = selected_id.clone();
    let extract_output_path = output_path.clone();
    let extract_token = Arc::clone(&operation_token);
    let extract_result = tauri::async_runtime::spawn_blocking(move || {
        perform_extract(
            &extract_file_path,
            &extract_mode,
            offset,
            &extract_selected_id,
            &extract_output_path,
            &extract_token,
        )
    })
    .await
    .map_err(|error| format!("還原工作執行緒失敗：{}", error));
    finish_operation(&job_id);
    let extract_result = extract_result?;
    match extract_result {
        Ok(msg) => {
            app.emit("extract-progress", "100% - 還原完成").ok();
            Ok(msg)
        }
        Err(e) => Err(friendly_runtime_error("還原", &file_path, &e)),
    }
}

#[tauri::command]
async fn run_recovery_extract_batch(
    app: tauri::AppHandle,
    file_path: String,
    mode: String,
    offset: u64,
    items: Vec<BatchExtractItem>,
    job_id: String,
) -> Result<Vec<String>, String> {
    validate_image_source_path(&file_path)?;
    if items.is_empty() {
        return Err("沒有選取任何要還原的檔案。".to_string());
    }
    let operation_token = begin_operation(&job_id)?;
    app.emit(
        "extract-progress",
        ExtractProgressPayload {
            pct: 1,
            done: 0,
            total: items.len(),
            current_file: "準備中...".to_string(),
        },
    )
    .ok();
    let req = BatchExtractRequest {
        device: file_path.clone(),
        mode: mode.clone(),
        offset,
        items,
    };
    let batch_app = app.clone();
    let batch_token = Arc::clone(&operation_token);
    let out = tauri::async_runtime::spawn_blocking(move || {
        perform_extract_batch(req, Some(&batch_app), &batch_token)
    })
    .await
    .map_err(|error| format!("批次還原工作執行緒失敗：{}", error));
    finish_operation(&job_id);
    let out = out?;
    if let Some(err) = out.error {
        return Err(friendly_runtime_error("批次還原", &file_path, &err));
    }
    app.emit(
        "extract-progress",
        ExtractProgressPayload {
            pct: 100,
            done: out.messages.len(),
            total: out.messages.len(),
            current_file: "批次還原完成".to_string(),
        },
    )
    .ok();
    Ok(out.messages)
}

#[tauri::command]
async fn cancel_current_operation(app: tauri::AppHandle, job_id: String) -> Result<String, String> {
    request_cancellation(&job_id)?;
    app.emit("scan-progress", "正在中止目前掃描 / 還原作業...")
        .ok();
    app.emit("extract-progress", "正在中止目前掃描 / 還原作業...")
        .ok();
    Ok("已送出中止要求；目前同步工作會在下一個安全檢查點停止。".to_string())
}

mod thumbnail;
// ── App entry point ───────────────────────────────────────────────────────────

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_log::Builder::default().build())
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            run_recovery_scan,
            get_scan_page,
            run_recovery_extract,
            run_recovery_extract_batch,
            get_recovery_default_dir,
            diagnostics::get_runtime_diagnostics,
            open_report_file,
            cancel_current_operation,
            get_imaging_workflow_plan,
            session::get_session_workflow_plan,
            session::create_recovery_session_direct,
            session::open_recovery_session_direct,
            session::refresh_recovery_session_index,
            session::get_session_doctor_report_direct,
            session::open_session_index_direct,
            session::import_session_artifact_direct,
            session::import_session_artifacts_direct,
            session::append_session_timeline_event_direct,
            session::open_session_timeline_direct,
            session::refresh_session_dashboard_direct,
            session::open_session_dashboard_direct,
            session::generate_session_handoff_reports_direct,
            session::open_session_customer_report_direct,
            session::open_session_engineer_report_direct,
            disks::list_disks,
            disks::detect_partitions,
            disks::check_output_isolation,
            thumbnail::read_file_thumbnail
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod tests {
    use super::{
        ScanEntryPayload, ScanResponse, begin_operation, check_output_isolation, finish_operation,
        get_scan_page, open_source, operation_cancelled, prepare_scan_response,
        request_cancellation, validate_report_open_target,
    };
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};

    fn temp_dir_name() -> PathBuf {
        static NEXT_ID: AtomicU64 = AtomicU64::new(0);
        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!("polik_tauri_test_{}_{id}", std::process::id()))
    }

    #[test]
    fn report_open_allows_generated_report_extensions() {
        let root = temp_dir_name();
        std::fs::create_dir_all(&root).unwrap();
        let report = root.join("quality_report.html");
        std::fs::write(&report, b"report").unwrap();

        assert_eq!(
            validate_report_open_target(&report).unwrap(),
            std::fs::canonicalize(&report).unwrap()
        );
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn report_open_allows_ordinary_directories() {
        let root = temp_dir_name();
        std::fs::create_dir_all(&root).unwrap();

        assert!(validate_report_open_target(&root).is_ok());
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn report_open_rejects_executables_and_app_bundles() {
        let root = temp_dir_name();
        let app = root.join("Untrusted.app");
        std::fs::create_dir_all(&app).unwrap();
        let executable = root.join("payload.command");
        std::fs::write(&executable, b"#!/bin/sh").unwrap();

        assert!(validate_report_open_target(&app).is_err());
        assert!(validate_report_open_target(&executable).is_err());
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn desktop_source_rejects_raw_device_paths_before_open() {
        let mac_error = open_source("/dev/disk999").unwrap_err();
        let windows_error = open_source(r"\\.\PhysicalDrive999").unwrap_err();
        assert!(mac_error.contains("RAW_DEVICE_UNSUPPORTED"));
        assert!(windows_error.contains("RAW_DEVICE_UNSUPPORTED"));
    }

    #[test]
    fn output_isolation_rejects_raw_source_before_resolver() {
        let result = tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(check_output_isolation(
                "/dev/disk4".to_string(),
                "/tmp/polik-recovery-output".to_string(),
            ))
            .unwrap();
        assert!(!result.ok);
        assert!(result.message.contains("實體磁碟不可直接掃描"));
    }

    #[test]
    fn large_scan_response_returns_first_page_and_cursor() {
        let response = ScanResponse {
            report: String::new(),
            vfs: String::new(),
            entries: (0..501)
                .map(|index| ScanEntryPayload {
                    path: format!("/file-{index}"),
                    kind: "File".to_string(),
                    status: "Allocated".to_string(),
                    size: "1".to_string(),
                    id: index.to_string(),
                    confidence: String::new(),
                    quality: String::new(),
                    tier: String::new(),
                    recovery_mode: String::new(),
                    bytes_lost: String::new(),
                    bytes_read: String::new(),
                    reason: String::new(),
                })
                .collect(),
            scan_id: String::new(),
            total_entries: 0,
            next_cursor: None,
        };

        let first_page = prepare_scan_response(response);
        assert_eq!(first_page.entries.len(), 500);
        assert_eq!(first_page.total_entries, 501);
        assert_eq!(first_page.next_cursor, Some(500));

        let scan_id = first_page.scan_id.clone();
        let second_page = tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(get_scan_page(scan_id, 500, 500))
            .unwrap();
        assert_eq!(second_page.entries.len(), 1);
        assert_eq!(second_page.entries[0].id, "500");
        assert_eq!(second_page.next_cursor, None);
    }

    #[test]
    fn operation_cancellation_is_scoped_to_matching_job_id() {
        let token = begin_operation("job-a").unwrap();
        assert!(!operation_cancelled());
        assert!(begin_operation("job-b").is_err());
        assert!(request_cancellation("job-b").is_err());
        request_cancellation("job-a").unwrap();
        assert!(token.load(Ordering::SeqCst));
        assert!(operation_cancelled());
        finish_operation("job-a");
        assert!(!operation_cancelled());
        assert!(begin_operation("job-b").is_ok());
        finish_operation("job-b");
    }
}
