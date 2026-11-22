use super::*;

// ── Core scan / extract logic ─────────────────────────────────────────────────

pub(crate) fn vfs_quality_hint(
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

pub(crate) fn format_vfs_text(tree: &data_recovery_vfs::VfsTree, mode: &str) -> String {
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

pub(crate) fn structured_entries_from_tree(
    tree: &data_recovery_vfs::VfsTree,
    mode: &str,
) -> Vec<ScanEntryPayload> {
    tree.flattened()
        .into_iter()
        .filter_map(|(id, path)| {
            let entry = tree.entry(id)?;
            let (tier, quality) = vfs_quality_hint(mode, entry)
                .map(|(tier, quality)| (tier.to_string(), quality.to_string()))
                .unwrap_or_else(|| (String::new(), String::new()));
            Some(ScanEntryPayload {
                path,
                kind: format!("{:?}", entry.kind),
                status: format!("{:?}", entry.status),
                size: entry
                    .size_bytes
                    .map(|size| size.to_string())
                    .unwrap_or_else(|| "-".to_string()),
                id: id.0.to_string(),
                confidence: String::new(),
                quality,
                tier,
                recovery_mode: String::new(),
                bytes_lost: String::new(),
                bytes_read: String::new(),
                reason: String::new(),
            })
        })
        .collect()
}

pub(crate) fn structured_entries_from_vfs_text(vfs: &str) -> Vec<ScanEntryPayload> {
    vfs.lines()
        .filter_map(|line| {
            let fields = line.split('\t').collect::<Vec<_>>();
            if fields.len() < 3 {
                return None;
            }
            let mut attrs = HashMap::new();
            for field in fields.iter().skip(3) {
                if let Some((key, value)) = field.split_once('=') {
                    attrs.insert(key, value);
                }
            }
            Some(ScanEntryPayload {
                path: fields[0].to_string(),
                kind: fields[1].to_string(),
                status: fields[2].to_string(),
                size: attrs.get("size").copied().unwrap_or("-").to_string(),
                id: attrs.get("id").copied().unwrap_or("0").to_string(),
                confidence: attrs.get("confidence").copied().unwrap_or("").to_string(),
                quality: attrs.get("quality").copied().unwrap_or("").to_string(),
                tier: attrs.get("tier").copied().unwrap_or("").to_string(),
                recovery_mode: attrs
                    .get("recovery_mode")
                    .copied()
                    .unwrap_or("")
                    .to_string(),
                bytes_lost: attrs.get("bytes_lost").copied().unwrap_or("").to_string(),
                bytes_read: attrs.get("bytes_read").copied().unwrap_or("").to_string(),
                reason: attrs.get("reason").copied().unwrap_or("").to_string(),
            })
        })
        .collect()
}

pub(crate) fn prepare_scan_response(mut response: ScanResponse) -> ScanResponse {
    let total_entries = response.entries.len();
    response.total_entries = total_entries;
    response.scan_id.clear();
    response.next_cursor = None;

    if total_entries <= SCAN_PAGE_SIZE {
        return response;
    }

    let scan_id = format!("scan-{}", SCAN_SEQUENCE.fetch_add(1, Ordering::Relaxed));
    let all_entries = std::mem::take(&mut response.entries);
    if let Ok(mut cache) = scan_result_cache().lock() {
        if cache.len() >= SCAN_RESULT_CACHE_CAPACITY {
            if let Some(oldest_id) = cache.keys().next().cloned() {
                cache.remove(&oldest_id);
            }
        }
        cache.insert(scan_id.clone(), all_entries.clone());
    }
    response.scan_id = scan_id;
    response.entries = all_entries.into_iter().take(SCAN_PAGE_SIZE).collect();
    response.next_cursor = Some(SCAN_PAGE_SIZE);
    response
}

pub(crate) fn open_source(file_path: &str) -> Result<FileBlockSource, String> {
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

pub(crate) fn decode_ntfs_selected_id(selected_id: &str) -> Result<(u64, Option<usize>), String> {
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

pub(crate) fn ntfs_stream_name_for_ordinal(
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
pub(crate) fn resolve_recovery_storage_identities(
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
pub(crate) fn resolve_recovery_storage_identities(
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

pub(crate) fn read_ntfs_records_tolerant<S: BlockSource + ?Sized>(
    source: &S,
    boot: &data_recovery_fs_ntfs::NtfsBootSector,
    volume_offset: u64,
    count: usize,
    notify: Option<&tauri::AppHandle>,
) -> Result<Vec<data_recovery_fs_ntfs::MftRecord>, String> {
    let mft_map = data_recovery_fs_ntfs::read_mft_run_map(source, boot, volume_offset)
        .map_err(|e| e.to_string())?;
    let mut records = Vec::new();
    let mut invalid_streak = 0usize;
    let mut first_error: Option<String> = None;

    for index in 0..count {
        check_cancelled()?;
        match data_recovery_fs_ntfs::read_mft_record_by_index_with_map(
            source,
            boot,
            &mft_map,
            index as u64,
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

pub(crate) fn emit_scan_progress(
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

pub(crate) fn emit_scan_activity(
    notify: Option<&tauri::AppHandle>,
    total_hint: usize,
    message: &str,
) {
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

// ── APFS deleted-file index cache ─────────────────────────────────────────────
//
// Finding deleted APFS files reads the whole container. The index from the
// last such scan is kept so extracting deleted files does not rescan per file.
// It is keyed by image path, offset, size and mtime so a changed image is
// never matched against stale results.

#[derive(Clone, PartialEq, Eq)]
pub(crate) struct ApfsIndexKey {
    path: PathBuf,
    offset: u64,
    len: u64,
    modified: Option<std::time::SystemTime>,
}

pub(crate) struct ApfsDeletedCache {
    key: ApfsIndexKey,
    entries: Arc<Vec<data_recovery_fs_apfs::ApfsEntry>>,
    index: Option<Arc<Vec<data_recovery_fs_apfs::spec::SpecVolumeIndex>>>,
}

static APFS_DELETED_CACHE: OnceLock<Mutex<Option<ApfsDeletedCache>>> = OnceLock::new();

pub(crate) fn apfs_index_key(file_path: &str, offset: u64) -> Option<ApfsIndexKey> {
    let meta = std::fs::metadata(file_path).ok()?;
    Some(ApfsIndexKey {
        path: std::fs::canonicalize(file_path).unwrap_or_else(|_| PathBuf::from(file_path)),
        offset,
        len: meta.len(),
        modified: meta.modified().ok(),
    })
}

/// Cached deleted-scan entries and index for this image, if any.
#[allow(clippy::type_complexity)]
pub(crate) fn cached_apfs_deleted(
    file_path: &str,
    offset: u64,
) -> Option<(
    Arc<Vec<data_recovery_fs_apfs::ApfsEntry>>,
    Option<Arc<Vec<data_recovery_fs_apfs::spec::SpecVolumeIndex>>>,
)> {
    let key = apfs_index_key(file_path, offset)?;
    let cache = APFS_DELETED_CACHE
        .get_or_init(|| Mutex::new(None))
        .lock()
        .ok()?;
    cache
        .as_ref()
        .filter(|c| c.key == key)
        .map(|c| (Arc::clone(&c.entries), c.index.clone()))
}

/// Whole-container APFS scan for deleted files, with progress and cancel.
/// Stores the result in the cache for later extraction.
pub(crate) fn apfs_deleted_scan<S: BlockSource + ?Sized>(
    source: &S,
    file_path: &str,
    csb: &data_recovery_fs_apfs::ApfsContainerSuperblock,
    offset: u64,
    notify: Option<&tauri::AppHandle>,
    operation_token: &AtomicBool,
) -> Result<Vec<data_recovery_fs_apfs::ApfsEntry>, String> {
    let block_size = csb.block_size as u64;
    let mut last_pct = u64::MAX;
    let mut progress = |done: u64, total: u64| {
        if operation_token.load(Ordering::SeqCst) || operation_cancelled() {
            return false;
        }
        // 25%–80% of the bar covers the whole-container pass.
        let pct = if total > 0 {
            25 + done.saturating_mul(55) / total
        } else {
            25
        };
        if pct != last_pct {
            last_pct = pct;
            let bytes =
                |blocks: u64| blocks.saturating_mul(block_size).min(usize::MAX as u64) as usize;
            emit_scan_progress(
                notify,
                pct.min(80) as u8,
                bytes(done),
                bytes(total),
                0,
                &format!(
                    "搜尋 APFS 已刪除檔：已讀 {} / {} MB...",
                    bytes(done) / 1_048_576,
                    bytes(total) / 1_048_576
                ),
            );
        }
        true
    };
    let scan = match data_recovery_fs_apfs::scan_apfs_vfs_with_deleted_progress(
        source,
        csb,
        offset,
        &mut progress,
    ) {
        Ok(scan) => scan,
        Err(data_recovery_fs_apfs::ApfsError::Cancelled) => {
            check_cancelled()?;
            return Err("使用者已中止目前作業。".to_string());
        }
        Err(e) => return Err(e.to_string()),
    };
    if let (Some(key), Ok(mut cache)) = (
        apfs_index_key(file_path, offset),
        APFS_DELETED_CACHE.get_or_init(|| Mutex::new(None)).lock(),
    ) {
        *cache = Some(ApfsDeletedCache {
            key,
            entries: Arc::new(scan.entries.clone()),
            index: scan.index.map(Arc::new),
        });
    }
    Ok(scan.entries)
}

pub(crate) fn perform_scan(
    file_path: &str,
    mode: &str,
    offset: u64,
    count: usize,
    include_deleted: bool,
    notify: Option<&tauri::AppHandle>,
    operation_token: &AtomicBool,
) -> Result<ScanResponse, String> {
    if operation_token.load(Ordering::SeqCst) {
        return Err("使用者已中止目前作業。".to_string());
    }
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
                entries: structured_entries_from_tree(&tree, "ntfs"),
                scan_id: String::new(),
                total_entries: 0,
                next_cursor: None,
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
                entries: structured_entries_from_tree(&tree, "exfat"),
                scan_id: String::new(),
                total_entries: 0,
                next_cursor: None,
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
                entries: structured_entries_from_tree(&tree, "fat32"),
                scan_id: String::new(),
                total_entries: 0,
                next_cursor: None,
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
            let entries = if include_deleted {
                apfs_deleted_scan(&source, file_path, &csb, offset, notify, operation_token)?
            } else {
                data_recovery_fs_apfs::scan_apfs_vfs(&source, &csb, offset)
                    .map_err(|e| e.to_string())?
            };
            check_cancelled()?;
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
                "APFS Container Superblock\nblock_size: {}\nblock_count: {}\nchecksum_status: OK (Fletcher64 Match)\nspaceman_oid: {}\nomap_oid: {}\nreaper_oid: {}\nmax_file_systems: {}\nentries_read: {}\ndeleted_scan: {}\n{}\nextract_note: v1.2 已接上未加密 APFS extent 提取；FileVault/加密、壓縮、clone/snapshot-only 與缺失 extent 會改列 suspect/partial。\nencrypted_hint: {}",
                csb.block_size,
                csb.block_count,
                csb.spaceman_oid,
                csb.omap_oid,
                csb.reaper_oid,
                csb.max_file_systems,
                entries.len(),
                if include_deleted {
                    "on (stale FS-tree nodes; content unverified)"
                } else {
                    "off"
                },
                volume_lines,
                encrypted_like
            );
            Ok(ScanResponse {
                report,
                vfs: format_vfs_text(&tree, "apfs"),
                entries: structured_entries_from_tree(&tree, "apfs"),
                scan_id: String::new(),
                total_entries: 0,
                next_cursor: None,
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
            let carve_report = data_recovery_file_carver::carve_source_with_callbacks(
                &source,
                &config,
                |progress| {
                    let pct = if progress.total_bytes > 0 {
                        ((progress.scanned_bytes.saturating_mul(55)) / progress.total_bytes).min(55)
                            as u8
                    } else {
                        0
                    };
                    emit_scan_progress(
                        notify,
                        pct.max(3),
                        progress.scanned_bytes.min(usize::MAX as u64) as usize,
                        progress.total_bytes.min(usize::MAX as u64) as usize,
                        progress.files_found,
                        "正在深度掃描檔案特徵碼...",
                    );
                    if operation_cancelled() {
                        ControlFlow::Break(())
                    } else {
                        ControlFlow::Continue(())
                    }
                },
                |file| {
                    if let Some(app) = notify {
                        app.emit(
                            "scan-carved-file",
                            CarvedFileProgressPayload {
                                file_type: file.file_type,
                                offset: file.offset,
                                length: file.length,
                                sha256: file.sha256,
                                confidence: file.confidence,
                            },
                        )
                        .ok();
                    }
                    if operation_cancelled() {
                        ControlFlow::Break(())
                    } else {
                        ControlFlow::Continue(())
                    }
                },
            )
            .map_err(|e| e.to_string())?;
            if carve_report.cancelled {
                return Err("使用者已中止目前深度掃描。".to_string());
            }
            let carved = carve_report.carved_files;
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
            Ok(ScanResponse {
                report,
                entries: structured_entries_from_vfs_text(&vfs),
                vfs,
                scan_id: String::new(),
                total_entries: 0,
                next_cursor: None,
            })
        }
        _ => Err(format!("不支援的掃描模式：{}", mode)),
    }
}
