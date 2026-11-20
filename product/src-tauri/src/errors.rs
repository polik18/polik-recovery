use super::*;

pub(crate) fn friendly_runtime_error(context: &str, target: &str, raw: &str) -> String {
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

    serde_json::to_string(&StructuredErrorPayload {
        code: code.to_string(),
        message_zh: format!("{}失敗：{}", context, reason),
        detail: format!("建議處理：{}；目標：{}；技術訊息：{}", action, target, raw),
    })
    .unwrap_or_else(|_| format!("{}失敗：{}", context, raw))
}
