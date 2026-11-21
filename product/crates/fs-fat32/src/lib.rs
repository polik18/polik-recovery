use data_recovery_core::{BlockSource, ReadError};
use std::collections::VecDeque;

#[derive(Debug)]
pub enum Fat32Error {
    Read(ReadError),
    InvalidBootSignature(u16),
    InvalidBytesPerSector(u16),
    InvalidSectorsPerCluster(u8),
    InvalidFatCount(u8),
    InvalidFatSize,
    InvalidRootCluster(u32),
    InvalidCluster(u32),
    UnexpectedEof,
    OffsetOverflow,
}

impl std::fmt::Display for Fat32Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Read(e) => write!(f, "Read error: {e}"),
            Self::InvalidBootSignature(sig) => write!(f, "Invalid FAT boot signature: {sig:#06x}"),
            Self::InvalidBytesPerSector(v) => write!(f, "Invalid FAT32 bytes per sector: {v}"),
            Self::InvalidSectorsPerCluster(v) => {
                write!(f, "Invalid FAT32 sectors per cluster: {v}")
            }
            Self::InvalidFatCount(v) => write!(f, "Invalid FAT count: {v}"),
            Self::InvalidFatSize => write!(f, "Invalid FAT32 FAT size"),
            Self::InvalidRootCluster(v) => write!(f, "Invalid FAT32 root cluster: {v}"),
            Self::InvalidCluster(v) => write!(f, "Invalid FAT32 cluster: {v}"),
            Self::UnexpectedEof => write!(f, "Unexpected end of file/stream"),
            Self::OffsetOverflow => write!(f, "Computed offset overflowed u64"),
        }
    }
}

impl std::error::Error for Fat32Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Read(err) => Some(err),
            _ => None,
        }
    }
}

impl From<ReadError> for Fat32Error {
    fn from(value: ReadError) -> Self {
        Self::Read(value)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fat32BootSector {
    pub bytes_per_sector: u16,
    pub sectors_per_cluster: u8,
    pub reserved_sector_count: u16,
    pub number_of_fats: u8,
    pub total_sectors: u32,
    pub fat_size_32: u32,
    pub root_cluster: u32,
    pub fs_info_sector: u16,
    pub backup_boot_sector: u16,
    pub volume_id: u32,
    pub volume_label: String,
    pub fs_type_label: String,
}

impl Fat32BootSector {
    pub fn parse(bytes: &[u8; 512]) -> Result<Self, Fat32Error> {
        let boot_sig = u16::from_le_bytes([bytes[510], bytes[511]]);
        if boot_sig != 0xAA55 {
            return Err(Fat32Error::InvalidBootSignature(boot_sig));
        }

        let bytes_per_sector = u16::from_le_bytes([bytes[11], bytes[12]]);
        if !matches!(bytes_per_sector, 512 | 1024 | 2048 | 4096) {
            return Err(Fat32Error::InvalidBytesPerSector(bytes_per_sector));
        }

        let sectors_per_cluster = bytes[13];
        if sectors_per_cluster == 0 || !sectors_per_cluster.is_power_of_two() {
            return Err(Fat32Error::InvalidSectorsPerCluster(sectors_per_cluster));
        }

        let number_of_fats = bytes[16];
        if number_of_fats == 0 || number_of_fats > 2 {
            return Err(Fat32Error::InvalidFatCount(number_of_fats));
        }

        let total_sectors_16 = u16::from_le_bytes([bytes[19], bytes[20]]) as u32;
        let total_sectors_32 = u32::from_le_bytes([bytes[32], bytes[33], bytes[34], bytes[35]]);
        let total_sectors = if total_sectors_16 != 0 {
            total_sectors_16
        } else {
            total_sectors_32
        };
        let fat_size_32 = u32::from_le_bytes([bytes[36], bytes[37], bytes[38], bytes[39]]);
        if fat_size_32 == 0 {
            return Err(Fat32Error::InvalidFatSize);
        }

        let root_cluster = u32::from_le_bytes([bytes[44], bytes[45], bytes[46], bytes[47]]);
        if root_cluster < 2 {
            return Err(Fat32Error::InvalidRootCluster(root_cluster));
        }

        Ok(Self {
            bytes_per_sector,
            sectors_per_cluster,
            reserved_sector_count: u16::from_le_bytes([bytes[14], bytes[15]]),
            number_of_fats,
            total_sectors,
            fat_size_32,
            root_cluster,
            fs_info_sector: u16::from_le_bytes([bytes[48], bytes[49]]),
            backup_boot_sector: u16::from_le_bytes([bytes[50], bytes[51]]),
            volume_id: u32::from_le_bytes([bytes[67], bytes[68], bytes[69], bytes[70]]),
            volume_label: ascii_trim(&bytes[71..82]),
            fs_type_label: ascii_trim(&bytes[82..90]),
        })
    }

    pub fn sector_size(&self) -> u64 {
        self.bytes_per_sector as u64
    }

    pub fn cluster_size(&self) -> u64 {
        self.bytes_per_sector as u64 * self.sectors_per_cluster as u64
    }

    pub fn first_data_sector(&self) -> u64 {
        self.reserved_sector_count as u64 + self.number_of_fats as u64 * self.fat_size_32 as u64
    }

    pub fn cluster_count(&self) -> u32 {
        let data_sectors = self.total_sectors as u64 - self.first_data_sector();
        (data_sectors / self.sectors_per_cluster as u64) as u32
    }

    pub fn fat_offset(&self) -> u64 {
        self.reserved_sector_count as u64 * self.sector_size()
    }

    pub fn cluster_to_byte_offset(&self, cluster: u32) -> Option<u64> {
        if cluster < 2 || cluster >= 2 + self.cluster_count() {
            return None;
        }
        let sector =
            self.first_data_sector() + (cluster as u64 - 2) * self.sectors_per_cluster as u64;
        sector.checked_mul(self.sector_size())
    }
}

pub fn read_boot_sector<S: BlockSource + ?Sized>(
    source: &S,
    volume_offset: u64,
) -> Result<Fat32BootSector, Fat32Error> {
    let mut buf = [0_u8; 512];
    let res = source.read_at(volume_offset, &mut buf)?;
    if res.bytes_read < 512 {
        return Err(Fat32Error::UnexpectedEof);
    }
    Fat32BootSector::parse(&buf)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fat32Entry {
    pub is_deleted: bool,
    pub is_directory: bool,
    pub name: String,
    pub short_name: String,
    pub size: u64,
    pub first_cluster: u32,
    pub file_offset: u64,
    pub parent_offset: Option<u64>,
    pub created_time: Option<u64>,
    pub modified_time: Option<u64>,
    pub accessed_time: Option<u64>,
}

/// How a deleted entry's data would be recovered and how much of it is
/// known to be overwritten. Deleted entries can never be proven intact
/// (FAT has no data checksums), so callers must not label them complete.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeletedEntryAssessment {
    pub clusters: Vec<u32>,
    pub used_chain: bool,
    pub overwritten_clusters: usize,
    pub overwritten_bytes: u64,
}

/// Clusters currently owned by live (non-deleted) entries and the root
/// directory. A deleted entry whose clusters appear here was overwritten.
pub fn live_cluster_set<S: BlockSource + ?Sized>(
    source: &S,
    boot: &Fat32BootSector,
    volume_offset: u64,
    entries: &[Fat32Entry],
) -> std::collections::HashSet<u32> {
    let mut live = std::collections::HashSet::new();
    live.extend(read_fat_chain(source, boot, volume_offset, boot.root_cluster).unwrap_or_default());
    for entry in entries
        .iter()
        .filter(|e| !e.is_deleted && e.first_cluster >= 2)
    {
        live.extend(
            read_fat_chain(source, boot, volume_offset, entry.first_cluster).unwrap_or_default(),
        );
    }
    live
}

/// Decide which clusters hold a deleted entry's data: its retained FAT chain
/// when that chain is long enough and owned by no live file, otherwise the
/// contiguous run from its first cluster. Reports live-owned clusters.
pub fn assess_deleted_entry<S: BlockSource + ?Sized>(
    source: &S,
    boot: &Fat32BootSector,
    volume_offset: u64,
    first_cluster: u32,
    data_len: u64,
    _no_fat_chain: bool,
    live: &std::collections::HashSet<u32>,
) -> DeletedEntryAssessment {
    let cluster_size = boot.cluster_size();
    let needed = data_len.div_ceil(cluster_size) as usize;
    let max_cluster = 2_u64 + boot.cluster_count() as u64;
    let chain = if first_cluster >= 2 {
        read_fat_chain(source, boot, volume_offset, first_cluster).unwrap_or_default()
    } else {
        Vec::new()
    };
    let chain_usable =
        needed > 0 && chain.len() >= needed && chain[..needed].iter().all(|c| !live.contains(c));
    let (clusters, used_chain) = if chain_usable {
        (chain[..needed].to_vec(), true)
    } else if first_cluster >= 2 {
        let end = (first_cluster as u64 + needed as u64).min(max_cluster);
        ((first_cluster..end as u32).collect(), false)
    } else {
        (Vec::new(), false)
    };
    let overwritten_clusters = clusters.iter().filter(|c| live.contains(c)).count();
    let overwritten_bytes = (overwritten_clusters as u64 * cluster_size).min(data_len);
    DeletedEntryAssessment {
        clusters,
        used_chain,
        overwritten_clusters,
        overwritten_bytes,
    }
}

/// Read `data_len` bytes from an explicit cluster list.
pub fn read_clusters<S: BlockSource + ?Sized>(
    source: &S,
    boot: &Fat32BootSector,
    volume_offset: u64,
    clusters: &[u32],
    data_len: u64,
) -> Result<Vec<u8>, Fat32Error> {
    let cluster_size = boot.cluster_size();
    let mut output = Vec::with_capacity(data_len.min(usize::MAX as u64) as usize);
    let mut remaining = data_len;
    for &cluster in clusters {
        if remaining == 0 {
            break;
        }
        let Some(offset) = boot.cluster_to_byte_offset(cluster) else {
            break;
        };
        let mut buf = vec![0_u8; remaining.min(cluster_size) as usize];
        let read = source.read_at(volume_offset + offset, &mut buf)?;
        output.extend_from_slice(&buf[..read.bytes_read]);
        if read.bytes_read < buf.len() {
            break;
        }
        remaining -= buf.len() as u64;
    }
    Ok(output)
}

pub fn read_fat_chain<S: BlockSource + ?Sized>(
    source: &S,
    boot: &Fat32BootSector,
    volume_offset: u64,
    start_cluster: u32,
) -> Result<Vec<u32>, Fat32Error> {
    let mut chain = Vec::new();
    let mut current = start_cluster;
    let max_clusters = boot.cluster_count().min(500_000);
    while (2..0x0FFF_FFF8).contains(&current) {
        if chain.len() >= max_clusters as usize {
            break;
        }
        if current >= 2 + boot.cluster_count() {
            return Err(Fat32Error::InvalidCluster(current));
        }
        chain.push(current);
        let entry_offset = volume_offset
            .checked_add(boot.fat_offset())
            .and_then(|v| v.checked_add(current as u64 * 4))
            .ok_or(Fat32Error::OffsetOverflow)?;
        let mut buf = [0_u8; 4];
        let res = source.read_at(entry_offset, &mut buf)?;
        if res.bytes_read < 4 {
            return Err(Fat32Error::UnexpectedEof);
        }
        current = u32::from_le_bytes(buf) & 0x0FFF_FFFF;
        if current == 0 {
            break;
        }
    }
    Ok(chain)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fat32EntryReadMode {
    Empty,
    FatChain,
    ContiguousFallback,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fat32EntryReadReport {
    pub data: Vec<u8>,
    pub mode: Fat32EntryReadMode,
    pub complete: bool,
    pub bytes_requested: u64,
    pub bytes_read: u64,
    pub bytes_lost: u64,
    pub clusters_used: usize,
    pub reason: String,
}

impl Fat32EntryReadReport {
    fn empty(data_len: u64, reason: impl Into<String>) -> Self {
        Self {
            data: Vec::new(),
            mode: Fat32EntryReadMode::Empty,
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
    boot: &Fat32BootSector,
    volume_offset: u64,
    first_cluster: u32,
    data_len: u64,
    allow_contiguous_fallback: bool,
) -> Result<Fat32EntryReadReport, Fat32Error> {
    if data_len == 0 {
        return Ok(Fat32EntryReadReport::empty(data_len, "empty file"));
    }
    if first_cluster < 2 {
        return Ok(Fat32EntryReadReport::empty(
            data_len,
            "missing first cluster",
        ));
    }
    if data_len > usize::MAX as u64 {
        return Err(Fat32Error::OffsetOverflow);
    }

    let cluster_size = boot.cluster_size();
    let needed_clusters = data_len.div_ceil(cluster_size);

    // v0.8: Always try the FAT chain first when it still exists, even for deleted entries.
    // Freshly deleted FAT32 entries sometimes keep a usable chain; blindly switching to
    // contiguous reads makes fragmented-but-recoverable files turn into garbage.
    let chain = read_fat_chain(source, boot, volume_offset, first_cluster).unwrap_or_default();
    if !chain.is_empty() {
        let mut output = Vec::with_capacity(data_len as usize);
        let mut remaining = data_len;
        for &cluster in &chain {
            if remaining == 0 {
                break;
            }
            let Some(cluster_offset) = boot.cluster_to_byte_offset(cluster) else {
                return Err(Fat32Error::InvalidCluster(cluster));
            };
            let bytes_to_read = remaining.min(cluster_size) as usize;
            let mut buf = vec![0_u8; bytes_to_read];
            let res = source.read_at(volume_offset + cluster_offset, &mut buf)?;
            if res.bytes_read < bytes_to_read {
                output.extend_from_slice(&buf[..res.bytes_read]);
                let bytes_read = output.len() as u64;
                return Ok(Fat32EntryReadReport {
                    data: output,
                    mode: Fat32EntryReadMode::FatChain,
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
        if output.len() as u64 >= data_len {
            output.truncate(data_len as usize);
            return Ok(Fat32EntryReadReport {
                data: output,
                mode: Fat32EntryReadMode::FatChain,
                complete: true,
                bytes_requested: data_len,
                bytes_read: data_len,
                bytes_lost: 0,
                clusters_used: chain.len(),
                reason: "recovered from FAT chain".to_string(),
            });
        }

        if !allow_contiguous_fallback {
            let bytes_read = output.len() as u64;
            return Ok(Fat32EntryReadReport {
                data: output,
                mode: Fat32EntryReadMode::FatChain,
                complete: false,
                bytes_requested: data_len,
                bytes_read,
                bytes_lost: data_len.saturating_sub(bytes_read),
                clusters_used: chain.len(),
                reason: "FAT chain shorter than requested size".to_string(),
            });
        }
    }

    if !allow_contiguous_fallback {
        return Ok(Fat32EntryReadReport::empty(
            data_len,
            "no FAT chain and contiguous fallback disabled",
        ));
    }

    // Deleted FAT32 files often have their FAT chain zeroed. A contiguous read from
    // the original first cluster is a best-effort fallback only; for multi-cluster
    // files it is marked suspect because fragmentation cannot be ruled out.
    if first_cluster as u64 + needed_clusters > 2 + boot.cluster_count() as u64 {
        return Err(Fat32Error::InvalidCluster(first_cluster));
    }

    let mut output = Vec::with_capacity(data_len as usize);
    let mut remaining = data_len;
    let mut cluster = first_cluster;
    while remaining > 0 {
        let Some(cluster_offset) = boot.cluster_to_byte_offset(cluster) else {
            return Err(Fat32Error::InvalidCluster(cluster));
        };
        let bytes_to_read = remaining.min(cluster_size) as usize;
        let mut buf = vec![0_u8; bytes_to_read];
        let res = source.read_at(volume_offset + cluster_offset, &mut buf)?;
        if res.bytes_read < bytes_to_read {
            output.extend_from_slice(&buf[..res.bytes_read]);
            let bytes_read = output.len() as u64;
            return Ok(Fat32EntryReadReport {
                data: output,
                mode: Fat32EntryReadMode::ContiguousFallback,
                complete: false,
                bytes_requested: data_len,
                bytes_read,
                bytes_lost: data_len.saturating_sub(bytes_read),
                clusters_used: (cluster - first_cluster + 1) as usize,
                reason: "contiguous fallback hit EOF before requested size".to_string(),
            });
        }
        output.extend_from_slice(&buf);
        remaining -= bytes_to_read as u64;
        cluster += 1;
    }

    let structurally_complete = needed_clusters <= 1;
    Ok(Fat32EntryReadReport {
        data: output,
        mode: Fat32EntryReadMode::ContiguousFallback,
        complete: structurally_complete,
        bytes_requested: data_len,
        bytes_read: data_len,
        bytes_lost: 0,
        clusters_used: needed_clusters as usize,
        reason: if structurally_complete {
            "single-cluster contiguous fallback; fragmentation risk is low".to_string()
        } else {
            "multi-cluster contiguous fallback; fragmentation risk is unknown, quarantine as suspect".to_string()
        },
    })
}

pub fn read_entry_data<S: BlockSource + ?Sized>(
    source: &S,
    boot: &Fat32BootSector,
    volume_offset: u64,
    first_cluster: u32,
    data_len: u64,
    contiguous_fallback: bool,
) -> Result<Vec<u8>, Fat32Error> {
    Ok(read_entry_data_with_report(
        source,
        boot,
        volume_offset,
        first_cluster,
        data_len,
        contiguous_fallback,
    )?
    .data)
}

pub fn parse_directory_entries(buf: &[u8], base_offset: u64) -> Vec<Fat32Entry> {
    let mut entries = Vec::new();
    let mut lfn_parts: Vec<(u8, Vec<u16>)> = Vec::new();
    // Deleted LFN entries lose their sequence byte (overwritten with 0xE5)
    // but keep their characters and short-name checksum. They are stored in
    // reverse order on disk, so keep encounter order and reverse later.
    let mut deleted_lfn: Vec<(u8, Vec<u16>)> = Vec::new();
    let mut i = 0;
    while i + 32 <= buf.len() {
        let entry_offset = base_offset + i as u64;
        let entry = &buf[i..i + 32];
        let first = entry[0];
        if first == 0x00 {
            break;
        }
        let attr = entry[11];
        if attr == 0x0F {
            if first != 0xE5 {
                lfn_parts.push((first & 0x1F, parse_lfn_chars(entry)));
                deleted_lfn.clear();
            } else {
                deleted_lfn.push((entry[13], parse_lfn_chars(entry)));
                lfn_parts.clear();
            }
            i += 32;
            continue;
        }
        if attr & 0x08 != 0 {
            lfn_parts.clear();
            deleted_lfn.clear();
            i += 32;
            continue;
        }

        let is_deleted = first == 0xE5;
        let is_directory = (attr & 0x10) != 0;
        let high = u16::from_le_bytes([entry[20], entry[21]]) as u32;
        let low = u16::from_le_bytes([entry[26], entry[27]]) as u32;
        let first_cluster = (high << 16) | low;
        let size = u32::from_le_bytes([entry[28], entry[29], entry[30], entry[31]]) as u64;
        let short_name = parse_short_name(entry, is_deleted);
        let lfn = if is_deleted {
            deleted_lfn_name(&deleted_lfn, entry)
        } else {
            lfn_name(&lfn_parts)
        };
        let name = lfn.unwrap_or_else(|| short_name.clone());
        lfn_parts.clear();
        deleted_lfn.clear();

        if name != "." && name != ".." {
            entries.push(Fat32Entry {
                is_deleted,
                is_directory,
                name,
                short_name,
                size,
                first_cluster,
                file_offset: entry_offset,
                parent_offset: None,
                created_time: Some(
                    u32::from_le_bytes([entry[14], entry[15], entry[16], entry[17]]) as u64,
                ),
                modified_time: Some(
                    u32::from_le_bytes([entry[22], entry[23], entry[24], entry[25]]) as u64,
                ),
                accessed_time: Some(u16::from_le_bytes([entry[18], entry[19]]) as u64),
            });
        }
        i += 32;
    }
    entries
}

pub fn list_directory<S: BlockSource + ?Sized>(
    source: &S,
    boot: &Fat32BootSector,
    volume_offset: u64,
    first_cluster: u32,
) -> Result<Vec<Fat32Entry>, Fat32Error> {
    let cluster_size = boot.cluster_size() as usize;
    let chain = read_fat_chain(source, boot, volume_offset, first_cluster)?;
    let mut buf = Vec::new();
    for cluster in chain {
        let Some(cluster_offset) = boot.cluster_to_byte_offset(cluster) else {
            break;
        };
        let mut cluster_buf = vec![0_u8; cluster_size];
        let res = source.read_at(volume_offset + cluster_offset, &mut cluster_buf)?;
        if res.bytes_read == 0 {
            break;
        }
        let hit_end = cluster_buf.chunks_exact(32).any(|chunk| chunk[0] == 0x00);
        buf.extend_from_slice(&cluster_buf);
        if hit_end {
            break;
        }
    }
    let base_offset = volume_offset + boot.cluster_to_byte_offset(first_cluster).unwrap_or(0);
    Ok(parse_directory_entries(&buf, base_offset))
}

pub fn scan_fat32_vfs<S: BlockSource + ?Sized>(
    source: &S,
    boot: &Fat32BootSector,
    volume_offset: u64,
) -> Result<Vec<Fat32Entry>, Fat32Error> {
    let mut all_entries = Vec::new();
    let mut queue = VecDeque::new();
    let root_entries = list_directory(source, boot, volume_offset, boot.root_cluster)?;
    for mut entry in root_entries {
        // Root-level entries have no parent in the VFS tree. Using the root cluster
        // byte offset as parent_offset would alias with the first entry's file_offset,
        // creating a self-reference cycle in VfsTree::path_for.
        entry.parent_offset = None;
        if entry.is_directory && !entry.is_deleted && entry.first_cluster >= 2 {
            queue.push_back((entry.first_cluster, entry.file_offset));
        }
        all_entries.push(entry);
    }

    let mut dirs_visited = 0;
    while let Some((cluster, parent_offset)) = queue.pop_front() {
        if dirs_visited > 2000 {
            break;
        }
        dirs_visited += 1;
        if let Ok(sub_entries) = list_directory(source, boot, volume_offset, cluster) {
            for mut entry in sub_entries {
                entry.parent_offset = Some(parent_offset);
                if entry.is_directory && !entry.is_deleted && entry.first_cluster >= 2 {
                    queue.push_back((entry.first_cluster, entry.file_offset));
                }
                all_entries.push(entry);
            }
        }
    }
    Ok(all_entries)
}

fn ascii_trim(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).trim().to_string()
}

fn parse_lfn_chars(entry: &[u8]) -> Vec<u16> {
    let mut chars = Vec::new();
    for &idx in &[1usize, 3, 5, 7, 9, 14, 16, 18, 20, 22, 24, 28, 30] {
        let c = u16::from_le_bytes([entry[idx], entry[idx + 1]]);
        if c == 0x0000 || c == 0xFFFF {
            continue;
        }
        chars.push(c);
    }
    chars
}

/// Rebuild a deleted long file name. Every fragment carries the checksum of
/// its short name, but deletion overwrote the short name's first byte with
/// 0xE5 and any checksum is reachable by some first byte. So the fragments
/// are only accepted when the checksum matches with the first byte that the
/// long name implies (its first character upper-cased), or when the long
/// name starts with a non-ASCII character whose 8.3 alias is unpredictable.
fn deleted_lfn_name(parts: &[(u8, Vec<u16>)], short_entry: &[u8]) -> Option<String> {
    let checksum = parts.first()?.0;
    if parts.iter().any(|(sum, _)| *sum != checksum) {
        return None;
    }
    let chars = parts
        .iter()
        .rev()
        .flat_map(|(_, part)| part.iter().copied())
        .collect::<Vec<_>>();
    let name = String::from_utf16_lossy(&chars)
        .trim_end_matches('\u{0}')
        .to_string();
    let first = name.chars().find(|c| *c != '.')?;
    if first.is_ascii() {
        let mut short = [0_u8; 11];
        short.copy_from_slice(short_entry.get(0..11)?);
        short[0] = first.to_ascii_uppercase() as u8;
        if short_name_checksum(&short) != checksum {
            return None;
        }
    }
    Some(name).filter(|name| !name.trim().is_empty())
}

fn short_name_checksum(short: &[u8; 11]) -> u8 {
    short
        .iter()
        .fold(0_u8, |sum, byte| sum.rotate_right(1).wrapping_add(*byte))
}

fn lfn_name(parts: &[(u8, Vec<u16>)]) -> Option<String> {
    if parts.is_empty() {
        return None;
    }
    let mut sorted = parts.to_vec();
    sorted.sort_by_key(|(seq, _)| *seq);
    let chars = sorted
        .into_iter()
        .flat_map(|(_, part)| part)
        .collect::<Vec<_>>();
    Some(
        String::from_utf16_lossy(&chars)
            .trim_end_matches('\u{0}')
            .to_string(),
    )
    .filter(|s| !s.trim().is_empty())
}

fn parse_short_name(entry: &[u8], is_deleted: bool) -> String {
    let mut raw = entry[0..11].to_vec();
    if is_deleted && !raw.is_empty() {
        raw[0] = b'_';
    }
    if !is_deleted && raw[0] == 0x05 {
        // 0x05 stands in for a leading 0xE5 byte in the stored short name.
        raw[0] = 0xE5;
    }
    let mut base = String::from_utf8_lossy(&raw[0..8]).trim().to_string();
    let mut ext = String::from_utf8_lossy(&raw[8..11]).trim().to_string();
    // Windows NT / macOS store all-lowercase 8.3 names in uppercase and set
    // these reserved-byte flags instead of writing a long-name entry.
    let case_flags = entry.get(12).copied().unwrap_or(0);
    if case_flags & 0x08 != 0 {
        base = base.to_ascii_lowercase();
    }
    if case_flags & 0x10 != 0 {
        ext = ext.to_ascii_lowercase();
    }
    let mut out = if ext.is_empty() {
        base
    } else {
        format!("{base}.{ext}")
    };
    if out.is_empty() {
        out = "unknown".to_string();
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lfn_entry(sequence: u8, checksum: u8, chars: &[u16]) -> [u8; 32] {
        let mut entry = [0xff_u8; 32];
        entry[0] = sequence;
        entry[11] = 0x0F;
        entry[12] = 0;
        entry[13] = checksum;
        entry[26] = 0;
        entry[27] = 0;
        let slots = [1usize, 3, 5, 7, 9, 14, 16, 18, 20, 22, 24, 28, 30];
        for (index, slot) in slots.iter().enumerate() {
            let value = match index.cmp(&chars.len()) {
                std::cmp::Ordering::Less => chars[index],
                std::cmp::Ordering::Equal => 0,
                std::cmp::Ordering::Greater => 0xffff,
            };
            entry[*slot..*slot + 2].copy_from_slice(&value.to_le_bytes());
        }
        entry
    }

    fn deleted_lfn_directory(corrupt_checksum: bool) -> Vec<u8> {
        let mut short = [b' '; 32];
        short[0..11].copy_from_slice(b"GONE_S~1TXT");
        let mut name11 = [0_u8; 11];
        name11.copy_from_slice(&short[0..11]);
        let mut checksum = short_name_checksum(&name11);
        if corrupt_checksum {
            checksum = checksum.wrapping_add(1);
        }
        short[0] = 0xE5;
        short[11] = 0x20;
        short[12] = 0;
        short[26..28].copy_from_slice(&5_u16.to_le_bytes());
        short[28..32].copy_from_slice(&760_u32.to_le_bytes());
        let name = "gone_small.txt".encode_utf16().collect::<Vec<_>>();
        let mut buf = Vec::new();
        // On disk the last fragment comes first; deletion overwrites the
        // sequence byte of every fragment with 0xE5.
        buf.extend_from_slice(&lfn_entry(0xE5, checksum, &name[13..]));
        buf.extend_from_slice(&lfn_entry(0xE5, checksum, &name[..13]));
        buf.extend_from_slice(&short);
        buf.extend_from_slice(&[0_u8; 32]);
        buf
    }

    #[test]
    fn recovers_deleted_long_file_name() {
        let entries = parse_directory_entries(&deleted_lfn_directory(false), 0);
        assert_eq!(entries.len(), 1);
        assert!(entries[0].is_deleted);
        assert_eq!(entries[0].name, "gone_small.txt");
        assert_eq!(entries[0].size, 760);
    }

    #[test]
    fn deleted_long_name_with_wrong_checksum_falls_back_to_short_name() {
        let entries = parse_directory_entries(&deleted_lfn_directory(true), 0);
        assert_eq!(entries.len(), 1);
        assert!(entries[0].is_deleted);
        assert_eq!(entries[0].name, "_ONE_S~1.TXT");
    }

    #[test]
    fn short_name_honours_nt_lowercase_flags() {
        let mut entry = [b' '; 32];
        entry[0..4].copy_from_slice(b"DOCS");
        entry[11] = 0x10;
        entry[12] = 0x08;
        assert_eq!(parse_short_name(&entry, false), "docs");

        let mut entry = [b' '; 32];
        entry[0..6].copy_from_slice(b"README");
        entry[8..11].copy_from_slice(b"TXT");
        entry[12] = 0x18;
        assert_eq!(parse_short_name(&entry, false), "readme.txt");

        entry[12] = 0x10;
        assert_eq!(parse_short_name(&entry, false), "README.txt");

        entry[12] = 0;
        assert_eq!(parse_short_name(&entry, false), "README.TXT");
    }

    #[test]
    fn parses_short_directory_entry() {
        let mut buf = vec![0_u8; 64];
        buf[0..11].copy_from_slice(b"HELLO   TXT");
        buf[11] = 0x20;
        buf[26..28].copy_from_slice(&5_u16.to_le_bytes());
        buf[28..32].copy_from_slice(&123_u32.to_le_bytes());
        buf[32] = 0x00;
        let entries = parse_directory_entries(&buf, 4096);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].name, "HELLO.TXT");
        assert_eq!(entries[0].first_cluster, 5);
        assert_eq!(entries[0].size, 123);
    }
}
