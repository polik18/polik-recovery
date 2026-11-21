#!/usr/bin/env python3
"""Real macOS-formatted filesystem image gate.

Builds a deterministic source folder, lets `hdiutil create -srcfolder` format
real FAT32 / exFAT / APFS images (with and without a GPT partition map), then
runs the Polik CLI end to end: partition detection -> VFS scan -> per-file
extraction -> SHA-256 comparison against the source bytes.

Safety: this script never attaches, mounts or reads any /dev device itself.
`hdiutil create -srcfolder` is the only formatter used, and the CLI only reads
the resulting regular image files. Images are written under
`fixtures/real_fs_generated/` (ignored by git) because debug builds only allow
the fixture -> target quality override inside the product tree.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import platform
import random
import shutil
import subprocess
import sys
from pathlib import Path

FILESYSTEMS = {
    # name: (hdiutil -fs value, CLI prefix, extract id flag, required)
    "fat32": ("MS-DOS FAT32", "fat32", "--entry-id", True),
    "exfat": ("ExFAT", "exfat", "--entry-id", True),
    "apfs": ("APFS", "apfs", "--inode", True),
}
LAYOUTS = ["NONE", "GPTSPUD"]
PARTITION_FS_NAMES = {"fat32": "FAT32", "exfat": "exFAT", "apfs": "APFS"}
IGNORED_PREFIXES = ("/.fseventsd", "/.Spotlight-V100", "/.Trashes", "/.TemporaryItems")


def build_source_tree(root: Path) -> dict[str, bytes]:
    if root.exists():
        shutil.rmtree(root)
    rng = random.Random(20261121)
    files: dict[str, bytes] = {
        "/readme.txt": b"Polik real image test\n" * 200,
        "/empty.txt": b"",
        "/docs/report.bin": rng.randbytes(5 * 1024 * 1024),
        "/docs/nested/small.dat": rng.randbytes(3000),
        "/docs/nested/中文檔名.txt": "unicode name\n".encode() * 50,
        "/Mixed Case Long Name.TXT": b"long file name entry\n" * 33,
    }
    for index in range(40):
        files[f"/docs/f{index:02d}.txt"] = f"file {index}\n".encode() * (index * 37 + 1)
    for rel, data in files.items():
        path = root / rel.lstrip("/")
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(data)
    return files


def run(cmd: list[str], cwd: Path) -> subprocess.CompletedProcess[str]:
    return subprocess.run(cmd, cwd=cwd, capture_output=True, text=True)


def create_image(src: Path, image: Path, fs_value: str, layout: str) -> None:
    image.parent.mkdir(parents=True, exist_ok=True)
    cmd = [
        "hdiutil", "create", "-quiet", "-srcfolder", str(src), "-fs", fs_value,
        "-volname", "POLIK", "-format", "UDRW", "-layout", layout, "-size", "64m",
        "-ov", str(image),
    ]
    result = subprocess.run(cmd, capture_output=True, text=True)
    if result.returncode != 0:
        raise RuntimeError(f"hdiutil create failed: {result.stderr.strip()}")


def sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def evaluate(cli: Path, project: Path, image: Path, fs: str, layout: str,
             expected: dict[str, bytes], out_dir: Path) -> dict:
    _, prefix, id_flag, _ = FILESYSTEMS[fs]
    case = {"filesystem": fs, "layout": layout, "image": str(image.relative_to(project)),
            "errors": []}
    rel_image = str(image.relative_to(project))

    part = run([str(cli), "partitions", "--file", rel_image, "--format", "json"], project)
    if part.returncode != 0:
        case["errors"].append(f"partitions: {part.stderr.strip()}")
        return case
    table = json.loads(part.stdout)
    case["partition_table"] = table["table"]
    match = [p for p in table["partitions"] if p.get("filesystem") == PARTITION_FS_NAMES[fs]]
    if not match:
        case["errors"].append(f"partitions: no {PARTITION_FS_NAMES[fs]} partition in {table}")
        return case
    offset = match[0]["offset"]
    case["volume_offset"] = offset

    scan = run([str(cli), f"{prefix}-scan-vfs", "--file", rel_image, "--offset", str(offset),
                "--format", "json"], project)
    if scan.returncode != 0:
        case["errors"].append(f"scan: exit {scan.returncode}: {scan.stderr.strip()[-400:]}")
        return case
    entries = json.loads(scan.stdout)["entries"]
    by_path: dict[str, dict] = {}
    for entry in entries:
        path = entry["path"]
        if entry["kind"] != "File" or path.startswith(IGNORED_PREFIXES) or "/._" in path:
            continue
        by_path[path] = entry

    found = extracted = 0
    if out_dir.exists():
        shutil.rmtree(out_dir)
    out_dir.mkdir(parents=True)
    for index, (path, data) in enumerate(sorted(expected.items())):
        entry = by_path.get(path)
        if entry is None:
            case["errors"].append(f"missing: {path}")
            continue
        if entry.get("size") != len(data):
            case["errors"].append(f"size: {path} expected {len(data)} got {entry.get('size')}")
            continue
        found += 1
        entry_id = entry["inode"] if id_flag == "--inode" else entry["id"]
        output = out_dir / f"{index:03d}.bin"
        ext = run([str(cli), f"{prefix}-extract", "--file", rel_image, id_flag, str(entry_id),
                   "--output", str(output.relative_to(project)), "--offset", str(offset),
                   "--fixture-storage-isolation"], project)
        if ext.returncode != 0 or not output.exists():
            case["errors"].append(f"extract: {path}: {ext.stderr.strip()[-300:]}")
            continue
        if sha256(output.read_bytes()) != sha256(data):
            case["errors"].append(f"sha256: {path}")
            continue
        extracted += 1
    case.update({"expected_files": len(expected), "found_files": found,
                 "hash_matched_files": extracted, "passed": not case["errors"]})
    return case


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--cli", default="target/debug/polik-recovery")
    parser.add_argument("--report", default="target/real-fs-gate/report.json")
    parser.add_argument("--filesystems", default="fat32,exfat,apfs")
    parser.add_argument("--require-all", action="store_true",
                        help="treat every filesystem as required, including future known gaps")
    args = parser.parse_args()

    if platform.system() != "Darwin" or not shutil.which("hdiutil"):
        print("real filesystem image gate requires macOS hdiutil; skipped")
        return 0

    project = Path(__file__).resolve().parents[2]
    cli = (project / args.cli).resolve()
    if not cli.exists():
        print(f"missing CLI {cli}; run cargo build --locked -p polik-recovery-cli", file=sys.stderr)
        return 2
    work = project / "target" / "real-fs-gate"
    src = work / "src"
    expected = build_source_tree(src)
    images = project / "fixtures" / "real_fs_generated"

    cases = []
    for fs in [name.strip() for name in args.filesystems.split(",") if name.strip()]:
        fs_value, _, _, required = FILESYSTEMS[fs]
        for layout in LAYOUTS:
            image = images / f"{fs}_{layout}.dmg"  # hdiutil always appends .dmg
            create_image(src, image, fs_value, layout)
            case = evaluate(cli, project, image, fs, layout, expected,
                            work / "out" / f"{fs}_{layout}")
            case["required"] = required or args.require_all
            cases.append(case)
            status = "PASS" if case.get("passed") else ("FAIL" if case["required"] else "KNOWN-GAP")
            print(f"{status} {fs}/{layout}: found {case.get('found_files', 0)}/"
                  f"{len(expected)}, hash {case.get('hash_matched_files', 0)}/{len(expected)}"
                  + (f" — {case['errors'][0]}" if case["errors"] else ""))

    report = project / args.report
    report.parent.mkdir(parents=True, exist_ok=True)
    report.write_text(json.dumps({"schema": "polik-real-fs-gate/v1", "cases": cases},
                                 ensure_ascii=False, indent=2) + "\n")
    failed = [c for c in cases if c["required"] and not c.get("passed")]
    return 1 if failed else 0


if __name__ == "__main__":
    raise SystemExit(main())
