use data_recovery_core::FileBlockSource;

#[test]
fn v08_fat32_fragmented_fixture_uses_fat_chain_before_fallback() {
    let source = FileBlockSource::open(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../fixtures/fragmented_quality/fat32_fragmented.bin"
    ))
    .expect("run `make fragmented-quality-fixture` first");
    let boot = data_recovery_fs_fat32::read_boot_sector(&source, 0).unwrap();
    let entries = data_recovery_fs_fat32::scan_fat32_vfs(&source, &boot, 0).unwrap();

    let fragmented = entries
        .iter()
        .find(|entry| entry.name == "FRAG.TXT")
        .unwrap();
    let fragmented_report = data_recovery_fs_fat32::read_entry_data_with_report(
        &source,
        &boot,
        0,
        fragmented.first_cluster,
        fragmented.size,
        fragmented.is_deleted,
    )
    .unwrap();
    assert!(fragmented_report.complete);
    assert_eq!(
        fragmented_report.mode,
        data_recovery_fs_fat32::Fat32EntryReadMode::FatChain
    );
    assert_eq!(fragmented_report.bytes_read, fragmented.size);

    let deleted_chain = entries
        .iter()
        .find(|entry| entry.name == "_ELCHN.TXT")
        .unwrap();
    assert!(deleted_chain.is_deleted);
    let deleted_chain_report = data_recovery_fs_fat32::read_entry_data_with_report(
        &source,
        &boot,
        0,
        deleted_chain.first_cluster,
        deleted_chain.size,
        deleted_chain.is_deleted,
    )
    .unwrap();
    assert!(deleted_chain_report.complete);
    assert_eq!(
        deleted_chain_report.mode,
        data_recovery_fs_fat32::Fat32EntryReadMode::FatChain
    );

    let missing_chain = entries
        .iter()
        .find(|entry| entry.name == "_ELMIS.TXT")
        .unwrap();
    assert!(missing_chain.is_deleted);
    let missing_chain_report = data_recovery_fs_fat32::read_entry_data_with_report(
        &source,
        &boot,
        0,
        missing_chain.first_cluster,
        missing_chain.size,
        missing_chain.is_deleted,
    )
    .unwrap();
    assert!(!missing_chain_report.complete);
    assert_eq!(
        missing_chain_report.mode,
        data_recovery_fs_fat32::Fat32EntryReadMode::ContiguousFallback
    );
}
