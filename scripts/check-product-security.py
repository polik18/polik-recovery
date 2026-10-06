#!/usr/bin/env python3
"""Fail CI if quarantined privileged prototypes or generated junk reappear."""

from __future__ import annotations

from pathlib import Path
import sys


ROOT = Path(__file__).resolve().parent.parent
PRODUCT = ROOT / "product"

FORBIDDEN_SOURCE = {
    "world-writable privileged socket": "0o666",
    "legacy privileged server entry point": "--privileged-server",
    "AppleScript password elevation": "administrator privileges",
    "legacy unauthenticated helper socket": "/var/run/polik-recovery-helper.sock",
    "force-unmount command": '"unmountDisk", "force"',
    "force-unmount CLI option": "--force-unmount",
    "unsafe dd imaging fallback": "conv=noerror,sync",
    "simulated scan percentage": "start_fake_progress",
    "Windows shell-based file opener": 'Command::new("cmd")',
    "stale AppleScript runtime dependency": "osascript",
    "legacy scan helper CLI entry": "--scan-helper",
    "legacy extract helper CLI entry": "--extract-helper",
    "legacy batch helper CLI entry": "--extract-batch-helper",
}

SCANNED_SUFFIXES = {".html", ".py", ".rs", ".toml", ".json", ".plist"}


def main() -> int:
    errors: list[str] = []

    for path in PRODUCT.rglob("*"):
        relative = path.relative_to(PRODUCT)
        if any(part in {"target", "__pycache__", "gen"} for part in relative.parts):
            continue
        if path.name == ".DS_Store" or path.suffix == ".pyc":
            errors.append(f"generated file must not be committed: product/{relative}")
            continue
        if not path.is_file() or path.suffix not in SCANNED_SUFFIXES:
            continue

        text = path.read_text(encoding="utf-8", errors="replace")
        for description, needle in FORBIDDEN_SOURCE.items():
            if needle in text:
                errors.append(f"{description}: product/{relative}")

    helper_source = PRODUCT / "crates" / "device-macos" / "src" / "bin" / "helper.rs"
    if helper_source.exists():
        errors.append("legacy helper binary source must remain removed")

    desktop_source = PRODUCT / "src-tauri" / "src" / "lib.rs"
    desktop_text = desktop_source.read_text(encoding="utf-8")
    if "data_recovery_safe_writer::write_recovered_file(" in desktop_text:
        errors.append("desktop recovery output must use the storage-identity verified writer")
    if "StorageIdentity::from_physical_stores" in desktop_text:
        errors.append("desktop must not construct caller-asserted storage identities")
    if "data_recovery_safe_writer::resolve_image_storage_identities" not in desktop_text:
        errors.append("desktop must use the canonical platform storage resolver")

    for relative in [
        Path("crates/cli/src/main.rs"),
        Path("crates/recovery-plan/src/lib.rs"),
    ]:
        source = (PRODUCT / relative).read_text(encoding="utf-8")
        if "data_recovery_safe_writer::write_recovered_file(" in source or (
            relative.name == "lib.rs" and "write_recovered_file(" in source
        ):
            errors.append(f"production recovery output must use verified identities: product/{relative}")
        if "verify_open_source_storage" not in source:
            errors.append(f"production source handle must be identity-bound: product/{relative}")

    if "verify_open_source_storage" not in desktop_text:
        errors.append("desktop source handle must be identity-bound before recovery writes")

    cli_source = (PRODUCT / "crates" / "cli" / "src" / "main.rs").read_text(
        encoding="utf-8"
    )
    for forbidden in ["MacOsRawDiskSource", "WindowsRawDiskSource", "fn raw_hash(", "fn raw_info("]:
        if forbidden in cli_source:
            errors.append(f"CLI raw-device implementation must remain disabled: {forbidden}")

    safe_writer_source = (
        PRODUCT / "crates" / "safe-writer" / "src" / "lib.rs"
    ).read_text(encoding="utf-8")
    if "pub fn write_recovered_file(" in safe_writer_source:
        errors.append("path-only recovered-file writer must not be public")
    for primitive in [
        "reject_symlink_components(",
        "libc::openat(",
        "libc::renameatx_np(",
        "libc::fstatfs(",
        "verify_opened_output_storage(",
        "verify_opened_source_fd(",
        "VerifiedSourceStorage",
        "SourceFileIdentityChanged",
        "StorageIdentityProvenance::MacOsResolved",
    ]:
        if primitive not in safe_writer_source:
            errors.append(
                f"Unix safe-writer directory-handle protection is missing: {primitive}"
            )
    for primitive in [
        "StorageIdentityProvenance::WindowsResolved",
        "IOCTL_VOLUME_GET_VOLUME_DISK_EXTENTS",
        "windows_file_object_identity(",
        "reject_windows_reparse_components(",
        "windows_file_parent_matches_directory(",
        "SetFileInformationByHandle(",
        "reconcile_recovery_output(",
        "injected disk-full failure",
    ]:
        if primitive not in safe_writer_source:
            errors.append(
                f"Windows safe-writer storage/handle protection is missing: {primitive}"
            )

    core_source = (PRODUCT / "crates" / "core" / "src" / "block.rs").read_text(
        encoding="utf-8"
    )
    for primitive in ["libc::O_NOFOLLOW", "file.metadata()", "with_raw_fd"]:
        if primitive not in core_source:
            errors.append(f"regular source handle protection is missing: {primitive}")
    for primitive in ["FILE_FLAG_OPEN_REPARSE_POINT", "with_raw_handle"]:
        if primitive not in core_source:
            errors.append(f"Windows regular source handle protection is missing: {primitive}")

    if errors:
        for error in errors:
            print(f"ERROR: {error}", file=sys.stderr)
        return 1

    print("Product source contains no quarantined privileged prototype or generated junk.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
