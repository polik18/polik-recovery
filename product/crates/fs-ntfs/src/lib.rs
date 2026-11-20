use data_recovery_core::{BlockSource, ReadError};

pub const NTFS_BOOT_SECTOR_SIZE: usize = 512;
const NTFS_OEM_ID: &[u8; 8] = b"NTFS    ";

#[derive(Debug)]
pub enum NtfsError {
    Read(ReadError),
    BufferTooSmall {
        actual: usize,
        minimum: usize,
    },
    InvalidJumpInstruction([u8; 3]),
    InvalidOemId([u8; 8]),
    InvalidMftRecordSignature([u8; 4]),
    InvalidBytesPerSector(u16),
    InvalidSectorsPerCluster(u8),
    InvalidBootSignature(u16),
    InvalidFileRecordSize(usize),
    InvalidAttribute {
        offset: usize,
        record_size: usize,
    },
    InvalidDataRun {
        offset: usize,
    },
    InvalidUtf16Name,
    InvalidFixupArray {
        usa_offset: u16,
        usa_count: u16,
        record_size: usize,
    },
    FixupMismatch {
        sector_index: usize,
        expected: u16,
        actual: u16,
    },
    OffsetOverflow,
    UnsupportedRecordSize(i8),
    EncryptedNotSupported,
    DecompressionFailed,
}

impl std::fmt::Display for NtfsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Read(err) => write!(f, "{err}"),
            Self::BufferTooSmall { actual, minimum } => {
                write!(f, "NTFS boot sector requires {minimum} bytes, got {actual}")
            }
            Self::InvalidJumpInstruction(bytes) => {
                write!(f, "invalid NTFS jump instruction: {bytes:02x?}")
            }
            Self::InvalidOemId(bytes) => write!(f, "invalid NTFS OEM id: {bytes:02x?}"),
            Self::InvalidMftRecordSignature(bytes) => {
                write!(f, "invalid NTFS MFT record signature: {bytes:02x?}")
            }
            Self::InvalidBytesPerSector(value) => {
                write!(f, "invalid NTFS bytes per sector: {value}")
            }
            Self::InvalidSectorsPerCluster(value) => {
                write!(f, "invalid NTFS sectors per cluster: {value}")
            }
            Self::InvalidBootSignature(value) => {
                write!(f, "invalid NTFS boot signature: 0x{value:04x}")
            }
            Self::InvalidFileRecordSize(value) => {
                write!(f, "invalid NTFS file record size: {value}")
            }
            Self::InvalidAttribute {
                offset,
                record_size,
            } => write!(
                f,
                "invalid NTFS attribute at offset {offset}, record_size={record_size}"
            ),
            Self::InvalidDataRun { offset } => {
                write!(f, "invalid NTFS data run at mapping pairs offset {offset}")
            }
            Self::InvalidUtf16Name => write!(f, "invalid UTF-16 file name in NTFS attribute"),
            Self::InvalidFixupArray {
                usa_offset,
                usa_count,
                record_size,
            } => write!(
                f,
                "invalid NTFS update sequence array: offset={usa_offset}, count={usa_count}, record_size={record_size}"
            ),
            Self::FixupMismatch {
                sector_index,
                expected,
                actual,
            } => write!(
                f,
                "NTFS fixup mismatch at sector {sector_index}: expected=0x{expected:04x}, actual=0x{actual:04x}"
            ),
            Self::OffsetOverflow => write!(f, "computed NTFS offset overflows u64"),
            Self::UnsupportedRecordSize(value) => {
                write!(f, "unsupported NTFS file record size code: {value}")
            }
            Self::EncryptedNotSupported => write!(f, "EFS 加密檔案目前不受跨平台支援"),
            Self::DecompressionFailed => write!(f, "NTFS LZNT1 解壓縮失敗"),
        }
    }
}

impl std::error::Error for NtfsError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Read(err) => Some(err),
            _ => None,
        }
    }
}

impl From<ReadError> for NtfsError {
    fn from(value: ReadError) -> Self {
        Self::Read(value)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NtfsBootSector {
    pub jump_instruction: [u8; 3],
    pub oem_id: [u8; 8],
    pub bytes_per_sector: u16,
    pub sectors_per_cluster: u8,
    pub media_descriptor: u8,
    pub sectors_per_track: u16,
    pub number_of_heads: u16,
    pub hidden_sectors: u32,
    pub total_sectors: u64,
    pub mft_lcn: u64,
    pub mft_mirror_lcn: u64,
    pub clusters_per_file_record_segment: i8,
    pub clusters_per_index_buffer: i8,
    pub volume_serial_number: u64,
    pub boot_signature: u16,
}

impl NtfsBootSector {
    pub fn parse(sector: &[u8]) -> Result<Self, NtfsError> {
        if sector.len() < NTFS_BOOT_SECTOR_SIZE {
            return Err(NtfsError::BufferTooSmall {
                actual: sector.len(),
                minimum: NTFS_BOOT_SECTOR_SIZE,
            });
        }

        let jump_instruction = read_array::<3>(sector, 0);
        if !matches!(jump_instruction[0], 0xeb | 0xe9) {
            return Err(NtfsError::InvalidJumpInstruction(jump_instruction));
        }

        let oem_id = read_array::<8>(sector, 3);
        if &oem_id != NTFS_OEM_ID {
            return Err(NtfsError::InvalidOemId(oem_id));
        }

        let bytes_per_sector = le_u16(sector, 11);
        if !matches!(bytes_per_sector, 512 | 1024 | 2048 | 4096) {
            return Err(NtfsError::InvalidBytesPerSector(bytes_per_sector));
        }

        let sectors_per_cluster = sector[13];
        if sectors_per_cluster == 0 || !sectors_per_cluster.is_power_of_two() {
            return Err(NtfsError::InvalidSectorsPerCluster(sectors_per_cluster));
        }

        let boot_signature = le_u16(sector, 510);
        if boot_signature != 0xaa55 {
            return Err(NtfsError::InvalidBootSignature(boot_signature));
        }

        Ok(Self {
            jump_instruction,
            oem_id,
            bytes_per_sector,
            sectors_per_cluster,
            media_descriptor: sector[21],
            sectors_per_track: le_u16(sector, 24),
            number_of_heads: le_u16(sector, 26),
            hidden_sectors: le_u32(sector, 28),
            total_sectors: le_u64(sector, 40),
            mft_lcn: le_u64(sector, 48),
            mft_mirror_lcn: le_u64(sector, 56),
            clusters_per_file_record_segment: sector[64] as i8,
            clusters_per_index_buffer: sector[68] as i8,
            volume_serial_number: le_u64(sector, 72),
            boot_signature,
        })
    }

    pub fn cluster_size(&self) -> u64 {
        self.bytes_per_sector as u64 * self.sectors_per_cluster as u64
    }

    pub fn mft_offset(&self) -> Result<u64, NtfsError> {
        self.mft_lcn
            .checked_mul(self.cluster_size())
            .ok_or(NtfsError::OffsetOverflow)
    }

    pub fn mft_mirror_offset(&self) -> Result<u64, NtfsError> {
        self.mft_mirror_lcn
            .checked_mul(self.cluster_size())
            .ok_or(NtfsError::OffsetOverflow)
    }

    pub fn file_record_size(&self) -> Result<u64, NtfsError> {
        decode_ntfs_size_code(self.clusters_per_file_record_segment, self.cluster_size())
    }

    pub fn index_buffer_size(&self) -> Result<u64, NtfsError> {
        decode_ntfs_size_code(self.clusters_per_index_buffer, self.cluster_size())
    }

    pub fn total_size_bytes(&self) -> Result<u64, NtfsError> {
        self.total_sectors
            .checked_mul(self.bytes_per_sector as u64)
            .ok_or(NtfsError::OffsetOverflow)
    }
}

pub fn read_boot_sector<S: BlockSource + ?Sized>(
    source: &S,
    volume_offset: u64,
) -> Result<NtfsBootSector, NtfsError> {
    let mut sector = [0_u8; NTFS_BOOT_SECTOR_SIZE];
    source.read_at(volume_offset, &mut sector)?;
    NtfsBootSector::parse(&sector)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MftRecord {
    pub header: MftRecordHeader,
    pub fixed_up_data: Vec<u8>,
    pub attributes: Vec<NtfsAttribute>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MftRecordHeader {
    pub signature: [u8; 4],
    pub usa_offset: u16,
    pub usa_count: u16,
    pub lsn: u64,
    pub sequence_number: u16,
    pub hard_link_count: u16,
    pub first_attribute_offset: u16,
    pub flags: MftRecordFlags,
    pub bytes_in_use: u32,
    pub bytes_allocated: u32,
    pub base_file_record: u64,
    pub next_attribute_id: u16,
    pub record_number: Option<u32>,
}

impl MftRecordHeader {
    pub fn is_in_use(&self) -> bool {
        self.flags.in_use
    }

    pub fn is_deleted(&self) -> bool {
        !self.flags.in_use
    }

    pub fn is_directory(&self) -> bool {
        self.flags.directory
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MftRecordFlags {
    pub raw: u16,
    pub in_use: bool,
    pub directory: bool,
}

impl MftRecordFlags {
    pub fn parse(raw: u16) -> Self {
        Self {
            raw,
            in_use: raw & 0x0001 != 0,
            directory: raw & 0x0002 != 0,
        }
    }
}

impl MftRecord {
    pub fn parse(record: &[u8], bytes_per_sector: u16) -> Result<Self, NtfsError> {
        if record.len() < 48 {
            return Err(NtfsError::BufferTooSmall {
                actual: record.len(),
                minimum: 48,
            });
        }

        if bytes_per_sector == 0 || record.len() % bytes_per_sector as usize != 0 {
            return Err(NtfsError::InvalidFileRecordSize(record.len()));
        }

        let signature = read_array::<4>(record, 0);
        if &signature != b"FILE" {
            return Err(NtfsError::InvalidMftRecordSignature(signature));
        }

        let usa_offset = le_u16(record, 4);
        let usa_count = le_u16(record, 6);
        let fixed_up_data =
            apply_update_sequence_array(record, bytes_per_sector as usize, usa_offset, usa_count)?;

        let header = MftRecordHeader {
            signature,
            usa_offset,
            usa_count,
            lsn: le_u64(&fixed_up_data, 8),
            sequence_number: le_u16(&fixed_up_data, 16),
            hard_link_count: le_u16(&fixed_up_data, 18),
            first_attribute_offset: le_u16(&fixed_up_data, 20),
            flags: MftRecordFlags::parse(le_u16(&fixed_up_data, 22)),
            bytes_in_use: le_u32(&fixed_up_data, 24),
            bytes_allocated: le_u32(&fixed_up_data, 28),
            base_file_record: le_u64(&fixed_up_data, 32),
            next_attribute_id: le_u16(&fixed_up_data, 40),
            record_number: (fixed_up_data.len() >= 48).then(|| le_u32(&fixed_up_data, 44)),
        };
        let attributes = parse_attributes(&fixed_up_data, header.first_attribute_offset as usize)?;

        Ok(Self {
            header,
            fixed_up_data,
            attributes,
        })
    }
}

pub fn read_mft_record<S: BlockSource + ?Sized>(
    source: &S,
    record_offset: u64,
    record_size: usize,
    bytes_per_sector: u16,
) -> Result<MftRecord, NtfsError> {
    if record_size == 0 {
        return Err(NtfsError::InvalidFileRecordSize(record_size));
    }

    let mut record = vec![0_u8; record_size];
    source.read_at(record_offset, &mut record)?;
    MftRecord::parse(&record, bytes_per_sector)
}

/// Physical mapping for the logical contents of the `$MFT` file.
///
/// NTFS may store `$MFT` in multiple non-contiguous data runs.  Callers should
/// build this map once per volume and reuse it for scans, extraction, and
/// previews instead of deriving a physical offset from the record index.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MftRunMap {
    volume_offset: u64,
    cluster_size: u64,
    record_size: usize,
    runs: Vec<DataRun>,
}

impl MftRunMap {
    pub fn from_record(
        boot: &NtfsBootSector,
        volume_offset: u64,
        record: &MftRecord,
    ) -> Result<Self, NtfsError> {
        let record_size = boot.file_record_size()?;
        let attribute = record.attributes.iter().find(|attribute| {
            attribute.header.attribute_type == AttributeType::Data
                && attribute.header.attribute_name.is_none()
        });
        let Some(attribute) = attribute else {
            return Err(NtfsError::InvalidDataRun { offset: 0 });
        };
        let AttributeValue::DataRuns(runs) = &attribute.value else {
            return Err(NtfsError::InvalidDataRun { offset: 0 });
        };
        if runs.is_empty() {
            return Err(NtfsError::InvalidDataRun { offset: 0 });
        }

        Ok(Self {
            volume_offset,
            cluster_size: boot.cluster_size(),
            record_size: usize::try_from(record_size)
                .map_err(|_| NtfsError::InvalidFileRecordSize(usize::MAX))?,
            runs: runs.clone(),
        })
    }

    fn contiguous(boot: &NtfsBootSector, volume_offset: u64) -> Result<Self, NtfsError> {
        let cluster_size = boot.cluster_size();
        let cluster_count = boot.total_sectors.div_ceil(boot.sectors_per_cluster as u64);
        Ok(Self {
            volume_offset,
            cluster_size,
            record_size: usize::try_from(boot.file_record_size()?)
                .map_err(|_| NtfsError::InvalidFileRecordSize(usize::MAX))?,
            runs: vec![DataRun {
                vcn_start: 0,
                cluster_count,
                lcn_start: Some(boot.mft_lcn as i64),
            }],
        })
    }

    pub fn record_size(&self) -> usize {
        self.record_size
    }

    pub fn runs(&self) -> &[DataRun] {
        &self.runs
    }

    pub fn read_record<S: BlockSource + ?Sized>(
        &self,
        source: &S,
        boot: &NtfsBootSector,
        index: u64,
    ) -> Result<MftRecord, NtfsError> {
        if index == 0 {
            let record_offset = self
                .volume_offset
                .checked_add(boot.mft_offset()?)
                .ok_or(NtfsError::OffsetOverflow)?;
            return read_mft_record(
                source,
                record_offset,
                self.record_size,
                boot.bytes_per_sector,
            );
        }
        let logical_offset = index
            .checked_mul(self.record_size as u64)
            .ok_or(NtfsError::OffsetOverflow)?;
        let mut record = vec![0_u8; self.record_size];
        self.read_logical(source, logical_offset, &mut record)?;
        MftRecord::parse(&record, boot.bytes_per_sector)
    }

    fn read_logical<S: BlockSource + ?Sized>(
        &self,
        source: &S,
        logical_offset: u64,
        buffer: &mut [u8],
    ) -> Result<(), NtfsError> {
        let mut cursor = logical_offset;
        let mut output_offset = 0usize;
        while output_offset < buffer.len() {
            let vcn = cursor / self.cluster_size;
            let offset_in_cluster = cursor % self.cluster_size;
            let run = self
                .runs
                .iter()
                .find(|run| {
                    vcn >= run.vcn_start && vcn < run.vcn_start.saturating_add(run.cluster_count)
                })
                .ok_or(NtfsError::InvalidDataRun {
                    offset: output_offset,
                })?;
            let run_end = run
                .vcn_start
                .saturating_add(run.cluster_count)
                .saturating_mul(self.cluster_size);
            let bytes_in_run = run_end
                .checked_sub(cursor)
                .ok_or(NtfsError::InvalidDataRun {
                    offset: output_offset,
                })?;
            let bytes_to_read = bytes_in_run.min((buffer.len() - output_offset) as u64) as usize;
            let target = &mut buffer[output_offset..output_offset + bytes_to_read];

            if let Some(lcn_start) = run.lcn_start {
                if lcn_start < 0 {
                    return Err(NtfsError::InvalidDataRun {
                        offset: output_offset,
                    });
                }
                let physical_offset = self
                    .volume_offset
                    .checked_add(
                        (lcn_start as u64)
                            .checked_add(vcn - run.vcn_start)
                            .and_then(|cluster| cluster.checked_mul(self.cluster_size))
                            .and_then(|offset| offset.checked_add(offset_in_cluster))
                            .ok_or(NtfsError::OffsetOverflow)?,
                    )
                    .ok_or(NtfsError::OffsetOverflow)?;
                let result = source.read_at(physical_offset, target)?;
                if result.bytes_read != target.len() {
                    return Err(NtfsError::Read(ReadError::Io(std::io::Error::new(
                        std::io::ErrorKind::UnexpectedEof,
                        "short read while reading NTFS MFT data run",
                    ))));
                }
            } else {
                target.fill(0);
            }

            cursor = cursor
                .checked_add(bytes_to_read as u64)
                .ok_or(NtfsError::OffsetOverflow)?;
            output_offset += bytes_to_read;
        }
        Ok(())
    }
}

/// Reads the `$MFT` record 0 using the boot-sector location and builds its
/// physical data-run map.  Very small legacy fixtures may omit record 0's
/// `$DATA` attribute; those use the historical contiguous fallback.
pub fn read_mft_run_map<S: BlockSource + ?Sized>(
    source: &S,
    boot: &NtfsBootSector,
    volume_offset: u64,
) -> Result<MftRunMap, NtfsError> {
    let mft_offset = volume_offset
        .checked_add(boot.mft_offset()?)
        .ok_or(NtfsError::OffsetOverflow)?;
    let record_size = usize::try_from(boot.file_record_size()?)
        .map_err(|_| NtfsError::InvalidFileRecordSize(usize::MAX))?;
    let record_zero = read_mft_record(source, mft_offset, record_size, boot.bytes_per_sector)?;
    let has_data_runlist = record_zero.attributes.iter().any(|attribute| {
        attribute.header.attribute_type == AttributeType::Data
            && attribute.header.attribute_name.is_none()
            && matches!(attribute.value, AttributeValue::DataRuns(_))
    });
    if has_data_runlist {
        MftRunMap::from_record(boot, volume_offset, &record_zero)
    } else {
        MftRunMap::contiguous(boot, volume_offset)
    }
}

pub fn read_mft_record_by_index_with_map<S: BlockSource + ?Sized>(
    source: &S,
    boot: &NtfsBootSector,
    map: &MftRunMap,
    index: u64,
) -> Result<MftRecord, NtfsError> {
    map.read_record(source, boot, index)
}

pub fn read_mft_records<S: BlockSource + ?Sized>(
    source: &S,
    boot: &NtfsBootSector,
    volume_offset: u64,
    count: usize,
) -> Result<Vec<MftRecord>, NtfsError> {
    let map = read_mft_run_map(source, boot, volume_offset)?;
    let mut records = Vec::with_capacity(count);

    for index in 0..count {
        records.push(map.read_record(source, boot, index as u64)?);
    }

    Ok(records)
}

/// Reads one MFT record by its record index without walking all earlier records.
/// This is critical for recovery UI usage: selecting record 180000 should not require
/// parsing 0..180000 and failing because an unrelated earlier record is damaged.
pub fn read_mft_record_by_index<S: BlockSource + ?Sized>(
    source: &S,
    boot: &NtfsBootSector,
    volume_offset: u64,
    index: u64,
) -> Result<MftRecord, NtfsError> {
    let map = read_mft_run_map(source, boot, volume_offset)?;
    read_mft_record_by_index_with_map(source, boot, &map, index)
}

/// Best-effort estimate of how many file records exist in the MFT.
/// It reads record #0 (the $MFT file) and uses its unnamed `$DATA` real size.
/// Some damaged volumes may not expose this value, so callers should fall back
/// to a conservative scan limit when this returns `None`.
pub fn estimate_mft_record_count<S: BlockSource + ?Sized>(
    source: &S,
    boot: &NtfsBootSector,
    volume_offset: u64,
) -> Result<Option<usize>, NtfsError> {
    let mft_record = read_mft_record_by_index(source, boot, volume_offset, 0)?;
    estimate_mft_record_count_from_record(boot, &mft_record)
}

pub fn estimate_mft_record_count_from_record(
    boot: &NtfsBootSector,
    mft_record: &MftRecord,
) -> Result<Option<usize>, NtfsError> {
    let record_size = boot.file_record_size()?;
    if record_size == 0 {
        return Ok(None);
    }

    let Some(stream) = list_data_streams(mft_record)
        .into_iter()
        .find(|stream| stream.name.is_none())
    else {
        return Ok(None);
    };
    let Some(mft_size) = stream.size else {
        return Ok(None);
    };

    if mft_size == 0 {
        return Ok(None);
    }

    let count = mft_size.div_ceil(record_size);
    Ok(Some(count.min(usize::MAX as u64) as usize))
}

// ── Data stream types ─────────────────────────────────────────────────────────

/// Describes a single `$DATA` attribute stream (named or unnamed) in an MFT record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataStreamInfo {
    /// `None` = default (unnamed) stream. `Some(name)` = alternate data stream.
    pub name: Option<String>,
    pub size: Option<u64>,
    pub is_compressed: bool,
    pub is_encrypted: bool,
    pub is_sparse: bool,
}

/// A contiguous file region that could not be read (bad sectors).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BadSectorRange {
    /// Byte offset within the logical file data.
    pub file_offset: u64,
    pub length: u64,
}

/// Result of a best-effort extraction that tolerates read errors.
#[derive(Debug)]
pub struct PartialRecovery {
    /// File bytes; unreadable regions are filled with zeroes.
    pub data: Vec<u8>,
    pub bad_sector_ranges: Vec<BadSectorRange>,
    pub bytes_recovered: u64,
    pub bytes_lost: u64,
    /// `true` when all data was read without any errors.
    pub complete: bool,
}

// ── Public extraction API ─────────────────────────────────────────────────────

/// Lists every `$DATA` stream present in the record (default + alternate streams).
pub fn list_data_streams(record: &MftRecord) -> Vec<DataStreamInfo> {
    record
        .attributes
        .iter()
        .filter(|attr| attr.header.attribute_type == AttributeType::Data)
        .map(|attr| {
            let size = match &attr.header.form {
                AttributeForm::Resident(r) => Some(r.value_length as u64),
                AttributeForm::NonResident(nr) => Some(nr.real_size),
            };
            DataStreamInfo {
                name: attr.header.attribute_name.clone(),
                size,
                is_compressed: attr.is_compressed(),
                is_encrypted: attr.is_encrypted(),
                is_sparse: attr.is_sparse(),
            }
        })
        .collect()
}

/// Extracts the data of the default (unnamed) `$DATA` stream.
pub fn extract_file_data<S: BlockSource + ?Sized>(
    source: &S,
    boot: &NtfsBootSector,
    volume_offset: u64,
    record: &MftRecord,
) -> Result<Vec<u8>, NtfsError> {
    extract_named_stream(source, boot, volume_offset, record, None)
}

/// Extracts the data of a named or unnamed `$DATA` stream.
/// Pass `None` to get the default stream; pass `Some("Zone.Identifier")` for ADS.
pub fn extract_named_stream<S: BlockSource + ?Sized>(
    source: &S,
    boot: &NtfsBootSector,
    volume_offset: u64,
    record: &MftRecord,
    stream_name: Option<&str>,
) -> Result<Vec<u8>, NtfsError> {
    let Some(attribute) = record.attributes.iter().find(|attr| {
        attr.header.attribute_type == AttributeType::Data
            && attr.header.attribute_name.as_deref() == stream_name
    }) else {
        return Ok(Vec::new());
    };
    extract_attribute_data(source, boot, volume_offset, attribute)
}

/// Best-effort extraction: reads as much data as possible, filling bad sectors with zeroes.
/// Never returns an error — use `PartialRecovery::complete` to check for bad sectors.
pub fn extract_file_data_partial<S: BlockSource + ?Sized>(
    source: &S,
    boot: &NtfsBootSector,
    volume_offset: u64,
    record: &MftRecord,
) -> PartialRecovery {
    let Some(attribute) = record.attributes.iter().find(|attr| {
        attr.header.attribute_type == AttributeType::Data && attr.header.attribute_name.is_none()
    }) else {
        return PartialRecovery {
            data: Vec::new(),
            bad_sector_ranges: Vec::new(),
            bytes_recovered: 0,
            bytes_lost: 0,
            complete: true,
        };
    };

    if attribute.is_encrypted() {
        let size = match &attribute.header.form {
            AttributeForm::Resident(_) => {
                if let AttributeValue::ResidentRaw(data) = &attribute.value {
                    data.len() as u64
                } else {
                    0
                }
            }
            AttributeForm::NonResident(nr) => nr.real_size,
        };
        return PartialRecovery {
            data: vec![0_u8; size as usize],
            bad_sector_ranges: vec![BadSectorRange {
                file_offset: 0,
                length: size,
            }],
            bytes_recovered: 0,
            bytes_lost: size,
            complete: false,
        };
    }

    match &attribute.header.form {
        AttributeForm::Resident(_) => {
            if let AttributeValue::ResidentRaw(data) = &attribute.value {
                let len = data.len() as u64;
                PartialRecovery {
                    data: data.clone(),
                    bad_sector_ranges: Vec::new(),
                    bytes_recovered: len,
                    bytes_lost: 0,
                    complete: true,
                }
            } else {
                PartialRecovery {
                    data: Vec::new(),
                    bad_sector_ranges: Vec::new(),
                    bytes_recovered: 0,
                    bytes_lost: 0,
                    complete: true,
                }
            }
        }
        AttributeForm::NonResident(non_resident) => {
            let AttributeValue::DataRuns(runs) = &attribute.value else {
                return PartialRecovery {
                    data: Vec::new(),
                    bad_sector_ranges: Vec::new(),
                    bytes_recovered: 0,
                    bytes_lost: 0,
                    complete: false,
                };
            };
            if attribute.is_compressed() && non_resident.compression_unit_size > 0 {
                return extract_compressed_runs_partial(
                    source,
                    boot,
                    volume_offset,
                    runs,
                    non_resident,
                );
            }
            let cluster_size = boot.cluster_size();
            let total_size = non_resident.real_size;
            let mut output = vec![0_u8; total_size as usize];
            let mut bad_ranges: Vec<BadSectorRange> = Vec::new();
            let mut bytes_recovered = 0u64;
            let mut file_offset = 0u64;
            let mut remaining = total_size;

            for run in runs {
                if remaining == 0 {
                    break;
                }
                let Some(run_bytes) = run.cluster_count.checked_mul(cluster_size) else {
                    break;
                };
                let bytes_to_read = run_bytes.min(remaining);

                if let Some(lcn_start) = run.lcn_start {
                    if lcn_start >= 0 {
                        let disk_offset = volume_offset
                            .checked_add((lcn_start as u64).saturating_mul(cluster_size));
                        match disk_offset {
                            Some(disk_off) => {
                                let buf_start = file_offset as usize;
                                let buf_end =
                                    (file_offset + bytes_to_read).min(total_size) as usize;
                                if source
                                    .read_at(disk_off, &mut output[buf_start..buf_end])
                                    .is_ok()
                                {
                                    bytes_recovered += bytes_to_read;
                                } else {
                                    bad_ranges.push(BadSectorRange {
                                        file_offset,
                                        length: bytes_to_read,
                                    });
                                }
                            }
                            None => {
                                bad_ranges.push(BadSectorRange {
                                    file_offset,
                                    length: bytes_to_read,
                                });
                            }
                        }
                    } else {
                        bad_ranges.push(BadSectorRange {
                            file_offset,
                            length: bytes_to_read,
                        });
                    }
                } else {
                    // Sparse run — zeroes are already in the buffer
                    bytes_recovered += bytes_to_read;
                }

                file_offset += bytes_to_read;
                remaining -= bytes_to_read;
            }

            let bytes_lost = bad_ranges.iter().map(|r| r.length).sum::<u64>();
            PartialRecovery {
                data: output,
                bad_sector_ranges: bad_ranges,
                bytes_recovered,
                bytes_lost,
                complete: bytes_lost == 0,
            }
        }
    }
}

#[deprecated(since = "0.2.0", note = "Please use extract_file_data instead")]
pub fn extract_non_resident_data<S: BlockSource + ?Sized>(
    source: &S,
    boot: &NtfsBootSector,
    volume_offset: u64,
    record: &MftRecord,
) -> Result<Vec<u8>, NtfsError> {
    extract_file_data(source, boot, volume_offset, record)
}

// ── Attribute types ───────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NtfsAttribute {
    pub header: AttributeHeader,
    pub value: AttributeValue,
}

impl NtfsAttribute {
    pub fn standard_information(&self) -> Option<&StandardInformation> {
        match &self.value {
            AttributeValue::StandardInformation(value) => Some(value),
            _ => None,
        }
    }

    pub fn file_name(&self) -> Option<&FileNameAttribute> {
        match &self.value {
            AttributeValue::FileName(value) => Some(value),
            _ => None,
        }
    }

    pub fn is_compressed(&self) -> bool {
        (self.header.flags & 0x0001) != 0
    }

    pub fn is_encrypted(&self) -> bool {
        (self.header.flags & 0x4000) != 0
    }

    pub fn is_sparse(&self) -> bool {
        (self.header.flags & 0x8000) != 0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AttributeHeader {
    pub attribute_type: AttributeType,
    pub raw_type: u32,
    pub length: u32,
    pub non_resident: bool,
    pub name_length: u8,
    pub name_offset: u16,
    pub flags: u16,
    pub attribute_id: u16,
    /// The optional attribute name (for alternate data streams, e.g. "Zone.Identifier").
    pub attribute_name: Option<String>,
    pub form: AttributeForm,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AttributeType {
    StandardInformation,
    AttributeList,
    FileName,
    ObjectId,
    SecurityDescriptor,
    VolumeName,
    VolumeInformation,
    Data,
    IndexRoot,
    IndexAllocation,
    Bitmap,
    ReparsePoint,
    EaInformation,
    Ea,
    LoggedUtilityStream,
    End,
    Unknown(u32),
}

impl AttributeType {
    pub fn from_raw(raw: u32) -> Self {
        match raw {
            0x10 => Self::StandardInformation,
            0x20 => Self::AttributeList,
            0x30 => Self::FileName,
            0x40 => Self::ObjectId,
            0x50 => Self::SecurityDescriptor,
            0x60 => Self::VolumeName,
            0x70 => Self::VolumeInformation,
            0x80 => Self::Data,
            0x90 => Self::IndexRoot,
            0xa0 => Self::IndexAllocation,
            0xb0 => Self::Bitmap,
            0xc0 => Self::ReparsePoint,
            0xd0 => Self::EaInformation,
            0xe0 => Self::Ea,
            0x100 => Self::LoggedUtilityStream,
            0xffff_ffff => Self::End,
            value => Self::Unknown(value),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AttributeForm {
    Resident(ResidentAttributeHeader),
    NonResident(NonResidentAttributeHeader),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResidentAttributeHeader {
    pub value_length: u32,
    pub value_offset: u16,
    pub indexed_flag: u8,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NonResidentAttributeHeader {
    pub lowest_vcn: u64,
    pub highest_vcn: u64,
    pub mapping_pairs_offset: u16,
    pub compression_unit_size: u16,
    pub allocated_size: u64,
    pub real_size: u64,
    pub initialized_size: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AttributeValue {
    StandardInformation(StandardInformation),
    FileName(FileNameAttribute),
    DataRuns(Vec<DataRun>),
    ResidentRaw(Vec<u8>),
    NonResident,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataRun {
    pub vcn_start: u64,
    pub cluster_count: u64,
    pub lcn_start: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StandardInformation {
    pub created_time: u64,
    pub modified_time: u64,
    pub mft_modified_time: u64,
    pub accessed_time: u64,
    pub file_attributes: u32,
    pub maximum_versions: Option<u32>,
    pub version_number: Option<u32>,
    pub class_id: Option<u32>,
    pub owner_id: Option<u32>,
    pub security_id: Option<u32>,
    pub quota_charged: Option<u64>,
    pub update_sequence_number: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileNameAttribute {
    pub parent_directory_reference: u64,
    pub parent_record_number: u64,
    pub parent_sequence_number: u16,
    pub created_time: u64,
    pub modified_time: u64,
    pub mft_modified_time: u64,
    pub accessed_time: u64,
    pub allocated_size: u64,
    pub real_size: u64,
    pub file_attributes: u32,
    pub ea_reparse: u32,
    pub namespace: FileNameNamespace,
    pub name: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileNameNamespace {
    Posix,
    Win32,
    Dos,
    Win32AndDos,
    Unknown(u8),
}

impl FileNameNamespace {
    fn from_raw(raw: u8) -> Self {
        match raw {
            0 => Self::Posix,
            1 => Self::Win32,
            2 => Self::Dos,
            3 => Self::Win32AndDos,
            value => Self::Unknown(value),
        }
    }
}

// ── Private helpers ───────────────────────────────────────────────────────────

fn get_compressed_size(data: &[u8]) -> usize {
    let mut offset = 0;
    while offset + 2 <= data.len() {
        let header = u16::from_le_bytes([data[offset], data[offset + 1]]);
        let signature = (header >> 12) & 0x7;
        if signature != 3 {
            break;
        }
        let block_size = (header & 0x0FFF) as usize + 3;
        if offset + block_size > data.len() {
            break;
        }
        offset += block_size;
    }
    offset
}

fn read_cu_data<S: BlockSource + ?Sized>(
    source: &S,
    boot: &NtfsBootSector,
    volume_offset: u64,
    runs: &[DataRun],
    cu_start_vcn: u64,
    cu_clusters: u64,
) -> Result<(Vec<u8>, u64), NtfsError> {
    let cluster_size = boot.cluster_size();
    let cu_end_vcn = cu_start_vcn + cu_clusters;
    let mut cu_data = Vec::new();
    let mut allocated_clusters = 0;

    for run in runs {
        let run_start_vcn = run.vcn_start;
        let run_end_vcn = run_start_vcn + run.cluster_count;

        let overlap_start = run_start_vcn.max(cu_start_vcn);
        let overlap_end = run_end_vcn.min(cu_end_vcn);

        if overlap_start < overlap_end {
            let overlap_count = overlap_end - overlap_start;
            if let Some(lcn_start) = run.lcn_start {
                allocated_clusters += overlap_count;
                let offset_in_run = overlap_start - run_start_vcn;
                let disk_offset = volume_offset
                    .checked_add(
                        (lcn_start as u64 + offset_in_run)
                            .checked_mul(cluster_size)
                            .ok_or(NtfsError::OffsetOverflow)?,
                    )
                    .ok_or(NtfsError::OffsetOverflow)?;

                let mut buffer = vec![0_u8; (overlap_count * cluster_size) as usize];
                source.read_at(disk_offset, &mut buffer)?;
                cu_data.extend_from_slice(&buffer);
            }
        }
    }

    Ok((cu_data, allocated_clusters))
}

fn extract_compressed_runs<S: BlockSource + ?Sized>(
    source: &S,
    boot: &NtfsBootSector,
    volume_offset: u64,
    runs: &[DataRun],
    non_resident: &NonResidentAttributeHeader,
) -> Result<Vec<u8>, NtfsError> {
    let cluster_size = boot.cluster_size();
    let cu_clusters = 1 << non_resident.compression_unit_size;
    let cu_bytes = cu_clusters * cluster_size;
    let mut output = Vec::new();
    let mut remaining = non_resident.real_size;

    let mut current_vcn = 0;
    let total_clusters = runs.iter().map(|r| r.cluster_count).sum::<u64>();

    while remaining > 0 && current_vcn < total_clusters {
        let cu_len = remaining.min(cu_bytes);
        let (cu_data, allocated) =
            read_cu_data(source, boot, volume_offset, runs, current_vcn, cu_clusters)?;

        let decompressed = if allocated == 0 {
            vec![0_u8; cu_bytes as usize]
        } else if allocated == cu_clusters {
            cu_data
        } else {
            let clean_size = get_compressed_size(&cu_data);
            let compressed_slice = &cu_data[..clean_size];

            let mut decomp = Vec::new();
            match lznt1::decompress(compressed_slice, &mut decomp) {
                Ok(_) => {
                    match decomp.len().cmp(&(cu_bytes as usize)) {
                        std::cmp::Ordering::Less => decomp.resize(cu_bytes as usize, 0),
                        std::cmp::Ordering::Greater => decomp.truncate(cu_bytes as usize),
                        std::cmp::Ordering::Equal => {}
                    }
                    decomp
                }
                Err(_) => {
                    return Err(NtfsError::DecompressionFailed);
                }
            }
        };

        let copy_size = (cu_len as usize).min(decompressed.len());
        output.extend_from_slice(&decompressed[..copy_size]);
        remaining -= copy_size as u64;
        current_vcn += cu_clusters;
    }

    if output.len() < non_resident.real_size as usize {
        output.resize(non_resident.real_size as usize, 0);
    }

    Ok(output)
}

fn extract_compressed_runs_partial<S: BlockSource + ?Sized>(
    source: &S,
    boot: &NtfsBootSector,
    volume_offset: u64,
    runs: &[DataRun],
    non_resident: &NonResidentAttributeHeader,
) -> PartialRecovery {
    let cluster_size = boot.cluster_size();
    let cu_clusters = 1 << non_resident.compression_unit_size;
    let cu_bytes = cu_clusters * cluster_size;
    let mut output = vec![0_u8; non_resident.real_size as usize];
    let mut bad_ranges: Vec<BadSectorRange> = Vec::new();
    let mut bytes_recovered = 0u64;
    let mut remaining = non_resident.real_size;

    let mut current_vcn = 0;
    let total_clusters = runs.iter().map(|r| r.cluster_count).sum::<u64>();
    let mut file_offset = 0u64;

    while remaining > 0 && current_vcn < total_clusters {
        let cu_len = remaining.min(cu_bytes);

        match read_cu_data(source, boot, volume_offset, runs, current_vcn, cu_clusters) {
            Ok((cu_data, allocated)) => {
                let decompressed = if allocated == 0 {
                    vec![0_u8; cu_bytes as usize]
                } else if allocated == cu_clusters {
                    cu_data
                } else {
                    let clean_size = get_compressed_size(&cu_data);
                    let compressed_slice = &cu_data[..clean_size];
                    let mut decomp = Vec::new();
                    match lznt1::decompress(compressed_slice, &mut decomp) {
                        Ok(_) => {
                            match decomp.len().cmp(&(cu_bytes as usize)) {
                                std::cmp::Ordering::Less => decomp.resize(cu_bytes as usize, 0),
                                std::cmp::Ordering::Greater => decomp.truncate(cu_bytes as usize),
                                std::cmp::Ordering::Equal => {}
                            }
                            decomp
                        }
                        Err(_) => {
                            bad_ranges.push(BadSectorRange {
                                file_offset,
                                length: cu_len,
                            });
                            vec![0_u8; cu_bytes as usize]
                        }
                    }
                };

                let buf_start = file_offset as usize;
                let buf_end = (file_offset + cu_len) as usize;
                output[buf_start..buf_end].copy_from_slice(&decompressed[..cu_len as usize]);
                bytes_recovered += cu_len;
            }
            Err(_) => {
                bad_ranges.push(BadSectorRange {
                    file_offset,
                    length: cu_len,
                });
            }
        }

        file_offset += cu_len;
        remaining -= cu_len;
        current_vcn += cu_clusters;
    }

    let bytes_lost = bad_ranges.iter().map(|r| r.length).sum::<u64>();
    PartialRecovery {
        data: output,
        bad_sector_ranges: bad_ranges,
        bytes_recovered,
        bytes_lost,
        complete: bytes_lost == 0,
    }
}

fn extract_attribute_data<S: BlockSource + ?Sized>(
    source: &S,
    boot: &NtfsBootSector,
    volume_offset: u64,
    attribute: &NtfsAttribute,
) -> Result<Vec<u8>, NtfsError> {
    if attribute.is_encrypted() {
        return Err(NtfsError::EncryptedNotSupported);
    }
    match &attribute.header.form {
        AttributeForm::Resident(_) => {
            if let AttributeValue::ResidentRaw(data) = &attribute.value {
                Ok(data.clone())
            } else {
                Ok(Vec::new())
            }
        }
        AttributeForm::NonResident(non_resident) => {
            let AttributeValue::DataRuns(runs) = &attribute.value else {
                return Ok(Vec::new());
            };
            if attribute.is_compressed() && non_resident.compression_unit_size > 0 {
                extract_compressed_runs(source, boot, volume_offset, runs, non_resident)
            } else {
                let cluster_size = boot.cluster_size();
                let mut output = Vec::new();
                let mut remaining = non_resident.real_size;

                for run in runs {
                    if remaining == 0 {
                        break;
                    }

                    let run_bytes = run
                        .cluster_count
                        .checked_mul(cluster_size)
                        .ok_or(NtfsError::OffsetOverflow)?;
                    let bytes_to_read = run_bytes.min(remaining);

                    if let Some(lcn_start) = run.lcn_start {
                        if lcn_start < 0 {
                            return Err(NtfsError::InvalidDataRun { offset: 0 });
                        }

                        let disk_offset = volume_offset
                            .checked_add(
                                (lcn_start as u64)
                                    .checked_mul(cluster_size)
                                    .ok_or(NtfsError::OffsetOverflow)?,
                            )
                            .ok_or(NtfsError::OffsetOverflow)?;
                        let mut buffer = vec![0_u8; bytes_to_read as usize];
                        source.read_at(disk_offset, &mut buffer)?;
                        output.extend_from_slice(&buffer);
                    } else {
                        output.resize(output.len() + bytes_to_read as usize, 0);
                    }

                    remaining -= bytes_to_read;
                }

                Ok(output)
            }
        }
    }
}

fn parse_attributes(record: &[u8], mut offset: usize) -> Result<Vec<NtfsAttribute>, NtfsError> {
    let mut attributes = Vec::new();

    loop {
        if offset + 4 > record.len() {
            return Err(NtfsError::InvalidAttribute {
                offset,
                record_size: record.len(),
            });
        }

        let raw_type = le_u32(record, offset);
        if raw_type == 0xffff_ffff {
            break;
        }

        let attribute = parse_attribute(record, offset)?;
        let length = attribute.header.length as usize;
        if length == 0 {
            return Err(NtfsError::InvalidAttribute {
                offset,
                record_size: record.len(),
            });
        }

        offset = offset
            .checked_add(length)
            .ok_or(NtfsError::OffsetOverflow)?;
        attributes.push(attribute);
    }

    Ok(attributes)
}

fn parse_attribute(record: &[u8], offset: usize) -> Result<NtfsAttribute, NtfsError> {
    if offset + 16 > record.len() {
        return Err(NtfsError::InvalidAttribute {
            offset,
            record_size: record.len(),
        });
    }

    let raw_type = le_u32(record, offset);
    let length = le_u32(record, offset + 4);
    let length_usize = length as usize;
    if length_usize < 16 || offset + length_usize > record.len() {
        return Err(NtfsError::InvalidAttribute {
            offset,
            record_size: record.len(),
        });
    }

    let non_resident = record[offset + 8] != 0;
    let name_length = record[offset + 9];
    let name_offset = le_u16(record, offset + 10);
    let flags = le_u16(record, offset + 12);
    let attribute_id = le_u16(record, offset + 14);
    let attribute_type = AttributeType::from_raw(raw_type);
    let attribute_name =
        parse_attribute_name(record, offset, name_length, name_offset, length_usize);

    if non_resident {
        if length_usize < 64 {
            return Err(NtfsError::InvalidAttribute {
                offset,
                record_size: record.len(),
            });
        }

        let header = NonResidentAttributeHeader {
            lowest_vcn: le_u64(record, offset + 16),
            highest_vcn: le_u64(record, offset + 24),
            mapping_pairs_offset: le_u16(record, offset + 32),
            compression_unit_size: le_u16(record, offset + 34),
            allocated_size: le_u64(record, offset + 40),
            real_size: le_u64(record, offset + 48),
            initialized_size: le_u64(record, offset + 56),
        };
        let value = if attribute_type == AttributeType::Data {
            let mapping_offset = offset + header.mapping_pairs_offset as usize;
            let attribute_end = offset + length_usize;
            if mapping_offset > attribute_end {
                return Err(NtfsError::InvalidAttribute {
                    offset,
                    record_size: record.len(),
                });
            }
            AttributeValue::DataRuns(parse_data_runs(
                &record[mapping_offset..attribute_end],
                mapping_offset,
            )?)
        } else {
            AttributeValue::NonResident
        };

        return Ok(NtfsAttribute {
            header: AttributeHeader {
                attribute_type,
                raw_type,
                length,
                non_resident,
                name_length,
                name_offset,
                flags,
                attribute_id,
                attribute_name,
                form: AttributeForm::NonResident(header),
            },
            value,
        });
    }

    if length_usize < 24 {
        return Err(NtfsError::InvalidAttribute {
            offset,
            record_size: record.len(),
        });
    }

    let resident = ResidentAttributeHeader {
        value_length: le_u32(record, offset + 16),
        value_offset: le_u16(record, offset + 20),
        indexed_flag: record[offset + 22],
    };

    let value_start = offset + resident.value_offset as usize;
    let value_end = value_start
        .checked_add(resident.value_length as usize)
        .ok_or(NtfsError::OffsetOverflow)?;
    if value_start < offset || value_end > offset + length_usize || value_end > record.len() {
        return Err(NtfsError::InvalidAttribute {
            offset,
            record_size: record.len(),
        });
    }

    let value_bytes = &record[value_start..value_end];
    let value = match attribute_type {
        AttributeType::StandardInformation => {
            AttributeValue::StandardInformation(StandardInformation::parse(value_bytes)?)
        }
        AttributeType::FileName => AttributeValue::FileName(FileNameAttribute::parse(value_bytes)?),
        _ => AttributeValue::ResidentRaw(value_bytes.to_vec()),
    };

    Ok(NtfsAttribute {
        header: AttributeHeader {
            attribute_type,
            raw_type,
            length,
            non_resident,
            name_length,
            name_offset,
            flags,
            attribute_id,
            attribute_name,
            form: AttributeForm::Resident(resident),
        },
        value,
    })
}

fn parse_attribute_name(
    record: &[u8],
    attr_offset: usize,
    name_length: u8,
    name_offset: u16,
    attr_length: usize,
) -> Option<String> {
    if name_length == 0 {
        return None;
    }
    let name_start = attr_offset + name_offset as usize;
    let name_bytes = name_length as usize * 2;
    let name_end = name_start + name_bytes;
    if name_end > attr_offset + attr_length || name_end > record.len() {
        return None;
    }
    decode_utf16le(&record[name_start..name_end]).ok()
}

pub fn parse_data_runs(bytes: &[u8], base_offset: usize) -> Result<Vec<DataRun>, NtfsError> {
    let mut runs = Vec::new();
    let mut cursor = 0_usize;
    let mut current_lcn = 0_i64;
    let mut current_vcn = 0_u64;

    while cursor < bytes.len() {
        let header = bytes[cursor];
        cursor += 1;
        if header == 0 {
            return Ok(runs);
        }

        let length_size = (header & 0x0f) as usize;
        let offset_size = (header >> 4) as usize;
        if length_size == 0 || length_size > 8 || offset_size > 8 {
            return Err(NtfsError::InvalidDataRun {
                offset: base_offset + cursor - 1,
            });
        }
        if cursor + length_size + offset_size > bytes.len() {
            return Err(NtfsError::InvalidDataRun {
                offset: base_offset + cursor - 1,
            });
        }

        let cluster_count = read_le_unsigned(&bytes[cursor..cursor + length_size]);
        cursor += length_size;

        let lcn_start = if offset_size == 0 {
            None
        } else {
            let delta = read_le_signed(&bytes[cursor..cursor + offset_size]);
            cursor += offset_size;
            current_lcn = current_lcn
                .checked_add(delta)
                .ok_or(NtfsError::InvalidDataRun {
                    offset: base_offset + cursor,
                })?;
            Some(current_lcn)
        };

        runs.push(DataRun {
            vcn_start: current_vcn,
            cluster_count,
            lcn_start,
        });
        current_vcn = current_vcn
            .checked_add(cluster_count)
            .ok_or(NtfsError::OffsetOverflow)?;
    }

    Err(NtfsError::InvalidDataRun {
        offset: base_offset + cursor,
    })
}

impl StandardInformation {
    pub fn parse(value: &[u8]) -> Result<Self, NtfsError> {
        if value.len() < 48 {
            return Err(NtfsError::BufferTooSmall {
                actual: value.len(),
                minimum: 48,
            });
        }

        Ok(Self {
            created_time: le_u64(value, 0),
            modified_time: le_u64(value, 8),
            mft_modified_time: le_u64(value, 16),
            accessed_time: le_u64(value, 24),
            file_attributes: le_u32(value, 32),
            maximum_versions: read_optional_u32(value, 36),
            version_number: read_optional_u32(value, 40),
            class_id: read_optional_u32(value, 44),
            owner_id: read_optional_u32(value, 48),
            security_id: read_optional_u32(value, 52),
            quota_charged: read_optional_u64(value, 56),
            update_sequence_number: read_optional_u64(value, 64),
        })
    }
}

impl FileNameAttribute {
    pub fn parse(value: &[u8]) -> Result<Self, NtfsError> {
        if value.len() < 66 {
            return Err(NtfsError::BufferTooSmall {
                actual: value.len(),
                minimum: 66,
            });
        }

        let name_length = value[64] as usize;
        let name_bytes = name_length
            .checked_mul(2)
            .ok_or(NtfsError::OffsetOverflow)?;
        if 66 + name_bytes > value.len() {
            return Err(NtfsError::BufferTooSmall {
                actual: value.len(),
                minimum: 66 + name_bytes,
            });
        }

        let name = decode_utf16le(&value[66..66 + name_bytes])?;
        let parent_directory_reference = le_u64(value, 0);

        Ok(Self {
            parent_directory_reference,
            parent_record_number: parent_directory_reference & 0x0000_ffff_ffff_ffff,
            parent_sequence_number: (parent_directory_reference >> 48) as u16,
            created_time: le_u64(value, 8),
            modified_time: le_u64(value, 16),
            mft_modified_time: le_u64(value, 24),
            accessed_time: le_u64(value, 32),
            allocated_size: le_u64(value, 40),
            real_size: le_u64(value, 48),
            file_attributes: le_u32(value, 56),
            ea_reparse: le_u32(value, 60),
            namespace: FileNameNamespace::from_raw(value[65]),
            name,
        })
    }
}

fn decode_utf16le(bytes: &[u8]) -> Result<String, NtfsError> {
    if bytes.len() % 2 != 0 {
        return Err(NtfsError::InvalidUtf16Name);
    }

    let units = bytes
        .chunks_exact(2)
        .map(|chunk| u16::from_le_bytes([chunk[0], chunk[1]]))
        .collect::<Vec<_>>();
    String::from_utf16(&units).map_err(|_| NtfsError::InvalidUtf16Name)
}

fn read_optional_u32(buffer: &[u8], offset: usize) -> Option<u32> {
    (offset + 4 <= buffer.len()).then(|| le_u32(buffer, offset))
}

fn read_optional_u64(buffer: &[u8], offset: usize) -> Option<u64> {
    (offset + 8 <= buffer.len()).then(|| le_u64(buffer, offset))
}

fn read_le_unsigned(bytes: &[u8]) -> u64 {
    let mut value = 0_u64;
    for (index, byte) in bytes.iter().enumerate() {
        value |= (*byte as u64) << (index * 8);
    }
    value
}

fn read_le_signed(bytes: &[u8]) -> i64 {
    let unsigned = read_le_unsigned(bytes);
    let bits = bytes.len() * 8;
    let sign_bit = 1_u64 << (bits - 1);
    if unsigned & sign_bit == 0 {
        unsigned as i64
    } else {
        let mask = (!0_u64) << bits;
        (unsigned | mask) as i64
    }
}

fn apply_update_sequence_array(
    record: &[u8],
    bytes_per_sector: usize,
    usa_offset: u16,
    usa_count: u16,
) -> Result<Vec<u8>, NtfsError> {
    let sectors = record.len() / bytes_per_sector;
    let usa_offset = usa_offset as usize;
    let usa_count = usa_count as usize;
    let usa_bytes = usa_count
        .checked_mul(2)
        .ok_or(NtfsError::InvalidFixupArray {
            usa_offset: usa_offset as u16,
            usa_count: usa_count as u16,
            record_size: record.len(),
        })?;

    if usa_count != sectors + 1 || usa_offset + usa_bytes > record.len() {
        return Err(NtfsError::InvalidFixupArray {
            usa_offset: usa_offset as u16,
            usa_count: usa_count as u16,
            record_size: record.len(),
        });
    }

    let expected = le_u16(record, usa_offset);
    let mut fixed = record.to_vec();

    for sector_index in 0..sectors {
        let trailer_offset = (sector_index + 1) * bytes_per_sector - 2;
        let actual = le_u16(record, trailer_offset);
        if actual != expected {
            return Err(NtfsError::FixupMismatch {
                sector_index,
                expected,
                actual,
            });
        }

        let replacement_offset = usa_offset + ((sector_index + 1) * 2);
        fixed[trailer_offset..trailer_offset + 2]
            .copy_from_slice(&record[replacement_offset..replacement_offset + 2]);
    }

    Ok(fixed)
}

fn decode_ntfs_size_code(code: i8, cluster_size: u64) -> Result<u64, NtfsError> {
    if code > 0 {
        return (code as u64)
            .checked_mul(cluster_size)
            .ok_or(NtfsError::OffsetOverflow);
    }

    if code == i8::MIN {
        return Err(NtfsError::UnsupportedRecordSize(code));
    }

    let shift = (-code) as u32;
    1_u64
        .checked_shl(shift)
        .ok_or(NtfsError::UnsupportedRecordSize(code))
}

fn read_array<const N: usize>(buffer: &[u8], offset: usize) -> [u8; N] {
    let mut out = [0_u8; N];
    out.copy_from_slice(&buffer[offset..offset + N]);
    out
}

fn le_u16(buffer: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes(read_array(buffer, offset))
}

fn le_u32(buffer: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(read_array(buffer, offset))
}

fn le_u64(buffer: &[u8], offset: usize) -> u64 {
    u64::from_le_bytes(read_array(buffer, offset))
}

#[cfg(test)]
mod tests {
    use super::*;
    use data_recovery_core::FileBlockSource;
    use std::io::Write;
    use std::sync::atomic::{AtomicU64, Ordering};

    fn sample_boot_sector() -> [u8; NTFS_BOOT_SECTOR_SIZE] {
        let mut sector = [0_u8; NTFS_BOOT_SECTOR_SIZE];
        sector[0..3].copy_from_slice(&[0xeb, 0x52, 0x90]);
        sector[3..11].copy_from_slice(b"NTFS    ");
        sector[11..13].copy_from_slice(&512_u16.to_le_bytes());
        sector[13] = 8;
        sector[21] = 0xf8;
        sector[24..26].copy_from_slice(&63_u16.to_le_bytes());
        sector[26..28].copy_from_slice(&255_u16.to_le_bytes());
        sector[28..32].copy_from_slice(&2048_u32.to_le_bytes());
        sector[40..48].copy_from_slice(&100_000_u64.to_le_bytes());
        sector[48..56].copy_from_slice(&786_432_u64.to_le_bytes());
        sector[56..64].copy_from_slice(&2_u64.to_le_bytes());
        sector[64] = -10_i8 as u8;
        sector[68] = 1;
        sector[72..80].copy_from_slice(&0x0123_4567_89ab_cdef_u64.to_le_bytes());
        sector[510..512].copy_from_slice(&0xaa55_u16.to_le_bytes());
        sector
    }

    #[test]
    fn parses_boot_sector_fields() {
        let parsed = NtfsBootSector::parse(&sample_boot_sector()).unwrap();

        assert_eq!(parsed.bytes_per_sector, 512);
        assert_eq!(parsed.sectors_per_cluster, 8);
        assert_eq!(parsed.cluster_size(), 4096);
        assert_eq!(parsed.mft_lcn, 786_432);
        assert_eq!(parsed.mft_offset().unwrap(), 3_221_225_472);
        assert_eq!(parsed.file_record_size().unwrap(), 1024);
        assert_eq!(parsed.index_buffer_size().unwrap(), 4096);
        assert_eq!(parsed.total_size_bytes().unwrap(), 51_200_000);
        assert_eq!(parsed.volume_serial_number, 0x0123_4567_89ab_cdef);
    }

    #[test]
    fn rejects_non_ntfs_oem_id() {
        let mut sector = sample_boot_sector();
        sector[3..11].copy_from_slice(b"EXFAT   ");

        assert!(matches!(
            NtfsBootSector::parse(&sector),
            Err(NtfsError::InvalidOemId(_))
        ));
    }

    #[test]
    fn rejects_invalid_signature() {
        let mut sector = sample_boot_sector();
        sector[510..512].copy_from_slice(&0_u16.to_le_bytes());

        assert!(matches!(
            NtfsBootSector::parse(&sector),
            Err(NtfsError::InvalidBootSignature(0))
        ));
    }

    fn sample_mft_record(in_use: bool, directory: bool) -> [u8; 1024] {
        let mut record = [0_u8; 1024];
        record[0..4].copy_from_slice(b"FILE");
        record[4..6].copy_from_slice(&48_u16.to_le_bytes());
        record[6..8].copy_from_slice(&3_u16.to_le_bytes());
        record[8..16].copy_from_slice(&0x1111_2222_3333_4444_u64.to_le_bytes());
        record[16..18].copy_from_slice(&7_u16.to_le_bytes());
        record[18..20].copy_from_slice(&2_u16.to_le_bytes());
        record[20..22].copy_from_slice(&56_u16.to_le_bytes());

        let mut flags = 0_u16;
        if in_use {
            flags |= 0x0001;
        }
        if directory {
            flags |= 0x0002;
        }
        record[22..24].copy_from_slice(&flags.to_le_bytes());

        record[24..28].copy_from_slice(&384_u32.to_le_bytes());
        record[28..32].copy_from_slice(&1024_u32.to_le_bytes());
        record[32..40].copy_from_slice(&0_u64.to_le_bytes());
        record[40..42].copy_from_slice(&5_u16.to_le_bytes());
        record[44..48].copy_from_slice(&42_u32.to_le_bytes());
        record[56..60].copy_from_slice(&0xffff_ffff_u32.to_le_bytes());

        record[48..50].copy_from_slice(&0xabcd_u16.to_le_bytes());
        record[50..52].copy_from_slice(&0xaaaa_u16.to_le_bytes());
        record[52..54].copy_from_slice(&0xbbbb_u16.to_le_bytes());
        record[510..512].copy_from_slice(&0xabcd_u16.to_le_bytes());
        record[1022..1024].copy_from_slice(&0xabcd_u16.to_le_bytes());
        record
    }

    fn sample_mft_record_with_attributes() -> [u8; 1024] {
        let mut record = sample_mft_record(true, false);
        let mut offset = 56;

        let standard_value_offset = 24_usize;
        let standard_value_length = 72_usize;
        let standard_length = standard_value_offset + standard_value_length;
        record[offset..offset + 4].copy_from_slice(&0x10_u32.to_le_bytes());
        record[offset + 4..offset + 8].copy_from_slice(&(standard_length as u32).to_le_bytes());
        record[offset + 8] = 0;
        record[offset + 9] = 0;
        record[offset + 10..offset + 12].copy_from_slice(&0_u16.to_le_bytes());
        record[offset + 12..offset + 14].copy_from_slice(&0_u16.to_le_bytes());
        record[offset + 14..offset + 16].copy_from_slice(&1_u16.to_le_bytes());
        record[offset + 16..offset + 20]
            .copy_from_slice(&(standard_value_length as u32).to_le_bytes());
        record[offset + 20..offset + 22]
            .copy_from_slice(&(standard_value_offset as u16).to_le_bytes());
        record[offset + 22] = 0;

        let value = offset + standard_value_offset;
        record[value..value + 8].copy_from_slice(&100_u64.to_le_bytes());
        record[value + 8..value + 16].copy_from_slice(&200_u64.to_le_bytes());
        record[value + 16..value + 24].copy_from_slice(&300_u64.to_le_bytes());
        record[value + 24..value + 32].copy_from_slice(&400_u64.to_le_bytes());
        record[value + 32..value + 36].copy_from_slice(&0x20_u32.to_le_bytes());
        record[value + 48..value + 52].copy_from_slice(&11_u32.to_le_bytes());
        record[value + 52..value + 56].copy_from_slice(&22_u32.to_le_bytes());
        record[value + 56..value + 64].copy_from_slice(&33_u64.to_le_bytes());
        record[value + 64..value + 72].copy_from_slice(&44_u64.to_le_bytes());
        offset += standard_length;

        let name_utf16 = "report.txt"
            .encode_utf16()
            .flat_map(|unit| unit.to_le_bytes())
            .collect::<Vec<_>>();
        let file_name_value_length = 66 + name_utf16.len();
        let file_name_value_offset = 24_usize;
        let file_name_length = file_name_value_offset + file_name_value_length;
        record[offset..offset + 4].copy_from_slice(&0x30_u32.to_le_bytes());
        record[offset + 4..offset + 8].copy_from_slice(&(file_name_length as u32).to_le_bytes());
        record[offset + 8] = 0;
        record[offset + 9] = 0;
        record[offset + 10..offset + 12].copy_from_slice(&0_u16.to_le_bytes());
        record[offset + 12..offset + 14].copy_from_slice(&0_u16.to_le_bytes());
        record[offset + 14..offset + 16].copy_from_slice(&2_u16.to_le_bytes());
        record[offset + 16..offset + 20]
            .copy_from_slice(&(file_name_value_length as u32).to_le_bytes());
        record[offset + 20..offset + 22]
            .copy_from_slice(&(file_name_value_offset as u16).to_le_bytes());
        record[offset + 22] = 1;

        let value = offset + file_name_value_offset;
        let parent_ref = (9_u64 << 48) | 5_u64;
        record[value..value + 8].copy_from_slice(&parent_ref.to_le_bytes());
        record[value + 8..value + 16].copy_from_slice(&101_u64.to_le_bytes());
        record[value + 16..value + 24].copy_from_slice(&201_u64.to_le_bytes());
        record[value + 24..value + 32].copy_from_slice(&301_u64.to_le_bytes());
        record[value + 32..value + 40].copy_from_slice(&401_u64.to_le_bytes());
        record[value + 40..value + 48].copy_from_slice(&4096_u64.to_le_bytes());
        record[value + 48..value + 56].copy_from_slice(&1234_u64.to_le_bytes());
        record[value + 56..value + 60].copy_from_slice(&0x20_u32.to_le_bytes());
        record[value + 60..value + 64].copy_from_slice(&0_u32.to_le_bytes());
        record[value + 64] = 10;
        record[value + 65] = 1;
        record[value + 66..value + 66 + name_utf16.len()].copy_from_slice(&name_utf16);
        offset += file_name_length;

        record[offset..offset + 4].copy_from_slice(&0xffff_ffff_u32.to_le_bytes());
        record[24..28].copy_from_slice(&(offset as u32 + 4).to_le_bytes());
        record
    }

    fn sample_mft_record_with_non_resident_data() -> [u8; 1024] {
        let mut record = sample_mft_record_with_attributes();
        let mut offset = record
            .windows(4)
            .position(|window| window == 0xffff_ffff_u32.to_le_bytes())
            .unwrap();

        let mapping_pairs = [0x11, 0x03, 0x20, 0x11, 0x02, 0x05, 0x00];
        let data_length = 64 + mapping_pairs.len();
        record[offset..offset + 4].copy_from_slice(&0x80_u32.to_le_bytes());
        record[offset + 4..offset + 8].copy_from_slice(&(data_length as u32).to_le_bytes());
        record[offset + 8] = 1;
        record[offset + 14..offset + 16].copy_from_slice(&3_u16.to_le_bytes());
        record[offset + 16..offset + 24].copy_from_slice(&0_u64.to_le_bytes());
        record[offset + 24..offset + 32].copy_from_slice(&4_u64.to_le_bytes());
        record[offset + 32..offset + 34].copy_from_slice(&64_u16.to_le_bytes());
        record[offset + 40..offset + 48].copy_from_slice(&(5_u64 * 4096).to_le_bytes());
        record[offset + 48..offset + 56].copy_from_slice(&12_345_u64.to_le_bytes());
        record[offset + 56..offset + 64].copy_from_slice(&12_345_u64.to_le_bytes());
        record[offset + 64..offset + 64 + mapping_pairs.len()].copy_from_slice(&mapping_pairs);
        offset += data_length;

        record[offset..offset + 4].copy_from_slice(&0xffff_ffff_u32.to_le_bytes());
        record[24..28].copy_from_slice(&(offset as u32 + 4).to_le_bytes());
        record
    }

    fn sample_mft_record_with_single_data_run(real_size: u64, lcn: u8) -> [u8; 1024] {
        let mut record = sample_mft_record_with_attributes();
        let mut offset = record
            .windows(4)
            .position(|window| window == 0xffff_ffff_u32.to_le_bytes())
            .unwrap();

        let mapping_pairs = [0x11, 0x01, lcn, 0x00];
        let data_length = 64 + mapping_pairs.len();
        record[offset..offset + 4].copy_from_slice(&0x80_u32.to_le_bytes());
        record[offset + 4..offset + 8].copy_from_slice(&(data_length as u32).to_le_bytes());
        record[offset + 8] = 1;
        record[offset + 14..offset + 16].copy_from_slice(&3_u16.to_le_bytes());
        record[offset + 16..offset + 24].copy_from_slice(&0_u64.to_le_bytes());
        record[offset + 24..offset + 32].copy_from_slice(&0_u64.to_le_bytes());
        record[offset + 32..offset + 34].copy_from_slice(&64_u16.to_le_bytes());
        record[offset + 40..offset + 48].copy_from_slice(&4096_u64.to_le_bytes());
        record[offset + 48..offset + 56].copy_from_slice(&real_size.to_le_bytes());
        record[offset + 56..offset + 64].copy_from_slice(&real_size.to_le_bytes());
        record[offset + 64..offset + 64 + mapping_pairs.len()].copy_from_slice(&mapping_pairs);
        offset += data_length;

        record[offset..offset + 4].copy_from_slice(&0xffff_ffff_u32.to_le_bytes());
        record[24..28].copy_from_slice(&(offset as u32 + 4).to_le_bytes());
        record
    }

    fn sample_mft_record_with_resident_data(payload: &[u8]) -> [u8; 1024] {
        let mut record = sample_mft_record_with_attributes();
        let mut offset = record
            .windows(4)
            .position(|window| window == 0xffff_ffff_u32.to_le_bytes())
            .unwrap();

        let data_value_offset = 24_usize;
        let data_value_length = payload.len();
        let data_length = data_value_offset + data_value_length;
        record[offset..offset + 4].copy_from_slice(&0x80_u32.to_le_bytes());
        record[offset + 4..offset + 8].copy_from_slice(&(data_length as u32).to_le_bytes());
        record[offset + 8] = 0;
        record[offset + 9] = 0;
        record[offset + 10..offset + 12].copy_from_slice(&0_u16.to_le_bytes());
        record[offset + 12..offset + 14].copy_from_slice(&0_u16.to_le_bytes());
        record[offset + 14..offset + 16].copy_from_slice(&3_u16.to_le_bytes());
        record[offset + 16..offset + 20].copy_from_slice(&(data_value_length as u32).to_le_bytes());
        record[offset + 20..offset + 22].copy_from_slice(&(data_value_offset as u16).to_le_bytes());
        record[offset + 22] = 0;

        let value_pos = offset + data_value_offset;
        record[value_pos..value_pos + payload.len()].copy_from_slice(payload);
        offset += data_length;

        record[offset..offset + 4].copy_from_slice(&0xffff_ffff_u32.to_le_bytes());
        record[24..28].copy_from_slice(&(offset as u32 + 4).to_le_bytes());
        record
    }

    /// Builds a record with two $DATA attributes: unnamed stream + a named "Zone.Identifier" ADS.
    fn sample_mft_record_with_two_streams(
        default_payload: &[u8],
        ads_payload: &[u8],
    ) -> [u8; 1024] {
        let mut record = sample_mft_record_with_resident_data(default_payload);
        // Find the end-marker and append a second resident $DATA with a name
        let mut offset = record
            .windows(4)
            .position(|window| window == 0xffff_ffff_u32.to_le_bytes())
            .unwrap();

        let ads_name_utf16: Vec<u8> = "Zone.Identifier"
            .encode_utf16()
            .flat_map(|u| u.to_le_bytes())
            .collect();
        let name_length_units = (ads_name_utf16.len() / 2) as u8;
        // layout: [common 16 bytes][resident extra 8 bytes][name bytes (aligned)][value bytes]
        // name_offset = 24 (right after the 16-byte common + 8-byte resident header)
        let name_offset: u16 = 24;
        let name_bytes = ads_name_utf16.len();
        // Align value start to 8 bytes
        let value_offset_raw = name_offset as usize + name_bytes;
        let value_offset = (value_offset_raw + 7) & !7;
        let value_length = ads_payload.len();
        let attr_length = (value_offset + value_length + 7) & !7;
        if offset + attr_length + 4 > 1024 {
            return record; // not enough space, skip
        }

        record[offset..offset + 4].copy_from_slice(&0x80_u32.to_le_bytes());
        record[offset + 4..offset + 8].copy_from_slice(&(attr_length as u32).to_le_bytes());
        record[offset + 8] = 0; // resident
        record[offset + 9] = name_length_units;
        record[offset + 10..offset + 12].copy_from_slice(&name_offset.to_le_bytes());
        record[offset + 12..offset + 14].copy_from_slice(&0_u16.to_le_bytes()); // flags
        record[offset + 14..offset + 16].copy_from_slice(&4_u16.to_le_bytes()); // attr_id
        record[offset + 16..offset + 20].copy_from_slice(&(value_length as u32).to_le_bytes());
        record[offset + 20..offset + 22].copy_from_slice(&(value_offset as u16).to_le_bytes());
        record[offset + 22] = 0;

        let name_start = offset + name_offset as usize;
        record[name_start..name_start + name_bytes].copy_from_slice(&ads_name_utf16);

        let val_start = offset + value_offset;
        record[val_start..val_start + value_length].copy_from_slice(ads_payload);

        offset += attr_length;
        record[offset..offset + 4].copy_from_slice(&0xffff_ffff_u32.to_le_bytes());
        record[24..28].copy_from_slice(&(offset as u32 + 4).to_le_bytes());
        record
    }

    fn temp_file_name() -> std::path::PathBuf {
        static NEXT_ID: AtomicU64 = AtomicU64::new(0);
        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!("dr_ntfs_volume_{}_{id}.bin", std::process::id()))
    }

    #[test]
    fn parses_mft_record_header_and_applies_fixup() {
        let record = MftRecord::parse(&sample_mft_record(true, false), 512).unwrap();

        assert_eq!(record.header.signature, *b"FILE");
        assert_eq!(record.header.sequence_number, 7);
        assert_eq!(record.header.hard_link_count, 2);
        assert_eq!(record.header.first_attribute_offset, 56);
        assert!(record.header.is_in_use());
        assert!(!record.header.is_deleted());
        assert!(!record.header.is_directory());
        assert_eq!(record.header.bytes_in_use, 384);
        assert_eq!(record.header.bytes_allocated, 1024);
        assert_eq!(record.header.next_attribute_id, 5);
        assert_eq!(record.header.record_number, Some(42));
        assert_eq!(le_u16(&record.fixed_up_data, 510), 0xaaaa);
        assert_eq!(le_u16(&record.fixed_up_data, 1022), 0xbbbb);
    }

    #[test]
    fn identifies_deleted_directory_record() {
        let record = MftRecord::parse(&sample_mft_record(false, true), 512).unwrap();

        assert!(!record.header.is_in_use());
        assert!(record.header.is_deleted());
        assert!(record.header.is_directory());
    }

    #[test]
    fn rejects_fixup_mismatch() {
        let mut record = sample_mft_record(true, false);
        record[1022..1024].copy_from_slice(&0xdddd_u16.to_le_bytes());

        assert!(matches!(
            MftRecord::parse(&record, 512),
            Err(NtfsError::FixupMismatch {
                sector_index: 1,
                ..
            })
        ));
    }

    #[test]
    fn parses_standard_information_and_file_name_attributes() {
        let record = MftRecord::parse(&sample_mft_record_with_attributes(), 512).unwrap();

        assert_eq!(record.attributes.len(), 2);

        let standard = record.attributes[0].standard_information().unwrap();
        assert_eq!(standard.created_time, 100);
        assert_eq!(standard.modified_time, 200);
        assert_eq!(standard.mft_modified_time, 300);
        assert_eq!(standard.accessed_time, 400);
        assert_eq!(standard.file_attributes, 0x20);
        assert_eq!(standard.owner_id, Some(11));
        assert_eq!(standard.security_id, Some(22));
        assert_eq!(standard.quota_charged, Some(33));
        assert_eq!(standard.update_sequence_number, Some(44));

        let file_name = record.attributes[1].file_name().unwrap();
        assert_eq!(file_name.parent_record_number, 5);
        assert_eq!(file_name.parent_sequence_number, 9);
        assert_eq!(file_name.name, "report.txt");
        assert_eq!(file_name.namespace, FileNameNamespace::Win32);
        assert_eq!(file_name.allocated_size, 4096);
        assert_eq!(file_name.real_size, 1234);
    }

    #[test]
    fn reads_mft_records_from_boot_sector_location() {
        let mut boot = sample_boot_sector();
        boot[48..56].copy_from_slice(&1_u64.to_le_bytes());
        let mut image = vec![0_u8; 4096 + 1024];
        image[..512].copy_from_slice(&boot);
        image[4096..4096 + 1024].copy_from_slice(&sample_mft_record_with_attributes());

        let path = temp_file_name();
        {
            let mut file = std::fs::File::create(&path).unwrap();
            file.write_all(&image).unwrap();
        }

        let source = FileBlockSource::open(&path).unwrap();
        let boot = read_boot_sector(&source, 0).unwrap();
        let records = read_mft_records(&source, &boot, 0, 1).unwrap();

        assert_eq!(records.len(), 1);
        assert_eq!(records[0].header.record_number, Some(42));

        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn reads_mft_record_by_index_through_fragmented_mft_runs() {
        let mut boot = sample_boot_sector();
        boot[48..56].copy_from_slice(&1_u64.to_le_bytes());
        let mut image = vec![0_u8; 40 * 4096];
        image[..512].copy_from_slice(&boot);
        image[4096..4096 + 1024].copy_from_slice(&sample_mft_record_with_non_resident_data());

        let record_one_offset = 32 * 4096 + 1024;
        image[record_one_offset..record_one_offset + 1024]
            .copy_from_slice(&sample_mft_record_with_attributes());

        let path = temp_file_name();
        std::fs::write(&path, &image).unwrap();
        let source = FileBlockSource::open(&path).unwrap();
        let boot = read_boot_sector(&source, 0).unwrap();
        let map = read_mft_run_map(&source, &boot, 0).unwrap();
        assert_eq!(map.runs().len(), 2);
        let record = read_mft_record_by_index_with_map(&source, &boot, &map, 1).unwrap();

        assert_eq!(record.header.record_number, Some(42));
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn parses_data_runs_mapping_pairs() {
        let runs = parse_data_runs(&[0x11, 0x03, 0x20, 0x11, 0x02, 0x05, 0x00], 100).unwrap();

        assert_eq!(
            runs,
            vec![
                DataRun {
                    vcn_start: 0,
                    cluster_count: 3,
                    lcn_start: Some(32),
                },
                DataRun {
                    vcn_start: 3,
                    cluster_count: 2,
                    lcn_start: Some(37),
                },
            ]
        );
    }

    #[test]
    fn parses_non_resident_data_attribute_runs() {
        let record = MftRecord::parse(&sample_mft_record_with_non_resident_data(), 512).unwrap();
        let data = record
            .attributes
            .iter()
            .find(|attribute| attribute.header.attribute_type == AttributeType::Data)
            .unwrap();

        let AttributeValue::DataRuns(runs) = &data.value else {
            panic!("expected data runs");
        };
        assert_eq!(runs.len(), 2);
        assert_eq!(runs[0].lcn_start, Some(32));
        assert_eq!(runs[1].lcn_start, Some(37));
    }

    #[test]
    fn extracts_non_resident_data_from_data_run() {
        let payload = b"hello recovered data";
        let mut boot = sample_boot_sector();
        boot[48..56].copy_from_slice(&1_u64.to_le_bytes());

        let mut image = vec![0_u8; 40 * 4096];
        image[..512].copy_from_slice(&boot);
        image[4096..4096 + 1024].copy_from_slice(&sample_mft_record_with_single_data_run(
            payload.len() as u64,
            32,
        ));
        image[32 * 4096..32 * 4096 + payload.len()].copy_from_slice(payload);

        let path = temp_file_name();
        {
            let mut file = std::fs::File::create(&path).unwrap();
            file.write_all(&image).unwrap();
        }

        let source = FileBlockSource::open(&path).unwrap();
        let boot = read_boot_sector(&source, 0).unwrap();
        let record = read_mft_records(&source, &boot, 0, 1).unwrap().remove(0);
        let extracted = extract_file_data(&source, &boot, 0, &record).unwrap();

        assert_eq!(extracted, payload);

        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn v08_extracts_fragmented_non_resident_data_from_multiple_runs() {
        let mut expected = vec![0_u8; 12_345];
        for (i, byte) in expected.iter_mut().enumerate() {
            *byte = (i % 251) as u8;
        }
        let mut boot = sample_boot_sector();
        boot[48..56].copy_from_slice(&1_u64.to_le_bytes());

        let mut image = vec![0_u8; 40 * 4096];
        image[..512].copy_from_slice(&boot);
        image[4096..4096 + 1024].copy_from_slice(&sample_mft_record_with_non_resident_data());
        image[32 * 4096..35 * 4096].copy_from_slice(&expected[..3 * 4096]);
        image[37 * 4096..37 * 4096 + (expected.len() - 3 * 4096)]
            .copy_from_slice(&expected[3 * 4096..]);

        let path = temp_file_name();
        std::fs::write(&path, &image).unwrap();
        let source = FileBlockSource::open(&path).unwrap();
        let boot = read_boot_sector(&source, 0).unwrap();
        let record = read_mft_records(&source, &boot, 0, 1).unwrap().remove(0);
        let extracted = extract_file_data(&source, &boot, 0, &record).unwrap();

        assert_eq!(extracted, expected);
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn extracts_resident_data() {
        let payload = b"resident data contents";
        let mut boot = sample_boot_sector();
        boot[48..56].copy_from_slice(&1_u64.to_le_bytes());

        let mut image = vec![0_u8; 8 * 4096];
        image[..512].copy_from_slice(&boot);
        image[4096..4096 + 1024].copy_from_slice(&sample_mft_record_with_resident_data(payload));

        let path = temp_file_name();
        {
            let mut file = std::fs::File::create(&path).unwrap();
            file.write_all(&image).unwrap();
        }

        let source = FileBlockSource::open(&path).unwrap();
        let boot = read_boot_sector(&source, 0).unwrap();
        let record = read_mft_records(&source, &boot, 0, 1).unwrap().remove(0);
        let extracted = extract_file_data(&source, &boot, 0, &record).unwrap();

        assert_eq!(extracted, payload);

        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn parses_attribute_flags() {
        let mut attr_bytes = sample_mft_record_with_attributes();
        attr_bytes[68..70].copy_from_slice(&(0x8001_u16).to_le_bytes());

        let record = MftRecord::parse(&attr_bytes, 512).unwrap();
        let std_info = record
            .attributes
            .iter()
            .find(|a| a.header.attribute_type == AttributeType::StandardInformation)
            .unwrap();

        assert!(std_info.is_compressed());
        assert!(!std_info.is_encrypted());
        assert!(std_info.is_sparse());
    }

    // ── New: multi-stream tests ───────────────────────────────────────────────

    #[test]
    fn lists_and_extracts_named_alternate_data_stream() {
        let default_data = b"main content";
        let ads_data = b"[ZoneTransfer]\r\nZoneId=3";
        let record_bytes = sample_mft_record_with_two_streams(default_data, ads_data);
        let record = MftRecord::parse(&record_bytes, 512).unwrap();

        let streams = list_data_streams(&record);
        assert_eq!(streams.len(), 2);

        let unnamed = streams.iter().find(|s| s.name.is_none()).unwrap();
        assert_eq!(unnamed.size, Some(default_data.len() as u64));

        let named = streams.iter().find(|s| s.name.is_some()).unwrap();
        assert_eq!(named.name.as_deref(), Some("Zone.Identifier"));
        assert_eq!(named.size, Some(ads_data.len() as u64));

        // extract_named_stream for the default stream
        let extracted_default = extract_named_stream(
            &data_recovery_core::FileBlockSource::open(
                std::env::temp_dir().join("unused_placeholder"),
            )
            .unwrap_or_else(|_| {
                // We only need a BlockSource to satisfy the type; resident data doesn't read from disk.
                // Use a tiny temp file as a placeholder.
                let p = std::env::temp_dir()
                    .join(format!("dr_ads_placeholder_{}.bin", std::process::id()));
                std::fs::write(&p, b"").ok();
                data_recovery_core::FileBlockSource::open(&p).unwrap()
            }),
            &NtfsBootSector::parse(&sample_boot_sector()).unwrap(),
            0,
            &record,
            None,
        )
        .unwrap();
        assert_eq!(extracted_default, default_data);

        // extract_named_stream for the ADS
        let extracted_ads = extract_named_stream(
            &data_recovery_core::FileBlockSource::open(
                std::env::temp_dir().join("unused_placeholder"),
            )
            .unwrap_or_else(|_| {
                let p = std::env::temp_dir()
                    .join(format!("dr_ads_placeholder2_{}.bin", std::process::id()));
                std::fs::write(&p, b"").ok();
                data_recovery_core::FileBlockSource::open(&p).unwrap()
            }),
            &NtfsBootSector::parse(&sample_boot_sector()).unwrap(),
            0,
            &record,
            Some("Zone.Identifier"),
        )
        .unwrap();
        assert_eq!(extracted_ads, ads_data);
    }

    // ── New: partial recovery tests ───────────────────────────────────────────

    #[test]
    fn partial_recovery_returns_complete_for_resident_data() {
        let payload = b"fully resident";
        let record = MftRecord::parse(&sample_mft_record_with_resident_data(payload), 512).unwrap();
        let boot = NtfsBootSector::parse(&sample_boot_sector()).unwrap();

        let p = temp_file_name();
        std::fs::write(&p, b"").unwrap();
        let source = FileBlockSource::open(&p).unwrap();

        let result = extract_file_data_partial(&source, &boot, 0, &record);
        assert!(result.complete);
        assert_eq!(result.bytes_lost, 0);
        assert_eq!(result.data, payload);
        std::fs::remove_file(p).unwrap();
    }

    #[test]
    fn partial_recovery_reads_good_runs_and_zeros_bad_ones() {
        let payload = b"hello partial";
        let mut boot_bytes = sample_boot_sector();
        boot_bytes[48..56].copy_from_slice(&1_u64.to_le_bytes());
        let boot = NtfsBootSector::parse(&boot_bytes).unwrap();

        // Disk with only cluster 32 populated; cluster 20 (lcn) is out of bounds
        let mut image = vec![0_u8; 40 * 4096];
        image[..512].copy_from_slice(&boot_bytes);

        // Build record: single run at lcn=32, size = payload.len()
        image[4096..4096 + 1024].copy_from_slice(&sample_mft_record_with_single_data_run(
            payload.len() as u64,
            32,
        ));
        image[32 * 4096..32 * 4096 + payload.len()].copy_from_slice(payload);

        let path = temp_file_name();
        std::fs::write(&path, &image).unwrap();
        let source = FileBlockSource::open(&path).unwrap();

        let result =
            extract_file_data_partial(&source, &boot, 0, &record_from_image(&source, &boot));
        assert!(result.complete);
        assert_eq!(&result.data[..payload.len()], payload);

        std::fs::remove_file(path).unwrap();
    }

    fn record_from_image(source: &FileBlockSource, boot: &NtfsBootSector) -> MftRecord {
        read_mft_records(source, boot, 0, 1).unwrap().remove(0)
    }

    #[test]
    fn test_compressed_decompression_and_encryption_error() {
        let header = AttributeHeader {
            attribute_type: AttributeType::Data,
            raw_type: 0x80,
            length: 72,
            non_resident: false,
            name_length: 0,
            name_offset: 0,
            flags: 0x4000, // Encrypted flag
            attribute_id: 1,
            attribute_name: None,
            form: AttributeForm::Resident(ResidentAttributeHeader {
                value_length: 8,
                value_offset: 24,
                indexed_flag: 0,
            }),
        };
        let attr = NtfsAttribute {
            header,
            value: AttributeValue::ResidentRaw(vec![0; 8]),
        };

        let path = temp_file_name();
        std::fs::write(&path, b"").unwrap();
        let source = FileBlockSource::open(&path).unwrap();
        let boot = NtfsBootSector::parse(&sample_boot_sector()).unwrap();

        let res = extract_attribute_data(&source, &boot, 0, &attr);
        assert!(matches!(res, Err(NtfsError::EncryptedNotSupported)));

        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn test_lznt1_decompress_block() {
        let compressed_data = [
            0x0c, 0xb0, 0x00, b'H', b'e', b'l', b'l', b'o', b' ', b'w', b'o', 0x00, b'r', b'l',
            b'd',
        ];
        let mut buffer = Vec::new();
        lznt1::decompress(&compressed_data, &mut buffer).unwrap();
        assert_eq!(buffer, b"Hello world");
    }
}
