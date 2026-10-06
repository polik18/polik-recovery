#!/usr/bin/env python3
"""Polik Recovery v1.5 build doctor.

This script does not build the app. It checks the local development environment
and prints actionable diagnostics before the user wastes time on cargo/tauri errors.
"""
from __future__ import annotations

import json
import os
import platform
import shutil
import subprocess
import sys
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[2]


def run(cmd: list[str], timeout: int = 12) -> tuple[bool, str]:
    try:
        p = subprocess.run(cmd, cwd=ROOT, text=True, capture_output=True, timeout=timeout)
        out = (p.stdout + p.stderr).strip()
        return p.returncode == 0, out
    except FileNotFoundError as exc:
        return False, str(exc)
    except subprocess.TimeoutExpired as exc:
        return False, f"timeout after {timeout}s: {exc}"


def check(name: str, ok: bool, detail: str, action: str = "") -> dict[str, Any]:
    return {"name": name, "status": "ok" if ok else "warn", "detail": detail, "action": action}


def main() -> int:
    checks: list[dict[str, Any]] = []

    checks.append(check("project_root", (ROOT / "Cargo.toml").exists(), str(ROOT), "請在專案根目錄保留 Cargo.toml。"))

    cargo = shutil.which("cargo")
    checks.append(check("cargo", bool(cargo), cargo or "not found", "安裝 Rust toolchain 後再執行 cargo build。"))
    if cargo:
        ok, out = run([cargo, "--version"])
        checks.append(check("cargo_version", ok, out, "cargo 無法執行時，重新安裝 rustup。"))
        ok, out = run([cargo, "metadata", "--no-deps", "--format-version", "1"], timeout=20)
        checks.append(check("cargo_metadata", ok, out[-2000:] if out else "ok", "若這裡失敗，通常是 Cargo.toml workspace 或 crate dependency 壞掉。"))

    rustc = shutil.which("rustc")
    checks.append(check("rustc", bool(rustc), rustc or "not found", "安裝 Rust stable toolchain。"))

    python = sys.executable
    checks.append(check("python", True, python, "品質門檻工具需要 Python 3。"))

    for rel in [
        "src-tauri/tauri.conf.json",
        "apps/desktop-prototype/index.html",
        "tools/quality/run_quality_gate.py",
        "tools/macos/run_real_apfs_quality_gate.py",
    ]:
        checks.append(check(f"file:{rel}", (ROOT / rel).exists(), rel, "若缺檔，請重新解壓完整專案 zip。"))

    # JSON syntax check for tauri config.
    cfg = ROOT / "src-tauri/tauri.conf.json"
    if cfg.exists():
        try:
            json.loads(cfg.read_text(encoding="utf-8"))
            checks.append(check("tauri_config_json", True, "valid json"))
        except Exception as exc:  # noqa: BLE001
            checks.append(check("tauri_config_json", False, str(exc), "修正 tauri.conf.json 語法。"))

    system = platform.system()
    if system == "Darwin":
        for tool in ["diskutil", "hdiutil"]:
            path = shutil.which(tool)
            checks.append(check(f"macos_tool:{tool}", bool(path), path or "not found", f"macOS 真實磁碟與 APFS 測試需要 {tool}。"))
    else:
        checks.append(check("platform", False, system, "桌面儲存拓樸與 real APFS quality gate 主要在 macOS 驗證；其他平台先用合成映像檔。"))

    ok_count = sum(1 for c in checks if c["status"] == "ok")
    print(f"Polik Recovery build doctor — {ok_count}/{len(checks)} checks OK")
    print("=" * 72)
    failed = False
    for c in checks:
        icon = "✅" if c["status"] == "ok" else "🟡"
        print(f"{icon} {c['name']}: {c['detail']}")
        if c["status"] != "ok":
            failed = True
            if c.get("action"):
                print(f"   建議：{c['action']}")
    print("=" * 72)
    print("接下來建議：make quality-gate-all")
    return 1 if failed else 0


if __name__ == "__main__":
    raise SystemExit(main())
