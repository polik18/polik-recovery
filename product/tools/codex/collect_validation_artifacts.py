#!/usr/bin/env python3
"""Collect validation logs/reports into a single handoff zip."""
from __future__ import annotations

import argparse
import json
import platform
import zipfile
from datetime import datetime, timezone
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
DEFAULT_OUT = ROOT / "target" / "codex-validation"

PATTERNS = [
    "target/codex-validation/**/*",
    "target/quality-gate/**/*",
    "target/fs-quality/**/*",
    "target/fragmented-quality/**/*",
    "target/apfs-quality/**/*",
    "target/real-apfs-quality/**/*",
    "target/release/bundle/dmg/*.dmg",
    "target/**/*map_report*.json",
    "target/**/*map_report*.html",
    "docs/plans/macos-codex-validation-plan.md",
    "docs/plans/codex-macos-handoff.md",
    "docs/plans/codex-agent-prompt.md",
    "docs/plans/macos-build-release-checklist.md",
    "docs/plans/macos-codex-image-first-plan.md",
    "docs/workflows/image-first-recovery.md",
    "docs/workflows/ddrescue-map-retry.md",
    "docs/workflows/session-based-recovery.md",
    "docs/system/12-image-first-recovery-workflow.md",
    "docs/system/13-ddrescue-map-retry.md",
    "docs/system/14-session-management.md",
    "V1_7_MACOS_CODEX_HANDOFF_IMPLEMENTATION.md",
    "V1_8_IMAGE_FIRST_WORKFLOW_IMPLEMENTATION.md",
    "V1_9_DDRESCUE_MAP_RETRY_IMPLEMENTATION.md",
    "V2_0_SESSION_MANAGEMENT_IMPLEMENTATION.md",
    "Cargo.toml",
    "src-tauri/tauri.conf.json",
    "project_status.md",
    "work_plan.md",
    "CODEX_WORK_LOG.md",
]


def iter_files(patterns: list[str]) -> list[Path]:
    files: set[Path] = set()
    for pattern in patterns:
        for path in ROOT.glob(pattern):
            if path.is_file() and path.suffix.lower() != ".zip":
                files.add(path)
    return sorted(files)


def main() -> int:
    parser = argparse.ArgumentParser(description="Collect Codex validation artifacts")
    parser.add_argument("--out-dir", type=Path, default=DEFAULT_OUT)
    parser.add_argument("--name", default=None, help="Zip file name")
    parser.add_argument("--extra", action="append", default=[], help="Additional glob pattern")
    parser.add_argument("--session", type=Path, help="Optional recovery session folder to include")
    args = parser.parse_args()

    out_dir = args.out_dir.resolve()
    out_dir.mkdir(parents=True, exist_ok=True)
    stamp = datetime.now(timezone.utc).astimezone().strftime("%Y%m%d-%H%M%S")
    zip_name = args.name or f"polik-codex-validation-artifacts-{stamp}.zip"
    zip_path = out_dir / zip_name

    patterns = list(PATTERNS) + args.extra
    files = iter_files(patterns)
    session_files: list[Path] = []
    session: Path | None = None
    if args.session:
        session = args.session.expanduser().resolve()
        if session.exists():
            session_files = sorted(p for p in session.glob("**/*") if p.is_file())
    manifest = {
        "schema": "polik.codex.artifacts.v1",
        "created_at": datetime.now(timezone.utc).astimezone().isoformat(timespec="seconds"),
        "platform": platform.platform(),
        "file_count": len(files) + len(session_files),
        "files": [str(path.relative_to(ROOT)) for path in files],
        "session": str(session) if session else None,
        "session_files": [str(path.relative_to(session)) for path in session_files] if session else [],
    }

    with zipfile.ZipFile(zip_path, "w", compression=zipfile.ZIP_DEFLATED) as zf:
        zf.writestr("artifact_manifest.json", json.dumps(manifest, ensure_ascii=False, indent=2))
        for path in files:
            zf.write(path, path.relative_to(ROOT))
        if session:
            for path in session_files:
                zf.write(path, Path("session") / path.relative_to(session))

    print(zip_path)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
