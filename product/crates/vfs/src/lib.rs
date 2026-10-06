use data_recovery_fs_ntfs::{FileNameAttribute, FileNameNamespace, MftRecord};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct VfsNodeId(pub u64);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VfsEntry {
    pub id: VfsNodeId,
    pub parent_id: Option<VfsNodeId>,
    pub name: String,
    pub kind: VfsEntryKind,
    pub status: VfsEntryStatus,
    pub size_bytes: Option<u64>,
    pub source: VfsSource,
    pub timestamps: VfsTimestamps,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VfsEntryKind {
    File,
    Directory,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VfsEntryStatus {
    Allocated,
    Deleted,
    Orphan,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VfsSource {
    Ntfs {
        record_number: u64,
        sequence_number: u16,
    },
    ExFat {
        file_offset: u64,
        first_cluster: u32,
    },
    Fat32 {
        file_offset: u64,
        first_cluster: u32,
    },
    Apfs {
        inode: u64,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct VfsTimestamps {
    pub created_time: Option<u64>,
    pub modified_time: Option<u64>,
    pub mft_modified_time: Option<u64>,
    pub accessed_time: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VfsTree {
    pub entries: BTreeMap<VfsNodeId, VfsEntry>,
    children: BTreeMap<VfsNodeId, Vec<VfsNodeId>>,
    roots: Vec<VfsNodeId>,
    orphans: Vec<VfsNodeId>,
}

impl VfsTree {
    pub fn build(entries: Vec<VfsEntry>) -> Self {
        let mut entry_map = BTreeMap::new();
        for entry in entries {
            entry_map.insert(entry.id, entry);
        }

        let known_ids = entry_map.keys().copied().collect::<BTreeSet<_>>();
        let mut children: BTreeMap<VfsNodeId, Vec<VfsNodeId>> = BTreeMap::new();
        let mut roots = Vec::new();
        let mut orphans = Vec::new();

        for (id, entry) in &entry_map {
            match entry.parent_id {
                Some(parent_id) if known_ids.contains(&parent_id) && parent_id != *id => {
                    children.entry(parent_id).or_default().push(*id);
                }
                Some(_) => orphans.push(*id),
                None => roots.push(*id),
            }
        }

        for ids in children.values_mut() {
            ids.sort_by(|left, right| {
                let left_entry = &entry_map[left];
                let right_entry = &entry_map[right];
                left_entry
                    .name
                    .to_ascii_lowercase()
                    .cmp(&right_entry.name.to_ascii_lowercase())
                    .then_with(|| left.cmp(right))
            });
        }
        roots.sort();
        orphans.sort();

        Self {
            entries: entry_map,
            children,
            roots,
            orphans,
        }
    }

    pub fn children_of(&self, id: VfsNodeId) -> &[VfsNodeId] {
        self.children.get(&id).map(Vec::as_slice).unwrap_or(&[])
    }

    pub fn roots(&self) -> &[VfsNodeId] {
        &self.roots
    }

    pub fn orphans(&self) -> &[VfsNodeId] {
        &self.orphans
    }

    pub fn entry(&self, id: VfsNodeId) -> Option<&VfsEntry> {
        self.entries.get(&id)
    }

    pub fn path_for(&self, id: VfsNodeId) -> Option<String> {
        let mut parts = Vec::new();
        let mut current = self.entries.get(&id)?;
        let mut seen = BTreeSet::new();

        loop {
            if !seen.insert(current.id) {
                return None;
            }
            parts.push(current.name.clone());

            let Some(parent_id) = current.parent_id else {
                break;
            };
            let Some(parent) = self.entries.get(&parent_id) else {
                parts.push("$ORPHANS".to_string());
                break;
            };
            current = parent;
        }

        parts.reverse();
        Some(format!("/{}", parts.join("/")))
    }

    pub fn flattened(&self) -> Vec<(VfsNodeId, String)> {
        let mut out = Vec::new();
        for root in &self.roots {
            self.walk(*root, &mut out);
        }
        for orphan in &self.orphans {
            if let Some(path) = self.path_for(*orphan) {
                out.push((*orphan, path));
            }
        }
        out
    }

    fn walk(&self, id: VfsNodeId, out: &mut Vec<(VfsNodeId, String)>) {
        if let Some(path) = self.path_for(id) {
            out.push((id, path));
        }
        for child in self.children_of(id) {
            self.walk(*child, out);
        }
    }
}

pub fn entries_from_ntfs_records(records: &[MftRecord]) -> Vec<VfsEntry> {
    let mut entries = Vec::new();
    for record in records {
        if let Some(main_entry) = entry_from_ntfs_record(record) {
            let main_id = main_entry.id.0;
            let parent_id = main_entry.parent_id;
            let name = main_entry.name.clone();
            let status = main_entry.status;
            let sequence_number = record.header.sequence_number;
            entries.push(main_entry);

            let streams = data_recovery_fs_ntfs::list_data_streams(record);
            let mut ads_index = 1;
            for stream in streams {
                if let Some(stream_name) = stream.name {
                    let ads_id = main_id | ((ads_index as u64) << 48);
                    ads_index += 1;
                    entries.push(VfsEntry {
                        id: VfsNodeId(ads_id),
                        parent_id,
                        name: format!("{}:{}", name, stream_name),
                        kind: VfsEntryKind::File,
                        status,
                        size_bytes: stream.size,
                        source: VfsSource::Ntfs {
                            record_number: main_id,
                            sequence_number,
                        },
                        timestamps: VfsTimestamps::default(),
                    });
                }
            }
        }
    }
    entries
}

pub fn tree_from_ntfs_records(records: &[MftRecord]) -> VfsTree {
    VfsTree::build(entries_from_ntfs_records(records))
}

pub fn entries_from_exfat_entries(entries: &[data_recovery_fs_exfat::ExFatEntry]) -> Vec<VfsEntry> {
    entries.iter().map(entry_from_exfat_entry).collect()
}

pub fn tree_from_exfat_entries(entries: &[data_recovery_fs_exfat::ExFatEntry]) -> VfsTree {
    VfsTree::build(entries_from_exfat_entries(entries))
}

pub fn entries_from_fat32_entries(entries: &[data_recovery_fs_fat32::Fat32Entry]) -> Vec<VfsEntry> {
    entries.iter().map(entry_from_fat32_entry).collect()
}

pub fn tree_from_fat32_entries(entries: &[data_recovery_fs_fat32::Fat32Entry]) -> VfsTree {
    VfsTree::build(entries_from_fat32_entries(entries))
}

pub fn entries_from_apfs_entries(entries: &[data_recovery_fs_apfs::ApfsEntry]) -> Vec<VfsEntry> {
    entries.iter().map(entry_from_apfs_entry).collect()
}

pub fn tree_from_apfs_entries(entries: &[data_recovery_fs_apfs::ApfsEntry]) -> VfsTree {
    VfsTree::build(entries_from_apfs_entries(entries))
}

fn entry_from_apfs_entry(entry: &data_recovery_fs_apfs::ApfsEntry) -> VfsEntry {
    let status = if entry.is_deleted {
        VfsEntryStatus::Deleted
    } else {
        VfsEntryStatus::Allocated
    };

    let kind = if entry.is_directory {
        VfsEntryKind::Directory
    } else {
        VfsEntryKind::File
    };

    VfsEntry {
        id: VfsNodeId(entry.inode),
        parent_id: if entry.parent_inode != 0
            && entry.parent_inode != entry.inode
            && entry.parent_inode != 1
        {
            Some(VfsNodeId(entry.parent_inode))
        } else {
            None
        },
        name: entry.name.clone(),
        kind,
        status,
        size_bytes: (!entry.is_directory).then_some(entry.size),
        source: VfsSource::Apfs { inode: entry.inode },
        timestamps: VfsTimestamps::default(),
    }
}

fn entry_from_exfat_entry(entry: &data_recovery_fs_exfat::ExFatEntry) -> VfsEntry {
    let status = if entry.is_deleted {
        VfsEntryStatus::Deleted
    } else {
        VfsEntryStatus::Allocated
    };

    let kind = if entry.is_directory {
        VfsEntryKind::Directory
    } else {
        VfsEntryKind::File
    };

    VfsEntry {
        id: VfsNodeId(entry.file_offset),
        parent_id: entry.parent_offset.map(VfsNodeId),
        name: entry.name.clone(),
        kind,
        status,
        size_bytes: (!entry.is_directory).then_some(entry.size),
        source: VfsSource::ExFat {
            file_offset: entry.file_offset,
            first_cluster: entry.first_cluster,
        },
        timestamps: VfsTimestamps {
            created_time: entry.created_time,
            modified_time: entry.modified_time,
            accessed_time: entry.accessed_time,
            mft_modified_time: None,
        },
    }
}

fn entry_from_fat32_entry(entry: &data_recovery_fs_fat32::Fat32Entry) -> VfsEntry {
    let status = if entry.is_deleted {
        VfsEntryStatus::Deleted
    } else {
        VfsEntryStatus::Allocated
    };

    let kind = if entry.is_directory {
        VfsEntryKind::Directory
    } else {
        VfsEntryKind::File
    };

    VfsEntry {
        id: VfsNodeId(entry.file_offset),
        parent_id: entry.parent_offset.map(VfsNodeId),
        name: entry.name.clone(),
        kind,
        status,
        size_bytes: (!entry.is_directory).then_some(entry.size),
        source: VfsSource::Fat32 {
            file_offset: entry.file_offset,
            first_cluster: entry.first_cluster,
        },
        timestamps: VfsTimestamps {
            created_time: entry.created_time,
            modified_time: entry.modified_time,
            accessed_time: entry.accessed_time,
            mft_modified_time: None,
        },
    }
}

fn entry_from_ntfs_record(record: &MftRecord) -> Option<VfsEntry> {
    let record_number = record.header.record_number? as u64;
    let file_name = choose_file_name(record)?;
    let standard = record
        .attributes
        .iter()
        .find_map(|attribute| attribute.standard_information());

    let parent_id = (file_name.parent_record_number != record_number)
        .then_some(VfsNodeId(file_name.parent_record_number));
    let status = if record.header.is_deleted() {
        VfsEntryStatus::Deleted
    } else {
        VfsEntryStatus::Allocated
    };
    let kind = if record.header.is_directory() {
        VfsEntryKind::Directory
    } else {
        VfsEntryKind::File
    };

    Some(VfsEntry {
        id: VfsNodeId(record_number),
        parent_id,
        name: file_name.name.clone(),
        kind,
        status,
        size_bytes: (!record.header.is_directory()).then_some(file_name.real_size),
        source: VfsSource::Ntfs {
            record_number,
            sequence_number: record.header.sequence_number,
        },
        timestamps: VfsTimestamps {
            created_time: standard
                .map(|value| value.created_time)
                .or(Some(file_name.created_time)),
            modified_time: standard
                .map(|value| value.modified_time)
                .or(Some(file_name.modified_time)),
            mft_modified_time: standard
                .map(|value| value.mft_modified_time)
                .or(Some(file_name.mft_modified_time)),
            accessed_time: standard
                .map(|value| value.accessed_time)
                .or(Some(file_name.accessed_time)),
        },
    })
}

fn choose_file_name(record: &MftRecord) -> Option<&FileNameAttribute> {
    let mut candidates = record
        .attributes
        .iter()
        .filter_map(|attribute| attribute.file_name())
        .collect::<Vec<_>>();

    candidates.sort_by_key(|file_name| match file_name.namespace {
        FileNameNamespace::Win32 | FileNameNamespace::Win32AndDos => 0,
        FileNameNamespace::Posix => 1,
        FileNameNamespace::Dos => 2,
        FileNameNamespace::Unknown(_) => 3,
    });
    candidates.into_iter().next()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(
        id: u64,
        parent: Option<u64>,
        name: &str,
        kind: VfsEntryKind,
        status: VfsEntryStatus,
    ) -> VfsEntry {
        VfsEntry {
            id: VfsNodeId(id),
            parent_id: parent.map(VfsNodeId),
            name: name.to_string(),
            kind,
            status,
            size_bytes: None,
            source: VfsSource::Ntfs {
                record_number: id,
                sequence_number: 1,
            },
            timestamps: VfsTimestamps::default(),
        }
    }

    #[test]
    fn builds_paths_for_parent_child_entries() {
        let tree = VfsTree::build(vec![
            entry(
                5,
                None,
                "root",
                VfsEntryKind::Directory,
                VfsEntryStatus::Allocated,
            ),
            entry(
                10,
                Some(5),
                "report.txt",
                VfsEntryKind::File,
                VfsEntryStatus::Deleted,
            ),
        ]);

        assert_eq!(tree.path_for(VfsNodeId(5)).unwrap(), "/root");
        assert_eq!(tree.path_for(VfsNodeId(10)).unwrap(), "/root/report.txt");
        assert_eq!(tree.children_of(VfsNodeId(5)), &[VfsNodeId(10)]);
    }

    #[test]
    fn routes_missing_parent_to_orphans() {
        let tree = VfsTree::build(vec![entry(
            99,
            Some(123),
            "lost.bin",
            VfsEntryKind::File,
            VfsEntryStatus::Deleted,
        )]);

        assert_eq!(tree.orphans(), &[VfsNodeId(99)]);
        assert_eq!(tree.path_for(VfsNodeId(99)).unwrap(), "/$ORPHANS/lost.bin");
    }

    #[test]
    fn test_tree_from_apfs_entries() {
        let apfs_entries = vec![
            data_recovery_fs_apfs::ApfsEntry {
                is_deleted: false,
                is_directory: true,
                name: "Users".to_string(),
                size: 0,
                inode: 100,
                parent_inode: 1,
            },
            data_recovery_fs_apfs::ApfsEntry {
                is_deleted: false,
                is_directory: false,
                name: "notes.txt".to_string(),
                size: 1500,
                inode: 101,
                parent_inode: 100,
            },
        ];

        let tree = tree_from_apfs_entries(&apfs_entries);
        assert_eq!(tree.path_for(VfsNodeId(100)).unwrap(), "/Users");
        assert_eq!(tree.path_for(VfsNodeId(101)).unwrap(), "/Users/notes.txt");
        assert_eq!(tree.entry(VfsNodeId(101)).unwrap().size_bytes, Some(1500));
        assert!(matches!(
            tree.entry(VfsNodeId(101)).unwrap().source,
            VfsSource::Apfs { inode: 101 }
        ));
    }
}
