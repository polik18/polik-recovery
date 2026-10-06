#!/usr/bin/env python3
"""Generate v0.8 fragmented and partial-recovery filesystem fixtures.

These fixtures are intentionally small but model the cases that made the product
feel unreliable in real use: non-contiguous cluster chains, deleted files whose
chain is still recoverable, and deleted/partial files where only a best-effort
contiguous fallback is possible.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import struct
from pathlib import Path

SECTOR = 512
FIXTURE_VERSION = "v0.8"
FIXTURE_GENERATED_AT = "2000-01-01T00:00:00+00:00"


def sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def write_at(image: bytearray, offset: int, data: bytes) -> None:
    image[offset : offset + len(data)] = data


def cluster_offset(first_data_sector: int, cluster: int) -> int:
    return (first_data_sector + (cluster - 2)) * SECTOR


def split_clusters(data: bytes) -> list[bytes]:
    return [data[i : i + SECTOR] for i in range(0, len(data), SECTOR)]


def fat32_short_entry(raw_name: bytes, first_cluster: int, size: int, *, deleted: bool = False) -> bytes:
    if len(raw_name) != 11:
        raise ValueError("FAT32 short name must be exactly 11 bytes")
    name = bytearray(raw_name)
    if deleted:
        name[0] = 0xE5
    entry = bytearray(32)
    entry[0:11] = name
    entry[11] = 0x20
    entry[20:22] = struct.pack("<H", (first_cluster >> 16) & 0xFFFF)
    entry[26:28] = struct.pack("<H", first_cluster & 0xFFFF)
    entry[28:32] = struct.pack("<I", size)
    return bytes(entry)


def build_fat32_fragmented(out_dir: Path) -> dict[str, object]:
    total_sectors = 96
    reserved_sectors = 1
    fat_size = 1
    number_of_fats = 1
    first_data_sector = reserved_sectors + number_of_fats * fat_size
    image = bytearray(total_sectors * SECTOR)

    active_fragmented = (b"FAT32 allocated fragmented file: " + b"A" * 620)[:700]
    deleted_chain = (b"FAT32 deleted file with preserved chain: " + b"B" * 620)[:700]
    deleted_missing_chain = (b"FAT32 deleted missing chain real payload: " + b"C" * 620)[:700]
    wrong_contiguous_second_cluster = b"WRONG-CONTIGUOUS-FALLBACK-CLUSTER" + b"X" * 480

    boot = bytearray(SECTOR)
    boot[0:3] = b"\xEB\x58\x90"
    boot[3:11] = b"MSDOS5.0"
    boot[11:13] = struct.pack("<H", SECTOR)
    boot[13] = 1
    boot[14:16] = struct.pack("<H", reserved_sectors)
    boot[16] = number_of_fats
    boot[17:19] = struct.pack("<H", 0)
    boot[19:21] = struct.pack("<H", 0)
    boot[21] = 0xF8
    boot[22:24] = struct.pack("<H", 0)
    boot[24:26] = struct.pack("<H", 63)
    boot[26:28] = struct.pack("<H", 255)
    boot[32:36] = struct.pack("<I", total_sectors)
    boot[36:40] = struct.pack("<I", fat_size)
    boot[44:48] = struct.pack("<I", 2)
    boot[48:50] = struct.pack("<H", 1)
    boot[50:52] = struct.pack("<H", 6)
    boot[64] = 0x80
    boot[66] = 0x29
    boot[67:71] = struct.pack("<I", 0x504F4C38)
    boot[71:82] = b"POLIK V08  "
    boot[82:90] = b"FAT32   "
    boot[510:512] = b"\x55\xAA"
    write_at(image, 0, boot)

    fat = bytearray(SECTOR)
    fat_entries = {
        0: 0x0FFFFFF8,
        1: 0x0FFFFFFF,
        2: 0x0FFFFFFF,  # root dir
        3: 5,           # active fragmented: cluster 3 -> 5
        5: 0x0FFFFFFF,
        6: 8,           # deleted but chain preserved: cluster 6 -> 8
        8: 0x0FFFFFFF,
        9: 0x00000000,  # deleted missing chain: fallback will read 9 -> 10, but real second is 11
        10: 0x00000000,
        11: 0x00000000,
    }
    for cluster, value in fat_entries.items():
        fat[cluster * 4 : cluster * 4 + 4] = struct.pack("<I", value)
    write_at(image, SECTOR, fat)

    root = bytearray(SECTOR)
    root[0:32] = fat32_short_entry(b"FRAG    TXT", 3, len(active_fragmented))
    root[32:64] = fat32_short_entry(b"DELCHN  TXT", 6, len(deleted_chain), deleted=True)
    root[64:96] = fat32_short_entry(b"DELMIS  TXT", 9, len(deleted_missing_chain), deleted=True)
    root[96] = 0x00
    root_offset = cluster_offset(first_data_sector, 2)
    write_at(image, root_offset, root)

    for cluster, chunk in zip([3, 5], split_clusters(active_fragmented)):
        write_at(image, cluster_offset(first_data_sector, cluster), chunk)
    for cluster, chunk in zip([6, 8], split_clusters(deleted_chain)):
        write_at(image, cluster_offset(first_data_sector, cluster), chunk)
    chunks = split_clusters(deleted_missing_chain)
    write_at(image, cluster_offset(first_data_sector, 9), chunks[0])
    write_at(image, cluster_offset(first_data_sector, 10), wrong_contiguous_second_cluster[:SECTOR])
    write_at(image, cluster_offset(first_data_sector, 11), chunks[1])

    path = out_dir / "fat32_fragmented.bin"
    path.write_bytes(image)
    image_rel = str(path.relative_to(Path.cwd()))
    return {
        "fs": "fat32-fragmented",
        "driver": "fat32",
        "image": image_rel,
        "scanCommand": ["fat32-scan-vfs", "--file", image_rel, "--format", "json"],
        "expectedEntries": [
            {"path": "/FRAG.TXT", "status": "Allocated", "kind": "File", "size": len(active_fragmented)},
            {"path": "/_ELCHN.TXT", "status": "Deleted", "kind": "File", "size": len(deleted_chain)},
            {"path": "/_ELMIS.TXT", "status": "Deleted", "kind": "File", "size": len(deleted_missing_chain)},
        ],
        "extracts": [
            {"name": "FRAG.TXT", "entryId": root_offset, "expectedSha256": sha256(active_fragmented), "expectedBytes": len(active_fragmented), "expectedQuality": "complete", "expectedMode": "FatChain"},
            {"name": "_ELCHN.TXT", "entryId": root_offset + 32, "expectedSha256": sha256(deleted_chain), "expectedBytes": len(deleted_chain), "expectedQuality": "complete", "expectedMode": "FatChain"},
            {"name": "_ELMIS.TXT", "entryId": root_offset + 64, "expectedSha256": sha256(deleted_missing_chain), "expectedBytes": len(deleted_missing_chain), "expectedQuality": "suspect_partial", "expectedMode": "ContiguousFallback", "allowHashMismatch": True},
        ],
    }


def exfat_file_set(name: str, first_cluster: int, data_len: int, *, no_fat_chain: bool, deleted: bool = False) -> bytes:
    file_type = 0x05 if deleted else 0x85
    stream_type = 0x40 if deleted else 0xC0
    name_type = 0x41 if deleted else 0xC1
    file_entry = bytearray(32)
    file_entry[0] = file_type
    file_entry[1] = 2
    file_entry[4:6] = struct.pack("<H", 0x20)

    stream = bytearray(32)
    stream[0] = stream_type
    stream[1] = 0x02 if no_fat_chain else 0x00
    stream[3] = len(name)
    stream[20:24] = struct.pack("<I", first_cluster)
    stream[24:32] = struct.pack("<Q", data_len)

    name_entry = bytearray(32)
    name_entry[0] = name_type
    encoded = name.encode("utf-16le")
    name_entry[2 : 2 + min(len(encoded), 30)] = encoded[:30]
    return bytes(file_entry + stream + name_entry)


def build_exfat_fragmented(out_dir: Path) -> dict[str, object]:
    total_sectors = 96
    cluster_heap_sector = 2
    image = bytearray(total_sectors * SECTOR)
    active_fragmented = (b"exFAT FAT-chain fragmented file: " + b"D" * 620)[:700]
    short_chain_partial = (b"exFAT partial short chain expected payload: " + b"E" * 620)[:700]

    boot = bytearray(SECTOR)
    boot[0:3] = b"\xEB\x76\x90"
    boot[3:11] = b"EXFAT   "
    boot[64:72] = struct.pack("<Q", 0)
    boot[72:80] = struct.pack("<Q", total_sectors)
    boot[80:84] = struct.pack("<I", 1)
    boot[84:88] = struct.pack("<I", 1)
    boot[88:92] = struct.pack("<I", cluster_heap_sector)
    boot[92:96] = struct.pack("<I", 64)
    boot[96:100] = struct.pack("<I", 2)
    boot[100:104] = struct.pack("<I", 0x504F4C38)
    boot[104:106] = struct.pack("<H", 0x0100)
    boot[108] = 9
    boot[109] = 0
    boot[110] = 1
    boot[510:512] = b"\x55\xAA"
    write_at(image, 0, boot)

    fat = bytearray(SECTOR)
    fat_entries = {
        0: 0xFFFFFFF8,
        1: 0xFFFFFFFF,
        2: 0xFFFFFFFF,
        3: 5,
        5: 0xFFFFFFFF,
        6: 0xFFFFFFFF,  # partial: entry claims 700 bytes but chain gives only one cluster
    }
    for cluster, value in fat_entries.items():
        fat[cluster * 4 : cluster * 4 + 4] = struct.pack("<I", value)
    write_at(image, SECTOR, fat)

    root_offset = cluster_heap_sector * SECTOR
    root = bytearray(SECTOR)
    root[0:96] = exfat_file_set("FRAG.TXT", 3, len(active_fragmented), no_fat_chain=False)
    root[96:192] = exfat_file_set("SHORT.TXT", 6, len(short_chain_partial), no_fat_chain=False)
    root[192] = 0x00
    write_at(image, root_offset, root)

    for cluster, chunk in zip([3, 5], split_clusters(active_fragmented)):
        write_at(image, cluster_offset(cluster_heap_sector, cluster), chunk)
    write_at(image, cluster_offset(cluster_heap_sector, 6), split_clusters(short_chain_partial)[0])

    path = out_dir / "exfat_fragmented.bin"
    path.write_bytes(image)
    image_rel = str(path.relative_to(Path.cwd()))
    return {
        "fs": "exfat-fragmented",
        "driver": "exfat",
        "image": image_rel,
        "scanCommand": ["exfat-scan-vfs", "--file", image_rel, "--format", "json"],
        "expectedEntries": [
            {"path": "/FRAG.TXT", "status": "Allocated", "kind": "File", "size": len(active_fragmented)},
            {"path": "/SHORT.TXT", "status": "Allocated", "kind": "File", "size": len(short_chain_partial)},
        ],
        "extracts": [
            {"name": "FRAG.TXT", "entryId": root_offset, "expectedSha256": sha256(active_fragmented), "expectedBytes": len(active_fragmented), "expectedQuality": "complete", "expectedMode": "FatChain"},
            {"name": "SHORT.TXT", "entryId": root_offset + 96, "expectedSha256": sha256(short_chain_partial), "expectedBytes": len(short_chain_partial), "expectedQuality": "suspect_partial", "expectedMode": "FatChain", "allowHashMismatch": True},
        ],
    }


def build_manifest(out_dir: Path) -> dict[str, object]:
    return {
        "version": FIXTURE_VERSION,
        "createdAt": FIXTURE_GENERATED_AT,
        "description": "Fragmentation and partial-recovery quality fixtures for FAT32/exFAT plus NTFS runlist regression coverage.",
        "fixtures": [build_fat32_fragmented(out_dir), build_exfat_fragmented(out_dir)],
        "thresholds": {
            "minimumScanEntryMatchRatio": 1.0,
            "minimumCompleteExtractHashRatio": 1.0,
            "minimumSuspectClassificationRatio": 1.0,
            "maximumUnexpectedCompleteForSuspect": 0,
            "maximumScanFailures": 0,
        },
    }


def main() -> int:
    parser = argparse.ArgumentParser(description="Generate v0.8 fragmentation quality fixtures.")
    parser.add_argument("--out", default="fixtures/fragmented_quality", help="Output fixture directory")
    args = parser.parse_args()
    out_dir = Path(args.out).resolve()
    out_dir.mkdir(parents=True, exist_ok=True)
    manifest = build_manifest(out_dir)
    manifest_path = out_dir / "expected_fragmented_manifest.json"
    manifest_path.write_text(json.dumps(manifest, ensure_ascii=False, indent=2), encoding="utf-8")
    print(f"created: {out_dir}")
    print(f"manifest: {manifest_path}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
