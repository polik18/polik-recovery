#!/usr/bin/env python3
"""Generate deterministic carver benchmark images and run the CLI against them.

The images and carved output live below product/target/bench and are disposable.
No real device path is accepted or used by this benchmark.
"""

from __future__ import annotations

import argparse
import json
import os
import random
import re
import shutil
import subprocess
import sys
import time
from pathlib import Path


MIB = 1024 * 1024
SEED = 0x504F4C494B202611
IMAGE_SPECS = (
    ("zero256", 256, "zero"),
    ("random256", 256, "random"),
    ("pk_dense64", 64, "pk"),
    ("ftyp_dense64", 64, "ftyp"),
)


def product_root() -> Path:
    return Path(__file__).resolve().parents[2]


def write_image(path: Path, size_mib: int, kind: str) -> None:
    size = size_mib * MIB
    chunk_size = MIB
    rng = random.Random(SEED)
    path.parent.mkdir(parents=True, exist_ok=True)

    with path.open("wb") as image:
        for offset in range(0, size, chunk_size):
            length = min(chunk_size, size - offset)
            if kind == "random":
                data = bytearray(rng.getrandbits(8) for _ in range(length))
            else:
                data = bytearray(length)

            if kind == "pk":
                data[:4] = b"PK\x03\x04"
            elif kind == "ftyp":
                data[:24] = b"\x00\x00\x00\x18ftypisom\x00\x00\x02\x00isomiso2"
            image.write(data)


def build_bench_runner(root: Path) -> Path:
    env = os.environ.copy()
    rust_toolchain = Path.home() / ".rustup" / "toolchains" / "1.85.1-aarch64-apple-darwin" / "bin"
    env["PATH"] = f"{rust_toolchain}:{env.get('PATH', '')}"
    target_dir = root / "target" / "bench" / "cargo-target"
    subprocess.run(
        [
            "cargo",
            "build",
            "--manifest-path",
            "tools/bench/Cargo.toml",
            "--target-dir",
            str(target_dir),
            "--release",
            "--locked",
        ],
        cwd=root,
        env=env,
        check=True,
    )
    return target_dir / "release" / "polik-recovery-carver-bench"


def parse_peak_rss(stderr: str) -> int | None:
    patterns = (
        r"maximum resident set size:\s*(\d+)",
        r"\s*(\d+)\s+maximum resident set size",
        r"Maximum resident set size \(kbytes\):\s*(\d+)",
    )
    for pattern in patterns:
        match = re.search(pattern, stderr)
        if match:
            value = int(match.group(1))
            return value if "kbytes" not in pattern.lower() else value * 1024
    return None


def run_case(cli: Path, image: Path, output_dir: Path) -> dict[str, object]:
    if output_dir.exists():
        shutil.rmtree(output_dir)
    output_dir.mkdir(parents=True)
    command = [
        "/usr/bin/time",
        "-l",
        str(cli),
        str(image),
    ]
    started = time.perf_counter()
    completed = subprocess.run(command, text=True, capture_output=True)
    elapsed = time.perf_counter() - started
    runner_output = completed.stdout.strip().splitlines()
    runner_summary = json.loads(runner_output[-1]) if runner_output else {}
    carved_count = int(runner_summary.get("carved_count", 0))
    by_type = runner_summary.get("by_type", {})
    if not isinstance(by_type, dict):
        raise RuntimeError(f"invalid by_type output for {image}")

    size_bytes = image.stat().st_size
    result: dict[str, object] = {
        "image": image.name,
        "size_bytes": size_bytes,
        "elapsed_seconds": round(elapsed, 3),
        "throughput_mib_s": round(size_bytes / MIB / elapsed, 2) if elapsed else 0.0,
        "peak_rss_bytes": parse_peak_rss(completed.stderr),
        "carved_count": carved_count,
        "by_type": by_type,
        "exit_code": completed.returncode,
    }
    if completed.returncode != 0:
        result["stderr_tail"] = completed.stderr[-2000:]
    return result


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--output",
        type=Path,
        help="JSON output path (default: target/bench/carver-bench.json)",
    )
    parser.add_argument(
        "--skip-build",
        action="store_true",
        help="reuse target/release/polik-recovery-cli",
    )
    parser.add_argument(
        "--enforce-random-zero",
        action="store_true",
        help="fail unless the random negative corpus carves zero files",
    )
    args = parser.parse_args()

    root = product_root()
    bench_root = root / "target" / "bench" / "carver"
    images_root = bench_root / "images"
    outputs_root = bench_root / "outputs"
    results_path = args.output or root / "target" / "bench" / "carver-bench.json"
    results_path.parent.mkdir(parents=True, exist_ok=True)

    cli = root / "target" / "bench" / "cargo-target" / "release" / "polik-recovery-carver-bench"
    if not args.skip_build:
        cli = build_bench_runner(root)
    if not cli.is_file():
        raise SystemExit(f"missing CLI binary: {cli}; omit --skip-build to build it")

    results = []
    for name, size_mib, kind in IMAGE_SPECS:
        image = images_root / f"{name}.img"
        write_image(image, size_mib, kind)
        results.append(run_case(cli, image, outputs_root / name))

    report = {
        "schema": "polik-recovery-carver-bench/v1",
        "seed": SEED,
        "cases": results,
        "random_carved_count": next(
            case["carved_count"] for case in results if case["image"] == "random256.img"
        ),
    }
    results_path.write_text(json.dumps(report, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    print(json.dumps(report, indent=2, sort_keys=True))

    random_count = report["random_carved_count"]
    if args.enforce_random_zero and random_count != 0:
        print(
            f"random negative corpus gate failed: carved_count={random_count}, expected 0",
            file=sys.stderr,
        )
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
