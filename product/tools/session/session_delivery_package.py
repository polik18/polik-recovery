#!/usr/bin/env python3
"""Build a customer-safe delivery package for a Polik Recovery session.

v2.7 separates customer deliverables from engineering artifacts. The delivery ZIP is
intentionally conservative: it includes customer reports, audit records, file inventory,
checksums, and recovered files, while excluding raw device/image files, ddrescue maps,
engineering logs, and absolute local paths unless explicitly requested.
"""
from __future__ import annotations

import argparse
import csv
import datetime as dt
import hashlib
import html
import json
import os
import re
import shutil
import zipfile
from pathlib import Path
from typing import Any, Iterable

try:
    from session_timeline import append_event as append_timeline_event
except Exception:  # pragma: no cover
    append_timeline_event = None

try:
    from session_dashboard import write_dashboard as write_session_dashboard
except Exception:  # pragma: no cover
    write_session_dashboard = None

DELIVERY_DIR = "11_delivery"
PACKAGE_DIR = "packages"
STAGING_DIR = "staging"
SCHEMA = "polik.recovery.delivery_package.v2.7"
DELIVERY_MANIFEST = "delivery_manifest.json"
DELIVERY_HTML = "delivery_manifest.html"
FILE_INVENTORY_JSON = "file_inventory.json"
FILE_INVENTORY_CSV = "file_inventory.csv"
SHA256SUMS = "SHA256SUMS.txt"
README = "README_交付說明.md"

DEFAULT_REPORT_FILES = [
    "09_reports/customer_summary.pdf",
    "09_reports/customer_summary.html",
    "09_reports/customer_summary.md",
    "09_reports/customer_summary.json",
    "10_audit/sample_audit.html",
    "10_audit/sample_audit.csv",
    "10_audit/sample_audit.md",
    "10_audit/sample_audit.json",
]

RECOVERY_GROUPS = [
    ("complete", "03_recovery_output/recovered", "recovered"),
    ("possible", "03_recovery_output/suspect/possible", "suspect/possible"),
    ("partial", "03_recovery_output/suspect/partial", "suspect/partial"),
    ("fragment", "03_recovery_output/suspect/fragments", "suspect/fragments"),
]

SENSITIVE_NAMES = {
    "engineer_handoff.html", "engineer_handoff.json", "engineer_handoff.md", "engineer_handoff.pdf",
}


def now_iso() -> str:
    return dt.datetime.now(dt.timezone.utc).astimezone().isoformat(timespec="seconds")


def stamp() -> str:
    return dt.datetime.now().strftime("%Y%m%d-%H%M%S")


def slug(value: str) -> str:
    value = value.strip() or "case"
    value = value.replace("/dev/", "")
    value = re.sub(r"[^A-Za-z0-9._\-\u4e00-\u9fff]+", "_", value)
    return value.strip("._-") or "case"


def human_bytes(n: Any) -> str:
    try:
        value = float(n or 0)
    except Exception:
        value = 0.0
    for unit in ["B", "KB", "MB", "GB", "TB"]:
        if abs(value) < 1024 or unit == "TB":
            return f"{value:.0f} {unit}" if unit == "B" else f"{value:.1f} {unit}"
        value /= 1024
    return f"{value:.1f} TB"


def load_json(path: Path) -> Any | None:
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except Exception:
        return None


def write_json(path: Path, payload: Any) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(payload, ensure_ascii=False, indent=2), encoding="utf-8")


def sha256_file(path: Path) -> str:
    h = hashlib.sha256()
    with path.open("rb") as f:
        for chunk in iter(lambda: f.read(1024 * 1024), b""):
            h.update(chunk)
    return h.hexdigest()


def rel(path: Path, base: Path) -> str:
    try:
        return str(path.resolve().relative_to(base.resolve()))
    except Exception:
        return str(path)


def redact_text(value: Any) -> Any:
    if isinstance(value, dict):
        return {k: redact_text(v) for k, v in value.items()}
    if isinstance(value, list):
        return [redact_text(v) for v in value]
    if not isinstance(value, str):
        return value
    text = value
    text = re.sub(r"/dev/(?:r?disk\w+|rdisk\w+)", "[redacted-device]", text)
    text = re.sub(r"/Users/[^/\s]+", "/Users/[redacted]", text)
    text = re.sub(r"/Volumes/[^/\s]+", "/Volumes/[redacted]", text)
    return text


def ensure_session(session: Path) -> dict[str, Any]:
    manifest_path = session / "session_manifest.json"
    if not session.is_dir():
        raise SystemExit(f"Session folder not found: {session}")
    if not manifest_path.is_file():
        raise SystemExit(f"Missing session_manifest.json: {manifest_path}")
    manifest = load_json(manifest_path)
    if not isinstance(manifest, dict):
        raise SystemExit(f"session_manifest.json is not a JSON object: {manifest_path}")
    return manifest


def clean_dir(path: Path) -> None:
    if path.exists():
        shutil.rmtree(path)
    path.mkdir(parents=True, exist_ok=True)


def safe_copy_file(src: Path, dest: Path) -> None:
    dest.parent.mkdir(parents=True, exist_ok=True)
    if dest.exists():
        stem = dest.stem
        suffix = dest.suffix
        dest = dest.with_name(f"{stem}_{stamp()}{suffix}")
    shutil.copy2(src, dest)


def inventory_row(path: Path, package_root: Path, source_rel: str, category: str, *, note: str = "") -> dict[str, Any]:
    st = path.stat()
    return {
        "category": category,
        "package_path": rel(path, package_root),
        "source_relative": source_rel,
        "file_name": path.name,
        "extension": path.suffix.lower().lstrip("."),
        "size_bytes": st.st_size,
        "size_human": human_bytes(st.st_size),
        "sha256": sha256_file(path),
        "note": note,
    }


def copy_reports(session: Path, package_root: Path, inventory: list[dict[str, Any]], warnings: list[str]) -> None:
    for rel_path in DEFAULT_REPORT_FILES:
        src = session / rel_path
        if not src.is_file():
            warnings.append(f"缺少可選交付檔：{rel_path}")
            continue
        if src.name in SENSITIVE_NAMES:
            continue
        dest = package_root / rel_path
        safe_copy_file(src, dest)
        inventory.append(inventory_row(dest, package_root, rel_path, "report"))


def iter_files(folder: Path) -> Iterable[Path]:
    if not folder.exists():
        return []
    return sorted([p for p in folder.glob("**/*") if p.is_file()], key=lambda p: str(p).lower())


def include_group(category: str, args: argparse.Namespace) -> bool:
    if category == "complete":
        return not args.no_recovered
    if args.include_all_suspect:
        return category in {"possible", "partial", "fragment"}
    if args.include_possible and category == "possible":
        return True
    if args.include_partial and category == "partial":
        return True
    if args.include_fragments and category == "fragment":
        return True
    return False


def copy_recovered_files(session: Path, package_root: Path, inventory: list[dict[str, Any]], warnings: list[str], args: argparse.Namespace) -> dict[str, int]:
    counts = {"complete": 0, "possible": 0, "partial": 0, "fragment": 0}
    for category, source_rel, dest_rel in RECOVERY_GROUPS:
        if not include_group(category, args):
            continue
        src_root = session / source_rel
        if not src_root.exists():
            warnings.append(f"找不到還原資料夾：{source_rel}")
            continue
        for src in iter_files(src_root):
            src_rel = rel(src, src_root)
            dest = package_root / dest_rel / src_rel
            safe_copy_file(src, dest)
            note = "完整區檔案" if category == "complete" else "可疑/部分救援檔案，交付前請確認客戶是否需要。"
            inventory.append(inventory_row(dest, package_root, f"{source_rel}/{src_rel}", category, note=note))
            counts[category] += 1
    return counts


def write_inventory(package_root: Path, inventory: list[dict[str, Any]]) -> None:
    write_json(package_root / FILE_INVENTORY_JSON, {"schema": "polik.recovery.delivery_inventory.v2.7", "generated_at": now_iso(), "file_count": len(inventory), "files": inventory})
    with (package_root / FILE_INVENTORY_CSV).open("w", encoding="utf-8-sig", newline="") as f:
        writer = csv.DictWriter(f, fieldnames=["category", "package_path", "source_relative", "file_name", "extension", "size_bytes", "size_human", "sha256", "note"])
        writer.writeheader()
        for row in inventory:
            writer.writerow(row)
    lines = [f"{row['sha256']}  {row['package_path']}" for row in inventory]
    (package_root / SHA256SUMS).write_text("\n".join(lines) + ("\n" if lines else ""), encoding="utf-8")


def write_readme(package_root: Path, payload: dict[str, Any]) -> None:
    counts = payload.get("included_counts", {})
    text = f"""# Polik Recovery 客戶交付包

此壓縮檔是 Polik Recovery Session v2.7 產生的「客戶交付包」。它只包含適合交付的摘要、抽查紀錄與救回檔案，不包含工程 log、raw device 路徑、ddrescue map 或原始磁碟 image。

## 案件

- Case ID：`{payload.get('case_id','')}`
- 產生時間：`{payload.get('generated_at','')}`
- Package ID：`{payload.get('package_id','')}`

## 內容

- `09_reports/customer_summary.pdf`：客戶版 PDF 摘要
- `09_reports/customer_summary.html`：客戶版 HTML 摘要
- `10_audit/sample_audit.*`：人工抽查紀錄
- `file_inventory.csv/json`：本交付包檔案清單與 SHA256
- `SHA256SUMS.txt`：交付包內檔案雜湊值
- `recovered/`：完整區救回檔案
- `suspect/`：可疑/partial 檔案，只有在操作者選擇納入時才會出現

## 本次納入數量

- 完整區：{counts.get('complete', 0)} 個
- 可疑 possible：{counts.get('possible', 0)} 個
- partial：{counts.get('partial', 0)} 個
- fragments：{counts.get('fragment', 0)} 個
- package 內總檔案：{payload.get('package_file_count', 0)} 個

## 重要提醒

1. `recovered/` 代表自動分流中的完整區，但仍建議抽查確認。
2. `suspect/` 代表可疑、部分或低信心結果，不等於完整成功。
3. 若需要工程 log、raw image、map report 或完整 Session，請另向工程人員索取，不應放在客戶交付包。
"""
    (package_root / README).write_text(text, encoding="utf-8")


def render_manifest_html(payload: dict[str, Any]) -> str:
    counts = payload.get("included_counts", {})
    warnings = payload.get("warnings", [])
    rows = "".join(f"<tr><th>{html.escape(str(k))}</th><td><pre>{html.escape(json.dumps(v, ensure_ascii=False, indent=2) if isinstance(v, (dict, list)) else str(v))}</pre></td></tr>" for k, v in payload.items() if k not in {"warnings"})
    warn_html = "".join(f"<li>{html.escape(str(w))}</li>" for w in warnings) or "<li>無</li>"
    return f"""<!doctype html><html lang="zh-Hant"><head><meta charset="utf-8"><title>Polik Delivery Package</title>
<style>body{{font-family:-apple-system,BlinkMacSystemFont,'Noto Sans TC',sans-serif;background:#f8fafc;color:#172033;line-height:1.65;margin:32px}}.card{{background:white;border:1px solid #e5e7eb;border-radius:16px;padding:18px;margin:14px 0;box-shadow:0 8px 22px rgba(15,23,42,.06)}}table{{border-collapse:collapse;width:100%}}th,td{{border:1px solid #e5e7eb;padding:10px;text-align:left;vertical-align:top}}th{{background:#f1f5f9;width:220px}}pre,code{{font-family:ui-monospace,SFMono-Regular,Menlo,monospace;white-space:pre-wrap}}.pill{{display:inline-block;background:#e0f2fe;border-radius:999px;padding:4px 10px;margin:3px}}</style></head><body>
<h1>Polik Recovery 客戶交付包</h1><p>Package：<code>{html.escape(str(payload.get('package_id','')))}</code></p>
<div class="card"><span class="pill">完整 {counts.get('complete',0)}</span><span class="pill">possible {counts.get('possible',0)}</span><span class="pill">partial {counts.get('partial',0)}</span><span class="pill">fragments {counts.get('fragment',0)}</span><span class="pill">總檔案 {payload.get('package_file_count',0)}</span></div>
<div class="card"><h2>交付包資訊</h2><table>{rows}</table></div>
<div class="card"><h2>警告 / 缺少項目</h2><ul>{warn_html}</ul></div>
</body></html>"""


def make_zip(package_root: Path, zip_path: Path) -> None:
    if zip_path.exists():
        zip_path.unlink()
    zip_path.parent.mkdir(parents=True, exist_ok=True)
    with zipfile.ZipFile(zip_path, "w", compression=zipfile.ZIP_DEFLATED, compresslevel=6) as zf:
        for path in sorted(package_root.glob("**/*")):
            if path.is_file():
                zf.write(path, arcname=str(path.relative_to(package_root.parent)))


def update_session_manifest(session: Path, payload: dict[str, Any]) -> None:
    path = session / "session_manifest.json"
    manifest = load_json(path)
    if not isinstance(manifest, dict):
        return
    manifest.setdefault("folders", {})["delivery"] = str(session / DELIVERY_DIR)
    packages = manifest.get("delivery_packages")
    if not isinstance(packages, list):
        packages = []
    packages.append({
        "package_id": payload.get("package_id"),
        "created_at": payload.get("generated_at"),
        "zip_path": payload.get("zip_path"),
        "manifest_path": payload.get("delivery_manifest_path"),
        "included_counts": payload.get("included_counts"),
        "package_file_count": payload.get("package_file_count"),
        "customer_safe": True,
    })
    manifest["delivery_packages"] = packages[-20:]
    manifest["updated_at"] = now_iso()
    manifest["version"] = "2.7-delivery-package"
    write_json(path, manifest)


def build_package(session: Path, args: argparse.Namespace) -> dict[str, Any]:
    manifest = ensure_session(session)
    case_id = str(manifest.get("case_id") or session.name)
    package_id = args.package_id or f"delivery-{slug(case_id)}-{stamp()}"
    delivery_root = session / DELIVERY_DIR
    staging_root = delivery_root / STAGING_DIR
    packages_root = delivery_root / PACKAGE_DIR
    package_root = staging_root / package_id
    if args.dry_run:
        package_root = staging_root / f"{package_id}-dry-run"
    clean_dir(package_root)

    inventory: list[dict[str, Any]] = []
    warnings: list[str] = []
    copy_reports(session, package_root, inventory, warnings)
    counts = copy_recovered_files(session, package_root, inventory, warnings, args)
    if counts.get("complete", 0) == 0:
        warnings.append("交付包沒有包含 recovered/ 完整區檔案；請確認是否尚未還原或輸出資料夾不同。")
    if not (package_root / "09_reports" / "customer_summary.pdf").is_file():
        warnings.append("交付包缺少 customer_summary.pdf；建議先執行 make session-report-pdf。")
    if not (package_root / "10_audit" / "sample_audit.json").is_file():
        warnings.append("交付包缺少 sample_audit.json；建議先執行 make session-audit。")

    payload: dict[str, Any] = {
        "schema": SCHEMA,
        "package_id": package_id,
        "generated_at": now_iso(),
        "case_id": case_id,
        "customer_safe": True,
        "redaction": "absolute local paths, raw device paths, logs, raw image files and engineering-only reports are excluded/redacted by default",
        "include_suspect": bool(args.include_all_suspect or args.include_possible or args.include_partial or args.include_fragments),
        "included_counts": counts,
        "package_file_count": len(inventory),
        "package_total_bytes": sum(int(row.get("size_bytes", 0) or 0) for row in inventory),
        "package_total_human": human_bytes(sum(int(row.get("size_bytes", 0) or 0) for row in inventory)),
        "warnings": warnings,
        "source_session": redact_text(str(session)),
        "delivery_root": redact_text(str(delivery_root)),
        "package_root": redact_text(str(package_root)),
        "zip_path": redact_text(str(packages_root / f"{package_id}.zip")),
        "delivery_manifest_path": redact_text(str(package_root / DELIVERY_MANIFEST)),
    }
    if args.include_paths:
        payload["source_session"] = str(session)
        payload["delivery_root"] = str(delivery_root)
        payload["package_root"] = str(package_root)
        payload["zip_path"] = str(packages_root / f"{package_id}.zip")
        payload["delivery_manifest_path"] = str(package_root / DELIVERY_MANIFEST)

    write_readme(package_root, payload)
    inventory.append(inventory_row(package_root / README, package_root, README, "delivery_metadata"))
    payload["package_file_count"] = len(inventory)
    payload["package_total_bytes"] = sum(int(row.get("size_bytes", 0) or 0) for row in inventory)
    payload["package_total_human"] = human_bytes(payload["package_total_bytes"])
    write_json(package_root / DELIVERY_MANIFEST, payload)
    (package_root / DELIVERY_HTML).write_text(render_manifest_html(payload), encoding="utf-8")
    for meta_file in [DELIVERY_MANIFEST, DELIVERY_HTML]:
        p = package_root / meta_file
        if p.is_file() and not any(row["package_path"] == meta_file for row in inventory):
            inventory.append(inventory_row(p, package_root, meta_file, "delivery_metadata"))
    # Inventory intentionally excludes file_inventory.* and SHA256SUMS.txt themselves to avoid self-referential checksum drift.
    payload["package_file_count"] = len(inventory)
    payload["package_total_bytes"] = sum(int(row.get("size_bytes", 0) or 0) for row in inventory)
    payload["package_total_human"] = human_bytes(payload["package_total_bytes"])
    write_json(package_root / DELIVERY_MANIFEST, payload)
    (package_root / DELIVERY_HTML).write_text(render_manifest_html(payload), encoding="utf-8")
    write_inventory(package_root, inventory)

    zip_path = packages_root / f"{package_id}.zip"
    if not args.dry_run:
        make_zip(package_root, zip_path)
        payload["zip_path"] = str(zip_path) if args.include_paths else redact_text(str(zip_path))
        payload["zip_size_bytes"] = zip_path.stat().st_size
        payload["zip_size_human"] = human_bytes(zip_path.stat().st_size)
        write_json(package_root / DELIVERY_MANIFEST, payload)
        (package_root / DELIVERY_HTML).write_text(render_manifest_html(payload), encoding="utf-8")
        update_session_manifest(session, payload | {"zip_path": str(zip_path), "delivery_manifest_path": str(package_root / DELIVERY_MANIFEST)})
        if append_timeline_event:
            try:
                append_timeline_event(
                    session,
                    event_type="delivery_package_created",
                    title="產生客戶交付 ZIP",
                    status="success",
                    summary=f"已建立客戶交付包：{zip_path.name}，完整檔 {counts.get('complete', 0)} 個。",
                    actor="session_delivery_package.py",
                    artifacts=[str(zip_path), str(package_root / DELIVERY_MANIFEST)],
                    tags=["delivery", "customer_safe"],
                    details={"package_id": package_id, "included_counts": counts, "warnings": warnings},
                )
            except Exception:
                pass
        if write_session_dashboard:
            try:
                write_session_dashboard(session)
            except Exception:
                pass
    print(json.dumps({"ok": True, "dry_run": bool(args.dry_run), "package_id": package_id, "package_root": str(package_root), "zip_path": str(zip_path), "included_counts": counts, "warnings": warnings}, ensure_ascii=False, indent=2))
    return payload


def main() -> int:
    ap = argparse.ArgumentParser(description="Build a customer-safe delivery ZIP from a Polik Recovery session.")
    ap.add_argument("--session", type=Path, required=True, help="Session folder")
    ap.add_argument("--package-id", help="Override package id/name")
    ap.add_argument("--include-possible", action="store_true", help="Include suspect/possible files")
    ap.add_argument("--include-partial", action="store_true", help="Include suspect/partial files")
    ap.add_argument("--include-fragments", action="store_true", help="Include suspect/fragments files")
    ap.add_argument("--include-all-suspect", action="store_true", help="Include all suspect folders")
    ap.add_argument("--no-recovered", action="store_true", help="Do not include recovered/ files; useful for report-only packages")
    ap.add_argument("--include-paths", action="store_true", help="Keep absolute paths in delivery manifest. Not recommended for customer delivery.")
    ap.add_argument("--dry-run", action="store_true", help="Create staging folder but do not zip or update session manifest")
    args = ap.parse_args()
    session = args.session.expanduser().resolve()
    build_package(session, args)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
