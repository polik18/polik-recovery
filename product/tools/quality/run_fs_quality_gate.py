#!/usr/bin/env python3
"""Run v0.7 filesystem-level quality gate around a built polik-recovery CLI."""
from __future__ import annotations

import argparse
import json
import shutil
import subprocess
import sys
from pathlib import Path


def run(cmd: list[str], *, cwd: Path, stdout: Path | None = None) -> None:
    print("$", " ".join(cmd))
    if stdout:
        stdout.parent.mkdir(parents=True, exist_ok=True)
        with stdout.open("w", encoding="utf-8") as f:
            subprocess.run(cmd, cwd=cwd, stdout=f, stderr=subprocess.PIPE, text=True, check=True)
    else:
        subprocess.run(cmd, cwd=cwd, check=True)


def main() -> int:
    parser = argparse.ArgumentParser(description="Generate FS fixtures, run scans/extracts, evaluate v0.7 quality gate.")
    parser.add_argument("--cli", default="target/debug/polik-recovery", help="Debug CLI path; fixture isolation is unavailable in release builds")
    parser.add_argument("--fixture-dir", default="fixtures/fs_quality")
    parser.add_argument("--output-root", default="target/fs-quality")
    args = parser.parse_args()

    project_root = Path(__file__).resolve().parents[2]
    cli = (project_root / args.cli).resolve()
    output_root = project_root / args.output_root
    fixture_dir = project_root / args.fixture_dir

    run([sys.executable, str(project_root / "tools/quality/generate_fs_fixtures.py"), "--out", str(fixture_dir)], cwd=project_root)
    if not cli.exists():
        raise SystemExit(f"找不到 debug CLI：{cli}\n請先執行 cargo build -p polik-recovery-cli。")

    if output_root.exists():
        shutil.rmtree(output_root)
    (output_root / "scans").mkdir(parents=True, exist_ok=True)
    (output_root / "extracts").mkdir(parents=True, exist_ok=True)

    manifest = json.loads((fixture_dir / "expected_fs_manifest.json").read_text(encoding="utf-8"))
    for fixture in manifest["fixtures"]:
        fs = fixture["fs"]
        scan_cmd = [str(cli)] + fixture["scanCommand"]
        run(scan_cmd, cwd=project_root, stdout=output_root / "scans" / f"{fs}_scan.json")

        for item in fixture.get("extracts", []):
            extract_dir = output_root / "extracts" / fs
            extract_dir.mkdir(parents=True, exist_ok=True)
            output_path = extract_dir / item["name"]
            image = item.get("image", fixture.get("image"))
            if fs == "ntfs":
                cmd = [str(cli), "ntfs-extract", "--file", str(image), "--record", str(item["record"]), "--output", str(output_path)]
            elif fs == "fat32":
                cmd = [str(cli), "fat32-extract", "--file", str(fixture["image"]), "--entry-id", str(item["entryId"]), "--output", str(output_path)]
            elif fs == "exfat":
                cmd = [str(cli), "exfat-extract", "--file", str(fixture["image"]), "--entry-id", str(item["entryId"]), "--output", str(output_path)]
            else:
                raise SystemExit(f"unsupported fs fixture: {fs}")
            cmd.append("--fixture-storage-isolation")
            run(cmd, cwd=project_root, stdout=output_root / "extracts" / fs / f"{item['name']}.stdout.txt")

    run([
        sys.executable,
        str(project_root / "tools/quality/evaluate_fs_recovery.py"),
        "--expected",
        str(fixture_dir / "expected_fs_manifest.json"),
        "--output-root",
        str(output_root),
        "--out",
        str(output_root / "fs_quality_report.json"),
        "--html",
        str(output_root / "fs_quality_report.html"),
    ], cwd=project_root)
    print(f"檔案系統品質報告：{output_root / 'fs_quality_report.html'}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
