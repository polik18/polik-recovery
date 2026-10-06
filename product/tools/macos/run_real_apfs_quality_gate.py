#!/usr/bin/env python3
"""Create and evaluate a real macOS APFS disk-image fixture for Polik Recovery v1.4.

This script must run on macOS. It uses hdiutil/diskutil to create an APFS
image, writes deterministic files, detaches the image, copies the APFS physical
store into a raw slice image, then runs the Polik CLI against that raw APFS
slice.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
import plistlib
import platform
import shutil
import subprocess
import sys
import time
from datetime import datetime, timezone
from pathlib import Path
from typing import Any


class CommandError(RuntimeError):
    pass


def run(cmd: list[str], *, cwd: Path | None = None, capture: bool = False, stdout: Path | None = None) -> str:
    print("$", " ".join(cmd))
    if stdout:
        stdout.parent.mkdir(parents=True, exist_ok=True)
        with stdout.open("w", encoding="utf-8") as f:
            proc = subprocess.run(cmd, cwd=cwd, text=True, stdout=f, stderr=subprocess.PIPE)
        if proc.returncode != 0:
            raise CommandError(f"command failed ({proc.returncode}): {' '.join(cmd)}\n{proc.stderr}")
        return ""
    proc = subprocess.run(cmd, cwd=cwd, text=True, capture_output=capture)
    if proc.returncode != 0:
        err = proc.stderr if capture else ""
        raise CommandError(f"command failed ({proc.returncode}): {' '.join(cmd)}\n{err}")
    return proc.stdout if capture else ""


def require_macos() -> None:
    if platform.system() != "Darwin":
        raise SystemExit("v1.4 real APFS quality gate 必須在 macOS 執行，因為需要 hdiutil / diskutil / APFS。")
    for name in ["hdiutil", "diskutil", "dd"]:
        if not shutil.which(name):
            raise SystemExit(f"找不到必要工具：{name}")


def sha256_bytes(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def sha256_file(path: Path) -> str:
    h = hashlib.sha256()
    with path.open("rb") as f:
        for chunk in iter(lambda: f.read(1024 * 1024), b""):
            h.update(chunk)
    return h.hexdigest()


def parse_attach_root_device(output: str) -> str | None:
    for line in output.splitlines():
        parts = line.split()
        if parts and parts[0].startswith("/dev/disk") and "GUID_partition_scheme" in line:
            return parts[0]
    for line in output.splitlines():
        parts = line.split()
        if parts and parts[0].startswith("/dev/disk"):
            return parts[0]
    return None


def parse_attach_apfs_store(output: str) -> str | None:
    for line in output.splitlines():
        parts = line.split()
        if parts and parts[0].startswith("/dev/disk") and "Apple_APFS" in line:
            return parts[0]
    return None


def diskutil_list_plist(device: str) -> dict[str, Any]:
    raw = subprocess.run(["diskutil", "list", "-plist", device], capture_output=True, check=True).stdout
    return plistlib.loads(raw)


def find_apfs_partition_from_plist(data: dict[str, Any]) -> str | None:
    def visit_partition(part: dict[str, Any]) -> str | None:
        content = str(part.get("Content", ""))
        type_name = str(part.get("Type", ""))
        device_id = part.get("DeviceIdentifier")
        if device_id and ("Apple_APFS" in content or "Apple_APFS" in type_name or "APFS" in content):
            return "/dev/" + str(device_id)
        for child in part.get("Partitions", []) or []:
            found = visit_partition(child)
            if found:
                return found
        return None

    for disk in data.get("AllDisksAndPartitions", []) or []:
        found = visit_partition(disk)
        if found:
            return found
    return None


def raw_device(device: str) -> str:
    if device.startswith("/dev/rdisk"):
        return device
    return device.replace("/dev/disk", "/dev/rdisk", 1)


def detach(device: str | None) -> None:
    if not device:
        return
    try:
        run(["hdiutil", "detach", device], capture=True)
    except Exception as exc:  # noqa: BLE001 - cleanup best effort
        print(f"warning: detach failed for {device}: {exc}", file=sys.stderr)


def deterministic_bytes(seed: int, length: int) -> bytes:
    state = seed & 0xFFFFFFFF
    out = bytearray()
    while len(out) < length:
        state = (1664525 * state + 1013904223) & 0xFFFFFFFF
        out.extend(state.to_bytes(4, "little"))
    return bytes(out[:length])


def write_fixture_files(mount_dir: Path, volume_name: str) -> list[dict[str, Any]]:
    files: list[dict[str, Any]] = []

    def add_file(rel: str, data: bytes, *, deleted: bool = False, required: bool = True, note: str = "") -> None:
        path = mount_dir / rel
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(data)
        os.sync()
        files.append({
            "name": Path(rel).name,
            "path": "/" + rel.replace(os.sep, "/"),
            "volumePathHint": f"/{volume_name}/" + rel.replace(os.sep, "/"),
            "bytes": len(data),
            "sha256": sha256_bytes(data),
            "deleted": deleted,
            "required": required,
            "extract": required and not deleted,
            "expectedQuality": "complete",
            "expectedBytesLost": 0,
            "note": note,
        })
        if deleted:
            path.unlink()
            os.sync()

    add_file(
        "KEEP_COMPLETE.txt",
        ("Polik real APFS allocated text fixture.\n" * 64).encode("utf-8"),
        note="必要測試：小型 allocated APFS 文字檔，應可掃描與完整提取。",
    )
    add_file(
        "Folder/NESTED_RANDOM.bin",
        deterministic_bytes(0xA14F5, 96 * 1024),
        note="必要測試：巢狀目錄中的二進位檔，應可掃描與完整提取。",
    )
    add_file(
        "Unicode_繁中測試.txt",
        "繁體中文檔名與 UTF-8 內容測試。\n".encode("utf-8") * 128,
        note="必要測試：繁中字元檔名，應避免路徑編碼退化。",
    )
    add_file(
        "Deleted/DELETE_ME_OBSERVE.txt",
        ("This deleted APFS record is informational only.\n" * 32).encode("utf-8"),
        deleted=True,
        required=False,
        note="資訊性觀察：真實 APFS deleted record 不穩定，不列入 gate 門檻。",
    )
    return files


def create_real_apfs_fixture(output_root: Path, *, size: str, volume_name: str, keep_image: bool) -> Path:
    fixture_dir = output_root / "fixture"
    if fixture_dir.exists():
        shutil.rmtree(fixture_dir)
    fixture_dir.mkdir(parents=True)
    mount_dir = fixture_dir / "mount"
    mount_dir.mkdir()
    image_path = fixture_dir / "polik_real_apfs.dmg"
    raw_slice = fixture_dir / "polik_real_apfs_slice.img"
    manifest_path = fixture_dir / "expected_real_apfs_manifest.json"

    mounted_root: str | None = None
    readonly_root: str | None = None
    try:
        run(["hdiutil", "create", "-size", size, "-fs", "APFS", "-volname", volume_name, "-ov", str(image_path)], capture=True)
        attach_out = run(["hdiutil", "attach", "-readwrite", "-nobrowse", "-mountpoint", str(mount_dir), str(image_path)], capture=True)
        mounted_root = parse_attach_root_device(attach_out)
        if not mounted_root:
            raise CommandError(f"無法從 hdiutil attach 輸出判斷 root device：\n{attach_out}")

        files = write_fixture_files(mount_dir, volume_name)
        time.sleep(1)
        run(["sync"], capture=True)
    finally:
        detach(mounted_root)

    try:
        attach_ro_out = run(["hdiutil", "attach", "-readonly", "-nomount", str(image_path)], capture=True)
        readonly_root = parse_attach_root_device(attach_ro_out)
        apfs_store = parse_attach_apfs_store(attach_ro_out)
        if not apfs_store and readonly_root:
            apfs_store = find_apfs_partition_from_plist(diskutil_list_plist(readonly_root))
        if not readonly_root or not apfs_store:
            raise CommandError(f"無法判斷 APFS physical store。attach output:\n{attach_ro_out}")

        run(["dd", f"if={raw_device(apfs_store)}", f"of={raw_slice}", "bs=1048576"], capture=True)
    finally:
        detach(readonly_root)

    manifest = {
        "version": "v1.4",
        "kind": "real_macos_apfs_fixture",
        "createdAt": datetime.now(timezone.utc).isoformat(),
        "generator": "tools/macos/run_real_apfs_quality_gate.py",
        "volumeName": volume_name,
        "sourceImage": str(image_path),
        "apfsSliceImage": str(raw_slice),
        "image": str(raw_slice),
        "volumeOffset": 0,
        "sliceSha256": sha256_file(raw_slice),
        "files": files,
        "thresholds": {
            "minimumRequiredScanMatchRatio": 1.0,
            "minimumRequiredExtractHashRatio": 1.0,
            "minimumExtractFieldMatchRatio": 1.0,
            "maximumRequiredScanFailures": 0,
            "maximumRequiredExtractFailures": 0,
        },
        "notes": [
            "這是真實 macOS APFS fixture，和 v1.3 合成 APFS fixture 不同。",
            "deleted APFS record 只做觀察，因 checkpoint / snapshot / unmount 後不保證仍存在。",
            "CLI 讀取 raw APFS slice image，不直接讀 live mounted volume，以維持唯讀測試。",
        ],
    }
    manifest_path.write_text(json.dumps(manifest, ensure_ascii=False, indent=2), encoding="utf-8")
    if not keep_image:
        # Keep the raw slice and manifest; the original dmg is no longer required for the quality gate.
        image_path.unlink(missing_ok=True)
    return manifest_path


def run_cli_quality(cli: Path, manifest_path: Path, output_root: Path, project_root: Path) -> None:
    manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
    image = Path(manifest["apfsSliceImage"])
    scan_dir = output_root / "scan"
    extracts_dir = output_root / "extracts"
    scan_dir.mkdir(parents=True, exist_ok=True)
    extracts_dir.mkdir(parents=True, exist_ok=True)

    run([str(cli), "apfs-info", "--file", str(image), "--offset", str(manifest.get("volumeOffset", 0))], cwd=project_root, stdout=scan_dir / "apfs_info.txt")
    run([str(cli), "apfs-scan-vfs", "--file", str(image), "--offset", str(manifest.get("volumeOffset", 0)), "--format", "json"], cwd=project_root, stdout=scan_dir / "apfs_scan.json")

    scan = json.loads((scan_dir / "apfs_scan.json").read_text(encoding="utf-8"))
    entries = scan.get("entries", [])

    def normalize(value: str) -> str:
        value = value.replace("\\", "/")
        if not value.startswith("/"):
            value = "/" + value
        while "//" in value:
            value = value.replace("//", "/")
        return value.rstrip("/") or "/"

    def matches(actual: str, expected: str) -> bool:
        actual_n = normalize(actual)
        expected_n = normalize(expected)
        return actual_n == expected_n or actual_n.endswith(expected_n)

    for item in manifest.get("files", []):
        if not item.get("extract", False):
            continue
        matched = next((entry for entry in entries if matches(str(entry.get("path", "")), item["path"])), None)
        if not matched:
            # Leave a clear marker for the evaluator instead of hiding the miss.
            (extracts_dir / f"{item['name']}.stdout.txt").write_text("error: scan entry not found\n", encoding="utf-8")
            continue
        inode = matched.get("inode", matched.get("id"))
        out_file = extracts_dir / item["name"]
        run([
            str(cli),
            "apfs-extract",
            "--file",
            str(image),
            "--offset",
            str(manifest.get("volumeOffset", 0)),
            "--inode",
            str(inode),
            "--output",
            str(out_file),
        ], cwd=project_root, stdout=extracts_dir / f"{item['name']}.stdout.txt")


def run_cli_metadata_gate(cli: Path, manifest_path: Path, output_root: Path, project_root: Path) -> Path:
    manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
    image = Path(manifest["apfsSliceImage"])
    scan_dir = output_root / "scan"
    scan_dir.mkdir(parents=True, exist_ok=True)
    apfs_info_path = scan_dir / "apfs_info.txt"
    run([str(cli), "apfs-info", "--file", str(image), "--offset", str(manifest.get("volumeOffset", 0))], cwd=project_root, stdout=apfs_info_path)
    apfs_info = apfs_info_path.read_text(encoding="utf-8", errors="replace")
    required_markers = [
        "Container Block Size:",
        "Container Block Count:",
        "Container UUID:",
        "Max File Systems:",
    ]
    missing = [marker for marker in required_markers if marker not in apfs_info]
    report = {
        "version": "v1.4",
        "kind": "real_macos_apfs_metadata_gate",
        "evaluatedAt": datetime.now(timezone.utc).isoformat(),
        "passed": not missing,
        "fixture": {
            "image": manifest.get("image"),
            "apfsSliceImage": manifest.get("apfsSliceImage"),
            "volumeName": manifest.get("volumeName"),
            "volumeOffset": manifest.get("volumeOffset", 0),
            "sliceSha256": manifest.get("sliceSha256"),
        },
        "checks": [
            {
                "name": "apfs_info_marker",
                "marker": marker,
                "passed": marker not in missing,
            }
            for marker in required_markers
        ],
        "scope": "metadata_only",
        "note": "This gate verifies real APFS image creation and container superblock checksum parsing. Full VFS scan/extract remains covered by real-apfs-quality-gate.",
    }
    report_json = output_root / "real_apfs_metadata_report.json"
    report_json.write_text(json.dumps(report, ensure_ascii=False, indent=2), encoding="utf-8")
    if missing:
        raise CommandError(f"metadata gate failed; missing markers: {', '.join(missing)}")
    return report_json


def main() -> int:
    parser = argparse.ArgumentParser(description="Run v1.4 real macOS APFS quality gate.")
    parser.add_argument("--cli", default="target/release/polik-recovery", help="Built CLI path")
    parser.add_argument("--output-root", default="target/real-apfs-quality")
    parser.add_argument("--size", default="256m", help="APFS disk image size passed to hdiutil create")
    parser.add_argument("--volume-name", default="PolikAPFSReal")
    parser.add_argument("--keep-image", action="store_true", help="Keep original .dmg in addition to raw APFS slice")
    parser.add_argument("--metadata-only", action="store_true", help="Only verify real APFS image creation and apfs-info metadata parsing.")
    args = parser.parse_args()

    require_macos()
    project_root = Path(__file__).resolve().parents[2]
    output_root = project_root / args.output_root
    cli = (project_root / args.cli).resolve()
    if not cli.exists():
        raise SystemExit(f"找不到 CLI：{cli}\n請先執行 cargo build --release -p polik-recovery-cli。")
    if output_root.exists():
        shutil.rmtree(output_root)
    output_root.mkdir(parents=True)

    manifest_path = create_real_apfs_fixture(output_root, size=args.size, volume_name=args.volume_name, keep_image=args.keep_image)
    if args.metadata_only:
        report_json = run_cli_metadata_gate(cli, manifest_path, output_root, project_root)
        print(f"真實 APFS metadata 報告：{report_json}")
        return 0
    run_cli_quality(cli, manifest_path, output_root, project_root)
    report_json = output_root / "real_apfs_quality_report.json"
    report_html = output_root / "real_apfs_quality_report.html"
    run([
        sys.executable,
        str(project_root / "tools/quality/evaluate_real_apfs_recovery.py"),
        "--expected",
        str(manifest_path),
        "--output-root",
        str(output_root),
        "--out",
        str(report_json),
        "--html",
        str(report_html),
    ], cwd=project_root)
    print(f"真實 APFS 品質報告：{report_html}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
