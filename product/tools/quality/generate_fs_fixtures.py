#!/usr/bin/env python3
"""Generate deterministic filesystem-level fixtures for Polik Recovery v0.7.

The v0.6 fixture only measured raw file-carving quality.  These fixtures exercise
filesystem metadata recovery paths: deleted directory entries, cluster-chain reads,
and contiguous fallback for freshly deleted files.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import struct
from pathlib import Path

SECTOR = 512
FIXTURE_VERSION = "v0.7"
FIXTURE_GENERATED_AT = "2000-01-01T00:00:00+00:00"


def sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def write_at(image: bytearray, offset: int, data: bytes) -> None:
    image[offset : offset + len(data)] = data


def cluster_offset(first_data_sector: int, cluster: int) -> int:
    return (first_data_sector + (cluster - 2)) * SECTOR


def fat32_short_entry(raw_name: bytes, first_cluster: int, size: int, *, deleted: bool = False) -> bytes:
    if len(raw_name) != 11:
        raise ValueError("FAT32 short name must be 11 bytes")
    name = bytearray(raw_name)
    if deleted:
        name[0] = 0xE5
    entry = bytearray(32)
    entry[0:11] = name
    entry[11] = 0x20  # archive/file
    entry[20:22] = struct.pack("<H", (first_cluster >> 16) & 0xFFFF)
    entry[26:28] = struct.pack("<H", first_cluster & 0xFFFF)
    entry[28:32] = struct.pack("<I", size)
    return bytes(entry)


def build_fat32_image(out_dir: Path) -> dict[str, object]:
    total_sectors = 48
    reserved_sectors = 1
    fat_size = 1
    number_of_fats = 1
    first_data_sector = reserved_sectors + number_of_fats * fat_size
    image = bytearray(total_sectors * SECTOR)

    active_data = b"FAT32 allocated fixture data\n"
    deleted_data = b"FAT32 freshly deleted contiguous fallback data\n"

    boot = bytearray(SECTOR)
    boot[0:3] = b"\xEB\x58\x90"
    boot[3:11] = b"MSDOS5.0"
    boot[11:13] = struct.pack("<H", SECTOR)
    boot[13] = 1  # sectors per cluster
    boot[14:16] = struct.pack("<H", reserved_sectors)
    boot[16] = number_of_fats
    boot[17:19] = struct.pack("<H", 0)  # root entry count, FAT32 uses cluster root
    boot[19:21] = struct.pack("<H", 0)
    boot[21] = 0xF8
    boot[22:24] = struct.pack("<H", 0)
    boot[24:26] = struct.pack("<H", 63)
    boot[26:28] = struct.pack("<H", 255)
    boot[28:32] = struct.pack("<I", 0)
    boot[32:36] = struct.pack("<I", total_sectors)
    boot[36:40] = struct.pack("<I", fat_size)
    boot[40:42] = struct.pack("<H", 0)
    boot[42:44] = struct.pack("<H", 0)
    boot[44:48] = struct.pack("<I", 2)  # root cluster
    boot[48:50] = struct.pack("<H", 1)
    boot[50:52] = struct.pack("<H", 6)
    boot[64] = 0x80
    boot[66] = 0x29
    boot[67:71] = struct.pack("<I", 0x504F4C49)
    boot[71:82] = b"POLIK V07  "
    boot[82:90] = b"FAT32   "
    boot[510:512] = b"\x55\xAA"
    write_at(image, 0, boot)

    fat = bytearray(SECTOR)
    for cluster, value in {
        0: 0x0FFFFFF8,
        1: 0x0FFFFFFF,
        2: 0x0FFFFFFF,  # root dir
        3: 0x0FFFFFFF,  # allocated file
        4: 0x00000000,  # deleted file chain often zeroed; extractor must use fallback
    }.items():
        fat[cluster * 4 : cluster * 4 + 4] = struct.pack("<I", value)
    write_at(image, SECTOR, fat)

    root = bytearray(SECTOR)
    root[0:32] = fat32_short_entry(b"ACTIVE  TXT", 3, len(active_data))
    root[32:64] = fat32_short_entry(b"DELETE  TXT", 4, len(deleted_data), deleted=True)
    root[64] = 0x00
    root_offset = cluster_offset(first_data_sector, 2)
    write_at(image, root_offset, root)
    write_at(image, cluster_offset(first_data_sector, 3), active_data)
    write_at(image, cluster_offset(first_data_sector, 4), deleted_data)

    path = out_dir / "fat32_image.bin"
    path.write_bytes(image)
    return {
        "fs": "fat32",
        "image": str(path.relative_to(Path.cwd())),
        "scanCommand": ["fat32-scan-vfs", "--file", str(path.relative_to(Path.cwd())), "--format", "json"],
        "extracts": [
            {
                "name": "ACTIVE.TXT",
                "entryId": root_offset,
                "expectedSha256": sha256(active_data),
                "expectedBytes": len(active_data),
                "status": "Allocated",
            },
            {
                "name": "_ELETE.TXT",
                "entryId": root_offset + 32,
                "expectedSha256": sha256(deleted_data),
                "expectedBytes": len(deleted_data),
                "status": "Deleted",
                "recoveryMode": "deleted-contiguous-fallback",
            },
        ],
        "expectedEntries": [
            {"path": "/ACTIVE.TXT", "status": "Allocated", "kind": "File", "size": len(active_data)},
            {"path": "/_ELETE.TXT", "status": "Deleted", "kind": "File", "size": len(deleted_data)},
        ],
    }


def exfat_file_set(name: str, first_cluster: int, data_len: int, *, deleted: bool = False) -> bytes:
    file_type = 0x05 if deleted else 0x85
    stream_type = 0x40 if deleted else 0xC0
    name_type = 0x41 if deleted else 0xC1
    file_entry = bytearray(32)
    file_entry[0] = file_type
    file_entry[1] = 2  # stream extension + one name entry
    file_entry[4:6] = struct.pack("<H", 0x20)

    stream = bytearray(32)
    stream[0] = stream_type
    stream[1] = 0x02  # no FAT chain / contiguous allocation
    stream[3] = len(name)
    stream[20:24] = struct.pack("<I", first_cluster)
    stream[24:32] = struct.pack("<Q", data_len)

    name_entry = bytearray(32)
    name_entry[0] = name_type
    encoded = name.encode("utf-16le")
    name_entry[2 : 2 + min(len(encoded), 30)] = encoded[:30]
    return bytes(file_entry + stream + name_entry)


def align8(value: int) -> int:
    return (value + 7) & ~7


def ntfs_resident_attribute(attribute_type: int, value: bytes, attribute_id: int) -> bytes:
    length = align8(24 + len(value))
    attribute = bytearray(length)
    attribute[0:4] = struct.pack("<I", attribute_type)
    attribute[4:8] = struct.pack("<I", length)
    attribute[8] = 0
    attribute[14:16] = struct.pack("<H", attribute_id)
    attribute[16:20] = struct.pack("<I", len(value))
    attribute[20:22] = struct.pack("<H", 24)
    attribute[24 : 24 + len(value)] = value
    return bytes(attribute)


def ntfs_filename_value(name: str, parent_record: int, real_size: int) -> bytes:
    value = bytearray(66 + len(name.encode("utf-16le")))
    value[0:8] = struct.pack("<Q", parent_record)
    value[40:48] = struct.pack("<Q", real_size)
    value[48:56] = struct.pack("<Q", real_size)
    value[56:60] = struct.pack("<I", 0x10 if real_size == 0 else 0x20)
    value[64] = len(name)
    value[65] = 1  # Win32 namespace
    value[66:] = name.encode("utf-16le")
    return bytes(value)


def ntfs_record(
    record_number: int,
    *,
    name: str | None = None,
    parent_record: int = 0,
    real_size: int = 0,
    in_use: bool = True,
    directory: bool = False,
    resident_data: bytes | None = None,
    mft_mapping_pairs: bytes | None = None,
    mft_real_size: int = 0,
) -> bytes:
    record = bytearray(1024)
    record[0:4] = b"FILE"
    record[4:6] = struct.pack("<H", 0x30)
    record[6:8] = struct.pack("<H", 3)
    record[8:16] = struct.pack("<Q", 0x504F4C494D4654 + record_number)
    record[16:18] = struct.pack("<H", 1)
    record[18:20] = struct.pack("<H", 1)
    record[20:22] = struct.pack("<H", 0x38)
    record[22:24] = struct.pack("<H", (1 if in_use else 0) | (2 if directory else 0))
    record[28:32] = struct.pack("<I", 1024)
    record[32:40] = struct.pack("<Q", 0)
    record[40:42] = struct.pack("<H", 1)
    record[44:48] = struct.pack("<I", record_number)

    attributes = bytearray()
    attribute_id = 0
    if name is not None:
        attributes.extend(ntfs_resident_attribute(0x30, ntfs_filename_value(name, parent_record, real_size), attribute_id))
        attribute_id += 1
    if resident_data is not None:
        attributes.extend(ntfs_resident_attribute(0x80, resident_data, attribute_id))
        attribute_id += 1
    if mft_mapping_pairs is not None:
        length = align8(0x40 + len(mft_mapping_pairs))
        attribute = bytearray(length)
        attribute[0:4] = struct.pack("<I", 0x80)
        attribute[4:8] = struct.pack("<I", length)
        attribute[8] = 1
        attribute[32:34] = struct.pack("<H", 0x40)
        attribute[40:48] = struct.pack("<Q", mft_real_size)
        attribute[48:56] = struct.pack("<Q", mft_real_size)
        attribute[56:64] = struct.pack("<Q", mft_real_size)
        attribute[64 : 64 + len(mft_mapping_pairs)] = mft_mapping_pairs
        attributes.extend(attribute)
        attribute_id += 1

    end = 0x38 + len(attributes)
    record[0x38:end] = attributes
    record[end : end + 4] = b"\xff\xff\xff\xff"
    record[24:28] = struct.pack("<I", align8(end + 4))
    record[0x30:0x36] = b"\xaa\xaa\xcd\xab\xbb\xbb"
    record[510:512] = b"\xaa\xaa"
    record[1022:1024] = b"\xaa\xaa"
    return bytes(record)


def build_ntfs_fragmented_mft(out_dir: Path) -> dict[str, object]:
    total_sectors = 256
    sectors_per_cluster = 8
    cluster_size = SECTOR * sectors_per_cluster
    image = bytearray(total_sectors * SECTOR)
    recovered_data = b"NTFS fragmented MFT recovery fixture\n"

    boot = bytearray(SECTOR)
    boot[0:3] = b"\xeb\x52\x90"
    boot[3:11] = b"NTFS    "
    boot[11:13] = struct.pack("<H", SECTOR)
    boot[13] = sectors_per_cluster
    boot[21] = 0xF8
    boot[40:48] = struct.pack("<Q", total_sectors)
    boot[48:56] = struct.pack("<Q", 1)
    boot[56:64] = struct.pack("<Q", 2)
    boot[64] = 0xF6  # -10 => 1024-byte MFT records
    boot[68] = 1
    boot[72:80] = struct.pack("<Q", 0x20261120)
    boot[510:512] = b"\x55\xaa"
    write_at(image, 0, boot)

    # $MFT logical clusters 0..2 are physically split across LCN 1, 10, 20.
    mapping_pairs = bytes([0x11, 1, 1, 0x11, 1, 9, 0x11, 1, 10, 0])
    record_zero = ntfs_record(
        0,
        mft_mapping_pairs=mapping_pairs,
        mft_real_size=3 * cluster_size,
    )
    write_at(image, cluster_size, record_zero)

    report_record = ntfs_record(
        4,
        name="report.txt",
        parent_record=8,
        real_size=len(recovered_data),
        in_use=False,
        resident_data=recovered_data,
    )
    write_at(image, 10 * cluster_size, report_record)

    root_record = ntfs_record(8, name="root", parent_record=8, directory=True)
    write_at(image, 20 * cluster_size, root_record)

    path = out_dir / "ntfs_fragmented_mft.bin"
    path.write_bytes(image)
    image_rel = str(path.relative_to(Path.cwd()))
    return {
        "fs": "ntfs-fragmented",
        "image": image_rel,
        "scanCommand": ["ntfs-scan-vfs", "--file", image_rel, "--count", "9", "--format", "json"],
        "expectedEntries": [
            {"path": "/root", "status": "Allocated", "kind": "Directory", "size": 0},
            {"path": "/root/report.txt", "status": "Deleted", "kind": "File", "size": len(recovered_data)},
        ],
        "extracts": [
            {
                "name": "report.txt",
                "image": image_rel,
                "record": 4,
                "expectedSha256": sha256(recovered_data),
                "expectedBytes": len(recovered_data),
                "status": "Complete",
            }
        ],
    }


def build_exfat_image(out_dir: Path) -> dict[str, object]:
    total_sectors = 48
    cluster_heap_sector = 2
    image = bytearray(total_sectors * SECTOR)
    active_data = b"exFAT allocated contiguous fixture data\n"
    deleted_data = b"exFAT deleted no-FAT-chain fallback data\n"

    boot = bytearray(SECTOR)
    boot[0:3] = b"\xEB\x76\x90"
    boot[3:11] = b"EXFAT   "
    boot[64:72] = struct.pack("<Q", 0)
    boot[72:80] = struct.pack("<Q", total_sectors)
    boot[80:84] = struct.pack("<I", 1)  # FAT offset
    boot[84:88] = struct.pack("<I", 1)  # FAT length
    boot[88:92] = struct.pack("<I", cluster_heap_sector)
    boot[92:96] = struct.pack("<I", 32)
    boot[96:100] = struct.pack("<I", 2)  # root dir cluster
    boot[100:104] = struct.pack("<I", 0x504F4C49)
    boot[104:106] = struct.pack("<H", 0x0100)
    boot[106:108] = struct.pack("<H", 0)
    boot[108] = 9  # 512 bytes/sector
    boot[109] = 0  # one sector per cluster
    boot[110] = 1
    boot[510:512] = b"\x55\xAA"
    write_at(image, 0, boot)

    fat = bytearray(SECTOR)
    for cluster, value in {0: 0xFFFFFFF8, 1: 0xFFFFFFFF, 2: 0xFFFFFFFF}.items():
        fat[cluster * 4 : cluster * 4 + 4] = struct.pack("<I", value)
    write_at(image, SECTOR, fat)

    root_offset = cluster_heap_sector * SECTOR
    root = bytearray(SECTOR)
    root[0:96] = exfat_file_set("ACTIVE.TXT", 3, len(active_data), deleted=False)
    root[96:192] = exfat_file_set("DELETE.TXT", 4, len(deleted_data), deleted=True)
    root[192] = 0x00
    write_at(image, root_offset, root)
    write_at(image, cluster_offset(cluster_heap_sector, 3), active_data)
    write_at(image, cluster_offset(cluster_heap_sector, 4), deleted_data)

    path = out_dir / "exfat_image.bin"
    path.write_bytes(image)
    return {
        "fs": "exfat",
        "image": str(path.relative_to(Path.cwd())),
        "scanCommand": ["exfat-scan-vfs", "--file", str(path.relative_to(Path.cwd())), "--format", "json"],
        "extracts": [
            {
                "name": "ACTIVE.TXT",
                "entryId": root_offset,
                "expectedSha256": sha256(active_data),
                "expectedBytes": len(active_data),
                "status": "Allocated",
            },
            {
                "name": "DELETE.TXT",
                "entryId": root_offset + 96,
                "expectedSha256": sha256(deleted_data),
                "expectedBytes": len(deleted_data),
                "status": "Deleted",
                "recoveryMode": "no-fat-chain-contiguous",
            },
        ],
        "expectedEntries": [
            {"path": "/ACTIVE.TXT", "status": "Allocated", "kind": "File", "size": len(active_data)},
            {"path": "/DELETE.TXT", "status": "Deleted", "kind": "File", "size": len(deleted_data)},
        ],
    }


def build_manifest(out_dir: Path) -> dict[str, object]:
    fat32 = build_fat32_image(out_dir)
    exfat = build_exfat_image(out_dir)
    ntfs_fragmented = build_ntfs_fragmented_mft(out_dir)
    ntfs_extract_data = b"hello recovered data"
    return {
        "version": FIXTURE_VERSION,
        "createdAt": FIXTURE_GENERATED_AT,
        "description": "Filesystem-level quality fixtures for NTFS/exFAT/FAT32 metadata recovery paths.",
        "fixtures": [
            {
                "fs": "ntfs",
                "image": "fixtures/images/ntfs_volume_with_mft.bin",
                "scanCommand": ["ntfs-scan-vfs", "--file", "fixtures/images/ntfs_volume_with_mft.bin", "--count", "2", "--format", "json"],
                "expectedEntries": [
                    {"path": "/root/report.txt", "status": "Deleted", "kind": "File", "size": 1234}
                ],
                "extracts": [
                    {
                        "name": "report.txt",
                        "image": "fixtures/images/ntfs_volume_extract.bin",
                        "record": 0,
                        "expectedSha256": sha256(ntfs_extract_data),
                        "expectedBytes": len(ntfs_extract_data),
                        "status": "Complete",
                    }
                ],
            },
            ntfs_fragmented,
            fat32,
            exfat,
        ],
        "thresholds": {
            "minimumScanEntryMatchRatio": 1.0,
            "minimumExtractHashMatchRatio": 1.0,
            "maximumScanFailures": 0,
            "maximumExtractFailures": 0,
        },
    }


def main() -> int:
    parser = argparse.ArgumentParser(description="Generate v0.7 filesystem quality fixtures.")
    parser.add_argument("--out", default="fixtures/fs_quality", help="Output fixture directory")
    args = parser.parse_args()
    out_dir = Path(args.out).resolve()
    out_dir.mkdir(parents=True, exist_ok=True)
    manifest = build_manifest(out_dir)
    manifest_path = out_dir / "expected_fs_manifest.json"
    manifest_path.write_text(json.dumps(manifest, ensure_ascii=False, indent=2), encoding="utf-8")
    print(f"created: {out_dir}")
    print(f"manifest: {manifest_path}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
