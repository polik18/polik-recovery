#!/usr/bin/env python3
"""Build a Polik Recovery Session dashboard.

The dashboard summarizes one recovery case at a glance:
- source image / ddrescue map status
- scan report presence and rough counts
- recovered / suspect / skipped output counts
- quality report PASS/FAIL signals
- recent timeline events and next recommended actions
"""
from __future__ import annotations

import argparse
import datetime as dt
import html
import json
import os
from pathlib import Path
from typing import Any

DASHBOARD_DIR = "08_dashboard"
DASHBOARD_JSON = "session_dashboard.json"
DASHBOARD_HTML = "session_dashboard.html"

SESSION_FOLDERS = {
    "intake": "00_intake",
    "source_image": "01_source_image",
    "scan_reports": "02_scan_reports",
    "recovery_output": "03_recovery_output",
    "recovered": "03_recovery_output/recovered",
    "suspect_possible": "03_recovery_output/suspect/possible",
    "suspect_partial": "03_recovery_output/suspect/partial",
    "suspect_fragments": "03_recovery_output/suspect/fragments",
    "skipped": "03_recovery_output/skipped",
    "quality_reports": "04_quality_reports",
    "logs": "05_logs",
    "handoff": "06_handoff",
    "timeline": "07_timeline",
    "dashboard": DASHBOARD_DIR,
    "reports": "09_reports",
    "audit": "10_audit",
    "delivery": "11_delivery",
}


def now_iso() -> str:
    return dt.datetime.now(dt.timezone.utc).astimezone().isoformat(timespec="seconds")


def load_json(path: Path) -> Any | None:
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except Exception:
        return None


def safe_int(value: Any, default: int = 0) -> int:
    try:
        if value is None:
            return default
        return int(value)
    except Exception:
        return default


def human_bytes(n: int | float | None) -> str:
    n = float(n or 0)
    units = ["B", "KB", "MB", "GB", "TB", "PB"]
    for unit in units:
        if abs(n) < 1024 or unit == units[-1]:
            return f"{n:.0f} {unit}" if unit == "B" else f"{n:.1f} {unit}"
        n /= 1024
    return f"{n:.1f} PB"


def rel(path: Path, base: Path) -> str:
    try:
        return str(path.resolve().relative_to(base.resolve()))
    except Exception:
        return str(path)


def list_files(path: Path, pattern: str = "**/*") -> list[Path]:
    if not path.exists():
        return []
    return sorted([p for p in path.glob(pattern) if p.is_file()])


def count_files(path: Path) -> int:
    return len(list_files(path))


def total_size(path: Path) -> int:
    total = 0
    for p in list_files(path):
        try:
            total += p.stat().st_size
        except OSError:
            pass
    return total


def newest(files: list[Path]) -> Path | None:
    if not files:
        return None
    return max(files, key=lambda p: p.stat().st_mtime if p.exists() else 0)


def extract_map_summary(payload: Any) -> dict[str, Any]:
    if not isinstance(payload, dict):
        return {}
    # Support both v1.9 map report shapes and future ddrescue parsers.
    candidates = [payload, payload.get("summary"), payload.get("totals"), payload.get("map_summary")]
    out: dict[str, Any] = {}
    for c in candidates:
        if not isinstance(c, dict):
            continue
        for key in [
            "finished_percent", "rescued_percent", "bad_percent", "non_tried_percent",
            "rescued_bytes", "bad_bytes", "non_tried_bytes", "total_bytes",
            "largest_bad_range_bytes", "status", "recommendation",
        ]:
            if key in c and key not in out:
                out[key] = c[key]
    # Some reports use state counters.
    states = payload.get("states") if isinstance(payload.get("states"), dict) else None
    if states:
        total = sum(safe_int(v.get("bytes") if isinstance(v, dict) else v) for v in states.values())
        if total > 0:
            rescued = safe_int(states.get("+", {}).get("bytes") if isinstance(states.get("+"), dict) else states.get("+"))
            bad = safe_int(states.get("-", {}).get("bytes") if isinstance(states.get("-"), dict) else states.get("-"))
            non = safe_int(states.get("?", {}).get("bytes") if isinstance(states.get("?"), dict) else states.get("?"))
            out.setdefault("total_bytes", total)
            out.setdefault("rescued_bytes", rescued)
            out.setdefault("bad_bytes", bad)
            out.setdefault("non_tried_bytes", non)
            out.setdefault("rescued_percent", round(rescued / total * 100, 2))
            out.setdefault("bad_percent", round(bad / total * 100, 2))
            out.setdefault("non_tried_percent", round(non / total * 100, 2))
    return out


def summarize_source_image(session: Path) -> dict[str, Any]:
    src = session / SESSION_FOLDERS["source_image"]
    image_files = [p for p in list_files(src) if p.suffix.lower() in {".img", ".dmg", ".raw", ".bin"}]
    manifests = [p for p in list_files(src, "**/*.manifest.json")]
    maps = [p for p in list_files(src) if p.suffix.lower() == ".map"]
    map_reports = [p for p in list_files(src) if "map_report" in p.name.lower() and p.suffix.lower() == ".json"]
    manifest_path = newest(manifests)
    manifest_payload = load_json(manifest_path) if manifest_path else None
    map_report_path = newest(map_reports)
    map_payload = load_json(map_report_path) if map_report_path else None
    map_summary = extract_map_summary(map_payload)
    image_size = total_size(src)
    manifest_summary: dict[str, Any] = {}
    if isinstance(manifest_payload, dict):
        for key in ["source", "source_path", "image_path", "method", "status", "size_bytes", "bytes_copied", "created_at", "started_at", "finished_at"]:
            if key in manifest_payload:
                manifest_summary[key] = manifest_payload[key]
    return {
        "file_count": count_files(src),
        "image_count": len(image_files),
        "manifest_count": len(manifests),
        "map_count": len(maps),
        "map_report_count": len(map_reports),
        "latest_manifest": rel(manifest_path, session) if manifest_path else "",
        "latest_map_report": rel(map_report_path, session) if map_report_path else "",
        "total_folder_bytes": image_size,
        "manifest": manifest_summary,
        "map": map_summary,
        "status": "ready" if (image_files or manifests) else "missing",
    }


def summarize_scan_reports(session: Path) -> dict[str, Any]:
    folder = session / SESSION_FOLDERS["scan_reports"]
    files = list_files(folder)
    json_files = [p for p in files if p.suffix.lower() == ".json"]
    newest_json = newest(json_files)
    total_entries = 0
    deleted_entries = 0
    allocated_entries = 0
    quality_counts: dict[str, int] = {}
    if newest_json:
        payload = load_json(newest_json)
        candidates = []
        if isinstance(payload, list):
            candidates = payload
        elif isinstance(payload, dict):
            for key in ("entries", "files", "results", "items", "records"):
                if isinstance(payload.get(key), list):
                    candidates = payload[key]
                    break
        total_entries = len(candidates)
        for item in candidates:
            if not isinstance(item, dict):
                continue
            status = str(item.get("status", item.get("state", ""))).lower()
            if "deleted" in status or item.get("deleted") is True:
                deleted_entries += 1
            elif status:
                allocated_entries += 1
            quality = str(item.get("quality", item.get("tier", "unknown"))).lower() or "unknown"
            quality_counts[quality] = quality_counts.get(quality, 0) + 1
    return {
        "file_count": len(files),
        "json_report_count": len(json_files),
        "latest_json": rel(newest_json, session) if newest_json else "",
        "total_entries": total_entries,
        "deleted_entries": deleted_entries,
        "allocated_entries": allocated_entries,
        "quality_counts": quality_counts,
        "status": "ready" if files else "missing",
    }


def summarize_recovery(session: Path) -> dict[str, Any]:
    base = session / SESSION_FOLDERS["recovery_output"]
    recovered = session / SESSION_FOLDERS["recovered"]
    possible = session / SESSION_FOLDERS["suspect_possible"]
    partial = session / SESSION_FOLDERS["suspect_partial"]
    fragments = session / SESSION_FOLDERS["suspect_fragments"]
    skipped = session / SESSION_FOLDERS["skipped"]
    reports = [p for p in list_files(base) if p.name.lower().startswith("recovery_report")]
    latest_report = newest(reports)
    report_payload = load_json(latest_report) if latest_report and latest_report.suffix.lower() == ".json" else None
    report_summary: dict[str, Any] = {}
    if isinstance(report_payload, dict):
        for key in ["success", "success_count", "recovered", "quarantined", "skipped", "failed", "completed", "reportVersion", "report_version"]:
            if key in report_payload:
                report_summary[key] = report_payload[key]
    complete_count = count_files(recovered)
    suspect_count = count_files(possible) + count_files(partial) + count_files(fragments)
    return {
        "complete_count": complete_count,
        "complete_bytes": total_size(recovered),
        "suspect_possible_count": count_files(possible),
        "suspect_partial_count": count_files(partial),
        "suspect_fragments_count": count_files(fragments),
        "suspect_count": suspect_count,
        "suspect_bytes": total_size(possible) + total_size(partial) + total_size(fragments),
        "skipped_count": count_files(skipped),
        "report_count": len(reports),
        "latest_report": rel(latest_report, session) if latest_report else "",
        "report_summary": report_summary,
        "status": "ready" if (complete_count or suspect_count or reports) else "missing",
    }


def summarize_quality(session: Path) -> dict[str, Any]:
    folder = session / SESSION_FOLDERS["quality_reports"]
    files = list_files(folder)
    jsons = [p for p in files if p.suffix.lower() == ".json"]
    pass_count = 0
    fail_count = 0
    latest = newest(jsons)
    latest_summary: dict[str, Any] = {}
    for p in jsons:
        payload = load_json(p)
        if not isinstance(payload, dict):
            continue
        ok = payload.get("ok", payload.get("passed", payload.get("pass")))
        status = str(payload.get("status", "")).lower()
        if ok is True or status == "pass":
            pass_count += 1
        elif ok is False or status == "fail":
            fail_count += 1
    if latest:
        payload = load_json(latest)
        if isinstance(payload, dict):
            for key in ["ok", "passed", "status", "expected_found_ratio", "high_confidence_usable_ratio", "false_positive_count", "missed_expected_count", "known_invalid_recovered_count"]:
                if key in payload:
                    latest_summary[key] = payload[key]
            metrics = payload.get("metrics")
            if isinstance(metrics, dict):
                latest_summary.setdefault("metrics", metrics)
    return {
        "file_count": len(files),
        "json_report_count": len(jsons),
        "pass_count": pass_count,
        "fail_count": fail_count,
        "latest_json": rel(latest, session) if latest else "",
        "latest_summary": latest_summary,
        "status": "ready" if files else "missing",
    }


def summarize_audit(session: Path) -> dict[str, Any]:
    folder = session / SESSION_FOLDERS.get("audit", "10_audit")
    files = list_files(folder)
    payload = load_json(folder / "sample_audit.json")
    summary = payload.get("summary", {}) if isinstance(payload, dict) and isinstance(payload.get("summary"), dict) else {}
    return {
        "file_count": len(files),
        "latest_json": rel(folder / "sample_audit.json", session) if (folder / "sample_audit.json").exists() else "",
        "latest_html": rel(folder / "sample_audit.html", session) if (folder / "sample_audit.html").exists() else "",
        "summary": summary,
        "status": "ready" if summary else "missing",
    }


def summarize_delivery(session: Path) -> dict[str, Any]:
    folder = session / SESSION_FOLDERS.get("delivery", "11_delivery")
    package_dir = folder / "packages"
    manifest_files = list_files(folder, "**/delivery_manifest.json")
    html_files = list_files(folder, "**/delivery_manifest.html")
    zip_files = sorted([p for p in list_files(package_dir) if p.suffix.lower() == ".zip"], key=lambda p: p.stat().st_mtime if p.exists() else 0)
    latest_zip = newest(zip_files)
    latest_manifest = newest(manifest_files)
    payload = load_json(latest_manifest) if latest_manifest else None
    latest_summary: dict[str, Any] = {}
    if isinstance(payload, dict):
        for key in ["package_id", "generated_at", "customer_safe", "included_counts", "package_file_count", "package_total_human", "zip_size_human"]:
            if key in payload:
                latest_summary[key] = payload[key]
    return {
        "package_count": len(zip_files),
        "manifest_count": len(manifest_files),
        "html_count": len(html_files),
        "latest_zip": rel(latest_zip, session) if latest_zip else "",
        "latest_manifest": rel(latest_manifest, session) if latest_manifest else "",
        "latest_summary": latest_summary,
        "status": "ready" if zip_files else "missing",
    }


def summarize_timeline(manifest: dict[str, Any]) -> dict[str, Any]:
    events = manifest.get("timeline") if isinstance(manifest.get("timeline"), list) else []
    recent = []
    for event in events[-8:][::-1]:
        if isinstance(event, dict):
            recent.append({
                "created_at": event.get("created_at", event.get("created_at_unix", "")),
                "status": event.get("status", ""),
                "title": event.get("title", ""),
                "summary": event.get("summary", ""),
                "event_type": event.get("event_type", ""),
            })
    return {"event_count": len(events), "recent": recent}


def next_actions(source: dict[str, Any], scan: dict[str, Any], recovery: dict[str, Any], quality: dict[str, Any], audit: dict[str, Any] | None = None, delivery: dict[str, Any] | None = None) -> list[dict[str, str]]:
    actions: list[dict[str, str]] = []
    if source.get("status") == "missing":
        actions.append({"level": "warning", "title": "尚未建立或匯入來源映像檔", "detail": "來源是實體磁碟時，先用 image-first / ddrescue 建立 .img 與 map，再掃描 image。"})
    elif not source.get("map_report_count"):
        actions.append({"level": "info", "title": "建議產生 ddrescue map 報告", "detail": "有 map report 才能看壞區比例、未完成區段與 retry 建議。"})
    if scan.get("status") == "missing":
        actions.append({"level": "warning", "title": "尚未匯入掃描報告", "detail": "掃描完成後，將 VFS/scan JSON 或 HTML 匯入 02_scan_reports。"})
    if recovery.get("status") == "missing":
        actions.append({"level": "info", "title": "尚未看到還原輸出", "detail": "還原時請輸出到 03_recovery_output，完整與可疑檔案要分流。"})
    if recovery.get("suspect_count", 0) > recovery.get("complete_count", 0) and recovery.get("suspect_count", 0) > 0:
        actions.append({"level": "warning", "title": "可疑檔案多於完整檔案", "detail": "建議先檢查來源 image/map 是否有壞區，並降低深度掃描假陽性。"})
    if quality.get("status") == "missing":
        actions.append({"level": "info", "title": "尚未匯入品質報告", "detail": "建議跑 quality gate 或匯入 Codex 驗證報告，確認可開啟率與垃圾檔比例。"})
    if quality.get("fail_count", 0) > 0:
        actions.append({"level": "danger", "title": "品質門檻有失敗紀錄", "detail": "請先看 04_quality_reports 最新報告，再決定是否發版或交付。"})
    audit = audit or {}
    if audit.get("status") == "missing":
        actions.append({"level": "info", "title": "尚未建立人工抽查紀錄", "detail": "交付前建議執行 make session-audit，抽查 recovered/ 與 suspect/ 樣本。"})
    elif int(audit.get("summary", {}).get("pending", 0) or 0) > 0:
        actions.append({"level": "warning", "title": "抽查樣本尚未全部確認", "detail": "請開啟 10_audit/sample_audit.html 並補上抽查結果。"})
    delivery = delivery or {}
    if delivery.get("status") == "missing":
        actions.append({"level": "info", "title": "尚未建立客戶交付包", "detail": "交付前請執行 make session-delivery-package，建立不含工程 log/raw image 的客戶安全 ZIP。"})
    if not actions:
        actions.append({"level": "success", "title": "案件資料看起來完整", "detail": "可檢查最新 timeline、quality report，準備交接、交付或發版驗證。"})
    return actions


def compute_health_score(source: dict[str, Any], scan: dict[str, Any], recovery: dict[str, Any], quality: dict[str, Any]) -> dict[str, Any]:
    score = 20
    score += 20 if source.get("status") != "missing" else 0
    score += 10 if source.get("map_report_count") else 0
    score += 20 if scan.get("status") != "missing" else 0
    score += 20 if recovery.get("status") != "missing" else 0
    score += 10 if quality.get("status") != "missing" else 0
    score -= 15 if quality.get("fail_count", 0) else 0
    score -= 10 if recovery.get("suspect_count", 0) > recovery.get("complete_count", 0) and recovery.get("suspect_count", 0) else 0
    score = max(0, min(100, score))
    if score >= 80:
        label, level = "案件資料完整", "good"
    elif score >= 55:
        label, level = "可繼續處理", "warn"
    else:
        label, level = "資料不足", "bad"
    return {"score": score, "label": label, "level": level}


def build_dashboard(session: Path) -> dict[str, Any]:
    session = session.expanduser().resolve()
    manifest_path = session / "session_manifest.json"
    if not manifest_path.is_file():
        raise SystemExit(f"Missing session_manifest.json: {manifest_path}")
    manifest = load_json(manifest_path)
    if not isinstance(manifest, dict):
        raise SystemExit(f"Invalid session_manifest.json: {manifest_path}")
    for folder in SESSION_FOLDERS.values():
        (session / folder).mkdir(parents=True, exist_ok=True)

    source = summarize_source_image(session)
    scan = summarize_scan_reports(session)
    recovery = summarize_recovery(session)
    quality = summarize_quality(session)
    audit = summarize_audit(session)
    delivery = summarize_delivery(session)
    timeline = summarize_timeline(manifest)
    actions = next_actions(source, scan, recovery, quality, audit, delivery)
    health = compute_health_score(source, scan, recovery, quality)

    folder_counts = {
        key: {"path": value, "files": count_files(session / value), "bytes": total_size(session / value)}
        for key, value in SESSION_FOLDERS.items()
    }
    payload = {
        "schema": "polik.recovery.session.dashboard.v2.7",
        "generated_at": now_iso(),
        "case_id": manifest.get("case_id", session.name),
        "session_dir": str(session),
        "source": manifest.get("source", ""),
        "mode": manifest.get("mode", ""),
        "health": health,
        "source_image": source,
        "scan": scan,
        "recovery": recovery,
        "quality": quality,
        "audit": audit,
        "delivery": delivery,
        "timeline": timeline,
        "folder_counts": folder_counts,
        "next_actions": actions,
    }
    return payload


def badge(level: str) -> str:
    return {"good": "✅", "warn": "🟡", "bad": "🔴", "success": "✅", "warning": "🟡", "danger": "🔴", "info": "ℹ️"}.get(level, "ℹ️")


def card(title: str, value: str, detail: str = "", cls: str = "") -> str:
    return f"<div class='metric {html.escape(cls)}'><span>{html.escape(title)}</span><strong>{html.escape(value)}</strong><small>{html.escape(detail)}</small></div>"


def render_dashboard_html(payload: dict[str, Any]) -> str:
    h = payload.get("health", {})
    source = payload.get("source_image", {})
    scan = payload.get("scan", {})
    recovery = payload.get("recovery", {})
    quality = payload.get("quality", {})
    audit = payload.get("audit", {})
    delivery = payload.get("delivery", {})
    timeline = payload.get("timeline", {})
    metrics = "".join([
        card("健康分數", f"{h.get('score', 0)}/100", str(h.get("label", "")), str(h.get("level", ""))),
        card("完整還原", str(recovery.get("complete_count", 0)), human_bytes(recovery.get("complete_bytes", 0)), "good"),
        card("可疑 / partial", str(recovery.get("suspect_count", 0)), human_bytes(recovery.get("suspect_bytes", 0)), "warn"),
        card("掃描項目", str(scan.get("total_entries", 0)), f"deleted {scan.get('deleted_entries', 0)}", ""),
        card("Image / Manifest", str(source.get("manifest_count", 0)), f"map report {source.get('map_report_count', 0)}", ""),
        card("Quality", f"PASS {quality.get('pass_count', 0)} / FAIL {quality.get('fail_count', 0)}", str(quality.get("latest_json", "")), "bad" if quality.get("fail_count", 0) else ""),
        card("人工抽查", f"{audit.get('summary', {}).get('checked', 0)}/{audit.get('summary', {}).get('total', 0)}", str(audit.get("latest_html", "")), "warn" if audit.get("status") == "missing" or audit.get("summary", {}).get("pending", 0) else "good"),
        card("客戶交付 ZIP", str(delivery.get("package_count", 0)), str(delivery.get("latest_zip", "")), "good" if delivery.get("package_count", 0) else "warn"),
    ])
    actions = "".join(
        f"<li class='{html.escape(a.get('level','info'))}'><strong>{badge(a.get('level','info'))} {html.escape(a.get('title',''))}</strong><br><span>{html.escape(a.get('detail',''))}</span></li>"
        for a in payload.get("next_actions", [])
    )
    timeline_rows = "".join(
        f"<tr><td>{html.escape(str(e.get('created_at','')))}</td><td>{html.escape(str(e.get('status','')))}</td><td>{html.escape(str(e.get('title','')))}</td><td>{html.escape(str(e.get('summary','')))}</td></tr>"
        for e in timeline.get("recent", [])
    ) or "<tr><td colspan='4' class='muted'>尚無時間線紀錄。</td></tr>"
    folders = payload.get("folder_counts", {})
    folder_rows = "".join(
        f"<tr><td>{html.escape(str(k))}</td><td><code>{html.escape(str(v.get('path','')))}</code></td><td>{v.get('files',0)}</td><td>{human_bytes(v.get('bytes',0))}</td></tr>"
        for k, v in folders.items()
    )
    map_summary = source.get("map", {}) or {}
    map_rows = "".join(
        f"<tr><th>{html.escape(str(k))}</th><td>{html.escape(str(v))}</td></tr>"
        for k, v in map_summary.items()
    ) or "<tr><td colspan='2' class='muted'>尚未匯入 ddrescue map report。</td></tr>"
    doc = f"""<!doctype html>
<html lang="zh-Hant"><head><meta charset="utf-8"><title>Polik Session Dashboard {html.escape(str(payload.get('case_id','')))}</title>
<style>
body{{font-family:-apple-system,BlinkMacSystemFont,'Noto Sans TC',sans-serif;background:#f8fafc;color:#172033;margin:32px;line-height:1.6}}
h1{{margin:0 0 4px}}.muted{{color:#64748b}}.grid{{display:grid;grid-template-columns:repeat(auto-fit,minmax(180px,1fr));gap:12px;margin:18px 0}}
.metric,section{{background:#fff;border:1px solid #e5e7eb;border-radius:16px;padding:16px;box-shadow:0 8px 22px rgba(15,23,42,.06)}}.metric span{{display:block;color:#64748b;font-size:13px}}.metric strong{{font-size:28px;display:block;margin:4px 0}}.metric small{{color:#64748b;word-break:break-all}}
.metric.good{{border-left:7px solid #22c55e}}.metric.warn{{border-left:7px solid #f59e0b}}.metric.bad{{border-left:7px solid #ef4444}}
table{{border-collapse:collapse;width:100%;background:#fff}}th,td{{border:1px solid #e5e7eb;padding:10px;text-align:left;vertical-align:top}}th{{background:#f1f5f9}}code{{font-family:ui-monospace,SFMono-Regular,Menlo,monospace;word-break:break-all}}
ul.actions{{padding-left:0;list-style:none}}ul.actions li{{background:#fff;border:1px solid #e5e7eb;border-left:7px solid #38bdf8;border-radius:14px;padding:12px;margin:10px 0}}ul.actions li.success{{border-left-color:#22c55e}}ul.actions li.warning{{border-left-color:#f59e0b}}ul.actions li.danger{{border-left-color:#ef4444}}
.links a{{display:inline-block;margin-right:10px}}
</style></head><body>
<h1>Polik Recovery Session Dashboard</h1>
<p class="muted">Case：<code>{html.escape(str(payload.get('case_id','')))}</code> ｜ Generated：{html.escape(str(payload.get('generated_at','')))}</p>
<p class="links"><a href="../session_index.html">Session Index</a><a href="../07_timeline/session_timeline.html">Timeline</a></p>
<div class="grid">{metrics}</div>
<section><h2>下一步建議</h2><ul class="actions">{actions}</ul></section>
<section><h2>ddrescue / image 狀態</h2><table>{map_rows}</table><p class="muted">Latest manifest：<code>{html.escape(str(source.get('latest_manifest','')))}</code></p></section>
<section><h2>最近時間線</h2><table><tr><th>時間</th><th>狀態</th><th>事件</th><th>摘要</th></tr>{timeline_rows}</table></section>
<section><h2>資料夾統計</h2><table><tr><th>key</th><th>path</th><th>files</th><th>size</th></tr>{folder_rows}</table></section>
</body></html>"""
    return doc


def write_dashboard(session: Path, payload: dict[str, Any] | None = None) -> dict[str, Any]:
    session = session.expanduser().resolve()
    if payload is None:
        payload = build_dashboard(session)
    ddir = session / DASHBOARD_DIR
    ddir.mkdir(parents=True, exist_ok=True)
    json_path = ddir / DASHBOARD_JSON
    html_path = ddir / DASHBOARD_HTML
    json_path.write_text(json.dumps(payload, ensure_ascii=False, indent=2), encoding="utf-8")
    html_path.write_text(render_dashboard_html(payload), encoding="utf-8")

    manifest_path = session / "session_manifest.json"
    manifest = load_json(manifest_path)
    if isinstance(manifest, dict):
        folders = manifest.setdefault("folders", {})
        if isinstance(folders, dict):
            folders["dashboard"] = str(ddir)
            folders["audit"] = str(session / SESSION_FOLDERS.get("audit", "10_audit"))
            folders["delivery"] = str(session / SESSION_FOLDERS.get("delivery", "11_delivery"))
        manifest["dashboard"] = {
            "schema": payload.get("schema"),
            "generated_at": payload.get("generated_at"),
            "health": payload.get("health"),
            "html": str(html_path),
            "json": str(json_path),
        }
        manifest["version"] = "2.7-delivery-package"
        if str(manifest.get("schema", "")).startswith("polik.recovery.session.v2"):
            manifest["schema"] = "polik.recovery.session.v2.7"
        manifest_path.write_text(json.dumps(manifest, ensure_ascii=False, indent=2), encoding="utf-8")
    return {"ok": True, "session": str(session), "dashboard_json": str(json_path), "dashboard_html": str(html_path), "health": payload.get("health", {})}


def main() -> int:
    ap = argparse.ArgumentParser(description="Build Polik Recovery Session dashboard")
    ap.add_argument("--session", required=True, help="Session folder containing session_manifest.json")
    ap.add_argument("--json-only", action="store_true", help="Only print dashboard payload without writing files")
    args = ap.parse_args()

    session = Path(args.session).expanduser().resolve()
    payload = build_dashboard(session)
    if args.json_only:
        print(json.dumps(payload, ensure_ascii=False, indent=2))
        return 0
    result = write_dashboard(session, payload)
    print(json.dumps(result, ensure_ascii=False, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

