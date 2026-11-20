use super::*;

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
pub(crate) fn classify_ole_extension(data: &[u8]) -> &'static str {
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
pub(crate) fn classify_mp4_extension(data: &[u8]) -> &'static str {
    let sample = &data[..data.len().min(256)];
    if data_contains(sample, b"qt  ") || data_contains(sample, b"M4V ") {
        "mov"
    } else {
        "mp4"
    }
}

pub(crate) fn confidence_tier(confidence: f32, quality_ok: bool) -> RecoveryConfidenceTier {
    if !quality_ok || confidence < 0.60 {
        RecoveryConfidenceTier::Low
    } else if confidence < 0.85 {
        RecoveryConfidenceTier::Medium
    } else {
        RecoveryConfidenceTier::High
    }
}

pub(crate) fn read_carved_data<S: BlockSource + ?Sized>(
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

pub(crate) fn read_exfat_contiguous_data<S: BlockSource + ?Sized>(
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

pub(crate) fn carved_extension_and_quality(
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
pub(crate) fn data_contains(haystack: &[u8], needle: &[u8]) -> bool {
    if needle.is_empty() || haystack.len() < needle.len() {
        return false;
    }
    haystack.windows(needle.len()).any(|w| w == needle)
}

#[allow(dead_code)]
pub(crate) fn data_rcontains(haystack: &[u8], needle: &[u8]) -> bool {
    if needle.is_empty() || haystack.len() < needle.len() {
        return false;
    }
    haystack.windows(needle.len()).rev().any(|w| w == needle)
}

pub(crate) fn recovered_data_is_padding(data: &[u8]) -> bool {
    if data.is_empty() {
        return true;
    }
    let sample_len = data.len().min(128 * 1024);
    let sample = &data[..sample_len];
    let padding = sample.iter().filter(|&&b| b == 0x00 || b == 0xff).count();
    padding * 100 >= sample_len * 98
}

pub(crate) fn text_like(data: &[u8]) -> bool {
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
pub(crate) fn valid_zip_bytes(data: &[u8]) -> bool {
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
pub(crate) fn valid_png_bytes(data: &[u8]) -> bool {
    data.len() >= 45 && data.starts_with(b"\x89PNG\r\n\x1a\n") && data_rcontains(data, b"IEND")
}

#[allow(dead_code)]
pub(crate) fn valid_jpeg_bytes(data: &[u8]) -> bool {
    data.len() >= 64
        && data.starts_with(b"\xff\xd8\xff")
        && data.ends_with(b"\xff\xd9")
        && data_contains(data, b"\xff\xda")
}

#[allow(dead_code)]
pub(crate) fn valid_pdf_bytes(data: &[u8]) -> bool {
    data.len() >= 128
        && data.starts_with(b"%PDF-1.")
        && data_contains(data, b"startxref")
        && data_rcontains(data, b"%%EOF")
}

#[allow(dead_code)]
pub(crate) fn valid_mp4_bytes(data: &[u8]) -> bool {
    data.len() >= 1024
        && data.get(4..8) == Some(&b"ftyp"[..])
        && data_contains(data, b"moov")
        && data_contains(data, b"mdat")
}

#[allow(dead_code)]
pub(crate) fn valid_ole_bytes(data: &[u8]) -> bool {
    data.len() >= 512
        && data.starts_with(b"\xD0\xCF\x11\xE0\xA1\xB1\x1A\xE1")
        && data.get(28) == Some(&0xFE)
        && data.get(29) == Some(&0xFF)
}

pub(crate) fn validate_recovered_data(data: &[u8], output_path: &Path) -> Result<String, String> {
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

pub(crate) fn perform_extract(
    file_path: &str,
    mode: &str,
    offset: u64,
    selected_id: &str,
    output_path: &str,
    operation_token: &AtomicBool,
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

    if operation_token.load(Ordering::SeqCst) {
        return Err("使用者已中止目前作業。".to_string());
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
            let mft_map = data_recovery_fs_ntfs::read_mft_run_map(&source, &boot, offset)
                .map_err(|e| e.to_string())?;
            let (record_number, stream_ordinal) = decode_ntfs_selected_id(selected_id)?;
            let record = data_recovery_fs_ntfs::read_mft_record_by_index_with_map(
                &source,
                &boot,
                &mft_map,
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

pub(crate) fn unique_output_path(path: PathBuf) -> PathBuf {
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

pub(crate) fn recovery_base_dir_from_output(output_path: &Path) -> PathBuf {
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

pub(crate) fn quarantine_output_path(output_path: &Path, category: &str) -> PathBuf {
    let base = recovery_base_dir_from_output(output_path);
    let file_name = output_path
        .file_name()
        .and_then(|v| v.to_str())
        .unwrap_or("recovered_fragment.bin");
    unique_output_path(base.join("suspect").join(category).join(file_name))
}

pub(crate) fn success_output_path(output_path: PathBuf) -> PathBuf {
    unique_output_path(output_path)
}

pub(crate) fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

pub(crate) fn write_batch_recovery_report(
    req: &BatchExtractRequest,
    messages: &[String],
) -> Option<String> {
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

pub(crate) fn perform_extract_batch(
    req: BatchExtractRequest,
    notify: Option<&tauri::AppHandle>,
    operation_token: &AtomicBool,
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
        if operation_token.load(Ordering::SeqCst) {
            messages.push("⚠️ 已中止批次還原：使用者已中止目前作業。".to_string());
            break;
        }
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
            req.offset,
            &item.selected_id,
            &item.output_path,
            operation_token,
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

// ── Tauri commands ────────────────────────────────────────────────────────────
