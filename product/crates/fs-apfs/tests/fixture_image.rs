use data_recovery_core::FileBlockSource;

#[test]
fn v13_apfs_fixture_scans_and_extracts_complete_and_partial_entries() {
    let source = FileBlockSource::open(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../fixtures/apfs_quality/apfs_complete_partial.bin"
    ))
    .expect("run `make apfs-quality-fixture` first");
    let csb = data_recovery_fs_apfs::read_container_superblock(&source, 0).unwrap();
    assert_eq!(csb.block_size, 4096);
    assert_eq!(csb.fs_oids, vec![2]);

    let entries = data_recovery_fs_apfs::scan_apfs_vfs(&source, &csb, 0).unwrap();
    let complete = entries
        .iter()
        .find(|entry| entry.name == "COMPLETE.TXT")
        .unwrap();
    assert_eq!(complete.inode, 202);
    assert_eq!(complete.size, 34);

    let complete_report = data_recovery_fs_apfs::extract_apfs_file_data_with_report(
        &source,
        &csb,
        0,
        complete.inode,
        complete.size,
    )
    .unwrap();
    assert!(complete_report.complete);
    assert_eq!(complete_report.bytes_lost, 0);
    assert_eq!(complete_report.data, b"APFS complete extent fixture data\n");

    let partial = entries
        .iter()
        .find(|entry| entry.name == "PARTIAL.BIN")
        .unwrap();
    assert_eq!(partial.inode, 303);
    assert_eq!(partial.size, 38);

    let partial_report = data_recovery_fs_apfs::extract_apfs_file_data_with_report(
        &source,
        &csb,
        0,
        partial.inode,
        partial.size,
    )
    .unwrap();
    assert!(!partial_report.complete);
    assert_eq!(partial_report.bytes_requested, 38);
    assert_eq!(partial_report.bytes_read, 22);
    assert_eq!(partial_report.bytes_lost, 16);
    assert_eq!(partial_report.holes, vec![(22, 16)]);
    assert!(partial_report.data.starts_with(b"APFS partial head only"));
}

#[test]
fn v13_apfs_fixture_reports_crypto_metadata_as_suspect() {
    let source = FileBlockSource::open(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../fixtures/apfs_quality/apfs_encrypted_hint.bin"
    ))
    .expect("run `make apfs-quality-fixture` first");
    let csb = data_recovery_fs_apfs::read_container_superblock(&source, 0).unwrap();
    let entries = data_recovery_fs_apfs::scan_apfs_vfs(&source, &csb, 0).unwrap();
    let encrypted = entries
        .iter()
        .find(|entry| entry.name == "ENCRYPTED.TXT")
        .unwrap();

    let report = data_recovery_fs_apfs::extract_apfs_file_data_with_report(
        &source,
        &csb,
        0,
        encrypted.inode,
        encrypted.size,
    )
    .unwrap();
    assert!(!report.complete);
    assert!(report.encrypted_hint);
    assert_eq!(report.bytes_lost, 0);
    assert_eq!(report.data, b"APFS encrypted-hint fixture data\n");
    assert!(report.reason.contains("crypto"));
}
