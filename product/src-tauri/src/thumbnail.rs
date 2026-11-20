use super::*;

// ── Thumbnail generation ──────────────────────────────────────────────────────

#[tauri::command]
pub(crate) async fn read_file_thumbnail(
    file_path: String,
    mode: String,
    selected_id: String,
    offset: u64,
) -> Result<String, String> {
    let cache_key = format!("{}|{}|{}|{}", file_path, mode, offset, selected_id);
    if let Some(cached) = cached_thumbnail(&cache_key) {
        return Ok(cached);
    }
    let result = tauri::async_runtime::spawn_blocking(move || {
        read_file_thumbnail_impl(file_path, mode, selected_id, offset)
    })
    .await
    .map_err(|error| format!("縮圖工作執行緒失敗：{}", error))??;
    cache_thumbnail(cache_key, result.clone());
    Ok(result)
}

pub(crate) fn read_file_thumbnail_impl(
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
            let mft_map = data_recovery_fs_ntfs::read_mft_run_map(&source, &boot, offset)
                .map_err(|e| e.to_string())?;
            let record = data_recovery_fs_ntfs::read_mft_record_by_index_with_map(
                &source,
                &boot,
                &mft_map,
                record_number,
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
pub(crate) fn sips_heic_to_jpeg_thumbnail(data: &[u8]) -> Result<Vec<u8>, String> {
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
pub(crate) fn powershell_heic_to_jpeg_thumbnail(data: &[u8]) -> Result<Vec<u8>, String> {
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
pub(crate) fn heif_convert_thumbnail(data: &[u8]) -> Result<Vec<u8>, String> {
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
