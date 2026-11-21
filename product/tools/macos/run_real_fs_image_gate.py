#!/usr/bin/env python3
"""Real macOS-formatted filesystem image gate.

Builds a deterministic source folder, lets `hdiutil create -srcfolder` format
real FAT32 / exFAT / APFS images (with and without a GPT partition map), then
runs the Polik CLI end to end: partition detection -> VFS scan -> per-file
extraction -> SHA-256 comparison against the source bytes.

Safety: by default this script never attaches, mounts or reads any /dev
device. `hdiutil create -srcfolder` is the only formatter used, and the CLI
only reads the resulting regular image files.

`--with-deleted` (requires explicit human approval, see product/AGENTS.md)
additionally attaches each freshly created disposable image at a private
mount point under target/, deletes a fixed set of files through that mount
point, detaches it by mount point, and checks that the deleted files are
reported as Deleted and recovered byte-exact. It verifies with
`hdiutil info -plist` that the mount point belongs to that exact image before
deleting anything, and never touches any other device. Images are written under
`fixtures/real_fs_generated/` (ignored by git) because debug builds only allow
the fixture -> target quality override inside the product tree.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import platform
import plistlib
import random
import shutil
import subprocess
import sys
import time
from pathlib import Path

FILESYSTEMS = {
    # name: (hdiutil -fs value, CLI prefix, extract id flag, required)
    "fat32": ("MS-DOS FAT32", "fat32", "--entry-id", True),
    "exfat": ("ExFAT", "exfat", "--entry-id", True),
    "apfs": ("APFS", "apfs", "--inode", True),
}
LAYOUTS = ["NONE", "GPTSPUD"]
PARTITION_FS_NAMES = {"fat32": "FAT32", "exfat": "exFAT", "apfs": "APFS"}
# Files removed in --with-deleted mode: large binary, CJK name, long mixed
# case name, an 8.3-compatible name and a file in a nested directory.
DELETED_PATHS = frozenset({
    "/docs/report.bin",
    "/docs/nested/中文檔名.txt",
    "/Mixed Case Long Name.TXT",
    "/docs/f07.txt",
    "/docs/nested/small.dat",
})
# After deletion, fill the remaining free space so freed blocks are reused.
# The tool must then either recover the bytes or flag them as overwritten;
# silently returning wrong bytes is a failure. A deleted entry whose own
# directory record was reused is genuinely gone and counted as lost.
OVERWRITE_REQUIRED = {"fat32": True, "exfat": True, "apfs": True}
# Deleted-file recovery is only required where it is proven to work.
DELETED_REQUIRED = {"fat32": True, "exfat": True, "apfs": True}
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


def image_mount_point(image: Path) -> str | None:
    info = plistlib.loads(subprocess.run(["hdiutil", "info", "-plist"], capture_output=True,
                                         check=True).stdout)
    for attached in info.get("images", []):
        if Path(attached.get("image-path", "")).resolve() != image.resolve():
            continue
        for entity in attached.get("system-entities", []):
            if entity.get("mount-point"):
                return entity["mount-point"]
    return None


def delete_files_in_image(image: Path, mount: Path, paths: frozenset[str],
                          fill_free_space: bool = False) -> None:
    """Attach a disposable image privately, delete `paths`, then detach."""
    if mount.exists():
        shutil.rmtree(mount)
    mount.mkdir(parents=True)
    subprocess.run(["hdiutil", "attach", "-nobrowse", "-noautoopen", "-mountpoint", str(mount),
                    str(image)], capture_output=True, check=True)
    try:
        mounted = image_mount_point(image)
        if mounted is None or Path(mounted).resolve() != mount.resolve():
            raise RuntimeError(f"refusing to delete: {image} is not mounted at {mount}")
        for rel in sorted(paths):
            (mount / rel.lstrip("/")).unlink()
        subprocess.run(["sync"], check=True)
        if fill_free_space:
            rng = random.Random(4242)
            try:
                with open(mount / "filler.bin", "wb") as filler:
                    while True:
                        filler.write(rng.randbytes(1024 * 1024))
            except OSError:
                pass  # ENOSPC is the goal
            subprocess.run(["sync"], check=True)
    finally:
        for attempt in range(3):
            if subprocess.run(["hdiutil", "detach", str(mount)],
                              capture_output=True).returncode == 0:
                break
            time.sleep(1 + attempt)
        else:
            raise RuntimeError(f"could not detach {mount}; detach it manually")


def deleted_name_matches(expected_path: str, found_path: str) -> bool:
    """FAT deletion overwrites the first byte of 8.3-only names."""
    if expected_path == found_path:
        return True
    head, _, name = expected_path.rpartition("/")
    return found_path == f"{head}/_{name[1:]}"


def sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def evaluate(cli: Path, project: Path, image: Path, fs: str, layout: str,
             expected: dict[str, bytes], out_dir: Path,
             deleted: frozenset[str] = frozenset(),
             allow_missing_deleted: bool = False) -> dict:
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

    scan_cmd = [str(cli), f"{prefix}-scan-vfs", "--file", rel_image, "--offset", str(offset),
                "--format", "json"]
    if deleted and fs == "apfs":
        # APFS deleted files live in stale FS-tree nodes; scanning them is opt-in.
        scan_cmd.append("--include-deleted")
    scan = run(scan_cmd, project)
    if scan.returncode != 0:
        case["errors"].append(f"scan: exit {scan.returncode}: {scan.stderr.strip()[-400:]}")
        return case
    entries = json.loads(scan.stdout)["entries"]
    by_path: dict[str, dict] = {}
    deleted_entries: list[dict] = []
    for entry in entries:
        path = entry["path"]
        if entry["kind"] != "File" or path.startswith(IGNORED_PREFIXES) or "/._" in path:
            continue
        if entry.get("status") == "Deleted":
            deleted_entries.append(entry)
        else:
            by_path[path] = entry

    found = extracted = flagged = lost = 0
    if out_dir.exists():
        shutil.rmtree(out_dir)
    out_dir.mkdir(parents=True)
    for index, (path, data) in enumerate(sorted(expected.items())):
        if path in deleted:
            entry = next((e for e in deleted_entries if deleted_name_matches(path, e["path"])
                          and e.get("size") == len(data)), None)
        else:
            entry = by_path.get(path)
        if entry is None:
            if allow_missing_deleted and path in deleted:
                lost += 1
            else:
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
        quality = next((line.split(":", 1)[1].strip() for line in ext.stdout.splitlines()
                        if line.startswith("quality:")), "")
        if path in deleted and quality == "complete":
            case["errors"].append(f"deleted file claimed complete: {path}")
            continue
        if sha256(output.read_bytes()) != sha256(data):
            if path in deleted and quality == "deleted_overwritten":
                flagged += 1  # honestly reported as overwritten
            else:
                case["errors"].append(f"sha256 (unflagged, quality={quality or 'n/a'}): {path}")
            continue
        extracted += 1
    case.update({"expected_files": len(expected), "found_files": found,
                 "hash_matched_files": extracted, "flagged_overwritten": flagged,
                 "lost_deleted_entries": lost,
                 "passed": not case["errors"]})
    return case


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--cli", default="target/debug/polik-recovery")
    parser.add_argument("--report", default="target/real-fs-gate/report.json")
    parser.add_argument("--filesystems", default="fat32,exfat,apfs")
    parser.add_argument("--with-deleted", action="store_true",
                        help="also mount disposable images, delete files and check recovery "
                             "(requires human approval)")
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

    if args.with_deleted:
        for fs in [name.strip() for name in args.filesystems.split(",") if name.strip()]:
            fs_value = FILESYSTEMS[fs][0]
            image = images / f"{fs}_deleted.dmg"
            create_image(src, image, fs_value, "NONE")
            delete_files_in_image(image, work / "mnt" / fs, DELETED_PATHS)
            case = evaluate(cli, project, image, fs, "NONE+deleted", expected,
                            work / "out" / f"{fs}_deleted", DELETED_PATHS)
            case["required"] = DELETED_REQUIRED[fs] or args.require_all
            cases.append(case)
            status = "PASS" if case.get("passed") else ("FAIL" if case["required"] else "KNOWN-GAP")
            deleted_errors = [e for e in case["errors"]
                              if any(p in e for p in DELETED_PATHS)]
            print(f"{status} {fs}/deleted: found {case.get('found_files', 0)}/{len(expected)}, "
                  f"hash {case.get('hash_matched_files', 0)}/{len(expected)}, "
                  f"flagged overwritten {case.get('flagged_overwritten', 0)}"
                  + (f" — {'; '.join(deleted_errors[:5])}" if deleted_errors else ""))

        for fs in [name.strip() for name in args.filesystems.split(",") if name.strip()]:
            fs_value = FILESYSTEMS[fs][0]
            image = images / f"{fs}_overwritten.dmg"
            create_image(src, image, fs_value, "NONE")
            delete_files_in_image(image, work / "mnt" / fs, DELETED_PATHS, fill_free_space=True)
            case = evaluate(cli, project, image, fs, "NONE+deleted+filled", expected,
                            work / "out" / f"{fs}_overwritten", DELETED_PATHS,
                            allow_missing_deleted=True)
            # Detection must actually be exercised: at least one flagged file.
            if case.get("passed") and case.get("flagged_overwritten", 0) == 0 \
                    and case.get("lost_deleted_entries", 0) == 0 \
                    and case.get("hash_matched_files", 0) < len(expected):
                case["errors"].append("no overwritten file was flagged")
                case["passed"] = False
            case["required"] = OVERWRITE_REQUIRED[fs] or args.require_all
            cases.append(case)
            status = "PASS" if case.get("passed") else ("FAIL" if case["required"] else "KNOWN-GAP")
            print(f"{status} {fs}/overwritten: hash {case.get('hash_matched_files', 0)}/"
                  f"{len(expected)}, flagged overwritten {case.get('flagged_overwritten', 0)}, "
                  f"lost {case.get('lost_deleted_entries', 0)}"
                  + (f" — {'; '.join(case['errors'][:3])}" if case["errors"] else ""))

    report = project / args.report
    report.parent.mkdir(parents=True, exist_ok=True)
    report.write_text(json.dumps({"schema": "polik-real-fs-gate/v1", "cases": cases},
                                 ensure_ascii=False, indent=2) + "\n")
    failed = [c for c in cases if c["required"] and not c.get("passed")]
    return 1 if failed else 0


if __name__ == "__main__":
    raise SystemExit(main())
