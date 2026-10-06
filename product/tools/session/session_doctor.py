#!/usr/bin/env python3
"""Validate a Polik Recovery session folder."""
from __future__ import annotations

import argparse
import json
from pathlib import Path
from typing import Any

REQUIRED_DIRS = [
    "00_intake",
    "01_source_image",
    "02_scan_reports",
    "03_recovery_output/recovered",
    "03_recovery_output/suspect/possible",
    "03_recovery_output/suspect/partial",
    "03_recovery_output/suspect/fragments",
    "04_quality_reports",
    "05_logs",
    "06_handoff",
    "07_timeline",
    "08_dashboard",
    "09_reports",
    "10_audit",
    "11_delivery",
]


def check_session(session: Path) -> dict[str, Any]:
    checks = []
    manifest = session / "session_manifest.json"
    checks.append({"id": "manifest", "ok": manifest.exists(), "detail": str(manifest)})
    if manifest.exists():
        try:
            data = json.loads(manifest.read_text(encoding="utf-8"))
            checks.append({"id": "manifest_json", "ok": str(data.get("schema", "")).startswith("polik.recovery.session.v2"), "detail": data.get("schema")})
        except Exception as exc:
            checks.append({"id": "manifest_json", "ok": False, "detail": str(exc)})
    for rel in REQUIRED_DIRS:
        p = session / rel
        checks.append({"id": f"dir:{rel}", "ok": p.is_dir(), "detail": str(p)})
    index = session / "session_index.html"
    checks.append({"id": "session_index", "ok": index.exists(), "detail": str(index)})
    warnings = []
    artifact_count = 0
    if manifest.exists():
        try:
            data = json.loads(manifest.read_text(encoding="utf-8"))
            artifacts = data.get("artifacts", [])
            artifact_count = len(artifacts) if isinstance(artifacts, list) else 0
        except Exception:
            artifact_count = 0
    checks.append({"id": "artifact_count", "ok": True, "detail": str(artifact_count)})
    if not list((session / "01_source_image").glob("*.manifest.json")):
        warnings.append("尚未匯入 image manifest；若來源是實體磁碟，建議先建立 image。")
    if not list((session / "02_scan_reports").glob("*")):
        warnings.append("尚未看到掃描報告。")
    if not list((session / "03_recovery_output").glob("**/recovery_report.*")):
        warnings.append("尚未看到還原報告。")
    dashboard_json = session / "08_dashboard" / "session_dashboard.json"
    dashboard_html = session / "08_dashboard" / "session_dashboard.html"
    checks.append({"id": "dashboard_json", "ok": dashboard_json.exists(), "detail": str(dashboard_json)})
    checks.append({"id": "dashboard_html", "ok": dashboard_html.exists(), "detail": str(dashboard_html)})
    if not dashboard_html.exists():
        warnings.append("尚未產生 v2.4 Session Dashboard；可用 make session-dashboard 或 App 更新 Dashboard。")
    dashboard_json = session / "08_dashboard" / "session_dashboard.json"
    dashboard_html = session / "08_dashboard" / "session_dashboard.html"
    checks.append({"id": "dashboard_json", "ok": dashboard_json.exists(), "detail": str(dashboard_json)})
    checks.append({"id": "dashboard_html", "ok": dashboard_html.exists(), "detail": str(dashboard_html)})
    timeline_json = session / "07_timeline" / "session_timeline.json"
    timeline_html = session / "07_timeline" / "session_timeline.html"
    checks.append({"id": "timeline_json", "ok": timeline_json.exists(), "detail": str(timeline_json)})
    checks.append({"id": "timeline_html", "ok": timeline_html.exists(), "detail": str(timeline_html)})
    timeline_count = 0
    if manifest.exists():
        try:
            data = json.loads(manifest.read_text(encoding="utf-8"))
            timeline = data.get("timeline", [])
            timeline_count = len(timeline) if isinstance(timeline, list) else 0
        except Exception:
            timeline_count = 0
    checks.append({"id": "timeline_count", "ok": True, "detail": str(timeline_count)})
    if artifact_count == 0:
        warnings.append("尚未透過 v2.2 artifact 匯入流程登錄外部報告；可用 make session-import 或 App 匯入。")
    if timeline_count == 0:
        warnings.append("尚未看到 v2.3 timeline 事件；可用 make session-timeline 或 App 時間線功能補記錄。")
    if not dashboard_html.exists():
        warnings.append("尚未產生 v2.4 Session Dashboard；可用 make session-dashboard 或 App 內重新整理 Dashboard。")

    reports_dir = session / "09_reports"
    customer_html = reports_dir / "customer_summary.html"
    engineer_html = reports_dir / "engineer_handoff.html"
    customer_pdf = reports_dir / "customer_summary.pdf"
    engineer_pdf = reports_dir / "engineer_handoff.pdf"
    checks.append({"id": "customer_summary_html", "ok": customer_html.exists(), "detail": str(customer_html)})
    checks.append({"id": "engineer_handoff_html", "ok": engineer_html.exists(), "detail": str(engineer_html)})
    checks.append({"id": "customer_summary_pdf", "ok": customer_pdf.exists(), "detail": str(customer_pdf)})
    checks.append({"id": "engineer_handoff_pdf", "ok": engineer_pdf.exists(), "detail": str(engineer_pdf)})
    if not customer_html.exists() or not engineer_html.exists():
        warnings.append("尚未產生 v2.5 交接摘要 / 客戶版報告；可用 make session-report 或 App 內產生報告。")
    if not customer_pdf.exists() or not engineer_pdf.exists():
        warnings.append("尚未產生 v2.6 PDF 交付報告；可用 make session-report-pdf。")
    audit_dir = session / "10_audit"
    audit_json = audit_dir / "sample_audit.json"
    audit_html = audit_dir / "sample_audit.html"
    checks.append({"id": "sample_audit_json", "ok": audit_json.exists(), "detail": str(audit_json)})
    checks.append({"id": "sample_audit_html", "ok": audit_html.exists(), "detail": str(audit_html)})
    if not audit_json.exists() or not audit_html.exists():
        warnings.append("尚未建立 v2.6 人工抽查紀錄；可用 make session-audit。")

    delivery_dir = session / "11_delivery"
    delivery_packages = delivery_dir / "packages"
    delivery_zips = list(delivery_packages.glob("*.zip")) if delivery_packages.exists() else []
    delivery_manifests = list(delivery_dir.glob("**/delivery_manifest.json")) if delivery_dir.exists() else []
    checks.append({"id": "delivery_dir", "ok": delivery_dir.exists(), "detail": str(delivery_dir)})
    checks.append({"id": "delivery_package_count", "ok": True, "detail": str(len(delivery_zips))})
    checks.append({"id": "delivery_manifest_count", "ok": True, "detail": str(len(delivery_manifests))})
    if not delivery_zips:
        warnings.append("尚未產生 v2.7 客戶交付 zip；可用 make session-delivery-package。")

    ok = all(c["ok"] for c in checks)
    return {"ok": ok, "checks": checks, "warnings": warnings}


def main() -> int:
    parser = argparse.ArgumentParser(description="Check a Polik Recovery session folder.")
    parser.add_argument("--session", type=Path, required=True)
    parser.add_argument("--out", type=Path)
    args = parser.parse_args()
    result = check_session(args.session.expanduser().resolve())
    text = json.dumps(result, ensure_ascii=False, indent=2)
    if args.out:
        args.out.parent.mkdir(parents=True, exist_ok=True)
        args.out.write_text(text, encoding="utf-8")
    print(text)
    return 0 if result["ok"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
