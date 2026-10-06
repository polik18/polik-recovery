#!/usr/bin/env python3
"""Maintain a chronological operation timeline for a Polik Recovery Session.

The timeline is intentionally plain JSON + HTML so it can be reviewed by the
user, Codex, or another engineer without launching the desktop app.
"""
from __future__ import annotations

import argparse
import datetime as dt
import html
import json
import time
from pathlib import Path
from typing import Any, Iterable

TIMELINE_DIR = "07_timeline"
TIMELINE_JSON = "session_timeline.json"
TIMELINE_HTML = "session_timeline.html"
DASHBOARD_DIR = "08_dashboard"


try:
    from session_dashboard import write_dashboard as write_session_dashboard
except Exception:  # pragma: no cover
    write_session_dashboard = None

STATUS_LABELS = {
    "info": "資訊",
    "success": "完成",
    "warning": "警告",
    "error": "錯誤",
}


def now_iso() -> str:
    return dt.datetime.now(dt.timezone.utc).astimezone().isoformat(timespec="seconds")


def now_unix() -> int:
    return int(time.time())


def load_manifest(session: Path) -> dict[str, Any]:
    path = session / "session_manifest.json"
    if not path.is_file():
        raise SystemExit(f"Missing session_manifest.json: {path}")
    data = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(data, dict):
        raise SystemExit(f"session_manifest.json is not an object: {path}")
    return data


def save_manifest(session: Path, manifest: dict[str, Any]) -> None:
    manifest["updated_at_unix"] = str(now_unix())
    manifest["updated_at"] = now_iso()
    (session / "session_manifest.json").write_text(
        json.dumps(manifest, ensure_ascii=False, indent=2),
        encoding="utf-8",
    )


def timeline_dir(session: Path) -> Path:
    d = session / TIMELINE_DIR
    d.mkdir(parents=True, exist_ok=True)
    return d


def normalize_artifacts(values: Iterable[str] | None) -> list[str]:
    if not values:
        return []
    return [str(v) for v in values if str(v).strip()]


def build_event(
    manifest: dict[str, Any],
    *,
    event_type: str,
    title: str,
    status: str = "info",
    summary: str = "",
    actor: str = "",
    artifacts: Iterable[str] | None = None,
    tags: Iterable[str] | None = None,
    details: dict[str, Any] | None = None,
) -> dict[str, Any]:
    events = manifest.get("timeline")
    seq = len(events) + 1 if isinstance(events, list) else 1
    stamp = now_unix()
    status = (status or "info").strip().lower()
    if status not in STATUS_LABELS:
        status = "info"
    return {
        "id": f"evt-{stamp}-{seq:04d}",
        "sequence": seq,
        "created_at": now_iso(),
        "created_at_unix": str(stamp),
        "event_type": (event_type or "note").strip() or "note",
        "title": (title or event_type or "Session event").strip(),
        "status": status,
        "summary": summary or "",
        "actor": actor or "",
        "artifacts": normalize_artifacts(artifacts),
        "tags": normalize_artifacts(tags),
        "details": details or {},
    }


def append_event(
    session: Path,
    *,
    event_type: str,
    title: str,
    status: str = "info",
    summary: str = "",
    actor: str = "",
    artifacts: Iterable[str] | None = None,
    tags: Iterable[str] | None = None,
    details: dict[str, Any] | None = None,
) -> dict[str, Any]:
    session = session.expanduser().resolve()
    manifest = load_manifest(session)
    events = manifest.get("timeline")
    if not isinstance(events, list):
        events = []
    event = build_event(
        manifest,
        event_type=event_type,
        title=title,
        status=status,
        summary=summary,
        actor=actor,
        artifacts=artifacts,
        tags=tags,
        details=details,
    )
    events.append(event)
    manifest["timeline"] = events
    manifest["timeline_count"] = len(events)
    manifest.setdefault("folders", {})["timeline"] = str(timeline_dir(session))
    save_manifest(session, manifest)
    write_timeline_files(session, manifest)
    return event


def write_timeline_files(session: Path, manifest: dict[str, Any] | None = None) -> dict[str, Any]:
    session = session.expanduser().resolve()
    if manifest is None:
        manifest = load_manifest(session)
    tdir = timeline_dir(session)
    events = manifest.get("timeline")
    if not isinstance(events, list):
        events = []
    payload = {
        "schema": "polik.recovery.session.timeline.v2.4",
        "case_id": manifest.get("case_id", ""),
        "session_dir": str(session),
        "generated_at": now_iso(),
        "event_count": len(events),
        "events": events,
    }
    (tdir / TIMELINE_JSON).write_text(json.dumps(payload, ensure_ascii=False, indent=2), encoding="utf-8")
    (tdir / TIMELINE_HTML).write_text(render_timeline_html(payload), encoding="utf-8")
    return payload


def render_timeline_html(payload: dict[str, Any]) -> str:
    rows = []
    for event in payload.get("events", []):
        if not isinstance(event, dict):
            continue
        status = html.escape(str(event.get("status", "info")))
        status_label = STATUS_LABELS.get(event.get("status", "info"), event.get("status", "info"))
        artifacts = event.get("artifacts") or []
        artifact_list = "".join(f"<li><code>{html.escape(str(a))}</code></li>" for a in artifacts) or "<li class='muted'>無</li>"
        details = event.get("details") or {}
        details_text = json.dumps(details, ensure_ascii=False, indent=2) if details else ""
        rows.append(f"""
        <article class="event {status}">
          <div class="time">#{html.escape(str(event.get('sequence','')))} · {html.escape(str(event.get('created_at','')))}</div>
          <h2>{html.escape(str(event.get('title','')))}</h2>
          <div class="meta"><span>{html.escape(str(status_label))}</span><span>{html.escape(str(event.get('event_type','')))}</span><span>{html.escape(str(event.get('actor','')) or '-')}</span></div>
          <p>{html.escape(str(event.get('summary','')))}</p>
          <details><summary>Artifacts / details</summary><ul>{artifact_list}</ul>{'<pre>'+html.escape(details_text)+'</pre>' if details_text else ''}</details>
        </article>
        """)
    if not rows:
        rows.append("<p class='muted'>尚無時間線紀錄。</p>")
    return f"""<!doctype html>
<html lang="zh-Hant"><head><meta charset="utf-8"><title>Polik Recovery Timeline {html.escape(str(payload.get('case_id','')))}</title>
<style>
body{{font-family:-apple-system,BlinkMacSystemFont,'Noto Sans TC',sans-serif;background:#f8fafc;color:#172033;line-height:1.65;margin:32px}}
h1{{margin-bottom:6px}}.muted{{color:#64748b}}.event{{background:white;border:1px solid #e5e7eb;border-left:8px solid #94a3b8;border-radius:16px;padding:16px;margin:14px 0;box-shadow:0 8px 22px rgba(15,23,42,.06)}}
.event.success{{border-left-color:#22c55e}}.event.warning{{border-left-color:#f59e0b}}.event.error{{border-left-color:#ef4444}}.event.info{{border-left-color:#38bdf8}}
.time,.meta{{font-size:12px;color:#64748b}}.meta span{{display:inline-block;background:#f1f5f9;border-radius:999px;padding:2px 9px;margin-right:6px}}
h2{{margin:6px 0 4px;font-size:20px}}code,pre{{font-family:ui-monospace,SFMono-Regular,Menlo,monospace}}pre{{background:#0f172a;color:#e2e8f0;border-radius:12px;padding:12px;overflow:auto}}details{{margin-top:8px}}
</style></head><body>
<h1>Polik Recovery Session Timeline</h1>
<p class="muted">Case：<code>{html.escape(str(payload.get('case_id','')))}</code> ｜ Session：<code>{html.escape(str(payload.get('session_dir','')))}</code> ｜ Events：{payload.get('event_count',0)}</p>
{''.join(rows)}
</body></html>"""


def parse_key_values(values: list[str] | None) -> dict[str, Any]:
    out: dict[str, Any] = {}
    for item in values or []:
        if "=" in item:
            k, v = item.split("=", 1)
            out[k.strip()] = v.strip()
    return out


def main() -> int:
    ap = argparse.ArgumentParser(description="Append or refresh a Polik Recovery Session timeline")
    ap.add_argument("--session", required=True, help="Session folder")
    ap.add_argument("--refresh", action="store_true", help="Only regenerate timeline JSON/HTML")
    ap.add_argument("--event-type", default="note")
    ap.add_argument("--title", default="Session note")
    ap.add_argument("--status", default="info", choices=sorted(STATUS_LABELS))
    ap.add_argument("--summary", default="")
    ap.add_argument("--actor", default="")
    ap.add_argument("--artifact", action="append", default=[])
    ap.add_argument("--tag", action="append", default=[])
    ap.add_argument("--detail", action="append", default=[], help="key=value; can repeat")
    ap.add_argument("--details-json", help="JSON object with additional event details")
    args = ap.parse_args()

    session = Path(args.session).expanduser().resolve()
    if not session.is_dir():
        raise SystemExit(f"Session not found: {session}")
    if args.refresh:
        payload = write_timeline_files(session)
        dashboard = {}
        if write_session_dashboard:
            try:
                dashboard = write_session_dashboard(session)
            except Exception as exc:
                dashboard = {"error": str(exc)}
        print(json.dumps({"ok": True, "refreshed": True, "timeline": str(timeline_dir(session) / TIMELINE_HTML), "dashboard": dashboard, "event_count": payload.get("event_count", 0)}, ensure_ascii=False, indent=2))
        return 0

    details = parse_key_values(args.detail)
    if args.details_json:
        parsed = json.loads(args.details_json)
        if isinstance(parsed, dict):
            details.update(parsed)
    event = append_event(
        session,
        event_type=args.event_type,
        title=args.title,
        status=args.status,
        summary=args.summary,
        actor=args.actor,
        artifacts=args.artifact,
        tags=args.tag,
        details=details,
    )
    dashboard = {}
    if write_session_dashboard:
        try:
            dashboard = write_session_dashboard(session)
        except Exception as exc:
            dashboard = {"error": str(exc)}
    print(json.dumps({"ok": True, "event": event, "timeline": str(timeline_dir(session) / TIMELINE_HTML), "dashboard": dashboard}, ensure_ascii=False, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
