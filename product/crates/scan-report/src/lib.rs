use data_recovery_fs_ntfs::{
    AttributeForm, AttributeType, AttributeValue, MftRecord, NtfsBootSector,
};
use data_recovery_vfs::{VfsEntryKind, VfsEntryStatus, VfsTree, tree_from_ntfs_records};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NtfsScanReport {
    pub volume: NtfsVolumeSummary,
    pub records_scanned: usize,
    pub vfs: VfsSummary,
    pub extractable_files: usize,
    pub data_run_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NtfsVolumeSummary {
    pub bytes_per_sector: u16,
    pub sectors_per_cluster: u8,
    pub cluster_size: u64,
    pub total_sectors: u64,
    pub total_size_bytes: u64,
    pub mft_lcn: u64,
    pub mft_offset: u64,
    pub file_record_size: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct VfsSummary {
    pub entries: usize,
    pub files: usize,
    pub directories: usize,
    pub allocated: usize,
    pub deleted: usize,
    pub orphaned: usize,
}

impl NtfsScanReport {
    pub fn from_records(
        boot: &NtfsBootSector,
        records: &[MftRecord],
    ) -> Result<Self, data_recovery_fs_ntfs::NtfsError> {
        let tree = tree_from_ntfs_records(records);
        let vfs = summarize_vfs(&tree);
        let extractable_files = records
            .iter()
            .filter(|record| record_has_non_resident_data(record))
            .count();
        let data_run_count = records.iter().map(record_data_run_count).sum();

        Ok(Self {
            volume: NtfsVolumeSummary {
                bytes_per_sector: boot.bytes_per_sector,
                sectors_per_cluster: boot.sectors_per_cluster,
                cluster_size: boot.cluster_size(),
                total_sectors: boot.total_sectors,
                total_size_bytes: boot.total_size_bytes()?,
                mft_lcn: boot.mft_lcn,
                mft_offset: boot.mft_offset()?,
                file_record_size: boot.file_record_size()?,
            },
            records_scanned: records.len(),
            vfs,
            extractable_files,
            data_run_count,
        })
    }

    pub fn to_text(&self) -> String {
        format!(
            "\
NTFS Scan Report
bytes_per_sector: {}
sectors_per_cluster: {}
cluster_size: {}
total_sectors: {}
total_size_bytes: {}
mft_lcn: {}
mft_offset: {}
file_record_size: {}
records_scanned: {}
vfs_entries: {}
files: {}
directories: {}
allocated: {}
deleted: {}
orphaned: {}
extractable_files: {}
data_run_count: {}
",
            self.volume.bytes_per_sector,
            self.volume.sectors_per_cluster,
            self.volume.cluster_size,
            self.volume.total_sectors,
            self.volume.total_size_bytes,
            self.volume.mft_lcn,
            self.volume.mft_offset,
            self.volume.file_record_size,
            self.records_scanned,
            self.vfs.entries,
            self.vfs.files,
            self.vfs.directories,
            self.vfs.allocated,
            self.vfs.deleted,
            self.vfs.orphaned,
            self.extractable_files,
            self.data_run_count,
        )
    }

    pub fn to_json(&self) -> String {
        format!(
            concat!(
                "{{",
                "\"volume\":{{",
                "\"bytes_per_sector\":{},",
                "\"sectors_per_cluster\":{},",
                "\"cluster_size\":{},",
                "\"total_sectors\":{},",
                "\"total_size_bytes\":{},",
                "\"mft_lcn\":{},",
                "\"mft_offset\":{},",
                "\"file_record_size\":{}",
                "}},",
                "\"records_scanned\":{},",
                "\"vfs\":{{",
                "\"entries\":{},",
                "\"files\":{},",
                "\"directories\":{},",
                "\"allocated\":{},",
                "\"deleted\":{},",
                "\"orphaned\":{}",
                "}},",
                "\"extractable_files\":{},",
                "\"data_run_count\":{}",
                "}}\n"
            ),
            self.volume.bytes_per_sector,
            self.volume.sectors_per_cluster,
            self.volume.cluster_size,
            self.volume.total_sectors,
            self.volume.total_size_bytes,
            self.volume.mft_lcn,
            self.volume.mft_offset,
            self.volume.file_record_size,
            self.records_scanned,
            self.vfs.entries,
            self.vfs.files,
            self.vfs.directories,
            self.vfs.allocated,
            self.vfs.deleted,
            self.vfs.orphaned,
            self.extractable_files,
            self.data_run_count,
        )
    }
}

fn summarize_vfs(tree: &VfsTree) -> VfsSummary {
    let mut summary = VfsSummary {
        entries: tree.entries.len(),
        orphaned: tree.orphans().len(),
        ..VfsSummary::default()
    };

    for entry in tree.entries.values() {
        match entry.kind {
            VfsEntryKind::File => summary.files += 1,
            VfsEntryKind::Directory => summary.directories += 1,
            VfsEntryKind::Unknown => {}
        }

        match entry.status {
            VfsEntryStatus::Allocated => summary.allocated += 1,
            VfsEntryStatus::Deleted => summary.deleted += 1,
            VfsEntryStatus::Orphan => summary.orphaned += 1,
        }
    }

    summary
}

fn record_has_non_resident_data(record: &MftRecord) -> bool {
    record.attributes.iter().any(|attribute| {
        attribute.header.attribute_type == AttributeType::Data
            && matches!(attribute.header.form, AttributeForm::NonResident(_))
            && matches!(attribute.value, AttributeValue::DataRuns(_))
    })
}

fn record_data_run_count(record: &MftRecord) -> usize {
    record
        .attributes
        .iter()
        .filter_map(|attribute| match &attribute.value {
            AttributeValue::DataRuns(runs) => Some(runs.len()),
            _ => None,
        })
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;
    use data_recovery_core::FileBlockSource;
    use data_recovery_fs_ntfs::{NtfsBootSector, read_mft_record};
    use std::io::Write;
    use std::sync::atomic::{AtomicU64, Ordering};

    fn temp_file_name() -> std::path::PathBuf {
        static NEXT_ID: AtomicU64 = AtomicU64::new(0);
        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!("dr_report_{}_{id}.bin", std::process::id()))
    }

    fn boot() -> NtfsBootSector {
        let mut sector = [0_u8; 512];
        sector[0..3].copy_from_slice(&[0xeb, 0x52, 0x90]);
        sector[3..11].copy_from_slice(b"NTFS    ");
        sector[11..13].copy_from_slice(&512_u16.to_le_bytes());
        sector[13] = 8;
        sector[40..48].copy_from_slice(&512_u64.to_le_bytes());
        sector[48..56].copy_from_slice(&1_u64.to_le_bytes());
        sector[56..64].copy_from_slice(&2_u64.to_le_bytes());
        sector[64] = -10_i8 as u8;
        sector[68] = 1;
        sector[510..512].copy_from_slice(&0xaa55_u16.to_le_bytes());
        NtfsBootSector::parse(&sector).unwrap()
    }

    #[test]
    fn summarizes_records() {
        let fixture = include_bytes!("../../../fixtures/images/ntfs_mft_records_vfs.bin");
        let path = temp_file_name();
        {
            let mut file = std::fs::File::create(&path).unwrap();
            file.write_all(fixture).unwrap();
        }
        let source = FileBlockSource::open(&path).unwrap();
        let records = vec![
            read_mft_record(&source, 0, 1024, 512).unwrap(),
            read_mft_record(&source, 1024, 1024, 512).unwrap(),
        ];

        let report = NtfsScanReport::from_records(&boot(), &records).unwrap();

        assert_eq!(report.records_scanned, 2);
        assert_eq!(report.vfs.files, 1);
        assert_eq!(report.vfs.directories, 1);
        assert_eq!(report.vfs.deleted, 1);
        assert!(report.to_json().contains("\"records_scanned\":2"));

        std::fs::remove_file(path).unwrap();
    }
}
