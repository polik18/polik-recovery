use data_recovery_core::{BlockSource, ReadError};
use std::collections::VecDeque;

#[derive(Debug)]
pub enum ExFatError {
    Read(ReadError),
    InvalidOemId([u8; 8]),
    InvalidBootSignature(u16),
    InvalidBytesPerSectorShift(u8),
    InvalidSectorsPerClusterShift(u8),
    InvalidCluster(u32),
    UnexpectedEof,
    OffsetOverflow,
}

impl std::fmt::Display for ExFatError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Read(e) => write!(f, "Read error: {e}"),
            Self::InvalidOemId(id) => write!(
                f,
                "Invalid exFAT OEM ID: {:?}",
                std::str::from_utf8(id).unwrap_or("")
            ),
            Self::InvalidBootSignature(sig) => write!(f, "Invalid boot signature: {sig:#06x}"),
            Self::InvalidBytesPerSectorShift(val) => {
                write!(f, "Invalid bytes per sector shift: {val}")
            }
            Self::InvalidSectorsPerClusterShift(val) => {
                write!(f, "Invalid sectors per cluster shift: {val}")
            }
            Self::InvalidCluster(val) => write!(f, "Invalid cluster: {val}"),
            Self::UnexpectedEof => write!(f, "Unexpected end of file/stream"),
            Self::OffsetOverflow => write!(f, "Computed offset overflowed u64"),
        }
    }
}

impl std::error::Error for ExFatError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Read(err) => Some(err),
            _ => None,
        }
    }
}

impl From<ReadError> for ExFatError {
    fn from(value: ReadError) -> Self {
        Self::Read(value)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExFatBootSector {
    pub oem_name: [u8; 8],
    pub partition_offset: u64,
    pub volume_length: u64,
    pub fat_offset: u32,
    pub fat_length: u32,
    pub cluster_heap_offset: u32,
    pub cluster_count: u32,
    pub first_cluster_of_root_directory: u32,
    pub volume_serial_number: u32,
    pub file_system_revision: u16,
    pub volume_flags: u16,
    pub bytes_per_sector_shift: u8,
    pub sectors_per_cluster_shift: u8,
    pub number_of_fats: u8,
}

impl ExFatBootSector {
    pub fn parse(bytes: &[u8; 512]) -> Result<Self, ExFatError> {
        let oem_name: [u8; 8] = bytes[3..11].try_into().unwrap();
        if &oem_name != b"EXFAT   " {
            return Err(ExFatError::InvalidOemId(oem_name));
        }

        let boot_sig = u16::from_le_bytes([bytes[510], bytes[511]]);
        if boot_sig != 0xAA55 {
            return Err(ExFatError::InvalidBootSignature(boot_sig));
        }

        let bytes_per_sector_shift = bytes[108];
        if !(9..=12).contains(&bytes_per_sector_shift) {
            return Err(ExFatError::InvalidBytesPerSectorShift(
                bytes_per_sector_shift,
            ));
        }

        let sectors_per_cluster_shift = bytes[109];
        if bytes_per_sector_shift + sectors_per_cluster_shift > 25 {
            return Err(ExFatError::InvalidSectorsPerClusterShift(
                sectors_per_cluster_shift,
            ));
        }

        Ok(Self {
            oem_name,
            partition_offset: u64::from_le_bytes(bytes[64..72].try_into().unwrap()),
            volume_length: u64::from_le_bytes(bytes[72..80].try_into().unwrap()),
            fat_offset: u32::from_le_bytes(bytes[80..84].try_into().unwrap()),
            fat_length: u32::from_le_bytes(bytes[84..88].try_into().unwrap()),
            cluster_heap_offset: u32::from_le_bytes(bytes[88..92].try_into().unwrap()),
            cluster_count: u32::from_le_bytes(bytes[92..96].try_into().unwrap()),
            first_cluster_of_root_directory: u32::from_le_bytes(bytes[96..100].try_into().unwrap()),
            volume_serial_number: u32::from_le_bytes(bytes[100..104].try_into().unwrap()),
            file_system_revision: u16::from_le_bytes(bytes[104..106].try_into().unwrap()),
            volume_flags: u16::from_le_bytes(bytes[106..108].try_into().unwrap()),
            bytes_per_sector_shift,
            sectors_per_cluster_shift,
            number_of_fats: bytes[110],
        })
    }

    pub fn sector_size(&self) -> u64 {
        1 << self.bytes_per_sector_shift as u64
    }

    pub fn cluster_size(&self) -> u64 {
        1 << (self.bytes_per_sector_shift as u64 + self.sectors_per_cluster_shift as u64)
    }

    pub fn cluster_to_byte_offset(&self, cluster: u32) -> Option<u64> {
        if cluster < 2 || cluster >= 2 + self.cluster_count {
            return None;
        }
        let sector_offset = self.cluster_heap_offset as u64
            + (cluster as u64 - 2) * (1 << self.sectors_per_cluster_shift as u64);
        Some(sector_offset * self.sector_size())
    }
}

pub fn read_boot_sector<S: BlockSource + ?Sized>(
    source: &S,
    volume_offset: u64,
) -> Result<ExFatBootSector, ExFatError> {
    let mut buf = [0_u8; 512];
    let res = source
        .read_at(volume_offset, &mut buf)
        .map_err(ExFatError::Read)?;
    if res.bytes_read < 512 {
        return Err(ExFatError::UnexpectedEof);
    }
    ExFatBootSector::parse(&buf)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExFatEntry {
    pub is_deleted: bool,
    pub is_directory: bool,
    pub name: String,
    pub size: u64,
    pub first_cluster: u32,
    pub no_fat_chain: bool,
    pub file_offset: u64,
    pub parent_offset: Option<u64>,
    pub created_time: Option<u64>,
    pub modified_time: Option<u64>,
    pub accessed_time: Option<u64>,
}

pub fn read_fat_chain<S: BlockSource + ?Sized>(
    source: &S,
    boot: &ExFatBootSector,
    volume_offset: u64,
    start_cluster: u32,
) -> Result<Vec<u32>, ExFatError> {
    let fat_start = volume_offset + boot.fat_offset as u64 * boot.sector_size();
    let mut chain = Vec::new();
    let mut current = start_cluster;
    let max_clusters = boot.cluster_count.min(100_000);

    while (2..0xFFFFFFF0).contains(&current) {
        if chain.len() >= max_clusters as usize {
            break;
        }
        chain.push(current);

        let entry_offset = fat_start + current as u64 * 4;
        let mut buf = [0_u8; 4];
        let read_res = source
            .read_at(entry_offset, &mut buf)
            .map_err(ExFatError::Read)?;
        if read_res.bytes_read < 4 {
            return Err(ExFatError::UnexpectedEof);
        }
        current = u32::from_le_bytes(buf);
    }

    Ok(chain)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExFatEntryReadMode {
    Empty,
    FatChain,
    ContiguousAllocation,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExFatEntryReadReport {
    pub data: Vec<u8>,
    pub mode: ExFatEntryReadMode,
    pub complete: bool,
    pub bytes_requested: u64,
    pub bytes_read: u64,
    pub bytes_lost: u64,
    pub clusters_used: usize,
    pub reason: String,
}

impl ExFatEntryReadReport {
    fn empty(data_len: u64, reason: impl Into<String>) -> Self {
        Self {
            data: Vec::new(),
            mode: ExFatEntryReadMode::Empty,
            complete: data_len == 0,
            bytes_requested: data_len,
            bytes_read: 0,
            bytes_lost: data_len,
            clusters_used: 0,
            reason: reason.into(),
        }
    }
}

pub fn read_entry_data_with_report<S: BlockSource + ?Sized>(
    source: &S,
    boot: &ExFatBootSector,
    volume_offset: u64,
    first_cluster: u32,
    data_len: u64,
    no_fat_chain: bool,
) -> Result<ExFatEntryReadReport, ExFatError> {
    if data_len == 0 {
        return Ok(ExFatEntryReadReport::empty(data_len, "empty file"));
    }
    if first_cluster < 2 {
        return Ok(ExFatEntryReadReport::empty(
            data_len,
            "missing first cluster",
        ));
    }
    if data_len > usize::MAX as u64 {
        return Err(ExFatError::OffsetOverflow);
    }

    let cluster_size = boot.cluster_size();
    let needed_clusters = data_len.div_ceil(cluster_size);
    let mut output = Vec::with_capacity(data_len as usize);
    let mut remaining = data_len;

    if no_fat_chain {
        let mut current_cluster = first_cluster;
        while remaining > 0 {
            let Some(cluster_offset) = boot.cluster_to_byte_offset(current_cluster) else {
                return Err(ExFatError::InvalidCluster(current_cluster));
            };
            let bytes_to_read = remaining.min(cluster_size) as usize;
            let mut buf = vec![0_u8; bytes_to_read];
            let res = source
                .read_at(volume_offset + cluster_offset, &mut buf)
                .map_err(ExFatError::Read)?;
            if res.bytes_read < bytes_to_read {
                output.extend_from_slice(&buf[..res.bytes_read]);
                let bytes_read = output.len() as u64;
                return Ok(ExFatEntryReadReport {
                    data: output,
                    mode: ExFatEntryReadMode::ContiguousAllocation,
                    complete: false,
                    bytes_requested: data_len,
                    bytes_read,
                    bytes_lost: data_len.saturating_sub(bytes_read),
                    clusters_used: (current_cluster - first_cluster + 1) as usize,
                    reason: "contiguous allocation read hit EOF before requested size".to_string(),
                });
            }
            output.extend_from_slice(&buf);
            remaining -= bytes_to_read as u64;
            current_cluster += 1;
        }

        return Ok(ExFatEntryReadReport {
            data: output,
            mode: ExFatEntryReadMode::ContiguousAllocation,
            complete: true,
            bytes_requested: data_len,
            bytes_read: data_len,
            bytes_lost: 0,
            clusters_used: needed_clusters as usize,
            reason: "recovered from exFAT contiguous allocation flag".to_string(),
        });
    }

    let chain = read_fat_chain(source, boot, volume_offset, first_cluster)?;
    for &cluster in &chain {
        if remaining == 0 {
            break;
        }
        let Some(cluster_offset) = boot.cluster_to_byte_offset(cluster) else {
            return Err(ExFatError::InvalidCluster(cluster));
        };
        let bytes_to_read = remaining.min(cluster_size) as usize;
        let mut buf = vec![0_u8; bytes_to_read];
        let res = source
            .read_at(volume_offset + cluster_offset, &mut buf)
            .map_err(ExFatError::Read)?;
        if res.bytes_read < bytes_to_read {
            output.extend_from_slice(&buf[..res.bytes_read]);
            let bytes_read = output.len() as u64;
            return Ok(ExFatEntryReadReport {
                data: output,
                mode: ExFatEntryReadMode::FatChain,
                complete: false,
                bytes_requested: data_len,
                bytes_read,
                bytes_lost: data_len.saturating_sub(bytes_read),
                clusters_used: chain.len(),
                reason: "FAT chain read ended before requested size".to_string(),
            });
        }
        output.extend_from_slice(&buf);
        remaining -= bytes_to_read as u64;
    }

    let bytes_read = output.len() as u64;
    Ok(ExFatEntryReadReport {
        data: output,
        mode: ExFatEntryReadMode::FatChain,
        complete: bytes_read >= data_len,
        bytes_requested: data_len,
        bytes_read,
        bytes_lost: data_len.saturating_sub(bytes_read),
        clusters_used: chain.len(),
        reason: if bytes_read >= data_len {
            "recovered from exFAT FAT chain".to_string()
        } else {
            "exFAT FAT chain shorter than requested size".to_string()
        },
    })
}

pub fn read_entry_data<S: BlockSource + ?Sized>(
    source: &S,
    boot: &ExFatBootSector,
    volume_offset: u64,
    first_cluster: u32,
    data_len: u64,
    no_fat_chain: bool,
) -> Result<Vec<u8>, ExFatError> {
    Ok(read_entry_data_with_report(
        source,
        boot,
        volume_offset,
        first_cluster,
        data_len,
        no_fat_chain,
    )?
    .data)
}

pub fn parse_directory_entries(buf: &[u8], base_offset: u64) -> Vec<ExFatEntry> {
    let mut entries = Vec::new();
    let mut i = 0;

    while i + 32 <= buf.len() {
        let entry_offset = base_offset + i as u64;
        let entry_bytes = &buf[i..i + 32];
        let entry_type = entry_bytes[0];

        if entry_type == 0x00 {
            break;
        }

        if entry_type == 0x85 || entry_type == 0x05 {
            let is_deleted = entry_type == 0x05;
            let secondary_count = entry_bytes[1] as usize;
            let file_attributes = u16::from_le_bytes([entry_bytes[4], entry_bytes[5]]);
            let is_directory = (file_attributes & 0x10) != 0;

            let create_time = u32::from_le_bytes([
                entry_bytes[8],
                entry_bytes[9],
                entry_bytes[10],
                entry_bytes[11],
            ]);
            let modified_time = u32::from_le_bytes([
                entry_bytes[12],
                entry_bytes[13],
                entry_bytes[14],
                entry_bytes[15],
            ]);
            let accessed_time = u32::from_le_bytes([
                entry_bytes[16],
                entry_bytes[17],
                entry_bytes[18],
                entry_bytes[19],
            ]);

            if i + 32 + secondary_count * 32 <= buf.len() {
                let stream_bytes = &buf[i + 32..i + 64];
                let stream_type = stream_bytes[0];

                if stream_type == 0xC0 || stream_type == 0x40 {
                    let gen_flags = stream_bytes[1];
                    let no_fat_chain = (gen_flags & 0x02) != 0;
                    let name_len = stream_bytes[3] as usize;
                    let first_cluster = u32::from_le_bytes([
                        stream_bytes[20],
                        stream_bytes[21],
                        stream_bytes[22],
                        stream_bytes[23],
                    ]);
                    let data_len = u64::from_le_bytes([
                        stream_bytes[24],
                        stream_bytes[25],
                        stream_bytes[26],
                        stream_bytes[27],
                        stream_bytes[28],
                        stream_bytes[29],
                        stream_bytes[30],
                        stream_bytes[31],
                    ]);

                    let mut utf16_chars = Vec::new();
                    for k in 2..=secondary_count {
                        let name_bytes = &buf[i + k * 32..i + (k + 1) * 32];
                        let name_type = name_bytes[0];
                        if name_type == 0xC1 || name_type == 0x41 {
                            for char_idx in 0..15 {
                                let c_offset = 2 + char_idx * 2;
                                let char_val = u16::from_le_bytes([
                                    name_bytes[c_offset],
                                    name_bytes[c_offset + 1],
                                ]);
                                if char_val != 0 {
                                    utf16_chars.push(char_val);
                                }
                            }
                        }
                    }

                    let mut name = String::from_utf16_lossy(&utf16_chars);
                    if name.len() > name_len {
                        name.truncate(name_len);
                    }

                    entries.push(ExFatEntry {
                        is_deleted,
                        is_directory,
                        name,
                        size: data_len,
                        first_cluster,
                        no_fat_chain,
                        file_offset: entry_offset,
                        parent_offset: None,
                        created_time: Some(create_time as u64),
                        modified_time: Some(modified_time as u64),
                        accessed_time: Some(accessed_time as u64),
                    });
                }
            }
            i += 32 + secondary_count * 32;
        } else {
            i += 32;
        }
    }

    entries
}

pub fn list_directory<S: BlockSource + ?Sized>(
    source: &S,
    boot: &ExFatBootSector,
    volume_offset: u64,
    first_cluster: u32,
    data_len: u64,
    no_fat_chain: bool,
) -> Result<Vec<ExFatEntry>, ExFatError> {
    let cluster_size = boot.cluster_size();
    let mut buf = Vec::new();

    if data_len > 0 {
        buf = read_entry_data(
            source,
            boot,
            volume_offset,
            first_cluster,
            data_len,
            no_fat_chain,
        )?;
    } else {
        let chain = if no_fat_chain {
            let mut c = Vec::new();
            let mut curr = first_cluster;
            for _ in 0..1000 {
                c.push(curr);
                curr += 1;
            }
            c
        } else {
            read_fat_chain(source, boot, volume_offset, first_cluster)?
        };

        for cluster in chain {
            let Some(cluster_offset) = boot.cluster_to_byte_offset(cluster) else {
                break;
            };
            let mut cluster_buf = vec![0_u8; cluster_size as usize];
            let read_res = source.read_at(volume_offset + cluster_offset, &mut cluster_buf)?;
            if read_res.bytes_read == 0 {
                break;
            }
            let mut hit_end = false;
            for chunk in cluster_buf.chunks_exact(32) {
                if chunk[0] == 0x00 {
                    hit_end = true;
                    break;
                }
            }
            buf.extend_from_slice(&cluster_buf);
            if hit_end {
                break;
            }
        }
    }

    let base_offset = if first_cluster >= 2 {
        volume_offset + boot.cluster_to_byte_offset(first_cluster).unwrap_or(0)
    } else {
        0
    };

    Ok(parse_directory_entries(&buf, base_offset))
}

pub fn scan_exfat_vfs<S: BlockSource + ?Sized>(
    source: &S,
    boot: &ExFatBootSector,
    volume_offset: u64,
) -> Result<Vec<ExFatEntry>, ExFatError> {
    let mut all_entries = Vec::new();
    let mut queue = VecDeque::new();

    let root_cluster = boot.first_cluster_of_root_directory;
    let root_entries = list_directory(source, boot, volume_offset, root_cluster, 0, false)?;

    for mut entry in root_entries {
        // Root-level entries have no parent in the VFS tree. Using the root cluster
        // byte offset as parent_offset would alias with the first entry's file_offset,
        // creating a self-reference cycle in VfsTree::path_for.
        entry.parent_offset = None;
        if entry.is_directory && !entry.is_deleted && entry.first_cluster >= 2 {
            queue.push_back((
                entry.first_cluster,
                entry.size,
                entry.no_fat_chain,
                entry.file_offset,
            ));
        }
        all_entries.push(entry);
    }

    let mut dirs_visited = 0;
    while let Some((cluster, size, no_fat_chain, parent_offset)) = queue.pop_front() {
        if dirs_visited > 1000 {
            break;
        }
        dirs_visited += 1;

        if let Ok(sub_entries) =
            list_directory(source, boot, volume_offset, cluster, size, no_fat_chain)
        {
            for mut entry in sub_entries {
                entry.parent_offset = Some(parent_offset);
                if entry.is_directory && !entry.is_deleted && entry.first_cluster >= 2 {
                    queue.push_back((
                        entry.first_cluster,
                        entry.size,
                        entry.no_fat_chain,
                        entry.file_offset,
                    ));
                }
                all_entries.push(entry);
            }
        }
    }

    Ok(all_entries)
}

#[cfg(test)]
mod tests {
    use super::*;
    use data_recovery_core::PhysicalDevice;

    struct MemBlockSource {
        device: PhysicalDevice,
        data: Vec<u8>,
    }

    impl MemBlockSource {
        fn new(data: Vec<u8>) -> Self {
            Self {
                device: PhysicalDevice {
                    id: "mem".into(),
                    display_name: "mem".into(),
                    platform_path: "mem".into(),
                    size_bytes: data.len() as u64,
                    logical_sector_size: 512,
                    physical_sector_size: 512,
                    is_removable: false,
                    is_system_disk: false,
                    bus_type: data_recovery_core::BusType::Virtual,
                    trim_supported: None,
                },
                data,
            }
        }
    }

    impl BlockSource for MemBlockSource {
        fn device(&self) -> &PhysicalDevice {
            &self.device
        }

        fn read_at(
            &self,
            offset: u64,
            buffer: &mut [u8],
        ) -> Result<data_recovery_core::ReadResult, ReadError> {
            let offset = offset as usize;
            if offset >= self.data.len() {
                return Ok(data_recovery_core::ReadResult {
                    offset: offset as u64,
                    bytes_read: 0,
                });
            }
            let bytes_to_read = buffer.len().min(self.data.len() - offset);
            buffer[..bytes_to_read].copy_from_slice(&self.data[offset..offset + bytes_to_read]);
            Ok(data_recovery_core::ReadResult {
                offset: offset as u64,
                bytes_read: bytes_to_read,
            })
        }
    }

    fn make_mock_exfat_boot() -> [u8; 512] {
        let mut boot = [0_u8; 512];
        boot[3..11].copy_from_slice(b"EXFAT   ");
        boot[510] = 0x55;
        boot[511] = 0xAA;

        // PartitionOffset = 0
        boot[64..72].copy_from_slice(&0_u64.to_le_bytes());
        // VolumeLength = 1000
        boot[72..80].copy_from_slice(&1000_u64.to_le_bytes());
        // FatOffset = 32
        boot[80..84].copy_from_slice(&32_u32.to_le_bytes());
        // FatLength = 32
        boot[84..88].copy_from_slice(&32_u32.to_le_bytes());
        // ClusterHeapOffset = 128
        boot[88..92].copy_from_slice(&128_u32.to_le_bytes());
        // ClusterCount = 100
        boot[92..96].copy_from_slice(&100_u32.to_le_bytes());
        // FirstClusterOfRootDirectory = 2
        boot[96..100].copy_from_slice(&2_u32.to_le_bytes());
        // BytesPerSectorShift = 9 (512 bytes)
        boot[108] = 9;
        // SectorsPerClusterShift = 3 (8 sectors = 4096 bytes)
        boot[109] = 3;

        boot
    }

    #[test]
    fn parses_exfat_boot_sector() {
        let raw = make_mock_exfat_boot();
        let parsed = ExFatBootSector::parse(&raw).unwrap();
        assert_eq!(parsed.oem_name, *b"EXFAT   ");
        assert_eq!(parsed.sector_size(), 512);
        assert_eq!(parsed.cluster_size(), 4096);
        assert_eq!(parsed.cluster_to_byte_offset(2), Some(128 * 512));
    }

    #[test]
    fn parses_exfat_directory_entries() {
        let mut buf = vec![0_u8; 96];

        // File Entry (0x85)
        buf[0] = 0x85;
        buf[1] = 2; // secondary count
        buf[4..6].copy_from_slice(&0x20_u16.to_le_bytes()); // Archive file attributes

        // Stream Extension (0xC0)
        buf[32] = 0xC0;
        buf[33] = 0x03; // AllocationPossible | NoFatChain
        buf[35] = 8; // NameLength
        buf[52..56].copy_from_slice(&5_u32.to_le_bytes()); // FirstCluster
        buf[56..64].copy_from_slice(&100_u64.to_le_bytes()); // DataLength

        // File Name (0xC1)
        buf[64] = 0xC1;
        let name_utf16 = "test.txt".encode_utf16().collect::<Vec<_>>();
        for (idx, &c) in name_utf16.iter().enumerate() {
            buf[66 + idx * 2..68 + idx * 2].copy_from_slice(&c.to_le_bytes());
        }

        let parsed = parse_directory_entries(&buf, 1000);
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].name, "test.txt");
        assert_eq!(parsed[0].size, 100);
        assert_eq!(parsed[0].first_cluster, 5);
        assert!(parsed[0].no_fat_chain);
        assert!(!parsed[0].is_deleted);
        assert!(!parsed[0].is_directory);
    }

    #[test]
    fn scans_exfat_volume() {
        let mut data = vec![0_u8; 256 * 1024]; // 256 KB
        data[..512].copy_from_slice(&make_mock_exfat_boot());

        // Root directory starts at cluster 2 (offset 65536)
        let mut root_dir = vec![0_u8; 4096];

        // 1. Dir entry for "hello.txt"
        root_dir[0] = 0x85;
        root_dir[1] = 2;
        root_dir[4..6].copy_from_slice(&0x20_u16.to_le_bytes()); // Archive

        root_dir[32] = 0xC0;
        root_dir[33] = 0x03; // AllocationPossible | NoFatChain
        root_dir[35] = 9; // NameLength
        root_dir[52..56].copy_from_slice(&3_u32.to_le_bytes()); // FirstCluster
        root_dir[56..64].copy_from_slice(&15_u64.to_le_bytes()); // DataLength

        root_dir[64] = 0xC1;
        let name1 = "hello.txt".encode_utf16().collect::<Vec<_>>();
        for (idx, &c) in name1.iter().enumerate() {
            root_dir[66 + idx * 2..68 + idx * 2].copy_from_slice(&c.to_le_bytes());
        }

        // 2. Dir entry for "docs" (Directory)
        root_dir[96] = 0x85;
        root_dir[97] = 2;
        root_dir[100..102].copy_from_slice(&0x10_u16.to_le_bytes()); // Directory

        root_dir[128] = 0xC0;
        root_dir[129] = 0x03;
        root_dir[131] = 4;
        root_dir[148..152].copy_from_slice(&4_u32.to_le_bytes());
        root_dir[152..160].copy_from_slice(&0_u64.to_le_bytes());

        root_dir[160] = 0xC1;
        let name2 = "docs".encode_utf16().collect::<Vec<_>>();
        for (idx, &c) in name2.iter().enumerate() {
            root_dir[162 + idx * 2..164 + idx * 2].copy_from_slice(&c.to_le_bytes());
        }

        // 3. Deleted File entry for "deleted.bin"
        root_dir[192] = 0x05;
        root_dir[193] = 2;
        root_dir[196..198].copy_from_slice(&0x20_u16.to_le_bytes());

        root_dir[224] = 0x40;
        root_dir[225] = 0x03;
        root_dir[227] = 11;
        root_dir[244..248].copy_from_slice(&5_u32.to_le_bytes());
        root_dir[248..256].copy_from_slice(&20000_u64.to_le_bytes());

        root_dir[256] = 0x41;
        let name3 = "deleted.bin".encode_utf16().collect::<Vec<_>>();
        for (idx, &c) in name3.iter().enumerate() {
            root_dir[258 + idx * 2..260 + idx * 2].copy_from_slice(&c.to_le_bytes());
        }

        data[65536..65536 + 4096].copy_from_slice(&root_dir);

        // Subdirectory "docs" starts at cluster 4 (offset 65536 + 2 * 4096 = 73728)
        let mut docs_dir = vec![0_u8; 4096];

        // 1. Dir entry for "readme.md"
        docs_dir[0] = 0x85;
        docs_dir[1] = 2;
        docs_dir[4..6].copy_from_slice(&0x20_u16.to_le_bytes());

        docs_dir[32] = 0xC0;
        docs_dir[33] = 0x03;
        docs_dir[35] = 9;
        docs_dir[52..56].copy_from_slice(&6_u32.to_le_bytes());
        docs_dir[56..64].copy_from_slice(&40_u64.to_le_bytes());

        docs_dir[64] = 0xC1;
        let name4 = "readme.md".encode_utf16().collect::<Vec<_>>();
        for (idx, &c) in name4.iter().enumerate() {
            docs_dir[66 + idx * 2..68 + idx * 2].copy_from_slice(&c.to_le_bytes());
        }

        data[73728..73728 + 4096].copy_from_slice(&docs_dir);

        // File "readme.md" content at cluster 6 (offset 65536 + 4 * 4096 = 81920)
        let file_content = b"this is the readme content in exfat file";
        data[81920..81920 + file_content.len()].copy_from_slice(file_content);

        let source = MemBlockSource::new(data);
        let boot = read_boot_sector(&source, 0).unwrap();
        let entries = scan_exfat_vfs(&source, &boot, 0).unwrap();

        assert_eq!(entries.len(), 4);

        // Verify "hello.txt"
        let hello = entries.iter().find(|e| e.name == "hello.txt").unwrap();
        assert!(!hello.is_directory);
        assert!(!hello.is_deleted);
        assert_eq!(hello.size, 15);
        assert_eq!(hello.first_cluster, 3);

        // Verify "docs"
        let docs = entries.iter().find(|e| e.name == "docs").unwrap();
        assert!(docs.is_directory);
        assert!(!docs.is_deleted);
        assert_eq!(docs.first_cluster, 4);

        // Verify "deleted.bin"
        let deleted = entries.iter().find(|e| e.name == "deleted.bin").unwrap();
        assert!(!deleted.is_directory);
        assert!(deleted.is_deleted);
        assert_eq!(deleted.size, 20000);
        assert_eq!(deleted.first_cluster, 5);

        // Verify "readme.md"
        let readme = entries.iter().find(|e| e.name == "readme.md").unwrap();
        assert!(!readme.is_directory);
        assert!(!readme.is_deleted);
        assert_eq!(readme.size, 40);
        assert_eq!(readme.first_cluster, 6);

        // Verify content extraction
        let content = read_entry_data(
            &source,
            &boot,
            0,
            readme.first_cluster,
            readme.size,
            readme.no_fat_chain,
        )
        .unwrap();
        assert_eq!(content, file_content);
    }
}
