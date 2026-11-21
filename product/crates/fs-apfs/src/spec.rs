//! Apple File System parsing that follows the published APFS reference layout.
//!
//! Unlike the legacy "APFS-like" path in `lib.rs`, this module resolves
//! virtual object identifiers through the container and volume object maps,
//! picks the newest valid checkpoint superblock, walks B-tree nodes with both
//! fixed- and variable-size table-of-contents entries, honours the root-node
//! `btree_info_t` footer and decodes file-system records using the real
//! `j_key_t` encoding (60-bit object id, 4-bit record type).
//!
//! Every read is bounds-checked and every object checksum is verified, so
//! corrupt or non-APFS input yields `None`/errors instead of panics.

use crate::{ApfsContainerSuperblock, ApfsEntry, fletcher64};
use data_recovery_core::BlockSource;
use std::collections::HashMap;

const OBJECT_TYPE_MASK: u32 = 0x0000_ffff;
const OBJECT_TYPE_NX_SUPERBLOCK: u32 = 0x0000_0001;
const OBJECT_TYPE_BTREE: u32 = 0x0000_0002;
const OBJECT_TYPE_BTREE_NODE: u32 = 0x0000_0003;
const OBJECT_TYPE_OMAP: u32 = 0x0000_000b;
const OBJECT_TYPE_FS: u32 = 0x0000_000d;

const BTNODE_ROOT: u16 = 0x0001;
const BTNODE_LEAF: u16 = 0x0002;
const BTNODE_FIXED_KV_SIZE: u16 = 0x0004;
const BTREE_NODE_HEADER: usize = 56;
const BTREE_INFO_SIZE: usize = 40;
const MAX_TREE_DEPTH: usize = 16;

const OBJ_ID_MASK: u64 = 0x0fff_ffff_ffff_ffff;
const APFS_TYPE_INODE: u64 = 3;
const APFS_TYPE_FILE_EXTENT: u64 = 8;
const APFS_TYPE_DIR_REC: u64 = 9;

const INO_EXT_TYPE_DSTREAM: u8 = 8;
const DREC_TYPE_MASK: u16 = 0x000f;
const DT_DIR: u16 = 4;
const APFS_FS_UNENCRYPTED: u64 = 0x1;
const UF_COMPRESSED: u32 = 0x20;
const ROOT_DIR_PARENT: u64 = 1;
const ROOT_DIR_INO_NUM: u64 = 2;

/// `(logical_offset, length, physical_block)` of one file extent.
pub type FileExtent = (u64, u64, u64);

/// Volume-level facts needed by scan/extract callers.
#[derive(Debug, Clone)]
pub struct SpecVolume {
    pub fs_oid: u64,
    pub role: u16,
    pub encrypted: bool,
    pub name: String,
    pub omap_oid: u64,
    pub root_tree_oid: u64,
    pub extentref_tree_oid: u64,
    pub snap_meta_tree_oid: u64,
}

#[derive(Debug, Clone, Default)]
struct Inode {
    parent: u64,
    private_id: u64,
    size: u64,
    compressed: bool,
    is_dir: bool,
}

/// A fully indexed APFS volume tree.
#[derive(Debug, Clone)]
pub struct SpecVolumeIndex {
    pub volume: SpecVolume,
    inodes: HashMap<u64, Inode>,
    drecs: Vec<(u64, u64, String, bool)>,
    extents: HashMap<u64, Vec<FileExtent>>,
}

impl SpecVolumeIndex {
    pub fn entries(&self) -> Vec<ApfsEntry> {
        self.drecs
            .iter()
            .map(|(parent, child, name, is_dir)| {
                let inode = self.inodes.get(child);
                ApfsEntry {
                    is_deleted: false,
                    is_directory: *is_dir || inode.is_some_and(|i| i.is_dir),
                    name: name.clone(),
                    size: inode.map(|i| i.size).unwrap_or(0),
                    inode: *child,
                    // The root directory (inode 2) has no directory record of
                    // its own; report its children as top-level entries.
                    parent_inode: if *parent == ROOT_DIR_INO_NUM {
                        ROOT_DIR_PARENT
                    } else {
                        *parent
                    },
                }
            })
            .collect()
    }

    /// File extents `(logical_offset, length, physical_block)` for an inode,
    /// plus whether the inode uses transparent compression.
    pub fn file_extents(&self, inode_id: u64) -> Option<(Vec<FileExtent>, bool)> {
        let inode = self.inodes.get(&inode_id)?;
        let stream = if inode.private_id != 0 {
            inode.private_id
        } else {
            inode_id
        };
        let mut extents = self.extents.get(&stream).cloned().unwrap_or_default();
        extents.sort_by_key(|extent| extent.0);
        Some((extents, inode.compressed))
    }
}

struct Reader<'a, S: BlockSource + ?Sized> {
    source: &'a S,
    container_offset: u64,
    block_size: usize,
}

impl<S: BlockSource + ?Sized> Reader<'_, S> {
    fn block(&self, paddr: u64) -> Option<Vec<u8>> {
        let offset = paddr
            .checked_mul(self.block_size as u64)?
            .checked_add(self.container_offset)?;
        let mut buf = vec![0_u8; self.block_size];
        let read = self.source.read_at(offset, &mut buf).ok()?;
        if read.bytes_read != buf.len() {
            return None;
        }
        Some(buf)
    }

    fn object(&self, paddr: u64, expected_type: u32) -> Option<Vec<u8>> {
        let buf = self.block(paddr)?;
        let cksum = le_u64(&buf, 0)?;
        if cksum == 0 || cksum != fletcher64(&buf) {
            return None;
        }
        let obj_type = le_u32(&buf, 24)? & OBJECT_TYPE_MASK;
        if obj_type != expected_type {
            return None;
        }
        Some(buf)
    }

    /// Look up `oid` in the object map whose B-tree root is at `tree_paddr`,
    /// returning the physical address of the newest version with xid <= `xid`.
    fn omap_lookup(&self, tree_paddr: u64, oid: u64, xid: u64) -> Option<u64> {
        let mut paddr = tree_paddr;
        for _ in 0..MAX_TREE_DEPTH {
            let node = self.node(paddr)?;
            let mut best: Option<(u64, u64, Vec<u8>)> = None;
            for (key, value) in node.records() {
                let key_oid = le_u64(key, 0)?;
                let key_xid = le_u64(key, 8)?;
                let candidate = if node.is_leaf() {
                    key_oid == oid && key_xid <= xid
                } else {
                    (key_oid, key_xid) <= (oid, xid)
                };
                if candidate
                    && best
                        .as_ref()
                        .is_none_or(|b| (key_oid, key_xid) >= (b.0, b.1))
                {
                    best = Some((key_oid, key_xid, value.to_vec()));
                }
            }
            let (_, _, value) = best?;
            if node.is_leaf() {
                // omap_val_t: ov_flags u32, ov_size u32, ov_paddr u64
                return le_u64(&value, 8);
            }
            paddr = le_u64(&value, 0)?;
        }
        None
    }

    fn omap_tree_root(&self, omap_paddr: u64) -> Option<u64> {
        let omap = self.object(omap_paddr, OBJECT_TYPE_OMAP)?;
        le_u64(&omap, 48)
    }

    fn node(&self, paddr: u64) -> Option<Node> {
        let buf = self.block(paddr)?;
        let cksum = le_u64(&buf, 0)?;
        if cksum != fletcher64(&buf) {
            return None;
        }
        let obj_type = le_u32(&buf, 24)? & OBJECT_TYPE_MASK;
        if obj_type != OBJECT_TYPE_BTREE && obj_type != OBJECT_TYPE_BTREE_NODE {
            return None;
        }
        Node::parse(buf)
    }

    /// Visit every leaf record of a file-system tree whose child pointers are
    /// virtual OIDs resolved through the volume object map.
    fn walk_fs_tree(
        &self,
        root_paddr: u64,
        omap_root: u64,
        xid: u64,
        visit: &mut dyn FnMut(&[u8], &[u8]),
    ) -> Option<()> {
        let mut stack = vec![(root_paddr, 0_usize)];
        let mut visited = 0_usize;
        while let Some((paddr, depth)) = stack.pop() {
            visited += 1;
            if depth > MAX_TREE_DEPTH || visited > 4_000_000 {
                return None;
            }
            let node = self.node(paddr)?;
            if node.is_leaf() {
                for (key, value) in node.records() {
                    visit(key, value);
                }
                continue;
            }
            let mut children = Vec::new();
            for (_, value) in node.records() {
                let child_oid = le_u64(value, 0)?;
                children.push(self.omap_lookup(omap_root, child_oid, xid)?);
            }
            for child in children.into_iter().rev() {
                stack.push((child, depth + 1));
            }
        }
        Some(())
    }
}

struct Node {
    buf: Vec<u8>,
    flags: u16,
    nkeys: usize,
    toc_start: usize,
    key_start: usize,
    value_end: usize,
}

impl Node {
    fn parse(buf: Vec<u8>) -> Option<Self> {
        let flags = le_u16(&buf, 32)?;
        let nkeys = le_u32(&buf, 36)? as usize;
        let toc_off = le_u16(&buf, 40)? as usize;
        let toc_len = le_u16(&buf, 42)? as usize;
        let toc_start = BTREE_NODE_HEADER.checked_add(toc_off)?;
        let key_start = toc_start.checked_add(toc_len)?;
        let value_end = if flags & BTNODE_ROOT != 0 {
            buf.len().checked_sub(BTREE_INFO_SIZE)?
        } else {
            buf.len()
        };
        if key_start > value_end || nkeys > buf.len() {
            return None;
        }
        Some(Self {
            buf,
            flags,
            nkeys,
            toc_start,
            key_start,
            value_end,
        })
    }

    fn is_leaf(&self) -> bool {
        self.flags & BTNODE_LEAF != 0
    }

    fn records(&self) -> Vec<(&[u8], &[u8])> {
        let fixed = self.flags & BTNODE_FIXED_KV_SIZE != 0;
        let mut out = Vec::with_capacity(self.nkeys);
        for index in 0..self.nkeys {
            let entry = self.toc_start + index * if fixed { 4 } else { 8 };
            let parsed = if fixed {
                // kvoff_t; omap trees use 16-byte keys, 16-byte leaf values
                // and 8-byte child pointers in index nodes.
                let k = le_u16(&self.buf, entry).map(usize::from);
                let v = le_u16(&self.buf, entry + 2).map(usize::from);
                let vlen = if self.is_leaf() { 16 } else { 8 };
                k.zip(v).map(|(k, v)| (k, 16, v, vlen))
            } else {
                let k = le_u16(&self.buf, entry).map(usize::from);
                let klen = le_u16(&self.buf, entry + 2).map(usize::from);
                let v = le_u16(&self.buf, entry + 4).map(usize::from);
                let vlen = le_u16(&self.buf, entry + 6).map(usize::from);
                match (k, klen, v, vlen) {
                    (Some(k), Some(kl), Some(v), Some(vl)) => Some((k, kl, v, vl)),
                    _ => None,
                }
            };
            let Some((k_off, k_len, v_off, v_len)) = parsed else {
                break;
            };
            let key_pos = self.key_start + k_off;
            let Some(value_pos) = self.value_end.checked_sub(v_off) else {
                continue;
            };
            let key = self.buf.get(key_pos..key_pos + k_len);
            let value = self.buf.get(value_pos..value_pos + v_len);
            if let (Some(key), Some(value)) = (key, value) {
                if value_pos + v_len <= self.value_end {
                    out.push((key, value));
                }
            }
        }
        out
    }
}

/// Pick the newest valid container superblock from the checkpoint descriptor
/// area, falling back to block 0.
fn latest_container_superblock<S: BlockSource + ?Sized>(
    reader: &Reader<'_, S>,
    block_zero: &ApfsContainerSuperblock,
) -> Option<ApfsContainerSuperblock> {
    let mut best = ApfsContainerSuperblock::parse(&reader.block(0)?).ok();
    // A set high bit means the descriptor area is itself a B-tree; only the
    // common contiguous layout is scanned here.
    if block_zero.xp_desc_blocks & 0x8000_0000 == 0 {
        for index in 0..u64::from(block_zero.xp_desc_blocks.min(4096)) {
            let Some(buf) =
                reader.object(block_zero.xp_desc_base + index, OBJECT_TYPE_NX_SUPERBLOCK)
            else {
                continue;
            };
            if let Ok(candidate) = ApfsContainerSuperblock::parse(&buf) {
                if best
                    .as_ref()
                    .is_none_or(|b| candidate.header.xid > b.header.xid)
                {
                    best = Some(candidate);
                }
            }
        }
    }
    best
}

/// Index every volume of the container using the APFS reference layout.
/// Returns `None` when the object maps cannot be resolved, which tells the
/// caller this is not a real APFS container (or is too damaged for this path).
pub fn index_volumes<S: BlockSource + ?Sized>(
    source: &S,
    csb: &ApfsContainerSuperblock,
    container_offset: u64,
) -> Option<Vec<SpecVolumeIndex>> {
    let reader = Reader {
        source,
        container_offset,
        block_size: csb.block_size as usize,
    };
    let nx = latest_container_superblock(&reader, csb)?;
    let xid = nx.header.xid;
    let container_omap = reader.omap_tree_root(nx.omap_oid)?;

    let mut volumes = Vec::new();
    for &fs_oid in nx.fs_oids.iter().filter(|oid| **oid != 0) {
        let Some(vsb_paddr) = reader.omap_lookup(container_omap, fs_oid, xid) else {
            continue;
        };
        let Some(vsb) = reader.object(vsb_paddr, OBJECT_TYPE_FS) else {
            continue;
        };
        // APFS_MAGIC is the little-endian u32 0x42535041, i.e. bytes "APSB".
        if vsb.get(32..36) != Some(b"APSB".as_slice()) {
            continue;
        }
        let field = |offset| le_u64(&vsb, offset).unwrap_or(0);
        let volume = SpecVolume {
            fs_oid,
            role: le_u16(&vsb, 964).unwrap_or(0),
            encrypted: field(264) & APFS_FS_UNENCRYPTED == 0,
            name: c_string(vsb.get(704..960).unwrap_or(&[])),
            omap_oid: field(128),
            root_tree_oid: field(136),
            extentref_tree_oid: field(144),
            snap_meta_tree_oid: field(152),
        };
        let index = index_volume(&reader, volume, xid);
        if let Some(index) = index {
            volumes.push(index);
        }
    }
    (!volumes.is_empty()).then_some(volumes)
}

fn index_volume<S: BlockSource + ?Sized>(
    reader: &Reader<'_, S>,
    volume: SpecVolume,
    xid: u64,
) -> Option<SpecVolumeIndex> {
    let omap_root = reader.omap_tree_root(volume.omap_oid)?;
    let root_paddr = reader.omap_lookup(omap_root, volume.root_tree_oid, xid)?;

    let mut inodes: HashMap<u64, Inode> = HashMap::new();
    let mut drecs = Vec::new();
    let mut extents: HashMap<u64, Vec<FileExtent>> = HashMap::new();
    reader.walk_fs_tree(root_paddr, omap_root, xid, &mut |key, value| {
        let Some(header) = le_u64(key, 0) else {
            return;
        };
        let obj_id = header & OBJ_ID_MASK;
        match header >> 60 {
            APFS_TYPE_INODE => {
                if let Some(inode) = parse_inode(value) {
                    inodes.insert(obj_id, inode);
                }
            }
            APFS_TYPE_DIR_REC => {
                if let Some((name, child, is_dir)) = parse_drec(key, value) {
                    if obj_id != ROOT_DIR_PARENT {
                        drecs.push((obj_id, child, name, is_dir));
                    }
                }
            }
            APFS_TYPE_FILE_EXTENT => {
                let logical = le_u64(key, 8);
                let len = le_u64(value, 0).map(|v| v & 0x00ff_ffff_ffff_ffff);
                let phys = le_u64(value, 8);
                if let (Some(logical), Some(len), Some(phys)) = (logical, len, phys) {
                    if len > 0 {
                        extents
                            .entry(obj_id)
                            .or_default()
                            .push((logical, len, phys));
                    }
                }
            }
            _ => {}
        }
    })?;
    Some(SpecVolumeIndex {
        volume,
        inodes,
        drecs,
        extents,
    })
}

fn parse_inode(value: &[u8]) -> Option<Inode> {
    let mode = le_u16(value, 80)?;
    let bsd_flags = le_u32(value, 68)?;
    let mut inode = Inode {
        parent: le_u64(value, 0)?,
        private_id: le_u64(value, 8)?,
        size: 0,
        compressed: bsd_flags & UF_COMPRESSED != 0,
        is_dir: mode & 0o170000 == 0o040000,
    };
    // xf_blob_t at offset 92: count, used bytes, then x_field_t headers and
    // 8-byte aligned payloads.
    if let (Some(count), Some(_used)) = (le_u16(value, 92), le_u16(value, 94)) {
        let headers = 96;
        let mut data = headers + usize::from(count) * 4;
        for index in 0..usize::from(count) {
            let h = headers + index * 4;
            let (Some(&x_type), Some(size)) = (value.get(h), le_u16(value, h + 2)) else {
                break;
            };
            if x_type == INO_EXT_TYPE_DSTREAM {
                if let Some(size) = le_u64(value, data) {
                    inode.size = size;
                }
            }
            data += usize::from(size).div_ceil(8) * 8;
        }
    }
    let _ = inode.parent;
    Some(inode)
}

fn parse_drec(key: &[u8], value: &[u8]) -> Option<(String, u64, bool)> {
    let child = le_u64(value, 0)?;
    let flags = le_u16(value, 16)?;
    // j_drec_hashed_key_t: low 10 bits of name_len_and_hash give the length
    // including the trailing NUL. Older volumes use a plain u16 length.
    let hashed_len = le_u32(key, 8).map(|v| (v & 0x3ff) as usize);
    let name_bytes = match hashed_len {
        Some(len) if 12 + len == key.len() => key.get(12..12 + len)?,
        _ => {
            let len = le_u16(key, 8)? as usize;
            key.get(10..10 + len)?
        }
    };
    let name = c_string(name_bytes);
    if name.is_empty() {
        return None;
    }
    Some((name, child, flags & DREC_TYPE_MASK == DT_DIR))
}

fn c_string(bytes: &[u8]) -> String {
    let end = bytes.iter().position(|b| *b == 0).unwrap_or(bytes.len());
    String::from_utf8_lossy(&bytes[..end]).into_owned()
}

fn le_u16(bytes: &[u8], offset: usize) -> Option<u16> {
    Some(u16::from_le_bytes(
        bytes.get(offset..offset + 2)?.try_into().ok()?,
    ))
}

fn le_u32(bytes: &[u8], offset: usize) -> Option<u32> {
    Some(u32::from_le_bytes(
        bytes.get(offset..offset + 4)?.try_into().ok()?,
    ))
}

fn le_u64(bytes: &[u8], offset: usize) -> Option<u64> {
    Some(u64::from_le_bytes(
        bytes.get(offset..offset + 8)?.try_into().ok()?,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_hashed_directory_record() {
        let name = "中文檔名.txt\0".as_bytes();
        let mut key = Vec::new();
        key.extend_from_slice(&((APFS_TYPE_DIR_REC << 60) | 2).to_le_bytes());
        key.extend_from_slice(&((0xabcde_u32 << 10) | name.len() as u32).to_le_bytes());
        key.extend_from_slice(name);
        let mut value = vec![0_u8; 18];
        value[0..8].copy_from_slice(&42_u64.to_le_bytes());
        value[16..18].copy_from_slice(&8_u16.to_le_bytes());
        assert_eq!(
            parse_drec(&key, &value),
            Some(("中文檔名.txt".to_string(), 42, false))
        );
        value[16..18].copy_from_slice(&DT_DIR.to_le_bytes());
        assert!(parse_drec(&key, &value).unwrap().2);
    }

    #[test]
    fn reads_inode_size_from_dstream_xfield() {
        let mut value = vec![0_u8; 92];
        value[8..16].copy_from_slice(&77_u64.to_le_bytes());
        value[80..82].copy_from_slice(&0o100644_u16.to_le_bytes());
        // two xfields: a 3-byte name (padded to 8) then a 40-byte dstream
        value.extend_from_slice(&2_u16.to_le_bytes());
        value.extend_from_slice(&48_u16.to_le_bytes());
        value.extend_from_slice(&[4, 0, 3, 0]);
        value.extend_from_slice(&[INO_EXT_TYPE_DSTREAM, 0, 40, 0]);
        value.extend_from_slice(b"ab\0\0\0\0\0\0");
        let mut dstream = [0_u8; 40];
        dstream[0..8].copy_from_slice(&5_242_880_u64.to_le_bytes());
        value.extend_from_slice(&dstream);
        let inode = parse_inode(&value).unwrap();
        assert_eq!(inode.size, 5_242_880);
        assert_eq!(inode.private_id, 77);
        assert!(!inode.is_dir);
        assert!(!inode.compressed);
    }

    #[test]
    fn malformed_inputs_do_not_panic() {
        assert!(parse_inode(&[0_u8; 10]).is_none());
        assert!(parse_drec(&[1, 2, 3], &[0_u8; 4]).is_none());
        assert!(Node::parse(vec![0xff_u8; 64]).is_none());
    }
}
