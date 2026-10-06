use data_recovery_core::FileBlockSource;

#[test]
fn v07_exfat_fixture_scans_allocated_and_deleted_entries() {
    let source = FileBlockSource::open(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../fixtures/fs_quality/exfat_image.bin"
    ))
    .expect("run `make fs-quality-fixture` first");
    let boot = data_recovery_fs_exfat::read_boot_sector(&source, 0).unwrap();
    assert_eq!(boot.sector_size(), 512);
    assert_eq!(boot.first_cluster_of_root_directory, 2);

    let entries = data_recovery_fs_exfat::scan_exfat_vfs(&source, &boot, 0).unwrap();
    let active = entries
        .iter()
        .find(|entry| entry.name == "ACTIVE.TXT")
        .unwrap();
    assert!(!active.is_deleted);
    assert!(active.no_fat_chain);
    assert_eq!(active.size, 40);
    let active_data = data_recovery_fs_exfat::read_entry_data(
        &source,
        &boot,
        0,
        active.first_cluster,
        active.size,
        active.no_fat_chain,
    )
    .unwrap();
    assert_eq!(active_data, b"exFAT allocated contiguous fixture data\n");

    let deleted = entries
        .iter()
        .find(|entry| entry.name == "DELETE.TXT")
        .unwrap();
    assert!(deleted.is_deleted);
    assert!(deleted.no_fat_chain);
    assert_eq!(deleted.size, 41);
    let deleted_data = data_recovery_fs_exfat::read_entry_data(
        &source,
        &boot,
        0,
        deleted.first_cluster,
        deleted.size,
        deleted.no_fat_chain,
    )
    .unwrap();
    assert_eq!(deleted_data, b"exFAT deleted no-FAT-chain fallback data\n");
}
