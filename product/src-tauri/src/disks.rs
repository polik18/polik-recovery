use super::*;

#[cfg(target_os = "macos")]
pub(crate) fn diskutil_list_json() -> Result<serde_json::Value, String> {
    let plist = std::process::Command::new("diskutil")
        .args(["list", "-plist"])
        .output()
        .map_err(|e| format!("無法執行 diskutil list -plist：{}", e))?;
    if !plist.status.success() {
        return Err(format!(
            "diskutil list -plist 失敗：{}",
            String::from_utf8_lossy(&plist.stderr).trim()
        ));
    }

    let json = std::process::Command::new("plutil")
        .args(["-convert", "json", "-o", "-", "-"])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .and_then(|mut child| {
            use std::io::Write;
            if let Some(stdin) = child.stdin.as_mut() {
                stdin.write_all(&plist.stdout)?;
            }
            child.wait_with_output()
        })
        .map_err(|e| format!("無法將 diskutil plist 轉成 JSON：{}", e))?;
    if !json.status.success() {
        return Err(format!(
            "plutil JSON 轉換失敗：{}",
            String::from_utf8_lossy(&json.stderr).trim()
        ));
    }
    serde_json::from_slice(&json.stdout).map_err(|e| format!("diskutil plist JSON 無法解析：{}", e))
}

#[cfg(target_os = "macos")]
pub(crate) fn diskutil_partition_ids(value: &serde_json::Value) -> Vec<(String, bool)> {
    let mut result = Vec::new();
    let Some(disks) = value
        .get("AllDisksAndPartitions")
        .and_then(serde_json::Value::as_array)
    else {
        return result;
    };
    for disk in disks {
        let removable = disk
            .get("Removable")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false)
            || disk
                .get("WholeDisk")
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(false)
                && disk
                    .get("OSInternal")
                    .and_then(serde_json::Value::as_bool)
                    .map(|internal| !internal)
                    .unwrap_or(false);
        let Some(partitions) = disk.get("Partitions").and_then(serde_json::Value::as_array) else {
            continue;
        };
        for partition in partitions {
            let Some(id) = partition
                .get("DeviceIdentifier")
                .and_then(serde_json::Value::as_str)
            else {
                continue;
            };
            let content = partition
                .get("Content")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("")
                .to_ascii_lowercase();
            if !id.starts_with("disk")
                || !id.contains('s')
                || content.contains("efi")
                || content.contains("recovery")
                || content.contains("apple_boot")
                || content.contains("apple_apfs_isc")
            {
                continue;
            }
            result.push((id.to_string(), removable));
        }
    }
    result
}

#[cfg(target_os = "macos")]
pub(crate) fn list_disks_impl() -> Result<Vec<DiskInfo>, String> {
    let partition_ids = diskutil_partition_ids(&diskutil_list_json()?)
        .into_iter()
        .collect::<Vec<_>>();

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
pub(crate) fn format_bytes(bytes: u64) -> String {
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
pub(crate) fn list_disks_impl() -> Result<Vec<DiskInfo>, String> {
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
pub(crate) fn list_disks_impl() -> Result<Vec<DiskInfo>, String> {
    Ok(vec![])
}

#[tauri::command]
pub(crate) async fn list_disks() -> Result<Vec<DiskInfo>, String> {
    list_disks_impl()
}

#[tauri::command]
pub(crate) async fn detect_partitions(file_path: String) -> Result<PartitionDetection, String> {
    let source = open_source(&file_path)?;
    let table = data_recovery_partition::detect(&source).map_err(|e| e.to_string())?;
    let kind = match table.kind {
        data_recovery_partition::PartitionTableKind::Mbr => "MBR",
        data_recovery_partition::PartitionTableKind::Gpt => "GPT",
    };
    Ok(PartitionDetection {
        table: kind.to_string(),
        used_backup_header: table.used_backup_header,
        partitions: table
            .partitions
            .into_iter()
            .map(|partition| PartitionInfo {
                index: partition.index,
                offset: partition.offset,
                length: partition.length,
                filesystem: partition.filesystem,
                name: partition.name,
                bootable: partition.bootable,
            })
            .collect(),
    })
}

#[tauri::command]
pub(crate) async fn check_output_isolation(
    source_path: String,
    output_dir: String,
) -> Result<OutputIsolationCheck, String> {
    let source = Path::new(source_path.trim());
    let output_dir = Path::new(output_dir.trim());
    if source.as_os_str().is_empty() || output_dir.as_os_str().is_empty() {
        return Ok(OutputIsolationCheck {
            ok: false,
            message: "請先選擇來源映像檔與輸出資料夾。".to_string(),
            source_stores: Vec::new(),
            output_stores: Vec::new(),
        });
    }
    if source.starts_with("/dev/")
        || source.to_string_lossy().starts_with(r"\\.\")
        || source.to_string_lossy().starts_with(r"\\?\")
    {
        return Ok(OutputIsolationCheck {
            ok: false,
            message: "實體磁碟不可直接掃描，請先建立唯讀映像檔。".to_string(),
            source_stores: Vec::new(),
            output_stores: Vec::new(),
        });
    }

    // The resolver expects a file-like output path and resolves its nearest
    // existing ancestor without creating anything. This probe path is never
    // written; it only lets an already-existing output directory be checked.
    let probe = output_dir.join(".polik-recovery-output-probe");
    match resolve_recovery_storage_identities(source, &probe) {
        Ok((source_identity, output_identity)) => Ok(OutputIsolationCheck {
            ok: true,
            message: format!(
                "來源與輸出已確認位於不同 physical store。來源：{}；輸出：{}",
                source_identity
                    .physical_stores
                    .iter()
                    .cloned()
                    .collect::<Vec<_>>()
                    .join(", "),
                output_identity
                    .physical_stores
                    .iter()
                    .cloned()
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            source_stores: source_identity.physical_stores.iter().cloned().collect(),
            output_stores: output_identity.physical_stores.iter().cloned().collect(),
        }),
        Err(error) => Ok(OutputIsolationCheck {
            ok: false,
            message: format!(
                "輸出必須在另一顆實體磁碟（例如外接 USB 硬碟）；安全檢查未通過：{}",
                error
            ),
            source_stores: Vec::new(),
            output_stores: Vec::new(),
        }),
    }
}

// ── Accurate partition size from diskutil ─────────────────────────────────────

pub(crate) fn diskutil_size_bytes(dev_path: &str) -> u64 {
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
