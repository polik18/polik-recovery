use data_recovery_core::{BlockSource, ByteRange, DEFAULT_CHUNK_SIZE, FileBlockSource, hash_range};
use std::env;
use std::path::PathBuf;
use std::process::ExitCode;

fn main() -> ExitCode {
    match run(env::args().skip(1).collect()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("error: {err}");
            if err.show_usage {
                eprintln!();
                print_usage();
            }
            ExitCode::from(1)
        }
    }
}

#[derive(Debug)]
struct CliError {
    message: String,
    show_usage: bool,
}

impl CliError {
    fn usage(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            show_usage: true,
        }
    }

    fn runtime(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            show_usage: false,
        }
    }
}

impl std::fmt::Display for CliError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

fn run(args: Vec<String>) -> Result<(), CliError> {
    let Some(command) = args.first().map(String::as_str) else {
        // 若無提供任何參數，預設啟動互動式嚮導
        return run_wizard();
    };

    match command {
        "--version" | "-V" | "version" => {
            println!("polik-recovery {}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
        "wizard" => run_wizard(),
        "devices" => list_devices(),
        "info" => {
            let path = arg_value(&args, "--file")?;
            let source = FileBlockSource::open(PathBuf::from(path)).map_err(runtime)?;
            let device = source.device();

            println!("id: {}", device.id);
            println!("display_name: {}", device.display_name);
            println!("path: {}", device.platform_path);
            println!("size_bytes: {}", device.size_bytes);
            println!("logical_sector_size: {}", device.logical_sector_size);
            println!("physical_sector_size: {}", device.physical_sector_size);
            println!("bus_type: {:?}", device.bus_type);
            Ok(())
        }
        "partitions" => {
            let path = arg_value(&args, "--file")?;
            let format = output_format(&args)?;
            partitions(path, format)
        }
        "hash" => {
            let path = arg_value(&args, "--file")?;
            let offset = parse_u64(arg_value(&args, "--offset")?, "--offset")?;
            let length = parse_u64(arg_value(&args, "--length")?, "--length")?;
            let chunk_size = optional_usize(&args, "--chunk-size")?.unwrap_or(DEFAULT_CHUNK_SIZE);

            let source = FileBlockSource::open(PathBuf::from(path)).map_err(runtime)?;
            let summary =
                hash_range(&source, ByteRange::new(offset, length), chunk_size).map_err(runtime)?;

            println!("source: {}", source.device().platform_path);
            println!("offset: {}", summary.range.offset);
            println!("length: {}", summary.range.length);
            println!("chunk_size: {}", summary.chunk_size);
            println!("chunks_read: {}", summary.chunks_read);
            println!("bytes_read: {}", summary.bytes_read);
            println!("sha256: {}", summary.sha256);
            Ok(())
        }
        "ntfs-info" => {
            let path = arg_value(&args, "--file")?;
            let offset = optional_u64(&args, "--offset")?.unwrap_or(0);
            ntfs_info(path, offset)
        }
        "ntfs-record-info" => {
            let path = arg_value(&args, "--file")?;
            let offset = parse_u64(arg_value(&args, "--offset")?, "--offset")?;
            let record_size = optional_usize(&args, "--record-size")?.unwrap_or(1024);
            let sector_size = optional_u16(&args, "--sector-size")?.unwrap_or(512);
            ntfs_record_info(path, offset, record_size, sector_size)
        }
        "ntfs-vfs" => {
            let path = arg_value(&args, "--file")?;
            let record_size = optional_usize(&args, "--record-size")?.unwrap_or(1024);
            let sector_size = optional_u16(&args, "--sector-size")?.unwrap_or(512);
            let count = optional_usize(&args, "--count")?;
            ntfs_vfs(path, record_size, sector_size, count)
        }
        "ntfs-scan-vfs" => {
            let path = arg_value(&args, "--file")?;
            let volume_offset = optional_u64(&args, "--offset")?.unwrap_or(0);
            let count = parse_usize(arg_value(&args, "--count")?, "--count")?;
            let format = output_format(&args)?;
            ntfs_scan_vfs(path, volume_offset, count, format)
        }
        "ntfs-extract" => {
            let path = arg_value(&args, "--file")?;
            let volume_offset = optional_u64(&args, "--offset")?.unwrap_or(0);
            let record_raw = arg_value(&args, "--record")?;
            let record_id = record_raw
                .parse::<u64>()
                .map_err(|_| CliError::usage("--record must be an integer"))?;

            let record_index = (record_id & 0x0000_FFFF_FFFF_FFFF) as usize;
            let stream_index = (record_id >> 48) as usize;

            let output = arg_value(&args, "--output")?;
            let mut stream_name = optional_str(&args, "--stream").map(|s| s.to_string());

            if stream_name.is_none() && stream_index > 0 {
                let source = FileBlockSource::open(PathBuf::from(path)).map_err(runtime)?;
                let boot = data_recovery_fs_ntfs::read_boot_sector(&source, volume_offset)
                    .map_err(runtime)?;
                let mft_map =
                    data_recovery_fs_ntfs::read_mft_run_map(&source, &boot, volume_offset)
                        .map_err(runtime)?;
                let record = data_recovery_fs_ntfs::read_mft_record_by_index_with_map(
                    &source,
                    &boot,
                    &mft_map,
                    record_index as u64,
                )
                .map_err(runtime)?;
                {
                    let streams = data_recovery_fs_ntfs::list_data_streams(&record);
                    let mut named_count = 0;
                    for stream in streams {
                        if stream.name.is_some() {
                            named_count += 1;
                            if named_count == stream_index {
                                stream_name = stream.name;
                                break;
                            }
                        }
                    }
                }
            }

            ntfs_extract(
                path,
                volume_offset,
                record_index,
                output,
                stream_name.as_deref(),
                fixture_storage_mode(&args),
            )
        }
        "ntfs-extract-batch" => {
            let plan_path = arg_value(&args, "--plan")?;
            let output_dir = arg_value(&args, "--output-dir")?;
            let format = output_format(&args)?;
            ntfs_extract_batch(plan_path, output_dir, format, fixture_storage_mode(&args))
        }
        "ntfs-make-plan" => {
            let scan_path = arg_value(&args, "--scan")?;
            let output = arg_value(&args, "--output")?;
            let status = args
                .windows(2)
                .find(|pair| pair[0] == "--status")
                .map(|pair| pair[1].as_str())
                .unwrap_or("deleted");
            ntfs_make_plan(scan_path, status, output)
        }
        "ntfs-report" => {
            let path = arg_value(&args, "--file")?;
            let volume_offset = optional_u64(&args, "--offset")?.unwrap_or(0);
            let count = parse_usize(arg_value(&args, "--count")?, "--count")?;
            let format = output_format(&args)?;
            ntfs_report(path, volume_offset, count, format)
        }
        "exfat-info" => {
            let path = arg_value(&args, "--file")?;
            let volume_offset = optional_u64(&args, "--offset")?.unwrap_or(0);
            exfat_info(path, volume_offset)
        }
        "exfat-scan-vfs" => {
            let path = arg_value(&args, "--file")?;
            let volume_offset = optional_u64(&args, "--offset")?.unwrap_or(0);
            let format = output_format(&args)?;
            exfat_scan_vfs(path, volume_offset, format)
        }
        "exfat-extract" => {
            let path = arg_value(&args, "--file")?;
            let volume_offset = optional_u64(&args, "--offset")?.unwrap_or(0);
            let file_offset = parse_u64(arg_value(&args, "--entry-id")?, "--entry-id")?;
            let output = arg_value(&args, "--output")?;
            exfat_extract(
                path,
                volume_offset,
                file_offset,
                output,
                fixture_storage_mode(&args),
            )
        }
        "fat32-info" => {
            let path = arg_value(&args, "--file")?;
            let volume_offset = optional_u64(&args, "--offset")?.unwrap_or(0);
            fat32_info(path, volume_offset)
        }
        "fat32-scan-vfs" => {
            let path = arg_value(&args, "--file")?;
            let volume_offset = optional_u64(&args, "--offset")?.unwrap_or(0);
            let format = output_format(&args)?;
            fat32_scan_vfs(path, volume_offset, format)
        }
        "fat32-extract" => {
            let path = arg_value(&args, "--file")?;
            let volume_offset = optional_u64(&args, "--offset")?.unwrap_or(0);
            let file_offset = parse_u64(arg_value(&args, "--entry-id")?, "--entry-id")?;
            let output = arg_value(&args, "--output")?;
            fat32_extract(
                path,
                volume_offset,
                file_offset,
                output,
                fixture_storage_mode(&args),
            )
        }
        "apfs-info" => {
            let path = arg_value(&args, "--file")?;
            let volume_offset = optional_u64(&args, "--offset")?.unwrap_or(0);
            apfs_info(path, volume_offset)
        }
        "apfs-scan-vfs" => {
            let path = arg_value(&args, "--file")?;
            let volume_offset = optional_u64(&args, "--offset")?.unwrap_or(0);
            let format = output_format(&args)?;
            let include_deleted = args.iter().any(|arg| arg == "--include-deleted");
            apfs_scan_vfs(path, volume_offset, format, include_deleted)
        }
        "apfs-extract" => {
            let path = arg_value(&args, "--file")?;
            let volume_offset = optional_u64(&args, "--offset")?.unwrap_or(0);
            let inode_id = parse_u64(arg_value(&args, "--inode")?, "--inode")?;
            let output = arg_value(&args, "--output")?;
            apfs_extract(
                path,
                volume_offset,
                inode_id,
                output,
                fixture_storage_mode(&args),
            )
        }
        "carve" => {
            let path = arg_value(&args, "--file")?;
            let output_dir = arg_value(&args, "--output-dir")?;
            let format = output_format(&args)?;
            carve_command(path, output_dir, format, fixture_storage_mode(&args))
        }
        "raw-info" | "raw-hash" => Err(CliError::runtime(
            "RAW_DEVICE_UNSUPPORTED: unsafe raw-device CLI access is disabled; create a controlled image with the documented image-first workflow",
        )),
        "help" | "--help" | "-h" => {
            print_usage();
            Ok(())
        }
        other => Err(CliError::usage(format!("unknown command: {other}"))),
    }
}

fn runtime(err: impl std::fmt::Display) -> CliError {
    CliError::runtime(err.to_string())
}

fn fixture_storage_mode(args: &[String]) -> bool {
    args.iter().any(|arg| arg == "--fixture-storage-isolation")
}

#[cfg(debug_assertions)]
fn nearest_existing_ancestor(path: &std::path::Path) -> Result<PathBuf, CliError> {
    let mut candidate = path;
    loop {
        if candidate.exists() {
            return std::fs::canonicalize(candidate).map_err(runtime);
        }
        candidate = candidate.parent().ok_or_else(|| {
            CliError::runtime(format!(
                "cannot find an existing ancestor for {}",
                path.display()
            ))
        })?;
    }
}

fn fixture_storage_identities(
    source: &std::path::Path,
    output: &std::path::Path,
) -> Result<
    (
        data_recovery_safe_writer::StorageIdentity,
        data_recovery_safe_writer::StorageIdentity,
    ),
    CliError,
> {
    #[cfg(not(debug_assertions))]
    {
        let _ = (source, output);
        return Err(CliError::runtime(
            "fixture storage isolation is unavailable in release builds",
        ));
    }

    #[cfg(debug_assertions)]
    {
        let workspace = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(std::path::Path::parent)
            .ok_or_else(|| CliError::runtime("cannot resolve workspace root"))?;
        let fixture_root = std::fs::canonicalize(workspace.join("fixtures")).map_err(runtime)?;
        let target_root = std::fs::canonicalize(workspace.join("target")).map_err(runtime)?;
        let source = std::fs::canonicalize(source).map_err(runtime)?;
        let output_ancestor = nearest_existing_ancestor(output)?;
        if !source.starts_with(&fixture_root) || !output_ancestor.starts_with(&target_root) {
            return Err(CliError::runtime(
                "fixture storage isolation is restricted to product/fixtures -> product/target",
            ));
        }
        Ok((
            data_recovery_safe_writer::StorageIdentity::debug_fixture(
                "quality fixture source",
                ["fixture-source"],
            ),
            data_recovery_safe_writer::StorageIdentity::debug_fixture(
                "quality fixture output",
                ["fixture-output"],
            ),
        ))
    }
}

fn cli_storage_identities(
    source: &std::path::Path,
    output: &std::path::Path,
    fixture_mode: bool,
) -> Result<
    (
        data_recovery_safe_writer::StorageIdentity,
        data_recovery_safe_writer::StorageIdentity,
    ),
    CliError,
> {
    if fixture_mode {
        fixture_storage_identities(source, output)
    } else {
        data_recovery_safe_writer::resolve_image_storage_identities(source, output).map_err(runtime)
    }
}

fn ntfs_info(path: &str, volume_offset: u64) -> Result<(), CliError> {
    let source = FileBlockSource::open(PathBuf::from(path)).map_err(runtime)?;
    let boot = data_recovery_fs_ntfs::read_boot_sector(&source, volume_offset).map_err(runtime)?;

    println!("source: {}", source.device().platform_path);
    println!("volume_offset: {volume_offset}");
    println!("bytes_per_sector: {}", boot.bytes_per_sector);
    println!("sectors_per_cluster: {}", boot.sectors_per_cluster);
    println!("cluster_size: {}", boot.cluster_size());
    println!("total_sectors: {}", boot.total_sectors);
    println!(
        "total_size_bytes: {}",
        boot.total_size_bytes().map_err(runtime)?
    );
    println!("mft_lcn: {}", boot.mft_lcn);
    println!("mft_offset: {}", boot.mft_offset().map_err(runtime)?);
    println!("mft_mirror_lcn: {}", boot.mft_mirror_lcn);
    println!(
        "mft_mirror_offset: {}",
        boot.mft_mirror_offset().map_err(runtime)?
    );
    println!(
        "file_record_size: {}",
        boot.file_record_size().map_err(runtime)?
    );
    println!(
        "index_buffer_size: {}",
        boot.index_buffer_size().map_err(runtime)?
    );
    println!("volume_serial_number: {:016x}", boot.volume_serial_number);
    Ok(())
}

fn partitions(path: &str, format: OutputFormat) -> Result<(), CliError> {
    let source = FileBlockSource::open(PathBuf::from(path)).map_err(runtime)?;
    let table = data_recovery_partition::detect(&source).map_err(runtime)?;
    match format {
        OutputFormat::Text => {
            println!("source: {}", source.device().platform_path);
            println!("table: {:?}", table.kind);
            println!("used_backup_header: {}", table.used_backup_header);
            for partition in table.partitions {
                println!(
                    "{}\toffset={}\tlength={}\tfs={}\tname={}\tbootable={}",
                    partition.index,
                    partition.offset,
                    partition.length,
                    partition.filesystem.as_deref().unwrap_or("unknown"),
                    partition.name.as_deref().unwrap_or(""),
                    partition.bootable
                );
            }
        }
        OutputFormat::Json => {
            println!("{{");
            println!(
                "  \"source\": \"{}\",",
                json_escape(&source.device().platform_path)
            );
            println!("  \"table\": \"{:?}\",", table.kind);
            println!("  \"used_backup_header\": {},", table.used_backup_header);
            println!("  \"partitions\": [");
            for (index, partition) in table.partitions.iter().enumerate() {
                let comma = if index + 1 == table.partitions.len() {
                    ""
                } else {
                    ","
                };
                println!(
                    "    {{\"index\":{},\"offset\":{},\"length\":{},\"filesystem\":{},\"name\":{},\"bootable\":{}}}{}",
                    partition.index,
                    partition.offset,
                    partition.length,
                    partition
                        .filesystem
                        .as_deref()
                        .map(|value| format!("\"{}\"", json_escape(value)))
                        .unwrap_or_else(|| "null".to_string()),
                    partition
                        .name
                        .as_deref()
                        .map(|value| format!("\"{}\"", json_escape(value)))
                        .unwrap_or_else(|| "null".to_string()),
                    partition.bootable,
                    comma
                );
            }
            println!("  ]\n}}");
        }
    }
    Ok(())
}

fn ntfs_record_info(
    path: &str,
    record_offset: u64,
    record_size: usize,
    sector_size: u16,
) -> Result<(), CliError> {
    let source = FileBlockSource::open(PathBuf::from(path)).map_err(runtime)?;
    let record =
        data_recovery_fs_ntfs::read_mft_record(&source, record_offset, record_size, sector_size)
            .map_err(runtime)?;
    let header = record.header;

    println!("source: {}", source.device().platform_path);
    println!("record_offset: {record_offset}");
    println!("record_size: {record_size}");
    println!("sector_size: {sector_size}");
    println!("signature: {}", String::from_utf8_lossy(&header.signature));
    println!("usa_offset: {}", header.usa_offset);
    println!("usa_count: {}", header.usa_count);
    println!("lsn: {}", header.lsn);
    println!("sequence_number: {}", header.sequence_number);
    println!("hard_link_count: {}", header.hard_link_count);
    println!("first_attribute_offset: {}", header.first_attribute_offset);
    println!("flags_raw: 0x{:04x}", header.flags.raw);
    println!("in_use: {}", header.is_in_use());
    println!("deleted: {}", header.is_deleted());
    println!("directory: {}", header.is_directory());
    println!("bytes_in_use: {}", header.bytes_in_use);
    println!("bytes_allocated: {}", header.bytes_allocated);
    println!("base_file_record: {}", header.base_file_record);
    println!("next_attribute_id: {}", header.next_attribute_id);
    println!(
        "record_number: {}",
        header
            .record_number
            .map(|value| value.to_string())
            .unwrap_or_else(|| "unknown".to_string())
    );
    println!("attribute_count: {}", record.attributes.len());
    for (index, attribute) in record.attributes.iter().enumerate() {
        print_ntfs_attribute(index, attribute);
    }
    Ok(())
}

fn ntfs_vfs(
    path: &str,
    record_size: usize,
    sector_size: u16,
    count: Option<usize>,
) -> Result<(), CliError> {
    let source = FileBlockSource::open(PathBuf::from(path)).map_err(runtime)?;
    let available_records = source.device().size_bytes as usize / record_size;
    let record_count = count.unwrap_or(available_records).min(available_records);
    let mut records = Vec::new();

    for index in 0..record_count {
        let offset = (index * record_size) as u64;
        match data_recovery_fs_ntfs::read_mft_record(&source, offset, record_size, sector_size) {
            Ok(record) => records.push(record),
            Err(err) => eprintln!("warning: skipping record {index} at offset {offset}: {err}"),
        }
    }

    let tree = data_recovery_vfs::tree_from_ntfs_records(&records);
    for (id, path) in tree.flattened() {
        let Some(entry) = tree.entry(id) else {
            continue;
        };
        println!(
            "{}\t{:?}\t{:?}\tsize={}\tid={}",
            path,
            entry.kind,
            entry.status,
            entry
                .size_bytes
                .map(|value| value.to_string())
                .unwrap_or_else(|| "-".to_string()),
            id.0
        );
    }

    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OutputFormat {
    Text,
    Json,
}

fn ntfs_scan_vfs(
    path: &str,
    volume_offset: u64,
    count: usize,
    format: OutputFormat,
) -> Result<(), CliError> {
    let source = FileBlockSource::open(PathBuf::from(path)).map_err(runtime)?;
    let boot = data_recovery_fs_ntfs::read_boot_sector(&source, volume_offset).map_err(runtime)?;
    let mft_map =
        data_recovery_fs_ntfs::read_mft_run_map(&source, &boot, volume_offset).map_err(runtime)?;
    let mut records = Vec::new();
    let mut skipped_records = 0usize;
    for index in 0..count {
        match data_recovery_fs_ntfs::read_mft_record_by_index_with_map(
            &source,
            &boot,
            &mft_map,
            index as u64,
        ) {
            Ok(record) => records.push(record),
            Err(err) => {
                skipped_records += 1;
                eprintln!("warning: skipping NTFS MFT record {index}: {err}");
            }
        }
    }
    let tree = data_recovery_vfs::tree_from_ntfs_records(&records);

    match format {
        OutputFormat::Text => {
            println!("source: {}", source.device().platform_path);
            println!("volume_offset: {volume_offset}");
            println!("mft_offset: {}", boot.mft_offset().map_err(runtime)?);
            println!(
                "file_record_size: {}",
                boot.file_record_size().map_err(runtime)?
            );
            println!("records_requested: {count}");
            println!("records_read: {}", records.len());
            println!("records_skipped: {skipped_records}");
            print_vfs_text(&tree);
        }
        OutputFormat::Json => {
            print_vfs_json(
                source.device().platform_path.as_str(),
                volume_offset,
                boot.mft_offset().map_err(runtime)?,
                boot.file_record_size().map_err(runtime)?,
                records.len(),
                &tree,
            );
        }
    }

    Ok(())
}

fn ntfs_extract(
    path: &str,
    volume_offset: u64,
    record_index: usize,
    output: &str,
    stream_name: Option<&str>,
    fixture_mode: bool,
) -> Result<(), CliError> {
    let output_path = PathBuf::from(output);
    let (source_identity, output_identity) =
        cli_storage_identities(std::path::Path::new(path), &output_path, fixture_mode)?;
    let source = FileBlockSource::open(PathBuf::from(path)).map_err(runtime)?;
    let verified_source =
        data_recovery_safe_writer::verify_open_source_storage(&source, &source_identity)
            .map_err(runtime)?;
    let boot = data_recovery_fs_ntfs::read_boot_sector(&source, volume_offset).map_err(runtime)?;
    let mft_map =
        data_recovery_fs_ntfs::read_mft_run_map(&source, &boot, volume_offset).map_err(runtime)?;
    let record = data_recovery_fs_ntfs::read_mft_record_by_index_with_map(
        &source,
        &boot,
        &mft_map,
        record_index as u64,
    )
    .map_err(runtime)?;

    let (data, is_partial, bytes_lost) = if stream_name.is_none() {
        let partial = data_recovery_fs_ntfs::extract_file_data_partial(
            &source,
            &boot,
            volume_offset,
            &record,
        );
        (partial.data, !partial.complete, partial.bytes_lost)
    } else {
        let data = data_recovery_fs_ntfs::extract_named_stream(
            &source,
            &boot,
            volume_offset,
            &record,
            stream_name,
        )
        .map_err(runtime)?;
        (data, false, 0)
    };

    let manifest = data_recovery_safe_writer::write_recovered_file_with_storage_identities(
        path,
        &output_path,
        &data,
        Some(record_index as u64),
        &verified_source,
        &output_identity,
    )
    .map_err(runtime)?;

    println!("source: {}", source.device().platform_path);
    println!("record: {record_index}");
    if let Some(stream) = stream_name {
        println!("stream: {stream}");
    }
    println!("output: {}", manifest.output.display());
    println!("manifest: {}", manifest.manifest_path.display());
    println!("bytes_written: {}", manifest.bytes_written);
    println!("sha256: {}", manifest.sha256);
    if is_partial {
        println!(
            "status: Partial Recovery ({} bytes lost in bad sectors)",
            bytes_lost
        );
    } else {
        println!("status: Complete");
    }
    Ok(())
}

fn ntfs_make_plan(scan_path: &str, status: &str, output: &str) -> Result<(), CliError> {
    let json = std::fs::read_to_string(scan_path).map_err(runtime)?;
    let filter = match status {
        "deleted" => data_recovery_recovery_plan::StatusFilter::Deleted,
        "allocated" => data_recovery_recovery_plan::StatusFilter::Allocated,
        "all" => data_recovery_recovery_plan::StatusFilter::All,
        _ => {
            return Err(CliError::usage(
                "--status must be deleted, allocated, or all",
            ));
        }
    };
    let plan = data_recovery_recovery_plan::RecoveryPlan::from_vfs_scan_json(&json, filter)
        .map_err(runtime)?;
    std::fs::write(output, plan.to_json().as_bytes()).map_err(runtime)?;
    println!("plan: {output}");
    println!("items: {}", plan.items.len());
    Ok(())
}

fn ntfs_extract_batch(
    plan_path: &str,
    output_dir: &str,
    format: OutputFormat,
    fixture_mode: bool,
) -> Result<(), CliError> {
    let plan = data_recovery_recovery_plan::RecoveryPlan::from_file(plan_path).map_err(runtime)?;
    let output_dir = std::path::PathBuf::from(output_dir);
    let identity_probe = output_dir.join(".storage-identity-preflight");
    let (source_identity, output_identity) = cli_storage_identities(
        std::path::Path::new(&plan.source),
        &identity_probe,
        fixture_mode,
    )?;
    let manifest = data_recovery_recovery_plan::execute_plan_with_storage_identities(
        &plan,
        &output_dir,
        &source_identity,
        &output_identity,
    )
    .map_err(runtime)?;
    match format {
        OutputFormat::Text => print!("{}", manifest.to_text()),
        OutputFormat::Json => print!("{}", manifest.to_json()),
    }
    Ok(())
}

fn ntfs_report(
    path: &str,
    volume_offset: u64,
    count: usize,
    format: OutputFormat,
) -> Result<(), CliError> {
    let source = FileBlockSource::open(PathBuf::from(path)).map_err(runtime)?;
    let boot = data_recovery_fs_ntfs::read_boot_sector(&source, volume_offset).map_err(runtime)?;
    let records = data_recovery_fs_ntfs::read_mft_records(&source, &boot, volume_offset, count)
        .map_err(runtime)?;
    let report = data_recovery_scan_report::NtfsScanReport::from_records(&boot, &records)
        .map_err(runtime)?;

    match format {
        OutputFormat::Text => print!("{}", report.to_text()),
        OutputFormat::Json => print!("{}", report.to_json()),
    }
    Ok(())
}

fn exfat_info(path: &str, volume_offset: u64) -> Result<(), CliError> {
    let source = FileBlockSource::open(PathBuf::from(path)).map_err(runtime)?;
    let boot = data_recovery_fs_exfat::read_boot_sector(&source, volume_offset).map_err(runtime)?;
    println!(
        "OEM ID: {:?}",
        std::str::from_utf8(&boot.oem_name).unwrap_or("")
    );
    println!("partition_offset: {}", boot.partition_offset);
    println!("volume_length: {}", boot.volume_length);
    println!("fat_offset: {}", boot.fat_offset);
    println!("fat_length: {}", boot.fat_length);
    println!("cluster_heap_offset: {}", boot.cluster_heap_offset);
    println!("cluster_count: {}", boot.cluster_count);
    println!(
        "first_cluster_of_root_directory: {}",
        boot.first_cluster_of_root_directory
    );
    println!("volume_serial_number: {:#010x}", boot.volume_serial_number);
    println!("file_system_revision: {:#06x}", boot.file_system_revision);
    println!("volume_flags: {:#06x}", boot.volume_flags);
    println!("bytes_per_sector: {}", boot.sector_size());
    println!(
        "sectors_per_cluster: {}",
        1 << boot.sectors_per_cluster_shift
    );
    println!("cluster_size: {}", boot.cluster_size());
    println!("number_of_fats: {}", boot.number_of_fats);
    Ok(())
}

fn exfat_scan_vfs(path: &str, volume_offset: u64, format: OutputFormat) -> Result<(), CliError> {
    let source = FileBlockSource::open(PathBuf::from(path)).map_err(runtime)?;
    let boot = data_recovery_fs_exfat::read_boot_sector(&source, volume_offset).map_err(runtime)?;
    let entries =
        data_recovery_fs_exfat::scan_exfat_vfs(&source, &boot, volume_offset).map_err(runtime)?;
    let tree = data_recovery_vfs::tree_from_exfat_entries(&entries);

    match format {
        OutputFormat::Text => {
            println!("source: {}", source.device().platform_path);
            println!("volume_offset: {volume_offset}");
            println!("cluster_size: {}", boot.cluster_size());
            println!("entries_read: {}", entries.len());
            print_vfs_text(&tree);
        }
        OutputFormat::Json => {
            print_exfat_vfs_json(
                source.device().platform_path.as_str(),
                volume_offset,
                boot.cluster_size(),
                entries.len(),
                &tree,
            );
        }
    }
    Ok(())
}

fn print_exfat_vfs_json(
    source: &str,
    volume_offset: u64,
    cluster_size: u64,
    entries_read: usize,
    tree: &data_recovery_vfs::VfsTree,
) {
    println!("{{");
    println!("  \"source\": \"{}\",", json_escape(source));
    println!("  \"volume_offset\": {volume_offset},");
    println!("  \"cluster_size\": {cluster_size},");
    println!("  \"entries_read\": {entries_read},");
    println!("  \"entries\": [");
    let flattened = tree.flattened();
    for (index, (id, path)) in flattened.iter().enumerate() {
        let Some(entry) = tree.entry(*id) else {
            continue;
        };
        let comma = if index + 1 == flattened.len() {
            ""
        } else {
            ","
        };
        let size = entry
            .size_bytes
            .map(|value| value.to_string())
            .unwrap_or_else(|| "null".to_string());
        let inode = match &entry.source {
            data_recovery_vfs::VfsSource::Apfs { inode } => inode.to_string(),
            _ => "null".to_string(),
        };
        println!(
            "    {{\"id\":{},\"inode\":{},\"path\":\"{}\",\"kind\":\"{:?}\",\"status\":\"{:?}\",\"size\":{}}}{}",
            id.0,
            inode,
            json_escape(path),
            entry.kind,
            entry.status,
            size,
            comma
        );
    }
    println!("  ]");
    println!("}}");
}

fn exfat_extract(
    path: &str,
    volume_offset: u64,
    file_offset: u64,
    output: &str,
    fixture_mode: bool,
) -> Result<(), CliError> {
    let output_path = PathBuf::from(output);
    let (source_identity, output_identity) =
        cli_storage_identities(std::path::Path::new(path), &output_path, fixture_mode)?;
    let source = FileBlockSource::open(PathBuf::from(path)).map_err(runtime)?;
    let verified_source =
        data_recovery_safe_writer::verify_open_source_storage(&source, &source_identity)
            .map_err(runtime)?;
    let boot = data_recovery_fs_exfat::read_boot_sector(&source, volume_offset).map_err(runtime)?;

    let entries =
        data_recovery_fs_exfat::scan_exfat_vfs(&source, &boot, volume_offset).map_err(runtime)?;
    let Some(entry) = entries.iter().find(|e| e.file_offset == file_offset) else {
        return Err(CliError::runtime(format!(
            "entry at file offset {file_offset} was not found"
        )));
    };

    let read_report = data_recovery_fs_exfat::read_entry_data_with_report(
        &source,
        &boot,
        volume_offset,
        entry.first_cluster,
        entry.size,
        entry.no_fat_chain,
    )
    .map_err(runtime)?;

    let manifest = data_recovery_safe_writer::write_recovered_file_with_storage_identities(
        path,
        &output_path,
        &read_report.data,
        Some(file_offset),
        &verified_source,
        &output_identity,
    )
    .map_err(runtime)?;

    println!("source: {}", source.device().platform_path);
    println!("entry_offset: {file_offset}");
    println!("name: {}", entry.name);
    println!("recovery_mode: {:?}", read_report.mode);
    println!(
        "quality: {}",
        if read_report.complete {
            "complete"
        } else {
            "suspect_partial"
        }
    );
    println!("bytes_requested: {}", read_report.bytes_requested);
    println!("bytes_read: {}", read_report.bytes_read);
    println!("bytes_lost: {}", read_report.bytes_lost);
    println!("clusters_used: {}", read_report.clusters_used);
    println!("reason: {}", read_report.reason);
    println!("output: {}", manifest.output.display());
    println!("manifest: {}", manifest.manifest_path.display());
    println!("bytes_written: {}", manifest.bytes_written);
    println!("sha256: {}", manifest.sha256);
    Ok(())
}

fn fat32_info(path: &str, volume_offset: u64) -> Result<(), CliError> {
    let source = FileBlockSource::open(PathBuf::from(path)).map_err(runtime)?;
    let boot = data_recovery_fs_fat32::read_boot_sector(&source, volume_offset).map_err(runtime)?;
    println!("source: {}", source.device().platform_path);
    println!("volume_offset: {volume_offset}");
    println!("bytes_per_sector: {}", boot.bytes_per_sector);
    println!("sectors_per_cluster: {}", boot.sectors_per_cluster);
    println!("cluster_size: {}", boot.cluster_size());
    println!("reserved_sectors: {}", boot.reserved_sector_count);
    println!("number_of_fats: {}", boot.number_of_fats);
    println!("fat_size: {}", boot.fat_size_32);
    println!("root_cluster: {}", boot.root_cluster);
    println!("cluster_count: {}", boot.cluster_count());
    println!("volume_label: {}", boot.volume_label);
    println!("fs_type_label: {}", boot.fs_type_label);
    Ok(())
}

fn fat32_scan_vfs(path: &str, volume_offset: u64, format: OutputFormat) -> Result<(), CliError> {
    let source = FileBlockSource::open(PathBuf::from(path)).map_err(runtime)?;
    let boot = data_recovery_fs_fat32::read_boot_sector(&source, volume_offset).map_err(runtime)?;
    let entries =
        data_recovery_fs_fat32::scan_fat32_vfs(&source, &boot, volume_offset).map_err(runtime)?;
    let tree = data_recovery_vfs::tree_from_fat32_entries(&entries);

    match format {
        OutputFormat::Text => {
            println!("source: {}", source.device().platform_path);
            println!("volume_offset: {volume_offset}");
            println!("cluster_size: {}", boot.cluster_size());
            println!("entries_read: {}", entries.len());
            print_vfs_text(&tree);
        }
        OutputFormat::Json => {
            print_exfat_vfs_json(
                source.device().platform_path.as_str(),
                volume_offset,
                boot.cluster_size(),
                entries.len(),
                &tree,
            );
        }
    }
    Ok(())
}

fn fat32_extract(
    path: &str,
    volume_offset: u64,
    file_offset: u64,
    output: &str,
    fixture_mode: bool,
) -> Result<(), CliError> {
    let output_path = PathBuf::from(output);
    let (source_identity, output_identity) =
        cli_storage_identities(std::path::Path::new(path), &output_path, fixture_mode)?;
    let source = FileBlockSource::open(PathBuf::from(path)).map_err(runtime)?;
    let verified_source =
        data_recovery_safe_writer::verify_open_source_storage(&source, &source_identity)
            .map_err(runtime)?;
    let boot = data_recovery_fs_fat32::read_boot_sector(&source, volume_offset).map_err(runtime)?;
    let entries =
        data_recovery_fs_fat32::scan_fat32_vfs(&source, &boot, volume_offset).map_err(runtime)?;
    let Some(entry) = entries.iter().find(|e| e.file_offset == file_offset) else {
        return Err(CliError::runtime(format!(
            "FAT32 entry at file offset {file_offset} was not found"
        )));
    };
    let read_report = data_recovery_fs_fat32::read_entry_data_with_report(
        &source,
        &boot,
        volume_offset,
        entry.first_cluster,
        entry.size,
        entry.is_deleted,
    )
    .map_err(runtime)?;
    let manifest = data_recovery_safe_writer::write_recovered_file_with_storage_identities(
        path,
        &output_path,
        &read_report.data,
        Some(file_offset),
        &verified_source,
        &output_identity,
    )
    .map_err(runtime)?;
    println!("source: {}", source.device().platform_path);
    println!("entry_offset: {file_offset}");
    println!("name: {}", entry.name);
    println!("recovery_mode: {:?}", read_report.mode);
    println!(
        "quality: {}",
        if read_report.complete {
            "complete"
        } else {
            "suspect_partial"
        }
    );
    println!("bytes_requested: {}", read_report.bytes_requested);
    println!("bytes_read: {}", read_report.bytes_read);
    println!("bytes_lost: {}", read_report.bytes_lost);
    println!("clusters_used: {}", read_report.clusters_used);
    println!("reason: {}", read_report.reason);
    println!("output: {}", manifest.output.display());
    println!("manifest: {}", manifest.manifest_path.display());
    println!("bytes_written: {}", manifest.bytes_written);
    println!("sha256: {}", manifest.sha256);
    Ok(())
}

fn apfs_scan_vfs(
    path: &str,
    volume_offset: u64,
    format: OutputFormat,
    include_deleted: bool,
) -> Result<(), CliError> {
    let source = FileBlockSource::open(PathBuf::from(path)).map_err(runtime)?;
    let csb = data_recovery_fs_apfs::read_container_superblock(&source, volume_offset)
        .map_err(runtime)?;
    let entries = if include_deleted {
        data_recovery_fs_apfs::scan_apfs_vfs_with_deleted(&source, &csb, volume_offset)
    } else {
        data_recovery_fs_apfs::scan_apfs_vfs(&source, &csb, volume_offset)
    }
    .map_err(runtime)?;
    let tree = data_recovery_vfs::tree_from_apfs_entries(&entries);

    match format {
        OutputFormat::Text => {
            println!("source: {}", source.device().platform_path);
            println!("volume_offset: {volume_offset}");
            println!("block_size: {}", csb.block_size);
            println!("entries_read: {}", entries.len());
            print_vfs_text(&tree);
        }
        OutputFormat::Json => {
            print_apfs_vfs_json(
                source.device().platform_path.as_str(),
                volume_offset,
                csb.block_size,
                entries.len(),
                &tree,
            );
        }
    }
    Ok(())
}

fn print_apfs_vfs_json(
    source: &str,
    volume_offset: u64,
    block_size: u32,
    entries_read: usize,
    tree: &data_recovery_vfs::VfsTree,
) {
    println!("{{");
    println!("  \"source\": \"{}\",", json_escape(source));
    println!("  \"volume_offset\": {volume_offset},");
    println!("  \"block_size\": {block_size},");
    println!("  \"entries_read\": {entries_read},");
    println!("  \"entries\": [");
    let flattened = tree.flattened();
    for (index, (id, path)) in flattened.iter().enumerate() {
        let Some(entry) = tree.entry(*id) else {
            continue;
        };
        let comma = if index + 1 == flattened.len() {
            ""
        } else {
            ","
        };
        let size = entry
            .size_bytes
            .map(|value| value.to_string())
            .unwrap_or_else(|| "null".to_string());
        let inode = match &entry.source {
            data_recovery_vfs::VfsSource::Apfs { inode } => inode.to_string(),
            _ => "null".to_string(),
        };
        println!(
            "    {{\"id\":{},\"inode\":{},\"path\":\"{}\",\"kind\":\"{:?}\",\"status\":\"{:?}\",\"size\":{}}}{}",
            id.0,
            inode,
            json_escape(path),
            entry.kind,
            entry.status,
            size,
            comma
        );
    }
    println!("  ]");
    println!("}}");
}

fn apfs_extract(
    path: &str,
    volume_offset: u64,
    inode_id: u64,
    output: &str,
    fixture_mode: bool,
) -> Result<(), CliError> {
    let output_path = PathBuf::from(output);
    let (source_identity, output_identity) =
        cli_storage_identities(std::path::Path::new(path), &output_path, fixture_mode)?;
    let source = FileBlockSource::open(PathBuf::from(path)).map_err(runtime)?;
    let verified_source =
        data_recovery_safe_writer::verify_open_source_storage(&source, &source_identity)
            .map_err(runtime)?;
    let csb = data_recovery_fs_apfs::read_container_superblock(&source, volume_offset)
        .map_err(runtime)?;

    let mut entries =
        data_recovery_fs_apfs::scan_apfs_vfs(&source, &csb, volume_offset).map_err(runtime)?;
    if !entries.iter().any(|e| e.inode == inode_id) {
        // The inode may belong to a deleted file only visible in stale nodes.
        entries = data_recovery_fs_apfs::scan_apfs_vfs_with_deleted(&source, &csb, volume_offset)
            .map_err(runtime)?;
    }
    let Some(entry) = entries.iter().find(|e| e.inode == inode_id) else {
        return Err(CliError::runtime(format!(
            "entry at inode {inode_id} was not found"
        )));
    };

    let read_report = data_recovery_fs_apfs::extract_apfs_file_data_with_report(
        &source,
        &csb,
        volume_offset,
        inode_id,
        entry.size,
    )
    .map_err(runtime)?;

    let manifest = data_recovery_safe_writer::write_recovered_file_with_storage_identities(
        path,
        &output_path,
        &read_report.data,
        Some(inode_id),
        &verified_source,
        &output_identity,
    )
    .map_err(runtime)?;

    println!("source: {}", source.device().platform_path);
    println!("inode_id: {inode_id}");
    println!("name: {}", entry.name);
    println!("recovery_mode: APFS extent map");
    println!(
        "quality: {}",
        if read_report.complete {
            "complete"
        } else {
            "suspect_partial"
        }
    );
    println!("bytes_requested: {}", read_report.bytes_requested);
    println!("bytes_read: {}", read_report.bytes_read);
    println!("bytes_lost: {}", read_report.bytes_lost);
    println!("extents_found: {}", read_report.extents_found);
    println!("extents_used: {}", read_report.extents_used);
    println!("holes: {}", read_report.holes.len());
    println!("encrypted_hint: {}", read_report.encrypted_hint);
    println!("reason: {}", read_report.reason);
    println!("output: {}", manifest.output.display());
    println!("manifest: {}", manifest.manifest_path.display());
    println!("bytes_written: {}", manifest.bytes_written);
    println!("sha256: {}", manifest.sha256);
    Ok(())
}

fn apfs_info(path: &str, volume_offset: u64) -> Result<(), CliError> {
    let source = FileBlockSource::open(PathBuf::from(path)).map_err(runtime)?;
    let csb = data_recovery_fs_apfs::read_container_superblock(&source, volume_offset)
        .map_err(runtime)?;

    println!("Container Block Size: {}", csb.block_size);
    println!("Container Block Count: {}", csb.block_count);
    println!("Container UUID: {}", uuid_to_string(&csb.uuid));
    println!("Space Manager OID: {}", csb.spaceman_oid);
    println!("Object Map OID: {}", csb.omap_oid);
    println!("Reaper OID: {}", csb.reaper_oid);
    println!("Max File Systems: {}", csb.max_file_systems);
    for (i, fs_oid) in csb.fs_oids.iter().enumerate() {
        println!("  Volume [{i}] OID: {fs_oid}");
    }

    Ok(())
}

fn uuid_to_string(uuid: &[u8; 16]) -> String {
    format!(
        "{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
        uuid[0],
        uuid[1],
        uuid[2],
        uuid[3],
        uuid[4],
        uuid[5],
        uuid[6],
        uuid[7],
        uuid[8],
        uuid[9],
        uuid[10],
        uuid[11],
        uuid[12],
        uuid[13],
        uuid[14],
        uuid[15]
    )
}

fn carve_command(
    path: &str,
    output_dir: &str,
    format: OutputFormat,
    fixture_mode: bool,
) -> Result<(), CliError> {
    let out_path = PathBuf::from(output_dir);
    let identity_probe = out_path
        .join("recovered")
        .join(".storage-identity-preflight");
    let (source_identity, output_identity) =
        cli_storage_identities(std::path::Path::new(path), &identity_probe, fixture_mode)?;
    let source = FileBlockSource::open(PathBuf::from(path)).map_err(runtime)?;
    let verified_source =
        data_recovery_safe_writer::verify_open_source_storage(&source, &source_identity)
            .map_err(runtime)?;
    let config = data_recovery_file_carver::CarverConfig::default();

    println!("Scanning for file signatures with v0.5 format validation...");
    let carve_report =
        data_recovery_file_carver::carve_source_with_report(&source, &config).map_err(runtime)?;
    let carved_files = &carve_report.carved_files;

    let recovered_dir = out_path.join("recovered");
    let possible_dir = out_path.join("suspect").join("possible");
    let fragment_dir = out_path.join("suspect").join("fragments");
    for dir in [&recovered_dir, &possible_dir, &fragment_dir] {
        if !dir.exists() {
            std::fs::create_dir_all(dir).map_err(runtime)?;
        }
    }

    struct CarveSummary {
        file_type: String,
        offset: u64,
        length: u64,
        output_file: PathBuf,
        sha256: String,
        confidence: f32,
        quality: String,
        tier: String,
    }

    let mut summaries = Vec::new();
    let mut skipped = 0usize;

    for f in carved_files {
        let mut data = vec![0_u8; f.length as usize];
        let read_res = source.read_at(f.offset, &mut data).map_err(runtime)?;
        if read_res.bytes_read < f.length as usize {
            skipped += 1;
            continue;
        }

        let validation = data_recovery_file_carver::validate_carved_bytes(&f.file_type, &data);
        let confidence = f.confidence.min(validation.confidence);
        let tier = if validation.ok && confidence >= 0.85 {
            "high"
        } else if validation.ok && confidence >= 0.60 {
            "medium"
        } else {
            "low"
        };
        if tier == "low" && !validation.ok {
            skipped += 1;
            continue;
        }
        let target_dir = match tier {
            "high" => &recovered_dir,
            "medium" => &possible_dir,
            _ => &fragment_dir,
        };
        let filename = format!("carved_{}_{}.{}", f.offset, f.length, validation.extension);
        let output_file = target_dir.join(filename);

        if let Ok(manifest) =
            data_recovery_safe_writer::write_recovered_file_with_storage_identities(
                path,
                &output_file,
                &data,
                None,
                &verified_source,
                &output_identity,
            )
        {
            summaries.push(CarveSummary {
                file_type: f.file_type.clone(),
                offset: f.offset,
                length: f.length,
                output_file: manifest.output,
                sha256: manifest.sha256,
                confidence,
                quality: validation.label.to_string(),
                tier: tier.to_string(),
            });
        }
    }

    let mut manifest_content = String::new();
    manifest_content.push_str("{\n");
    manifest_content.push_str(&format!("  \"source\": \"{}\",\n", json_escape(path)));
    manifest_content.push_str(&format!("  \"skipped\": {},\n", skipped));
    manifest_content.push_str("  \"unreadable_ranges\": [\n");
    for (idx, range) in carve_report.unreadable_ranges.iter().enumerate() {
        let comma = if idx + 1 == carve_report.unreadable_ranges.len() {
            ""
        } else {
            ","
        };
        manifest_content.push_str(&format!(
            "    {{\"offset\":{},\"length\":{},\"errno\":{}}}{}\n",
            range.offset, range.length, range.errno, comma
        ));
    }
    manifest_content.push_str("  ],\n");
    manifest_content.push_str("  \"carved_items\": [\n");
    for (idx, item) in summaries.iter().enumerate() {
        let comma = if idx + 1 == summaries.len() { "" } else { "," };
        manifest_content.push_str(&format!(
            "    {{\n      \"file_type\": \"{}\",\n      \"offset\": {},\n      \"length\": {},\n      \"output_path\": \"{}\",\n      \"sha256\": \"{}\",\n      \"confidence\": {},\n      \"quality\": \"{}\",\n      \"tier\": \"{}\"\n    }}{}\n",
            item.file_type,
            item.offset,
            item.length,
            json_escape(item.output_file.to_str().unwrap_or("")),
            item.sha256,
            item.confidence,
            json_escape(&item.quality),
            item.tier,
            comma
        ));
    }
    manifest_content.push_str("  ]\n");
    manifest_content.push_str("}\n");

    let manifest_path = out_path.join("carve_manifest.json");
    std::fs::write(&manifest_path, manifest_content).map_err(runtime)?;

    match format {
        OutputFormat::Text => {
            println!("Carving complete!");
            println!(
                "High-confidence files: {}",
                summaries.iter().filter(|s| s.tier == "high").count()
            );
            println!(
                "Quarantined possible/fragments: {}",
                summaries.iter().filter(|s| s.tier != "high").count()
            );
            println!("Skipped invalid fragments: {}", skipped);
            println!(
                "Unreadable ranges: {}",
                carve_report.unreadable_ranges.len()
            );
            println!("Results saved to: {}", out_path.display());
            println!("Summary manifest: {}", manifest_path.display());
            for item in &summaries {
                println!(
                    "  - [{} / {}] Offset: {}, Size: {} bytes -> {} ({})",
                    item.file_type,
                    item.tier,
                    item.offset,
                    item.length,
                    item.output_file.file_name().unwrap().to_string_lossy(),
                    item.quality
                );
            }
        }
        OutputFormat::Json => {
            print!(
                "{}",
                std::fs::read_to_string(&manifest_path).unwrap_or_default()
            );
        }
    }

    Ok(())
}

fn print_vfs_text(tree: &data_recovery_vfs::VfsTree) {
    for (id, path) in tree.flattened() {
        let Some(entry) = tree.entry(id) else {
            continue;
        };
        println!(
            "{}\t{:?}\t{:?}\tsize={}\tid={}",
            path,
            entry.kind,
            entry.status,
            entry
                .size_bytes
                .map(|value| value.to_string())
                .unwrap_or_else(|| "-".to_string()),
            id.0
        );
    }
}

fn print_vfs_json(
    source: &str,
    volume_offset: u64,
    mft_offset: u64,
    file_record_size: u64,
    records_read: usize,
    tree: &data_recovery_vfs::VfsTree,
) {
    println!("{{");
    println!("  \"source\": \"{}\",", json_escape(source));
    println!("  \"volume_offset\": {volume_offset},");
    println!("  \"mft_offset\": {mft_offset},");
    println!("  \"file_record_size\": {file_record_size},");
    println!("  \"records_read\": {records_read},");
    println!("  \"entries\": [");
    let flattened = tree.flattened();
    for (index, (id, path)) in flattened.iter().enumerate() {
        let Some(entry) = tree.entry(*id) else {
            continue;
        };
        let comma = if index + 1 == flattened.len() {
            ""
        } else {
            ","
        };
        let size = entry
            .size_bytes
            .map(|value| value.to_string())
            .unwrap_or_else(|| "null".to_string());
        let inode = match &entry.source {
            data_recovery_vfs::VfsSource::Apfs { inode } => inode.to_string(),
            _ => "null".to_string(),
        };
        println!(
            "    {{\"id\":{},\"inode\":{},\"path\":\"{}\",\"kind\":\"{:?}\",\"status\":\"{:?}\",\"size\":{}}}{}",
            id.0,
            inode,
            json_escape(path),
            entry.kind,
            entry.status,
            size,
            comma
        );
    }
    println!("  ]");
    println!("}}");
}

fn print_ntfs_attribute(index: usize, attribute: &data_recovery_fs_ntfs::NtfsAttribute) {
    println!(
        "attribute[{index}].type: {:?}",
        attribute.header.attribute_type
    );
    println!(
        "attribute[{index}].raw_type: 0x{:08x}",
        attribute.header.raw_type
    );
    println!("attribute[{index}].length: {}", attribute.header.length);
    println!(
        "attribute[{index}].non_resident: {}",
        attribute.header.non_resident
    );
    println!("attribute[{index}].id: {}", attribute.header.attribute_id);
    if let Some(name) = &attribute.header.attribute_name {
        println!("attribute[{index}].attribute_name: {name}");
    }

    match &attribute.value {
        data_recovery_fs_ntfs::AttributeValue::StandardInformation(value) => {
            println!("attribute[{index}].created_time: {}", value.created_time);
            println!("attribute[{index}].modified_time: {}", value.modified_time);
            println!(
                "attribute[{index}].mft_modified_time: {}",
                value.mft_modified_time
            );
            println!("attribute[{index}].accessed_time: {}", value.accessed_time);
            println!(
                "attribute[{index}].file_attributes: 0x{:08x}",
                value.file_attributes
            );
            if let Some(owner_id) = value.owner_id {
                println!("attribute[{index}].owner_id: {owner_id}");
            }
            if let Some(security_id) = value.security_id {
                println!("attribute[{index}].security_id: {security_id}");
            }
        }
        data_recovery_fs_ntfs::AttributeValue::FileName(value) => {
            println!("attribute[{index}].name: {}", value.name);
            println!("attribute[{index}].namespace: {:?}", value.namespace);
            println!(
                "attribute[{index}].parent_record_number: {}",
                value.parent_record_number
            );
            println!(
                "attribute[{index}].parent_sequence_number: {}",
                value.parent_sequence_number
            );
            println!(
                "attribute[{index}].allocated_size: {}",
                value.allocated_size
            );
            println!("attribute[{index}].real_size: {}", value.real_size);
            println!(
                "attribute[{index}].file_attributes: 0x{:08x}",
                value.file_attributes
            );
        }
        data_recovery_fs_ntfs::AttributeValue::ResidentRaw(value) => {
            println!("attribute[{index}].resident_value_size: {}", value.len());
        }
        data_recovery_fs_ntfs::AttributeValue::DataRuns(runs) => {
            println!("attribute[{index}].data_run_count: {}", runs.len());
            for (run_index, run) in runs.iter().enumerate() {
                println!(
                    "attribute[{index}].data_run[{run_index}]: vcn={} clusters={} lcn={}",
                    run.vcn_start,
                    run.cluster_count,
                    run.lcn_start
                        .map(|value| value.to_string())
                        .unwrap_or_else(|| "sparse".to_string())
                );
            }
        }
        data_recovery_fs_ntfs::AttributeValue::NonResident => {}
    }
}

#[cfg(target_os = "macos")]
fn list_devices() -> Result<(), CliError> {
    let devices = data_recovery_device_macos::list_whole_disks().map_err(runtime)?;
    for disk in devices {
        let device = disk.device;
        println!(
            "{}\t{}\t{} bytes\tsector={}\tbus={:?}\tremovable={}\tsystem={}",
            device.id,
            device.display_name,
            device.size_bytes,
            device.logical_sector_size,
            device.bus_type,
            device.is_removable,
            device.is_system_disk
        );
    }
    Ok(())
}

#[cfg(target_os = "windows")]
fn list_devices() -> Result<(), CliError> {
    let devices = data_recovery_device_windows::list_whole_disks().map_err(runtime)?;
    for disk in devices {
        let device = disk.device;
        println!(
            "{}\t{}\t{} bytes\tsector={}\tbus={:?}\tremovable={}\tsystem={}",
            device.id,
            device.display_name,
            device.size_bytes,
            device.logical_sector_size,
            device.bus_type,
            device.is_removable,
            device.is_system_disk
        );
    }
    Ok(())
}

#[cfg(all(not(target_os = "macos"), not(target_os = "windows")))]
fn list_devices() -> Result<(), CliError> {
    Err(CliError::runtime(
        "device enumeration is currently implemented only on macOS and Windows",
    ))
}

fn arg_value<'a>(args: &'a [String], name: &str) -> Result<&'a str, CliError> {
    args.windows(2)
        .find(|pair| pair[0] == name)
        .map(|pair| pair[1].as_str())
        .ok_or_else(|| CliError::usage(format!("missing required argument: {name}")))
}

fn optional_usize(args: &[String], name: &str) -> Result<Option<usize>, CliError> {
    let Some(value) = args
        .windows(2)
        .find(|pair| pair[0] == name)
        .map(|pair| pair[1].as_str())
    else {
        return Ok(None);
    };

    value
        .parse::<usize>()
        .map(Some)
        .map_err(|_| CliError::usage(format!("{name} must be an unsigned integer")))
}

fn output_format(args: &[String]) -> Result<OutputFormat, CliError> {
    let Some(value) = args
        .windows(2)
        .find(|pair| pair[0] == "--format")
        .map(|pair| pair[1].as_str())
    else {
        return Ok(OutputFormat::Text);
    };

    match value {
        "text" => Ok(OutputFormat::Text),
        "json" => Ok(OutputFormat::Json),
        _ => Err(CliError::usage("--format must be text or json")),
    }
}

fn json_escape(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for ch in value.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            value if value.is_control() => out.push_str(&format!("\\u{:04x}", value as u32)),
            value => out.push(value),
        }
    }
    out
}

fn parse_usize(value: &str, name: &str) -> Result<usize, CliError> {
    value
        .parse::<usize>()
        .map_err(|_| CliError::usage(format!("{name} must be an unsigned integer")))
}

fn optional_u64(args: &[String], name: &str) -> Result<Option<u64>, CliError> {
    let Some(value) = args
        .windows(2)
        .find(|pair| pair[0] == name)
        .map(|pair| pair[1].as_str())
    else {
        return Ok(None);
    };

    value
        .parse::<u64>()
        .map(Some)
        .map_err(|_| CliError::usage(format!("{name} must be an unsigned integer")))
}

fn optional_u16(args: &[String], name: &str) -> Result<Option<u16>, CliError> {
    let Some(value) = args
        .windows(2)
        .find(|pair| pair[0] == name)
        .map(|pair| pair[1].as_str())
    else {
        return Ok(None);
    };

    value
        .parse::<u16>()
        .map(Some)
        .map_err(|_| CliError::usage(format!("{name} must be an unsigned 16-bit integer")))
}

fn optional_str<'a>(args: &'a [String], name: &str) -> Option<&'a str> {
    args.windows(2)
        .find(|pair| pair[0] == name)
        .map(|pair| pair[1].as_str())
}

fn parse_u64(value: &str, name: &str) -> Result<u64, CliError> {
    value
        .parse::<u64>()
        .map_err(|_| CliError::usage(format!("{name} must be an unsigned integer")))
}

fn print_usage() {
    eprintln!(
        "Usage:
  polik-recovery wizard
  polik-recovery devices
  polik-recovery info --file <path>
  polik-recovery partitions --file <path> [--format text|json]
  polik-recovery hash --file <path> --offset <bytes> --length <bytes> [--chunk-size <bytes>]
  polik-recovery ntfs-info --file <image> [--offset <bytes>]
  polik-recovery ntfs-record-info --file <image> --offset <bytes> [--record-size <bytes>] [--sector-size <bytes>]
  polik-recovery ntfs-vfs --file <mft-records-image> [--record-size <bytes>] [--sector-size <bytes>] [--count <records>]
  polik-recovery ntfs-scan-vfs --file <volume-image> --count <records> [--offset <bytes>] [--format text|json]
  polik-recovery ntfs-extract --file <volume-image> --record <mft-index> --output <path> [--offset <bytes>]
  polik-recovery ntfs-make-plan --scan <scan.json> --output <plan.json> [--status deleted|allocated|all]
  polik-recovery ntfs-extract-batch --plan <plan.json> --output-dir <path> [--format text|json]
  polik-recovery ntfs-report --file <volume-image> --count <records> [--offset <bytes>] [--format text|json]
  polik-recovery exfat-info --file <image> [--offset <bytes>]
  polik-recovery exfat-scan-vfs --file <image> [--offset <bytes>] [--format text|json]
  polik-recovery exfat-extract --file <image> --entry-id <bytes-offset> --output <path> [--offset <bytes>]
  polik-recovery fat32-info --file <image> [--offset <bytes>]
  polik-recovery fat32-scan-vfs --file <image> [--offset <bytes>] [--format text|json]
  polik-recovery fat32-extract --file <image> --entry-id <bytes-offset> --output <path> [--offset <bytes>]
  polik-recovery apfs-info --file <image> [--offset <bytes>]
  polik-recovery apfs-scan-vfs --file <image> [--offset <bytes>] [--format text|json] [--include-deleted]
  polik-recovery apfs-extract --file <image> --inode <inode_id> --output <path> [--offset <bytes>]
  polik-recovery carve --file <path> --output-dir <path> [--format text|json]
  Raw-device commands are disabled in this preview. Use a controlled image-first workflow."
    );
}

fn read_line() -> String {
    let mut input = String::new();
    let _ = std::io::stdin().read_line(&mut input);
    input.trim().to_string()
}

fn ask_file_path(prompt: &str) -> String {
    use std::io::{self, Write};
    print!("{}", prompt);
    let _ = io::stdout().flush();
    read_line()
}

fn ask_u64_or_cancel(prompt: &str, default: u64) -> Option<u64> {
    use std::io::{self, Write};
    print!("{}", prompt);
    let _ = io::stdout().flush();
    let val = read_line();
    if val.is_empty() {
        Some(default)
    } else if val == "q" || val == "exit" {
        None
    } else {
        match val.parse::<u64>() {
            Ok(v) => Some(v),
            Err(_) => {
                println!("輸入無效，使用預設值: {default}");
                Some(default)
            }
        }
    }
}

fn ask_u64_required(prompt: &str) -> Option<u64> {
    use std::io::{self, Write};
    loop {
        print!("{}", prompt);
        let _ = io::stdout().flush();
        let val = read_line();
        if val == "q" || val == "exit" {
            return None;
        }
        match val.parse::<u64>() {
            Ok(v) => return Some(v),
            Err(_) => {
                println!("請輸入有效的正整數，或輸入 'q' 取消。");
            }
        }
    }
}

fn ask_usize_or_cancel(prompt: &str, default: usize) -> Option<usize> {
    use std::io::{self, Write};
    print!("{}", prompt);
    let _ = io::stdout().flush();
    let val = read_line();
    if val.is_empty() {
        Some(default)
    } else if val == "q" || val == "exit" {
        None
    } else {
        match val.parse::<usize>() {
            Ok(v) => Some(v),
            Err(_) => {
                println!("輸入無效，使用預設值: {default}");
                Some(default)
            }
        }
    }
}

fn ask_usize_required(prompt: &str) -> Option<usize> {
    use std::io::{self, Write};
    loop {
        print!("{}", prompt);
        let _ = io::stdout().flush();
        let val = read_line();
        if val == "q" || val == "exit" {
            return None;
        }
        match val.parse::<usize>() {
            Ok(v) => return Some(v),
            Err(_) => {
                println!("請輸入有效的正整數，或輸入 'q' 取消。");
            }
        }
    }
}

fn ask_format() -> OutputFormat {
    use std::io::{self, Write};
    loop {
        print!("請選擇輸出格式 (1. 文字 Text, 2. JSON, 預設 1): ");
        let _ = io::stdout().flush();
        let val = read_line();
        if val == "2" {
            return OutputFormat::Json;
        } else if val == "1" || val.is_empty() || val == "q" || val == "exit" {
            return OutputFormat::Text;
        }
        println!("無效選項，請輸入 1 或 2！");
    }
}

fn run_wizard() -> Result<(), CliError> {
    use std::io::{self, Write};

    println!("\n========================================================");
    println!("   📂 歡迎使用資料救援核心引擎互動式嚮導 (Interactive Wizard)   ");
    println!("========================================================");
    println!("提示：此嚮導將一步步引導您進行硬碟掃描與檔案復原。");
    println!("您可以隨時輸入 'q' 或 'exit' 返回或退出。");

    loop {
        println!("\n【 主選單 】");
        println!("  1. 掃描 NTFS 磁碟分區 (VFS 虛擬檔案系統 / 檔案復原)");
        println!("  2. 掃描 exFAT 磁碟分區 (VFS 虛擬檔案系統 / 檔案復原)");
        println!("  3. 檢視 APFS 容器與卷資訊 (驗證 Fletcher64 校驗碼)");
        println!("  4. 執行 Raw 檔案特徵彫刻 (File Carving - 適用損毀/未知檔案系統)");
        println!("  5. 列出本機所有實體/虛擬磁碟裝置 (devices)");
        println!("  6. 離開系統");

        print!("\n請輸入選項數字 (1-6): ");
        let _ = io::stdout().flush();
        let choice = read_line();

        if choice == "q" || choice == "exit" || choice == "6" {
            println!("\n感謝使用資料救援核心引擎，再見！");
            break;
        }

        match choice.as_str() {
            "1" => {
                println!("\n=== NTFS 磁碟分區掃描與復原 ===");
                let path = ask_file_path(
                    "請輸入 NTFS 磁碟映像檔或裝置路徑 (例如 fixtures/images/ntfs_volume_with_mft.bin): ",
                );
                if path.is_empty() || path == "q" || path == "exit" {
                    continue;
                }
                let offset = match ask_u64_or_cancel("請輸入 Volume 偏移量位元組 (預設 0): ", 0)
                {
                    Some(val) => val,
                    None => continue,
                };

                loop {
                    println!("\n--- NTFS 功能選擇 ---");
                    println!("  1. 檢視引導磁區與配置資訊 (ntfs-info)");
                    println!("  2. 掃描虛擬檔案樹與所有檔案 (ntfs-scan-vfs)");
                    println!("  3. 導出特定的 MFT 紀錄檔案 (ntfs-extract)");
                    println!("  4. 建立批次復原計畫檔 (ntfs-make-plan)");
                    println!("  5. 執行批次復原計畫 (ntfs-extract-batch)");
                    println!("  6. 返回主選單");
                    print!("請選擇 (1-6): ");
                    let _ = io::stdout().flush();
                    let sub = read_line();
                    if sub == "q" || sub == "exit" || sub == "6" {
                        break;
                    }
                    match sub.as_str() {
                        "1" => {
                            println!("\n[正在執行 ntfs-info...]");
                            if let Err(e) = ntfs_info(&path, offset) {
                                println!("❌ 執行錯誤: {e}");
                            }
                        }
                        "2" => {
                            let count = match ask_usize_or_cancel(
                                "請輸入掃描的最大 MFT 紀錄數量 (預設 100): ",
                                100,
                            ) {
                                Some(val) => val,
                                None => continue,
                            };
                            let format = ask_format();
                            println!("\n[正在執行 ntfs-scan-vfs...]");
                            if let Err(e) = ntfs_scan_vfs(&path, offset, count, format) {
                                println!("❌ 執行錯誤: {e}");
                            }
                        }
                        "3" => {
                            let record = match ask_usize_required(
                                "請輸入要提取的 MFT 紀錄索引號 (例如 0, 39): ",
                            ) {
                                Some(val) => val,
                                None => continue,
                            };
                            let out = ask_file_path("請輸入復原輸出的檔案路徑: ");
                            if out.is_empty() || out == "q" || out == "exit" {
                                continue;
                            }
                            println!("\n[正在執行 ntfs-extract...]");
                            if let Err(e) = ntfs_extract(&path, offset, record, &out, None, false) {
                                println!("❌ 執行錯誤: {e}");
                            }
                        }
                        "4" => {
                            let scan_json = ask_file_path("請輸入已產生的 scan.json 路徑: ");
                            if scan_json.is_empty() || scan_json == "q" || scan_json == "exit" {
                                continue;
                            }
                            let out_plan =
                                ask_file_path("請輸入欲輸出的計畫檔路徑 (例如 plan.json): ");
                            if out_plan.is_empty() || out_plan == "q" || out_plan == "exit" {
                                continue;
                            }
                            println!(
                                "請選擇要篩選的檔案狀態 (1. 已刪除 deleted, 2. 所有檔案 all, 3. 未刪除 allocated): "
                            );
                            let status = match read_line().as_str() {
                                "2" => "all",
                                "3" => "allocated",
                                _ => "deleted",
                            };
                            println!("\n[正在執行 ntfs-make-plan...]");
                            if let Err(e) = ntfs_make_plan(&scan_json, status, &out_plan) {
                                println!("❌ 執行錯誤: {e}");
                            }
                        }
                        "5" => {
                            let plan_json = ask_file_path("請輸入計畫檔 plan.json 路徑: ");
                            if plan_json.is_empty() || plan_json == "q" || plan_json == "exit" {
                                continue;
                            }
                            let out_dir =
                                ask_file_path("請輸入復原輸出目錄 (例如 /tmp/recovered): ");
                            if out_dir.is_empty() || out_dir == "q" || out_dir == "exit" {
                                continue;
                            }
                            let format = ask_format();
                            println!("\n[正在執行 ntfs-extract-batch...]");
                            if let Err(e) = ntfs_extract_batch(&plan_json, &out_dir, format, false)
                            {
                                println!("❌ 執行錯誤: {e}");
                            }
                        }
                        _ => {
                            println!("無效的選項！");
                        }
                    }
                }
            }
            "2" => {
                println!("\n=== exFAT 磁碟分區掃描與復原 ===");
                let path = ask_file_path("請輸入 exFAT 磁碟映像檔或裝置路徑: ");
                if path.is_empty() || path == "q" || path == "exit" {
                    continue;
                }
                let offset = match ask_u64_or_cancel("請輸入 Volume 偏移量位元組 (預設 0): ", 0)
                {
                    Some(val) => val,
                    None => continue,
                };

                loop {
                    println!("\n--- exFAT 功能選擇 ---");
                    println!("  1. 檢視引導磁區與配置資訊 (exfat-info)");
                    println!("  2. 掃描 VFS 檔案樹 (支援被刪除檔案) (exfat-scan-vfs)");
                    println!("  3. 依據 Entry ID 提取指定檔案 (exfat-extract)");
                    println!("  4. 返回主選單");
                    print!("請選擇 (1-4): ");
                    let _ = io::stdout().flush();
                    let sub = read_line();
                    if sub == "q" || sub == "exit" || sub == "4" {
                        break;
                    }
                    match sub.as_str() {
                        "1" => {
                            println!("\n[正在執行 exfat-info...]");
                            if let Err(e) = exfat_info(&path, offset) {
                                println!("❌ 執行錯誤: {e}");
                            }
                        }
                        "2" => {
                            let format = ask_format();
                            println!("\n[正在執行 exfat-scan-vfs...]");
                            if let Err(e) = exfat_scan_vfs(&path, offset, format) {
                                println!("❌ 執行錯誤: {e}");
                            }
                        }
                        "3" => {
                            let entry_id = match ask_u64_required(
                                "請輸入檔案項目的 Entry ID (位元組偏移量): ",
                            ) {
                                Some(val) => val,
                                None => continue,
                            };
                            let out = ask_file_path("請輸入復原輸出的檔案路徑: ");
                            if out.is_empty() || out == "q" || out == "exit" {
                                continue;
                            }
                            println!("\n[正在執行 exfat-extract...]");
                            if let Err(e) = exfat_extract(&path, offset, entry_id, &out, false) {
                                println!("❌ 執行錯誤: {e}");
                            }
                        }
                        _ => {
                            println!("無效的選項！");
                        }
                    }
                }
            }
            "3" => {
                println!("\n=== APFS 容器與卷資訊檢視 ===");
                let path = ask_file_path("請輸入 APFS 磁碟映像檔或裝置路徑: ");
                if path.is_empty() || path == "q" || path == "exit" {
                    continue;
                }
                let offset = match ask_u64_or_cancel("請輸入 Volume 偏移量位元組 (預設 0): ", 0)
                {
                    Some(val) => val,
                    None => continue,
                };

                loop {
                    println!("\n--- APFS 功能選擇 ---");
                    println!("  1. 檢視容器與卷資訊 (apfs-info)");
                    println!("  2. 掃描 VFS 檔案樹 (apfs-scan-vfs)");
                    println!("  3. 提取指定 Inode 檔案 (apfs-extract)");
                    println!("  4. 返回主選單");
                    print!("請選擇 (1-4): ");
                    let _ = io::stdout().flush();
                    let sub = read_line();
                    if sub == "q" || sub == "exit" || sub == "4" {
                        break;
                    }
                    match sub.as_str() {
                        "1" => {
                            println!("\n[正在執行 apfs-info...]");
                            if let Err(e) = apfs_info(&path, offset) {
                                println!("❌ 執行錯誤: {e}");
                            }
                        }
                        "2" => {
                            let format = ask_format();
                            println!("\n[正在執行 apfs-scan-vfs...]");
                            if let Err(e) = apfs_scan_vfs(&path, offset, format, false) {
                                println!("❌ 執行錯誤: {e}");
                            }
                        }
                        "3" => {
                            let inode_id = match ask_u64_or_cancel("請輸入要提取的 Inode ID: ", 0)
                            {
                                Some(val) => val,
                                None => continue,
                            };
                            let out = ask_file_path("請輸入輸出檔案路徑 (例如 ./recovered.png): ");
                            if out.is_empty() || out == "q" || out == "exit" {
                                continue;
                            }
                            println!("\n[正在執行 apfs-extract...]");
                            if let Err(e) = apfs_extract(&path, offset, inode_id, &out, false) {
                                println!("❌ 執行錯誤: {e}");
                            }
                        }
                        _ => {
                            println!("無效的選項！");
                        }
                    }
                }
            }
            "4" => {
                println!("\n=== Raw 檔案特徵彫刻 (File Carving) ===");
                let path = ask_file_path("請輸入要進行 File Carving 的磁碟映像檔路徑: ");
                if path.is_empty() || path == "q" || path == "exit" {
                    continue;
                }
                let out_dir = ask_file_path("請輸入保存復原檔案的輸出目錄 (如 ./carved_output): ");
                if out_dir.is_empty() || out_dir == "q" || out_dir == "exit" {
                    continue;
                }
                let format = ask_format();
                println!("\n[正在執行 carve...]");
                if let Err(e) = carve_command(&path, &out_dir, format, false) {
                    println!("❌ 執行錯誤: {e}");
                }
            }
            "5" => {
                println!("\n=== 本機所有實體/虛擬磁碟裝置 ===");
                if let Err(e) = list_devices() {
                    println!("❌ 執行錯誤: {e}");
                }
                println!("\n按 Enter 鍵返回主選單...");
                let _ = io::stdout().flush();
                let _ = read_line();
            }
            _ => {
                println!("無效的選項，請重新輸入！");
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::run;

    #[test]
    fn raw_device_commands_fail_before_argument_or_device_access() {
        for command in ["raw-info", "raw-hash"] {
            let error = run(vec![command.to_string()]).unwrap_err();
            assert!(error.message.contains("RAW_DEVICE_UNSUPPORTED"));
        }
    }
}
