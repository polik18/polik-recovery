#!/usr/bin/env python3
"""Create deterministic quality-gate fixtures for Polik Recovery.

This script builds a small synthetic disk image containing known-good files,
deliberately broken signatures, and padding/noise.  It is intentionally stdlib
only so it can run on a clean developer Mac before the Rust toolchain is ready.
"""
from __future__ import annotations

import argparse
import hashlib
import io
import json
import os
import random
import struct
import zlib
import zipfile
from pathlib import Path
from typing import Iterable

FIXTURE_VERSION = "v0.6"
FIXTURE_GENERATED_AT = "2000-01-01T00:00:00+00:00"
ZIP_TIMESTAMP = (2000, 1, 1, 0, 0, 0)
RANDOM_SEED = 0x504F4C494B  # POLIK


def sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def png_chunk(kind: bytes, payload: bytes) -> bytes:
    return (
        struct.pack(">I", len(payload))
        + kind
        + payload
        + struct.pack(">I", zlib.crc32(kind + payload) & 0xFFFFFFFF)
    )


def make_png() -> bytes:
    # 2x2 RGB image, four visible pixels.  No external imaging libraries needed.
    width, height = 2, 2
    raw_rows = [
        b"\x00" + bytes([255, 0, 0, 0, 255, 0]),
        b"\x00" + bytes([0, 0, 255, 255, 255, 0]),
    ]
    ihdr = struct.pack(">IIBBBBB", width, height, 8, 2, 0, 0, 0)
    return (
        b"\x89PNG\r\n\x1a\n"
        + png_chunk(b"IHDR", ihdr)
        + png_chunk(b"IDAT", zlib.compress(b"".join(raw_rows), level=9))
        + png_chunk(b"IEND", b"")
    )


def make_minimal_jpeg() -> bytes:
    # Structurally valid enough for the project validator: SOI, APP0, DQT, SOF0, SOS, EOI.
    app0_payload = b"JFIF\x00\x01\x01\x00\x00\x01\x00\x01\x00\x00"
    dqt_payload = bytes([0x00]) + bytes(range(1, 65))
    sof0_payload = bytes([8, 0, 2, 0, 2, 3, 1, 0x11, 0, 2, 0x11, 0, 3, 0x11, 0])
    sos_payload = bytes([3, 1, 0, 2, 0x11, 3, 0x11, 0, 0x3F, 0])
    entropy = b"\x11\x22\x33\x44\x55\x66\x77\x88" * 8

    def segment(marker: int, payload: bytes) -> bytes:
        return b"\xff" + bytes([marker]) + struct.pack(">H", len(payload) + 2) + payload

    return (
        b"\xff\xd8"
        + segment(0xE0, app0_payload)
        + segment(0xDB, dqt_payload)
        + segment(0xC0, sof0_payload)
        + segment(0xDA, sos_payload)
        + entropy
        + b"\xff\xd9"
    )


def make_pdf() -> bytes:
    # Build a small PDF and compute xref offsets correctly.
    parts: list[bytes] = [b"%PDF-1.4\n"]
    offsets = [0]
    objects = [
        b"1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n",
        b"2 0 obj\n<< /Type /Pages /Kids [3 0 R] /Count 1 >>\nendobj\n",
        b"3 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 200 120] /Contents 4 0 R >>\nendobj\n",
        b"4 0 obj\n<< /Length 44 >>\nstream\nBT /F1 12 Tf 20 60 Td (Polik fixture) Tj ET\nendstream\nendobj\n",
    ]
    current = len(parts[0])
    for obj in objects:
        offsets.append(current)
        parts.append(obj)
        current += len(obj)
    xref_offset = current
    xref = [b"xref\n0 5\n", b"0000000000 65535 f \n"]
    for off in offsets[1:]:
        xref.append(f"{off:010d} 00000 n \n".encode())
    trailer = (
        b"trailer\n<< /Root 1 0 R /Size 5 >>\n"
        + b"startxref\n"
        + str(xref_offset).encode()
        + b"\n%%EOF\n"
    )
    return b"".join(parts + xref + [trailer])


def make_zip(entries: dict[str, bytes]) -> bytes:
    buf = io.BytesIO()
    with zipfile.ZipFile(buf, "w", compression=zipfile.ZIP_DEFLATED) as zf:
        for name, content in entries.items():
            info = zipfile.ZipInfo(name, date_time=ZIP_TIMESTAMP)
            info.compress_type = zipfile.ZIP_DEFLATED
            info.external_attr = 0o600 << 16
            zf.writestr(info, content)
    return buf.getvalue()


def make_docx() -> bytes:
    return make_zip({
        "[Content_Types].xml": b"<Types><Override PartName='/word/document.xml' ContentType='application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml'/></Types>",
        "_rels/.rels": b"<Relationships></Relationships>",
        "word/document.xml": b"<w:document><w:body><w:p><w:r><w:t>Polik fixture DOCX</w:t></w:r></w:p></w:body></w:document>",
    })


def make_xlsx() -> bytes:
    return make_zip({
        "[Content_Types].xml": b"<Types><Override PartName='/xl/workbook.xml' ContentType='application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml'/></Types>",
        "xl/workbook.xml": b"<workbook><sheets><sheet name='Sheet1' sheetId='1'/></sheets></workbook>",
        "xl/worksheets/sheet1.xml": b"<worksheet><sheetData></sheetData></worksheet>",
    })


def make_pptx() -> bytes:
    return make_zip({
        "[Content_Types].xml": b"<Types><Override PartName='/ppt/presentation.xml' ContentType='application/vnd.openxmlformats-officedocument.presentationml.presentation.main+xml'/></Types>",
        "ppt/presentation.xml": b"<p:presentation><p:sldIdLst></p:sldIdLst></p:presentation>",
    })


def bmff_box(kind: bytes, payload: bytes) -> bytes:
    return struct.pack(">I", len(payload) + 8) + kind + payload


def make_mp4() -> bytes:
    ftyp = bmff_box(b"ftyp", b"isom\x00\x00\x02\x00isomiso2mp41")
    moov = bmff_box(b"moov", bmff_box(b"mvhd", b"\x00" * 96))
    mdat = bmff_box(b"mdat", b"POLIK-MEDIA-DATA" * 16)
    return ftyp + moov + mdat


def make_heic_like() -> bytes:
    # Tiny HEIF-like BMFF fixture.  It is not a photograph, but exercises HEIC/HEIF signature logic.
    ftyp = bmff_box(b"ftyp", b"heic\x00\x00\x00\x00mif1heic")
    meta = bmff_box(b"meta", b"\x00\x00\x00\x00" + bmff_box(b"iinf", b"\x00" * 16) + bmff_box(b"iloc", b"\x00" * 16))
    return ftyp + meta


def deterministic_noise(rng: random.Random, length: int) -> bytes:
    return bytes(rng.randrange(0, 256) for _ in range(length))


def write_sample_files(samples_dir: Path) -> list[dict[str, object]]:
    samples_dir.mkdir(parents=True, exist_ok=True)
    definitions: list[tuple[str, str, bytes, str]] = [
        ("valid_photo.png", "PNG", make_png(), "png"),
        ("valid_photo.jpg", "JPEG", make_minimal_jpeg(), "jpg"),
        ("valid_document.pdf", "PDF", make_pdf(), "pdf"),
        ("valid_word.docx", "ZIP", make_docx(), "docx"),
        ("valid_sheet.xlsx", "ZIP", make_xlsx(), "xlsx"),
        ("valid_slides.pptx", "ZIP", make_pptx(), "pptx"),
        ("valid_movie.mp4", "MP4", make_mp4(), "mp4"),
        ("valid_iphone_photo.heic", "HEIC", make_heic_like(), "heic"),
    ]
    records: list[dict[str, object]] = []
    for filename, file_type, data, extension in definitions:
        path = samples_dir / filename
        path.write_bytes(data)
        records.append({
            "name": filename,
            "fileType": file_type,
            "expectedExtension": extension,
            "size": len(data),
            "sha256": sha256(data),
            "expectedTier": "high",
            "minConfidence": 0.70 if extension == "heic" else 0.85,
        })
    return records


def build_disk_image(image_path: Path, samples: list[dict[str, object]], samples_dir: Path) -> list[dict[str, object]]:
    rng = random.Random(RANDOM_SEED)
    image_path.parent.mkdir(parents=True, exist_ok=True)
    offsets: list[dict[str, object]] = []
    cursor = 0
    with image_path.open("wb") as f:
        for index, sample in enumerate(samples):
            gap = 256 + (index * 37 % 113)
            noise = deterministic_noise(rng, gap)
            f.write(noise)
            cursor += len(noise)

            data = (samples_dir / str(sample["name"])).read_bytes()
            offsets.append({**sample, "offset": cursor, "length": len(data)})
            f.write(data)
            cursor += len(data)

        # Broken signatures that must not be counted as successful recovery.
        broken = [
            ("broken_png_bad_crc", b"\x89PNG\r\n\x1a\n" + b"\x00\x00\x00\rIHDR" + b"\x00" * 13 + b"BAD!"),
            ("broken_pdf_no_xref", b"%PDF-1.7\n1 0 obj<<>>endobj\n"),
            ("broken_zip_no_eocd", b"PK\x03\x04" + b"\x00" * 96),
            ("padding_blob", b"\x00" * 4096),
        ]
        for name, payload in broken:
            gap = deterministic_noise(rng, 211)
            f.write(gap)
            cursor += len(gap)
            offsets.append({
                "name": name,
                "expectedTier": "skip",
                "offset": cursor,
                "length": len(payload),
                "sha256": sha256(payload),
            })
            f.write(payload)
            cursor += len(payload)
    return offsets


def write_expected_manifest(root: Path, image_path: Path, offsets: list[dict[str, object]]) -> None:
    expected = [item for item in offsets if item.get("expectedTier") == "high"]
    invalid = [item for item in offsets if item.get("expectedTier") == "skip"]
    manifest = {
        "version": FIXTURE_VERSION,
        "createdAt": FIXTURE_GENERATED_AT,
        "description": "Synthetic fixture for Polik Recovery quality gates.  Use it to measure recovery correctness, not real-world success guarantees.",
        "image": str(image_path.relative_to(root)),
        "expectedRecoverable": expected,
        "expectedInvalidOrSkipped": invalid,
        "thresholds": {
            "minimumExpectedFoundRatio": 0.75,
            "minimumHighConfidenceUsableRatio": 0.80,
            "maximumFalsePositiveCount": 2,
            "maximumSkippedExpectedCount": 2,
        },
    }
    (root / "expected_manifest.json").write_text(json.dumps(manifest, ensure_ascii=False, indent=2), encoding="utf-8")


def main() -> int:
    parser = argparse.ArgumentParser(description="Generate v0.6 quality-gate fixture image.")
    parser.add_argument("--out", default="fixtures/quality/carver_baseline", help="Output fixture directory")
    args = parser.parse_args()

    root = Path(args.out).resolve()
    samples_dir = root / "samples"
    samples = write_sample_files(samples_dir)
    image_path = root / "disk_image.bin"
    offsets = build_disk_image(image_path, samples, samples_dir)
    write_expected_manifest(root, image_path, offsets)

    print(f"created: {root}")
    print(f"image: {image_path} ({image_path.stat().st_size} bytes)")
    print(f"expected recoverable: {sum(1 for item in offsets if item.get('expectedTier') == 'high')}")
    print(f"expected skipped: {sum(1 for item in offsets if item.get('expectedTier') == 'skip')}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
