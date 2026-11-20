#!/usr/bin/env python3
"""Generate a deterministic GPT image with FAT32 and exFAT partitions."""
from __future__ import annotations

import argparse
import json
import struct
import zlib
from pathlib import Path

SECTOR = 512


def guid_bytes(seed: int) -> bytes:
    return bytes(((seed + index * 17) & 0xFF) for index in range(16))


def write_header(image: bytearray, lba: int, backup_lba: int, entries_lba: int, array_crc: int) -> None:
    header = bytearray(SECTOR)
    header[0:8] = b"EFI PART"
    header[8:12] = struct.pack("<I", 0x0001_0000)
    header[12:16] = struct.pack("<I", 92)
    header[24:32] = struct.pack("<Q", lba)
    header[32:40] = struct.pack("<Q", backup_lba)
    header[40:48] = struct.pack("<Q", 34)
    header[48:56] = struct.pack("<Q", 220)
    header[56:72] = guid_bytes(0x50)
    header[72:80] = struct.pack("<Q", entries_lba)
    header[80:84] = struct.pack("<I", 4)
    header[84:88] = struct.pack("<I", 128)
    header[88:92] = struct.pack("<I", array_crc)
    header[16:20] = struct.pack("<I", zlib.crc32(header[:92]) & 0xFFFF_FFFF)
    image[lba * SECTOR : (lba + 1) * SECTOR] = header


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--out", default="fixtures/partition_quality")
    args = parser.parse_args()
    out = Path(args.out).resolve()
    out.mkdir(parents=True, exist_ok=True)

    sector_count = 256
    image = bytearray(sector_count * SECTOR)
    image[446 + 4] = 0xEE
    image[446 + 8 : 446 + 12] = struct.pack("<I", 1)
    image[446 + 12 : 446 + 16] = struct.pack("<I", sector_count - 2)
    image[510:512] = b"\x55\xAA"

    entries = bytearray(4 * 128)
    for index, (first_lba, last_lba, seed, name) in enumerate(
        [(34, 100, 0x10, "FAT32 fixture"), (101, 220, 0x30, "exFAT fixture")]
    ):
        start = index * 128
        entries[start : start + 16] = guid_bytes(seed)
        entries[start + 16 : start + 32] = guid_bytes(seed + 4)
        entries[start + 32 : start + 40] = struct.pack("<Q", first_lba)
        entries[start + 40 : start + 48] = struct.pack("<Q", last_lba)
        entries[start + 56 : start + 56 + len(name.encode("utf-16le"))] = name.encode("utf-16le")
    array_crc = zlib.crc32(entries) & 0xFFFF_FFFF
    image[2 * SECTOR : 2 * SECTOR + len(entries)] = entries
    image[252 * SECTOR : 252 * SECTOR + len(entries)] = entries
    write_header(image, 1, 255, 2, array_crc)
    write_header(image, 255, 1, 252, array_crc)
    image[34 * SECTOR + 82 : 34 * SECTOR + 90] = b"FAT32   "
    image[101 * SECTOR + 3 : 101 * SECTOR + 11] = b"EXFAT   "

    path = out / "gpt_fat32_exfat.bin"
    path.write_bytes(image)
    (out / "expected.json").write_text(
        json.dumps(
            {
                "table": "Gpt",
                "partitions": [
                    {"index": 1, "offset": 34 * SECTOR, "length": 67 * SECTOR, "filesystem": "FAT32"},
                    {"index": 2, "offset": 101 * SECTOR, "length": 120 * SECTOR, "filesystem": "exFAT"},
                ],
            },
            indent=2,
        ),
        encoding="utf-8",
    )
    print(f"created: {path}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
