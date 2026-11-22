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
const OBJECT_TYPE_FSTREE: u32 = 0x0000_000e;

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
const INO_EXT_TYPE_NAME: u8 = 4;
const S_IFMT: u16 = 0o170000;
const S_IFREG: u16 = 0o100000;
/// Synthetic directory that holds files rebuilt from node residue, whose
/// parent directory cannot be recovered.
pub const RESIDUE_DIR_INODE: u64 = 0x0fff_ffff_ffff_fff0;
pub const RESIDUE_DIR_NAME: &str = "_residue_recovered";

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
    block_size: u64,
    inodes: HashMap<u64, Inode>,
    drecs: Vec<(u64, u64, String, bool)>,
    extents: HashMap<u64, Vec<FileExtent>>,
    /// Records recovered from stale (no longer referenced) FS-tree leaf
    /// nodes; only populated when deleted-file scanning was requested.
    deleted_inodes: HashMap<u64, Inode>,
    deleted_drecs: Vec<(u64, u64, String, bool)>,
    deleted_extents: HashMap<u64, Vec<FileExtent>>,
    /// Deleted files rebuilt from freed node space; their extent is paired
    /// heuristically, so content is even less certain than stale records.
    residue_inodes: std::collections::HashSet<u64>,
}

impl SpecVolumeIndex {
    /// Whether `inode_id` was rebuilt from node residue (heuristic extent).
    pub fn is_residue_inode(&self, inode_id: u64) -> bool {
        self.residue_inodes.contains(&inode_id)
    }

    pub fn entries(&self) -> Vec<ApfsEntry> {
        let live = self.drecs.iter().map(|d| (d, false));
        let deleted = self.deleted_drecs.iter().map(|d| (d, true));
        live.chain(deleted)
            .map(|((parent, child, name, is_dir), is_deleted)| {
                let inode = if is_deleted {
                    self.deleted_inodes.get(child)
                } else {
                    self.inodes.get(child)
                };
                ApfsEntry {
                    is_deleted,
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

    /// Whether `inode_id` is only known from stale (deleted) records.
    pub fn is_deleted_inode(&self, inode_id: u64) -> bool {
        !self.inodes.contains_key(&inode_id) && self.deleted_inodes.contains_key(&inode_id)
    }

    /// Logical byte ranges of a deleted file whose physical blocks are now
    /// owned by a live file, i.e. definitely overwritten.
    pub fn live_overlap_ranges(&self, extents: &[FileExtent]) -> Vec<(u64, u64)> {
        let bs = self.block_size.max(1);
        let mut live: Vec<(u64, u64)> = self
            .extents
            .values()
            .flatten()
            .filter(|(_, len, phys)| *len > 0 && *phys != 0)
            .map(|(_, len, phys)| (*phys, phys.saturating_add(len.div_ceil(bs))))
            .collect();
        live.sort_unstable();
        let mut overlaps = Vec::new();
        for (logical, len, phys) in extents {
            let blocks = len.div_ceil(bs);
            for block in 0..blocks {
                let pblock = phys.saturating_add(block);
                let idx = live.partition_point(|(start, _)| *start <= pblock);
                let owned = live[..idx]
                    .iter()
                    .rev()
                    .take(64)
                    .any(|(s, e)| *s <= pblock && pblock < *e);
                if owned {
                    let start = logical.saturating_add(block * bs);
                    let end = (logical.saturating_add(*len)).min(start.saturating_add(bs));
                    overlaps.push((start, end.saturating_sub(start)));
                }
            }
        }
        overlaps
    }

    /// File extents `(logical_offset, length, physical_block)` for an inode,
    /// plus whether the inode uses transparent compression.
    pub fn file_extents(&self, inode_id: u64) -> Option<(Vec<FileExtent>, bool)> {
        let (inode, extent_map) = match self.inodes.get(&inode_id) {
            Some(inode) => (inode, &self.extents),
            None => (self.deleted_inodes.get(&inode_id)?, &self.deleted_extents),
        };
        let stream = if inode.private_id != 0 {
            inode.private_id
        } else {
            inode_id
        };
        let mut extents = extent_map.get(&stream).cloned().unwrap_or_default();
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
        self.spans()
            .into_iter()
            .map(|(k, kl, v, vl)| (&self.buf[k..k + kl], &self.buf[v..v + vl]))
            .collect()
    }

    /// Byte ranges `(key_pos, key_len, value_pos, value_len)` of the records
    /// the table of contents still references.
    fn spans(&self) -> Vec<(usize, usize, usize, usize)> {
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
            let key_ok = key_pos + k_len <= self.buf.len();
            let value_ok = value_pos + v_len <= self.value_end;
            if key_ok && value_ok {
                out.push((key_pos, k_len, value_pos, v_len));
            }
        }
        out
    }

    /// Records left behind in freed key/value space: inode values that still
    /// carry a name, and extent values that still carry a physical block.
    /// Freed slots start with a 4-byte free-list link, which clobbers the low
    /// half of an inode's parent id and of an extent's length, so neither is
    /// trusted; the extent owner is not recorded at all.
    fn residue(&self, block_count: u64) -> (Vec<ResidueInode>, Vec<u64>) {
        let mut covered = vec![false; self.buf.len()];
        covered[..self.key_start].iter_mut().for_each(|c| *c = true);
        covered[self.value_end..].iter_mut().for_each(|c| *c = true);
        for (k, kl, v, vl) in self.spans() {
            covered[k..k + kl].iter_mut().for_each(|c| *c = true);
            covered[v..v + vl].iter_mut().for_each(|c| *c = true);
        }
        let mut inodes = Vec::new();
        let mut extents = Vec::new();
        // Pass 1: inode values; mark their bytes so their internals (e.g. a
        // dstream size followed by zeros) are not read as extent values.
        let mut q = self.key_start;
        while q + 96 <= self.value_end {
            if !covered[q] {
                if let Some((inode, len)) = residue_inode(&self.buf[q..self.value_end]) {
                    if !covered[q..q + len].iter().any(|c| *c) {
                        covered[q..q + len].iter_mut().for_each(|c| *c = true);
                        inodes.push(inode);
                        q += len;
                        continue;
                    }
                }
            }
            q += 1;
        }
        if inodes.is_empty() {
            return (inodes, extents);
        }
        // Pass 2: j_file_extent_val_t {len_and_flags (low half may be the
        // free link), phys_block_num, crypto_id}. Unencrypted: high half of
        // len_and_flags and crypto_id are zero.
        let mut q = self.key_start;
        while q + 24 <= self.value_end {
            let hi = le_u32(&self.buf, q + 4);
            let phys = le_u64(&self.buf, q + 8);
            let crypto = le_u64(&self.buf, q + 16);
            if let (Some(0), Some(phys), Some(0)) = (hi, phys, crypto) {
                if phys > 0 && phys < block_count && !covered[q..q + 24].iter().any(|c| *c) {
                    extents.push(phys);
                    q += 24;
                    continue;
                }
            }
            q += 1;
        }
        (inodes, extents)
    }
}

#[derive(Debug, Clone)]
struct ResidueInode {
    private_id: u64,
    name: String,
    size: u64,
    compressed: bool,
}

/// Parse a regular-file inode value that still carries its name xfield.
/// Returns the inode and the value's length.
fn residue_inode(value: &[u8]) -> Option<(ResidueInode, usize)> {
    let mode = le_u16(value, 80)?;
    if mode & S_IFMT != S_IFREG {
        return None;
    }
    let count = usize::from(le_u16(value, 92)?);
    let used = usize::from(le_u16(value, 94)?);
    if !(1..=8).contains(&count) || used > 1024 {
        return None;
    }
    let private_id = le_u64(value, 8)?;
    if private_id == 0 || private_id > OBJ_ID_MASK {
        return None;
    }
    let headers = 96;
    let mut data = headers + count * 4;
    let (mut name, mut size) = (None, 0);
    for index in 0..count {
        let h = headers + index * 4;
        let x_type = *value.get(h)?;
        let x_size = usize::from(le_u16(value, h + 2)?);
        let payload = value.get(data..data + x_size)?;
        match x_type {
            INO_EXT_TYPE_NAME => {
                let raw = payload.strip_suffix(&[0])?;
                let text = std::str::from_utf8(raw).ok()?;
                if text.is_empty()
                    || text.len() > 255
                    || text.chars().any(|c| c.is_control() || c == '/')
                {
                    return None;
                }
                name = Some(text.to_string());
            }
            INO_EXT_TYPE_DSTREAM => size = le_u64(payload, 0)?,
            _ => {}
        }
        data += x_size.div_ceil(8) * 8;
    }
    let bsd_flags = le_u32(value, 68)?;
    Some((
        ResidueInode {
            private_id,
            name: name?,
            size,
            compressed: bsd_flags & UF_COMPRESSED != 0,
        },
        data,
    ))
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
    index_volumes_with_options(source, csb, container_offset, false)
}

/// Like [`index_volumes`], optionally scanning every container block for
/// stale file-system tree leaf nodes to surface deleted files. Deleted-file
/// scanning reads the whole container and is therefore opt-in.
pub fn index_volumes_with_options<S: BlockSource + ?Sized>(
    source: &S,
    csb: &ApfsContainerSuperblock,
    container_offset: u64,
    include_deleted: bool,
) -> Option<Vec<SpecVolumeIndex>> {
    index_volumes_inner(
        source,
        csb,
        container_offset,
        include_deleted,
        &mut |_, _| true,
    )
    .unwrap_or(None)
}

/// Deleted-file scanning was stopped by the progress callback.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScanCancelled;

/// Like [`index_volumes_with_options`] with deleted-file scanning enabled,
/// reporting `(blocks_scanned, blocks_total)` during the whole-container
/// stale-node pass. Returning `false` from `progress` cancels the scan.
pub fn index_volumes_with_deleted_progress<S: BlockSource + ?Sized>(
    source: &S,
    csb: &ApfsContainerSuperblock,
    container_offset: u64,
    progress: &mut dyn FnMut(u64, u64) -> bool,
) -> Result<Option<Vec<SpecVolumeIndex>>, ScanCancelled> {
    index_volumes_inner(source, csb, container_offset, true, progress)
}

fn index_volumes_inner<S: BlockSource + ?Sized>(
    source: &S,
    csb: &ApfsContainerSuperblock,
    container_offset: u64,
    include_deleted: bool,
    progress: &mut dyn FnMut(u64, u64) -> bool,
) -> Result<Option<Vec<SpecVolumeIndex>>, ScanCancelled> {
    let reader = Reader {
        source,
        container_offset,
        block_size: csb.block_size as usize,
    };
    let Some(nx) = latest_container_superblock(&reader, csb) else {
        return Ok(None);
    };
    let xid = nx.header.xid;
    let Some(container_omap) = reader.omap_tree_root(nx.omap_oid) else {
        return Ok(None);
    };

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
    if include_deleted && !volumes.is_empty() {
        let mut stale = collect_stale_records(&reader, nx.block_count, progress)?;
        // A residue "extent" pointing at a live metadata object is noise.
        for (_, _, blocks) in &mut stale.residue {
            blocks.retain(|b| {
                !reader
                    .block(*b)
                    .is_some_and(|blk| is_apfs_object_block(&blk))
            });
        }
        // Stale nodes carry no volume identity; attribute them to the first
        // volume. Multi-volume containers are reported as a limitation.
        if let Some(first) = volumes.first_mut() {
            first.attach_deleted(stale);
        }
    }
    Ok((!volumes.is_empty()).then_some(volumes))
}

#[derive(Default)]
struct StaleRecords {
    inodes: HashMap<u64, (u64, Inode)>,
    drecs: HashMap<(u64, String), (u64, u64, bool)>,
    extents: HashMap<(u64, u64), (u64, FileExtent)>,
    /// Per leaf node: (node xid, residue inodes, residue extent blocks).
    residue: Vec<(u64, Vec<ResidueInode>, Vec<u64>)>,
}

impl SpecVolumeIndex {
    fn attach_deleted(&mut self, stale: StaleRecords) {
        let live_names: std::collections::HashSet<(u64, &str)> = self
            .drecs
            .iter()
            .map(|(parent, _, name, _)| (*parent, name.as_str()))
            .collect();
        let mut seen = std::collections::HashSet::new();
        let mut deleted = Vec::new();
        for ((parent, name), (_, child, is_dir)) in &stale.drecs {
            if self.inodes.contains_key(child) || live_names.contains(&(*parent, name.as_str())) {
                continue;
            }
            if seen.insert(*child) {
                deleted.push((*parent, *child, name.clone(), *is_dir));
            }
        }
        deleted.sort_by(|a, b| (a.0, &a.2).cmp(&(b.0, &b.2)));
        let mut deleted_extents: HashMap<u64, Vec<FileExtent>> = HashMap::new();
        for ((stream, _), (_, extent)) in stale.extents {
            deleted_extents.entry(stream).or_default().push(extent);
        }
        self.deleted_inodes = stale
            .inodes
            .into_iter()
            .filter(|(id, _)| !self.inodes.contains_key(id))
            .map(|(id, (_, inode))| (id, inode))
            .collect();
        self.deleted_drecs = deleted;
        self.deleted_extents = deleted_extents;
        self.attach_residue(stale.residue);
    }

    /// Add deleted files rebuilt from freed node space. Inode and extent
    /// records of one file sit next to each other in key order, so they
    /// share a leaf node; APFS allocates inode numbers and (on a young
    /// volume) blocks in increasing order. Within one node the residue inodes
    /// sorted by id are therefore paired with residue extents sorted by block,
    /// but only when the counts match; otherwise the file is listed without
    /// data rather than risk giving it another file's content.
    fn attach_residue(&mut self, residue: Vec<(u64, Vec<ResidueInode>, Vec<u64>)>) {
        let known = |id: u64, this: &Self| {
            this.inodes.contains_key(&id)
                || this.inodes.values().any(|i| i.private_id == id)
                || this.deleted_inodes.contains_key(&id)
                || this.deleted_inodes.values().any(|i| i.private_id == id)
        };
        let live_starts: std::collections::HashSet<u64> = self
            .extents
            .values()
            .flatten()
            .map(|(_, _, phys)| *phys)
            .collect();
        // id -> (paired, xid, inode, block)
        let mut best: HashMap<u64, (bool, u64, ResidueInode, Option<u64>)> = HashMap::new();
        for (xid, inodes, extents) in residue {
            let mut files: Vec<ResidueInode> = inodes
                .into_iter()
                .filter(|i| i.size > 0 && !known(i.private_id, self))
                .collect();
            files.sort_by_key(|i| i.private_id);
            files.dedup_by_key(|i| i.private_id);
            if files.is_empty() {
                continue;
            }
            let mut blocks: Vec<u64> = extents;
            blocks.sort_unstable();
            blocks.dedup();
            // Residue of a live file's older record copy adds a stray extent;
            // drop extents starting where a live extent starts only when the
            // counts do not already match (a deleted file's blocks may have
            // been reused by a live file, and must then stay to be flagged).
            if blocks.len() != files.len() {
                let filtered: Vec<u64> = blocks
                    .iter()
                    .copied()
                    .filter(|b| !live_starts.contains(b))
                    .collect();
                if filtered.len() == files.len() {
                    blocks = filtered;
                }
            }
            let paired = blocks.len() == files.len();
            for (index, file) in files.into_iter().enumerate() {
                let block = paired.then(|| blocks[index]);
                let candidate = (paired, xid, file, block);
                match best.get(&candidate.2.private_id) {
                    Some(current) if (current.0, current.1) >= (candidate.0, candidate.1) => {}
                    _ => {
                        best.insert(candidate.2.private_id, candidate);
                    }
                }
            }
        }
        if best.is_empty() {
            return;
        }
        self.deleted_drecs.push((
            ROOT_DIR_INO_NUM,
            RESIDUE_DIR_INODE,
            RESIDUE_DIR_NAME.to_string(),
            true,
        ));
        let mut ids: Vec<u64> = best.keys().copied().collect();
        ids.sort_unstable();
        let bs = self.block_size.max(1);
        for id in ids {
            let (_, _, file, block) = best.remove(&id).expect("id from keys");
            self.deleted_inodes.insert(
                id,
                Inode {
                    parent: RESIDUE_DIR_INODE,
                    private_id: id,
                    size: file.size,
                    compressed: file.compressed,
                    is_dir: false,
                },
            );
            if let Some(block) = block {
                // The extent length was clobbered; assume one contiguous run.
                let len = file.size.div_ceil(bs) * bs;
                self.deleted_extents.insert(id, vec![(0, len, block)]);
            }
            self.deleted_drecs
                .push((RESIDUE_DIR_INODE, id, file.name, false));
            self.residue_inodes.insert(id);
        }
    }
}

/// Scan every container block for checksum-valid FS-tree leaf nodes and keep
/// the newest version (by node xid) of each record.
fn collect_stale_records<S: BlockSource + ?Sized>(
    reader: &Reader<'_, S>,
    block_count: u64,
    progress: &mut dyn FnMut(u64, u64) -> bool,
) -> Result<StaleRecords, ScanCancelled> {
    const BATCH_BLOCKS: u64 = 256;
    let mut stale = StaleRecords::default();
    let block_size = reader.block_size;
    let mut chunk = vec![0_u8; block_size * BATCH_BLOCKS as usize];
    let mut base = 0_u64;
    while base < block_count {
        if !progress(base, block_count) {
            return Err(ScanCancelled);
        }
        let count = BATCH_BLOCKS.min(block_count - base) as usize;
        let Some(offset) = base
            .checked_mul(block_size as u64)
            .and_then(|o| o.checked_add(reader.container_offset))
        else {
            break;
        };
        let buf = &mut chunk[..count * block_size];
        let read = match reader.source.read_at(offset, buf) {
            Ok(read) => read.bytes_read,
            Err(_) => 0,
        };
        for block in buf[..read - read % block_size].chunks_exact(block_size) {
            let (Some(obj_type), Some(subtype)) = (le_u32(block, 24), le_u32(block, 28)) else {
                continue;
            };
            if obj_type & OBJECT_TYPE_MASK != OBJECT_TYPE_BTREE_NODE
                && obj_type & OBJECT_TYPE_MASK != OBJECT_TYPE_BTREE
                || subtype != OBJECT_TYPE_FSTREE
            {
                continue;
            }
            if le_u64(block, 0) != Some(fletcher64(block)) {
                continue;
            }
            let xid = le_u64(block, 16).unwrap_or(0);
            let Some(node) = Node::parse(block.to_vec()) else {
                continue;
            };
            if !node.is_leaf() {
                continue;
            }
            for (key, value) in node.records() {
                stale.absorb(xid, key, value);
            }
            let (inodes, extents) = node.residue(block_count);
            if !inodes.is_empty() {
                stale.residue.push((xid, inodes, extents));
            }
        }
        if read < buf.len() {
            break;
        }
        base += count as u64;
    }
    progress(block_count, block_count);
    Ok(stale)
}

impl StaleRecords {
    fn absorb(&mut self, xid: u64, key: &[u8], value: &[u8]) {
        let Some(header) = le_u64(key, 0) else {
            return;
        };
        let obj_id = header & OBJ_ID_MASK;
        match header >> 60 {
            APFS_TYPE_INODE => {
                if let Some(inode) = parse_inode(value) {
                    let slot = self.inodes.entry(obj_id).or_insert((xid, inode.clone()));
                    if xid >= slot.0 {
                        *slot = (xid, inode);
                    }
                }
            }
            APFS_TYPE_DIR_REC => {
                if let Some((name, child, is_dir)) = parse_drec(key, value) {
                    if obj_id == ROOT_DIR_PARENT {
                        return;
                    }
                    let slot = self
                        .drecs
                        .entry((obj_id, name))
                        .or_insert((xid, child, is_dir));
                    if xid >= slot.0 {
                        *slot = (xid, child, is_dir);
                    }
                }
            }
            APFS_TYPE_FILE_EXTENT => {
                let logical = le_u64(key, 8);
                let len = le_u64(value, 0).map(|v| v & 0x00ff_ffff_ffff_ffff);
                let phys = le_u64(value, 8);
                if let (Some(logical), Some(len), Some(phys)) = (logical, len, phys) {
                    if len > 0 {
                        let slot = self
                            .extents
                            .entry((obj_id, logical))
                            .or_insert((xid, (logical, len, phys)));
                        if xid >= slot.0 {
                            *slot = (xid, (logical, len, phys));
                        }
                    }
                }
            }
            _ => {}
        }
    }
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
        block_size: reader.block_size as u64,
        inodes,
        drecs,
        extents,
        deleted_inodes: HashMap::new(),
        deleted_drecs: Vec::new(),
        deleted_extents: HashMap::new(),
        residue_inodes: Default::default(),
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

/// True when `block` is a checksum-valid APFS object, meaning a freed data
/// block has since been reused for file-system metadata.
pub fn is_apfs_object_block(block: &[u8]) -> bool {
    let (Some(cksum), Some(obj_type)) = (le_u64(block, 0), le_u32(block, 24)) else {
        return false;
    };
    cksum != 0 && obj_type & OBJECT_TYPE_MASK != 0 && cksum == fletcher64(block)
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

    fn inode_record(id: u64, size: u64) -> (Vec<u8>, Vec<u8>) {
        let key = ((APFS_TYPE_INODE << 60) | id).to_le_bytes().to_vec();
        let mut value = vec![0_u8; 92];
        value[8..16].copy_from_slice(&id.to_le_bytes());
        value[80..82].copy_from_slice(&0o100644_u16.to_le_bytes());
        value.extend_from_slice(&1_u16.to_le_bytes());
        value.extend_from_slice(&40_u16.to_le_bytes());
        value.extend_from_slice(&[INO_EXT_TYPE_DSTREAM, 0, 40, 0]);
        let mut dstream = [0_u8; 40];
        dstream[0..8].copy_from_slice(&size.to_le_bytes());
        value.extend_from_slice(&dstream);
        (key, value)
    }

    fn drec_record(parent: u64, name: &str, child: u64) -> (Vec<u8>, Vec<u8>) {
        let name = format!("{name}\0");
        let mut key = ((APFS_TYPE_DIR_REC << 60) | parent).to_le_bytes().to_vec();
        key.extend_from_slice(&(name.len() as u32).to_le_bytes());
        key.extend_from_slice(name.as_bytes());
        let mut value = vec![0_u8; 18];
        value[0..8].copy_from_slice(&child.to_le_bytes());
        value[16..18].copy_from_slice(&8_u16.to_le_bytes());
        (key, value)
    }

    #[test]
    fn stale_records_surface_only_deleted_files_with_newest_version() {
        let mut stale = StaleRecords::default();
        for (xid, (key, value)) in [
            (3, inode_record(20, 100)),
            (5, inode_record(20, 4096)),
            (5, drec_record(2, "gone.bin", 20)),
            (5, inode_record(21, 7)),
            (5, drec_record(2, "live.txt", 21)),
        ] {
            stale.absorb(xid, &key, &value);
        }
        let mut index = SpecVolumeIndex {
            block_size: 4096,
            volume: SpecVolume {
                fs_oid: 1,
                role: 0,
                encrypted: false,
                name: String::new(),
                omap_oid: 0,
                root_tree_oid: 0,
                extentref_tree_oid: 0,
                snap_meta_tree_oid: 0,
            },
            inodes: HashMap::from([(21, parse_inode(&inode_record(21, 7).1).unwrap())]),
            drecs: vec![(2, 21, "live.txt".to_string(), false)],
            extents: HashMap::new(),
            deleted_inodes: HashMap::new(),
            deleted_drecs: Vec::new(),
            deleted_extents: HashMap::new(),
            residue_inodes: Default::default(),
        };
        index.attach_deleted(stale);
        let entries = index.entries();
        let deleted: Vec<_> = entries.iter().filter(|e| e.is_deleted).collect();
        assert_eq!(deleted.len(), 1);
        assert_eq!(deleted[0].name, "gone.bin");
        assert_eq!(deleted[0].size, 4096);
        assert_eq!(deleted[0].parent_inode, ROOT_DIR_PARENT);
        assert_eq!(entries.iter().filter(|e| !e.is_deleted).count(), 1);
    }

    #[test]
    fn malformed_inputs_do_not_panic() {
        assert!(parse_inode(&[0_u8; 10]).is_none());
        assert!(parse_drec(&[1, 2, 3], &[0_u8; 4]).is_none());
        assert!(Node::parse(vec![0xff_u8; 64]).is_none());
    }

    struct ZeroSource {
        device: data_recovery_core::PhysicalDevice,
        len: u64,
    }

    impl BlockSource for ZeroSource {
        fn read_at(
            &self,
            offset: u64,
            buffer: &mut [u8],
        ) -> Result<data_recovery_core::ReadResult, data_recovery_core::ReadError> {
            let n = self.len.saturating_sub(offset).min(buffer.len() as u64) as usize;
            buffer[..n].fill(0);
            Ok(data_recovery_core::ReadResult {
                offset,
                bytes_read: n,
            })
        }

        fn device(&self) -> &data_recovery_core::PhysicalDevice {
            &self.device
        }
    }

    fn zero_reader_source(blocks: u64) -> ZeroSource {
        ZeroSource {
            device: data_recovery_core::PhysicalDevice {
                id: "zero".to_string(),
                display_name: "zero".to_string(),
                platform_path: "/dev/null".to_string(),
                size_bytes: blocks * 4096,
                logical_sector_size: 512,
                physical_sector_size: 512,
                bus_type: data_recovery_core::BusType::Sata,
                is_removable: true,
                is_system_disk: false,
                trim_supported: None,
            },
            len: blocks * 4096,
        }
    }

    #[test]
    fn stale_scan_reports_progress_and_honours_cancel() {
        let source = zero_reader_source(1000);
        let reader = Reader {
            source: &source,
            container_offset: 0,
            block_size: 4096,
        };
        let mut calls = Vec::new();
        let result = collect_stale_records(&reader, 1000, &mut |done, total| {
            calls.push((done, total));
            true
        });
        assert!(result.is_ok());
        assert_eq!(calls.first(), Some(&(0, 1000)));
        assert_eq!(calls.last(), Some(&(1000, 1000)));
        assert!(calls.windows(2).all(|w| w[0].0 <= w[1].0));

        let mut seen = 0;
        let cancelled = collect_stale_records(&reader, 1000, &mut |_, _| {
            seen += 1;
            seen < 2
        });
        assert!(matches!(cancelled, Err(ScanCancelled)));
        assert_eq!(seen, 2, "scan must stop at the first refused batch");
    }

    /// An inode value with name + dstream xfields, first 4 bytes clobbered
    /// by a free-list link as in freed node space.
    fn residue_inode_value(id: u64, name: &str, size: u64) -> Vec<u8> {
        let mut value = vec![0_u8; 92];
        value[0..4].copy_from_slice(&[0x8b, 0x02, 0x18, 0x00]);
        value[8..16].copy_from_slice(&id.to_le_bytes());
        value[80..82].copy_from_slice(&0o100644_u16.to_le_bytes());
        let name = [name.as_bytes(), &[0]].concat();
        let padded = name.len().div_ceil(8) * 8;
        value.extend_from_slice(&2_u16.to_le_bytes());
        value.extend_from_slice(&((padded + 40) as u16).to_le_bytes());
        value.extend_from_slice(&[INO_EXT_TYPE_NAME, 0, name.len() as u8, 0]);
        value.extend_from_slice(&[INO_EXT_TYPE_DSTREAM, 0, 40, 0]);
        value.extend_from_slice(&name);
        value.resize(value.len() + padded - name.len(), 0);
        let mut dstream = [0_u8; 40];
        dstream[0..8].copy_from_slice(&size.to_le_bytes());
        value.extend_from_slice(&dstream);
        value
    }

    fn residue_extent_value(phys: u64) -> Vec<u8> {
        let mut value = vec![0x10, 0x00, 0x18, 0x00, 0, 0, 0, 0];
        value.extend_from_slice(&phys.to_le_bytes());
        value.extend_from_slice(&0_u64.to_le_bytes());
        value
    }

    #[test]
    fn parses_residue_inode_with_clobbered_parent() {
        let value = residue_inode_value(49, "中文檔名.txt", 650);
        let (inode, len) = residue_inode(&value).unwrap();
        assert_eq!((inode.private_id, inode.size), (49, 650));
        assert_eq!(inode.name, "中文檔名.txt");
        assert_eq!(len, value.len());

        let mut dir = value.clone();
        dir[80..82].copy_from_slice(&0o040755_u16.to_le_bytes());
        assert!(residue_inode(&dir).is_none(), "directories are not files");
        let mut bad_name = value.clone();
        bad_name[100 + 8] = 0xff;
        assert!(
            residue_inode(&bad_name).is_none(),
            "name must be valid UTF-8"
        );
        assert!(residue_inode(&value[..100]).is_none());
    }

    #[test]
    fn node_residue_skips_live_records_and_inode_internals() {
        // Leaf, non-root node with one live record; residue lives in the
        // rest. The inode's dstream (size 3000, then zeros) must not be
        // mistaken for an extent value.
        let mut buf = vec![0_u8; 4096];
        buf[32..34].copy_from_slice(&BTNODE_LEAF.to_le_bytes());
        buf[36..40].copy_from_slice(&1_u32.to_le_bytes());
        buf[40..42].copy_from_slice(&0_u16.to_le_bytes());
        buf[42..44].copy_from_slice(&8_u16.to_le_bytes());
        // toc: key at 0 len 8, value 24 bytes ending at block end
        buf[56..64].copy_from_slice(&[0, 0, 8, 0, 24, 0, 24, 0]);
        let live_extent = residue_extent_value(777);
        buf[4096 - 24..].copy_from_slice(&live_extent);
        let inode = residue_inode_value(50, "small.dat", 3000);
        buf[200..200 + inode.len()].copy_from_slice(&inode);
        let ext = residue_extent_value(1545);
        buf[2000..2024].copy_from_slice(&ext);
        let node = Node::parse(buf).unwrap();
        let (inodes, extents) = node.residue(16_384);
        assert_eq!(inodes.len(), 1);
        assert_eq!(inodes[0].private_id, 50);
        assert_eq!(
            extents,
            vec![1545],
            "live extent and dstream fields excluded"
        );
    }

    fn empty_index() -> SpecVolumeIndex {
        SpecVolumeIndex {
            volume: SpecVolume {
                fs_oid: 1026,
                role: 0,
                encrypted: false,
                name: "t".into(),
                omap_oid: 0,
                root_tree_oid: 0,
                extentref_tree_oid: 0,
                snap_meta_tree_oid: 0,
            },
            block_size: 4096,
            inodes: HashMap::new(),
            drecs: Vec::new(),
            extents: HashMap::new(),
            deleted_inodes: HashMap::new(),
            deleted_drecs: Vec::new(),
            deleted_extents: HashMap::new(),
            residue_inodes: Default::default(),
        }
    }

    fn residue(id: u64, size: u64) -> ResidueInode {
        ResidueInode {
            private_id: id,
            name: format!("f{id}"),
            size,
            compressed: false,
        }
    }

    #[test]
    fn residue_pairs_in_order_only_when_counts_match() {
        let mut index = empty_index();
        // live file 30 owns block 900; its old record copy left residue 900
        index.inodes.insert(
            30,
            Inode {
                private_id: 30,
                size: 10,
                ..Default::default()
            },
        );
        index.extents.insert(30, vec![(0, 4096, 900)]);
        index.attach_residue(vec![
            // ids unsorted, blocks unsorted: pair by order 39->249, 49->1544, 50->1545
            (
                5,
                vec![residue(50, 3000), residue(39, 5000), residue(49, 650)],
                vec![1545, 249, 1544],
            ),
            // stray live-copy extent 900 resolved by the live-start filter
            (5, vec![residue(56, 1820)], vec![900, 1552]),
            // two files, one extent: listed but not paired
            (5, vec![residue(60, 10), residue(61, 10)], vec![2000]),
            // a live file's residue is ignored
            (5, vec![residue(30, 10)], vec![3000]),
        ]);
        let ext = |id| index.deleted_extents.get(&id).map(|v| v[0].2);
        assert_eq!(
            (ext(39), ext(49), ext(50)),
            (Some(249), Some(1544), Some(1545))
        );
        assert_eq!(ext(56), Some(1552));
        assert_eq!(
            index.deleted_extents.get(&39).unwrap()[0].1,
            8192,
            "rounded to blocks"
        );
        assert_eq!((ext(60), ext(61)), (None, None));
        assert!(index.is_residue_inode(60) && !index.is_residue_inode(30));
        let entries = index.entries();
        let dir = entries
            .iter()
            .find(|e| e.inode == RESIDUE_DIR_INODE)
            .unwrap();
        assert!(dir.is_directory && dir.is_deleted);
        assert!(
            entries
                .iter()
                .filter(|e| !e.is_directory)
                .all(|e| e.parent_inode == RESIDUE_DIR_INODE && e.is_deleted)
        );
    }
}
