#!/usr/bin/env python3
"""Regenerate every synthetic corpus twice and require byte-identical trees."""

from __future__ import annotations

import hashlib
from pathlib import Path
import subprocess
import sys
import tempfile


ROOT = Path(__file__).resolve().parents[2]
GENERATORS = (
    "generate_fixture.py",
    "generate_fs_fixtures.py",
    "generate_fragmented_fixtures.py",
    "generate_apfs_fixtures.py",
    "generate_partition_fixture.py",
)


def tree_hashes(root: Path) -> dict[str, str]:
    return {
        str(path.relative_to(root)): hashlib.sha256(path.read_bytes()).hexdigest()
        for path in sorted(root.rglob("*"))
        if path.is_file()
    }


def main() -> int:
    target = ROOT / "target"
    target.mkdir(exist_ok=True)
    with tempfile.TemporaryDirectory(prefix="fixture-repro-", dir=target) as temp:
        output = Path(temp) / "corpus"
        for generator in GENERATORS:
            script = ROOT / "tools" / "quality" / generator
            subprocess.run(
                [sys.executable, str(script), "--out", str(output / script.stem)],
                cwd=ROOT,
                check=True,
                stdout=subprocess.DEVNULL,
            )
        first = tree_hashes(output)

        for generator in GENERATORS:
            script = ROOT / "tools" / "quality" / generator
            subprocess.run(
                [sys.executable, str(script), "--out", str(output / script.stem)],
                cwd=ROOT,
                check=True,
                stdout=subprocess.DEVNULL,
            )
        second = tree_hashes(output)

    if first != second:
        changed = sorted(set(first) | set(second))
        for path in changed:
            if first.get(path) != second.get(path):
                print(f"non-reproducible fixture: {path}", file=sys.stderr)
        return 1

    print(f"Synthetic corpus is reproducible ({len(first)} files).")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
