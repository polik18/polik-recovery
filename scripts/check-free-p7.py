#!/usr/bin/env python3
"""Fail closed when P7 workflows drift from the owner's free-only policy."""

from __future__ import annotations

import json
import pathlib
import re
import sys


ROOT = pathlib.Path(__file__).resolve().parent.parent
FULL_SHA = re.compile(r"^[0-9a-f]{40}$")


def main() -> int:
    errors: list[str] = []
    policy = json.loads((ROOT / "release" / "free-supply-chain-policy.json").read_text(encoding="utf-8"))
    if policy.get("cost_policy") != "free-only":
        errors.append("release policy must remain free-only")

    workflows = sorted((ROOT / ".github" / "workflows").glob("*.yml"))
    for workflow in workflows:
        for number, line in enumerate(workflow.read_text(encoding="utf-8").splitlines(), start=1):
            match = re.search(r"\buses:\s*([^\s]+)", line)
            if not match:
                continue
            action = match.group(1)
            if "@" not in action or not FULL_SHA.fullmatch(action.rsplit("@", 1)[1]):
                errors.append(f"{workflow.name}:{number}: action is not pinned to a full commit SHA")

    free_workflow = ROOT / ".github" / "workflows" / "free-p7-evidence.yml"
    text = free_workflow.read_text(encoding="utf-8")
    required_markers = [
        "workflow_dispatch:",
        "github.repository_visibility",
        "free-only policy",
        "actions/attest@",
        "scripts/release/verify_release_evidence.py",
    ]
    for marker in required_markers:
        if marker not in text:
            errors.append(f"free P7 workflow is missing fail-closed marker: {marker}")
    forbidden_markers = ["gh release", "softprops/action-gh-release", "release: write"]
    for marker in forbidden_markers:
        if marker in text:
            errors.append(f"free P7 evidence workflow must not publish releases: {marker}")

    if errors:
        for error in errors:
            print(f"ERROR: {error}", file=sys.stderr)
        return 1
    print("Free-only P7 policy and pinned workflows are consistent.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
