#!/usr/bin/env python3
"""Generate deterministic, free-tooling release evidence for local artifacts.

This does not claim that an unsigned artifact is safe to publish.  It emits an
explicit signing status and keeps public_release_eligible false unless the
source is clean and the caller records an externally verifiable attestation.
"""

from __future__ import annotations

import argparse
import datetime as dt
import hashlib
import json
import os
import pathlib
import shutil
import subprocess
import sys
import urllib.parse
import uuid


def run(command: list[str], cwd: pathlib.Path) -> str:
    completed = subprocess.run(
        command,
        cwd=cwd,
        check=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
        encoding="utf-8",
    )
    return completed.stdout.strip()


def sha256(path: pathlib.Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def write_json(path: pathlib.Path, value: object) -> None:
    path.write_text(json.dumps(value, indent=2, sort_keys=True) + "\n", encoding="utf-8")


def tool_path(name: str) -> str:
    configured = os.environ.get(name.upper())
    if configured:
        return configured
    discovered = shutil.which(name)
    if discovered:
        return discovered
    candidate = pathlib.Path.home() / ".cargo" / "bin" / (f"{name}.exe" if os.name == "nt" else name)
    if candidate.is_file():
        return str(candidate)
    raise FileNotFoundError(f"required tool not found: {name}")


def source_timestamp(repo: pathlib.Path, commit: str) -> str:
    epoch = os.environ.get("SOURCE_DATE_EPOCH")
    if epoch is None:
        epoch = run(["git", "show", "-s", "--format=%ct", commit], repo)
    moment = dt.datetime.fromtimestamp(int(epoch), tz=dt.timezone.utc)
    return moment.isoformat().replace("+00:00", "Z")


def cargo_purl(name: str, version: str) -> str:
    return f"pkg:cargo/{urllib.parse.quote(name, safe='')}@{urllib.parse.quote(version, safe='.+-')}"


def license_screen(packages: list[dict], forbidden: list[str]) -> dict:
    missing: list[str] = []
    forbidden_hits: list[dict[str, str]] = []
    inventory: list[dict[str, str]] = []
    for package in sorted(packages, key=lambda item: (item["name"], item["version"], item["id"])):
        license_expression = package.get("license") or ""
        item = {
            "name": package["name"],
            "version": package["version"],
            "license": license_expression,
            "source": package.get("source") or "workspace",
        }
        inventory.append(item)
        if not license_expression:
            missing.append(f'{package["name"]}@{package["version"]}')
        for marker in forbidden:
            if marker.lower() in license_expression.lower():
                forbidden_hits.append(
                    {
                        "package": f'{package["name"]}@{package["version"]}',
                        "license": license_expression,
                        "marker": marker,
                    }
                )
    passed = not missing and not forbidden_hits
    return {
        "status": "pass" if passed else "fail",
        "scope": "Cargo metadata automated license-expression screen",
        "limitations": [
            "This is not legal advice or a review of license text and notices.",
            "A passing result does not replace source-offer or attribution review.",
        ],
        "missing_license": missing,
        "forbidden_hits": forbidden_hits,
        "packages": inventory,
    }


def make_sbom(metadata: dict, timestamp: str, commit: str) -> dict:
    packages = sorted(metadata["packages"], key=lambda item: (item["name"], item["version"], item["id"]))
    components = []
    for package in packages:
        component = {
            "type": "library",
            "bom-ref": package["id"],
            "name": package["name"],
            "version": package["version"],
            "purl": cargo_purl(package["name"], package["version"]),
        }
        if package.get("license"):
            component["licenses"] = [{"expression": package["license"]}]
        if package.get("source"):
            component["properties"] = [{"name": "cargo:source", "value": package["source"]}]
        components.append(component)

    dependencies = []
    package_ids = {package["id"] for package in packages}
    for node in sorted(metadata.get("resolve", {}).get("nodes", []), key=lambda item: item["id"]):
        dependencies.append(
            {
                "ref": node["id"],
                "dependsOn": sorted(dep for dep in node.get("dependencies", []) if dep in package_ids),
            }
        )
    serial_seed = f"polik-recovery\0{commit}".encode()
    serial = uuid.UUID(bytes=hashlib.sha256(serial_seed).digest()[:16])
    return {
        "bomFormat": "CycloneDX",
        "specVersion": "1.5",
        "serialNumber": f"urn:uuid:{serial}",
        "version": 1,
        "metadata": {
            "timestamp": timestamp,
            "tools": {"components": [{"type": "application", "name": "generate_release_evidence.py"}]},
            "component": {"type": "application", "name": "polik-recovery", "version": workspace_version(metadata)},
        },
        "components": components,
        "dependencies": dependencies,
    }


def workspace_version(metadata: dict) -> str:
    workspace_ids = set(metadata["workspace_members"])
    versions = {package["version"] for package in metadata["packages"] if package["id"] in workspace_ids}
    if len(versions) != 1:
        raise ValueError(f"workspace versions are not unified: {sorted(versions)}")
    return versions.pop()


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--artifact", action="append", required=True, type=pathlib.Path)
    parser.add_argument("--output", required=True, type=pathlib.Path)
    parser.add_argument("--attestation-url", help="Externally verifiable free keyless attestation URL")
    parser.add_argument("--repo", type=pathlib.Path, default=pathlib.Path(__file__).resolve().parents[2])
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    repo = args.repo.resolve()
    output = args.output.resolve()
    product = repo / "product"
    policy = json.loads((repo / "release" / "free-supply-chain-policy.json").read_text(encoding="utf-8"))

    artifacts: list[pathlib.Path] = []
    seen_names: set[str] = set()
    for requested in args.artifact:
        artifact = (repo / requested).resolve() if not requested.is_absolute() else requested.resolve()
        if not artifact.is_file() or artifact.is_symlink():
            raise ValueError(f"artifact must be an existing regular non-symlink file: {artifact}")
        if artifact.name in seen_names:
            raise ValueError(f"artifact basenames must be unique: {artifact.name}")
        seen_names.add(artifact.name)
        artifacts.append(artifact)

    output.mkdir(parents=True, exist_ok=True)
    commit = run(["git", "rev-parse", "HEAD"], repo)
    dirty_entries = run(["git", "status", "--porcelain=v1"], repo).splitlines()
    dirty = bool(dirty_entries)
    timestamp = source_timestamp(repo, commit)
    cargo = tool_path("cargo")
    metadata = json.loads(
        run(
            [cargo, "metadata", "--locked", "--all-features", "--format-version", "1"],
            product,
        )
    )
    version = workspace_version(metadata)
    rustc = run([tool_path("rustc"), "--version", "--verbose"], product)

    artifact_subjects = []
    for artifact in sorted(artifacts, key=lambda path: path.name):
        bundled_artifact = output / artifact.name
        if artifact != bundled_artifact:
            shutil.copyfile(artifact, bundled_artifact)
        artifact_subjects.append(
            {
                "name": artifact.name,
                "digest": {"sha256": sha256(artifact)},
                "bytes": artifact.stat().st_size,
            }
        )

    screen = license_screen(metadata["packages"], policy["forbidden_license_markers"])
    write_json(output / "license-screen.json", screen)
    write_json(output / "sbom.cdx.json", make_sbom(metadata, timestamp, commit))
    shutil.copyfile(repo / "docs" / "capability-matrix.md", output / "capability-matrix.md")

    build_metadata = {
        "schema_version": 1,
        "project": "polik-recovery",
        "version": version,
        "commit": commit,
        "source_dirty": dirty,
        "source_timestamp": timestamp,
        "rustc": rustc.splitlines(),
        "cargo_lock_sha256": sha256(product / "Cargo.lock"),
        "cost_policy": policy["cost_policy"],
        "artifacts": artifact_subjects,
    }
    write_json(output / "build-metadata.json", build_metadata)

    attested = bool(args.attestation_url)
    evidence_bundle_complete = bool(attested and not dirty and screen["status"] == "pass")
    signing_status = {
        "schema_version": 1,
        "artifacts_signed": False,
        "checksums_signed": False,
        "keyless_attestation_present": attested,
        "attestation_url": args.attestation_url,
        "evidence_bundle_complete": evidence_bundle_complete,
        "public_release_eligible": False,
        "release_blockers": [
            "An artifact attestation is not a Windows Authenticode signature.",
            "The product P2-P6 platform and safety gates are not all complete.",
            "macOS Developer ID notarization has no generally available free-only path.",
        ],
        "free_paths": {
            "windows": "Apply to SignPath Foundation; acceptance is external and not assumed.",
            "provenance": "GitHub artifact attestation on a public repository.",
            "macos": "No free generally available Developer ID notarization path; keep distribution blocked.",
        },
    }
    write_json(output / "signing-status.json", signing_status)

    statement = {
        "_type": "https://in-toto.io/Statement/v1",
        "subject": [{"name": item["name"], "digest": item["digest"]} for item in artifact_subjects],
        "predicateType": "https://slsa.dev/provenance/v1",
        "predicate": {
            "buildDefinition": {
                "buildType": "https://github.com/polik18/polik-recovery/release/free-local-build/v1",
                "externalParameters": {"profile": "release", "costPolicy": "free-only"},
                "internalParameters": {"sourceDirty": dirty},
                "resolvedDependencies": [
                    {"uri": "git+https://github.com/polik18/polik-recovery", "digest": {"gitCommit": commit}}
                ],
            },
            "runDetails": {
                "builder": {"id": os.environ.get("GITHUB_SERVER_URL", "local-free-tooling")},
                "metadata": {"invocationId": os.environ.get("GITHUB_RUN_ID", "local-unattested")},
            },
        },
    }
    (output / "provenance.intoto.jsonl").write_text(
        json.dumps(statement, separators=(",", ":"), sort_keys=True) + "\n", encoding="utf-8"
    )

    readme = f"""# Release evidence for {version}

Generated from `{commit}` using only repository code, Git, Python, Cargo and Rust.

- Source dirty: `{str(dirty).lower()}`
- License screen: `{screen['status']}`
- External keyless attestation recorded: `{str(attested).lower()}`
- Evidence bundle complete: `{str(evidence_bundle_complete).lower()}`
- Public release eligible: `false`

`SHA256SUMS` proves byte identity only. It does not prove recovery correctness.
`provenance.intoto.jsonl` is unsigned local metadata unless `signing-status.json`
records an externally verifiable attestation URL.
"""
    (output / "README.md").write_text(readme, encoding="utf-8", newline="\n")

    sum_targets = sorted(
        [path for path in output.iterdir() if path.is_file() and path.name != "SHA256SUMS"],
        key=lambda path: path.name,
    )
    lines = [f"{sha256(path)}  {path.name}" for path in sum_targets]
    (output / "SHA256SUMS").write_text("\n".join(lines) + "\n", encoding="ascii", newline="\n")

    missing = [name for name in policy["required_release_files"] if not (output / name).is_file()]
    if missing:
        raise RuntimeError(f"required release evidence missing: {missing}")
    if screen["status"] != "pass":
        print("license screen failed; evidence retained for review", file=sys.stderr)
        return 2
    print(f"release evidence generated at {output}")
    print(f"public_release_eligible={str(signing_status['public_release_eligible']).lower()}")
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (OSError, ValueError, subprocess.CalledProcessError) as error:
        print(f"error: {error}", file=sys.stderr)
        raise SystemExit(1)
