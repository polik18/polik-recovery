use data_recovery_core::{BlockSource, FileBlockSource};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::Emitter;

const NTFS_QUICK_SCAN_COUNT: usize = 10_000;
const NTFS_STANDARD_FALLBACK_SCAN_COUNT: usize = 50_000;
const NTFS_MAX_SCAN_COUNT: usize = 1_000_000;
const NTFS_ADS_ID_MASK: u64 = 0x0000_FFFF_FFFF_FFFF;
const APFS_MAX_EXTRACT_BYTES: u64 = 2 * 1024 * 1024 * 1024;

static CANCEL_REQUESTED: AtomicBool = AtomicBool::new(false);

fn reset_cancellation() {
    CANCEL_REQUESTED.store(false, Ordering::SeqCst);
}

fn request_cancellation() {
    CANCEL_REQUESTED.store(true, Ordering::SeqCst);
}

fn operation_cancelled() -> bool {
    CANCEL_REQUESTED.load(Ordering::SeqCst)
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
struct ExtractProgressPayload {
    pct: u8,
    done: usize,
    total: usize,
    current_file: String,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct ScanResponse {
    report: String,
    vfs: String,
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

fn safe_image_slug(value: &str) -> String {
    let base = value
        .trim()
        .trim_start_matches("/dev/")
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.' {
                c
            } else {
                '_'
            }
        })
        .collect::<String>();
    let trimmed = base.trim_matches('_').trim_matches('.');
    if trimmed.is_empty() {
        "source".to_string()
    } else {
        trimmed.to_string()
    }
}

fn shell_quote(value: &str) -> String {
    let escaped = value.replace("'", "'\"'\"'");
    format!("'{}'", escaped)
}

fn imaging_stamp() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    secs.to_string()
}

fn diag(
    id: &str,
    title: &str,
    status: &str,
    detail: impl Into<String>,
    action: impl Into<String>,
) -> RuntimeDiagnosticCheck {
    RuntimeDiagnosticCheck {
        id: id.to_string(),
        title: title.to_string(),
        status: status.to_string(),
        detail: detail.into(),
        action: action.into(),
    }
}

fn session_default_root(output_root: &str) -> PathBuf {
    let raw = output_root.trim();
    if raw.is_empty() {
        if let Ok(home) = std::env::var("HOME") {
            return PathBuf::from(home)
                .join("Desktop")
                .join("Polik-Recovery-Sessions");
        }
        return PathBuf::from("/Users/Shared/Polik-Recovery-Sessions");
    }
    PathBuf::from(raw.trim_end_matches('/'))
}

fn session_required_dirs(session_dir: &std::path::Path) -> Vec<PathBuf> {
    vec![
        session_dir.join("00_intake"),
        session_dir.join("01_source_image"),
        session_dir.join("02_scan_reports"),
        session_dir.join("03_recovery_output"),
        session_dir.join("03_recovery_output/recovered"),
        session_dir.join("03_recovery_output/suspect"),
        session_dir.join("03_recovery_output/suspect/possible"),
        session_dir.join("03_recovery_output/suspect/partial"),
        session_dir.join("03_recovery_output/suspect/fragments"),
        session_dir.join("03_recovery_output/skipped"),
        session_dir.join("04_quality_reports"),
        session_dir.join("05_logs"),
        session_dir.join("06_handoff"),
        session_dir.join("07_timeline"),
        session_dir.join("08_dashboard"),
        session_dir.join("09_reports"),
        session_dir.join("10_audit"),
        session_dir.join("11_delivery"),
        session_dir.join("11_delivery/packages"),
        session_dir.join("11_delivery/staging"),
    ]
}

fn ensure_session_dirs(session_dir: &std::path::Path) -> Result<(), String> {
    if session_dir.to_string_lossy().starts_with("/dev/") {
        return Err(
            "Session 資料夾不能建立在 /dev/* 裝置；請選另一顆安全磁碟上的資料夾。".to_string(),
        );
    }
    for dir in session_required_dirs(session_dir) {
        std::fs::create_dir_all(&dir)
            .map_err(|e| format!("無法建立 Session 資料夾 {}：{}", dir.display(), e))?;
    }
    Ok(())
}

fn manifest_text_value(manifest: &serde_json::Value, key: &str) -> String {
    manifest
        .get(key)
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string()
}

fn session_manifest_path(session_dir: &std::path::Path) -> PathBuf {
    session_dir.join("session_manifest.json")
}
fn session_index_path(session_dir: &std::path::Path) -> PathBuf {
    session_dir.join("session_index.html")
}
fn session_timeline_dir(session_dir: &std::path::Path) -> PathBuf {
    session_dir.join("07_timeline")
}
fn session_timeline_json_path(session_dir: &std::path::Path) -> PathBuf {
    session_timeline_dir(session_dir).join("session_timeline.json")
}
fn session_timeline_html_path(session_dir: &std::path::Path) -> PathBuf {
    session_timeline_dir(session_dir).join("session_timeline.html")
}
fn session_dashboard_dir(session_dir: &std::path::Path) -> PathBuf {
    session_dir.join("08_dashboard")
}
fn session_dashboard_json_path(session_dir: &std::path::Path) -> PathBuf {
    session_dashboard_dir(session_dir).join("session_dashboard.json")
}
fn session_dashboard_html_path(session_dir: &std::path::Path) -> PathBuf {
    session_dashboard_dir(session_dir).join("session_dashboard.html")
}
fn session_reports_dir(session_dir: &std::path::Path) -> PathBuf {
    session_dir.join("09_reports")
}
fn session_customer_report_html_path(session_dir: &std::path::Path) -> PathBuf {
    session_reports_dir(session_dir).join("customer_summary.html")
}
fn session_engineer_report_html_path(session_dir: &std::path::Path) -> PathBuf {
    session_reports_dir(session_dir).join("engineer_handoff.html")
}
#[allow(dead_code)]
fn session_customer_report_pdf_path(session_dir: &std::path::Path) -> PathBuf {
    session_reports_dir(session_dir).join("customer_summary.pdf")
}
fn session_audit_dir(session_dir: &std::path::Path) -> PathBuf {
    session_dir.join("10_audit")
}
#[allow(dead_code)]
fn session_audit_html_path(session_dir: &std::path::Path) -> PathBuf {
    session_audit_dir(session_dir).join("sample_audit.html")
}
fn session_delivery_dir(session_dir: &std::path::Path) -> PathBuf {
    session_dir.join("11_delivery")
}
fn session_delivery_packages_dir(session_dir: &std::path::Path) -> PathBuf {
    session_delivery_dir(session_dir).join("packages")
}

fn normalize_artifact_kind(path: &std::path::Path, requested: Option<&str>) -> (String, String) {
    let req = requested.unwrap_or("auto").trim().to_ascii_lowercase();
    let filename = path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    let ext = path
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    let kind = if !req.is_empty() && req != "auto" {
        req
    } else if filename.contains("scan") || filename.contains("vfs") {
        "scan_report".to_string()
    } else if filename.contains("quality") || filename.contains("gate") {
        "quality_report".to_string()
    } else if filename.contains("recovery_report") || filename.contains("recover") {
        "recovery_report".to_string()
    } else if filename.contains("map_report")
        || ext == "map"
        || filename.ends_with(".map_report.json")
        || filename.ends_with(".map_report.html")
    {
        "ddrescue_map".to_string()
    } else if filename.contains("manifest") || matches!(ext.as_str(), "img" | "dmg" | "raw" | "bin")
    {
        "source_image".to_string()
    } else if ext == "log" || filename.contains("log") {
        "log".to_string()
    } else if filename.contains("codex")
        || filename.contains("handoff")
        || filename.contains("validation")
        || filename.contains("artifact")
    {
        "handoff".to_string()
    } else {
        "intake".to_string()
    };
    let rel = match kind.as_str() {
        "source_image" | "image_manifest" | "image" | "ddrescue_map" | "map_report" => {
            "01_source_image"
        }
        "scan_report" | "vfs" => "02_scan_reports",
        "recovery_report" | "recovery_output" => "03_recovery_output",
        "quality_report" | "quality" => "04_quality_reports",
        "log" | "logs" => "05_logs",
        "handoff" | "codex" | "validation" => "06_handoff",
        _ => "00_intake",
    };
    (kind, rel.to_string())
}

fn unique_destination_path(dir: &std::path::Path, original_name: &str) -> PathBuf {
    let clean = if original_name.trim().is_empty() {
        "artifact"
    } else {
        original_name
    };
    let mut dest = dir.join(clean);
    if !dest.exists() {
        return dest;
    }
    let stamp = imaging_stamp();
    let stem = std::path::Path::new(clean)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("artifact");
    let ext = std::path::Path::new(clean)
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("");
    let name = if ext.is_empty() {
        format!("{}_imported_{}", stem, stamp)
    } else {
        format!("{}_imported_{}.{}", stem, stamp, ext)
    };
    dest = dir.join(name);
    dest
}

fn copy_dir_recursive(src: &std::path::Path, dst: &std::path::Path) -> Result<(), String> {
    std::fs::create_dir_all(dst).map_err(|e| format!("無法建立資料夾 {}：{}", dst.display(), e))?;
    for entry in
        std::fs::read_dir(src).map_err(|e| format!("無法讀取資料夾 {}：{}", src.display(), e))?
    {
        let entry = entry.map_err(|e| format!("無法讀取資料夾項目：{}", e))?;
        let from = entry.path();
        let to = dst.join(entry.file_name());
        if from.is_dir() {
            copy_dir_recursive(&from, &to)?;
        } else {
            std::fs::copy(&from, &to)
                .map_err(|e| format!("無法複製 {} 到 {}：{}", from.display(), to.display(), e))?;
        }
    }
    Ok(())
}

fn import_session_artifact_impl(
    session_dir: &std::path::Path,
    source_path: &std::path::Path,
    artifact_kind: Option<&str>,
) -> Result<DesktopSessionArtifactImportResult, String> {
    if !session_dir.is_dir() {
        return Err(format!("找不到 Session 資料夾：{}", session_dir.display()));
    }
    ensure_session_dirs(session_dir)?;
    if !session_manifest_path(session_dir).is_file() {
        return Err("這個資料夾缺少 session_manifest.json；請先建立或開啟 Session。".to_string());
    }
    if !source_path.exists() {
        return Err(format!(
            "找不到要匯入的檔案或資料夾：{}",
            source_path.display()
        ));
    }
    if source_path.to_string_lossy().starts_with("/dev/") {
        return Err(
            "不能把 /dev/* 裝置直接當 artifact 匯入；請先建立 image 或報告檔。".to_string(),
        );
    }
    let (kind, rel) = normalize_artifact_kind(source_path, artifact_kind);
    let target_dir = session_dir.join(&rel);
    std::fs::create_dir_all(&target_dir).map_err(|e| {
        format!(
            "無法建立 artifact 目標資料夾 {}：{}",
            target_dir.display(),
            e
        )
    })?;
    let original_name = source_path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("artifact");
    let dest = unique_destination_path(&target_dir, original_name);
    if source_path.is_dir() {
        copy_dir_recursive(source_path, &dest)?;
    } else {
        std::fs::copy(source_path, &dest).map_err(|e| {
            format!(
                "無法複製 artifact {} 到 {}：{}",
                source_path.display(),
                dest.display(),
                e
            )
        })?;
    }
    append_session_artifact_manifest(session_dir, &kind, source_path, &dest, &rel)?;
    let _ = append_desktop_session_timeline_event(
        session_dir,
        DesktopTimelineEvent {
            event_type: "artifact_imported",
            title: "匯入 Session Artifact",
            status: "success",
            summary: &format!("已匯入 {} 到 {}", kind, rel),
            actor: "desktop_app",
            artifacts: vec![dest.to_string_lossy().to_string()],
            details: serde_json::json!({"kind": kind, "target_folder": rel}),
        },
    );
    write_desktop_session_index(session_dir)?;
    Ok(DesktopSessionArtifactImportResult {
        ok: true,
        session_dir: session_dir.to_string_lossy().to_string(),
        artifact_kind: kind.clone(),
        source_path: source_path.to_string_lossy().to_string(),
        destination_path: dest.to_string_lossy().to_string(),
        target_folder: rel,
        copied: true,
        message: format!("已匯入 {} 到 {}", kind, dest.display()),
    })
}

fn append_session_artifact_manifest(
    session_dir: &std::path::Path,
    kind: &str,
    source: &std::path::Path,
    dest: &std::path::Path,
    target_folder: &str,
) -> Result<(), String> {
    let path = session_manifest_path(session_dir);
    let mut manifest = read_session_manifest(session_dir)?;
    let stamp = imaging_stamp();
    manifest["updated_at_unix"] = serde_json::Value::String(stamp.clone());
    let entry = serde_json::json!({
        "kind": kind,
        "source_path": source.to_string_lossy(),
        "destination_path": dest.to_string_lossy(),
        "target_folder": target_folder,
        "imported_at_unix": stamp
    });
    if !manifest
        .get("artifacts")
        .map(|v| v.is_array())
        .unwrap_or(false)
    {
        manifest["artifacts"] = serde_json::Value::Array(Vec::new());
    }
    if let Some(arr) = manifest.get_mut("artifacts").and_then(|v| v.as_array_mut()) {
        arr.push(entry);
    }
    std::fs::write(
        &path,
        serde_json::to_string_pretty(&manifest).unwrap_or_else(|_| "{}".to_string()),
    )
    .map_err(|e| format!("無法更新 Session manifest：{}", e))
}

fn read_session_manifest(session_dir: &std::path::Path) -> Result<serde_json::Value, String> {
    let path = session_manifest_path(session_dir);
    let text = std::fs::read_to_string(&path)
        .map_err(|e| format!("無法讀取 Session manifest {}：{}", path.display(), e))?;
    serde_json::from_str(&text).map_err(|e| format!("Session manifest JSON 無法解析：{}", e))
}

fn write_session_manifest_value(
    session_dir: &std::path::Path,
    manifest: &serde_json::Value,
) -> Result<(), String> {
    std::fs::write(
        session_manifest_path(session_dir),
        serde_json::to_string_pretty(manifest).unwrap_or_else(|_| "{}".to_string()),
    )
    .map_err(|e| format!("無法寫入 Session manifest：{}", e))
}

fn write_desktop_session_timeline_files(
    session_dir: &std::path::Path,
    manifest: &serde_json::Value,
) -> Result<(), String> {
    std::fs::create_dir_all(session_timeline_dir(session_dir))
        .map_err(|e| format!("無法建立 timeline 資料夾：{}", e))?;
    let events = manifest
        .get("timeline")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    let case_id = manifest_text_value(manifest, "case_id");
    let payload = serde_json::json!({
        "schema": "polik.recovery.session.timeline.v2.3",
        "case_id": case_id,
        "session_dir": session_dir.to_string_lossy(),
        "event_count": events.len(),
        "events": events
    });
    std::fs::write(
        session_timeline_json_path(session_dir),
        serde_json::to_string_pretty(&payload).unwrap_or_else(|_| "{}".to_string()),
    )
    .map_err(|e| format!("無法寫入 session_timeline.json：{}", e))?;
    let mut rows = String::new();
    if let Some(arr) = payload.get("events").and_then(|v| v.as_array()) {
        for item in arr {
            let seq = item.get("sequence").and_then(|v| v.as_u64()).unwrap_or(0);
            let ts = item
                .get("created_at")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let status = item
                .get("status")
                .and_then(|v| v.as_str())
                .unwrap_or("info");
            let title = item.get("title").and_then(|v| v.as_str()).unwrap_or("");
            let summary = item.get("summary").and_then(|v| v.as_str()).unwrap_or("");
            rows.push_str(&format!("<article class='event {}'><div class='time'>#{} · {}</div><h2>{}</h2><p>{}</p></article>", html_escape(status), seq, html_escape(ts), html_escape(title), html_escape(summary)));
        }
    }
    if rows.is_empty() {
        rows.push_str("<p class='muted'>尚無時間線紀錄。</p>");
    }
    let doc = format!(
        r#"<!doctype html><html lang="zh-Hant"><head><meta charset="utf-8"><title>Polik Session Timeline {}</title><style>body{{font-family:-apple-system,BlinkMacSystemFont,'Noto Sans TC',sans-serif;background:#f8fafc;color:#172033;line-height:1.65;margin:32px}}.event{{background:#fff;border:1px solid #e5e7eb;border-left:8px solid #38bdf8;border-radius:16px;padding:16px;margin:14px 0;box-shadow:0 8px 22px rgba(15,23,42,.06)}}.event.success{{border-left-color:#22c55e}}.event.warning{{border-left-color:#f59e0b}}.event.error{{border-left-color:#ef4444}}.time,.muted{{color:#64748b;font-size:12px}}h2{{margin:6px 0}}</style></head><body><h1>Polik Recovery Session Timeline</h1><p class='muted'>Case：<code>{}</code> ｜ Session：<code>{}</code></p>{}</body></html>"#,
        html_escape(&case_id),
        html_escape(&case_id),
        html_escape(session_dir.to_string_lossy().as_ref()),
        rows
    );
    std::fs::write(session_timeline_html_path(session_dir), doc)
        .map_err(|e| format!("無法寫入 session_timeline.html：{}", e))
}

struct DesktopTimelineEvent<'a> {
    event_type: &'a str,
    title: &'a str,
    status: &'a str,
    summary: &'a str,
    actor: &'a str,
    artifacts: Vec<String>,
    details: serde_json::Value,
}

fn append_desktop_session_timeline_event(
    session_dir: &std::path::Path,
    event: DesktopTimelineEvent<'_>,
) -> Result<DesktopSessionTimelineResult, String> {
    let DesktopTimelineEvent {
        event_type,
        title,
        status,
        summary,
        actor,
        artifacts,
        details,
    } = event;
    ensure_session_dirs(session_dir)?;
    let mut manifest = read_session_manifest(session_dir)?;
    let mut events = manifest
        .get("timeline")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    let seq = events.len() + 1;
    let stamp = imaging_stamp();
    let event_id = format!("evt-{}-{:04}", stamp, seq);
    let event = serde_json::json!({
        "id": event_id,
        "sequence": seq,
        "created_at_unix": stamp,
        "created_at": stamp,
        "event_type": event_type,
        "title": title,
        "status": status,
        "summary": summary,
        "actor": actor,
        "artifacts": artifacts,
        "details": details
    });
    events.push(event);
    manifest["timeline"] = serde_json::Value::Array(events.clone());
    manifest["timeline_count"] = serde_json::Value::Number(serde_json::Number::from(events.len()));
    manifest["updated_at_unix"] = serde_json::Value::String(imaging_stamp());
    if let Some(obj) = manifest.get_mut("folders").and_then(|v| v.as_object_mut()) {
        obj.insert(
            "timeline".to_string(),
            serde_json::Value::String(
                session_timeline_dir(session_dir)
                    .to_string_lossy()
                    .to_string(),
            ),
        );
    }
    write_session_manifest_value(session_dir, &manifest)?;
    write_desktop_session_timeline_files(session_dir, &manifest)?;
    Ok(DesktopSessionTimelineResult {
        ok: true,
        session_dir: session_dir.to_string_lossy().to_string(),
        event_id: event_id.clone(),
        timeline_html: session_timeline_html_path(session_dir)
            .to_string_lossy()
            .to_string(),
        timeline_json: session_timeline_json_path(session_dir)
            .to_string_lossy()
            .to_string(),
        message: format!("已寫入 timeline 事件：{}", title),
    })
}

fn desktop_session_state(
    session_dir: &std::path::Path,
    created: bool,
) -> Result<DesktopSessionState, String> {
    let manifest = read_session_manifest(session_dir)?;
    let case_id = manifest_text_value(&manifest, "case_id");
    let source = manifest_text_value(&manifest, "source");
    let mode = manifest_text_value(&manifest, "mode");
    let recovery_output_dir = session_dir.join("03_recovery_output");
    let artifact_count = manifest
        .get("artifacts")
        .and_then(|v| v.as_array())
        .map(|a| a.len())
        .unwrap_or(0);
    let timeline_count = manifest
        .get("timeline")
        .and_then(|v| v.as_array())
        .map(|a| a.len())
        .unwrap_or(0);
    let warnings = manifest
        .get("warnings")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|x| x.as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_else(Vec::new);
    Ok(DesktopSessionState {
        active: true,
        created,
        case_id,
        session_dir: session_dir.to_string_lossy().to_string(),
        manifest_path: session_manifest_path(session_dir).to_string_lossy().to_string(),
        index_path: session_index_path(session_dir).to_string_lossy().to_string(),
        source,
        mode,
        recovery_output_dir: recovery_output_dir.to_string_lossy().to_string(),
        recovered_dir: recovery_output_dir.join("recovered").to_string_lossy().to_string(),
        suspect_dir: recovery_output_dir.join("suspect").to_string_lossy().to_string(),
        image_dir: session_dir.join("01_source_image").to_string_lossy().to_string(),
        scan_reports_dir: session_dir.join("02_scan_reports").to_string_lossy().to_string(),
        quality_reports_dir: session_dir.join("04_quality_reports").to_string_lossy().to_string(),
        logs_dir: session_dir.join("05_logs").to_string_lossy().to_string(),
        handoff_dir: session_dir.join("06_handoff").to_string_lossy().to_string(),
        timeline_dir: session_timeline_dir(session_dir).to_string_lossy().to_string(),
        timeline_count,
        dashboard_dir: session_dashboard_dir(session_dir).to_string_lossy().to_string(),
        dashboard_html: session_dashboard_html_path(session_dir).to_string_lossy().to_string(),
        dashboard_json: session_dashboard_json_path(session_dir).to_string_lossy().to_string(),
        dashboard_score: manifest.get("dashboard").and_then(|d| d.get("health")).and_then(|h| h.get("score")).and_then(|v| v.as_u64()).unwrap_or(0) as usize,
        reports_dir: session_reports_dir(session_dir).to_string_lossy().to_string(),
        customer_report_html: session_customer_report_html_path(session_dir).to_string_lossy().to_string(),
        engineer_report_html: session_engineer_report_html_path(session_dir).to_string_lossy().to_string(),
        artifact_count,
        warnings,
        next_step: "已啟用 Session；建議先把來源 image/map 放到 01_source_image，再掃描並還原到 03_recovery_output。".to_string(),
    })
}

fn write_desktop_session_readme(
    session_dir: &std::path::Path,
    case_id: &str,
    source: &str,
    mode: &str,
) -> Result<(), String> {
    let text = format!("# Polik Recovery Session\n\n此資料夾是 Desktop App 直接建立/開啟的救援案件。請把同一次救援的 image、map、scan、recovery、quality、log 與 Codex 交接資料都集中在這裡。\n\n- Case ID：`{}`\n- Source：`{}`\n- Mode：`{}`\n\n## App 工作流\n\n1. 來源是實體磁碟時，先在 `01_source_image/` 建立 `.img` 與 ddrescue `.map`。\n2. 掃描 `.img`，掃描報告放 `02_scan_reports/`。\n3. 還原輸出固定指向 `03_recovery_output/`。\n4. 完整檔案放 `03_recovery_output/recovered/`，可疑檔案放 `03_recovery_output/suspect/`。\n5. 重新產生 `session_index.html`、`08_dashboard/session_dashboard.html`、`09_reports/` 報告與 `10_audit/` 抽查紀錄。
6. 交付客戶前，用 v2.7 建立 `11_delivery/packages/*.zip`，避免把 raw image、工程 log 或工程版報告混進客戶包。\n\n## macOS + Codex 待驗證\n\n此專案曾在非 macOS、無 Rust/Cargo/Tauri CLI 的環境修改；請於 macOS 上依 `docs/plans/macos-codex-validation-plan.md` 執行實機驗證。\n", case_id, source, mode);
    std::fs::write(session_dir.join("SESSION_README.md"), text)
        .map_err(|e| format!("無法寫入 SESSION_README.md：{}", e))
}

fn count_session_files_recursive(dir: &std::path::Path) -> usize {
    fn walk(p: &std::path::Path, n: &mut usize) {
        if let Ok(rd) = std::fs::read_dir(p) {
            for entry in rd.filter_map(Result::ok) {
                let path = entry.path();
                if path.is_dir() {
                    walk(&path, n);
                } else {
                    *n += 1;
                }
            }
        }
    }
    let mut n = 0usize;
    walk(dir, &mut n);
    n
}

fn size_session_files_recursive(dir: &std::path::Path) -> u64 {
    fn walk(p: &std::path::Path, n: &mut u64) {
        if let Ok(rd) = std::fs::read_dir(p) {
            for entry in rd.filter_map(Result::ok) {
                let path = entry.path();
                if path.is_dir() {
                    walk(&path, n);
                } else if let Ok(meta) = std::fs::metadata(&path) {
                    *n += meta.len();
                }
            }
        }
    }
    let mut n = 0u64;
    walk(dir, &mut n);
    n
}

fn write_desktop_session_dashboard(
    session_dir: &std::path::Path,
) -> Result<DesktopSessionDashboardResult, String> {
    ensure_session_dirs(session_dir)?;
    std::fs::create_dir_all(session_dashboard_dir(session_dir))
        .map_err(|e| format!("無法建立 dashboard 資料夾：{}", e))?;
    let mut manifest = read_session_manifest(session_dir)?;
    let case_id = manifest_text_value(&manifest, "case_id");
    let source = manifest_text_value(&manifest, "source");
    let mode = manifest_text_value(&manifest, "mode");
    let source_count = count_session_files_recursive(&session_dir.join("01_source_image"));
    let scan_count = count_session_files_recursive(&session_dir.join("02_scan_reports"));
    let recovered_count =
        count_session_files_recursive(&session_dir.join("03_recovery_output/recovered"));
    let possible_count =
        count_session_files_recursive(&session_dir.join("03_recovery_output/suspect/possible"));
    let partial_count =
        count_session_files_recursive(&session_dir.join("03_recovery_output/suspect/partial"));
    let fragments_count =
        count_session_files_recursive(&session_dir.join("03_recovery_output/suspect/fragments"));
    let quality_count = count_session_files_recursive(&session_dir.join("04_quality_reports"));
    let log_count = count_session_files_recursive(&session_dir.join("05_logs"));
    let handoff_count = count_session_files_recursive(&session_dir.join("06_handoff"));
    let timeline_count = manifest
        .get("timeline")
        .and_then(|v| v.as_array())
        .map(|a| a.len())
        .unwrap_or(0);
    let artifact_count = manifest
        .get("artifacts")
        .and_then(|v| v.as_array())
        .map(|a| a.len())
        .unwrap_or(0);
    let suspect_count = possible_count + partial_count + fragments_count;
    let mut score = 20usize;
    if source_count > 0 {
        score += 25;
    }
    if scan_count > 0 {
        score += 20;
    }
    if recovered_count + suspect_count > 0 {
        score += 20;
    }
    if quality_count > 0 {
        score += 15;
    }
    if suspect_count > recovered_count && suspect_count > 0 {
        score = score.saturating_sub(10);
    }
    if score > 100 {
        score = 100;
    }
    let label = if score >= 80 {
        "案件資料完整"
    } else if score >= 55 {
        "可繼續處理"
    } else {
        "資料不足"
    }
    .to_string();
    let level = if score >= 80 {
        "good"
    } else if score >= 55 {
        "warn"
    } else {
        "bad"
    };
    let mut next_actions = Vec::new();
    if source_count == 0 {
        next_actions.push(
            "尚未看到 source image / manifest / ddrescue map；實體磁碟請先建立 .img。".to_string(),
        );
    }
    if scan_count == 0 {
        next_actions.push("尚未看到掃描報告；掃描後請匯入 02_scan_reports。".to_string());
    }
    if recovered_count + suspect_count == 0 {
        next_actions.push("尚未看到還原輸出；請確認輸出位置為 03_recovery_output。".to_string());
    }
    if quality_count == 0 {
        next_actions
            .push("尚未看到品質報告；建議匯入 quality gate 或 Codex 驗證結果。".to_string());
    }
    if next_actions.is_empty() {
        next_actions
            .push("案件資料看起來完整；請檢查 timeline 與最新 quality report 後交接。".to_string());
    }
    let payload = serde_json::json!({
        "schema": "polik.recovery.session.dashboard.v2.4.desktop",
        "case_id": case_id.clone(),
        "session_dir": session_dir.to_string_lossy(),
        "source": source.clone(),
        "mode": mode.clone(),
        "health": {"score": score, "label": label.clone(), "level": level},
        "counts": {
            "source_image": source_count,
            "scan_reports": scan_count,
            "recovered": recovered_count,
            "suspect_possible": possible_count,
            "suspect_partial": partial_count,
            "suspect_fragments": fragments_count,
            "quality_reports": quality_count,
            "logs": log_count,
            "handoff": handoff_count,
            "timeline": timeline_count,
            "artifacts": artifact_count
        },
        "sizes": {
            "recovered_bytes": size_session_files_recursive(&session_dir.join("03_recovery_output/recovered")),
            "suspect_bytes": size_session_files_recursive(&session_dir.join("03_recovery_output/suspect")),
            "source_image_bytes": size_session_files_recursive(&session_dir.join("01_source_image"))
        },
        "next_actions": next_actions,
    });
    std::fs::write(
        session_dashboard_json_path(session_dir),
        serde_json::to_string_pretty(&payload).unwrap_or_else(|_| "{}".to_string()),
    )
    .map_err(|e| format!("無法寫入 dashboard JSON：{}", e))?;
    let actions_html = payload
        .get("next_actions")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .map(|a| format!("<li>{}</li>", html_escape(a.as_str().unwrap_or(""))))
                .collect::<Vec<_>>()
                .join("")
        })
        .unwrap_or_default();
    let doc = format!(
        r#"<!doctype html><html lang="zh-Hant"><head><meta charset="utf-8"><title>Polik Session Dashboard {case_id}</title><style>body{{font-family:-apple-system,BlinkMacSystemFont,'Noto Sans TC',sans-serif;background:#f8fafc;color:#172033;margin:32px;line-height:1.6}}.grid{{display:grid;grid-template-columns:repeat(auto-fit,minmax(180px,1fr));gap:12px}}.card,.metric{{background:#fff;border:1px solid #e5e7eb;border-radius:16px;padding:16px;margin:12px 0;box-shadow:0 8px 22px rgba(15,23,42,.06)}}.metric strong{{font-size:28px;display:block}}.good{{border-left:8px solid #22c55e}}.warn{{border-left:8px solid #f59e0b}}.bad{{border-left:8px solid #ef4444}}table{{border-collapse:collapse;width:100%}}th,td{{border:1px solid #e5e7eb;padding:10px;text-align:left}}th{{background:#f1f5f9}}code{{font-family:ui-monospace,SFMono-Regular,Menlo,monospace}}</style></head><body><h1>Polik Recovery Session Dashboard</h1><p>Case：<code>{case_id}</code> ｜ Source：<code>{source}</code> ｜ Mode：{mode}</p><p><a href="../session_index.html">Session Index</a> ｜ <a href="../07_timeline/session_timeline.html">Timeline</a></p><div class="grid"><div class="metric {level}"><span>健康分數</span><strong>{score}/100</strong><small>{label}</small></div><div class="metric good"><span>完整還原</span><strong>{recovered_count}</strong><small>recovered/</small></div><div class="metric warn"><span>可疑 / partial</span><strong>{suspect_count}</strong><small>suspect/</small></div><div class="metric"><span>掃描報告</span><strong>{scan_count}</strong><small>02_scan_reports</small></div><div class="metric"><span>Quality</span><strong>{quality_count}</strong><small>04_quality_reports</small></div><div class="metric"><span>Timeline</span><strong>{timeline_count}</strong><small>events</small></div></div><div class="card"><h2>下一步建議</h2><ul>{actions_html}</ul></div><div class="card"><h2>資料夾統計</h2><table><tr><th>項目</th><th>數量</th></tr><tr><td>source image / map</td><td>{source_count}</td></tr><tr><td>scan reports</td><td>{scan_count}</td></tr><tr><td>recovered</td><td>{recovered_count}</td></tr><tr><td>suspect possible</td><td>{possible_count}</td></tr><tr><td>suspect partial</td><td>{partial_count}</td></tr><tr><td>suspect fragments</td><td>{fragments_count}</td></tr><tr><td>quality reports</td><td>{quality_count}</td></tr><tr><td>logs</td><td>{log_count}</td></tr><tr><td>handoff</td><td>{handoff_count}</td></tr></table></div></body></html>"#,
        case_id = html_escape(&case_id),
        source = html_escape(&source),
        mode = html_escape(&mode),
        level = level,
        score = score,
        label = html_escape(&label),
        recovered_count = recovered_count,
        suspect_count = suspect_count,
        scan_count = scan_count,
        quality_count = quality_count,
        timeline_count = timeline_count,
        actions_html = actions_html,
        source_count = source_count,
        possible_count = possible_count,
        partial_count = partial_count,
        fragments_count = fragments_count,
        log_count = log_count,
        handoff_count = handoff_count
    );
    std::fs::write(session_dashboard_html_path(session_dir), doc)
        .map_err(|e| format!("無法寫入 dashboard HTML：{}", e))?;
    manifest["dashboard"] = serde_json::json!({"html": session_dashboard_html_path(session_dir).to_string_lossy(), "json": session_dashboard_json_path(session_dir).to_string_lossy(), "health": {"score": score, "label": label.clone(), "level": level}});
    if let Some(obj) = manifest.get_mut("folders").and_then(|v| v.as_object_mut()) {
        obj.insert(
            "dashboard".to_string(),
            serde_json::Value::String(
                session_dashboard_dir(session_dir)
                    .to_string_lossy()
                    .to_string(),
            ),
        );
    }
    std::fs::write(
        session_manifest_path(session_dir),
        serde_json::to_string_pretty(&manifest).unwrap_or_else(|_| "{}".to_string()),
    )
    .map_err(|e| format!("無法更新 dashboard manifest：{}", e))?;
    Ok(DesktopSessionDashboardResult {
        ok: true,
        session_dir: session_dir.to_string_lossy().to_string(),
        dashboard_html: session_dashboard_html_path(session_dir)
            .to_string_lossy()
            .to_string(),
        dashboard_json: session_dashboard_json_path(session_dir)
            .to_string_lossy()
            .to_string(),
        score,
        label,
        message: "已重新產生 Session Dashboard。".to_string(),
    })
}

fn write_desktop_session_handoff_reports(
    session_dir: &std::path::Path,
) -> Result<DesktopSessionHandoffReportResult, String> {
    ensure_session_dirs(session_dir)?;
    let _ = write_desktop_session_dashboard(session_dir);
    std::fs::create_dir_all(session_reports_dir(session_dir))
        .map_err(|e| format!("無法建立 09_reports 資料夾：{}", e))?;
    let mut manifest = read_session_manifest(session_dir)?;
    let case_id = manifest_text_value(&manifest, "case_id");
    let source = manifest_text_value(&manifest, "source");
    let mode = manifest_text_value(&manifest, "mode");
    let recovered_count =
        count_session_files_recursive(&session_dir.join("03_recovery_output/recovered"));
    let possible_count =
        count_session_files_recursive(&session_dir.join("03_recovery_output/suspect/possible"));
    let partial_count =
        count_session_files_recursive(&session_dir.join("03_recovery_output/suspect/partial"));
    let fragments_count =
        count_session_files_recursive(&session_dir.join("03_recovery_output/suspect/fragments"));
    let suspect_count = possible_count + partial_count + fragments_count;
    let skipped_count =
        count_session_files_recursive(&session_dir.join("03_recovery_output/skipped"));
    let scan_count = count_session_files_recursive(&session_dir.join("02_scan_reports"));
    let quality_count = count_session_files_recursive(&session_dir.join("04_quality_reports"));
    let timeline_count = manifest
        .get("timeline")
        .and_then(|v| v.as_array())
        .map(|a| a.len())
        .unwrap_or(0);
    let recovered_bytes =
        size_session_files_recursive(&session_dir.join("03_recovery_output/recovered"));
    let suspect_bytes =
        size_session_files_recursive(&session_dir.join("03_recovery_output/suspect"));
    let outcome = if recovered_count > 0 && suspect_count == 0 {
        "可交付：已有完整還原檔案"
    } else if recovered_count > 0 && suspect_count > 0 {
        "部分可交付：完整檔案與可疑檔案已分流"
    } else if recovered_count == 0 && suspect_count > 0 {
        "僅可疑結果：需要人工檢查或再次掃描"
    } else {
        "資料不足：尚未完成掃描或還原"
    };
    let generated = imaging_stamp();
    let customer_json = serde_json::json!({
        "schema": "polik.recovery.customer_summary.v2.5.desktop",
        "generated_at_unix": generated.clone(),
        "case_id": case_id.clone(),
        "outcome": outcome,
        "counts": {"complete_files": recovered_count, "complete_bytes": recovered_bytes, "suspect_files": suspect_count, "suspect_bytes": suspect_bytes, "skipped_files": skipped_count, "scan_reports": scan_count, "quality_reports": quality_count, "timeline_events": timeline_count},
        "notes": [
            "完整檔案與可疑檔案已分流，避免把殘片誤當成成功救援。",
            "可疑/partial 檔案可能可以修復，也可能無法開啟，建議人工抽查。",
            "若來源是實體磁碟，建議優先檢查 image manifest 與 ddrescue map 報告。"
        ]
    });
    let engineer_json = serde_json::json!({
        "schema": "polik.recovery.engineer_handoff.v2.5.desktop",
        "generated_at_unix": generated.clone(),
        "case_id": customer_json.get("case_id").and_then(|v| v.as_str()).unwrap_or(""),
        "session_dir": session_dir.to_string_lossy(),
        "source": source.clone(),
        "mode": mode.clone(),
        "outcome": outcome,
        "counts": customer_json.get("counts"),
        "folders": {"source_image":"01_source_image", "scan_reports":"02_scan_reports", "recovery_output":"03_recovery_output", "quality_reports":"04_quality_reports", "logs":"05_logs", "handoff":"06_handoff", "timeline":"07_timeline", "dashboard":"08_dashboard", "reports":"09_reports", "audit":"10_audit"},
        "macos_codex_validation": ["make codex-bootstrap", "make codex-validation-quick", "make codex-validation-full", "make quality-gate-all", "cargo tauri build"],
        "note": "此專案曾在非 macOS、無 Rust/Cargo/Tauri CLI 的環境修改；搬到 macOS 後請先跑驗證流程。"
    });
    let customer_html = format!(
        r#"<!doctype html><html lang="zh-Hant"><head><meta charset="utf-8"><title>資料救援案件摘要 {case_id}</title><style>body{{font-family:-apple-system,BlinkMacSystemFont,'Noto Sans TC',sans-serif;background:#f8fafc;color:#172033;line-height:1.7;margin:32px}}main{{max-width:920px;margin:auto;background:#fff;border:1px solid #e5e7eb;border-radius:18px;padding:26px;box-shadow:0 12px 30px rgba(15,23,42,.08)}}table{{border-collapse:collapse;width:100%;margin:16px 0}}th,td{{border:1px solid #e5e7eb;padding:10px;text-align:left}}th{{background:#f1f5f9}}.ok{{color:#166534}}.warn{{color:#92400e}}</style></head><body><main><h1>資料救援案件摘要（客戶版）</h1><table><tr><th>案件編號</th><td>{case_id}</td></tr><tr><th>目前判定</th><td>{outcome}</td></tr><tr><th>完整檔案</th><td>{recovered_count} 個 / {recovered_bytes} bytes</td></tr><tr><th>可疑或部分檔案</th><td>{suspect_count} 個 / {suspect_bytes} bytes</td></tr><tr><th>略過項目</th><td>{skipped_count}</td></tr></table><h2>說明</h2><ul><li>完整檔案會放在 recovered/。</li><li>可疑或部分檔案會放在 suspect/，不會混入完整檔案。</li><li>可疑檔案可能可以修復，也可能無法開啟，建議抽查後再交付。</li></ul><h2>建議下一步</h2><p>請先抽查完整檔案是否能開啟；若完整檔案很少，請檢查 image/map、掃描報告與可疑檔案區。</p></main></body></html>"#,
        case_id = html_escape(
            customer_json
                .get("case_id")
                .and_then(|v| v.as_str())
                .unwrap_or("")
        ),
        outcome = html_escape(outcome),
        recovered_count = recovered_count,
        recovered_bytes = recovered_bytes,
        suspect_count = suspect_count,
        suspect_bytes = suspect_bytes,
        skipped_count = skipped_count
    );
    let engineer_html = format!(
        r#"<!doctype html><html lang="zh-Hant"><head><meta charset="utf-8"><title>Polik Engineer Handoff {case_id}</title><style>body{{font-family:-apple-system,BlinkMacSystemFont,'Noto Sans TC',sans-serif;background:#f8fafc;color:#172033;line-height:1.7;margin:32px}}main{{max-width:980px;margin:auto;background:#fff;border:1px solid #e5e7eb;border-radius:18px;padding:26px;box-shadow:0 12px 30px rgba(15,23,42,.08)}}table{{border-collapse:collapse;width:100%;margin:16px 0}}th,td{{border:1px solid #e5e7eb;padding:10px;text-align:left;vertical-align:top}}th{{background:#f1f5f9}}pre{{background:#0f172a;color:#e2e8f0;padding:14px;border-radius:12px}}</style></head><body><main><h1>Polik Recovery Engineer Handoff</h1><table><tr><th>Case</th><td>{case_id}</td></tr><tr><th>Session</th><td>{session}</td></tr><tr><th>Source</th><td>{source}</td></tr><tr><th>Mode</th><td>{mode}</td></tr><tr><th>Outcome</th><td>{outcome}</td></tr><tr><th>Counts</th><td>complete={recovered_count}, suspect={suspect_count}, skipped={skipped_count}, scan_reports={scan_count}, quality_reports={quality_count}, timeline={timeline_count}</td></tr></table><h2>macOS + Codex validation</h2><pre>make codex-bootstrap
make codex-validation-quick
make codex-validation-full
make quality-gate-all
cargo tauri build</pre><p>此專案曾在非 macOS、無 Rust/Cargo/Tauri CLI 的環境修改；搬到 macOS 後請先跑驗證流程。</p></main></body></html>"#,
        case_id = html_escape(
            customer_json
                .get("case_id")
                .and_then(|v| v.as_str())
                .unwrap_or("")
        ),
        session = html_escape(session_dir.to_string_lossy().as_ref()),
        source = html_escape(&source),
        mode = html_escape(&mode),
        outcome = html_escape(outcome),
        recovered_count = recovered_count,
        suspect_count = suspect_count,
        skipped_count = skipped_count,
        scan_count = scan_count,
        quality_count = quality_count,
        timeline_count = timeline_count
    );

    let report_dir = session_reports_dir(session_dir);
    std::fs::write(
        report_dir.join("customer_summary.json"),
        serde_json::to_string_pretty(&customer_json).unwrap_or_else(|_| "{}".to_string()),
    )
    .map_err(|e| format!("無法寫入 customer_summary.json：{}", e))?;
    std::fs::write(
        report_dir.join("engineer_handoff.json"),
        serde_json::to_string_pretty(&engineer_json).unwrap_or_else(|_| "{}".to_string()),
    )
    .map_err(|e| format!("無法寫入 engineer_handoff.json：{}", e))?;
    std::fs::write(
        session_customer_report_html_path(session_dir),
        customer_html,
    )
    .map_err(|e| format!("無法寫入 customer_summary.html：{}", e))?;
    std::fs::write(
        session_engineer_report_html_path(session_dir),
        engineer_html,
    )
    .map_err(|e| format!("無法寫入 engineer_handoff.html：{}", e))?;
    manifest["handoff_reports"] = serde_json::json!({"customer_html": session_customer_report_html_path(session_dir).to_string_lossy(), "engineer_html": session_engineer_report_html_path(session_dir).to_string_lossy(), "generated_at_unix": generated});
    if let Some(obj) = manifest.get_mut("folders").and_then(|v| v.as_object_mut()) {
        obj.insert(
            "reports".to_string(),
            serde_json::Value::String(
                session_reports_dir(session_dir)
                    .to_string_lossy()
                    .to_string(),
            ),
        );
    }
    std::fs::write(
        session_manifest_path(session_dir),
        serde_json::to_string_pretty(&manifest).unwrap_or_else(|_| "{}".to_string()),
    )
    .map_err(|e| format!("無法更新 handoff report manifest：{}", e))?;
    Ok(DesktopSessionHandoffReportResult {
        ok: true,
        session_dir: session_dir.to_string_lossy().to_string(),
        report_dir: report_dir.to_string_lossy().to_string(),
        customer_html: session_customer_report_html_path(session_dir)
            .to_string_lossy()
            .to_string(),
        engineer_html: session_engineer_report_html_path(session_dir)
            .to_string_lossy()
            .to_string(),
        message: "已產生 v2.5 客戶版摘要與工程交接報告。".to_string(),
    })
}

fn write_desktop_session_index(session_dir: &std::path::Path) -> Result<(), String> {
    let manifest = read_session_manifest(session_dir)?;
    let case_id = manifest_text_value(&manifest, "case_id");
    let source = manifest_text_value(&manifest, "source");
    let mode = manifest_text_value(&manifest, "mode");
    let mut file_rows = String::new();
    let folders = [
        ("來源 image / map", "01_source_image"),
        ("掃描報告", "02_scan_reports"),
        ("完整還原", "03_recovery_output/recovered"),
        ("可疑還原", "03_recovery_output/suspect"),
        ("品質報告", "04_quality_reports"),
        ("Logs", "05_logs"),
        ("交接資料", "06_handoff"),
        ("客戶交付包", "11_delivery/packages"),
    ];
    for (label, rel) in folders {
        let p = session_dir.join(rel);
        let count = if p.exists() {
            std::fs::read_dir(&p)
                .map(|rd| rd.filter_map(Result::ok).count())
                .unwrap_or(0)
        } else {
            0
        };
        file_rows.push_str(&format!(
            "<tr><th>{}</th><td><code>{}</code></td><td>{}</td></tr>",
            label, rel, count
        ));
    }
    let mut artifact_rows = String::new();
    if let Some(arr) = manifest.get("artifacts").and_then(|v| v.as_array()) {
        for item in arr.iter().rev().take(20) {
            let kind = item.get("kind").and_then(|v| v.as_str()).unwrap_or("");
            let dest = item
                .get("destination_path")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let ts = item
                .get("imported_at_unix")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            artifact_rows.push_str(&format!(
                "<tr><td>{}</td><td><code>{}</code></td><td>{}</td></tr>",
                kind, dest, ts
            ));
        }
    }
    if artifact_rows.is_empty() {
        artifact_rows
            .push_str(r#"<tr><td colspan="3" class="muted">尚未匯入 artifacts。</td></tr>"#);
    }
    let mut timeline_rows = String::new();
    if let Some(arr) = manifest.get("timeline").and_then(|v| v.as_array()) {
        for item in arr.iter().rev().take(20) {
            let ts = item
                .get("created_at")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let status = item.get("status").and_then(|v| v.as_str()).unwrap_or("");
            let title = item.get("title").and_then(|v| v.as_str()).unwrap_or("");
            let summary = item.get("summary").and_then(|v| v.as_str()).unwrap_or("");
            timeline_rows.push_str(&format!(
                "<tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>",
                html_escape(ts),
                html_escape(status),
                html_escape(title),
                html_escape(summary)
            ));
        }
    }
    if timeline_rows.is_empty() {
        timeline_rows.push_str(r#"<tr><td colspan="4" class="muted">尚無 timeline。</td></tr>"#);
    }
    let _ = write_desktop_session_timeline_files(session_dir, &manifest);
    let doc = format!(
        r#"<!doctype html>
<html lang="zh-Hant"><head><meta charset="utf-8"><title>Polik Recovery Session {case_id}</title>
<style>body{{font-family:-apple-system,BlinkMacSystemFont,'Noto Sans TC',sans-serif;background:#f8fafc;color:#1f2937;line-height:1.6;margin:32px}}.card{{background:white;border:1px solid #e5e7eb;border-radius:16px;padding:18px;margin:14px 0;box-shadow:0 8px 22px rgba(15,23,42,.06)}}table{{border-collapse:collapse;width:100%}}th,td{{border:1px solid #e5e7eb;padding:10px;text-align:left}}th{{background:#f1f5f9;width:190px}}code{{font-family:ui-monospace,SFMono-Regular,Menlo,monospace}}.muted{{color:#64748b}}</style></head>
<body><h1>Polik Recovery Session</h1><p class="muted">Desktop App 建立/開啟的救援案件索引。v2.4 另提供 Dashboard 總覽。</p>
<div class="card"><table><tr><th>Case ID</th><td>{case_id}</td></tr><tr><th>Source</th><td><code>{source}</code></td></tr><tr><th>Mode</th><td>{mode}</td></tr><tr><th>Session Dir</th><td><code>{session}</code></td></tr></table></div>
<div class="card"><h2>資料夾狀態</h2><table><tr><th>項目</th><th>路徑</th><th>第一層項目數</th></tr>{file_rows}</table></div>
<div class="card"><h2>最近匯入 Artifacts</h2><table><tr><th>類型</th><th>目標路徑</th><th>匯入時間</th></tr>{artifact_rows}</table></div>
<div class="card"><h2>操作時間線</h2><p><a href="07_timeline/session_timeline.html">開啟完整時間線</a></p><table><tr><th>時間</th><th>狀態</th><th>事件</th><th>摘要</th></tr>{timeline_rows}</table></div>
<div class="card"><h2>案件 Dashboard</h2><p><a href="08_dashboard/session_dashboard.html">開啟 v2.4 Dashboard</a></p><p class="muted">集中顯示 image/map、scan、recovery、quality、timeline 與下一步建議。</p></div>
<div class="card"><h2>v2.6 報告 / PDF / 抽查</h2><p><a href="09_reports/customer_summary.html">客戶版摘要</a> ｜ <a href="09_reports/engineer_handoff.html">工程交接版</a> ｜ <a href="09_reports/customer_summary.pdf">客戶版 PDF</a></p><p><a href="10_audit/sample_audit.html">人工抽查紀錄</a></p><p class="muted">整理 Dashboard、timeline、抽查紀錄與還原結果，方便交付、回報與 Codex 接手。</p></div>
<div class="card"><h2>v2.7 客戶交付 ZIP</h2><p><a href="11_delivery/packages/">開啟交付包資料夾</a></p><p class="muted">客戶交付包只應包含客戶版報告、抽查紀錄、檔案清單、checksum 與 recovered/ 結果；raw image、ddrescue map、工程 log 與工程版報告不應放入客戶 zip。</p></div>
<div class="card"><h2>提醒</h2><p>來源是實體磁碟時，請先建立 image，再掃描 image。可疑或 partial 檔案應留在 suspect/，不要混進 recovered/。所有外部報告請用 v2.2 artifact 匯入流程收進本 Session，並用 v2.3 timeline 記錄操作。</p></div>
</body></html>"#,
        case_id = case_id,
        source = source,
        mode = mode,
        session = session_dir.to_string_lossy(),
        file_rows = file_rows,
        artifact_rows = artifact_rows,
        timeline_rows = timeline_rows
    );
    std::fs::write(session_index_path(session_dir), doc)
        .map_err(|e| format!("無法寫入 session_index.html：{}", e))
}

fn write_desktop_session_manifest(
    session_dir: &std::path::Path,
    case_id: &str,
    source: &str,
    mode: &str,
) -> Result<(), String> {
    ensure_session_dirs(session_dir)?;
    let stamp = imaging_stamp();
    let manifest = serde_json::json!({
        "schema": "polik.recovery.session.v2.7.desktop",
        "case_id": case_id,
        "created_at_unix": stamp,
        "updated_at_unix": stamp,
        "version": "2.7-delivery-package",
        "source": source,
        "mode": mode,
        "session_dir": session_dir.to_string_lossy(),
        "environment_note": "此專案曾在非 macOS、無 Rust/Cargo/Tauri CLI 的環境修改；實機驗證需於 macOS + Codex 執行。",
        "folders": {
            "intake": session_dir.join("00_intake").to_string_lossy(),
            "source_image": session_dir.join("01_source_image").to_string_lossy(),
            "scan_reports": session_dir.join("02_scan_reports").to_string_lossy(),
            "recovery_output": session_dir.join("03_recovery_output").to_string_lossy(),
            "quality_reports": session_dir.join("04_quality_reports").to_string_lossy(),
            "logs": session_dir.join("05_logs").to_string_lossy(),
            "handoff": session_dir.join("06_handoff").to_string_lossy(),
            "timeline": session_timeline_dir(session_dir).to_string_lossy(),
            "dashboard": session_dashboard_dir(session_dir).to_string_lossy(),
            "reports": session_reports_dir(session_dir).to_string_lossy(),
            "audit": session_audit_dir(session_dir).to_string_lossy(),
            "delivery": session_delivery_dir(session_dir).to_string_lossy()
        },
        "timeline": [{
            "id": format!("evt-{}-0001", stamp.clone()),
            "sequence": 1,
            "created_at_unix": stamp.clone(),
            "created_at": stamp.clone(),
            "event_type": "session_created",
            "title": "建立救援 Session",
            "status": "success",
            "summary": "Desktop App 建立案件資料夾。",
            "actor": "desktop_app",
            "artifacts": [],
            "details": {"source": source, "mode": mode}
        }],
        "timeline_count": 1,
        "warnings": [
            "Desktop App 已把還原輸出指向 03_recovery_output；請勿改回來源磁碟。",
            "若來源是 /dev/disk*，請先 image-first，再掃描 .img。"
        ]
    });
    std::fs::write(
        session_manifest_path(session_dir),
        serde_json::to_string_pretty(&manifest).unwrap_or_else(|_| "{}".to_string()),
    )
    .map_err(|e| format!("無法寫入 session_manifest.json：{}", e))?;
    write_desktop_session_readme(session_dir, case_id, source, mode)?;
    let current_manifest = read_session_manifest(session_dir)?;
    write_desktop_session_timeline_files(session_dir, &current_manifest)?;
    let _ = write_desktop_session_dashboard(session_dir);
    write_desktop_session_index(session_dir)?;
    Ok(())
}

fn open_path_with_system(path: &std::path::Path) -> Result<(), String> {
    if !path.exists() {
        return Err(format!("找不到路徑：{}", path.display()));
    }
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(path)
            .spawn()
            .map_err(|e| format!("無法開啟：{}", e))?;
        Ok(())
    }
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("explorer.exe")
            .arg(path)
            .spawn()
            .map_err(|e| format!("無法開啟：{}", e))?;
        Ok(())
    }
    #[cfg(all(not(target_os = "macos"), not(target_os = "windows")))]
    {
        std::process::Command::new("xdg-open")
            .arg(path)
            .spawn()
            .map_err(|e| format!("無法開啟：{}", e))?;
        Ok(())
    }
}

fn validate_report_open_target(path: &std::path::Path) -> Result<PathBuf, String> {
    let canonical = std::fs::canonicalize(path)
        .map_err(|e| format!("無法解析要開啟的報告路徑 {}：{}", path.display(), e))?;
    let extension = canonical
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();

    if canonical.is_dir() {
        if matches!(
            extension.as_str(),
            "app" | "bundle" | "command" | "exe" | "plugin" | "workflow"
        ) {
            return Err("拒絕以報告開啟功能啟動應用程式或可執行 bundle。".to_string());
        }
        return Ok(canonical);
    }

    if !canonical.is_file() {
        return Err(format!(
            "報告目標不是一般檔案或資料夾：{}",
            canonical.display()
        ));
    }
    if !matches!(
        extension.as_str(),
        "csv" | "htm" | "html" | "json" | "log" | "pdf" | "txt" | "zip"
    ) {
        return Err(format!(
            "不允許以報告開啟功能開啟 .{} 檔案。",
            if extension.is_empty() {
                "<無副檔名>"
            } else {
                extension.as_str()
            }
        ));
    }
    Ok(canonical)
}

#[cfg(target_os = "macos")]
fn list_disks_impl() -> Result<Vec<DiskInfo>, String> {
    let list_out = std::process::Command::new("diskutil")
        .args(["list"])
        .output()
        .map_err(|e| format!("無法執行 diskutil list：{}", e))?;

    let list_text = String::from_utf8_lossy(&list_out.stdout).into_owned();
    let mut partition_ids: Vec<(String, bool)> = Vec::new();
    let mut cur_ext = false;
    let mut cur_skip = false;

    for line in list_text.lines() {
        if line.starts_with("/dev/disk") {
            cur_ext = line.contains("external");
            cur_skip = line.contains("synthesized") || line.contains("disk image");
        } else if !cur_skip {
            let t = line.trim();
            if t.is_empty() {
                continue;
            }
            if let Some(colon) = t.find(':') {
                let row_num: i32 = t[..colon].trim().parse().unwrap_or(-1);
                if row_num > 0 {
                    let rest_lower = t[colon + 1..].trim().to_lowercase();
                    if rest_lower.contains("efi")
                        || rest_lower.contains("apple_apfs_isc")
                        || rest_lower.contains("apple_boot")
                        || rest_lower.contains("_recovery")
                    {
                        continue;
                    }
                    let tokens: Vec<&str> = t[colon + 1..].split_whitespace().collect();
                    if let Some(&id) = tokens.last() {
                        if id.starts_with("disk") && id.contains('s') {
                            partition_ids.push((id.to_string(), cur_ext));
                        }
                    }
                }
            }
        }
    }

    let mut disks: Vec<DiskInfo> = Vec::new();
    for (id, is_ext) in partition_ids {
        let dev_path = format!("/dev/{}", id);
        let info_text = std::process::Command::new("diskutil")
            .args(["info", &dev_path])
            .output()
            .map(|o| String::from_utf8_lossy(&o.stdout).into_owned())
            .unwrap_or_default();

        let mut vol_name = String::new();
        let mut fs_type = String::new();
        let mut size = String::new();
        let mut solid_state = false;
        let mut encrypted = false;

        for line in info_text.lines() {
            let t = line.trim();
            if let Some(value) = t.strip_prefix("Volume Name:") {
                let v = value.trim();
                if v != "Not applicable" {
                    vol_name = v.to_string();
                }
            } else if let Some(value) = t.strip_prefix("File System Personality:") {
                fs_type = value.trim().to_string();
            } else if let Some(value) = t.strip_prefix("Type (Bundle):") {
                if fs_type.is_empty() {
                    fs_type = value.trim().to_string();
                }
            } else if let Some(value) = t.strip_prefix("Disk Size:") {
                let s = value.trim();
                size = s.split('(').next().unwrap_or(s).trim().to_string();
            } else if t.starts_with("Solid State:") {
                solid_state = t.to_ascii_lowercase().contains("yes");
            } else if t.starts_with("FileVault:") || t.starts_with("Encrypted:") {
                let value = t
                    .split_once(':')
                    .map(|(_, v)| v.trim())
                    .unwrap_or("")
                    .to_ascii_lowercase();
                let clearly_no =
                    value == "no" || value.contains("not encrypted") || value.contains("none");
                if !clearly_no {
                    encrypted = value.contains("yes")
                        || value.contains("encrypted")
                        || value.contains("unlocked")
                        || value.contains("locked");
                }
            }
        }

        let display = if vol_name.is_empty() {
            id.clone()
        } else {
            vol_name
        };
        let flag = if is_ext { "🔌 外接" } else { "💾 內建" };
        let label = format!("{} · {} · {} ({})", display, fs_type, size, flag);
        disks.push(DiskInfo {
            path: dev_path,
            label,
            size,
            removable: is_ext,
            fs_type,
            solid_state,
            encrypted,
        });
    }
    Ok(disks)
}

#[cfg(target_os = "windows")]
fn format_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = 1024 * 1024;
    const GB: u64 = 1024 * 1024 * 1024;
    const TB: u64 = 1024 * 1024 * 1024 * 1024;

    if bytes >= TB {
        format!("{:.1} TB", bytes as f64 / TB as f64)
    } else if bytes >= GB {
        format!("{:.1} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.1} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}

#[cfg(target_os = "windows")]
fn list_disks_impl() -> Result<Vec<DiskInfo>, String> {
    let raw_disks = data_recovery_device_windows::list_whole_disks()
        .map_err(|e| format!("無法列出 Windows 磁碟：{}", e))?;
    let mut disks = Vec::new();
    for d in raw_disks {
        let size_str = format_bytes(d.device.size_bytes);
        let flag = if d.device.is_removable {
            "🔌 外接"
        } else {
            "💾 內建"
        };
        let fs_type = if d.whole_disk {
            "實體磁碟區".to_string()
        } else {
            d.content.clone().unwrap_or_else(|| "分割區".to_string())
        };
        let label = format!(
            "{} · {} · {} ({})",
            d.device.display_name, fs_type, size_str, flag
        );
        disks.push(DiskInfo {
            path: d.device.platform_path.clone(),
            label,
            size: size_str,
            removable: d.device.is_removable,
            fs_type,
            solid_state: false,
            encrypted: false,
        });
    }
    Ok(disks)
}

#[cfg(all(not(target_os = "macos"), not(target_os = "windows")))]
fn list_disks_impl() -> Result<Vec<DiskInfo>, String> {
    Ok(vec![])
}

#[tauri::command]
async fn list_disks() -> Result<Vec<DiskInfo>, String> {
    list_disks_impl()
}

// ── Accurate partition size from diskutil ─────────────────────────────────────

fn diskutil_size_bytes(dev_path: &str) -> u64 {
    if !dev_path.starts_with("/dev/") {
        return 0;
    }
    let out = std::process::Command::new("diskutil")
        .args(["info", dev_path])
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).into_owned())
        .unwrap_or_default();
    for line in out.lines() {
        let t = line.trim();
        if let Some(value) = t.strip_prefix("Disk Size:") {
            let s = value.trim();
            if let Some(p) = s.find('(') {
                if let Some(q) = s[p + 1..].find(' ') {
                    if let Ok(n) = s[p + 1..p + 1 + q].parse::<u64>() {
                        return n;
                    }
                }
            }
        }
    }
    0
}

// ── Core scan / extract logic ─────────────────────────────────────────────────

fn vfs_quality_hint(
    mode: &str,
    entry: &data_recovery_vfs::VfsEntry,
) -> Option<(&'static str, &'static str)> {
    if entry.kind != data_recovery_vfs::VfsEntryKind::File {
        return None;
    }

    let deleted = entry.status == data_recovery_vfs::VfsEntryStatus::Deleted;
    match mode {
        "ntfs" if deleted => Some((
            "medium",
            "NTFS 已刪除檔：若 MFT runlist 仍完整可重組；若資料已覆蓋，輸出前會略過或隔離。",
        )),
        "ntfs" => Some((
            "high",
            "NTFS 正常檔：依 MFT runlist 讀取，仍會在輸出前做格式或內容檢查。",
        )),
        "exfat" if deleted => Some((
            "medium",
            "exFAT 已刪除檔：優先依 FAT chain，必要時才用連續 cluster fallback，可能隔離到 suspect/。",
        )),
        "exfat" => Some((
            "high",
            "exFAT 正常檔：依 FAT chain / NoFatChain 旗標讀取，輸出前仍會檢查內容。",
        )),
        "fat32" if deleted => Some((
            "medium",
            "FAT32 已刪除檔：FAT chain 可能被清除；若只能連續猜測，會視為可疑救援。",
        )),
        "fat32" => Some((
            "high",
            "FAT32 正常檔：依 FAT chain 讀取，輸出前仍會檢查內容。",
        )),
        "apfs" if deleted => Some((
            "low",
            "APFS 刪除項目目前僅能依仍存在的 inode/extent 嘗試；snapshot/已覆蓋/TRIM 仍可能無法救回。",
        )),
        "apfs" => Some((
            "medium",
            "APFS v1.2：已接上未加密卷宗的 extent 提取；若 FileVault/加密或缺 extent，會隔離為 partial/suspect。",
        )),
        _ => None,
    }
}

fn format_vfs_text(tree: &data_recovery_vfs::VfsTree, mode: &str) -> String {
    let mut out = String::new();
    for (id, path) in tree.flattened() {
        let Some(entry) = tree.entry(id) else {
            continue;
        };
        let size = entry
            .size_bytes
            .map(|v| v.to_string())
            .unwrap_or_else(|| "-".to_string());
        out.push_str(&format!(
            "{}	{:?}	{:?}	size={}	id={}",
            path, entry.kind, entry.status, size, id.0
        ));
        if let Some((tier, quality)) = vfs_quality_hint(mode, entry) {
            out.push_str(&format!("	tier={}	quality={}", tier, quality));
        }
        out.push('\n');
    }
    out
}

fn friendly_runtime_error(context: &str, target: &str, raw: &str) -> String {
    let lower = raw.to_ascii_lowercase();
    let (code, reason, action) = if lower.contains("permission denied")
        || lower.contains("operation not permitted")
        || lower.contains("os error 1")
        || lower.contains("os error 13")
    {
        (
            "PERMISSION_DENIED",
            "作業系統拒絕讀取來源檔案。這通常不是資料不存在，而是檔案權限不足。",
            "確認目前帳號可唯讀開啟映像檔；實體磁碟請先使用受控的 image-first 流程建立映像，不要以管理員身分啟動桌面 App。",
        )
    } else if lower.contains("resource busy")
        || lower.contains("device busy")
        || lower.contains("os error 16")
    {
        (
            "DEVICE_BUSY",
            "來源磁碟目前被系統掛載或其他 App 使用中，raw 讀取可能被鎖住。",
            "先關閉 Finder 預覽、照片 App、磁碟工具程式，必要時卸載該分割區；不要退出整顆外接碟，卸載後再掃描。",
        )
    } else if lower.contains("no such file")
        || lower.contains("not found")
        || lower.contains("os error 2")
    {
        (
            "SOURCE_NOT_FOUND",
            "找不到指定來源、輸出位置或 helper 檔案，可能是路徑變了、磁碟被拔掉，或 App 沒有完整安裝。",
            "重新選擇來源磁碟/映像檔；如果是打包後 App，請從 .app 啟動，不要只移動其中某個執行檔。",
        )
    } else if lower.contains("input/output error")
        || lower.contains("i/o error")
        || lower.contains("os error 5")
    {
        (
            "IO_ERROR",
            "讀取來源時發生 I/O 錯誤，常見於壞軌、接觸不良、外接盒不穩或磁碟即將故障。",
            "先停止反覆掃描，改用 ddrescue 或專業工具建立映像檔，再掃描映像檔；外接碟請換線、換 USB 埠或供電充足的外接盒。",
        )
    } else if lower.contains("invalid argument")
        || lower.contains("os error 22")
        || lower.contains("unsupported")
        || lower.contains("不支援")
    {
        (
            "UNSUPPORTED_OR_BAD_OFFSET",
            "目前選擇的檔案系統模式、offset 或來源格式不相符。",
            "確認模式是否選對：NTFS / exFAT / FAT32 / APFS；不確定時先用磁碟下拉自動判斷，或改用 Deep Scan 只做特徵掃描。",
        )
    } else if lower.contains("apfs")
        && (lower.contains("encrypted")
            || lower.contains("filevault")
            || lower.contains("crypto")
            || lower.contains("加密"))
    {
        (
            "APFS_ENCRYPTED",
            "APFS 疑似加密或 FileVault 狀態，未解鎖時底層資料看起來會像亂碼。",
            "先在 macOS 正常解鎖卷宗，或掃描已解鎖後製作的映像檔；加密未解鎖時不要期待 Deep Scan 能找出 JPG/PDF 特徵。",
        )
    } else {
        (
            "RECOVERY_ERROR",
            "程式在這一步遇到未分類錯誤。",
            "請先看下方技術訊息；若是在真實磁碟上發生，建議改掃映像檔並保留 recovery_report.html / helper log 方便排查。",
        )
    };

    format!(
        "{}失敗\n錯誤代碼：{}\n可能原因：{}\n建議處理：{}\n目標：{}\n技術訊息：{}",
        context, code, reason, action, target, raw
    )
}

fn open_source(file_path: &str) -> Result<FileBlockSource, String> {
    if file_path.starts_with("/dev/") || file_path.starts_with(r"\\.\") {
        return Err(
            "RAW_DEVICE_UNSUPPORTED：桌面 App 的舊版特權流程已停用；請先用受控的 ddrescue/image-first 流程建立映像檔，再選擇映像檔掃描。"
                .to_string(),
        );
    }
    let path_buf = PathBuf::from(file_path);
    let size_hint = diskutil_size_bytes(file_path);
    if size_hint > 0 {
        FileBlockSource::open_with_known_size(path_buf, size_hint).map_err(|e| e.to_string())
    } else {
        FileBlockSource::open(path_buf).map_err(|e| e.to_string())
    }
}

#[derive(Debug, Clone)]
struct NtfsScanPlan {
    effective_count: usize,
    estimated_total_records: Option<usize>,
    strategy: &'static str,
    capped: bool,
}

fn plan_ntfs_scan_count<S: BlockSource + ?Sized>(
    source: &S,
    boot: &data_recovery_fs_ntfs::NtfsBootSector,
    volume_offset: u64,
    requested: usize,
) -> NtfsScanPlan {
    let estimated_total_records =
        data_recovery_fs_ntfs::estimate_mft_record_count(source, boot, volume_offset)
            .ok()
            .flatten();

    let (raw_count, strategy) = if requested == NTFS_QUICK_SCAN_COUNT {
        (NTFS_QUICK_SCAN_COUNT, "quick_fixed_10000")
    } else if requested == 0 {
        (
            estimated_total_records.unwrap_or(NTFS_STANDARD_FALLBACK_SCAN_COUNT),
            if estimated_total_records.is_some() {
                "auto_full_mft_from_$MFT_size"
            } else {
                "auto_fallback_standard_limit"
            },
        )
    } else {
        (requested, "user_requested_limit")
    };

    let effective_count = raw_count.clamp(1, NTFS_MAX_SCAN_COUNT);
    NtfsScanPlan {
        effective_count,
        estimated_total_records,
        strategy,
        capped: raw_count > effective_count,
    }
}

fn decode_ntfs_selected_id(selected_id: &str) -> Result<(u64, Option<usize>), String> {
    let raw_id = selected_id
        .parse::<u64>()
        .map_err(|e| format!("無效的 MFT ID: {}", e))?;
    let ads_ordinal = (raw_id >> 48) as usize;
    let record_number = raw_id & NTFS_ADS_ID_MASK;
    let stream_ordinal = if ads_ordinal == 0 {
        None
    } else {
        Some(ads_ordinal)
    };
    Ok((record_number, stream_ordinal))
}

fn ntfs_stream_name_for_ordinal(
    record: &data_recovery_fs_ntfs::MftRecord,
    ordinal: usize,
) -> Result<Option<String>, String> {
    if ordinal == 0 {
        return Ok(None);
    }
    data_recovery_fs_ntfs::list_data_streams(record)
        .into_iter()
        .filter_map(|stream| stream.name)
        .nth(ordinal - 1)
        .map(Some)
        .ok_or_else(|| format!("找不到第 {} 個 NTFS 附加資料流。", ordinal))
}

#[cfg(any(target_os = "macos", windows))]
fn resolve_recovery_storage_identities(
    source: &Path,
    output: &Path,
) -> Result<
    (
        data_recovery_safe_writer::StorageIdentity,
        data_recovery_safe_writer::StorageIdentity,
    ),
    String,
> {
    data_recovery_safe_writer::resolve_image_storage_identities(source, output)
        .map_err(|error| error.to_string())
}

#[cfg(not(any(target_os = "macos", windows)))]
fn resolve_recovery_storage_identities(
    _source: &Path,
    _output: &Path,
) -> Result<
    (
        data_recovery_safe_writer::StorageIdentity,
        data_recovery_safe_writer::StorageIdentity,
    ),
    String,
> {
    Err("此平台尚未實作可證明 physical store 的安全 resolver；依安全政策停止寫入。".to_string())
}

fn read_ntfs_records_tolerant<S: BlockSource + ?Sized>(
    source: &S,
    boot: &data_recovery_fs_ntfs::NtfsBootSector,
    volume_offset: u64,
    count: usize,
    notify: Option<&tauri::AppHandle>,
) -> Result<Vec<data_recovery_fs_ntfs::MftRecord>, String> {
    let mft_offset = volume_offset
        .checked_add(boot.mft_offset().map_err(|e| e.to_string())?)
        .ok_or_else(|| "NTFS MFT 位址計算溢位".to_string())?;
    let record_size = boot.file_record_size().map_err(|e| e.to_string())? as usize;
    let mut records = Vec::new();
    let mut invalid_streak = 0usize;
    let mut first_error: Option<String> = None;

    for index in 0..count {
        check_cancelled()?;
        let record_offset = mft_offset
            .checked_add((index as u64).saturating_mul(record_size as u64))
            .ok_or_else(|| "NTFS MFT 紀錄位址計算溢位".to_string())?;

        match data_recovery_fs_ntfs::read_mft_record(
            source,
            record_offset,
            record_size,
            boot.bytes_per_sector,
        ) {
            Ok(record) => {
                invalid_streak = 0;
                records.push(record);
            }
            Err(e) => {
                invalid_streak += 1;
                first_error.get_or_insert_with(|| e.to_string());
                // 真實大型磁碟的 MFT 可能有空白、損毀或未初始化區段；
                // 連續遇到多筆無效紀錄後停止，避免為了讀取不存在的尾端把掃描判定失敗。
                if records.is_empty() && index < 16 {
                    return Err(format!("無法讀取 NTFS MFT 起始紀錄：{}", e));
                }
                if invalid_streak >= 128 {
                    break;
                }
            }
        }

        // 每 1000 筆 emit 一次進度，讓前端顯示動態數字
        if index > 0 && index % 1000 == 0 {
            if let Some(app) = notify {
                let pct = if count > 0 {
                    ((index * 90 / count) as u8).min(90)
                } else {
                    10
                };
                let files_found = records.iter().filter(|r| !r.header.is_directory()).count();
                app.emit(
                    "scan-progress",
                    ScanProgressPayload {
                        pct: Some(pct),
                        scanned: index,
                        total: count,
                        files_found,
                        msg: format!(
                            "NTFS 已掃 {} / {} 筆記錄，找到 {} 個檔案",
                            index, count, files_found
                        ),
                    },
                )
                .ok();
            }
        }
    }

    if records.is_empty() {
        Err(first_error.unwrap_or_else(|| "沒有讀到任何有效 NTFS MFT 紀錄".to_string()))
    } else {
        Ok(records)
    }
}

fn emit_scan_progress(
    notify: Option<&tauri::AppHandle>,
    pct: u8,
    scanned: usize,
    total: usize,
    files_found: usize,
    msg: &str,
) {
    if let Some(app) = notify {
        app.emit(
            "scan-progress",
            ScanProgressPayload {
                pct: Some(pct),
                scanned,
                total,
                files_found,
                msg: msg.to_string(),
            },
        )
        .ok();
    }
}

fn emit_scan_activity(notify: Option<&tauri::AppHandle>, total_hint: usize, message: &str) {
    if let Some(app) = notify {
        app.emit(
            "scan-progress",
            ScanProgressPayload {
                pct: None,
                scanned: 0,
                total: total_hint,
                files_found: 0,
                msg: message.to_string(),
            },
        )
        .ok();
    }
}

fn perform_scan(
    file_path: &str,
    mode: &str,
    offset: u64,
    count: usize,
    notify: Option<&tauri::AppHandle>,
) -> Result<ScanResponse, String> {
    check_cancelled()?;
    emit_scan_progress(notify, 2, 0, 0, 0, "開啟來源磁碟...");
    let source = open_source(file_path)?;
    check_cancelled()?;
    match mode {
        "ntfs" => {
            emit_scan_progress(notify, 5, 0, 0, 0, "讀取 NTFS 開機磁區...");
            let boot = data_recovery_fs_ntfs::read_boot_sector(&source, offset)
                .map_err(|e| e.to_string())?;
            let plan = plan_ntfs_scan_count(&source, &boot, offset, count);
            emit_scan_progress(
                notify,
                8,
                0,
                plan.effective_count,
                0,
                &format!("開始掃描 NTFS MFT，預估 {} 筆記錄...", plan.effective_count),
            );
            let records =
                read_ntfs_records_tolerant(&source, &boot, offset, plan.effective_count, notify)?;
            let tree = data_recovery_vfs::tree_from_ntfs_records(&records);
            let mut report =
                data_recovery_scan_report::NtfsScanReport::from_records(&boot, &records)
                    .map_err(|e| e.to_string())?
                    .to_text();
            report.push_str(&format!(
                "\nscan_strategy: tolerant_mft_read\nntfs_scan_plan: {}\nrequested_records: {}\nestimated_total_records: {}\neffective_record_limit: {}\nrecord_limit_capped: {}\nquality_note: 會先讀取 $MFT 自身大小估算總 MFT 筆數；空白或無效 MFT 紀錄會被略過，避免大型磁碟掃描因單筆壞紀錄中斷。",
                plan.strategy,
                count,
                plan.estimated_total_records
                    .map(|v| v.to_string())
                    .unwrap_or_else(|| "unknown".to_string()),
                plan.effective_count,
                plan.capped
            ));
            Ok(ScanResponse {
                report,
                vfs: format_vfs_text(&tree, "ntfs"),
            })
        }
        "exfat" => {
            emit_scan_progress(notify, 5, 0, 0, 0, "讀取 exFAT 開機磁區...");
            let boot = data_recovery_fs_exfat::read_boot_sector(&source, offset)
                .map_err(|e| e.to_string())?;
            let cluster_total = boot.cluster_count as usize;
            emit_scan_progress(
                notify,
                15,
                0,
                cluster_total,
                0,
                &format!("掃描 exFAT 目錄項目（共 {} clusters）...", cluster_total),
            );
            emit_scan_activity(
                notify,
                cluster_total,
                "正在掃描 exFAT；此階段沒有可靠的完成百分比…",
            );
            let entries = data_recovery_fs_exfat::scan_exfat_vfs(&source, &boot, offset)
                .map_err(|e| e.to_string())?;
            let files = entries.iter().filter(|e| !e.is_directory).count();
            let deleted = entries.iter().filter(|e| e.is_deleted).count();
            emit_scan_progress(
                notify,
                85,
                entries.len(),
                entries.len(),
                files,
                &format!(
                    "建構虛擬檔案樹：{} 個項目（含 {} 個已刪除）...",
                    entries.len(),
                    deleted
                ),
            );
            let tree = data_recovery_vfs::tree_from_exfat_entries(&entries);
            let report = format!(
                "exFAT Volume Report\nbytes_per_sector: {}\nsectors_per_cluster: {}\ncluster_size: {}\nfat_offset: {}\nfat_length: {}\ncluster_heap_offset: {}\ncluster_count: {}\nrecords_scanned: {}\nfiles: {}\ndeleted: {}\nextractable_files: {}",
                boot.sector_size(),
                1 << boot.sectors_per_cluster_shift,
                boot.cluster_size(),
                boot.fat_offset,
                boot.fat_length,
                boot.cluster_heap_offset,
                boot.cluster_count,
                entries.len(),
                files,
                deleted,
                entries.len()
            );
            Ok(ScanResponse {
                report,
                vfs: format_vfs_text(&tree, "exfat"),
            })
        }
        "fat32" => {
            emit_scan_progress(notify, 5, 0, 0, 0, "讀取 FAT32 開機磁區...");
            let boot = data_recovery_fs_fat32::read_boot_sector(&source, offset)
                .map_err(|e| e.to_string())?;
            let cluster_total = boot.cluster_count() as usize;
            emit_scan_progress(
                notify,
                15,
                0,
                cluster_total,
                0,
                &format!("掃描 FAT32 目錄項目（共 {} clusters）...", cluster_total),
            );
            emit_scan_activity(
                notify,
                cluster_total,
                "正在掃描 FAT32；此階段沒有可靠的完成百分比…",
            );
            let entries = data_recovery_fs_fat32::scan_fat32_vfs(&source, &boot, offset)
                .map_err(|e| e.to_string())?;
            let files = entries.iter().filter(|e| !e.is_directory).count();
            let deleted = entries.iter().filter(|e| e.is_deleted).count();
            emit_scan_progress(
                notify,
                85,
                entries.len(),
                entries.len(),
                files,
                &format!(
                    "建構虛擬檔案樹：{} 個項目（含 {} 個已刪除）...",
                    entries.len(),
                    deleted
                ),
            );
            let tree = data_recovery_vfs::tree_from_fat32_entries(&entries);
            let report = format!(
                "FAT32 Volume Report\nbytes_per_sector: {}\nsectors_per_cluster: {}\ncluster_size: {}\nreserved_sectors: {}\nfat_count: {}\nfat_size: {}\nroot_cluster: {}\ncluster_count: {}\nrecords_scanned: {}\nfiles: {}\ndeleted: {}\nextractable_files: {}\nquality_note: FAT32 已刪除檔案若 FAT chain 被清除，會以 first_cluster 連續讀取作為保守 fallback；碎片化刪除檔仍可能只救回部分資料。",
                boot.sector_size(),
                boot.sectors_per_cluster,
                boot.cluster_size(),
                boot.reserved_sector_count,
                boot.number_of_fats,
                boot.fat_size_32,
                boot.root_cluster,
                boot.cluster_count(),
                entries.len(),
                files,
                deleted,
                entries.len()
            );
            Ok(ScanResponse {
                report,
                vfs: format_vfs_text(&tree, "fat32"),
            })
        }
        "apfs" => {
            emit_scan_progress(notify, 5, 0, 0, 0, "讀取 APFS Container Superblock...");
            let csb = data_recovery_fs_apfs::read_container_superblock(&source, offset)
                .map_err(|e| e.to_string())?;
            let volume_statuses =
                data_recovery_fs_apfs::read_apfs_volume_statuses(&source, &csb, offset);
            let encrypted_like = volume_statuses.iter().any(|v| v.encrypted_hint);
            let block_total = csb.block_count as usize;
            emit_scan_progress(
                notify,
                20,
                0,
                block_total,
                0,
                &format!("遍歷 APFS B-Tree（{} blocks）...", block_total),
            );
            emit_scan_activity(
                notify,
                block_total,
                "正在遍歷 APFS B-Tree；此階段沒有可靠的完成百分比…",
            );
            let entries = data_recovery_fs_apfs::scan_apfs_vfs(&source, &csb, offset)
                .map_err(|e| e.to_string())?;
            emit_scan_progress(
                notify,
                85,
                entries.len(),
                entries.len(),
                entries.len(),
                &format!("建構虛擬檔案樹：{} 個項目...", entries.len()),
            );
            let tree = data_recovery_vfs::tree_from_apfs_entries(&entries);
            let volume_lines = if volume_statuses.is_empty() {
                "volume_status: none_readable".to_string()
            } else {
                volume_statuses
                    .iter()
                    .enumerate()
                    .map(|(idx, v)| format!(
                        "volume_{}_oid: {} role={} root_tree_oid={} meta_crypto={} encrypted_hint={}",
                        idx, v.fs_oid, v.role, v.root_tree_oid, v.meta_crypto, v.encrypted_hint
                    ))
                    .collect::<Vec<_>>()
                    .join("\n")
            };
            let report = format!(
                "APFS Container Superblock\nblock_size: {}\nblock_count: {}\nchecksum_status: OK (Fletcher64 Match)\nspaceman_oid: {}\nomap_oid: {}\nreaper_oid: {}\nmax_file_systems: {}\nentries_read: {}\n{}\nextract_note: v1.2 已接上未加密 APFS extent 提取；FileVault/加密、壓縮、clone/snapshot-only 與缺失 extent 會改列 suspect/partial。\nencrypted_hint: {}",
                csb.block_size,
                csb.block_count,
                csb.spaceman_oid,
                csb.omap_oid,
                csb.reaper_oid,
                csb.max_file_systems,
                entries.len(),
                volume_lines,
                encrypted_like
            );
            Ok(ScanResponse {
                report,
                vfs: format_vfs_text(&tree, "apfs"),
            })
        }
        "carver" => {
            let file_size = std::fs::metadata(file_path).map(|m| m.len()).unwrap_or(0);
            let size_mb = file_size / 1_048_576;
            emit_scan_progress(
                notify,
                3,
                0,
                file_size as usize,
                0,
                &format!("深度掃描 {} MB，搜尋檔案特徵碼...", size_mb),
            );
            emit_scan_activity(
                notify,
                file_size as usize,
                &format!("正在深度掃描 {} MB；找到候選片段前無可靠百分比…", size_mb),
            );
            let config = data_recovery_file_carver::CarverConfig::default();
            let carved = data_recovery_file_carver::carve_source(&source, &config)
                .map_err(|e| e.to_string())?;
            check_cancelled()?;
            emit_scan_progress(
                notify,
                60,
                0,
                carved.len(),
                0,
                &format!("找到 {} 個候選片段，驗證格式...", carved.len()),
            );
            let mut png = 0;
            let mut jpeg = 0;
            let mut pdf = 0;
            let mut zip = 0;
            let mut mp4 = 0;
            let mut heic = 0;
            let mut ole = 0;
            let mut conf_sum = 0.0_f32;
            let mut vfs = String::new();
            let total_carved = carved.len();
            for (idx, f) in carved.iter().enumerate() {
                check_cancelled()?;
                // emit every 50 items or at first item
                if idx == 0 || (idx + 1) % 50 == 0 || idx + 1 == total_carved {
                    let pct = 60 + ((idx + 1) * 35 / total_carved.max(1)) as u8;
                    let found_so_far = png + jpeg + pdf + zip + mp4 + heic + ole;
                    emit_scan_progress(
                        notify,
                        pct.min(95),
                        idx + 1,
                        total_carved,
                        found_so_far,
                        &format!(
                            "驗證中 {}/{}: PNG={} JPG={} PDF={} ZIP={} MP4={}",
                            idx + 1,
                            total_carved,
                            png,
                            jpeg,
                            pdf,
                            zip,
                            mp4
                        ),
                    );
                }
                let data = read_carved_data(&source, f.offset, f.length).unwrap_or_default();
                let (ext, quality, tier) =
                    carved_extension_and_quality(&f.file_type, &data, f.confidence);
                match f.file_type.as_str() {
                    "PNG" => png += 1,
                    "JPEG" => jpeg += 1,
                    "PDF" => pdf += 1,
                    "ZIP" => zip += 1,
                    "MP4" => mp4 += 1,
                    "HEIC" => heic += 1,
                    "OLE" => ole += 1,
                    _ => {}
                }
                conf_sum += f.confidence;
                vfs.push_str(&format!(
                    "/carved_{}_{}.{}\tFile\tAllocated\tsize={}\tid={}\tconfidence={:.2}\tquality={}\ttier={}\n",
                    f.offset,
                    f.length,
                    ext,
                    f.length,
                    f.offset,
                    f.confidence,
                    quality,
                    tier.as_str()
                ));
            }
            let conf_avg = if carved.is_empty() {
                1.0
            } else {
                conf_sum / carved.len() as f32
            };
            let report = format!(
                "Carver Scan Complete\ntotal_bytes_scanned: {}\nsignatures_detected: {}\npng_carved: {}\njpeg_carved: {}\npdf_carved: {}\nzip_carved: {}\nmp4_carved: {}\nheic_carved: {}\nole_carved: {}\nconfidence_average: {:.2}\nquality_note: v0.9 會做格式驗證並在 UI 顯示完整/可疑/部分/碎片分流；高信心檔寫入 recovered/，其他隔離到 suspect/。",
                file_size,
                carved.len(),
                png,
                jpeg,
                pdf,
                zip,
                mp4,
                heic,
                ole,
                conf_avg
            );
            Ok(ScanResponse { report, vfs })
        }
        _ => Err(format!("不支援的掃描模式：{}", mode)),
    }
}

#[allow(dead_code)]
fn classify_zip_extension(data: &[u8]) -> &'static str {
    if data_contains(data, b"word/document.xml")
        || data_contains(data, b"[Content_Types].xml")
            && data_contains(
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

#[allow(dead_code)]
fn classify_ole_extension(data: &[u8]) -> &'static str {
    if data_contains(data, b"WordDocument") {
        "doc"
    } else if data_contains(data, b"Workbook") || data_contains(data, b"Book") {
        "xls"
    } else if data_contains(data, b"PowerPoint Document") || data_contains(data, b"Pictures") {
        "ppt"
    } else {
        "doc"
    }
}

#[allow(dead_code)]
fn classify_mp4_extension(data: &[u8]) -> &'static str {
    let sample = &data[..data.len().min(256)];
    if data_contains(sample, b"qt  ") || data_contains(sample, b"M4V ") {
        "mov"
    } else {
        "mp4"
    }
}

fn confidence_tier(confidence: f32, quality_ok: bool) -> RecoveryConfidenceTier {
    if !quality_ok || confidence < 0.60 {
        RecoveryConfidenceTier::Low
    } else if confidence < 0.85 {
        RecoveryConfidenceTier::Medium
    } else {
        RecoveryConfidenceTier::High
    }
}

fn read_carved_data<S: BlockSource + ?Sized>(
    source: &S,
    offset: u64,
    length: u64,
) -> Option<Vec<u8>> {
    if length == 0 || length > usize::MAX as u64 {
        return None;
    }
    let mut data = vec![0u8; length as usize];
    let read = source.read_at(offset, &mut data).ok()?;
    if read.bytes_read < data.len() {
        return None;
    }
    Some(data)
}

fn read_exfat_contiguous_data<S: BlockSource + ?Sized>(
    source: &S,
    boot: &data_recovery_fs_exfat::ExFatBootSector,
    volume_offset: u64,
    first_cluster: u32,
    data_len: u64,
) -> Result<Vec<u8>, String> {
    if data_len == 0 {
        return Ok(Vec::new());
    }
    if first_cluster < 2 {
        return Err("exFAT 檔案沒有可用的起始 cluster。".to_string());
    }
    if data_len > usize::MAX as u64 {
        return Err("exFAT 檔案過大，超出目前版本一次性還原限制。".to_string());
    }

    let cluster_size = boot.cluster_size();
    let cluster_offset = boot
        .cluster_to_byte_offset(first_cluster)
        .ok_or_else(|| format!("exFAT 起始 cluster 無效：{}", first_cluster))?;
    let mut data = vec![0_u8; data_len as usize];
    let res = source
        .read_at(volume_offset + cluster_offset, &mut data)
        .map_err(|e| e.to_string())?;
    if res.bytes_read < data.len() {
        return Err(format!(
            "exFAT 連續讀取不足：預期 {} bytes，實際 {} bytes",
            data.len(),
            res.bytes_read
        ));
    }

    // Avoid accidentally reading far past the cluster heap when a damaged directory entry has a bogus size.
    let needed_clusters = data_len.div_ceil(cluster_size);
    if (first_cluster as u64) + needed_clusters > 2 + (boot.cluster_count as u64) {
        return Err("exFAT 檔案大小或 cluster 範圍超出分割區，已略過。".to_string());
    }

    Ok(data)
}

fn carved_extension_and_quality(
    file_type: &str,
    data: &[u8],
    confidence: f32,
) -> (&'static str, &'static str, RecoveryConfidenceTier) {
    let validation = data_recovery_file_carver::validate_carved_bytes(file_type, data);
    let effective_confidence = confidence.min(validation.confidence);
    let tier = confidence_tier(effective_confidence, validation.ok);
    (validation.extension, validation.label, tier)
}

#[allow(dead_code)]
fn data_contains(haystack: &[u8], needle: &[u8]) -> bool {
    if needle.is_empty() || haystack.len() < needle.len() {
        return false;
    }
    haystack.windows(needle.len()).any(|w| w == needle)
}

#[allow(dead_code)]
fn data_rcontains(haystack: &[u8], needle: &[u8]) -> bool {
    if needle.is_empty() || haystack.len() < needle.len() {
        return false;
    }
    haystack.windows(needle.len()).rev().any(|w| w == needle)
}

fn recovered_data_is_padding(data: &[u8]) -> bool {
    if data.is_empty() {
        return true;
    }
    let sample_len = data.len().min(128 * 1024);
    let sample = &data[..sample_len];
    let padding = sample.iter().filter(|&&b| b == 0x00 || b == 0xff).count();
    padding * 100 >= sample_len * 98
}

fn text_like(data: &[u8]) -> bool {
    if data.is_empty() {
        return false;
    }
    let sample = &data[..data.len().min(64 * 1024)];
    if std::str::from_utf8(sample).is_ok() {
        return true;
    }
    let printable = sample
        .iter()
        .filter(|&&b| matches!(b, b'\n' | b'\r' | b'\t' | 0x20..=0x7e))
        .count();
    printable * 100 >= sample.len() * 85
}

#[allow(dead_code)]
fn valid_zip_bytes(data: &[u8]) -> bool {
    if data.len() < 64 || !data.starts_with(b"PK\x03\x04") {
        return false;
    }
    let Some(pos) = data.windows(4).rposition(|w| w == b"PK\x05\x06") else {
        return false;
    };
    if pos + 22 > data.len() {
        return false;
    }
    let cd_entries = u16::from_le_bytes([data[pos + 10], data[pos + 11]]) as u32;
    let cd_size = u32::from_le_bytes([
        data[pos + 12],
        data[pos + 13],
        data[pos + 14],
        data[pos + 15],
    ]) as usize;
    let cd_offset = u32::from_le_bytes([
        data[pos + 16],
        data[pos + 17],
        data[pos + 18],
        data[pos + 19],
    ]) as usize;
    let comment_len = u16::from_le_bytes([data[pos + 20], data[pos + 21]]) as usize;
    cd_entries > 0
        && pos + 22 + comment_len <= data.len()
        && cd_offset.saturating_add(cd_size) <= pos
}

#[allow(dead_code)]
fn valid_png_bytes(data: &[u8]) -> bool {
    data.len() >= 45 && data.starts_with(b"\x89PNG\r\n\x1a\n") && data_rcontains(data, b"IEND")
}

#[allow(dead_code)]
fn valid_jpeg_bytes(data: &[u8]) -> bool {
    data.len() >= 64
        && data.starts_with(b"\xff\xd8\xff")
        && data.ends_with(b"\xff\xd9")
        && data_contains(data, b"\xff\xda")
}

#[allow(dead_code)]
fn valid_pdf_bytes(data: &[u8]) -> bool {
    data.len() >= 128
        && data.starts_with(b"%PDF-1.")
        && data_contains(data, b"startxref")
        && data_rcontains(data, b"%%EOF")
}

#[allow(dead_code)]
fn valid_mp4_bytes(data: &[u8]) -> bool {
    data.len() >= 1024
        && data.get(4..8) == Some(&b"ftyp"[..])
        && data_contains(data, b"moov")
        && data_contains(data, b"mdat")
}

#[allow(dead_code)]
fn valid_ole_bytes(data: &[u8]) -> bool {
    data.len() >= 512
        && data.starts_with(b"\xD0\xCF\x11\xE0\xA1\xB1\x1A\xE1")
        && data.get(28) == Some(&0xFE)
        && data.get(29) == Some(&0xFF)
}

fn validate_recovered_data(data: &[u8], output_path: &Path) -> Result<String, String> {
    if data.is_empty() {
        return Err("檔案內容是 0 位元組，已略過，避免輸出空檔。".to_string());
    }
    if recovered_data_is_padding(data) {
        return Err(
            "檔案內容幾乎都是 00/FF 填充值，通常代表原始資料已被覆蓋或只剩碎片，已略過。"
                .to_string(),
        );
    }

    let ext = output_path
        .extension()
        .and_then(|v| v.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();

    let ok = match ext.as_str() {
        "png" => data_recovery_file_carver::validate_carved_bytes("PNG", data).ok,
        "jpg" | "jpeg" => data_recovery_file_carver::validate_carved_bytes("JPEG", data).ok,
        "pdf" => data_recovery_file_carver::validate_carved_bytes("PDF", data).ok,
        "zip" | "docx" | "xlsx" | "pptx" | "pages" | "numbers" | "key" => {
            data_recovery_file_carver::validate_carved_bytes("ZIP", data).ok
        }
        "mp4" | "mov" | "m4v" => data_recovery_file_carver::validate_carved_bytes("MP4", data).ok,
        "heic" | "heif" => data_recovery_file_carver::validate_carved_bytes("HEIC", data).ok,
        "doc" | "xls" | "ppt" | "ole" => {
            data_recovery_file_carver::validate_carved_bytes("OLE", data).ok
        }
        "txt" | "csv" | "md" | "rtf" | "log" => text_like(data),
        _ => true,
    };

    if !ok {
        return Err(format!(
            "檔案格式驗證失敗（副檔名 .{} 與內容不一致，或檔案結構不完整），已略過以避免救出垃圾檔。",
            if ext.is_empty() { "未知" } else { &ext }
        ));
    }

    let label = match ext.as_str() {
        "png" | "jpg" | "jpeg" | "pdf" | "zip" | "docx" | "xlsx" | "pptx" | "mp4" | "mov"
        | "m4v" | "heic" | "heif" | "doc" | "xls" | "ppt" | "ole" => "格式驗證通過",
        "txt" | "csv" | "md" | "rtf" | "log" => "文字內容檢查通過",
        _ => "基本內容檢查通過",
    };
    Ok(label.to_string())
}

fn perform_extract(
    file_path: &str,
    mode: &str,
    offset: u64,
    selected_id: &str,
    output_path: &str,
) -> Result<String, String> {
    if output_path.starts_with("/dev/") {
        return Err("輸出位置不能是磁碟裝置，請選擇資料夾並由程式自動建立檔名。".to_string());
    }
    if !file_path.starts_with("/dev/") {
        let src_parent = PathBuf::from(file_path).parent().map(PathBuf::from);
        if let Some(src_parent) = src_parent {
            let out_path_check = PathBuf::from(output_path);
            if !src_parent.as_os_str().is_empty() && out_path_check.starts_with(&src_parent) {
                return Err(
                    "輸出位置疑似和來源映像檔在同一個資料夾，已阻止以避免覆蓋待救資料。"
                        .to_string(),
                );
            }
        }
    }

    check_cancelled()?;
    let output_path = success_output_path(PathBuf::from(output_path));
    let (source_identity, output_identity) =
        resolve_recovery_storage_identities(Path::new(file_path), &output_path)?;
    if let Some(parent) = output_path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("無法建立輸出目錄 {}：{}", parent.display(), e))?;
        }
    }

    let source = open_source(file_path)?;
    let verified_source =
        data_recovery_safe_writer::verify_open_source_storage(&source, &source_identity)
            .map_err(|e| e.to_string())?;
    let write_recovered = |target: &Path, data: &[u8], source_record: Option<u64>| {
        data_recovery_safe_writer::write_recovered_file_with_storage_identities(
            file_path,
            target,
            data,
            source_record,
            &verified_source,
            &output_identity,
        )
        .map_err(|e| e.to_string())
    };
    check_cancelled()?;
    match mode {
        "ntfs" => {
            let boot = data_recovery_fs_ntfs::read_boot_sector(&source, offset)
                .map_err(|e| e.to_string())?;
            let (record_number, stream_ordinal) = decode_ntfs_selected_id(selected_id)?;
            let record = data_recovery_fs_ntfs::read_mft_record_by_index(
                &source,
                &boot,
                offset,
                record_number,
            )
            .map_err(|e| e.to_string())?;

            let stream_name_owned = match stream_ordinal {
                Some(ordinal) => ntfs_stream_name_for_ordinal(&record, ordinal)?,
                None => None,
            };

            let data = if let Some(stream_name) = stream_name_owned.as_deref() {
                data_recovery_fs_ntfs::extract_named_stream(
                    &source,
                    &boot,
                    offset,
                    &record,
                    Some(stream_name),
                )
                .map_err(|e| e.to_string())?
            } else {
                let partial = data_recovery_fs_ntfs::extract_file_data_partial(
                    &source, &boot, offset, &record,
                );
                if !partial.complete {
                    if partial.data.is_empty() || recovered_data_is_padding(&partial.data) {
                        return Err(format!(
                            "NTFS 檔案只救回部分資料，且內容幾乎為空白/填充值（已讀 {} bytes，遺失 {} bytes，壞區段 {} 段），已略過。",
                            partial.bytes_recovered,
                            partial.bytes_lost,
                            partial.bad_sector_ranges.len()
                        ));
                    }
                    let partial_path = quarantine_output_path(&output_path, "partial");
                    if let Some(parent) = partial_path.parent() {
                        std::fs::create_dir_all(parent).map_err(|e| {
                            format!("無法建立部分救援目錄 {}：{}", parent.display(), e)
                        })?;
                    }
                    let manifest =
                        write_recovered(&partial_path, &partial.data, Some(record_number))?;
                    return Ok(format!(
                        "📦 NTFS 部分救援已隔離輸出。\nMFT 紀錄：{}\n已讀：{} bytes\n遺失：{} bytes\n壞區段：{} 段\n隔離路徑：{}\nSHA-256：{}\n提醒：這不是完整成功檔案，請用對應軟體或修復工具嘗試開啟。",
                        record_number,
                        partial.bytes_recovered,
                        partial.bytes_lost,
                        partial.bad_sector_ranges.len(),
                        manifest.output.display(),
                        manifest.sha256
                    ));
                }
                partial.data
            };
            let validation = validate_recovered_data(&data, &output_path)?;
            let manifest = write_recovered(&output_path, &data, Some(record_number))?;
            Ok(format!(
                "✓ 成功復原 NTFS 檔案！\nMFT 紀錄：{}{}\n檔案檢查：{}\n輸出路徑：{}\n大小：{} 位元組\nSHA-256：{}",
                record_number,
                stream_name_owned
                    .as_deref()
                    .map(|name| format!("，資料流：{}", name))
                    .unwrap_or_default(),
                validation,
                manifest.output.display(),
                manifest.bytes_written,
                manifest.sha256
            ))
        }
        "exfat" => {
            let boot = data_recovery_fs_exfat::read_boot_sector(&source, offset)
                .map_err(|e| e.to_string())?;
            let entry_id = selected_id
                .parse::<u64>()
                .map_err(|e| format!("無效的 Entry ID: {}", e))?;
            let entries = data_recovery_fs_exfat::scan_exfat_vfs(&source, &boot, offset)
                .map_err(|e| e.to_string())?;
            let entry = entries
                .iter()
                .find(|e| e.file_offset == entry_id)
                .ok_or_else(|| "找不到該 Entry ID".to_string())?;
            let mut recovery_note = if entry.is_deleted {
                "刪除項目：優先依目錄 entry 與 FAT chain 還原".to_string()
            } else {
                "正常項目：依 exFAT FAT chain / NoFatChain 旗標還原".to_string()
            };

            let mut data = data_recovery_fs_exfat::read_entry_data(
                &source,
                &boot,
                offset,
                entry.first_cluster,
                entry.size,
                entry.no_fat_chain,
            )
            .map_err(|e| e.to_string())?;

            if entry.is_deleted && !entry.no_fat_chain && (data.len() as u64) < entry.size {
                data = read_exfat_contiguous_data(
                    &source,
                    &boot,
                    offset,
                    entry.first_cluster,
                    entry.size,
                )?;
                recovery_note =
                    "刪除項目：FAT chain 不完整，已改用連續 cluster 保守還原".to_string();
            }

            let validation = validate_recovered_data(&data, &output_path)?;
            let manifest = write_recovered(&output_path, &data, None)?;
            Ok(format!(
                "✓ 成功復原 exFAT 檔案！\n還原策略：{}\n檔案檢查：{}\n輸出路徑：{}\n大小：{} 位元組\nSHA-256：{}",
                recovery_note,
                validation,
                manifest.output.display(),
                manifest.bytes_written,
                manifest.sha256
            ))
        }
        "fat32" => {
            let boot = data_recovery_fs_fat32::read_boot_sector(&source, offset)
                .map_err(|e| e.to_string())?;
            let entry_id = selected_id
                .parse::<u64>()
                .map_err(|e| format!("無效的 FAT32 Entry ID: {}", e))?;
            let entries = data_recovery_fs_fat32::scan_fat32_vfs(&source, &boot, offset)
                .map_err(|e| e.to_string())?;
            let entry = entries
                .iter()
                .find(|e| e.file_offset == entry_id)
                .ok_or_else(|| "找不到該 FAT32 Entry ID".to_string())?;
            let recovery_note = if entry.is_deleted {
                "刪除項目：FAT chain 可能已清除，已採 first_cluster 連續讀取 fallback"
            } else {
                "正常項目：依 FAT32 FAT chain 還原"
            };
            let data = data_recovery_fs_fat32::read_entry_data(
                &source,
                &boot,
                offset,
                entry.first_cluster,
                entry.size,
                entry.is_deleted,
            )
            .map_err(|e| e.to_string())?;
            let validation = validate_recovered_data(&data, &output_path)?;
            let manifest = write_recovered(&output_path, &data, None)?;
            Ok(format!(
                "✓ 成功復原 FAT32 檔案！\n還原策略：{}\n檔案檢查：{}\n輸出路徑：{}\n大小：{} 位元組\nSHA-256：{}",
                recovery_note,
                validation,
                manifest.output.display(),
                manifest.bytes_written,
                manifest.sha256
            ))
        }
        "carver" => {
            let config = data_recovery_file_carver::CarverConfig::default();
            let carved = data_recovery_file_carver::carve_source(&source, &config)
                .map_err(|e| e.to_string())?;
            let file_offset = selected_id
                .parse::<u64>()
                .map_err(|e| format!("無效的特徵偏移量：{}", e))?;
            let f = carved
                .iter()
                .find(|c| c.offset == file_offset)
                .ok_or_else(|| "找不到該特徵起點的彫刻檔案".to_string())?;
            let mut data = vec![0u8; f.length as usize];
            let res = source
                .read_at(f.offset, &mut data)
                .map_err(|e| e.to_string())?;
            if res.bytes_read < f.length as usize {
                return Err("無法讀取完整資料".to_string());
            }
            let (_ext, quality, tier) =
                carved_extension_and_quality(&f.file_type, &data, f.confidence);
            if tier != RecoveryConfidenceTier::High {
                if data.is_empty() || recovered_data_is_padding(&data) {
                    return Err("低信心碎片內容幾乎都是 00/FF 填充值，已略過。".to_string());
                }
                let category = if tier == RecoveryConfidenceTier::Medium {
                    "possible"
                } else {
                    "fragments"
                };
                let suspect_path = quarantine_output_path(&output_path, category);
                if let Some(parent) = suspect_path.parent() {
                    std::fs::create_dir_all(parent)
                        .map_err(|e| format!("無法建立可疑檔案目錄 {}：{}", parent.display(), e))?;
                }
                let manifest = write_recovered(&suspect_path, &data, None)?;
                return Ok(format!(
                    "📦 深度掃描可疑檔案已隔離輸出。\n品質標記：{}\n信心分級：{}\n隔離路徑：{}\n大小：{} 位元組\nSHA-256：{}",
                    quality,
                    tier.zh_label(),
                    manifest.output.display(),
                    manifest.bytes_written,
                    manifest.sha256
                ));
            }
            let validation = validate_recovered_data(&data, &output_path)?;
            let manifest = write_recovered(&output_path, &data, None)?;
            Ok(format!(
                "✓ 成功彫刻並復原！\n檔案檢查：{}\n輸出路徑：{}\n大小：{} 位元組\nSHA-256：{}",
                validation,
                manifest.output.display(),
                manifest.bytes_written,
                manifest.sha256
            ))
        }
        "apfs" => {
            let csb = data_recovery_fs_apfs::read_container_superblock(&source, offset)
                .map_err(|e| e.to_string())?;
            let inode_id = selected_id
                .parse::<u64>()
                .map_err(|e| format!("無效的 APFS inode / object id: {}", e))?;
            let entries = data_recovery_fs_apfs::scan_apfs_vfs(&source, &csb, offset)
                .map_err(|e| e.to_string())?;
            let entry = entries
                .iter()
                .find(|e| e.inode == inode_id)
                .ok_or_else(|| "找不到該 APFS inode / object id".to_string())?;
            if entry.is_directory {
                return Err("APFS 目前只支援檔案提取，不支援直接輸出資料夾。".to_string());
            }
            if entry.size > APFS_MAX_EXTRACT_BYTES {
                return Err(format!(
                    "APFS 檔案大小 {} bytes 超過目前桌面端一次性安全提取上限 {} bytes；請先用 CLI 或映像檔分段處理。",
                    entry.size, APFS_MAX_EXTRACT_BYTES
                ));
            }

            let read_report = data_recovery_fs_apfs::extract_apfs_file_data_with_report(
                &source, &csb, offset, inode_id, entry.size,
            )
            .map_err(|e| e.to_string())?;

            if !read_report.complete {
                if read_report.data.is_empty() || recovered_data_is_padding(&read_report.data) {
                    return Err(format!(
                        "APFS 只取得不完整或疑似填充值資料，已略過。extents_found={} extents_used={} 已讀={} 遺失={} reason={}",
                        read_report.extents_found,
                        read_report.extents_used,
                        read_report.bytes_read,
                        read_report.bytes_lost,
                        read_report.reason
                    ));
                }
                let partial_path = quarantine_output_path(&output_path, "partial");
                if let Some(parent) = partial_path.parent() {
                    std::fs::create_dir_all(parent).map_err(|e| {
                        format!("無法建立 APFS 部分救援目錄 {}：{}", parent.display(), e)
                    })?;
                }
                let manifest = write_recovered(&partial_path, &read_report.data, Some(inode_id))?;
                return Ok(format!(
                    "📦 APFS 部分/可疑救援已隔離輸出。\ninode：{}\n檔名：{}\nextents_found：{}\nextents_used：{}\n已讀：{} bytes\n遺失：{} bytes\n洞/缺失區段：{}\nreason：{}\n隔離路徑：{}\nSHA-256：{}\n提醒：APFS 壓縮、clone、snapshot-only、FileVault/加密或 TRIM 後資料仍可能無法完整還原。",
                    inode_id,
                    entry.name,
                    read_report.extents_found,
                    read_report.extents_used,
                    read_report.bytes_read,
                    read_report.bytes_lost,
                    read_report.holes.len(),
                    read_report.reason,
                    manifest.output.display(),
                    manifest.sha256
                ));
            }

            let validation = validate_recovered_data(&read_report.data, &output_path)?;
            let manifest = write_recovered(&output_path, &read_report.data, Some(inode_id))?;
            Ok(format!(
                "✓ 成功復原 APFS 檔案！\ninode：{}\n檔名：{}\n讀取策略：APFS extent map\nextents_used：{}\n檔案檢查：{}\n輸出路徑：{}\n大小：{} 位元組\nSHA-256：{}",
                inode_id,
                entry.name,
                read_report.extents_used,
                validation,
                manifest.output.display(),
                manifest.bytes_written,
                manifest.sha256
            ))
        }
        _ => Err(format!("不支援的模式：{}", mode)),
    }
}

fn unique_output_path(path: PathBuf) -> PathBuf {
    if !path.exists() {
        return path;
    }
    let parent = path.parent().map(PathBuf::from).unwrap_or_default();
    let stem = path
        .file_stem()
        .and_then(|v| v.to_str())
        .unwrap_or("recovered");
    let ext = path.extension().and_then(|v| v.to_str());
    for n in 1..10_000 {
        let name = match ext {
            Some(ext) if !ext.is_empty() => format!("{stem}_{n}.{ext}"),
            _ => format!("{stem}_{n}"),
        };
        let candidate = parent.join(name);
        if !candidate.exists() {
            return candidate;
        }
    }
    path
}

fn recovery_base_dir_from_output(output_path: &Path) -> PathBuf {
    let parent = output_path.parent().map(PathBuf::from).unwrap_or_default();
    if parent
        .file_name()
        .and_then(|v| v.to_str())
        .is_some_and(|name| matches!(name, "recovered" | "partial" | "fragments" | "possible"))
    {
        parent.parent().map(PathBuf::from).unwrap_or(parent)
    } else {
        parent
    }
}

fn quarantine_output_path(output_path: &Path, category: &str) -> PathBuf {
    let base = recovery_base_dir_from_output(output_path);
    let file_name = output_path
        .file_name()
        .and_then(|v| v.to_str())
        .unwrap_or("recovered_fragment.bin");
    unique_output_path(base.join("suspect").join(category).join(file_name))
}

fn success_output_path(output_path: PathBuf) -> PathBuf {
    unique_output_path(output_path)
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

fn write_batch_recovery_report(req: &BatchExtractRequest, messages: &[String]) -> Option<String> {
    let first_output = PathBuf::from(&req.items.first()?.output_path);
    let first_parent = recovery_base_dir_from_output(&first_output);
    std::fs::create_dir_all(&first_parent).ok()?;

    let success = messages.iter().filter(|m| m.starts_with('✓')).count();
    let quarantined = messages.iter().filter(|m| m.starts_with('📦')).count();
    let skipped = messages
        .len()
        .saturating_sub(success)
        .saturating_sub(quarantined);
    let generated_at = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or_default();

    let json_path = first_parent.join("recovery_report.json");
    let html_path = first_parent.join("recovery_report.html");

    let report_json = serde_json::json!({
        "app": "波立克資料救援",
        "reportVersion": "1.0",
        "generatedAtUnix": generated_at,
        "source": req.device,
        "mode": req.mode,
        "requestedItems": req.items.len(),
        "successCount": success,
        "quarantinedCount": quarantined,
        "skippedCount": skipped,
        "folderPolicy": "recovered/ 放完整成功檔；suspect/partial 放部分救援檔；suspect/fragments 放低信心碎片；報告寫在救援根目錄。",
        "qualityPolicy": "只有通過格式/內容檢查的檔案會寫入 recovered/；可疑碎片會隔離到 suspect/，空白/填充值內容會略過。",
        "messages": messages,
    });
    let _ = std::fs::write(&json_path, serde_json::to_string_pretty(&report_json).ok()?);

    let mut rows = String::new();
    for msg in messages {
        let ok = msg.starts_with('✓');
        rows.push_str(&format!(
            "<tr><td>{}</td><td><pre>{}</pre></td></tr>",
            if ok { "成功" } else { "略過/警告" },
            html_escape(msg)
        ));
    }
    let html = format!(
        "<!doctype html><html lang=\"zh-TW\"><meta charset=\"utf-8\"><title>資料救援報告</title><style>body{{font-family:-apple-system,BlinkMacSystemFont,'Noto Sans TC',sans-serif;margin:32px;line-height:1.6;color:#1f2937}}.card{{border:1px solid #e5e7eb;border-radius:14px;padding:18px;margin-bottom:18px}}table{{width:100%;border-collapse:collapse}}td,th{{border-top:1px solid #e5e7eb;padding:10px;text-align:left;vertical-align:top}}pre{{white-space:pre-wrap;margin:0;font-family:ui-monospace,SFMono-Regular,Menlo,monospace;font-size:12px}}.ok{{color:#047857}}.warn{{color:#b45309}}</style><body><h1>波立克資料救援報告</h1><div class=card><p><b>來源：</b>{}</p><p><b>模式：</b>{}</p><p><b>完整成功：</b><span class=ok>{}</span>　<b>隔離輸出：</b><span class=warn>{}</span>　<b>略過：</b><span class=warn>{}</span></p><p><b>資料夾策略：</b><code>recovered/</code> 放通過驗證的完整檔；<code>suspect/</code> 放部分救援或低信心碎片；報告保存在救援根目錄。</p></div><table><thead><tr><th style=\"width:120px\">狀態</th><th>細節</th></tr></thead><tbody>{}</tbody></table></body></html>",
        html_escape(&req.device),
        html_escape(&req.mode),
        success,
        quarantined,
        skipped,
        rows
    );
    let _ = std::fs::write(&html_path, html);
    Some(html_path.display().to_string())
}

fn perform_extract_batch(
    req: BatchExtractRequest,
    notify: Option<&tauri::AppHandle>,
) -> HelperBatchExtractOut {
    if req.items.is_empty() {
        return HelperBatchExtractOut {
            messages: Vec::new(),
            error: Some("沒有選取任何要還原的檔案。".to_string()),
        };
    }
    for item in &req.items {
        let output = success_output_path(PathBuf::from(&item.output_path));
        if let Err(error) = resolve_recovery_storage_identities(Path::new(&req.device), &output) {
            return HelperBatchExtractOut {
                messages: Vec::new(),
                error: Some(format!(
                    "批次還原在寫入前被 storage identity gate 拒絕（{}）：{}",
                    item.output_path, error
                )),
            };
        }
    }
    let mut messages = Vec::new();
    let total = req.items.len();
    for (idx, item) in req.items.iter().enumerate() {
        if let Err(err) = check_cancelled() {
            messages.push(format!("⚠️ 已中止批次還原：{}", err));
            break;
        }
        // 還原前發進度
        if let Some(app) = notify {
            let current_file = std::path::Path::new(&item.output_path)
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or(&item.selected_id)
                .to_string();
            let pct = ((idx * 95 / total.max(1)) as u8).min(95);
            app.emit(
                "extract-progress",
                ExtractProgressPayload {
                    pct,
                    done: idx,
                    total,
                    current_file,
                },
            )
            .ok();
        }
        match perform_extract(
            &req.device,
            &req.mode,
            0,
            &item.selected_id,
            &item.output_path,
        ) {
            Ok(msg) => messages.push(msg),
            Err(err) => messages.push(format!("⚠️ 已略過 ID {}：{}", item.selected_id, err)),
        }
    }
    if let Some(report_path) = write_batch_recovery_report(&req, &messages) {
        messages.push(format!("📄 已建立還原報告：{}", report_path));
    }
    HelperBatchExtractOut {
        messages,
        error: None,
    }
}

// The legacy AppleScript/root Unix-socket prototype was removed during the
// open-source import. A signed, authenticated least-privilege helper must be
// designed before raw-device elevation can return.

fn is_perm_denied(e: &str) -> bool {
    e.contains("Permission denied") || e.contains("os error 1)")
}

fn privileged_access_error() -> String {
    "實體磁碟的舊版管理員權限流程已因安全風險停用；目前只能使用一般權限可讀的測試映像檔。"
        .to_string()
}

fn run_privileged_scan(
    device: &str,
    mode: &str,
    count: usize,
    app: &tauri::AppHandle,
) -> Result<ScanResponse, String> {
    let _ = (device, mode, count, app);
    Err(privileged_access_error())
}

fn run_privileged_extract(
    device: &str,
    mode: &str,
    selected_id: &str,
    output_path: &str,
    app: &tauri::AppHandle,
) -> Result<String, String> {
    let _ = (device, mode, selected_id, output_path, app);
    Err(privileged_access_error())
}

fn run_privileged_extract_batch(
    device: &str,
    mode: &str,
    items: Vec<BatchExtractItem>,
    app: &tauri::AppHandle,
) -> Result<Vec<String>, String> {
    let _ = (device, mode, items, app);
    Err(privileged_access_error())
}

#[cfg(target_os = "macos")]
fn shutdown_privileged_server_for_cancel() {}

// ── Tauri commands ────────────────────────────────────────────────────────────

#[tauri::command]
async fn create_recovery_session_direct(
    source_path: String,
    mode: String,
    output_root: String,
    case_id: Option<String>,
) -> Result<DesktopSessionState, String> {
    let source = source_path.trim().to_string();
    let mode = if mode.trim().is_empty() {
        "unknown".to_string()
    } else {
        mode.trim().to_string()
    };
    let root = session_default_root(&output_root);
    if root.to_string_lossy().starts_with("/dev/") {
        return Err("Session 根目錄不能是 /dev/* 裝置；請選另一顆安全磁碟上的資料夾。".to_string());
    }
    let slug = if source.is_empty() {
        "source".to_string()
    } else {
        safe_image_slug(&source)
    };
    let cid = case_id
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| format!("polik-session-{}-{}", slug, imaging_stamp()));
    let session_dir = root.join(&cid);
    write_desktop_session_manifest(&session_dir, &cid, &source, &mode)?;
    desktop_session_state(&session_dir, true)
}

#[tauri::command]
async fn open_recovery_session_direct(session_dir: String) -> Result<DesktopSessionState, String> {
    let dir = PathBuf::from(session_dir.trim());
    if !dir.exists() || !dir.is_dir() {
        return Err(format!("找不到 Session 資料夾：{}", dir.display()));
    }
    ensure_session_dirs(&dir)?;
    if !session_manifest_path(&dir).exists() {
        return Err(
            "這個資料夾缺少 session_manifest.json；請先建立 Session，或選擇正確的案件資料夾。"
                .to_string(),
        );
    }
    let _ = write_desktop_session_dashboard(&dir);
    write_desktop_session_index(&dir)?;
    desktop_session_state(&dir, false)
}

#[tauri::command]
async fn refresh_recovery_session_index(
    session_dir: String,
) -> Result<DesktopSessionState, String> {
    let dir = PathBuf::from(session_dir.trim());
    if !dir.exists() || !dir.is_dir() {
        return Err(format!("找不到 Session 資料夾：{}", dir.display()));
    }
    ensure_session_dirs(&dir)?;
    let _ = append_desktop_session_timeline_event(
        &dir,
        DesktopTimelineEvent {
            event_type: "session_index_refreshed",
            title: "更新 Session 索引",
            status: "success",
            summary: "重新產生 session_index.html、timeline 與 dashboard。",
            actor: "desktop_app",
            artifacts: Vec::new(),
            details: serde_json::json!({}),
        },
    );
    let _ = write_desktop_session_dashboard(&dir);
    write_desktop_session_index(&dir)?;
    desktop_session_state(&dir, false)
}

#[tauri::command]
async fn get_session_doctor_report_direct(
    session_dir: String,
) -> Result<DesktopSessionDoctorReport, String> {
    let dir = PathBuf::from(session_dir.trim());
    let mut checks = Vec::new();
    let mut warnings = Vec::new();
    let manifest = session_manifest_path(&dir);
    checks.push(DesktopSessionCheck {
        id: "session_dir".to_string(),
        ok: dir.is_dir(),
        detail: dir.to_string_lossy().to_string(),
    });
    checks.push(DesktopSessionCheck {
        id: "manifest".to_string(),
        ok: manifest.is_file(),
        detail: manifest.to_string_lossy().to_string(),
    });
    if manifest.is_file() {
        match read_session_manifest(&dir) {
            Ok(v) => {
                let schema = manifest_text_value(&v, "schema");
                checks.push(DesktopSessionCheck {
                    id: "manifest_json".to_string(),
                    ok: schema.starts_with("polik.recovery.session"),
                    detail: schema,
                });
            }
            Err(e) => checks.push(DesktopSessionCheck {
                id: "manifest_json".to_string(),
                ok: false,
                detail: e,
            }),
        }
    }
    for d in session_required_dirs(&dir) {
        checks.push(DesktopSessionCheck {
            id: format!(
                "dir:{}",
                d.file_name().and_then(|s| s.to_str()).unwrap_or("folder")
            ),
            ok: d.is_dir(),
            detail: d.to_string_lossy().to_string(),
        });
    }
    let dashboard_exists = session_dashboard_html_path(&dir).is_file();
    checks.push(DesktopSessionCheck {
        id: "dashboard_html".to_string(),
        ok: dashboard_exists,
        detail: session_dashboard_html_path(&dir)
            .to_string_lossy()
            .to_string(),
    });
    let delivery_exists = session_delivery_packages_dir(&dir).is_dir();
    checks.push(DesktopSessionCheck {
        id: "delivery_packages_dir".to_string(),
        ok: delivery_exists,
        detail: session_delivery_packages_dir(&dir)
            .to_string_lossy()
            .to_string(),
    });
    let image_count = std::fs::read_dir(dir.join("01_source_image"))
        .map(|rd| rd.filter_map(Result::ok).count())
        .unwrap_or(0);
    let scan_count = std::fs::read_dir(dir.join("02_scan_reports"))
        .map(|rd| rd.filter_map(Result::ok).count())
        .unwrap_or(0);
    let recovery_count = std::fs::read_dir(dir.join("03_recovery_output/recovered"))
        .map(|rd| rd.filter_map(Result::ok).count())
        .unwrap_or(0);
    if image_count == 0 {
        warnings.push(
            "尚未看到 image / map / manifest；若來源是實體磁碟，建議先建立 .img。".to_string(),
        );
    }
    if scan_count == 0 {
        warnings.push("尚未看到掃描報告；掃描後請把報告收進 02_scan_reports。".to_string());
    }
    if recovery_count == 0 {
        warnings.push("recovered/ 目前沒有檔案；若尚未還原，這是正常的。".to_string());
    }
    if !dashboard_exists {
        warnings.push("尚未產生 v2.4 Session Dashboard；請按更新 Dashboard。".to_string());
    }
    let ok = checks.iter().all(|c| c.ok);
    let _ = append_desktop_session_timeline_event(
        &dir,
        DesktopTimelineEvent {
            event_type: "session_doctor",
            title: "執行 Session Doctor",
            status: if ok { "success" } else { "warning" },
            summary: if ok {
                "Session 結構檢查通過。"
            } else {
                "Session 結構有項目需要處理。"
            },
            actor: "desktop_app",
            artifacts: Vec::new(),
            details: serde_json::json!({"ok": ok, "warnings": warnings.clone()}),
        },
    );
    Ok(DesktopSessionDoctorReport {
        ok,
        session_dir: dir.to_string_lossy().to_string(),
        checks,
        warnings,
    })
}

#[tauri::command]
async fn open_session_index_direct(session_dir: String) -> Result<(), String> {
    let dir = PathBuf::from(session_dir.trim());
    let index = session_index_path(&dir);
    if !index.exists() {
        write_desktop_session_index(&dir)?;
    }
    open_path_with_system(&index)
}

#[tauri::command]
async fn import_session_artifact_direct(
    session_dir: String,
    source_path: String,
    artifact_kind: Option<String>,
) -> Result<DesktopSessionArtifactImportResult, String> {
    let dir = PathBuf::from(session_dir.trim());
    let src = PathBuf::from(source_path.trim());
    let kind_ref = artifact_kind.as_deref();
    import_session_artifact_impl(&dir, &src, kind_ref)
}

#[tauri::command]
async fn import_session_artifacts_direct(
    session_dir: String,
    source_paths: Vec<String>,
    artifact_kind: Option<String>,
) -> Result<Vec<DesktopSessionArtifactImportResult>, String> {
    let dir = PathBuf::from(session_dir.trim());
    if source_paths.is_empty() {
        return Err("沒有提供要匯入的檔案路徑。".to_string());
    }
    let mut results = Vec::new();
    for p in source_paths {
        let src = PathBuf::from(p.trim());
        let result = import_session_artifact_impl(&dir, &src, artifact_kind.as_deref())?;
        results.push(result);
    }
    Ok(results)
}

#[tauri::command]
async fn append_session_timeline_event_direct(
    session_dir: String,
    event_type: String,
    title: String,
    status: String,
    summary: String,
) -> Result<DesktopSessionTimelineResult, String> {
    let dir = PathBuf::from(session_dir.trim());
    let result = append_desktop_session_timeline_event(
        &dir,
        DesktopTimelineEvent {
            event_type: event_type.trim(),
            title: title.trim(),
            status: status.trim(),
            summary: summary.trim(),
            actor: "desktop_app",
            artifacts: Vec::new(),
            details: serde_json::json!({}),
        },
    )?;
    write_desktop_session_index(&dir)?;
    Ok(result)
}

#[tauri::command]
async fn open_session_timeline_direct(session_dir: String) -> Result<(), String> {
    let dir = PathBuf::from(session_dir.trim());
    if !session_timeline_html_path(&dir).exists() {
        let manifest = read_session_manifest(&dir)?;
        write_desktop_session_timeline_files(&dir, &manifest)?;
    }
    open_path_with_system(&session_timeline_html_path(&dir))
}

#[tauri::command]
async fn refresh_session_dashboard_direct(
    session_dir: String,
) -> Result<DesktopSessionDashboardResult, String> {
    let dir = PathBuf::from(session_dir.trim());
    if !dir.exists() || !dir.is_dir() {
        return Err(format!("找不到 Session 資料夾：{}", dir.display()));
    }
    let result = write_desktop_session_dashboard(&dir)?;
    let _ = append_desktop_session_timeline_event(
        &dir,
        DesktopTimelineEvent {
            event_type: "session_dashboard_refreshed",
            title: "更新 Session Dashboard",
            status: "success",
            summary: "重新產生 08_dashboard/session_dashboard.html。",
            actor: "desktop_app",
            artifacts: vec![result.dashboard_html.clone()],
            details: serde_json::json!({"score": result.score}),
        },
    );
    write_desktop_session_index(&dir)?;
    Ok(result)
}

#[tauri::command]
async fn open_session_dashboard_direct(session_dir: String) -> Result<(), String> {
    let dir = PathBuf::from(session_dir.trim());
    if !session_dashboard_html_path(&dir).exists() {
        let _ = write_desktop_session_dashboard(&dir)?;
    }
    open_path_with_system(&session_dashboard_html_path(&dir))
}

#[tauri::command]
async fn generate_session_handoff_reports_direct(
    session_dir: String,
) -> Result<DesktopSessionHandoffReportResult, String> {
    let dir = PathBuf::from(session_dir.trim());
    if !dir.exists() || !dir.is_dir() {
        return Err(format!("找不到 Session 資料夾：{}", dir.display()));
    }
    let result = write_desktop_session_handoff_reports(&dir)?;
    let _ = append_desktop_session_timeline_event(
        &dir,
        DesktopTimelineEvent {
            event_type: "handoff_report_generated",
            title: "產生 Session 交接摘要 / 客戶版報告",
            status: "success",
            summary: "已產生 09_reports/customer_summary.html 與 engineer_handoff.html。",
            actor: "desktop_app",
            artifacts: vec![result.customer_html.clone(), result.engineer_html.clone()],
            details: serde_json::json!({}),
        },
    );
    write_desktop_session_index(&dir)?;
    let _ = write_desktop_session_dashboard(&dir);
    Ok(result)
}

#[tauri::command]
async fn open_session_customer_report_direct(session_dir: String) -> Result<(), String> {
    let dir = PathBuf::from(session_dir.trim());
    if !session_customer_report_html_path(&dir).exists() {
        let _ = write_desktop_session_handoff_reports(&dir)?;
    }
    open_path_with_system(&session_customer_report_html_path(&dir))
}

#[tauri::command]
async fn open_session_engineer_report_direct(session_dir: String) -> Result<(), String> {
    let dir = PathBuf::from(session_dir.trim());
    if !session_engineer_report_html_path(&dir).exists() {
        let _ = write_desktop_session_handoff_reports(&dir)?;
    }
    open_path_with_system(&session_engineer_report_html_path(&dir))
}

#[tauri::command]
async fn get_session_workflow_plan(
    source_path: String,
    output_dir: String,
    mode: String,
) -> Result<SessionWorkflowPlan, String> {
    let source = source_path.trim().to_string();
    let mode = if mode.trim().is_empty() {
        "unknown".to_string()
    } else {
        mode.trim().to_string()
    };
    let root = if output_dir.trim().is_empty() {
        std::env::var("HOME")
            .map(|h| format!("{}/Desktop/Polik-Recovery-Sessions", h))
            .unwrap_or_else(|_| "/Users/Shared/Polik-Recovery-Sessions".to_string())
    } else {
        output_dir.trim().trim_end_matches('/').to_string()
    };
    if root.starts_with("/dev/") {
        return Err("Session 根目錄不能是 /dev/* 裝置；請選另一顆磁碟上的資料夾。".to_string());
    }
    let source_slug = if source.is_empty() {
        "source".to_string()
    } else {
        safe_image_slug(&source)
    };
    let case_id = format!("polik-session-{}-{}", source_slug, imaging_stamp());
    let session_dir = format!("{}/{}", root, case_id);
    let create_command = format!(
        "python3 tools/session/create_recovery_session.py --source {} --mode {} --out-root {}",
        shell_quote(if source.is_empty() {
            "/dev/diskX"
        } else {
            &source
        }),
        shell_quote(&mode),
        shell_quote(&root)
    );
    let index_command = format!(
        "python3 tools/session/create_recovery_session.py --session {}",
        shell_quote(&session_dir)
    );
    let doctor_command = format!(
        "python3 tools/session/session_doctor.py --session {}",
        shell_quote(&session_dir)
    );
    let recovery_output_dir = format!("{}/03_recovery_output", session_dir);
    let mut warnings = Vec::new();
    warnings.push("v2.4 建議每一次救援都先建立 session/case 資料夾，把 image、map、scan、recovery、quality、log、timeline 與 dashboard 收在同一處。".to_string());
    if source.starts_with("/dev/") {
        warnings.push("來源是實體磁碟；請先用 image-first 流程在 session/01_source_image 建立 .img，再掃描 .img。".to_string());
    }
    if root.contains("/tmp") {
        warnings.push(
            "Session 根目錄看起來在暫存區；救援案件資料夾應放在外接硬碟或安全位置。".to_string(),
        );
    }
    Ok(SessionWorkflowPlan {
        recommended: true,
        case_id,
        session_root: root.clone(),
        session_dir: session_dir.clone(),
        create_command,
        index_command,
        doctor_command,
        image_manifest_target: format!("{}/01_source_image/", session_dir),
        scan_report_target: format!("{}/02_scan_reports/", session_dir),
        recovery_output_dir: recovery_output_dir.clone(),
        handoff_dir: format!("{}/06_handoff/", session_dir),
        next_step: format!("建立 session 後，建議把還原輸出資料夾改成 {}，並把 scan/recovery 報告匯入 session，再產生 Dashboard。", recovery_output_dir),
        warnings,
        steps: vec![
            "先建立 session/case 資料夾。".to_string(),
            "來源若是實體磁碟，先在 01_source_image 建立 ddrescue image 與 map report。".to_string(),
            "掃描 .img，把報告存到 02_scan_reports。".to_string(),
            "還原到 03_recovery_output，完整與可疑檔案分流。".to_string(),
            "跑 session-dashboard / session-index / session-doctor，產生可交接的 dashboard 與 session_index.html。".to_string(),
        ],
    })
}

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

#[tauri::command]
async fn get_runtime_diagnostics() -> Result<RuntimeDiagnostics, String> {
    let app_path = std::env::current_exe()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|e| format!("unknown: {}", e));
    let mut checks = Vec::new();

    checks.push(diag(
        "app_path",
        "App 執行檔路徑",
        if app_path.starts_with("unknown") {
            "warn"
        } else {
            "ok"
        },
        app_path.clone(),
        "打包版請從 .app 啟動；不要只複製或移動內部執行檔。",
    ));

    let recovery_dir = std::env::var("HOME")
        .map(|h| PathBuf::from(h).join("Desktop").join("已救援"))
        .unwrap_or_else(|_| PathBuf::from("/Users/Shared/已救援"));
    let recovery_status = std::fs::create_dir_all(&recovery_dir)
        .and_then(|_| {
            std::fs::OpenOptions::new()
                .create(true)
                .write(true)
                .truncate(true)
                .open(recovery_dir.join(".polik_write_test"))
        })
        .map(|_| {
            let _ = std::fs::remove_file(recovery_dir.join(".polik_write_test"));
            "ok"
        })
        .unwrap_or("warn");
    checks.push(diag(
        "output_writable",
        "預設輸出資料夾可寫入",
        recovery_status,
        recovery_dir.display().to_string(),
        "若這裡警告，請改選桌面、外接硬碟或你有權限寫入的資料夾。",
    ));

    #[cfg(target_os = "macos")]
    {
        let diskutil_ok = std::process::Command::new("diskutil")
            .arg("list")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);
        checks.push(diag(
            "diskutil",
            "diskutil 可用性",
            if diskutil_ok { "ok" } else { "error" },
            if diskutil_ok {
                "可列出磁碟。"
            } else {
                "diskutil list 執行失敗。"
            },
            "請確認你是在 macOS 上執行，且系統工具 diskutil 可用。",
        ));

        let hdiutil_ok = std::process::Command::new("hdiutil")
            .arg("help")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);
        checks.push(diag(
            "hdiutil",
            "hdiutil 可用性",
            if hdiutil_ok { "ok" } else { "warn" },
            if hdiutil_ok {
                "可建立/掛載測試映像檔。"
            } else {
                "hdiutil 無法執行；真實 APFS 測試流程會失敗。"
            },
            "若要跑 real-apfs-quality-gate，hdiutil 必須可用。一般掃描不一定需要。",
        ));
    }

    #[cfg(not(target_os = "macos"))]
    {
        checks.push(diag(
            "platform",
            "平台支援狀態",
            "warn",
            "目前桌面 App 的磁碟拓樸解析主要針對 macOS。",
            "Windows / Linux 建議先使用 CLI 與映像檔測試流程。",
        ));
    }

    Ok(RuntimeDiagnostics {
        app_version: env!("CARGO_PKG_VERSION").to_string(),
        platform: std::env::consts::OS.to_string(),
        app_path,
        checks,
    })
}

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

fn raw_path_exists(file_path: &str) -> bool {
    if file_path.starts_with(r"\\.\") {
        true
    } else {
        PathBuf::from(file_path).exists()
    }
}

#[tauri::command]
async fn run_recovery_scan(
    app: tauri::AppHandle,
    file_path: String,
    mode: String,
    offset: u64,
    count: usize,
) -> Result<ScanResponse, String> {
    reset_cancellation();
    if !raw_path_exists(&file_path) {
        return Err(format!("找不到指定的來源映像檔或設備：{}", file_path));
    }
    emit_scan_progress(Some(&app), 1, 0, 0, 0, "正在讀取磁碟資料...");
    match perform_scan(&file_path, &mode, offset, count, Some(&app)) {
        Ok(r) => {
            check_cancelled()?;
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
            return Ok(r);
        }
        Err(e) if file_path.starts_with("/dev/") && is_perm_denied(&e) => {}
        Err(e) => return Err(friendly_runtime_error("掃描", &file_path, &e)),
    }
    run_privileged_scan(&file_path, &mode, count, &app)
        .map_err(|e| friendly_runtime_error("掃描", &file_path, &e))
}

#[tauri::command]
async fn run_recovery_extract(
    app: tauri::AppHandle,
    file_path: String,
    mode: String,
    offset: u64,
    selected_id: String,
    output_path: String,
) -> Result<String, String> {
    reset_cancellation();
    if !raw_path_exists(&file_path) {
        return Err(format!("找不到指定的來源映像檔：{}", file_path));
    }
    app.emit("extract-progress", "正在讀取來源磁碟...").ok();
    match perform_extract(&file_path, &mode, offset, &selected_id, &output_path) {
        Ok(msg) => {
            check_cancelled()?;
            app.emit("extract-progress", "100% - 還原完成").ok();
            return Ok(msg);
        }
        Err(e) if file_path.starts_with("/dev/") && is_perm_denied(&e) => {}
        Err(e) => return Err(friendly_runtime_error("還原", &file_path, &e)),
    }
    run_privileged_extract(&file_path, &mode, &selected_id, &output_path, &app)
        .map_err(|e| friendly_runtime_error("還原", &file_path, &e))
}

#[tauri::command]
async fn run_recovery_extract_batch(
    app: tauri::AppHandle,
    file_path: String,
    mode: String,
    items: Vec<BatchExtractItem>,
) -> Result<Vec<String>, String> {
    reset_cancellation();
    if !raw_path_exists(&file_path) {
        return Err(format!("找不到指定的來源映像檔：{}", file_path));
    }
    if items.is_empty() {
        return Err("沒有選取任何要還原的檔案。".to_string());
    }
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
    if !file_path.starts_with("/dev/") {
        let req = BatchExtractRequest {
            device: file_path.clone(),
            mode: mode.clone(),
            items,
        };
        let out = perform_extract_batch(req, Some(&app));
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
        return Ok(out.messages);
    }
    run_privileged_extract_batch(&file_path, &mode, items, &app)
        .map_err(|e| friendly_runtime_error("批次還原", &file_path, &e))
}

#[tauri::command]
async fn cancel_current_operation(app: tauri::AppHandle) -> Result<String, String> {
    request_cancellation();
    app.emit("scan-progress", "正在中止目前掃描 / 還原作業...")
        .ok();
    app.emit("extract-progress", "正在中止目前掃描 / 還原作業...")
        .ok();
    #[cfg(target_os = "macos")]
    {
        shutdown_privileged_server_for_cancel();
    }
    Ok(
        "已送出中止要求；若正在讀取大型磁碟，會在目前讀取區段完成或權限工作站關閉後停止。"
            .to_string(),
    )
}

// ── Thumbnail generation ──────────────────────────────────────────────────────

#[tauri::command]
async fn read_file_thumbnail(
    file_path: String,
    mode: String,
    selected_id: String,
    offset: u64,
) -> Result<String, String> {
    use image::imageops::FilterType;
    use std::io::Cursor;

    // Read up to 512 KB of file data
    let source = open_source(&file_path)?;
    let raw_id = selected_id.parse::<u64>().unwrap_or(0);
    let record_number = raw_id & NTFS_ADS_ID_MASK;

    let data: Vec<u8> = match mode.as_str() {
        "ntfs" => {
            let boot = data_recovery_fs_ntfs::read_boot_sector(&source, offset)
                .map_err(|e| e.to_string())?;
            let mft_offset = offset
                .checked_add(boot.mft_offset().map_err(|e| e.to_string())?)
                .ok_or("MFT offset overflow")?;
            let record_size = boot.file_record_size().map_err(|e| e.to_string())? as usize;
            let rec_offset =
                mft_offset.saturating_add(record_number.saturating_mul(record_size as u64));
            let record = data_recovery_fs_ntfs::read_mft_record(
                &source,
                rec_offset,
                record_size,
                boot.bytes_per_sector,
            )
            .map_err(|e| e.to_string())?;
            let mut full =
                data_recovery_fs_ntfs::extract_file_data(&source, &boot, offset, &record)
                    .map_err(|e| e.to_string())?;
            full.truncate(512 * 1024);
            full
        }
        "exfat" => {
            let boot = data_recovery_fs_exfat::read_boot_sector(&source, offset)
                .map_err(|e| e.to_string())?;
            let entry_id = selected_id
                .parse::<u64>()
                .map_err(|e| format!("無效的 exFAT Entry ID: {}", e))?;
            let entries = data_recovery_fs_exfat::scan_exfat_vfs(&source, &boot, offset)
                .map_err(|e| e.to_string())?;
            let entry = entries
                .iter()
                .find(|e| e.file_offset == entry_id)
                .ok_or_else(|| "找不到該 exFAT Entry ID".to_string())?;
            if entry.is_directory || entry.first_cluster < 2 {
                return Err("目錄或無 cluster 的項目無法產生縮圖".to_string());
            }
            let thumb_len = entry.size.min(512 * 1024);
            let mut data = data_recovery_fs_exfat::read_entry_data(
                &source,
                &boot,
                offset,
                entry.first_cluster,
                thumb_len,
                entry.no_fat_chain,
            )
            .map_err(|e| e.to_string())?;
            data.truncate(512 * 1024);
            data
        }
        "fat32" => {
            let boot = data_recovery_fs_fat32::read_boot_sector(&source, offset)
                .map_err(|e| e.to_string())?;
            let entry_id = selected_id
                .parse::<u64>()
                .map_err(|e| format!("無效的 FAT32 Entry ID: {}", e))?;
            let entries = data_recovery_fs_fat32::scan_fat32_vfs(&source, &boot, offset)
                .map_err(|e| e.to_string())?;
            let entry = entries
                .iter()
                .find(|e| e.file_offset == entry_id)
                .ok_or_else(|| "找不到該 FAT32 Entry ID".to_string())?;
            if entry.is_directory || entry.first_cluster < 2 {
                return Err("目錄或無 cluster 的項目無法產生縮圖".to_string());
            }
            let thumb_len = entry.size.min(512 * 1024);
            let mut data = data_recovery_fs_fat32::read_entry_data(
                &source,
                &boot,
                offset,
                entry.first_cluster,
                thumb_len,
                entry.is_deleted,
            )
            .map_err(|e| e.to_string())?;
            data.truncate(512 * 1024);
            data
        }
        "apfs" => {
            let inode_id = selected_id
                .parse::<u64>()
                .map_err(|e| format!("無效的 APFS inode id: {}", e))?;
            let csb = data_recovery_fs_apfs::read_container_superblock(&source, offset)
                .map_err(|e| e.to_string())?;
            // scan VFS to get file size
            let entries = data_recovery_fs_apfs::scan_apfs_vfs(&source, &csb, offset)
                .map_err(|e| e.to_string())?;
            let entry = entries
                .iter()
                .find(|e| e.inode == inode_id)
                .ok_or_else(|| "找不到該 APFS inode".to_string())?;
            if entry.is_directory {
                return Err("目錄無法產生縮圖".to_string());
            }
            let thumb_size = entry.size.min(512 * 1024);
            // extract up to 512KB
            let mut data = data_recovery_fs_apfs::extract_apfs_file_data(
                &source, &csb, offset, inode_id, thumb_size,
            )
            .map_err(|e| e.to_string())?;
            data.truncate(512 * 1024);
            data
        }
        _ => {
            // carver / unknown: read raw bytes at offset indicated by id
            let read_len = 512 * 1024usize;
            let mut buf = vec![0u8; read_len];
            let res = source
                .read_at(record_number, &mut buf)
                .map_err(|e| e.to_string())?;
            buf.truncate(res.bytes_read);
            buf
        }
    };

    if data.len() < 64 {
        return Err("資料太短，無法產生縮圖".to_string());
    }

    use base64::Engine;

    // 偵測 HEIC（ftyp box at byte 4, brand at byte 8）
    let is_heic = data.len() >= 12
        && &data[4..8] == b"ftyp"
        && matches!(
            &data[8..12],
            b"heic" | b"heis" | b"heim" | b"heix" | b"mif1" | b"msf1"
        );

    // macOS：HEIC 用 sips 轉換
    #[cfg(target_os = "macos")]
    if is_heic {
        let jpg = sips_heic_to_jpeg_thumbnail(&data)?;
        return Ok(base64::engine::general_purpose::STANDARD.encode(&jpg));
    }

    // Windows：HEIC 用 PowerShell + WPF BitmapFrame（透過 WIC，需安裝 HEIF 擴充）
    #[cfg(target_os = "windows")]
    if is_heic {
        let jpg = powershell_heic_to_jpeg_thumbnail(&data)?;
        return Ok(base64::engine::general_purpose::STANDARD.encode(&jpg));
    }

    // Linux：嘗試 heif-convert（選裝工具）
    #[cfg(all(not(target_os = "macos"), not(target_os = "windows")))]
    if is_heic {
        let jpg = heif_convert_thumbnail(&data)?;
        return Ok(base64::engine::general_purpose::STANDARD.encode(&jpg));
    }

    let img = image::load_from_memory(&data).map_err(|e| format!("圖片解碼失敗：{}", e))?;

    let thumb = img.resize(128, 128, FilterType::Triangle);

    let mut jpg_buf: Vec<u8> = Vec::new();
    thumb
        .write_to(&mut Cursor::new(&mut jpg_buf), image::ImageFormat::Jpeg)
        .map_err(|e| format!("縮圖編碼失敗：{}", e))?;

    Ok(base64::engine::general_purpose::STANDARD.encode(&jpg_buf))
}

/// 使用 macOS 內建 sips 將 HEIC bytes 轉換為 128px JPEG bytes。
/// sips 是 macOS 系統工具，不需要額外安裝。
#[cfg(target_os = "macos")]
fn sips_heic_to_jpeg_thumbnail(data: &[u8]) -> Result<Vec<u8>, String> {
    let tmp_dir = std::env::temp_dir();
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let heic_path = tmp_dir.join(format!("polik_thumb_{}.heic", stamp));
    let jpg_path = tmp_dir.join(format!("polik_thumb_{}.jpg", stamp));

    std::fs::write(&heic_path, data).map_err(|e| format!("無法寫入暫存 HEIC：{}", e))?;

    let output = std::process::Command::new("sips")
        .args(["-s", "format", "jpeg", "-Z", "128"])
        .arg(&heic_path)
        .arg("--out")
        .arg(&jpg_path)
        .output()
        .map_err(|e| {
            let _ = std::fs::remove_file(&heic_path);
            format!("sips 執行失敗：{}", e)
        })?;

    let _ = std::fs::remove_file(&heic_path);

    if !output.status.success() {
        let _ = std::fs::remove_file(&jpg_path);
        return Err(format!(
            "sips 轉換失敗：{}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }

    let jpg_data = std::fs::read(&jpg_path).map_err(|e| format!("無法讀取轉換後的 JPEG：{}", e))?;
    let _ = std::fs::remove_file(&jpg_path);

    if jpg_data.len() < 8 {
        return Err("sips 產生的 JPEG 太小".to_string());
    }
    Ok(jpg_data)
}

/// Windows：透過 PowerShell + WPF BitmapFrame 將 HEIC bytes 轉換為 128px JPEG bytes。
/// 需要 Windows 10/11 安裝「HEIF 影像延伸模組」（Microsoft Store 免費）。
#[cfg(target_os = "windows")]
fn powershell_heic_to_jpeg_thumbnail(data: &[u8]) -> Result<Vec<u8>, String> {
    let tmp_dir = std::env::temp_dir();
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let heic_path = tmp_dir.join(format!("polik_thumb_{}.heic", stamp));
    let jpg_path = tmp_dir.join(format!("polik_thumb_{}.jpg", stamp));

    std::fs::write(&heic_path, data).map_err(|e| format!("無法寫入暫存 HEIC：{}", e))?;

    // PowerShell script 使用 WPF BitmapFrame（走 WIC，需 HEIF 擴充）縮至 128px
    let script = format!(
        r#"Add-Type -AssemblyName PresentationCore
$heic = '{heic}'
$jpg  = '{jpg}'
try {{
  $bytes  = [System.IO.File]::ReadAllBytes($heic)
  $ms     = [System.IO.MemoryStream]::new($bytes)
  $frame  = [System.Windows.Media.Imaging.BitmapFrame]::Create($ms)
  $scale  = [System.Windows.Media.ScaleTransform]::new(128.0/$frame.PixelWidth, 128.0/$frame.PixelHeight)
  $scaled = [System.Windows.Media.Imaging.TransformedBitmap]::new($frame, $scale)
  $enc    = [System.Windows.Media.Imaging.JpegBitmapEncoder]::new()
  $enc.Frames.Add([System.Windows.Media.Imaging.BitmapFrame]::Create($scaled))
  $out = [System.IO.FileStream]::new($jpg, 'Create')
  $enc.Save($out)
  $out.Close()
  exit 0
}} catch {{ Write-Error $_; exit 1 }}"#,
        heic = heic_path.display(),
        jpg = jpg_path.display()
    );

    let output = std::process::Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", &script])
        .output()
        .map_err(|e| {
            let _ = std::fs::remove_file(&heic_path);
            format!("PowerShell 執行失敗：{}", e)
        })?;

    let _ = std::fs::remove_file(&heic_path);

    if !output.status.success() {
        let _ = std::fs::remove_file(&jpg_path);
        return Err(format!(
            "PowerShell HEIC 轉換失敗（需安裝 HEIF 影像延伸模組）：{}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }

    let jpg_data = std::fs::read(&jpg_path).map_err(|e| format!("無法讀取轉換後的 JPEG：{}", e))?;
    let _ = std::fs::remove_file(&jpg_path);

    if jpg_data.len() < 8 {
        return Err("PowerShell 產生的 JPEG 太小".to_string());
    }
    Ok(jpg_data)
}

/// Linux：嘗試呼叫 heif-convert（libheif 工具，需另行安裝）將 HEIC 轉換為 JPEG。
/// `sudo apt install libheif-examples` 或 `brew install libheif`
#[cfg(all(not(target_os = "macos"), not(target_os = "windows")))]
fn heif_convert_thumbnail(data: &[u8]) -> Result<Vec<u8>, String> {
    let tmp_dir = std::env::temp_dir();
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let heic_path = tmp_dir.join(format!("polik_thumb_{}.heic", stamp));
    let jpg_path = tmp_dir.join(format!("polik_thumb_{}.jpg", stamp));

    std::fs::write(&heic_path, data).map_err(|e| format!("無法寫入暫存 HEIC：{}", e))?;

    let output = std::process::Command::new("heif-convert")
        .args(["-q", "85"])
        .arg(&heic_path)
        .arg(&jpg_path)
        .output()
        .map_err(|e| {
            let _ = std::fs::remove_file(&heic_path);
            format!("heif-convert 不可用（請安裝 libheif）：{}", e)
        })?;

    let _ = std::fs::remove_file(&heic_path);

    if !output.status.success() {
        let _ = std::fs::remove_file(&jpg_path);
        return Err(format!(
            "heif-convert 失敗：{}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }

    let jpg_data = std::fs::read(&jpg_path).map_err(|e| format!("無法讀取轉換後的 JPEG：{}", e))?;
    let _ = std::fs::remove_file(&jpg_path);

    // 縮至 128x128
    let img = image::load_from_memory(&jpg_data).map_err(|e| format!("JPEG 解碼失敗：{}", e))?;
    let thumb = img.resize(128, 128, image::imageops::FilterType::Triangle);
    let mut out = Vec::new();
    thumb
        .write_to(
            &mut std::io::Cursor::new(&mut out),
            image::ImageFormat::Jpeg,
        )
        .map_err(|e| format!("縮圖編碼失敗：{}", e))?;

    Ok(out)
}

// ── App entry point ───────────────────────────────────────────────────────────

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_log::Builder::default().build())
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            run_recovery_scan,
            run_recovery_extract,
            run_recovery_extract_batch,
            get_recovery_default_dir,
            get_runtime_diagnostics,
            open_report_file,
            cancel_current_operation,
            get_imaging_workflow_plan,
            get_session_workflow_plan,
            create_recovery_session_direct,
            open_recovery_session_direct,
            refresh_recovery_session_index,
            get_session_doctor_report_direct,
            open_session_index_direct,
            import_session_artifact_direct,
            import_session_artifacts_direct,
            append_session_timeline_event_direct,
            open_session_timeline_direct,
            refresh_session_dashboard_direct,
            open_session_dashboard_direct,
            generate_session_handoff_reports_direct,
            open_session_customer_report_direct,
            open_session_engineer_report_direct,
            list_disks,
            read_file_thumbnail
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod tests {
    use super::{open_source, validate_report_open_target};
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
}
