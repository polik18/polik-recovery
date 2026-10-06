use data_recovery_core::FileBlockSource;

#[test]
fn v08_exfat_fragmented_fixture_reports_complete_and_partial_cases() {
    let source = FileBlockSource::open(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../fixtures/fragmented_quality/exfat_fragmented.bin"
    ))
    .expect("run `make fragmented-quality-fixture` first");
    let boot = data_recovery_fs_exfat::read_boot_sector(&source, 0).unwrap();
    let entries = data_recovery_fs_exfat::scan_exfat_vfs(&source, &boot, 0).unwrap();

    let fragmented = entries
        .iter()
        .find(|entry| entry.name == "FRAG.TXT")
        .unwrap();
    assert!(!fragmented.no_fat_chain);
    let fragmented_report = data_recovery_fs_exfat::read_entry_data_with_report(
        &source,
        &boot,
        0,
        fragmented.first_cluster,
        fragmented.size,
        fragmented.no_fat_chain,
    )
    .unwrap();
    assert!(fragmented_report.complete);
    assert_eq!(
        fragmented_report.mode,
        data_recovery_fs_exfat::ExFatEntryReadMode::FatChain
    );

    let short = entries
        .iter()
        .find(|entry| entry.name == "SHORT.TXT")
        .unwrap();
    assert!(!short.no_fat_chain);
    let short_report = data_recovery_fs_exfat::read_entry_data_with_report(
        &source,
        &boot,
        0,
        short.first_cluster,
        short.size,
        short.no_fat_chain,
    )
    .unwrap();
    assert!(!short_report.complete);
    assert_eq!(
        short_report.mode,
        data_recovery_fs_exfat::ExFatEntryReadMode::FatChain
    );
    assert!(short_report.bytes_lost > 0);
}
