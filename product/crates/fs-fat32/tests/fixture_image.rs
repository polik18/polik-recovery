use data_recovery_core::FileBlockSource;

#[test]
fn v07_fat32_fixture_scans_allocated_and_deleted_entries() {
    let source = FileBlockSource::open(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../fixtures/fs_quality/fat32_image.bin"
    ))
    .expect("run `make fs-quality-fixture` first");
    let boot = data_recovery_fs_fat32::read_boot_sector(&source, 0).unwrap();
    assert_eq!(boot.bytes_per_sector, 512);
    assert_eq!(boot.root_cluster, 2);

    let entries = data_recovery_fs_fat32::scan_fat32_vfs(&source, &boot, 0).unwrap();
    let active = entries
        .iter()
        .find(|entry| entry.name == "ACTIVE.TXT")
        .unwrap();
    assert!(!active.is_deleted);
    assert_eq!(active.size, 29);
    let active_data = data_recovery_fs_fat32::read_entry_data(
        &source,
        &boot,
        0,
        active.first_cluster,
        active.size,
        active.is_deleted,
    )
    .unwrap();
    assert_eq!(active_data, b"FAT32 allocated fixture data\n");

    let deleted = entries
        .iter()
        .find(|entry| entry.name == "_ELETE.TXT")
        .unwrap();
    assert!(deleted.is_deleted);
    assert_eq!(deleted.size, 47);
    let deleted_data = data_recovery_fs_fat32::read_entry_data(
        &source,
        &boot,
        0,
        deleted.first_cluster,
        deleted.size,
        deleted.is_deleted,
    )
    .unwrap();
    assert_eq!(
        deleted_data,
        b"FAT32 freshly deleted contiguous fallback data\n"
    );
}
