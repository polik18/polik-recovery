use data_recovery_core::{BlockSource, ReadError};

const SECTOR_SIZE: u64 = 512;

#[derive(Debug)]
pub enum PartitionError {
    Read(ReadError),
    InvalidSectorSize(usize),
    InvalidMbrSignature,
    InvalidGptSignature,
    InvalidGptHeader { lba: u64, reason: &'static str },
    InvalidGptPartitions { lba: u64, reason: &'static str },
    OffsetOverflow,
}

impl std::fmt::Display for PartitionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Read(error) => write!(f, "{error}"),
            Self::InvalidSectorSize(size) => write!(f, "unsupported logical sector size: {size}"),
            Self::InvalidMbrSignature => write!(f, "invalid MBR signature"),
            Self::InvalidGptSignature => write!(f, "invalid GPT header signature"),
            Self::InvalidGptHeader { lba, reason } => {
                write!(f, "invalid GPT header at LBA {lba}: {reason}")
            }
            Self::InvalidGptPartitions { lba, reason } => {
                write!(f, "invalid GPT partition array at LBA {lba}: {reason}")
            }
            Self::OffsetOverflow => write!(f, "partition offset calculation overflow"),
        }
    }
}

impl std::error::Error for PartitionError {}

impl From<ReadError> for PartitionError {
    fn from(value: ReadError) -> Self {
        Self::Read(value)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PartitionTableKind {
    Mbr,
    Gpt,
    /// No partition table: the image starts directly with a filesystem
    /// (common for USB sticks, SD cards and `hdiutil -layout NONE` images).
    Volume,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PartitionEntry {
    pub index: u32,
    pub offset: u64,
    pub length: u64,
    pub type_code: Option<u8>,
    pub type_guid: Option<String>,
    pub filesystem: Option<String>,
    pub name: Option<String>,
    pub bootable: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PartitionTable {
    pub kind: PartitionTableKind,
    pub partitions: Vec<PartitionEntry>,
    pub used_backup_header: bool,
}

pub fn detect<S: BlockSource + ?Sized>(source: &S) -> Result<PartitionTable, PartitionError> {
    let mbr = read_sector(source, 0)?;
    // A FAT32 volume boot record also ends in 0x55AA, so check for a bare
    // filesystem before interpreting sector 0 as an MBR.
    if let Some(fs) = filesystem_signature(&mbr) {
        return Ok(PartitionTable {
            kind: PartitionTableKind::Volume,
            partitions: vec![PartitionEntry {
                index: 1,
                offset: 0,
                length: source.size_bytes(),
                type_code: None,
                type_guid: None,
                filesystem: Some(fs.to_string()),
                name: None,
                bootable: false,
            }],
            used_backup_header: false,
        });
    }
    if mbr[510..512] != [0x55, 0xaa] {
        return Err(PartitionError::InvalidMbrSignature);
    }

    let protective = (0..4).any(|index| mbr[446 + index * 16 + 4] == 0xee);
    if protective {
        return parse_gpt(source);
    }

    Ok(PartitionTable {
        kind: PartitionTableKind::Mbr,
        partitions: parse_mbr_partitions(source, &mbr)?,
        used_backup_header: false,
    })
}

fn parse_mbr_partitions<S: BlockSource + ?Sized>(
    source: &S,
    mbr: &[u8; 512],
) -> Result<Vec<PartitionEntry>, PartitionError> {
    let mut partitions = Vec::new();
    for index in 0..4 {
        let offset = 446 + index * 16;
        let type_code = mbr[offset + 4];
        let start_lba = le_u32(mbr, offset + 8) as u64;
        let sectors = le_u32(mbr, offset + 12) as u64;
        if type_code == 0 || sectors == 0 {
            continue;
        }
        let byte_offset = start_lba
            .checked_mul(SECTOR_SIZE)
            .ok_or(PartitionError::OffsetOverflow)?;
        let length = sectors
            .checked_mul(SECTOR_SIZE)
            .ok_or(PartitionError::OffsetOverflow)?;
        partitions.push(PartitionEntry {
            index: index as u32 + 1,
            offset: byte_offset,
            length,
            type_code: Some(type_code),
            type_guid: None,
            filesystem: probe_filesystem(source, byte_offset, type_code)?,
            name: None,
            bootable: mbr[offset] == 0x80,
        });
    }
    Ok(partitions)
}

fn parse_gpt<S: BlockSource + ?Sized>(source: &S) -> Result<PartitionTable, PartitionError> {
    let primary = read_gpt_header(source, 1);
    let (header, used_backup_header) = match primary {
        Ok(header) => (header, false),
        Err(primary_error) => {
            let backup_lba = source.device().size_bytes.saturating_sub(SECTOR_SIZE) / SECTOR_SIZE;
            let header = read_gpt_header(source, backup_lba).map_err(|_| primary_error)?;
            (header, true)
        }
    };

    let partitions = read_gpt_partitions(source, &header)?;
    Ok(PartitionTable {
        kind: PartitionTableKind::Gpt,
        partitions,
        used_backup_header,
    })
}

struct GptHeader {
    partition_entries_lba: u64,
    partition_count: u32,
    partition_size: u32,
    partition_array_crc32: u32,
}

fn read_gpt_header<S: BlockSource + ?Sized>(
    source: &S,
    lba: u64,
) -> Result<GptHeader, PartitionError> {
    let sector = read_sector(source, lba)?;
    if &sector[0..8] != b"EFI PART" {
        return Err(PartitionError::InvalidGptHeader {
            lba,
            reason: "signature",
        });
    }
    let header_size = le_u32(&sector, 12) as usize;
    if !(92..=512).contains(&header_size) {
        return Err(PartitionError::InvalidGptHeader {
            lba,
            reason: "header size",
        });
    }
    let expected_crc = le_u32(&sector, 16);
    let mut crc_data = sector[..header_size].to_vec();
    crc_data[16..20].fill(0);
    if crc32(&crc_data) != expected_crc {
        return Err(PartitionError::InvalidGptHeader {
            lba,
            reason: "header CRC32",
        });
    }
    let current_lba = le_u64(&sector, 24);
    let partition_entries_lba = le_u64(&sector, 72);
    let partition_count = le_u32(&sector, 80);
    let partition_size = le_u32(&sector, 84);
    if current_lba != lba || partition_size < 128 || partition_size % 8 != 0 || partition_count == 0
    {
        return Err(PartitionError::InvalidGptHeader {
            lba,
            reason: "field range",
        });
    }
    Ok(GptHeader {
        partition_entries_lba,
        partition_count,
        partition_size,
        partition_array_crc32: le_u32(&sector, 88),
    })
}

fn read_gpt_partitions<S: BlockSource + ?Sized>(
    source: &S,
    header: &GptHeader,
) -> Result<Vec<PartitionEntry>, PartitionError> {
    let bytes = (header.partition_count as u64)
        .checked_mul(header.partition_size as u64)
        .ok_or(PartitionError::OffsetOverflow)?;
    let mut data = vec![0_u8; bytes as usize];
    let offset = header
        .partition_entries_lba
        .checked_mul(SECTOR_SIZE)
        .ok_or(PartitionError::OffsetOverflow)?;
    let read = source.read_at(offset, &mut data)?;
    if read.bytes_read != data.len() || crc32(&data) != header.partition_array_crc32 {
        return Err(PartitionError::InvalidGptPartitions {
            lba: header.partition_entries_lba,
            reason: "array CRC32",
        });
    }

    let mut partitions = Vec::new();
    for index in 0..header.partition_count as usize {
        let start = index * header.partition_size as usize;
        let entry = &data[start..start + header.partition_size as usize];
        if entry[..16].iter().all(|byte| *byte == 0) {
            continue;
        }
        let first_lba = le_u64(entry, 32);
        let last_lba = le_u64(entry, 40);
        if last_lba < first_lba {
            continue;
        }
        let offset = first_lba
            .checked_mul(SECTOR_SIZE)
            .ok_or(PartitionError::OffsetOverflow)?;
        let length = (last_lba - first_lba + 1)
            .checked_mul(SECTOR_SIZE)
            .ok_or(PartitionError::OffsetOverflow)?;
        let type_guid = guid_string(&entry[..16]);
        let name = utf16_name(&entry[56..128]);
        partitions.push(PartitionEntry {
            index: index as u32 + 1,
            offset,
            length,
            type_code: None,
            filesystem: probe_filesystem(source, offset, 0)?,
            type_guid: Some(type_guid),
            name,
            bootable: false,
        });
    }
    Ok(partitions)
}

fn read_sector<S: BlockSource + ?Sized>(source: &S, lba: u64) -> Result<[u8; 512], PartitionError> {
    let offset = lba
        .checked_mul(SECTOR_SIZE)
        .ok_or(PartitionError::OffsetOverflow)?;
    let mut sector = [0_u8; 512];
    let read = source.read_at(offset, &mut sector)?;
    if read.bytes_read != sector.len() {
        return Err(PartitionError::Read(ReadError::Io(std::io::Error::new(
            std::io::ErrorKind::UnexpectedEof,
            "short partition-table sector read",
        ))));
    }
    Ok(sector)
}

fn probe_filesystem<S: BlockSource + ?Sized>(
    source: &S,
    offset: u64,
    type_code: u8,
) -> Result<Option<String>, PartitionError> {
    let mut boot = [0_u8; 512];
    let read = source.read_at(offset, &mut boot)?;
    if read.bytes_read < 90 {
        return Ok(Some(mbr_type_name(type_code).to_string()).filter(|name| name != "Unknown"));
    }
    let fs = filesystem_signature(&boot).or_else(|| {
        let name = mbr_type_name(type_code);
        (name != "Unknown").then_some(name)
    });
    Ok(fs.map(str::to_string))
}

fn filesystem_signature(boot: &[u8; 512]) -> Option<&'static str> {
    if &boot[3..11] == b"NTFS    " {
        Some("NTFS")
    } else if &boot[3..11] == b"EXFAT   " {
        Some("exFAT")
    } else if &boot[82..90] == b"FAT32   " && boot[510..512] == [0x55, 0xaa] {
        Some("FAT32")
    } else if &boot[32..36] == b"NXSB" {
        Some("APFS")
    } else {
        None
    }
}

fn mbr_type_name(type_code: u8) -> &'static str {
    match type_code {
        0x07 => "NTFS/exFAT",
        0x0b | 0x0c => "FAT32",
        0xaf => "HFS+",
        _ => "Unknown",
    }
}

fn guid_string(bytes: &[u8]) -> String {
    format!(
        "{:08x}-{:04x}-{:04x}-{}-{}",
        u32::from_le_bytes(bytes[0..4].try_into().unwrap()),
        u16::from_le_bytes(bytes[4..6].try_into().unwrap()),
        u16::from_le_bytes(bytes[6..8].try_into().unwrap()),
        hex_bytes(&bytes[8..10]),
        hex_bytes(&bytes[10..16])
    )
}

fn hex_bytes(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        write!(&mut output, "{byte:02x}").expect("writing to String cannot fail");
    }
    output
}

fn utf16_name(bytes: &[u8]) -> Option<String> {
    let end = bytes
        .chunks_exact(2)
        .position(|pair| pair == [0, 0])
        .unwrap_or(bytes.len() / 2);
    let units = bytes[..end * 2]
        .chunks_exact(2)
        .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
        .collect::<Vec<_>>();
    (!units.is_empty()).then(|| String::from_utf16_lossy(&units))
}

fn le_u32(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap())
}

fn le_u64(bytes: &[u8], offset: usize) -> u64 {
    u64::from_le_bytes(bytes[offset..offset + 8].try_into().unwrap())
}

fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = 0xffff_ffff_u32;
    for byte in bytes {
        crc ^= u32::from(*byte);
        for _ in 0..8 {
            crc = if crc & 1 != 0 {
                (crc >> 1) ^ 0xedb8_8320
            } else {
                crc >> 1
            };
        }
    }
    !crc
}

#[cfg(test)]
mod tests {
    use super::*;
    use data_recovery_core::{BusType, PhysicalDevice, ReadResult};

    struct MemorySource {
        data: Vec<u8>,
        device: PhysicalDevice,
    }

    impl MemorySource {
        fn new(data: Vec<u8>) -> Self {
            Self {
                device: PhysicalDevice {
                    id: "partition-test".to_string(),
                    display_name: "partition-test".to_string(),
                    platform_path: "fixture".to_string(),
                    size_bytes: data.len() as u64,
                    logical_sector_size: 512,
                    physical_sector_size: 512,
                    is_removable: false,
                    is_system_disk: false,
                    bus_type: BusType::Unknown,
                    trim_supported: None,
                },
                data,
            }
        }
    }

    impl BlockSource for MemorySource {
        fn device(&self) -> &PhysicalDevice {
            &self.device
        }
        fn read_at(&self, offset: u64, buffer: &mut [u8]) -> Result<ReadResult, ReadError> {
            let start = offset as usize;
            if start >= self.data.len() {
                return Ok(ReadResult {
                    offset,
                    bytes_read: 0,
                });
            }
            let count = buffer.len().min(self.data.len() - start);
            buffer[..count].copy_from_slice(&self.data[start..start + count]);
            Ok(ReadResult {
                offset,
                bytes_read: count,
            })
        }
    }

    #[test]
    fn parses_mbr_partition_and_probes_ntfs() {
        let mut image = vec![0_u8; 64 * 512];
        image[446 + 4] = 0x07;
        image[446 + 8..446 + 12].copy_from_slice(&8_u32.to_le_bytes());
        image[446 + 12..446 + 16].copy_from_slice(&32_u32.to_le_bytes());
        image[8 * 512 + 3..8 * 512 + 11].copy_from_slice(b"NTFS    ");
        image[510..512].copy_from_slice(&[0x55, 0xaa]);
        let table = detect(&MemorySource::new(image)).unwrap();
        assert_eq!(table.kind, PartitionTableKind::Mbr);
        assert_eq!(table.partitions[0].offset, 4096);
        assert_eq!(table.partitions[0].filesystem.as_deref(), Some("NTFS"));
    }

    #[test]
    fn falls_back_to_backup_gpt_after_primary_crc_failure() {
        let sector_count = 128_u64;
        let mut image = vec![0_u8; sector_count as usize * 512];
        image[446 + 4] = 0xee;
        image[446 + 8..446 + 12].copy_from_slice(&1_u32.to_le_bytes());
        image[446 + 12..446 + 16].copy_from_slice(&126_u32.to_le_bytes());
        image[510..512].copy_from_slice(&[0x55, 0xaa]);

        let mut entries = vec![0_u8; 512];
        entries[0..16].copy_from_slice(&[1; 16]);
        entries[32..40].copy_from_slice(&34_u64.to_le_bytes());
        entries[40..48].copy_from_slice(&60_u64.to_le_bytes());
        entries[56..64].copy_from_slice(&[b'T', 0, b'e', 0, b's', 0, b't', 0]);
        image[2 * 512..3 * 512].copy_from_slice(&entries);
        image[125 * 512..126 * 512].copy_from_slice(&entries);

        write_gpt_header(&mut image, 1, 127, 2, crc32(&entries));
        write_gpt_header(&mut image, 127, 1, 125, crc32(&entries));
        image[512 + 20] ^= 1;
        image[34 * 512 + 3..34 * 512 + 11].copy_from_slice(b"NTFS    ");

        let table = detect(&MemorySource::new(image)).unwrap();
        assert_eq!(table.kind, PartitionTableKind::Gpt);
        assert!(table.used_backup_header);
        assert_eq!(table.partitions[0].offset, 34 * 512);
        assert_eq!(table.partitions[0].name.as_deref(), Some("Test"));
    }

    fn write_gpt_header(
        image: &mut [u8],
        current_lba: u64,
        backup_lba: u64,
        entries_lba: u64,
        entries_crc: u32,
    ) {
        let start = current_lba as usize * 512;
        let header = &mut image[start..start + 512];
        header[0..8].copy_from_slice(b"EFI PART");
        header[8..12].copy_from_slice(&0x0001_0000_u32.to_le_bytes());
        header[12..16].copy_from_slice(&92_u32.to_le_bytes());
        header[24..32].copy_from_slice(&current_lba.to_le_bytes());
        header[32..40].copy_from_slice(&backup_lba.to_le_bytes());
        header[40..48].copy_from_slice(&34_u64.to_le_bytes());
        header[48..56].copy_from_slice(&120_u64.to_le_bytes());
        header[56..72].copy_from_slice(&[2; 16]);
        header[72..80].copy_from_slice(&entries_lba.to_le_bytes());
        header[80..84].copy_from_slice(&4_u32.to_le_bytes());
        header[84..88].copy_from_slice(&128_u32.to_le_bytes());
        header[88..92].copy_from_slice(&entries_crc.to_le_bytes());
        header[16..20].fill(0);
        let header_crc = crc32(&header[..92]);
        header[16..20].copy_from_slice(&header_crc.to_le_bytes());
    }

    #[test]
    fn detects_bare_fat32_volume_instead_of_empty_mbr() {
        let mut image = vec![0_u8; 4096];
        image[82..90].copy_from_slice(b"FAT32   ");
        image[510] = 0x55;
        image[511] = 0xaa;
        let table = detect(&MemorySource::new(image)).unwrap();
        assert_eq!(table.kind, PartitionTableKind::Volume);
        assert_eq!(table.partitions.len(), 1);
        assert_eq!(table.partitions[0].offset, 0);
        assert_eq!(table.partitions[0].length, 4096);
        assert_eq!(table.partitions[0].filesystem.as_deref(), Some("FAT32"));
    }

    #[test]
    fn detects_bare_exfat_and_apfs_volumes() {
        let mut exfat = vec![0_u8; 1024];
        exfat[3..11].copy_from_slice(b"EXFAT   ");
        exfat[510] = 0x55;
        exfat[511] = 0xaa;
        let table = detect(&MemorySource::new(exfat)).unwrap();
        assert_eq!(table.kind, PartitionTableKind::Volume);
        assert_eq!(table.partitions[0].filesystem.as_deref(), Some("exFAT"));

        let mut apfs = vec![0_u8; 4096];
        apfs[32..36].copy_from_slice(b"NXSB");
        let table = detect(&MemorySource::new(apfs)).unwrap();
        assert_eq!(table.kind, PartitionTableKind::Volume);
        assert_eq!(table.partitions[0].filesystem.as_deref(), Some("APFS"));
    }

    #[test]
    fn rejects_mbr_without_signature() {
        let mut image = vec![0_u8; 512];
        image[0] = 1;
        assert!(matches!(
            detect(&MemorySource::new(image)),
            Err(PartitionError::InvalidMbrSignature)
        ));
    }
}
