#!/usr/bin/env python3
"""Generate customer and engineering handoff reports for a Polik Recovery session.

v2.5 turns the session folder into a shareable case packet:
- customer_summary: plain-language result/status report for non-technical recipients
- engineer_handoff: technical report for Codex / developers / recovery engineers

The reports intentionally consume existing session artifacts rather than running recovery again.
"""
from __future__ import annotations

import argparse
import datetime as dt
import html
import json
import re
import sys
from pathlib import Path
from typing import Any

HERE = Path(__file__).resolve().parent
if str(HERE) not in sys.path:
    sys.path.insert(0, str(HERE))
try:
    from session_dashboard import build_dashboard, write_dashboard, human_bytes
except Exception:  # pragma: no cover
    build_dashboard = None
    write_dashboard = None
    def human_bytes(n: Any) -> str:
        try:
            n = float(n or 0)
        except Exception:
            n = 0.0
        units = ["B", "KB", "MB", "GB", "TB"]
        for unit in units:
            if abs(n) < 1024 or unit == units[-1]:
                return f"{n:.0f} {unit}" if unit == "B" else f"{n:.1f} {unit}"
            n /= 1024
        return f"{n:.1f} TB"

REPORT_DIR = "09_reports"
AUDIT_DIR = "10_audit"
CUSTOMER_BASE = "customer_summary"
ENGINEER_BASE = "engineer_handoff"


def now_iso() -> str:
    return dt.datetime.now(dt.timezone.utc).astimezone().isoformat(timespec="seconds")


def load_json(path: Path) -> Any | None:
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except Exception:
        return None


def write_json(path: Path, payload: Any) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(payload, ensure_ascii=False, indent=2), encoding="utf-8")


def count_files(path: Path) -> int:
    if not path.exists():
        return 0
    return sum(1 for p in path.glob("**/*") if p.is_file())


def total_size(path: Path) -> int:
    total = 0
    if not path.exists():
        return 0
    for p in path.glob("**/*"):
        if p.is_file():
            try:
                total += p.stat().st_size
            except OSError:
                pass
    return total


def redact(value: Any, include_paths: bool = False) -> Any:
    if include_paths:
        return value
    if isinstance(value, dict):
        out = {}
        for k, v in value.items():
            kl = str(k).lower()
            if any(token in kl for token in ["path", "dir", "source", "session", "image"]):
                out[k] = redact_path(v)
            else:
                out[k] = redact(v, include_paths=False)
        return out
    if isinstance(value, list):
        return [redact(v, include_paths=False) for v in value]
    if isinstance(value, str):
        return redact_path(value)
    return value


def redact_path(value: Any) -> Any:
    if not isinstance(value, str):
        return value
    text = value
    # Keep file names and public folder labels, hide user/device specifics.
    text = re.sub(r"/Users/[^/]+", "/Users/<user>", text)
    text = re.sub(r"/Volumes/[^/]+", "/Volumes/<volume>", text)
    text = re.sub(r"/dev/disk\d+s?\d*", "/dev/diskX", text)
    return text


def ensure_dashboard(session: Path) -> dict[str, Any]:
    dashboard_json = session / "08_dashboard" / "session_dashboard.json"
    payload = load_json(dashboard_json)
    if isinstance(payload, dict):
        return payload
    if build_dashboard is None:
        return {}
    payload = build_dashboard(session)
    if write_dashboard is not None:
        try:
            write_dashboard(session, payload)
        except Exception:
            pass
    return payload if isinstance(payload, dict) else {}


def load_session(session: Path) -> dict[str, Any]:
    manifest = load_json(session / "session_manifest.json")
    if not isinstance(manifest, dict):
        raise SystemExit(f"Invalid or missing session_manifest.json: {session / 'session_manifest.json'}")
    dashboard = ensure_dashboard(session)
    timeline = manifest.get("timeline", []) if isinstance(manifest.get("timeline"), list) else []
    return {"manifest": manifest, "dashboard": dashboard, "timeline": timeline}


def outcome_label(dashboard: dict[str, Any]) -> tuple[str, str]:
    recovery = dashboard.get("recovery", {}) if isinstance(dashboard.get("recovery"), dict) else {}
    quality = dashboard.get("quality", {}) if isinstance(dashboard.get("quality"), dict) else {}
    scan = dashboard.get("scan", {}) if isinstance(dashboard.get("scan"), dict) else {}
    complete = int(recovery.get("complete_count", 0) or 0)
    suspect = int(recovery.get("suspect_count", 0) or 0)
    failed = int(quality.get("fail_count", 0) or 0)
    scanned = int(scan.get("total_entries", 0) or 0)
    if complete > 0 and suspect == 0 and failed == 0:
        return "可交付：已有完整還原檔案", "success"
    if complete > 0 and suspect > 0:
        return "部分可交付：完整檔案與可疑檔案分流", "warning"
    if complete == 0 and suspect > 0:
        return "僅可疑結果：需要人工檢查或再次掃描", "warning"
    if scanned > 0 and complete == 0:
        return "已掃描但尚未看到完整還原結果", "warning"
    return "資料不足：尚未完成掃描或還原", "danger"


def build_common_payload(session: Path, include_paths: bool) -> dict[str, Any]:
    data = load_session(session)
    manifest = data["manifest"]
    dashboard = data["dashboard"]
    timeline = data["timeline"]
    recovery = dashboard.get("recovery", {}) if isinstance(dashboard.get("recovery"), dict) else {}
    scan = dashboard.get("scan", {}) if isinstance(dashboard.get("scan"), dict) else {}
    source = dashboard.get("source_image", {}) if isinstance(dashboard.get("source_image"), dict) else {}
    quality = dashboard.get("quality", {}) if isinstance(dashboard.get("quality"), dict) else {}
    health = dashboard.get("health", {}) if isinstance(dashboard.get("health"), dict) else {}
    outcome, severity = outcome_label(dashboard)
    audit_payload = load_json(session / AUDIT_DIR / "sample_audit.json")
    audit_summary = audit_payload.get("summary", {}) if isinstance(audit_payload, dict) and isinstance(audit_payload.get("summary"), dict) else {}
    payload = {
        "schema": "polik.recovery.session.handoff.v2.6",
        "generated_at": now_iso(),
        "case_id": manifest.get("case_id", session.name),
        "session_dir": str(session),
        "mode": manifest.get("mode", ""),
        "source": manifest.get("source", ""),
        "outcome": {"label": outcome, "severity": severity},
        "health": health,
        "summary_counts": {
            "source_image_files": int(source.get("file_count", 0) or 0),
            "image_manifests": int(source.get("manifest_count", 0) or 0),
            "map_reports": int(source.get("map_report_count", 0) or 0),
            "scan_reports": int(scan.get("file_count", 0) or 0),
            "scan_entries": int(scan.get("total_entries", 0) or 0),
            "deleted_entries": int(scan.get("deleted_entries", 0) or 0),
            "complete_files": int(recovery.get("complete_count", 0) or 0),
            "complete_bytes": int(recovery.get("complete_bytes", 0) or 0),
            "suspect_files": int(recovery.get("suspect_count", 0) or 0),
            "suspect_bytes": int(recovery.get("suspect_bytes", 0) or 0),
            "skipped_files": int(recovery.get("skipped_count", 0) or 0),
            "quality_pass": int(quality.get("pass_count", 0) or 0),
            "quality_fail": int(quality.get("fail_count", 0) or 0),
            "timeline_events": len(timeline),
        },
        "latest_artifacts": manifest.get("artifacts", [])[-20:] if isinstance(manifest.get("artifacts"), list) else [],
        "recent_timeline": timeline[-12:],
        "next_actions": dashboard.get("next_actions", []),
        "sample_audit": {"summary": audit_summary, "json": str(session / AUDIT_DIR / "sample_audit.json"), "html": str(session / AUDIT_DIR / "sample_audit.html")},
        "dashboard": dashboard,
        "folders": manifest.get("folders", {}),
    }
    return payload if include_paths else redact(payload, include_paths=False)


def customer_payload(session: Path) -> dict[str, Any]:
    p = build_common_payload(session, include_paths=False)
    # Keep customer payload compact and non-technical.
    return {
        "schema": "polik.recovery.customer_summary.v2.6",
        "generated_at": p["generated_at"],
        "case_id": p["case_id"],
        "outcome": p["outcome"],
        "counts": p["summary_counts"],
        "plain_language_notes": customer_notes(p),
        "sample_audit": p.get("sample_audit", {}),
        "next_steps": customer_next_steps(p),
        "recent_timeline": p["recent_timeline"],
    }


def engineer_payload(session: Path, include_paths: bool) -> dict[str, Any]:
    p = build_common_payload(session, include_paths=include_paths)
    p["engineering_checklist"] = engineering_checklist(p)
    p["macos_codex_validation"] = {
        "note": "此專案曾在非 macOS、無 Rust/Cargo/Tauri CLI 的環境修改；搬到 macOS 後請先跑驗證流程。",
        "commands": [
            "make codex-bootstrap",
            "make codex-validation-quick",
            "make codex-validation-full",
            "make quality-gate-all",
            "cargo tauri build",
        ],
    }
    return p


def customer_notes(payload: dict[str, Any]) -> list[str]:
    c = payload.get("counts", payload.get("summary_counts", {}))
    complete = int(c.get("complete_files", 0) or 0)
    suspect = int(c.get("suspect_files", 0) or 0)
    skipped = int(c.get("skipped_files", 0) or 0)
    notes = []
    if complete:
        notes.append(f"目前已有 {complete} 個檔案被歸類為完整可交付，總大小約 {human_bytes(c.get('complete_bytes', 0))}。")
    else:
        notes.append("目前尚未看到完整可交付檔案；可能仍需掃描、重新成像或人工檢查可疑檔案。")
    if suspect:
        notes.append(f"另有 {suspect} 個檔案放在可疑/部分救援區，這些檔案可能可以修復，也可能無法開啟。")
    if skipped:
        notes.append(f"有 {skipped} 個項目被略過，通常代表空白、填充值、格式不完整或風險過高。")
    audit = payload.get("sample_audit", {}).get("summary", {}) if isinstance(payload.get("sample_audit"), dict) else {}
    if audit:
        notes.append(f"已建立人工抽查紀錄：樣本 {audit.get('total', 0)} 筆，已檢查 {audit.get('checked', 0)} 筆，可正常開啟 {audit.get('opens_ok', 0)} 筆。")
        if int(audit.get('pending', 0) or 0) > 0:
            notes.append(f"仍有 {audit.get('pending', 0)} 筆抽查樣本尚未人工確認，交付前建議補查。")
    else:
        notes.append("尚未建立人工抽查紀錄；交付前建議抽查 recovered/ 與 suspect/ 樣本。")
    notes.append("可疑檔案不會混入完整檔案區，避免把殘片誤當成成功救援。")
    return notes


def customer_next_steps(payload: dict[str, Any]) -> list[str]:
    c = payload.get("counts", payload.get("summary_counts", {}))
    actions = []
    if int(c.get("image_manifests", 0) or 0) == 0:
        actions.append("若來源是實體磁碟，建議先建立磁碟映像檔，再對映像檔掃描。")
    if int(c.get("scan_entries", 0) or 0) == 0:
        actions.append("下一步是完成掃描並產生掃描報告。")
    if int(c.get("complete_files", 0) or 0) == 0 and int(c.get("suspect_files", 0) or 0) > 0:
        actions.append("建議先人工抽查可疑檔案，再決定是否進行格式修復或更深層掃描。")
    if not actions:
        actions.append("請抽查完整檔案是否能正常開啟，確認後再交付或備份。")
    return actions


def engineering_checklist(payload: dict[str, Any]) -> list[dict[str, Any]]:
    c = payload.get("summary_counts", {})
    return [
        {"id": "image_manifest", "ok": int(c.get("image_manifests", 0) or 0) > 0, "action": "確認 image manifest 與來源資訊。"},
        {"id": "map_report", "ok": int(c.get("map_reports", 0) or 0) > 0, "action": "若使用 ddrescue，確認 map report 是否有壞區/未完成區段。"},
        {"id": "scan_report", "ok": int(c.get("scan_reports", 0) or 0) > 0, "action": "確認 02_scan_reports 中是否有 VFS/scan JSON。"},
        {"id": "recovery_output", "ok": int(c.get("complete_files", 0) or 0) + int(c.get("suspect_files", 0) or 0) > 0, "action": "確認 recovered/ 與 suspect/ 分流。"},
        {"id": "quality_gate", "ok": int(c.get("quality_fail", 0) or 0) == 0 and int(c.get("quality_pass", 0) or 0) > 0, "action": "若尚未跑品質門檻，請在 macOS + Codex 跑 make quality-gate-all。"},
        {"id": "timeline", "ok": int(c.get("timeline_events", 0) or 0) > 0, "action": "確認每個重要操作已寫入 timeline。"},
    ]


def md_table(rows: list[tuple[str, str]]) -> str:
    out = ["| 項目 | 內容 |", "|---|---|"]
    for a, b in rows:
        out.append(f"| {a} | {str(b).replace('|', '/')} |")
    return "\n".join(out)


def render_customer_markdown(payload: dict[str, Any]) -> str:
    c = payload.get("counts", {})
    rows = [
        ("案件編號", payload.get("case_id", "")),
        ("目前判定", payload.get("outcome", {}).get("label", "")),
        ("完整檔案", f"{c.get('complete_files', 0)} 個 / {human_bytes(c.get('complete_bytes', 0))}"),
        ("可疑或部分檔案", f"{c.get('suspect_files', 0)} 個 / {human_bytes(c.get('suspect_bytes', 0))}"),
        ("略過項目", c.get("skipped_files", 0)),
        ("掃描項目", c.get("scan_entries", 0)),
        ("抽查樣本", payload.get("sample_audit", {}).get("summary", {}).get("total", 0) if isinstance(payload.get("sample_audit"), dict) else 0),
        ("已抽查", payload.get("sample_audit", {}).get("summary", {}).get("checked", 0) if isinstance(payload.get("sample_audit"), dict) else 0),
        ("產生時間", payload.get("generated_at", "")),
    ]
    notes = "\n".join(f"- {x}" for x in payload.get("plain_language_notes", []))
    steps = "\n".join(f"- {x}" for x in payload.get("next_steps", []))
    return f"# 資料救援案件摘要（客戶版）\n\n{md_table(rows)}\n\n## 說明\n\n{notes}\n\n## 建議下一步\n\n{steps}\n"


def render_engineer_markdown(payload: dict[str, Any]) -> str:
    c = payload.get("summary_counts", {})
    rows = [
        ("Case ID", payload.get("case_id", "")),
        ("Session", payload.get("session_dir", "")),
        ("Mode", payload.get("mode", "")),
        ("Source", payload.get("source", "")),
        ("Outcome", payload.get("outcome", {}).get("label", "")),
        ("Health", json.dumps(payload.get("health", {}), ensure_ascii=False)),
        ("Image manifests", c.get("image_manifests", 0)),
        ("Map reports", c.get("map_reports", 0)),
        ("Scan reports", c.get("scan_reports", 0)),
        ("Scan entries", c.get("scan_entries", 0)),
        ("Complete", f"{c.get('complete_files', 0)} / {human_bytes(c.get('complete_bytes', 0))}"),
        ("Suspect", f"{c.get('suspect_files', 0)} / {human_bytes(c.get('suspect_bytes', 0))}"),
        ("Quality", f"PASS {c.get('quality_pass', 0)} / FAIL {c.get('quality_fail', 0)}"),
    ]
    checklist = "\n".join(f"- [{'x' if item.get('ok') else ' '}] {item.get('id')}: {item.get('action')}" for item in payload.get("engineering_checklist", []))
    commands = "\n".join(f"```bash\n{cmd}\n```" for cmd in payload.get("macos_codex_validation", {}).get("commands", []))
    return f"# Polik Recovery Engineer Handoff\n\n{md_table(rows)}\n\n## Checklist\n\n{checklist}\n\n## macOS + Codex Validation\n\n{payload.get('macos_codex_validation', {}).get('note', '')}\n\n{commands}\n"


def render_html(title: str, markdown_text: str, payload: dict[str, Any]) -> str:
    # Tiny markdown renderer for the limited syntax we emit.
    body = html.escape(markdown_text)
    body = re.sub(r"^# (.+)$", r"<h1>\1</h1>", body, flags=re.M)
    body = re.sub(r"^## (.+)$", r"<h2>\1</h2>", body, flags=re.M)
    body = re.sub(r"^- \[x\] (.+)$", r"<li class='ok'>✅ \1</li>", body, flags=re.M)
    body = re.sub(r"^- \[ \] (.+)$", r"<li class='warn'>🟡 \1</li>", body, flags=re.M)
    body = re.sub(r"^- (.+)$", r"<li>\1</li>", body, flags=re.M)
    body = re.sub(r"```bash\n(.+?)\n```", lambda m: f"<pre><code>{m.group(1)}</code></pre>", body, flags=re.S)
    # Convert markdown tables very simply.
    lines = body.splitlines()
    rendered = []
    i = 0
    while i < len(lines):
        if i + 1 < len(lines) and lines[i].startswith("| ") and lines[i + 1].startswith("|---"):
            rendered.append("<table>")
            headers = [x.strip() for x in lines[i].strip("|").split("|")]
            rendered.append("<tr>" + "".join(f"<th>{h}</th>" for h in headers) + "</tr>")
            i += 2
            while i < len(lines) and lines[i].startswith("| "):
                cells = [x.strip() for x in lines[i].strip("|").split("|")]
                rendered.append("<tr>" + "".join(f"<td>{c}</td>" for c in cells) + "</tr>")
                i += 1
            rendered.append("</table>")
            continue
        line = lines[i]
        if line.startswith("<h") or line.startswith("<li") or line.startswith("<pre") or line.startswith("<table") or line.startswith("<tr") or line.startswith("</"):
            rendered.append(line)
        elif line.strip():
            rendered.append(f"<p>{line}</p>")
        i += 1
    return f"""<!doctype html><html lang=\"zh-Hant\"><head><meta charset=\"utf-8\"><title>{html.escape(title)}</title>
<style>body{{font-family:-apple-system,BlinkMacSystemFont,'Noto Sans TC',sans-serif;line-height:1.7;margin:32px;background:#f8fafc;color:#172033}}main{{max-width:980px;margin:auto;background:#fff;border:1px solid #e5e7eb;border-radius:18px;padding:26px;box-shadow:0 12px 30px rgba(15,23,42,.08)}}table{{border-collapse:collapse;width:100%;margin:14px 0}}th,td{{border:1px solid #e5e7eb;padding:10px;text-align:left;vertical-align:top}}th{{background:#f1f5f9}}pre{{background:#0f172a;color:#e2e8f0;padding:14px;border-radius:12px;overflow:auto}}li{{margin:6px 0}}.ok{{color:#166534}}.warn{{color:#92400e}}.muted{{color:#64748b}}</style></head><body><main>{''.join(rendered)}<hr><p class=\"muted\">Generated by Polik Recovery v2.5 Session Handoff Report.</p></main></body></html>"""


def write_reports(session: Path, audience: str, include_paths: bool) -> dict[str, Any]:
    session = session.expanduser().resolve()
    out_dir = session / REPORT_DIR
    out_dir.mkdir(parents=True, exist_ok=True)
    written: dict[str, str] = {}
    if audience in {"customer", "both"}:
        payload = customer_payload(session)
        md = render_customer_markdown(payload)
        write_json(out_dir / f"{CUSTOMER_BASE}.json", payload)
        (out_dir / f"{CUSTOMER_BASE}.md").write_text(md, encoding="utf-8")
        (out_dir / f"{CUSTOMER_BASE}.html").write_text(render_html("資料救援案件摘要（客戶版）", md, payload), encoding="utf-8")
        written["customer_json"] = str(out_dir / f"{CUSTOMER_BASE}.json")
        written["customer_html"] = str(out_dir / f"{CUSTOMER_BASE}.html")
        written["customer_md"] = str(out_dir / f"{CUSTOMER_BASE}.md")
    if audience in {"engineer", "both"}:
        payload = engineer_payload(session, include_paths=include_paths)
        md = render_engineer_markdown(payload)
        write_json(out_dir / f"{ENGINEER_BASE}.json", payload)
        (out_dir / f"{ENGINEER_BASE}.md").write_text(md, encoding="utf-8")
        (out_dir / f"{ENGINEER_BASE}.html").write_text(render_html("Polik Recovery Engineer Handoff", md, payload), encoding="utf-8")
        written["engineer_json"] = str(out_dir / f"{ENGINEER_BASE}.json")
        written["engineer_html"] = str(out_dir / f"{ENGINEER_BASE}.html")
        written["engineer_md"] = str(out_dir / f"{ENGINEER_BASE}.md")

    manifest_path = session / "session_manifest.json"
    manifest = load_json(manifest_path)
    if isinstance(manifest, dict):
        folders = manifest.setdefault("folders", {})
        if isinstance(folders, dict):
            folders["reports"] = str(out_dir)
        manifest["handoff_reports"] = {"generated_at": now_iso(), "folder": str(out_dir), "files": written}
        manifest["version"] = "2.6-report-pdf-sample-audit"
        manifest["schema"] = "polik.recovery.session.v2.6"
        # Append lightweight timeline event without depending on session_timeline.py internals.
        timeline = manifest.setdefault("timeline", [])
        if isinstance(timeline, list):
            timeline.append({
                "event_id": f"handoff_report_{int(dt.datetime.now().timestamp())}",
                "event_type": "handoff_report_generated",
                "title": "產生 Session 交接摘要 / 客戶版報告",
                "status": "success",
                "summary": f"已產生 {audience} 報告。",
                "created_at": now_iso(),
                "actor": "session_handoff_report.py",
                "artifacts": list(written.values()),
            })
        manifest_path.write_text(json.dumps(manifest, ensure_ascii=False, indent=2), encoding="utf-8")
        # Refresh timeline/dashboard if tools are available.
        try:
            from session_timeline import write_timeline_files
            write_timeline_files(session, manifest)
        except Exception:
            pass
        try:
            if build_dashboard is not None and write_dashboard is not None:
                write_dashboard(session, build_dashboard(session))
        except Exception:
            pass
    return {"ok": True, "session": str(session), "report_dir": str(out_dir), "audience": audience, "written": written}


def main() -> int:
    ap = argparse.ArgumentParser(description="Generate Polik Recovery session customer/engineering handoff reports")
    ap.add_argument("--session", required=True, help="Session folder containing session_manifest.json")
    ap.add_argument("--audience", choices=["customer", "engineer", "both"], default="both")
    ap.add_argument("--include-paths", action="store_true", help="Include full paths/device identifiers in engineer report. Customer report is always redacted.")
    args = ap.parse_args()
    result = write_reports(Path(args.session), args.audience, include_paths=args.include_paths)
    print(json.dumps(result, ensure_ascii=False, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
