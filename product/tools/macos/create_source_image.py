#!/usr/bin/env python3
"""Create a read-only recovery image from a source disk/device on macOS.

This script is intentionally conservative. It requires GNU ddrescue/gddrescue
because failing disks should be imaged once, with a resumable map file, and then
scanned from that image. Missing prerequisites fail closed.
"""
from __future__ import annotations

import argparse
import datetime as dt
import hashlib
import html
import json
import os
import platform
import plistlib
import re
import shutil
import subprocess
import sys
from pathlib import Path
from typing import Any


def now_iso() -> str:
    return dt.datetime.now(dt.timezone.utc).astimezone().isoformat(timespec="seconds")


def slug(value: str) -> str:
    value = value.strip().replace("/dev/", "") or "source"
    return re.sub(r"[^A-Za-z0-9._-]+", "_", value).strip("._-") or "source"


def run(cmd: list[str], *, check: bool = False) -> subprocess.CompletedProcess[str]:
    return subprocess.run(cmd, text=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE, check=check)


def command_exists(name: str) -> bool:
    return shutil.which(name) is not None


def whole_disk_identifier(identifier: str) -> str | None:
    value = identifier.removeprefix("/dev/")
    match = re.fullmatch(r"(disk\d+)(?:s\d+)*", value)
    return match.group(1) if match else None


def physical_stores_from_info(info: dict[str, Any]) -> set[str]:
    stores: set[str] = set()

    def collect(value: Any) -> None:
        if isinstance(value, str):
            identifier = whole_disk_identifier(value)
            if identifier:
                stores.add(identifier)
        elif isinstance(value, dict):
            for nested in value.values():
                collect(nested)
        elif isinstance(value, list):
            for nested in value:
                collect(nested)

    collect(info.get("APFSPhysicalStores", []))
    if stores:
        return stores

    parent = info.get("ParentWholeDisk")
    if isinstance(parent, str):
        identifier = whole_disk_identifier(parent)
        if identifier:
            return {identifier}

    if info.get("WholeDisk") is True and isinstance(info.get("DeviceIdentifier"), str):
        identifier = whole_disk_identifier(info["DeviceIdentifier"])
        if identifier:
            return {identifier}

    return set()


def device_from_df_output(output: str) -> str:
    lines = [line for line in output.splitlines() if line.strip()]
    if len(lines) < 2:
        raise RuntimeError("df 未回傳可解析的掛載資訊。")
    fields = lines[-1].split()
    if not fields or not fields[0].startswith("/dev/"):
        raise RuntimeError("df 未回傳本機 block device；無法證明實體儲存裝置身分。")
    return fields[0]


def diskutil_lookup_target(target: str) -> str:
    if target.startswith("/dev/"):
        return target
    proc = subprocess.run(
        ["/bin/df", "-P", target],
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        check=False,
        text=True,
    )
    if proc.returncode != 0:
        detail = proc.stderr.strip()
        raise RuntimeError(f"df 無法解析 {target} 的掛載裝置：{detail}")
    return device_from_df_output(proc.stdout)


def diskutil_info(target: str) -> dict[str, Any]:
    if not command_exists("diskutil"):
        raise RuntimeError("找不到 diskutil，無法證明實體儲存裝置身分。")
    proc = subprocess.run(
        ["diskutil", "info", "-plist", target],
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        check=False,
    )
    if proc.returncode != 0:
        detail = proc.stderr.decode("utf-8", errors="replace").strip()
        raise RuntimeError(f"diskutil 無法解析 {target} 的儲存裝置身分：{detail}")
    try:
        value = plistlib.loads(proc.stdout)
    except plistlib.InvalidFileException as exc:
        raise RuntimeError(f"diskutil 回傳無效 plist：{target}") from exc
    if not isinstance(value, dict):
        raise RuntimeError(f"diskutil 未回傳可用的裝置資料：{target}")
    return value


def physical_stores_for_target(target: str) -> set[str]:
    info = diskutil_info(diskutil_lookup_target(target))
    stores = physical_stores_from_info(info)
    container = info.get("APFSContainerReference")
    if isinstance(container, str) and container:
        stores.update(physical_stores_from_info(diskutil_info(container)))
    if not stores:
        raise RuntimeError(f"無法證明 {target} 所在的實體儲存裝置；依安全政策停止。")
    return stores


def existing_ancestor(path: Path) -> Path:
    candidate = path
    while not candidate.exists():
        parent = candidate.parent
        if parent == candidate:
            raise RuntimeError(f"找不到輸出路徑的既有父目錄：{path}")
        candidate = parent
    return candidate


def validate_distinct_physical_storage(source: str, output_dir: Path) -> dict[str, list[str]]:
    source_target = source
    if not source.startswith("/dev/"):
        source_target = str(Path(source).expanduser().resolve(strict=True))
    output_target = str(existing_ancestor(output_dir).resolve(strict=True))
    source_stores = physical_stores_for_target(source_target)
    output_stores = physical_stores_for_target(output_target)
    shared = sorted(source_stores & output_stores)
    if shared:
        raise RuntimeError(
            "來源與輸出共用實體儲存裝置 "
            f"{', '.join(shared)}；為避免覆寫待救資料，成像已停止。"
        )
    return {
        "source_physical_stores": sorted(source_stores),
        "output_physical_stores": sorted(output_stores),
    }


def raw_device_path(source: str) -> str:
    if source.startswith("/dev/disk"):
        return source.replace("/dev/disk", "/dev/rdisk", 1)
    return source


def source_size_bytes(source: str) -> int | None:
    if source.startswith("/dev/") and command_exists("diskutil"):
        out = run(["diskutil", "info", source])
        text = out.stdout + "\n" + out.stderr
        for line in text.splitlines():
            t = line.strip()
            if t.startswith("Disk Size:") or t.startswith("Total Size:"):
                m = re.search(r"\((\d+)\s+Bytes\)", t)
                if m:
                    return int(m.group(1))
    try:
        return Path(source).stat().st_size
    except OSError:
        return None


def sha256_file(path: Path, chunk_size: int = 1024 * 1024 * 8) -> str:
    h = hashlib.sha256()
    with path.open("rb") as f:
        for chunk in iter(lambda: f.read(chunk_size), b""):
            h.update(chunk)
    return h.hexdigest()


def sample_sha256(path: Path, sample_size: int = 1024 * 1024) -> dict[str, Any]:
    size = path.stat().st_size
    ranges: list[tuple[str, int, int]] = []
    if size <= sample_size * 3:
        ranges.append(("full", 0, size))
    else:
        ranges.append(("head", 0, sample_size))
        mid = max(0, (size // 2) - (sample_size // 2))
        ranges.append(("middle", mid, sample_size))
        ranges.append(("tail", max(0, size - sample_size), min(sample_size, size)))
    result = {"size_bytes": size, "sample_size": sample_size, "ranges": []}
    with path.open("rb") as f:
        for label, offset, length in ranges:
            f.seek(offset)
            data = f.read(length)
            result["ranges"].append({
                "label": label,
                "offset": offset,
                "length": len(data),
                "sha256": hashlib.sha256(data).hexdigest(),
            })
    return result


def write_html_report(manifest: dict[str, Any], html_path: Path) -> None:
    rows = []
    for key, value in manifest.items():
        if isinstance(value, (dict, list)):
            value = json.dumps(value, ensure_ascii=False, indent=2)
        rows.append(
            f"<tr><th>{html.escape(str(key))}</th><td><pre>{html.escape(str(value))}</pre></td></tr>"
        )
    html_doc = f"""<!doctype html>
<html lang="zh-Hant"><head><meta charset="utf-8"><title>Polik Recovery Image Manifest</title>
<style>body{{font-family:-apple-system,BlinkMacSystemFont,'Noto Sans TC',sans-serif;line-height:1.55;margin:32px;background:#f8fafc;color:#1a202c}}table{{border-collapse:collapse;width:100%;background:#fff}}th,td{{border:1px solid #e2e8f0;padding:10px;text-align:left;vertical-align:top}}th{{width:220px;background:#edf2f7}}pre{{white-space:pre-wrap;margin:0;font-family:ui-monospace,SFMono-Regular,Menlo,monospace}}.warn{{background:#fffaf0;border:1px solid #f6ad55;padding:12px;border-radius:8px;margin-bottom:16px}}</style></head>
<body><h1>Polik Recovery Image Manifest</h1><div class="warn">資料救援建議流程：先對來源磁碟建立唯讀映像檔，再針對映像檔掃描與還原，避免反覆讀取快故障的原始磁碟。</div><table>{''.join(rows)}</table></body></html>"""
    html_path.write_text(html_doc, encoding="utf-8")


def unmount_source(source: str) -> dict[str, Any]:
    if not source.startswith("/dev/"):
        return {"attempted": False, "ok": True, "reason": "not a raw device"}
    if not command_exists("diskutil"):
        return {"attempted": False, "ok": False, "reason": "diskutil not found"}
    cmd = ["diskutil", "unmountDisk", source]
    out = run(cmd)
    return {
        "attempted": True,
        "ok": out.returncode == 0,
        "command": cmd,
        "stdout": out.stdout.strip(),
        "stderr": out.stderr.strip(),
    }


def imaging_command(source: str, image_path: Path, map_path: Path, method: str) -> tuple[list[str], str]:
    raw = raw_device_path(source)
    ddrescue = shutil.which("gddrescue") or shutil.which("ddrescue")
    if method in {"auto", "ddrescue"} and ddrescue:
        return ([ddrescue, "-f", "-n", raw, str(image_path), str(map_path)], "ddrescue")
    raise RuntimeError(
        "找不到 ddrescue/gddrescue；為避免對故障媒體造成額外讀取且遺失壞區紀錄，"
        "成像已停止。請先安裝 ddrescue，或交由專業資料復原服務處理。"
    )


def main() -> int:
    parser = argparse.ArgumentParser(description="Create a safe source image for Polik Recovery.")
    parser.add_argument("--source", help="來源，例如 /dev/disk4 或 /dev/disk4s1；續跑 manifest 時可省略")
    parser.add_argument("--out-dir", help="映像檔輸出資料夾，請使用另一顆磁碟；續跑 manifest 時可省略")
    parser.add_argument("--name", help="輸出檔名前綴；預設依來源與時間產生")
    parser.add_argument("--method", choices=["auto", "ddrescue"], default="auto")
    parser.add_argument("--full-sha256", action="store_true", help="建立完整 SHA-256；大磁碟會很久")
    parser.add_argument("--dry-run", action="store_true", help="只輸出計畫，不實際建立映像檔")
    parser.add_argument("--resume-manifest", help="使用既有 manifest / .map 續跑 ddrescue，不重新產生檔名")
    parser.add_argument("--retry-bad", action="store_true", help="ddrescue 針對壞區重試；建議搭配 --retries")
    parser.add_argument("--retries", type=int, default=3, help="--retry-bad 時的重試次數，預設 3")
    parser.add_argument("--reverse", action="store_true", help="ddrescue 反向讀取，常用於壞區 retry")
    parser.add_argument("--direct", action="store_true", help="ddrescue 使用 direct I/O，部分外接盒可能更穩，部分可能更慢")
    args = parser.parse_args()

    resume_manifest: dict[str, Any] | None = None
    resume_manifest_path: Path | None = None
    if args.resume_manifest:
        resume_manifest_path = Path(args.resume_manifest).expanduser().resolve()
        if not resume_manifest_path.exists():
            print(f"resume manifest not found: {resume_manifest_path}", file=sys.stderr)
            return 2
        resume_manifest = json.loads(resume_manifest_path.read_text(encoding="utf-8"))
        args.source = args.source or resume_manifest.get("source")
        if not args.out_dir:
            image_from_manifest = Path(str(resume_manifest.get("output_image", ""))).expanduser()
            args.out_dir = str(image_from_manifest.parent if str(image_from_manifest) else resume_manifest_path.parent)
        if args.name is None and resume_manifest.get("output_image"):
            args.name = Path(str(resume_manifest["output_image"])).stem

    if not args.source or not args.out_dir:
        print("--source and --out-dir are required unless --resume-manifest supplies them.", file=sys.stderr)
        return 2

    warnings: list[str] = []
    if args.resume_manifest:
        warnings.append("續跑模式：會沿用既有 .img 與 .map，ddrescue 會根據 map 跳過已完成區段。")
    if platform.system() != "Darwin":
        warnings.append("這個工具設計給 macOS；其他平台請先改用 CLI 或專業 imaging 工具。")
    if args.source.startswith("/dev/disk"):
        warnings.append("將使用 /dev/rdisk* 讀取以提升速度；請確認來源磁碟已停止使用。")
    out_dir = Path(args.out_dir).expanduser().resolve()
    try:
        storage_identity = validate_distinct_physical_storage(args.source, out_dir)
    except (OSError, RuntimeError) as exc:
        print(str(exc), file=sys.stderr)
        return 2
    out_dir.mkdir(parents=True, exist_ok=True)
    stamp = dt.datetime.now().strftime("%Y%m%d-%H%M%S")
    if resume_manifest and resume_manifest.get("output_image"):
        image_path = Path(str(resume_manifest["output_image"])).expanduser().resolve()
        prefix = image_path.stem
        map_path = Path(str(resume_manifest.get("map_file") or image_path.with_suffix(".map"))).expanduser().resolve()
        manifest_path = resume_manifest_path or out_dir / f"{prefix}.manifest.json"
        html_path = Path(str(resume_manifest.get("html_report") or manifest_path.with_suffix(".manifest.html"))).expanduser().resolve()
    else:
        prefix = slug(args.name or f"polik-image-{slug(args.source)}-{stamp}")
        image_path = out_dir / f"{prefix}.img"
        map_path = out_dir / f"{prefix}.map"
        manifest_path = out_dir / f"{prefix}.manifest.json"
        html_path = out_dir / f"{prefix}.manifest.html"
    map_report_json_path = manifest_path.with_suffix(".map_report.json")
    map_report_html_path = manifest_path.with_suffix(".map_report.html")

    try:
        cmd, resolved_method = imaging_command(args.source, image_path, map_path, args.method)
    except RuntimeError as exc:
        print(str(exc), file=sys.stderr)
        return 2

    size_before = source_size_bytes(args.source)
    unmount = {"attempted": False, "ok": True, "reason": "dry-run"}
    if not args.dry_run:
        unmount = unmount_source(args.source)
        if not unmount.get("ok"):
            print(
                "來源磁碟無法正常卸載；成像已停止。請關閉正在使用該磁碟的 App 後重試，"
                "不要 force-unmount。",
                file=sys.stderr,
            )
            return 2
    if resolved_method == "ddrescue":
        # Resume is automatic when the same image/map are reused. Retry options are intentionally
        # explicit so users do not over-stress failing media accidentally.
        if args.retry_bad:
            cmd = [cmd[0], "-f", f"-r{max(0, args.retries)}", raw_device_path(args.source), str(image_path), str(map_path)]
        if args.reverse:
            cmd.insert(1, "-R")
        if args.direct:
            cmd.insert(1, "-d")

    manifest: dict[str, Any] = {
        "schema": "polik.recovery.source_image_manifest.v1",
        "created_at": now_iso(),
        "source": args.source,
        "raw_source_used": raw_device_path(args.source),
        "source_size_bytes_before": size_before,
        **storage_identity,
        "output_image": str(image_path),
        "map_file": str(map_path) if resolved_method == "ddrescue" else None,
        "manifest": str(manifest_path),
        "html_report": str(html_path),
        "map_report_json": str(map_report_json_path) if resolved_method == "ddrescue" else None,
        "map_report_html": str(map_report_html_path) if resolved_method == "ddrescue" else None,
        "resume_manifest": str(resume_manifest_path) if resume_manifest_path else None,
        "retry_bad": bool(args.retry_bad),
        "retries": int(args.retries),
        "reverse": bool(args.reverse),
        "direct": bool(args.direct),
        "method": resolved_method,
        "command": cmd,
        "unmount": unmount,
        "dry_run": args.dry_run,
        "warnings": warnings,
        "status": "planned" if args.dry_run else "running",
    }

    if args.dry_run:
        print(json.dumps(manifest, ensure_ascii=False, indent=2))
        return 0

    started = now_iso()
    proc = subprocess.Popen(cmd)
    return_code = proc.wait()
    finished = now_iso()

    image_exists = image_path.exists()
    image_size = image_path.stat().st_size if image_exists else 0
    manifest.update({
        "started_at": started,
        "finished_at": finished,
        "return_code": return_code,
        "image_exists": image_exists,
        "image_size_bytes": image_size,
        "status": "complete" if return_code == 0 and image_exists and image_size > 0 else "failed_or_partial",
    })
    if size_before and image_size and image_size < size_before:
        warnings.append("映像檔大小小於來源大小；可能是讀取中斷或部分成像，請優先用 ddrescue map 續跑。")
    if image_exists and image_size > 0:
        manifest["sample_sha256"] = sample_sha256(image_path)
        if args.full_sha256:
            manifest["sha256"] = sha256_file(image_path)
    manifest["warnings"] = warnings
    manifest_path.write_text(json.dumps(manifest, ensure_ascii=False, indent=2), encoding="utf-8")
    write_html_report(manifest, html_path)
    if resolved_method == "ddrescue" and map_path.exists():
        map_tool = Path(__file__).with_name("ddrescue_map_report.py")
        if map_tool.exists():
            map_proc = run([sys.executable, str(map_tool), "--manifest", str(manifest_path), "--out", str(map_report_json_path), "--html", str(map_report_html_path)])
            manifest["map_report_return_code"] = map_proc.returncode
            manifest["map_report_stdout_tail"] = map_proc.stdout[-2000:]
            manifest["map_report_stderr_tail"] = map_proc.stderr[-2000:]
            manifest_path.write_text(json.dumps(manifest, ensure_ascii=False, indent=2), encoding="utf-8")
            write_html_report(manifest, html_path)
    print(json.dumps(manifest, ensure_ascii=False, indent=2))
    return 0 if manifest["status"] == "complete" else 2


if __name__ == "__main__":
    raise SystemExit(main())
