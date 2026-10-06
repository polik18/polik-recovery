#!/usr/bin/env python3
"""Run the v1.3 APFS quality gate around a built polik-recovery CLI."""
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
    parser = argparse.ArgumentParser(description="Generate APFS fixtures, run scans/extracts, evaluate v1.3 APFS quality gate.")
    parser.add_argument("--cli", default="target/debug/polik-recovery", help="Debug CLI path; fixture isolation is unavailable in release builds")
    parser.add_argument("--fixture-dir", default="fixtures/apfs_quality")
    parser.add_argument("--output-root", default="target/apfs-quality")
    args = parser.parse_args()

    project_root = Path(__file__).resolve().parents[2]
    cli = (project_root / args.cli).resolve()
    output_root = project_root / args.output_root
    fixture_dir = project_root / args.fixture_dir

    run([sys.executable, str(project_root / "tools/quality/generate_apfs_fixtures.py"), "--out", str(fixture_dir)], cwd=project_root)
    if not cli.exists():
        raise SystemExit(f"找不到 debug CLI：{cli}\n請先執行 cargo build -p polik-recovery-cli。")

    if output_root.exists():
        shutil.rmtree(output_root)
    (output_root / "scans").mkdir(parents=True, exist_ok=True)
    (output_root / "extracts").mkdir(parents=True, exist_ok=True)

    manifest = json.loads((fixture_dir / "expected_apfs_manifest.json").read_text(encoding="utf-8"))
    for fixture_index, fixture in enumerate(manifest["fixtures"]):
        scan_cmd = [str(cli)] + fixture["scanCommand"]
        run(scan_cmd, cwd=project_root, stdout=output_root / "scans" / f"apfs_{fixture_index}_scan.json")

        extract_dir = output_root / "extracts" / f"apfs_{fixture_index}"
        extract_dir.mkdir(parents=True, exist_ok=True)
        for item in fixture.get("extracts", []):
            output_path = extract_dir / item["name"]
            cmd = [
                str(cli),
                "apfs-extract",
                "--file",
                str(fixture["image"]),
                "--inode",
                str(item["inode"]),
                "--output",
                str(output_path),
                "--fixture-storage-isolation",
            ]
            run(cmd, cwd=project_root, stdout=extract_dir / f"{item['name']}.stdout.txt")

    run([
        sys.executable,
        str(project_root / "tools/quality/evaluate_apfs_recovery.py"),
        "--expected",
        str(fixture_dir / "expected_apfs_manifest.json"),
        "--output-root",
        str(output_root),
        "--out",
        str(output_root / "apfs_quality_report.json"),
        "--html",
        str(output_root / "apfs_quality_report.html"),
    ], cwd=project_root)
    print(f"APFS 品質報告：{output_root / 'apfs_quality_report.html'}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
