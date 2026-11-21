use data_recovery_core::{BlockSource, ReadError};

pub mod spec;

#[derive(Debug)]
pub enum ApfsError {
    Read(ReadError),
    ChecksumMismatch { expected: u64, actual: u64 },
    InvalidMagic([u8; 4]),
    UnexpectedEof,
    InvalidBlockSize(u32),
}

impl std::fmt::Display for ApfsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Read(e) => write!(f, "Read error: {e}"),
            Self::ChecksumMismatch { expected, actual } => write!(
                f,
                "Fletcher64 checksum mismatch: expected {expected:#018x}, got {actual:#018x}"
            ),
            Self::InvalidMagic(m) => write!(
                f,
                "Invalid APFS magic: {:?}",
                std::str::from_utf8(m).unwrap_or("")
            ),
            Self::UnexpectedEof => write!(f, "Unexpected end of file/stream"),
            Self::InvalidBlockSize(sz) => write!(f, "Invalid APFS block size: {sz}"),
        }
    }
}

impl std::error::Error for ApfsError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Read(err) => Some(err),
            _ => None,
        }
    }
}

impl From<ReadError> for ApfsError {
    fn from(value: ReadError) -> Self {
        Self::Read(value)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObjHeader {
    pub cksum: u64,
    pub oid: u64,
    pub xid: u64,
    pub obj_type: u32,
    pub subtype: u32,
}

impl ObjHeader {
    pub fn parse(bytes: &[u8; 32]) -> Self {
        Self {
            cksum: u64::from_le_bytes(bytes[0..8].try_into().unwrap()),
            oid: u64::from_le_bytes(bytes[8..16].try_into().unwrap()),
            xid: u64::from_le_bytes(bytes[16..24].try_into().unwrap()),
            obj_type: u32::from_le_bytes(bytes[24..28].try_into().unwrap()),
            subtype: u32::from_le_bytes(bytes[28..32].try_into().unwrap()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApfsContainerSuperblock {
    pub header: ObjHeader,
    pub magic: [u8; 4],
    pub block_size: u32,
    pub block_count: u64,
    pub features: u64,
    pub readonly_compat_features: u64,
    pub incompat_features: u64,
    pub uuid: [u8; 16],
    pub next_oid: u64,
    pub next_xid: u64,
    pub xp_desc_blocks: u32,
    pub xp_data_blocks: u32,
    pub xp_desc_base: u64,
    pub xp_data_base: u64,
    pub xp_desc_next: u32,
    pub xp_data_next: u32,
    pub xp_desc_index: u32,
    pub xp_desc_len: u32,
    pub xp_data_index: u32,
    pub xp_data_len: u32,
    pub spaceman_oid: u64,
    pub omap_oid: u64,
    pub reaper_oid: u64,
    pub test_type: u32,
    pub max_file_systems: u32,
    pub fs_oids: Vec<u64>,
}

impl ApfsContainerSuperblock {
    pub fn parse(bytes: &[u8]) -> Result<Self, ApfsError> {
        if bytes.len() < 184 {
            return Err(ApfsError::UnexpectedEof);
        }

        let header = ObjHeader::parse(bytes[0..32].try_into().unwrap());
        let magic: [u8; 4] = bytes[32..36].try_into().unwrap();
        if &magic != b"NXSB" {
            return Err(ApfsError::InvalidMagic(magic));
        }

        let block_size = u32::from_le_bytes(bytes[36..40].try_into().unwrap());
        if !(512..=65536).contains(&block_size) || (block_size & (block_size - 1)) != 0 {
            return Err(ApfsError::InvalidBlockSize(block_size));
        }

        // Verify APFS object checksum. The checksum field itself is excluded
        // from the Fletcher sums and stores two check words.
        let original_cksum = header.cksum;
        let computed_cksum = fletcher64(&bytes[..block_size as usize]);
        if original_cksum != computed_cksum {
            return Err(ApfsError::ChecksumMismatch {
                expected: original_cksum,
                actual: computed_cksum,
            });
        }

        let max_file_systems = u32::from_le_bytes(bytes[180..184].try_into().unwrap());
        let mut fs_oids = Vec::new();
        let mut offset = 184;
        for _ in 0..max_file_systems {
            if offset + 8 > bytes.len() {
                break;
            }
            fs_oids.push(u64::from_le_bytes(
                bytes[offset..offset + 8].try_into().unwrap(),
            ));
            offset += 8;
        }

        Ok(Self {
            header,
            magic,
            block_size,
            block_count: u64::from_le_bytes(bytes[40..48].try_into().unwrap()),
            features: u64::from_le_bytes(bytes[48..56].try_into().unwrap()),
            readonly_compat_features: u64::from_le_bytes(bytes[56..64].try_into().unwrap()),
            incompat_features: u64::from_le_bytes(bytes[64..72].try_into().unwrap()),
            uuid: bytes[72..88].try_into().unwrap(),
            next_oid: u64::from_le_bytes(bytes[88..96].try_into().unwrap()),
            next_xid: u64::from_le_bytes(bytes[96..104].try_into().unwrap()),
            xp_desc_blocks: u32::from_le_bytes(bytes[104..108].try_into().unwrap()),
            xp_data_blocks: u32::from_le_bytes(bytes[108..112].try_into().unwrap()),
            xp_desc_base: u64::from_le_bytes(bytes[112..120].try_into().unwrap()),
            xp_data_base: u64::from_le_bytes(bytes[120..128].try_into().unwrap()),
            xp_desc_next: u32::from_le_bytes(bytes[128..132].try_into().unwrap()),
            xp_data_next: u32::from_le_bytes(bytes[132..136].try_into().unwrap()),
            xp_desc_index: u32::from_le_bytes(bytes[136..140].try_into().unwrap()),
            xp_desc_len: u32::from_le_bytes(bytes[140..144].try_into().unwrap()),
            xp_data_index: u32::from_le_bytes(bytes[144..148].try_into().unwrap()),
            xp_data_len: u32::from_le_bytes(bytes[148..152].try_into().unwrap()),
            spaceman_oid: u64::from_le_bytes(bytes[152..160].try_into().unwrap()),
            omap_oid: u64::from_le_bytes(bytes[160..168].try_into().unwrap()),
            reaper_oid: u64::from_le_bytes(bytes[168..176].try_into().unwrap()),
            test_type: u32::from_le_bytes(bytes[176..180].try_into().unwrap()),
            max_file_systems,
            fs_oids,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApfsVolumeSuperblock {
    pub header: ObjHeader,
    pub magic: [u8; 4],
    pub fs_index: u32,
    pub features: u64,
    pub readonly_compat_features: u64,
    pub incompat_features: u64,
    pub uniquifier: u64,
    pub role: u16,
    pub formatted_by: [u8; 32],
    pub meta_crypto: u64,
    pub omap_oid: u64,
    pub root_tree_oid: u64,
    pub extentref_tree_oid: u64,
    pub snap_meta_tree_oid: u64,
}

impl ApfsVolumeSuperblock {
    pub fn parse(bytes: &[u8], block_size: u32) -> Result<Self, ApfsError> {
        if bytes.len() < 148 {
            return Err(ApfsError::UnexpectedEof);
        }

        let header = ObjHeader::parse(bytes[0..32].try_into().unwrap());
        let magic: [u8; 4] = bytes[32..36].try_into().unwrap();
        if &magic != b"APFS" {
            return Err(ApfsError::InvalidMagic(magic));
        }

        // Verify APFS object checksum. The checksum field itself is excluded
        // from the Fletcher sums and stores two check words.
        let original_cksum = header.cksum;
        let computed_cksum = fletcher64(&bytes[..block_size as usize]);
        if original_cksum != computed_cksum {
            return Err(ApfsError::ChecksumMismatch {
                expected: original_cksum,
                actual: computed_cksum,
            });
        }

        Ok(Self {
            header,
            magic,
            fs_index: u32::from_le_bytes(bytes[36..40].try_into().unwrap()),
            features: u64::from_le_bytes(bytes[40..48].try_into().unwrap()),
            readonly_compat_features: u64::from_le_bytes(bytes[48..56].try_into().unwrap()),
            incompat_features: u64::from_le_bytes(bytes[56..64].try_into().unwrap()),
            uniquifier: u64::from_le_bytes(bytes[64..72].try_into().unwrap()),
            role: u16::from_le_bytes(bytes[72..74].try_into().unwrap()),
            formatted_by: bytes[76..108].try_into().unwrap(),
            meta_crypto: u64::from_le_bytes(bytes[108..116].try_into().unwrap()),
            omap_oid: u64::from_le_bytes(bytes[116..124].try_into().unwrap()),
            root_tree_oid: u64::from_le_bytes(bytes[124..132].try_into().unwrap()),
            extentref_tree_oid: u64::from_le_bytes(bytes[132..140].try_into().unwrap()),
            snap_meta_tree_oid: u64::from_le_bytes(bytes[140..148].try_into().unwrap()),
        })
    }
}

pub fn fletcher64(data: &[u8]) -> u64 {
    const MOD: u64 = 0xffff_ffff;
    let mut sum1: u64 = 0;
    let mut sum2: u64 = 0;

    for chunk in data[8.min(data.len())..].chunks_exact(4) {
        let word = u32::from_le_bytes(chunk.try_into().unwrap());
        sum1 = (sum1 + word as u64) % MOD;
        sum2 = (sum2 + sum1) % MOD;
    }

    let low = (MOD - ((sum1 + sum2) % MOD)) % MOD;
    let high = (MOD - ((sum1 + low) % MOD)) % MOD;
    (high << 32) | low
}

pub fn read_container_superblock<S: BlockSource + ?Sized>(
    source: &S,
    volume_offset: u64,
) -> Result<ApfsContainerSuperblock, ApfsError> {
    // APFS Container Superblock is always in block 0 (which is 4096 bytes or at least 512 bytes)
    // We first read 512 bytes to determine the block size
    let mut buf = vec![0_u8; 4096];
    let res = source
        .read_at(volume_offset, &mut buf)
        .map_err(ApfsError::Read)?;
    if res.bytes_read < 184 {
        return Err(ApfsError::UnexpectedEof);
    }

    let block_size = u32::from_le_bytes(buf[36..40].try_into().unwrap());
    if !(512..=65536).contains(&block_size) || (block_size & (block_size - 1)) != 0 {
        return Err(ApfsError::InvalidBlockSize(block_size));
    }

    if res.bytes_read < block_size as usize {
        // Read the remaining bytes up to block_size
        buf.resize(block_size as usize, 0);
        let res2 = source
            .read_at(volume_offset, &mut buf)
            .map_err(ApfsError::Read)?;
        if res2.bytes_read < block_size as usize {
            return Err(ApfsError::UnexpectedEof);
        }
    }

    ApfsContainerSuperblock::parse(&buf[..block_size as usize])
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BtreeNodeHeader {
    pub flags: u16,
    pub num_keys: u16,
    pub keys_offset: u16,
    pub keys_len: u16,
    pub data_offset: u16,
    pub data_len: u16,
}

impl BtreeNodeHeader {
    pub fn parse(bytes: &[u8; 24]) -> Self {
        Self {
            flags: u16::from_le_bytes(bytes[0..2].try_into().unwrap()),
            num_keys: u16::from_le_bytes(bytes[2..4].try_into().unwrap()),
            keys_offset: u16::from_le_bytes(bytes[4..6].try_into().unwrap()),
            keys_len: u16::from_le_bytes(bytes[6..8].try_into().unwrap()),
            data_offset: u16::from_le_bytes(bytes[8..10].try_into().unwrap()),
            data_len: u16::from_le_bytes(bytes[10..12].try_into().unwrap()),
        }
    }
}

pub fn traverse_btree_node<S: BlockSource + ?Sized>(
    source: &S,
    block_offset: u64,
    block_size: u32,
    results: &mut Vec<(Vec<u8>, Vec<u8>)>,
) -> Result<(), ApfsError> {
    let mut buf = vec![0_u8; block_size as usize];
    let res = source
        .read_at(block_offset, &mut buf)
        .map_err(ApfsError::Read)?;
    if res.bytes_read < block_size as usize {
        return Err(ApfsError::UnexpectedEof);
    }

    // Parse Object Header to confirm structure
    let _header = ObjHeader::parse(buf[0..32].try_into().unwrap());

    // B-Tree node info is at offset 32
    let btn = BtreeNodeHeader::parse(buf[32..56].try_into().unwrap());

    let is_leaf = (btn.flags & 0x2) != 0;

    let keys_start = 56 + btn.keys_offset as usize;
    let data_start = 56 + btn.data_offset as usize;

    for i in 0..btn.num_keys as usize {
        let k_entry_offset = keys_start + i * 4;
        let d_entry_offset = data_start + i * 4;
        if k_entry_offset + 4 > buf.len() || d_entry_offset + 4 > buf.len() {
            break;
        }

        let k_off = u16::from_le_bytes(buf[k_entry_offset..k_entry_offset + 2].try_into().unwrap())
            as usize;
        let k_len = u16::from_le_bytes(
            buf[k_entry_offset + 2..k_entry_offset + 4]
                .try_into()
                .unwrap(),
        ) as usize;

        let d_off = u16::from_le_bytes(buf[d_entry_offset..d_entry_offset + 2].try_into().unwrap())
            as usize;
        let d_len = u16::from_le_bytes(
            buf[d_entry_offset + 2..d_entry_offset + 4]
                .try_into()
                .unwrap(),
        ) as usize;

        let key_pos = keys_start + k_off;

        // Data can be stored growing backwards from the end of block, or growing forwards
        let data_pos = if d_off + d_len <= block_size as usize {
            block_size as usize - d_off
        } else {
            data_start + d_off
        };

        if key_pos + k_len <= buf.len() && data_pos + d_len <= buf.len() {
            let key = buf[key_pos..key_pos + k_len].to_vec();
            let val = buf[data_pos..data_pos + d_len].to_vec();

            if is_leaf {
                results.push((key, val));
            } else {
                // Index node: child block is at val[0..8]
                if val.len() >= 8 {
                    let child_block = u64::from_le_bytes(val[0..8].try_into().unwrap());
                    let child_offset = child_block * block_size as u64;
                    let _ = traverse_btree_node(source, child_offset, block_size, results);
                }
            }
        }
    }

    Ok(())
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ApfsEntry {
    pub is_deleted: bool,
    pub is_directory: bool,
    pub name: String,
    pub size: u64,
    pub inode: u64,
    pub parent_inode: u64,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ApfsVolumeStatus {
    pub fs_oid: u64,
    pub role: u16,
    pub meta_crypto: u64,
    pub omap_oid: u64,
    pub root_tree_oid: u64,
    pub extentref_tree_oid: u64,
    pub snap_meta_tree_oid: u64,
    pub encrypted_hint: bool,
}

pub fn read_apfs_volume_statuses<S: BlockSource + ?Sized>(
    source: &S,
    csb: &ApfsContainerSuperblock,
    volume_offset: u64,
) -> Vec<ApfsVolumeStatus> {
    if let Some(volumes) = spec::index_volumes(source, csb, volume_offset) {
        return volumes
            .iter()
            .map(|index| ApfsVolumeStatus {
                fs_oid: index.volume.fs_oid,
                role: index.volume.role,
                meta_crypto: u64::from(index.volume.encrypted),
                omap_oid: index.volume.omap_oid,
                root_tree_oid: index.volume.root_tree_oid,
                extentref_tree_oid: index.volume.extentref_tree_oid,
                snap_meta_tree_oid: index.volume.snap_meta_tree_oid,
                encrypted_hint: index.volume.encrypted,
            })
            .collect();
    }
    let mut statuses = Vec::new();
    for &fs_oid in &csb.fs_oids {
        let vsb_offset = volume_offset + fs_oid * csb.block_size as u64;
        let mut vsb_buf = vec![0_u8; csb.block_size as usize];
        if source.read_at(vsb_offset, &mut vsb_buf).is_err() {
            continue;
        }
        let Ok(vsb) = ApfsVolumeSuperblock::parse(&vsb_buf, csb.block_size) else {
            continue;
        };
        statuses.push(ApfsVolumeStatus {
            fs_oid,
            role: vsb.role,
            meta_crypto: vsb.meta_crypto,
            omap_oid: vsb.omap_oid,
            root_tree_oid: vsb.root_tree_oid,
            extentref_tree_oid: vsb.extentref_tree_oid,
            snap_meta_tree_oid: vsb.snap_meta_tree_oid,
            encrypted_hint: vsb.meta_crypto != 0,
        });
    }
    statuses
}

/// Scan live entries plus deleted files recovered from stale FS-tree nodes.
/// This reads every block of the container, so it is much slower than
/// [`scan_apfs_vfs`]. Returns only live entries for non-reference layouts.
pub fn scan_apfs_vfs_with_deleted<S: BlockSource + ?Sized>(
    source: &S,
    csb: &ApfsContainerSuperblock,
    volume_offset: u64,
) -> Result<Vec<ApfsEntry>, ApfsError> {
    if let Some(volumes) = spec::index_volumes_with_options(source, csb, volume_offset, true) {
        return Ok(volumes.iter().flat_map(|v| v.entries()).collect());
    }
    scan_apfs_vfs(source, csb, volume_offset)
}

pub fn scan_apfs_vfs<S: BlockSource + ?Sized>(
    source: &S,
    csb: &ApfsContainerSuperblock,
    volume_offset: u64,
) -> Result<Vec<ApfsEntry>, ApfsError> {
    // Real APFS: resolve virtual OIDs through the object maps. The legacy
    // path below only understands the simplified synthetic fixture layout.
    if let Some(volumes) = spec::index_volumes(source, csb, volume_offset) {
        return Ok(volumes.iter().flat_map(|v| v.entries()).collect());
    }
    let mut entries = Vec::new();

    for &fs_oid in &csb.fs_oids {
        // Resolve volume superblock block. OID maps to block index.
        let vsb_offset = volume_offset + fs_oid * csb.block_size as u64;
        let mut vsb_buf = vec![0_u8; csb.block_size as usize];
        if source.read_at(vsb_offset, &mut vsb_buf).is_err() {
            continue;
        }

        let vsb = match ApfsVolumeSuperblock::parse(&vsb_buf, csb.block_size) {
            Ok(v) => v,
            Err(_) => continue,
        };

        // Resolve root tree physical offset
        let root_tree_offset = volume_offset + vsb.root_tree_oid * csb.block_size as u64;

        let mut btree_results = Vec::new();
        if traverse_btree_node(source, root_tree_offset, csb.block_size, &mut btree_results)
            .is_err()
        {
            continue;
        }

        let mut inode_sizes = std::collections::HashMap::new();
        let mut dir_records = Vec::new();

        for (key, val) in btree_results {
            if key.len() < 12 {
                continue;
            }
            let obj_id = u64::from_le_bytes(key[0..8].try_into().unwrap());
            let type_and_flags = u32::from_le_bytes(key[8..12].try_into().unwrap());
            let key_type = (type_and_flags >> 28) & 0xF;

            if key_type == 0x2 {
                // Inode
                if val.len() >= 32 {
                    let size = u64::from_le_bytes(val[24..32].try_into().unwrap());
                    inode_sizes.insert(obj_id, size);
                }
            } else if key_type == 0x3 {
                // Dir Record
                if key.len() >= 14 && val.len() >= 8 {
                    let name_len = u16::from_le_bytes(key[12..14].try_into().unwrap()) as usize;
                    if key.len() >= 14 + name_len {
                        let name = String::from_utf8_lossy(&key[14..14 + name_len]).into_owned();
                        let child_inode = u64::from_le_bytes(val[0..8].try_into().unwrap());
                        let file_type = if val.len() >= 19 { val[18] } else { 8 };
                        let is_dir = file_type == 4;
                        dir_records.push((obj_id, child_inode, name, is_dir));
                    }
                }
            }
        }

        for (parent_inode, child_inode, name, is_dir) in dir_records {
            let size = inode_sizes.get(&child_inode).copied().unwrap_or(0);
            entries.push(ApfsEntry {
                is_deleted: false,
                is_directory: is_dir,
                name,
                size,
                inode: child_inode,
                parent_inode,
            });
        }
    }

    Ok(entries)
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ApfsExtentRead {
    pub logical_offset: u64,
    pub length: u64,
    pub phys_block_num: u64,
    pub bytes_read: u64,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ApfsReadReport {
    pub data: Vec<u8>,
    pub bytes_requested: u64,
    pub bytes_read: u64,
    pub bytes_lost: u64,
    pub extents_found: usize,
    pub extents_used: usize,
    pub holes: Vec<(u64, u64)>,
    pub complete: bool,
    pub encrypted_hint: bool,
    pub sparse_or_missing_extents: bool,
    pub reason: String,
    pub extent_reads: Vec<ApfsExtentRead>,
}

/// Extract file data from APFS extent records and return a detailed quality report.
///
/// This function is intentionally conservative. If the inode has no readable extent records,
/// or if logical ranges are missing, the caller should quarantine the output instead of
/// presenting it as a fully recovered file. APFS compressed files, encrypted volumes, cloned
/// extents and snapshot-only historical records are not fully reconstructed by this MVP parser.
pub fn extract_apfs_file_data_with_report<S: BlockSource + ?Sized>(
    source: &S,
    csb: &ApfsContainerSuperblock,
    volume_offset: u64,
    inode_id: u64,
    file_size: u64,
) -> Result<ApfsReadReport, ApfsError> {
    if file_size > usize::MAX as u64 {
        return Err(ApfsError::UnexpectedEof);
    }

    let mut file_data = vec![0_u8; file_size as usize];
    if file_size == 0 {
        return Ok(ApfsReadReport {
            data: file_data,
            bytes_requested: 0,
            bytes_read: 0,
            bytes_lost: 0,
            extents_found: 0,
            extents_used: 0,
            holes: Vec::new(),
            complete: true,
            encrypted_hint: false,
            sparse_or_missing_extents: false,
            reason: "zero_length_file".to_string(),
            extent_reads: Vec::new(),
        });
    }

    let mut extents: Vec<(u64, u64, u64)> = Vec::new();
    let mut encrypted_hint = false;
    let mut compressed = false;

    let spec_volumes = spec::index_volumes(source, csb, volume_offset);
    let find = |volumes: &[spec::SpecVolumeIndex]| {
        volumes.iter().find_map(|index| {
            index
                .file_extents(inode_id)
                .map(|(found, is_compressed)| (found, is_compressed, index.volume.encrypted))
        })
    };
    if let Some(volumes) = &spec_volumes {
        // Fall back to stale-node records only when the inode is not live.
        let hit = find(volumes).or_else(|| {
            spec::index_volumes_with_options(source, csb, volume_offset, true)
                .and_then(|all| find(&all))
        });
        if let Some((found, is_compressed, encrypted)) = hit {
            extents = found;
            compressed = is_compressed;
            encrypted_hint = encrypted;
        }
    }

    for &fs_oid in spec_volumes
        .is_none()
        .then_some(&csb.fs_oids)
        .into_iter()
        .flatten()
    {
        let vsb_offset = volume_offset + fs_oid * csb.block_size as u64;
        let mut vsb_buf = vec![0_u8; csb.block_size as usize];
        if source.read_at(vsb_offset, &mut vsb_buf).is_err() {
            continue;
        }

        let vsb = match ApfsVolumeSuperblock::parse(&vsb_buf, csb.block_size) {
            Ok(v) => v,
            Err(_) => continue,
        };
        encrypted_hint |= vsb.meta_crypto != 0;

        let root_tree_offset = volume_offset + vsb.root_tree_oid * csb.block_size as u64;
        let mut btree_results = Vec::new();
        if traverse_btree_node(source, root_tree_offset, csb.block_size, &mut btree_results).is_ok()
        {
            for (key, val) in btree_results {
                if key.len() < 20 {
                    continue;
                }
                let obj_id = u64::from_le_bytes(key[0..8].try_into().unwrap());
                let type_and_flags = u32::from_le_bytes(key[8..12].try_into().unwrap());
                let key_type = (type_and_flags >> 28) & 0xF;

                if key_type == 0x8 && obj_id == inode_id && val.len() >= 16 {
                    let logical_offset = u64::from_le_bytes(key[12..20].try_into().unwrap());
                    let len_and_flags = u64::from_le_bytes(val[0..8].try_into().unwrap());
                    let length = len_and_flags & 0x00FF_FFFF_FFFF_FFFF;
                    let phys_block_num = u64::from_le_bytes(val[8..16].try_into().unwrap());
                    if length > 0 {
                        extents.push((logical_offset, length, phys_block_num));
                    }
                }
            }
        }
    }

    extents.sort_by_key(|e| e.0);

    let mut cursor = 0_u64;
    let mut holes = Vec::new();
    let mut bytes_read_total = 0_u64;
    let mut extents_used = 0_usize;
    let mut extent_reads = Vec::new();

    for (logical_offset, length, phys_block_num) in extents.iter().copied() {
        if logical_offset >= file_size {
            continue;
        }
        if logical_offset > cursor {
            holes.push((cursor, logical_offset - cursor));
        }

        let bytes_to_read = length.min(file_size.saturating_sub(logical_offset));
        if bytes_to_read == 0 || phys_block_num == 0 {
            continue;
        }

        let extent_disk_offset = volume_offset + phys_block_num * csb.block_size as u64;
        let mut temp_buf = vec![0_u8; bytes_to_read as usize];
        let read = source
            .read_at(extent_disk_offset, &mut temp_buf)
            .map_err(ApfsError::Read)?;
        let actual = read.bytes_read.min(temp_buf.len()) as u64;
        if actual > 0 {
            let start = logical_offset as usize;
            let end = start + actual as usize;
            if end <= file_data.len() {
                file_data[start..end].copy_from_slice(&temp_buf[..actual as usize]);
                bytes_read_total = bytes_read_total.saturating_add(actual);
                extents_used += 1;
                cursor = cursor.max(logical_offset.saturating_add(actual));
            }
        }
        if actual < bytes_to_read {
            holes.push((logical_offset + actual, bytes_to_read - actual));
        }
        extent_reads.push(ApfsExtentRead {
            logical_offset,
            length: bytes_to_read,
            phys_block_num,
            bytes_read: actual,
        });
    }

    if cursor < file_size {
        holes.push((cursor, file_size - cursor));
    }

    // Merge adjacent/overlapping holes to keep reports readable.
    holes.sort_by_key(|h| h.0);
    let mut merged: Vec<(u64, u64)> = Vec::new();
    for (start, len) in holes {
        if len == 0 {
            continue;
        }
        if let Some(last) = merged.last_mut() {
            let last_end = last.0.saturating_add(last.1);
            if start <= last_end {
                let new_end = last_end.max(start.saturating_add(len));
                last.1 = new_end.saturating_sub(last.0);
                continue;
            }
        }
        merged.push((start, len));
    }

    let bytes_lost = file_size.saturating_sub(bytes_read_total.min(file_size));
    let sparse_or_missing_extents = !merged.is_empty();
    let complete = bytes_read_total >= file_size
        && !sparse_or_missing_extents
        && !encrypted_hint
        && !compressed;
    let reason = if compressed {
        "apfs_transparent_compression_not_supported; output quarantined".to_string()
    } else if encrypted_hint {
        "volume_has_crypto_metadata; output may require unlocked/decrypted APFS context".to_string()
    } else if extents.is_empty() {
        "no_extent_records_found_for_inode".to_string()
    } else if sparse_or_missing_extents {
        "logical_ranges_missing_or_unreadable; quarantined as partial/suspect".to_string()
    } else if complete {
        "all_extent_ranges_read".to_string()
    } else {
        "partial_extent_read".to_string()
    };

    Ok(ApfsReadReport {
        data: file_data,
        bytes_requested: file_size,
        bytes_read: bytes_read_total.min(file_size),
        bytes_lost,
        extents_found: extents.len(),
        extents_used,
        holes: merged,
        complete,
        encrypted_hint,
        sparse_or_missing_extents,
        reason,
        extent_reads,
    })
}

pub fn extract_apfs_file_data<S: BlockSource + ?Sized>(
    source: &S,
    csb: &ApfsContainerSuperblock,
    volume_offset: u64,
    inode_id: u64,
    file_size: u64,
) -> Result<Vec<u8>, ApfsError> {
    extract_apfs_file_data_with_report(source, csb, volume_offset, inode_id, file_size)
        .map(|report| report.data)
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MemBlockSource {
        device: data_recovery_core::PhysicalDevice,
        data: Vec<u8>,
    }

    impl MemBlockSource {
        fn new(data: Vec<u8>) -> Self {
            let device = data_recovery_core::PhysicalDevice {
                id: "mem".to_string(),
                display_name: "Memory Source".to_string(),
                platform_path: "/dev/mem".to_string(),
                size_bytes: data.len() as u64,
                logical_sector_size: 512,
                physical_sector_size: 512,
                bus_type: data_recovery_core::BusType::Sata,
                is_removable: true,
                is_system_disk: false,
                trim_supported: None,
            };
            Self { device, data }
        }
    }

    impl BlockSource for MemBlockSource {
        fn read_at(
            &self,
            offset: u64,
            buffer: &mut [u8],
        ) -> Result<data_recovery_core::ReadResult, ReadError> {
            let offset_idx = offset as usize;
            if offset_idx >= self.data.len() {
                return Ok(data_recovery_core::ReadResult {
                    offset,
                    bytes_read: 0,
                });
            }
            let to_read = std::cmp::min(buffer.len(), self.data.len() - offset_idx);
            buffer[..to_read].copy_from_slice(&self.data[offset_idx..offset_idx + to_read]);
            Ok(data_recovery_core::ReadResult {
                offset,
                bytes_read: to_read,
            })
        }

        fn device(&self) -> &data_recovery_core::PhysicalDevice {
            &self.device
        }
    }

    fn create_mock_block(magic: &[u8; 4], block_size: usize) -> Vec<u8> {
        let mut block = vec![0_u8; block_size];
        // Object ID = 1
        block[8..16].copy_from_slice(&1_u64.to_le_bytes());
        // Transaction ID = 100
        block[16..24].copy_from_slice(&100_u64.to_le_bytes());
        // Object Type = 1 (NXSB)
        block[24..28].copy_from_slice(&1_u32.to_le_bytes());

        // Magic
        block[32..36].copy_from_slice(magic);
        // Block size
        block[36..40].copy_from_slice(&(block_size as u32).to_le_bytes());

        if magic == b"NXSB" {
            // max_file_systems = 1
            block[180..184].copy_from_slice(&1_u32.to_le_bytes());
            // fs_oids[0] = 2 (Volume superblock is at block 2)
            block[184..192].copy_from_slice(&2_u64.to_le_bytes());
        }

        let cksum = fletcher64(&block);
        block[0..8].copy_from_slice(&cksum.to_le_bytes());
        block
    }

    #[test]
    fn parses_apfs_container_superblock() {
        let block_size = 4096;
        let mock_data = create_mock_block(b"NXSB", block_size);
        let csb = ApfsContainerSuperblock::parse(&mock_data).unwrap();
        assert_eq!(csb.magic, *b"NXSB");
        assert_eq!(csb.block_size, block_size as u32);
        assert_eq!(csb.max_file_systems, 1);
        assert_eq!(csb.fs_oids, vec![2]);
    }

    #[test]
    fn parses_apfs_volume_superblock() {
        let block_size = 4096;
        let mock_data = create_mock_block(b"APFS", block_size);
        let vsb = ApfsVolumeSuperblock::parse(&mock_data, block_size as u32).unwrap();
        assert_eq!(vsb.magic, *b"APFS");
    }

    #[test]
    fn rejects_checksum_mismatch() {
        let block_size = 4096;
        let mut mock_data = create_mock_block(b"NXSB", block_size);
        mock_data[50] ^= 0xFF;
        let result = ApfsContainerSuperblock::parse(&mock_data);
        assert!(matches!(result, Err(ApfsError::ChecksumMismatch { .. })));
    }

    #[test]
    fn test_scan_apfs_vfs_mock() {
        let block_size = 4096;
        let mut disk = vec![0_u8; block_size * 5];

        // 1. Container superblock at block 0
        let csb_data = create_mock_block(b"NXSB", block_size);
        disk[0..block_size].copy_from_slice(&csb_data);

        // 2. Volume superblock at block 2
        let mut vsb_data = create_mock_block(b"APFS", block_size);
        // Set root_tree_oid = 3
        vsb_data[124..132].copy_from_slice(&3_u64.to_le_bytes());
        vsb_data[0..8].fill(0);
        let vsb_cksum = fletcher64(&vsb_data);
        vsb_data[0..8].copy_from_slice(&vsb_cksum.to_le_bytes());
        disk[block_size * 2..block_size * 3].copy_from_slice(&vsb_data);

        // 3. Root tree B-Tree leaf node at block 3
        let mut btree_node = vec![0_u8; block_size];
        // Obj Header: oid = 3
        btree_node[8..16].copy_from_slice(&3_u64.to_le_bytes());

        // Btree Node: flags = 0x2 (Leaf node), num_keys = 2, keys_offset = 0, keys_len = 8, data_offset = 100, data_len = 8
        btree_node[32..34].copy_from_slice(&0x2_u16.to_le_bytes()); // flags
        btree_node[34..36].copy_from_slice(&2_u16.to_le_bytes()); // num_keys
        btree_node[36..38].copy_from_slice(&0_u16.to_le_bytes()); // keys_offset
        btree_node[38..40].copy_from_slice(&8_u16.to_le_bytes()); // keys_len
        btree_node[40..42].copy_from_slice(&100_u16.to_le_bytes()); // data_offset
        btree_node[42..44].copy_from_slice(&8_u16.to_le_bytes()); // data_len

        let keys_area_start = 56;
        let data_area_start = 156; // 56 + 100

        // Entry 0: Key pointing to child (type 0x3, Dir Record) parent = 101, child_inode = 202, name = "file.txt"
        // key_off = 8, key_len = 22
        btree_node[keys_area_start..keys_area_start + 2].copy_from_slice(&8_u16.to_le_bytes());
        btree_node[keys_area_start + 2..keys_area_start + 4].copy_from_slice(&22_u16.to_le_bytes());

        // data_off = 20, data_len = 20 (grows backwards from end of block to block_size-20)
        btree_node[data_area_start..data_area_start + 2].copy_from_slice(&20_u16.to_le_bytes());
        btree_node[data_area_start + 2..data_area_start + 4].copy_from_slice(&20_u16.to_le_bytes());

        // Entry 1: Key of child inode size metadata (type 0x2, Inode), inode = 202
        // key_off = 30, key_len = 12
        btree_node[keys_area_start + 4..keys_area_start + 6].copy_from_slice(&30_u16.to_le_bytes());
        btree_node[keys_area_start + 6..keys_area_start + 8].copy_from_slice(&12_u16.to_le_bytes());

        // data_off = 52, data_len = 32 (grows backwards to block_size-52)
        btree_node[data_area_start + 4..data_area_start + 6].copy_from_slice(&52_u16.to_le_bytes());
        btree_node[data_area_start + 6..data_area_start + 8].copy_from_slice(&32_u16.to_le_bytes());

        // Put keys into keys area:
        // Key 0 (starts at 56 + 8 = 64): parent_inode = 101 (u64), type_and_flags = 0x30000000 (type = 0x3), name_len = 8, name = "file.txt"
        let key_0_start = keys_area_start + 8;
        btree_node[key_0_start..key_0_start + 8].copy_from_slice(&101_u64.to_le_bytes());
        btree_node[key_0_start + 8..key_0_start + 12]
            .copy_from_slice(&0x30000000_u32.to_le_bytes());
        btree_node[key_0_start + 12..key_0_start + 14].copy_from_slice(&8_u16.to_le_bytes());
        btree_node[key_0_start + 14..key_0_start + 22].copy_from_slice(b"file.txt");

        // Key 1 (starts at 56 + 30 = 86): inode = 202, type_and_flags = 0x20000000 (type = 0x2)
        let key_1_start = keys_area_start + 30;
        btree_node[key_1_start..key_1_start + 8].copy_from_slice(&202_u64.to_le_bytes());
        btree_node[key_1_start + 8..key_1_start + 12]
            .copy_from_slice(&0x20000000_u32.to_le_bytes());

        // Put values into values area (backward growing layout):
        // Value 0 (starts at block_size - 20 = 4076): child_inode = 202 (u64), date_added = 0 (u64), flags = 0, file_type = 8 (regular file)
        let val_0_start = block_size - 20;
        btree_node[val_0_start..val_0_start + 8].copy_from_slice(&202_u64.to_le_bytes());
        btree_node[val_0_start + 18] = 8; // file_type = regular file

        // Value 1 (starts at block_size - 52 = 4044): size = 5000 (u64) at offset 24
        let val_1_start = block_size - 52;
        btree_node[val_1_start + 24..val_1_start + 32].copy_from_slice(&5000_u64.to_le_bytes());

        let btree_cksum = fletcher64(&btree_node);
        btree_node[0..8].copy_from_slice(&btree_cksum.to_le_bytes());
        disk[block_size * 3..block_size * 4].copy_from_slice(&btree_node);

        let source = MemBlockSource::new(disk);
        let csb = read_container_superblock(&source, 0).unwrap();
        let entries = scan_apfs_vfs(&source, &csb, 0).unwrap();

        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].name, "file.txt");
        assert_eq!(entries[0].size, 5000);
        assert_eq!(entries[0].inode, 202);
        assert_eq!(entries[0].parent_inode, 101);
        assert!(!entries[0].is_directory);
    }

    #[test]
    fn test_extract_apfs_file_data_mock() {
        let block_size = 4096;
        let mut disk = vec![0_u8; block_size * 5];

        // 1. Container superblock at block 0
        let csb_data = create_mock_block(b"NXSB", block_size);
        disk[0..block_size].copy_from_slice(&csb_data);

        // 2. Volume superblock at block 2
        let mut vsb_data = create_mock_block(b"APFS", block_size);
        // Set root_tree_oid = 3
        vsb_data[124..132].copy_from_slice(&3_u64.to_le_bytes());
        vsb_data[0..8].fill(0);
        let vsb_cksum = fletcher64(&vsb_data);
        vsb_data[0..8].copy_from_slice(&vsb_cksum.to_le_bytes());
        disk[block_size * 2..block_size * 3].copy_from_slice(&vsb_data);

        // 3. Root tree B-Tree leaf node at block 3 containing the extent record
        let mut btree_node = vec![0_u8; block_size];
        btree_node[8..16].copy_from_slice(&3_u64.to_le_bytes());

        // Btree Node: flags = 0x2 (Leaf node), num_keys = 1, keys_offset = 0, keys_len = 20, data_offset = 100, data_len = 24
        btree_node[32..34].copy_from_slice(&0x2_u16.to_le_bytes()); // flags
        btree_node[34..36].copy_from_slice(&1_u16.to_le_bytes()); // num_keys
        btree_node[36..38].copy_from_slice(&0_u16.to_le_bytes()); // keys_offset
        btree_node[38..40].copy_from_slice(&20_u16.to_le_bytes()); // keys_len
        btree_node[40..42].copy_from_slice(&100_u16.to_le_bytes()); // data_offset
        btree_node[42..44].copy_from_slice(&24_u16.to_le_bytes()); // data_len

        let keys_area_start = 56;
        let data_area_start = 156;

        // Entry 0: key_off = 8, key_len = 20
        btree_node[keys_area_start..keys_area_start + 2].copy_from_slice(&8_u16.to_le_bytes());
        btree_node[keys_area_start + 2..keys_area_start + 4].copy_from_slice(&20_u16.to_le_bytes());

        // data_off = 24, data_len = 24
        btree_node[data_area_start..data_area_start + 2].copy_from_slice(&24_u16.to_le_bytes());
        btree_node[data_area_start + 2..data_area_start + 4].copy_from_slice(&24_u16.to_le_bytes());

        // Key (starts at 56 + 8 = 64): obj_id = 202, key_type = 0x8, logical_offset = 0
        let key_0_start = keys_area_start + 8;
        btree_node[key_0_start..key_0_start + 8].copy_from_slice(&202_u64.to_le_bytes());
        btree_node[key_0_start + 8..key_0_start + 12]
            .copy_from_slice(&0x80000000_u32.to_le_bytes());
        btree_node[key_0_start + 12..key_0_start + 20].copy_from_slice(&0_u64.to_le_bytes());

        // Value (starts at block_size - 24 = 4072): length = 12, phys_block_num = 4, crypto_id = 0
        let val_start = block_size - 24;
        btree_node[val_start..val_start + 8].copy_from_slice(&12_u64.to_le_bytes());
        btree_node[val_start + 8..val_start + 16].copy_from_slice(&4_u64.to_le_bytes());

        let btree_cksum = fletcher64(&btree_node);
        btree_node[0..8].copy_from_slice(&btree_cksum.to_le_bytes());
        disk[block_size * 3..block_size * 4].copy_from_slice(&btree_node);

        // 4. File data block at block 4
        let file_content = b"Hello, APFS!";
        disk[block_size * 4..block_size * 4 + file_content.len()].copy_from_slice(file_content);

        let source = MemBlockSource::new(disk);
        let csb = read_container_superblock(&source, 0).unwrap();
        let data = extract_apfs_file_data(&source, &csb, 0, 202, 12).unwrap();

        assert_eq!(data, file_content);

        let report = extract_apfs_file_data_with_report(&source, &csb, 0, 202, 12).unwrap();
        assert!(report.complete);
        assert_eq!(report.bytes_requested, 12);
        assert_eq!(report.bytes_read, 12);
        assert_eq!(report.bytes_lost, 0);
        assert_eq!(report.extents_found, 1);
        assert_eq!(report.extents_used, 1);
        assert!(report.holes.is_empty());
        assert_eq!(report.data, file_content);
    }
}
