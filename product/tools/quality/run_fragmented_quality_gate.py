#!/usr/bin/env python3
"""Run v0.8 fragmentation/partial-recovery quality gate around a built CLI."""
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
    parser = argparse.ArgumentParser(description="Generate fragmented fixtures, run scans/extracts, evaluate v0.8 quality gate.")
    parser.add_argument("--cli", default="target/debug/polik-recovery", help="Debug CLI path; fixture isolation is unavailable in release builds")
    parser.add_argument("--fixture-dir", default="fixtures/fragmented_quality")
    parser.add_argument("--output-root", default="target/fragmented-quality")
    args = parser.parse_args()

    project_root = Path(__file__).resolve().parents[2]
    cli = (project_root / args.cli).resolve()
    output_root = project_root / args.output_root
    fixture_dir = project_root / args.fixture_dir

    run([sys.executable, str(project_root / "tools/quality/generate_fragmented_fixtures.py"), "--out", str(fixture_dir)], cwd=project_root)
    if not cli.exists():
        raise SystemExit(f"找不到 debug CLI：{cli}\n請先執行 cargo build -p polik-recovery-cli。")

    if output_root.exists():
        shutil.rmtree(output_root)
    (output_root / "scans").mkdir(parents=True, exist_ok=True)
    (output_root / "extracts").mkdir(parents=True, exist_ok=True)

    manifest = json.loads((fixture_dir / "expected_fragmented_manifest.json").read_text(encoding="utf-8"))
    for fixture in manifest["fixtures"]:
        fixture_id = fixture["fs"]
        driver = fixture.get("driver", fixture_id)
        run([str(cli)] + fixture["scanCommand"], cwd=project_root, stdout=output_root / "scans" / f"{fixture_id}_scan.json")

        for item in fixture.get("extracts", []):
            extract_dir = output_root / "extracts" / fixture_id
            extract_dir.mkdir(parents=True, exist_ok=True)
            output_path = extract_dir / item["name"]
            if driver == "fat32":
                cmd = [str(cli), "fat32-extract", "--file", str(fixture["image"]), "--entry-id", str(item["entryId"]), "--output", str(output_path)]
            elif driver == "exfat":
                cmd = [str(cli), "exfat-extract", "--file", str(fixture["image"]), "--entry-id", str(item["entryId"]), "--output", str(output_path)]
            else:
                raise SystemExit(f"unsupported fragmented fixture driver: {driver}")
            cmd.append("--fixture-storage-isolation")
            run(cmd, cwd=project_root, stdout=output_root / "extracts" / fixture_id / f"{item['name']}.stdout.txt")

    run([
        sys.executable,
        str(project_root / "tools/quality/evaluate_fragmented_recovery.py"),
        "--expected",
        str(fixture_dir / "expected_fragmented_manifest.json"),
        "--output-root",
        str(output_root),
        "--out",
        str(output_root / "fragmented_quality_report.json"),
        "--html",
        str(output_root / "fragmented_quality_report.html"),
    ], cwd=project_root)
    print(f"碎片化與 partial recovery 品質報告：{output_root / 'fragmented_quality_report.html'}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
