use super::*;

pub(crate) fn safe_image_slug(value: &str) -> String {
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

pub(crate) fn shell_quote(value: &str) -> String {
    let escaped = value.replace("'", "'\"'\"'");
    format!("'{}'", escaped)
}

pub(crate) fn imaging_stamp() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    secs.to_string()
}

pub(crate) fn diag(
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

pub(crate) fn session_default_root(output_root: &str) -> PathBuf {
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

pub(crate) fn session_required_dirs(session_dir: &std::path::Path) -> Vec<PathBuf> {
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

pub(crate) fn ensure_session_dirs(session_dir: &std::path::Path) -> Result<(), String> {
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

pub(crate) fn manifest_text_value(manifest: &serde_json::Value, key: &str) -> String {
    manifest
        .get(key)
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string()
}

pub(crate) fn session_manifest_path(session_dir: &std::path::Path) -> PathBuf {
    session_dir.join("session_manifest.json")
}
pub(crate) fn session_index_path(session_dir: &std::path::Path) -> PathBuf {
    session_dir.join("session_index.html")
}
pub(crate) fn session_timeline_dir(session_dir: &std::path::Path) -> PathBuf {
    session_dir.join("07_timeline")
}
pub(crate) fn session_timeline_json_path(session_dir: &std::path::Path) -> PathBuf {
    session_timeline_dir(session_dir).join("session_timeline.json")
}
pub(crate) fn session_timeline_html_path(session_dir: &std::path::Path) -> PathBuf {
    session_timeline_dir(session_dir).join("session_timeline.html")
}
pub(crate) fn session_dashboard_dir(session_dir: &std::path::Path) -> PathBuf {
    session_dir.join("08_dashboard")
}
pub(crate) fn session_dashboard_json_path(session_dir: &std::path::Path) -> PathBuf {
    session_dashboard_dir(session_dir).join("session_dashboard.json")
}
pub(crate) fn session_dashboard_html_path(session_dir: &std::path::Path) -> PathBuf {
    session_dashboard_dir(session_dir).join("session_dashboard.html")
}
pub(crate) fn session_reports_dir(session_dir: &std::path::Path) -> PathBuf {
    session_dir.join("09_reports")
}
pub(crate) fn session_customer_report_html_path(session_dir: &std::path::Path) -> PathBuf {
    session_reports_dir(session_dir).join("customer_summary.html")
}
pub(crate) fn session_engineer_report_html_path(session_dir: &std::path::Path) -> PathBuf {
    session_reports_dir(session_dir).join("engineer_handoff.html")
}
#[allow(dead_code)]
pub(crate) fn session_customer_report_pdf_path(session_dir: &std::path::Path) -> PathBuf {
    session_reports_dir(session_dir).join("customer_summary.pdf")
}
pub(crate) fn session_audit_dir(session_dir: &std::path::Path) -> PathBuf {
    session_dir.join("10_audit")
}
#[allow(dead_code)]
pub(crate) fn session_audit_html_path(session_dir: &std::path::Path) -> PathBuf {
    session_audit_dir(session_dir).join("sample_audit.html")
}
pub(crate) fn session_delivery_dir(session_dir: &std::path::Path) -> PathBuf {
    session_dir.join("11_delivery")
}
pub(crate) fn session_delivery_packages_dir(session_dir: &std::path::Path) -> PathBuf {
    session_delivery_dir(session_dir).join("packages")
}

pub(crate) fn normalize_artifact_kind(
    path: &std::path::Path,
    requested: Option<&str>,
) -> (String, String) {
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

pub(crate) fn unique_destination_path(dir: &std::path::Path, original_name: &str) -> PathBuf {
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

pub(crate) fn copy_dir_recursive(
    src: &std::path::Path,
    dst: &std::path::Path,
) -> Result<(), String> {
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

pub(crate) fn import_session_artifact_impl(
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

pub(crate) fn append_session_artifact_manifest(
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

pub(crate) fn read_session_manifest(
    session_dir: &std::path::Path,
) -> Result<serde_json::Value, String> {
    let path = session_manifest_path(session_dir);
    let text = std::fs::read_to_string(&path)
        .map_err(|e| format!("無法讀取 Session manifest {}：{}", path.display(), e))?;
    serde_json::from_str(&text).map_err(|e| format!("Session manifest JSON 無法解析：{}", e))
}

pub(crate) fn write_session_manifest_value(
    session_dir: &std::path::Path,
    manifest: &serde_json::Value,
) -> Result<(), String> {
    std::fs::write(
        session_manifest_path(session_dir),
        serde_json::to_string_pretty(manifest).unwrap_or_else(|_| "{}".to_string()),
    )
    .map_err(|e| format!("無法寫入 Session manifest：{}", e))
}

pub(crate) fn write_desktop_session_timeline_files(
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

pub(crate) fn desktop_session_state(
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

pub(crate) fn write_desktop_session_readme(
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

pub(crate) fn count_session_files_recursive(dir: &std::path::Path) -> usize {
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

pub(crate) fn size_session_files_recursive(dir: &std::path::Path) -> u64 {
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

pub(crate) fn write_desktop_session_dashboard(
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

pub(crate) fn write_desktop_session_handoff_reports(
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

pub(crate) fn write_desktop_session_index(session_dir: &std::path::Path) -> Result<(), String> {
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

pub(crate) fn write_desktop_session_manifest(
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

pub(crate) fn open_path_with_system(path: &std::path::Path) -> Result<(), String> {
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

pub(crate) fn validate_report_open_target(path: &std::path::Path) -> Result<PathBuf, String> {
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

#[tauri::command]
pub(crate) async fn create_recovery_session_direct(
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
pub(crate) async fn open_recovery_session_direct(
    session_dir: String,
) -> Result<DesktopSessionState, String> {
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
pub(crate) async fn refresh_recovery_session_index(
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
pub(crate) async fn get_session_doctor_report_direct(
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
pub(crate) async fn open_session_index_direct(session_dir: String) -> Result<(), String> {
    let dir = PathBuf::from(session_dir.trim());
    let index = session_index_path(&dir);
    if !index.exists() {
        write_desktop_session_index(&dir)?;
    }
    open_path_with_system(&index)
}

#[tauri::command]
pub(crate) async fn import_session_artifact_direct(
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
pub(crate) async fn import_session_artifacts_direct(
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
pub(crate) async fn append_session_timeline_event_direct(
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
pub(crate) async fn open_session_timeline_direct(session_dir: String) -> Result<(), String> {
    let dir = PathBuf::from(session_dir.trim());
    if !session_timeline_html_path(&dir).exists() {
        let manifest = read_session_manifest(&dir)?;
        write_desktop_session_timeline_files(&dir, &manifest)?;
    }
    open_path_with_system(&session_timeline_html_path(&dir))
}

#[tauri::command]
pub(crate) async fn refresh_session_dashboard_direct(
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
pub(crate) async fn open_session_dashboard_direct(session_dir: String) -> Result<(), String> {
    let dir = PathBuf::from(session_dir.trim());
    if !session_dashboard_html_path(&dir).exists() {
        let _ = write_desktop_session_dashboard(&dir)?;
    }
    open_path_with_system(&session_dashboard_html_path(&dir))
}

#[tauri::command]
pub(crate) async fn generate_session_handoff_reports_direct(
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
pub(crate) async fn open_session_customer_report_direct(session_dir: String) -> Result<(), String> {
    let dir = PathBuf::from(session_dir.trim());
    if !session_customer_report_html_path(&dir).exists() {
        let _ = write_desktop_session_handoff_reports(&dir)?;
    }
    open_path_with_system(&session_customer_report_html_path(&dir))
}

#[tauri::command]
pub(crate) async fn open_session_engineer_report_direct(session_dir: String) -> Result<(), String> {
    let dir = PathBuf::from(session_dir.trim());
    if !session_engineer_report_html_path(&dir).exists() {
        let _ = write_desktop_session_handoff_reports(&dir)?;
    }
    open_path_with_system(&session_engineer_report_html_path(&dir))
}

#[tauri::command]
pub(crate) async fn get_session_workflow_plan(
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
