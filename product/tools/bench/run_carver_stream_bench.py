#!/usr/bin/env python3
"""Bounded-memory cancellation check on a 32 GiB sparse synthetic image."""

from __future__ import annotations

import argparse
import json
import os
import re
import subprocess
from pathlib import Path


GIB = 1024 * 1024 * 1024


def product_root() -> Path:
    return Path(__file__).resolve().parents[2]


def peak_rss(stderr: str) -> int | None:
    match = re.search(r"maximum resident set size:\s*(\d+)", stderr)
    if match is None:
        match = re.search(r"(\d+)\s+maximum resident set size", stderr)
    return int(match.group(1)) if match else None


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--image-size-gib", type=int, default=32)
    parser.add_argument("--cancel-after-mib", type=int, default=64)
    parser.add_argument("--keep-image", action="store_true")
    args = parser.parse_args()

    root = product_root()
    bench_root = root / "target" / "bench" / "carver-stream"
    bench_root.mkdir(parents=True, exist_ok=True)
    image = bench_root / f"sparse-{args.image_size_gib}g.img"
    target = root / "target" / "bench" / "ntfs-mft-cargo-target"
    env = os.environ.copy()
    toolchain = Path.home() / ".rustup" / "toolchains" / "1.85.1-aarch64-apple-darwin" / "bin"
    env["PATH"] = f"{toolchain}:{env.get('PATH', '')}"
    subprocess.run(
        [
            "cargo",
            "build",
            "--manifest-path",
            "tools/bench/Cargo.toml",
            "--bin",
            "carver-stream-bench",
            "--target-dir",
            str(target),
            "--release",
            "--locked",
        ],
        cwd=root,
        env=env,
        check=True,
    )
    binary = target / "release" / "carver-stream-bench"
    completed = subprocess.run(
        [
            "/usr/bin/time",
            "-l",
            str(binary),
            str(image),
            str(args.image_size_gib * GIB),
            str(args.cancel_after_mib * 1024 * 1024),
        ],
        cwd=root,
        check=True,
        capture_output=True,
        text=True,
    )
    result = json.loads(completed.stdout.strip().splitlines()[-1])
    result["peak_rss_bytes"] = peak_rss(completed.stderr)
    result["memory_gate"] = bool(result["peak_rss_bytes"] and result["peak_rss_bytes"] < 64 * 1024 * 1024)
    result["pass"] = bool(result["cancelled"] and result["progress_events"] >= 1 and result["memory_gate"])
    print(json.dumps(result, ensure_ascii=False, indent=2))
    if not args.keep_image:
        image.unlink(missing_ok=True)
    return 0 if result["pass"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
