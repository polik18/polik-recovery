use super::*;

#[tauri::command]
pub(crate) async fn get_runtime_diagnostics() -> Result<RuntimeDiagnostics, String> {
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
