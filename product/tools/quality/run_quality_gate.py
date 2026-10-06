#!/usr/bin/env python3
"""Run the full v0.6 quality gate around a built polik-recovery CLI."""
from __future__ import annotations

import argparse
import shutil
import subprocess
import sys
from pathlib import Path


def run(cmd: list[str], cwd: Path | None = None) -> None:
    print("$", " ".join(cmd))
    subprocess.run(cmd, cwd=cwd, check=True)


def main() -> int:
    parser = argparse.ArgumentParser(description="Generate fixture, run CLI carving, evaluate quality report.")
    parser.add_argument("--cli", default="target/debug/polik-recovery", help="Debug CLI path; fixture isolation is unavailable in release builds")
    parser.add_argument("--fixture-dir", default="fixtures/quality/carver_baseline")
    parser.add_argument("--output-dir", default="target/quality-gate/carver_output")
    parser.add_argument("--report-dir", default="target/quality-gate")
    args = parser.parse_args()

    project_root = Path(__file__).resolve().parents[2]
    cli = (project_root / args.cli).resolve() if not Path(args.cli).is_absolute() else Path(args.cli)
    fixture_dir = project_root / args.fixture_dir
    output_dir = project_root / args.output_dir
    report_dir = project_root / args.report_dir
    image = fixture_dir / "disk_image.bin"

    run([sys.executable, str(project_root / "tools/quality/generate_fixture.py"), "--out", str(fixture_dir)])
    shutil.rmtree(output_dir, ignore_errors=True)
    output_dir.mkdir(parents=True, exist_ok=True)
    report_dir.mkdir(parents=True, exist_ok=True)
    if not cli.exists():
        raise SystemExit(f"找不到 debug CLI：{cli}\n請先執行 cargo build -p polik-recovery-cli。")

    run([str(cli), "carve", "--file", str(image), "--output-dir", str(output_dir), "--format", "json", "--fixture-storage-isolation"])
    manifest = output_dir / "carve_manifest.json"
    run([
        sys.executable,
        str(project_root / "tools/quality/evaluate_recovery.py"),
        "--expected", str(fixture_dir / "expected_manifest.json"),
        "--manifest", str(manifest),
        "--out", str(report_dir / "quality_report.json"),
        "--html", str(report_dir / "quality_report.html"),
    ])
    print(f"品質報告：{report_dir / 'quality_report.html'}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
