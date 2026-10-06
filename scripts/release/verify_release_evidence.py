#!/usr/bin/env python3
from __future__ import annotations

import argparse
import hashlib
import json
import pathlib
import re
import sys


SUM_LINE = re.compile(r"^([0-9a-f]{64})  ([^/\\]+)$")


def sha256(path: pathlib.Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("bundle", type=pathlib.Path)
    parser.add_argument("--repo", type=pathlib.Path, default=pathlib.Path(__file__).resolve().parents[2])
    args = parser.parse_args()
    bundle = args.bundle.resolve()
    policy = json.loads(
        (args.repo.resolve() / "release" / "free-supply-chain-policy.json").read_text(encoding="utf-8")
    )
    if not bundle.is_dir() or bundle.is_symlink():
        raise ValueError("bundle must be a regular directory, not a symlink")

    required = set(policy["required_release_files"])
    metadata = json.loads((bundle / "build-metadata.json").read_text(encoding="utf-8"))
    required.update(item["name"] for item in metadata["artifacts"])
    missing = sorted(name for name in required if not (bundle / name).is_file())
    if missing:
        raise ValueError(f"bundle is missing required files: {missing}")

    seen: set[str] = set()
    for line in (bundle / "SHA256SUMS").read_text(encoding="ascii").splitlines():
        match = SUM_LINE.fullmatch(line)
        if not match:
            raise ValueError(f"invalid SHA256SUMS line: {line!r}")
        expected, name = match.groups()
        if name in seen:
            raise ValueError(f"duplicate SHA256SUMS entry: {name}")
        seen.add(name)
        path = bundle / name
        if not path.is_file() or path.is_symlink():
            raise ValueError(f"hashed entry is missing or not a regular file: {name}")
        if sha256(path) != expected:
            raise ValueError(f"SHA-256 mismatch: {name}")
    unhashed = sorted(name for name in required - {"SHA256SUMS"} if name not in seen)
    if unhashed:
        raise ValueError(f"required files are not covered by SHA256SUMS: {unhashed}")

    sbom = json.loads((bundle / "sbom.cdx.json").read_text(encoding="utf-8"))
    if sbom.get("bomFormat") != "CycloneDX" or sbom.get("specVersion") != "1.5":
        raise ValueError("unsupported or malformed CycloneDX SBOM")
    screen = json.loads((bundle / "license-screen.json").read_text(encoding="utf-8"))
    if screen.get("status") != "pass":
        raise ValueError("license screen did not pass")
    signing = json.loads((bundle / "signing-status.json").read_text(encoding="utf-8"))
    if signing.get("public_release_eligible") is not False:
        raise ValueError("free-only preview bundle must remain blocked from public release")
    print(f"release evidence verified: {bundle}")
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (OSError, ValueError, json.JSONDecodeError) as error:
        print(f"error: {error}", file=sys.stderr)
        raise SystemExit(1)
