#!/usr/bin/env python3
"""Print a safe macOS setup checklist for Codex validation.

This script does not install anything by default. It checks what is present and
prints the exact commands a developer may run manually. Use --install-hints to
print Homebrew/Rust/Tauri installation hints.
"""
from __future__ import annotations

import argparse
import platform
import shutil
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
REQUIRED = ["git", "python3", "make", "rustc", "cargo", "node", "npm", "diskutil", "hdiutil"]
OPTIONAL_BUT_RECOMMENDED = ["gddrescue", "ddrescue"]


def version_of(cmd: str) -> str:
    try:
        if cmd in {"diskutil", "hdiutil"}:
            return "available"
        proc = subprocess.run([cmd, "--version"], stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True, timeout=8)
        return (proc.stdout or "").splitlines()[0][:120] if proc.stdout else "available"
    except Exception as exc:  # noqa: BLE001 - diagnostic script
        return f"available, version check failed: {exc}"


def main() -> int:
    parser = argparse.ArgumentParser(description="Check macOS Codex validation environment")
    parser.add_argument("--install-hints", action="store_true", help="Print manual install hints")
    args = parser.parse_args()

    print("Polik Recovery macOS/Codex bootstrap check")
    print(f"project: {ROOT}")
    print(f"platform: {platform.platform()}")
    print("")
    missing = []
    for cmd in REQUIRED:
        path = shutil.which(cmd)
        if path:
            print(f"[OK] {cmd}: {path} ({version_of(cmd)})")
        else:
            print(f"[MISS] {cmd}")
            missing.append(cmd)

    print("\nOptional but strongly recommended for failing disks:")
    if shutil.which("gddrescue") or shutil.which("ddrescue"):
        print(f"[OK] ddrescue/gddrescue: {shutil.which('gddrescue') or shutil.which('ddrescue')}")
    else:
        print("[MISS] ddrescue/gddrescue: not found; safe imaging will stop instead of falling back to dd")

    print("\nNext commands after tools are ready:")
    print("  make dev-doctor")
    print("  make macos-doctor")
    print("  make codex-validation-quick")
    print("  make codex-validation-full")
    print("  make codex-artifacts")

    print("\nmacOS permission reminder:")
    print("  System Settings → Privacy & Security → Full Disk Access")
    print("  Add Terminal / iTerm / Codex runner / packaged App before raw device tests.")

    if args.install_hints or missing:
        print("\nManual install hints:")
        print("  xcode-select --install")
        print("  /bin/bash -c \"$(curl -fsSL https://sh.rustup.rs)\"")
        print("  brew install node")
        print("  brew install ddrescue")
        print("  cargo install tauri-cli --locked")
        print("  rustup update stable")
    return 1 if missing else 0


if __name__ == "__main__":
    raise SystemExit(main())
