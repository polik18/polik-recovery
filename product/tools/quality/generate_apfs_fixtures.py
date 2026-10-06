#!/usr/bin/env python3
"""Generate deterministic APFS fixtures for Polik Recovery v1.3.

These are intentionally small synthetic APFS-like images that match the MVP APFS
parser used by the project. They are not full macOS volumes; they exercise the
paths that matter for regression tests:

* VFS directory records + inode size records
* extent based extraction
* missing logical ranges / partial recovery reporting
* APFS volume crypto metadata hint reporting
"""
from __future__ import annotations

import argparse
import hashlib
import json
import struct
from pathlib import Path

BLOCK = 4096
FIXTURE_VERSION = "v1.3"
FIXTURE_GENERATED_AT = "2000-01-01T00:00:00+00:00"


def sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def fletcher64(data: bytes | bytearray) -> int:
    mod = 0xFFFFFFFF
    sum1 = 0
    sum2 = 0
    # APFS object checksums skip the first eight bytes and store two check words.
    for idx in range(8, len(data) - (len(data) % 4), 4):
        word = struct.unpack_from("<I", data, idx)[0]
        sum1 = (sum1 + word) % mod
        sum2 = (sum2 + sum1) % mod
    low = (mod - ((sum1 + sum2) % mod)) % mod
    high = (mod - ((sum1 + low) % mod)) % mod
    return ((high << 32) | low) & 0xFFFFFFFFFFFFFFFF


def seal_checksum(block: bytearray) -> None:
    block[0:8] = b"\x00" * 8
    block[0:8] = struct.pack("<Q", fletcher64(block))


def object_block(magic: bytes, oid: int, obj_type: int = 1, subtype: int = 0) -> bytearray:
    block = bytearray(BLOCK)
    block[8:16] = struct.pack("<Q", oid)
    block[16:24] = struct.pack("<Q", 100)
    block[24:28] = struct.pack("<I", obj_type)
    block[28:32] = struct.pack("<I", subtype)
    block[32:36] = magic
    block[36:40] = struct.pack("<I", BLOCK)
    return block


def container_superblock(fs_oid: int, block_count: int) -> bytes:
    block = object_block(b"NXSB", 1)
    block[40:48] = struct.pack("<Q", block_count)
    block[72:88] = bytes.fromhex("504f4c494b4150465356313300000000")[:16]
    block[88:96] = struct.pack("<Q", 1000)
    block[96:104] = struct.pack("<Q", 1)
    block[160:168] = struct.pack("<Q", 0)  # omap oid intentionally unused in MVP parser
    block[180:184] = struct.pack("<I", 1)
    block[184:192] = struct.pack("<Q", fs_oid)
    seal_checksum(block)
    return bytes(block)


def volume_superblock(fs_oid: int, root_tree_oid: int, *, meta_crypto: int = 0) -> bytes:
    block = object_block(b"APFS", fs_oid)
    block[36:40] = struct.pack("<I", 0)  # fs_index
    block[72:74] = struct.pack("<H", 0)  # role
    block[108:116] = struct.pack("<Q", meta_crypto)
    block[116:124] = struct.pack("<Q", 0)  # omap oid intentionally unused
    block[124:132] = struct.pack("<Q", root_tree_oid)
    block[132:140] = struct.pack("<Q", 0)  # extentref tree oid
    block[140:148] = struct.pack("<Q", 0)  # snapshot metadata tree oid
    seal_checksum(block)
    return bytes(block)


def dir_record_key(parent_inode: int, name: str) -> bytes:
    raw_name = name.encode("utf-8")
    return (
        struct.pack("<Q", parent_inode)
        + struct.pack("<I", 0x30000000)
        + struct.pack("<H", len(raw_name))
        + raw_name
    )


def dir_record_value(child_inode: int, *, is_dir: bool = False) -> bytes:
    val = bytearray(20)
    val[0:8] = struct.pack("<Q", child_inode)
    val[18] = 4 if is_dir else 8
    return bytes(val)


def inode_key(inode: int) -> bytes:
    return struct.pack("<Q", inode) + struct.pack("<I", 0x20000000)


def inode_value(size: int) -> bytes:
    val = bytearray(32)
    val[24:32] = struct.pack("<Q", size)
    return bytes(val)


def extent_key(inode: int, logical_offset: int) -> bytes:
    return (
        struct.pack("<Q", inode)
        + struct.pack("<I", 0x80000000)
        + struct.pack("<Q", logical_offset)
    )


def extent_value(length: int, phys_block: int) -> bytes:
    val = bytearray(24)
    val[0:8] = struct.pack("<Q", length)
    val[8:16] = struct.pack("<Q", phys_block)
    return bytes(val)


def btree_leaf(records: list[tuple[bytes, bytes]], oid: int) -> bytes:
    block = bytearray(BLOCK)
    block[8:16] = struct.pack("<Q", oid)
    block[16:24] = struct.pack("<Q", 100)
    block[24:28] = struct.pack("<I", 0)

    table_len = len(records) * 4
    keys_start = 56
    data_offset = 1024
    data_entry_start = 56 + data_offset
    key_cursor = table_len + 8
    value_back_cursor = 0

    block[32:34] = struct.pack("<H", 0x2)  # leaf
    block[34:36] = struct.pack("<H", len(records))
    block[36:38] = struct.pack("<H", 0)  # keys_offset
    block[40:42] = struct.pack("<H", data_offset)  # data_offset

    max_key_end = 0
    max_data_end = 0
    for idx, (key, value) in enumerate(records):
        key_off = key_cursor
        key_len = len(key)
        key_pos = keys_start + key_off
        block[key_pos : key_pos + key_len] = key
        block[keys_start + idx * 4 : keys_start + idx * 4 + 2] = struct.pack("<H", key_off)
        block[keys_start + idx * 4 + 2 : keys_start + idx * 4 + 4] = struct.pack("<H", key_len)
        key_cursor += key_len
        max_key_end = max(max_key_end, key_off + key_len)

        value_back_cursor += len(value)
        value_pos = BLOCK - value_back_cursor
        block[value_pos : value_pos + len(value)] = value
        block[data_entry_start + idx * 4 : data_entry_start + idx * 4 + 2] = struct.pack("<H", value_back_cursor)
        block[data_entry_start + idx * 4 + 2 : data_entry_start + idx * 4 + 4] = struct.pack("<H", len(value))
        max_data_end = max(max_data_end, value_back_cursor)

    block[38:40] = struct.pack("<H", max_key_end)
    block[42:44] = struct.pack("<H", max_data_end)
    seal_checksum(block)
    return bytes(block)


def build_apfs_image(
    out_path: Path,
    *,
    encrypted: bool = False,
    include_partial: bool = True,
) -> dict[str, object]:
    fs_oid = 2
    root_tree_oid = 3
    complete_inode = 202
    partial_inode = 303
    encrypted_inode = 404
    image = bytearray(BLOCK * 10)

    complete_data = b"APFS complete extent fixture data\n"
    partial_head = b"APFS partial head only"
    partial_size = len(partial_head) + 16
    encrypted_data = b"APFS encrypted-hint fixture data\n"

    records: list[tuple[bytes, bytes]] = []
    include_complete = not encrypted
    if include_complete:
        records += [
            (dir_record_key(1, "COMPLETE.TXT"), dir_record_value(complete_inode)),
            (inode_key(complete_inode), inode_value(len(complete_data))),
            (extent_key(complete_inode, 0), extent_value(len(complete_data), 5)),
        ]
    if include_partial:
        records += [
            (dir_record_key(1, "PARTIAL.BIN"), dir_record_value(partial_inode)),
            (inode_key(partial_inode), inode_value(partial_size)),
            (extent_key(partial_inode, 0), extent_value(len(partial_head), 6)),
        ]
    if encrypted:
        records += [
            (dir_record_key(1, "ENCRYPTED.TXT"), dir_record_value(encrypted_inode)),
            (inode_key(encrypted_inode), inode_value(len(encrypted_data))),
            (extent_key(encrypted_inode, 0), extent_value(len(encrypted_data), 7)),
        ]

    image[0:BLOCK] = container_superblock(fs_oid, block_count=10)
    image[fs_oid * BLOCK : (fs_oid + 1) * BLOCK] = volume_superblock(
        fs_oid, root_tree_oid, meta_crypto=0xA11CE if encrypted else 0
    )
    image[root_tree_oid * BLOCK : (root_tree_oid + 1) * BLOCK] = btree_leaf(records, root_tree_oid)
    image[5 * BLOCK : 5 * BLOCK + len(complete_data)] = complete_data
    image[6 * BLOCK : 6 * BLOCK + len(partial_head)] = partial_head
    image[7 * BLOCK : 7 * BLOCK + len(encrypted_data)] = encrypted_data
    out_path.write_bytes(image)

    extracts: list[dict[str, object]] = []
    expected_entries: list[dict[str, object]] = []
    if include_complete:
        extracts.append(
            {
                "name": "COMPLETE.TXT",
                "inode": complete_inode,
                "expectedSha256": sha256(complete_data),
                "expectedBytes": len(complete_data),
                "expectedQuality": "complete",
                "expectedBytesLost": 0,
                "expectedEncryptedHint": False,
            }
        )
        expected_entries.append(
            {"path": "/COMPLETE.TXT", "status": "Allocated", "kind": "File", "size": len(complete_data)}
        )
    if include_partial:
        partial_output = partial_head + b"\x00" * (partial_size - len(partial_head))
        extracts.append(
            {
                "name": "PARTIAL.BIN",
                "inode": partial_inode,
                "expectedSha256": sha256(partial_output),
                "expectedBytes": partial_size,
                "expectedQuality": "suspect_partial",
                "expectedBytesLost": partial_size - len(partial_head),
                "expectedEncryptedHint": False,
            }
        )
        expected_entries.append(
            {"path": "/PARTIAL.BIN", "status": "Allocated", "kind": "File", "size": partial_size}
        )
    if encrypted:
        extracts.append(
            {
                "name": "ENCRYPTED.TXT",
                "inode": encrypted_inode,
                "expectedSha256": sha256(encrypted_data),
                "expectedBytes": len(encrypted_data),
                "expectedQuality": "suspect_partial",
                "expectedBytesLost": 0,
                "expectedEncryptedHint": True,
            }
        )
        expected_entries.append(
            {"path": "/ENCRYPTED.TXT", "status": "Allocated", "kind": "File", "size": len(encrypted_data)}
        )

    rel_image = str(out_path.resolve().relative_to(Path.cwd().resolve()))
    return {
        "fs": "apfs",
        "image": rel_image,
        "scanCommand": ["apfs-scan-vfs", "--file", rel_image, "--format", "json"],
        "extracts": extracts,
        "expectedEntries": expected_entries,
        "encryptedFixture": encrypted,
    }


def main() -> int:
    parser = argparse.ArgumentParser(description="Generate APFS quality-gate fixtures.")
    parser.add_argument("--out", default="fixtures/apfs_quality")
    args = parser.parse_args()

    out_dir = Path(args.out)
    out_dir.mkdir(parents=True, exist_ok=True)

    fixtures = [
        build_apfs_image(out_dir / "apfs_complete_partial.bin", encrypted=False, include_partial=True),
        build_apfs_image(out_dir / "apfs_encrypted_hint.bin", encrypted=True, include_partial=False),
    ]
    manifest = {
        "version": FIXTURE_VERSION,
        "generatedAt": FIXTURE_GENERATED_AT,
        "description": "APFS synthetic fixtures for complete, partial, and crypto-metadata extraction quality checks.",
        "fixtures": fixtures,
        "thresholds": {
            "minimumScanEntryMatchRatio": 1.0,
            "minimumExtractHashMatchRatio": 1.0,
            "maximumScanFailures": 0,
            "maximumExtractFailures": 0,
            "maximumQualityMismatches": 0,
            "maximumBytesLostMismatches": 0,
            "maximumEncryptedHintMismatches": 0,
        },
    }
    (out_dir / "expected_apfs_manifest.json").write_text(
        json.dumps(manifest, ensure_ascii=False, indent=2), encoding="utf-8"
    )
    print(f"wrote {out_dir / 'expected_apfs_manifest.json'}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
