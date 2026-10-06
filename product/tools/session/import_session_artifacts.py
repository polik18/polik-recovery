#!/usr/bin/env python3
"""Import external recovery artifacts into a Polik Recovery Session.

This keeps image manifests, ddrescue maps, scan reports, recovery reports,
quality reports, logs, and Codex handoff artifacts inside the active case folder.
"""
from __future__ import annotations

import argparse
import json
import os
import shutil
import time
from pathlib import Path
from typing import Dict, List, Tuple

try:
    from session_timeline import append_event as append_timeline_event
except Exception:  # pragma: no cover
    append_timeline_event = None
try:
    from session_dashboard import write_dashboard as write_session_dashboard
except Exception:  # pragma: no cover
    write_session_dashboard = None

SESSION_DIRS = {
    "intake": "00_intake",
    "source_image": "01_source_image",
    "image_manifest": "01_source_image",
    "image": "01_source_image",
    "ddrescue_map": "01_source_image",
    "map_report": "01_source_image",
    "scan_report": "02_scan_reports",
    "vfs": "02_scan_reports",
    "recovery_report": "03_recovery_output",
    "recovery_output": "03_recovery_output",
    "quality_report": "04_quality_reports",
    "quality": "04_quality_reports",
    "log": "05_logs",
    "logs": "05_logs",
    "handoff": "06_handoff",
    "codex": "06_handoff",
    "validation": "06_handoff",
}


def detect_kind(path: Path, requested: str) -> Tuple[str, str]:
    req = (requested or "auto").strip().lower()
    name = path.name.lower()
    ext = path.suffix.lower().lstrip(".")
    if req and req != "auto":
        kind = req
    elif "scan" in name or "vfs" in name:
        kind = "scan_report"
    elif "quality" in name or "gate" in name:
        kind = "quality_report"
    elif "recovery_report" in name or "recover" in name:
        kind = "recovery_report"
    elif "map_report" in name or ext == "map":
        kind = "ddrescue_map"
    elif "manifest" in name or ext in {"img", "dmg", "raw", "bin"}:
        kind = "source_image"
    elif ext == "log" or "log" in name:
        kind = "log"
    elif any(token in name for token in ("codex", "handoff", "validation", "artifact")):
        kind = "handoff"
    else:
        kind = "intake"
    return kind, SESSION_DIRS.get(kind, "00_intake")


def unique_destination(target_dir: Path, name: str) -> Path:
    dest = target_dir / (name or "artifact")
    if not dest.exists():
        return dest
    stamp = str(int(time.time()))
    stem = dest.stem or "artifact"
    suffix = dest.suffix
    return target_dir / f"{stem}_imported_{stamp}{suffix}"


def load_manifest(session: Path) -> Dict:
    manifest_path = session / "session_manifest.json"
    if not manifest_path.is_file():
        raise SystemExit(f"Missing session_manifest.json: {manifest_path}")
    return json.loads(manifest_path.read_text(encoding="utf-8"))


def save_manifest(session: Path, manifest: Dict) -> None:
    manifest["updated_at_unix"] = str(int(time.time()))
    manifest["version"] = "2.4-session-dashboard"
    if str(manifest.get("schema", "")).startswith("polik.recovery.session.v2"):
        manifest["schema"] = "polik.recovery.session.v2.4"
    (session / "session_manifest.json").write_text(
        json.dumps(manifest, ensure_ascii=False, indent=2),
        encoding="utf-8",
    )


def refresh_index(session: Path, manifest: Dict) -> None:
    folders = [
        ("來源 image / map", "01_source_image"),
        ("掃描報告", "02_scan_reports"),
        ("完整還原", "03_recovery_output/recovered"),
        ("可疑還原", "03_recovery_output/suspect"),
        ("品質報告", "04_quality_reports"),
        ("Logs", "05_logs"),
        ("交接資料", "06_handoff"),
    ]
    rows = []
    for label, rel in folders:
        path = session / rel
        count = len(list(path.iterdir())) if path.exists() else 0
        rows.append(f"<tr><th>{label}</th><td><code>{rel}</code></td><td>{count}</td></tr>")
    artifacts = list(reversed(manifest.get("artifacts", [])))[:30]
    if artifacts:
        artifact_rows = "".join(
            f"<tr><td>{a.get('kind','')}</td><td><code>{a.get('destination_path','')}</code></td><td>{a.get('imported_at_unix','')}</td></tr>"
            for a in artifacts
        )
    else:
        artifact_rows = '<tr><td colspan="3" class="muted">尚未匯入 artifacts。</td></tr>'
    timeline = list(reversed(manifest.get("timeline", [])))[:20] if isinstance(manifest.get("timeline"), list) else []
    if timeline:
        timeline_rows = "".join(
            f"<tr><td>{e.get('created_at','')}</td><td>{e.get('status','')}</td><td>{e.get('title','')}</td><td>{e.get('summary','')}</td></tr>"
            for e in timeline
        )
    else:
        timeline_rows = '<tr><td colspan="4" class="muted">尚未建立 timeline。</td></tr>'
    html = f"""<!doctype html>
<html lang="zh-Hant"><head><meta charset="utf-8"><title>Polik Recovery Session {manifest.get('case_id','')}</title>
<style>body{{font-family:-apple-system,BlinkMacSystemFont,'Noto Sans TC',sans-serif;background:#f8fafc;color:#1f2937;line-height:1.6;margin:32px}}.card{{background:white;border:1px solid #e5e7eb;border-radius:16px;padding:18px;margin:14px 0;box-shadow:0 8px 22px rgba(15,23,42,.06)}}table{{border-collapse:collapse;width:100%}}th,td{{border:1px solid #e5e7eb;padding:10px;text-align:left}}th{{background:#f1f5f9;width:190px}}code{{font-family:ui-monospace,SFMono-Regular,Menlo,monospace}}.muted{{color:#64748b}}</style></head>
<body><h1>Polik Recovery Session</h1><p class="muted">v2.4 artifact 匯入、timeline 與 dashboard 索引。</p>
<div class="card"><table><tr><th>Case ID</th><td>{manifest.get('case_id','')}</td></tr><tr><th>Source</th><td><code>{manifest.get('source','')}</code></td></tr><tr><th>Mode</th><td>{manifest.get('mode','')}</td></tr><tr><th>Session Dir</th><td><code>{session}</code></td></tr></table></div>
<div class="card"><h2>資料夾狀態</h2><table><tr><th>項目</th><th>路徑</th><th>第一層項目數</th></tr>{''.join(rows)}</table></div>
<div class="card"><h2>最近匯入 Artifacts</h2><table><tr><th>類型</th><th>目標路徑</th><th>匯入時間</th></tr>{artifact_rows}</table></div>
<div class="card"><h2>操作時間線</h2><p><a href="07_timeline/session_timeline.html">開啟完整時間線</a></p><table><tr><th>時間</th><th>狀態</th><th>事件</th><th>摘要</th></tr>{timeline_rows}</table></div>
<div class="card"><h2>案件 Dashboard</h2><p><a href="08_dashboard/session_dashboard.html">開啟 v2.4 Session Dashboard</a></p><p class="muted">集中顯示 image/map、scan、recovery、quality 與最近操作。</p></div>
</body></html>"""
    (session / "session_index.html").write_text(html, encoding="utf-8")


def import_one(session: Path, source: Path, requested_kind: str, move: bool = False) -> Dict:
    if not source.exists():
        raise FileNotFoundError(source)
    if str(source).startswith("/dev/"):
        raise ValueError("Do not import /dev/* directly. Create an image/report first.")
    kind, rel = detect_kind(source, requested_kind)
    target_dir = session / rel
    target_dir.mkdir(parents=True, exist_ok=True)
    dest = unique_destination(target_dir, source.name)
    if source.is_dir():
        if move:
            shutil.move(str(source), str(dest))
        else:
            shutil.copytree(source, dest)
    else:
        if move:
            shutil.move(str(source), str(dest))
        else:
            shutil.copy2(source, dest)
    return {
        "kind": kind,
        "source_path": str(source),
        "destination_path": str(dest),
        "target_folder": rel,
        "imported_at_unix": str(int(time.time())),
        "mode": "move" if move else "copy",
    }


def main() -> int:
    ap = argparse.ArgumentParser(description="Import reports/artifacts into a Polik Recovery Session")
    ap.add_argument("--session", required=True, help="Session folder containing session_manifest.json")
    ap.add_argument("--kind", default="auto", help="auto, source_image, ddrescue_map, scan_report, recovery_report, quality_report, log, handoff, intake")
    ap.add_argument("--move", action="store_true", help="Move instead of copy")
    ap.add_argument("paths", nargs="+", help="Files or folders to import")
    args = ap.parse_args()

    session = Path(args.session).expanduser().resolve()
    if not session.is_dir():
        raise SystemExit(f"Session not found: {session}")
    manifest = load_manifest(session)
    imported: List[Dict] = []
    for raw in args.paths:
        entry = import_one(session, Path(raw).expanduser().resolve(), args.kind, args.move)
        imported.append(entry)
    existing_artifacts = manifest.get("artifacts")
    if not isinstance(existing_artifacts, list):
        if existing_artifacts:
            manifest.setdefault("artifact_inventory", existing_artifacts)
        manifest["artifacts"] = []
    manifest["artifacts"].extend(imported)
    save_manifest(session, manifest)
    if append_timeline_event:
        event = append_timeline_event(
            session,
            event_type="artifact_imported",
            title="匯入 Session Artifacts",
            status="success",
            summary=f"匯入 {len(imported)} 個 artifact 到案件資料夾。",
            actor="import_session_artifacts.py",
            artifacts=[item.get("destination_path", "") for item in imported],
            details={"kind": args.kind, "count": len(imported)},
        )
        manifest = load_manifest(session)
    refresh_index(session, manifest)
    dashboard_path = ""
    if write_session_dashboard:
        try:
            dashboard = write_session_dashboard(session)
            dashboard_path = dashboard.get("dashboard_html", "")
        except Exception:
            dashboard_path = ""
    out = {"ok": True, "session": str(session), "imported": imported, "index": str(session / "session_index.html"), "timeline": str(session / "07_timeline/session_timeline.html"), "dashboard": dashboard_path}
    print(json.dumps(out, ensure_ascii=False, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
