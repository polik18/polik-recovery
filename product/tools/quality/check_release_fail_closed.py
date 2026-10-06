#!/usr/bin/env python3
"""Verify that the release CLI exposes no fixture or raw-device write bypass."""

from __future__ import annotations

import argparse
import platform
import subprocess
import tempfile
from pathlib import Path


def run_rejected(command: list[str], expected: str) -> subprocess.CompletedProcess[str]:
    result = subprocess.run(command, text=True, capture_output=True, check=False)
    if result.returncode == 0:
        raise SystemExit(f"command unexpectedly succeeded: {' '.join(command)}")
    combined = result.stdout + result.stderr
    if expected not in combined:
        raise SystemExit(
            f"command did not report {expected!r}: {' '.join(command)}\n{combined}"
        )
    return result


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cli", default="target/release/polik-recovery")
    args = parser.parse_args()

    root = Path(__file__).resolve().parents[2]
    cli = (root / args.cli).resolve()
    fixture = root / "fixtures/images/ntfs_volume_extract.bin"
    if not cli.is_file():
        raise SystemExit(f"release CLI not found: {cli}")
    if not fixture.is_file():
        raise SystemExit(f"fixture not found: {fixture}")

    # Dispatch must reject before parsing or opening any device path.
    run_rejected([str(cli), "raw-hash"], "RAW_DEVICE_UNSUPPORTED")

    with tempfile.TemporaryDirectory(prefix="release-safety-", dir=root / "target") as tmp:
        output = Path(tmp) / "recovered.bin"
        base = [
            str(cli),
            "ntfs-extract",
            "--file",
            str(fixture),
            "--record",
            "0",
            "--output",
            str(output),
        ]
        expected = (
            "share physical storage"
            if platform.system() == "Darwin"
            else "not implemented on this platform"
        )
        run_rejected(base, expected)
        run_rejected(
            [*base, "--fixture-storage-isolation"],
            "unavailable in release builds",
        )
        for path in [
            output,
            output.with_name(output.name + ".partial"),
            output.with_name(output.name + ".manifest.txt"),
            output.with_name(output.name + ".manifest.txt.partial"),
        ]:
            if path.exists():
                raise SystemExit(f"rejected release command created output: {path}")

    print("Release CLI raw-device, same-store and fixture-bypass checks passed.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
