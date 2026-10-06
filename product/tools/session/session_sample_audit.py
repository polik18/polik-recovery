#!/usr/bin/env python3
"""Create and maintain manual sample-audit records for a Polik Recovery session.

v2.6 adds a human verification layer. File carving and filesystem metadata can say a
file is structurally plausible, but delivery quality still depends on opening a sample
set and recording whether the files actually work.

This tool intentionally does not try to open the recovered files automatically. It
creates a review list, stores hashes/sizes, and lets the operator record manual results.
"""
from __future__ import annotations

import argparse
import csv
import datetime as dt
import hashlib
import html
import json
import mimetypes
from pathlib import Path
from typing import Any

AUDIT_DIR = "10_audit"
AUDIT_JSON = "sample_audit.json"
AUDIT_HTML = "sample_audit.html"
AUDIT_MD = "sample_audit.md"
AUDIT_CSV = "sample_audit.csv"
SCHEMA = "polik.recovery.sample_audit.v2.6"
VALID_RESULTS = {
    "pending": "尚未抽查",
    "opens_ok": "可正常開啟",
    "opens_with_warning": "可開啟但有警告/局部損毀",
    "cannot_open": "無法開啟",
    "wrong_type": "副檔名或檔案類型疑似錯誤",
    "duplicate": "疑似重複檔案",
    "not_needed": "不需抽查/排除",
}
CATEGORY_DIRS = [
    ("complete", "03_recovery_output/recovered"),
    ("possible", "03_recovery_output/suspect/possible"),
    ("partial", "03_recovery_output/suspect/partial"),
    ("fragment", "03_recovery_output/suspect/fragments"),
]


def now_iso() -> str:
    return dt.datetime.now(dt.timezone.utc).astimezone().isoformat(timespec="seconds")


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


def sha256_file(path: Path, limit: int | None = None) -> str:
    h = hashlib.sha256()
    remaining = limit
    with path.open("rb") as f:
        while True:
            size = 1024 * 1024 if remaining is None else min(1024 * 1024, remaining)
            if size <= 0:
                break
            chunk = f.read(size)
            if not chunk:
                break
            h.update(chunk)
            if remaining is not None:
                remaining -= len(chunk)
    return h.hexdigest()


def rel(path: Path, session: Path) -> str:
    try:
        return str(path.resolve().relative_to(session.resolve()))
    except Exception:
        return str(path)


def load_json(path: Path) -> Any | None:
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except Exception:
        return None


def write_json(path: Path, payload: Any) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(payload, ensure_ascii=False, indent=2), encoding="utf-8")


def mime_guess(path: Path) -> str:
    guessed, _ = mimetypes.guess_type(str(path))
    return guessed or "application/octet-stream"


def existing_items_by_rel(payload: dict[str, Any]) -> dict[str, dict[str, Any]]:
    out: dict[str, dict[str, Any]] = {}
    for item in payload.get("items", []):
        if isinstance(item, dict):
            key = str(item.get("relative_path") or item.get("path") or "")
            if key:
                out[key] = item
    return out


def collect_candidates(session: Path, sample_size: int) -> list[dict[str, Any]]:
    items: list[dict[str, Any]] = []
    for category, folder in CATEGORY_DIRS:
        base = session / folder
        files = sorted([p for p in base.glob("**/*") if p.is_file()], key=lambda p: (p.stat().st_size if p.exists() else 0, str(p))) if base.exists() else []
        if not files:
            continue
        # Blend small/medium/large files without trying to be too clever.
        if len(files) <= sample_size:
            selected = files
        else:
            selected = []
            for idx in range(sample_size):
                pos = round(idx * (len(files) - 1) / max(sample_size - 1, 1))
                selected.append(files[pos])
        for p in selected:
            try:
                st = p.stat()
            except OSError:
                continue
            items.append({
                "id": f"audit_{len(items)+1:04d}",
                "category": category,
                "relative_path": rel(p, session),
                "file_name": p.name,
                "extension": p.suffix.lower().lstrip("."),
                "mime_guess": mime_guess(p),
                "size_bytes": st.st_size,
                "size_human": human_bytes(st.st_size),
                "sha256": sha256_file(p),
                "sample_reason": category_reason(category),
                "result": "pending",
                "result_label": VALID_RESULTS["pending"],
                "checked_by": "",
                "checked_at": "",
                "notes": "",
            })
    return items


def category_reason(category: str) -> str:
    return {
        "complete": "完整區抽查：確認可交付檔案真的能開啟。",
        "possible": "可疑區抽查：確認是否可修復或可局部使用。",
        "partial": "partial 區抽查：確認部分救援檔案是否仍有價值。",
        "fragment": "低信心碎片抽查：確認是否應排除或保留。",
    }.get(category, "抽查樣本。")


def init_or_update_audit(session: Path, sample_size: int, keep_existing: bool = True) -> dict[str, Any]:
    audit_path = session / AUDIT_DIR / AUDIT_JSON
    existing = load_json(audit_path)
    existing = existing if isinstance(existing, dict) else {}
    old_by_rel = existing_items_by_rel(existing) if keep_existing else {}
    candidates = collect_candidates(session, sample_size)
    merged: list[dict[str, Any]] = []
    for item in candidates:
        old = old_by_rel.get(item["relative_path"])
        if old:
            # Preserve human review fields but refresh file metadata.
            for field in ["result", "result_label", "checked_by", "checked_at", "notes"]:
                item[field] = old.get(field, item.get(field, ""))
        merged.append(item)
    payload = {
        "schema": SCHEMA,
        "generated_at": now_iso(),
        "session_dir": str(session),
        "sample_size_per_category": sample_size,
        "summary": summarize_items(merged),
        "items": merged,
        "manual_result_values": VALID_RESULTS,
        "notes": [
            "抽查紀錄是人工驗證層，不等於自動修復。",
            "pending 表示尚未人工開啟確認；customer PDF 會提醒尚未抽查。",
            "完整區也需要抽查，避免把結構通過但實際打不開的檔案交付出去。",
        ],
    }
    write_outputs(session, payload)
    update_manifest(session, payload, "sample_audit_created", f"建立/更新抽查紀錄，共 {len(merged)} 筆樣本。")
    return payload


def add_or_update_result(session: Path, file_path: str, result: str, checked_by: str, notes: str) -> dict[str, Any]:
    if result not in VALID_RESULTS:
        raise SystemExit(f"Invalid result: {result}. Valid values: {', '.join(VALID_RESULTS)}")
    audit_path = session / AUDIT_DIR / AUDIT_JSON
    payload = load_json(audit_path)
    if not isinstance(payload, dict):
        payload = init_or_update_audit(session, sample_size=5)
    target = Path(file_path).expanduser()
    if target.exists():
        target_rel = rel(target, session)
    else:
        target_rel = file_path
    found = False
    for item in payload.get("items", []):
        if not isinstance(item, dict):
            continue
        if item.get("relative_path") == target_rel or item.get("file_name") == target_rel or item.get("relative_path", "").endswith(target_rel):
            item["result"] = result
            item["result_label"] = VALID_RESULTS[result]
            item["checked_by"] = checked_by
            item["checked_at"] = now_iso()
            item["notes"] = notes
            found = True
            break
    if not found:
        p = target if target.exists() else session / target_rel
        category = "manual"
        item = {
            "id": f"audit_{len(payload.get('items', []))+1:04d}",
            "category": category,
            "relative_path": rel(p, session) if p.exists() else target_rel,
            "file_name": p.name if p.exists() else Path(target_rel).name,
            "extension": p.suffix.lower().lstrip(".") if p.exists() else Path(target_rel).suffix.lower().lstrip("."),
            "mime_guess": mime_guess(p) if p.exists() else "application/octet-stream",
            "size_bytes": p.stat().st_size if p.exists() else 0,
            "size_human": human_bytes(p.stat().st_size) if p.exists() else "0 B",
            "sha256": sha256_file(p) if p.exists() else "",
            "sample_reason": "手動新增抽查項目。",
            "result": result,
            "result_label": VALID_RESULTS[result],
            "checked_by": checked_by,
            "checked_at": now_iso(),
            "notes": notes,
        }
        payload.setdefault("items", []).append(item)
    payload["generated_at"] = now_iso()
    payload["summary"] = summarize_items([i for i in payload.get("items", []) if isinstance(i, dict)])
    write_outputs(session, payload)
    update_manifest(session, payload, "sample_audit_updated", f"更新抽查結果：{target_rel} -> {VALID_RESULTS[result]}")
    return payload


def summarize_items(items: list[dict[str, Any]]) -> dict[str, Any]:
    summary: dict[str, Any] = {"total": len(items), "pending": 0, "opens_ok": 0, "warning": 0, "failed": 0, "by_category": {}, "by_result": {}}
    for item in items:
        cat = str(item.get("category") or "unknown")
        res = str(item.get("result") or "pending")
        summary["by_category"][cat] = summary["by_category"].get(cat, 0) + 1
        summary["by_result"][res] = summary["by_result"].get(res, 0) + 1
        if res == "pending":
            summary["pending"] += 1
        elif res == "opens_ok":
            summary["opens_ok"] += 1
        elif res == "opens_with_warning":
            summary["warning"] += 1
        elif res in {"cannot_open", "wrong_type"}:
            summary["failed"] += 1
    checked = summary["total"] - summary["pending"]
    summary["checked"] = checked
    summary["checked_ratio"] = round(checked / summary["total"], 4) if summary["total"] else 0.0
    summary["usable_checked_ratio"] = round((summary["opens_ok"] + summary["warning"]) / checked, 4) if checked else 0.0
    return summary


def render_markdown(payload: dict[str, Any]) -> str:
    s = payload.get("summary", {})
    lines = [
        "# Polik Recovery 抽查紀錄",
        "",
        f"- 產生時間：`{payload.get('generated_at','')}`",
        f"- 抽查樣本：`{s.get('total',0)}` 筆",
        f"- 已檢查：`{s.get('checked',0)}` 筆",
        f"- 可正常開啟：`{s.get('opens_ok',0)}` 筆",
        f"- 可開啟但有警告：`{s.get('warning',0)}` 筆",
        f"- 無法開啟/類型錯誤：`{s.get('failed',0)}` 筆",
        f"- 尚未抽查：`{s.get('pending',0)}` 筆",
        "",
        "| 類別 | 檔案 | 大小 | SHA256 | 抽查結果 | 備註 |",
        "|---|---|---:|---|---|---|",
    ]
    for item in payload.get("items", []):
        if not isinstance(item, dict):
            continue
        lines.append("| {category} | `{path}` | {size} | `{sha}` | {result} | {notes} |".format(
            category=item.get("category", ""),
            path=str(item.get("relative_path", "")).replace("|", "/"),
            size=item.get("size_human", ""),
            sha=str(item.get("sha256", ""))[:12] + ("..." if item.get("sha256") else ""),
            result=item.get("result_label", item.get("result", "")),
            notes=str(item.get("notes", "")).replace("|", "/"),
        ))
    return "\n".join(lines) + "\n"


def render_html(payload: dict[str, Any]) -> str:
    s = payload.get("summary", {})
    rows = []
    for item in payload.get("items", []):
        if not isinstance(item, dict):
            continue
        res = str(item.get("result", "pending"))
        cls = "ok" if res == "opens_ok" else "warn" if res in {"pending", "opens_with_warning"} else "bad" if res in {"cannot_open", "wrong_type"} else ""
        rows.append("<tr class='{cls}'><td>{cat}</td><td><code>{path}</code></td><td>{size}</td><td><code>{sha}</code></td><td>{result}</td><td>{checked}</td><td>{notes}</td></tr>".format(
            cls=cls,
            cat=html.escape(str(item.get("category", ""))),
            path=html.escape(str(item.get("relative_path", ""))),
            size=html.escape(str(item.get("size_human", ""))),
            sha=html.escape(str(item.get("sha256", ""))[:16]),
            result=html.escape(str(item.get("result_label", item.get("result", "")))),
            checked=html.escape(str(item.get("checked_at", ""))),
            notes=html.escape(str(item.get("notes", ""))),
        ))
    return f"""<!doctype html><html lang=\"zh-Hant\"><head><meta charset=\"utf-8\"><title>Polik Recovery 抽查紀錄</title>
<style>body{{font-family:-apple-system,BlinkMacSystemFont,'Noto Sans TC',sans-serif;background:#f8fafc;color:#172033;line-height:1.7;margin:32px}}main{{max-width:1180px;margin:auto;background:#fff;border:1px solid #e5e7eb;border-radius:18px;padding:26px;box-shadow:0 12px 30px rgba(15,23,42,.08)}}.grid{{display:grid;grid-template-columns:repeat(auto-fit,minmax(160px,1fr));gap:12px}}.metric{{border:1px solid #e5e7eb;border-radius:14px;padding:14px;background:#f8fafc}}.metric strong{{font-size:26px;display:block}}table{{border-collapse:collapse;width:100%;font-size:14px}}th,td{{border:1px solid #e5e7eb;padding:8px;text-align:left;vertical-align:top}}th{{background:#f1f5f9}}code{{font-family:ui-monospace,SFMono-Regular,Menlo,monospace}}tr.ok{{background:#f0fdf4}}tr.warn{{background:#fffbeb}}tr.bad{{background:#fef2f2}}.muted{{color:#64748b}}</style></head><body><main>
<h1>Polik Recovery 抽查紀錄</h1><p class=\"muted\">人工抽查用來確認救回檔案是否真的能開啟。尚未抽查的檔案不應直接視為已驗證交付。</p>
<div class=\"grid\"><div class=\"metric\"><span>樣本數</span><strong>{s.get('total',0)}</strong></div><div class=\"metric\"><span>已檢查</span><strong>{s.get('checked',0)}</strong></div><div class=\"metric\"><span>可正常開啟</span><strong>{s.get('opens_ok',0)}</strong></div><div class=\"metric\"><span>有警告</span><strong>{s.get('warning',0)}</strong></div><div class=\"metric\"><span>失敗</span><strong>{s.get('failed',0)}</strong></div><div class=\"metric\"><span>待抽查</span><strong>{s.get('pending',0)}</strong></div></div>
<h2>抽查明細</h2><table><tr><th>類別</th><th>檔案</th><th>大小</th><th>SHA256</th><th>結果</th><th>檢查時間</th><th>備註</th></tr>{''.join(rows) or '<tr><td colspan="7">尚無抽查項目</td></tr>'}</table>
</main></body></html>"""


def write_csv(path: Path, payload: dict[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    fields = ["id", "category", "relative_path", "file_name", "extension", "mime_guess", "size_bytes", "sha256", "result", "result_label", "checked_by", "checked_at", "notes"]
    with path.open("w", encoding="utf-8-sig", newline="") as f:
        writer = csv.DictWriter(f, fieldnames=fields)
        writer.writeheader()
        for item in payload.get("items", []):
            if isinstance(item, dict):
                writer.writerow({k: item.get(k, "") for k in fields})


def write_outputs(session: Path, payload: dict[str, Any]) -> None:
    audit_dir = session / AUDIT_DIR
    audit_dir.mkdir(parents=True, exist_ok=True)
    write_json(audit_dir / AUDIT_JSON, payload)
    (audit_dir / AUDIT_MD).write_text(render_markdown(payload), encoding="utf-8")
    (audit_dir / AUDIT_HTML).write_text(render_html(payload), encoding="utf-8")
    write_csv(audit_dir / AUDIT_CSV, payload)


def update_manifest(session: Path, payload: dict[str, Any], event_type: str, summary: str) -> None:
    manifest_path = session / "session_manifest.json"
    manifest = load_json(manifest_path)
    if not isinstance(manifest, dict):
        return
    manifest["schema"] = "polik.recovery.session.v2.6"
    manifest["version"] = "2.6-report-pdf-sample-audit"
    manifest.setdefault("folders", {})["audit"] = str(session / AUDIT_DIR)
    manifest["sample_audit"] = {
        "generated_at": payload.get("generated_at"),
        "folder": str(session / AUDIT_DIR),
        "json": str(session / AUDIT_DIR / AUDIT_JSON),
        "html": str(session / AUDIT_DIR / AUDIT_HTML),
        "summary": payload.get("summary", {}),
    }
    timeline = manifest.setdefault("timeline", [])
    if isinstance(timeline, list):
        timeline.append({
            "event_id": f"{event_type}_{int(dt.datetime.now().timestamp())}",
            "event_type": event_type,
            "title": "更新抽查紀錄",
            "status": "success",
            "summary": summary,
            "created_at": now_iso(),
            "actor": "session_sample_audit.py",
            "artifacts": [str(session / AUDIT_DIR / AUDIT_JSON), str(session / AUDIT_DIR / AUDIT_HTML)],
        })
    write_json(manifest_path, manifest)
    try:
        from session_timeline import write_timeline_files
        write_timeline_files(session, manifest)
    except Exception:
        pass


def main() -> int:
    parser = argparse.ArgumentParser(description="Create/update manual sample-audit records for a Polik Recovery session")
    parser.add_argument("--session", type=Path, required=True)
    parser.add_argument("--sample-size", type=int, default=5, help="Samples per category: recovered, possible, partial, fragment")
    parser.add_argument("--reset", action="store_true", help="Rebuild sample list and discard previous manual results")
    parser.add_argument("--add-file", help="Add/update one file review result. Path can be absolute or relative to session.")
    parser.add_argument("--result", choices=sorted(VALID_RESULTS), default="pending")
    parser.add_argument("--checked-by", default="")
    parser.add_argument("--note", default="")
    args = parser.parse_args()
    session = args.session.expanduser().resolve()
    if not (session / "session_manifest.json").exists():
        raise SystemExit(f"Missing session_manifest.json: {session}")
    if args.add_file:
        payload = add_or_update_result(session, args.add_file, args.result, args.checked_by, args.note)
    else:
        payload = init_or_update_audit(session, sample_size=max(args.sample_size, 1), keep_existing=not args.reset)
    print(json.dumps({"ok": True, "audit_json": str(session / AUDIT_DIR / AUDIT_JSON), "audit_html": str(session / AUDIT_DIR / AUDIT_HTML), "summary": payload.get("summary", {})}, ensure_ascii=False, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
