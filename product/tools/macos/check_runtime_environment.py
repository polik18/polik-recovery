#!/usr/bin/env python3
"""macOS runtime preflight for Polik Recovery.

This does safe, read-only checks. It does not scan user data and does not modify
source disks. It is meant to explain common real-machine failures before a user
starts a recovery job.
"""
from __future__ import annotations

import os
import platform
import shutil
import subprocess
from pathlib import Path


def run(cmd: list[str], timeout: int = 10) -> tuple[bool, str]:
    try:
        p = subprocess.run(cmd, text=True, capture_output=True, timeout=timeout)
        return p.returncode == 0, (p.stdout + p.stderr).strip()
    except Exception as exc:  # noqa: BLE001
        return False, str(exc)


def row(icon: str, title: str, detail: str, action: str = "") -> None:
    print(f"{icon} {title}: {detail}")
    if action:
        print(f"   建議：{action}")


def is_diskmanagement_framework_error(output: str) -> bool:
    return (
        "unable to use the DiskManagement framework" in output
        or "DiskArbitration" in output
    )


def main() -> int:
    print("Polik Recovery macOS runtime preflight")
    print("=" * 72)
    if platform.system() != "Darwin":
        row("🟡", "平台", platform.system(), "這個檢查器是給 macOS 實機掃描用；其他平台請用 tools/dev/build_doctor.py。")
        return 1

    fail = False
    for tool in ["diskutil", "hdiutil"]:
        path = shutil.which(tool)
        ok = bool(path)
        fail = fail or not ok
        row("✅" if ok else "❌", tool, path or "not found", f"請確認系統工具 {tool} 可用。")

    ddrescue = shutil.which("gddrescue") or shutil.which("ddrescue")
    row(
        "✅" if ddrescue else "❌",
        "ddrescue / gddrescue",
        ddrescue or "not found；安全成像流程會停止，不會退回 dd",
        "使用 Homebrew 安裝：brew install ddrescue；map 視覺化與 retry 流程需要它。",
    )
    fail = fail or not bool(ddrescue)

    ok, out = run(["diskutil", "list"], timeout=12)
    if ok:
        row("✅", "diskutil list", "可列出磁碟", "若失敗，App 也無法自動偵測磁碟。")
    elif is_diskmanagement_framework_error(out):
        row(
            "🟡",
            "diskutil list",
            out[-400:],
            "在部分 macOS/Codex Python 子行程會出現此限制；請在終端機直接執行 diskutil list 做實機確認。",
        )
    else:
        fail = True
        row("❌", "diskutil list", out[-400:], "若失敗，App 也無法自動偵測磁碟。")

    # Full Disk Access cannot be queried directly. We give a practical hint.
    row(
        "🟡",
        "完整磁碟取用權",
        "macOS 不提供可靠 CLI 查詢；若掃 /dev/disk* 出現 Operation not permitted / Permission denied，就需要開權限。",
        "系統設定 → 隱私權與安全性 → 完整磁碟取用權 → 加入此 App / Terminal / 開發環境。",
    )

    out_dir = Path.home() / "Desktop" / "已救援"
    try:
        out_dir.mkdir(parents=True, exist_ok=True)
        test = out_dir / ".polik_write_test"
        test.write_text("ok", encoding="utf-8")
        test.unlink(missing_ok=True)
        row("✅", "輸出資料夾", str(out_dir), "救回檔案仍建議存到另一顆磁碟。")
    except PermissionError:
        # Directory exists (possibly created by a privileged App run) but isn't writable
        # by the current user. This is a warning, not a hard failure — the App lets the
        # user choose any output directory at runtime.
        if out_dir.exists():
            row(
                "🟡",
                "輸出資料夾存在但不可寫",
                str(out_dir),
                "目錄可能由管理員身份建立。可用 sudo chown $USER 修正，或在 App 內改選可寫路徑。",
            )
        else:
            fail = True
            row("❌", "輸出資料夾無法建立", str(out_dir), "請確認桌面路徑存在且有寫入權限。")
    except Exception as exc:  # noqa: BLE001
        fail = True
        row("❌", "輸出資料夾不可寫", str(exc), "改選桌面、外接碟或有權限的資料夾。")

    ok, out = run(["hdiutil", "help"], timeout=10)
    row("✅" if ok else "🟡", "APFS 真實測試工具", "hdiutil 可用" if ok else out[-400:], "若要跑 make real-apfs-quality-gate，hdiutil 必須可用。")

    print("=" * 72)
    print("若要測 CLI：cargo build --release -p polik-recovery-cli")
    print("若要測真實 APFS：make real-apfs-quality-gate")
    print("若要測成像 map：make macos-image-map MANIFEST=/path/to/image.manifest.json")
    return 1 if fail else 0


if __name__ == "__main__":
    raise SystemExit(main())
