#!/usr/bin/env python3
"""Benchmark one million deterministic NTFS MFT records.

The generated image and JSON stay under product/target/bench and are disposable.
The comparison path intentionally reads one record per positional read while the
production path reads 4 MiB batches and then parses records from each batch.
"""

from __future__ import annotations

import argparse
import json
import os
import subprocess
import time
from pathlib import Path


DEFAULT_RECORDS = 1_000_000


def product_root() -> Path:
    return Path(__file__).resolve().parents[2]


def build_runner(root: Path) -> Path:
    toolchain = Path.home() / ".rustup" / "toolchains" / "1.85.1-aarch64-apple-darwin" / "bin"
    env = os.environ.copy()
    env["PATH"] = f"{toolchain}:{env.get('PATH', '')}"
    target = root / "target" / "bench" / "ntfs-mft-cargo-target"
    subprocess.run(
        ["cargo", "build", "--manifest-path", "tools/bench/Cargo.toml", "--bin", "ntfs-mft-bench", "--target-dir", str(target), "--release", "--locked"],
        cwd=root,
        env=env,
        check=True,
    )
    return target / "release" / "ntfs-mft-bench"


def run_case(binary: Path, image: Path, mode: str, records: int) -> dict[str, object]:
    started = time.perf_counter()
    completed = subprocess.run(
        [str(binary), "run", str(image), mode, str(records)],
        check=True,
        capture_output=True,
        text=True,
    )
    elapsed_wall = time.perf_counter() - started
    result = json.loads(completed.stdout.strip().splitlines()[-1])
    result["wall_seconds"] = round(elapsed_wall, 6)
    return result


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--records", type=int, default=DEFAULT_RECORDS)
    parser.add_argument("--output", type=Path)
    parser.add_argument("--keep-image", action="store_true")
    args = parser.parse_args()

    if args.records <= 0 or args.records > 1_000_000:
        raise SystemExit("--records must be between 1 and 1000000")
    root = product_root()
    bench_root = root / "target" / "bench" / "ntfs-mft"
    bench_root.mkdir(parents=True, exist_ok=True)
    image = bench_root / f"synthetic-{args.records}-records.img"
    output = args.output or root / "target" / "bench" / "ntfs-mft-bench.json"
    binary = build_runner(root)

    expected_size = 4096 + args.records * 1024
    generated = False
    if not image.exists() or image.stat().st_size != expected_size:
        subprocess.run([str(binary), "generate", str(image), str(args.records)], check=True)
        generated = True

    per_record = run_case(binary, image, "per-record", args.records)
    batched = run_case(binary, image, "batched", args.records)
    per_time = float(per_record["elapsed_seconds"])
    batched_time = float(batched["elapsed_seconds"])
    speedup = per_time / batched_time if batched_time else 0.0
    result = {
        "benchmark": "ntfs-mft-batched-read",
        "record_count": args.records,
        "image": str(image.relative_to(root)),
        "image_size_bytes": image.stat().st_size,
        "generated_this_run": generated,
        "baseline_per_record": per_record,
        "batched": batched,
        "speedup": round(speedup, 2),
        "read_call_reduction": round(1 - int(batched["read_calls"]) / int(per_record["read_calls"]), 6),
        "pass": speedup >= 3.0 and per_record["parsed"] == args.records and batched["parsed"] == args.records,
    }
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text(json.dumps(result, ensure_ascii=False, indent=2) + "\n")
    print(json.dumps(result, ensure_ascii=False, indent=2))
    if not args.keep_image:
        image.unlink(missing_ok=True)
    return 0 if result["pass"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
