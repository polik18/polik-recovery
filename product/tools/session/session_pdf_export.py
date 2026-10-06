#!/usr/bin/env python3
"""Export Polik Recovery session reports to PDF.

v2.6 keeps HTML/JSON as the source of truth, then creates compact PDF deliverables:
- customer_summary.pdf: redacted, plain-language customer report
- engineer_handoff.pdf: technical handoff report

The script uses ReportLab and built-in CID CJK fonts. On macOS/Codex, validate the
rendered PDF before customer delivery.
"""
from __future__ import annotations

import argparse
import datetime as dt
import json
import sys
from pathlib import Path
from typing import Any

try:
    from reportlab.lib import colors
    from reportlab.lib.enums import TA_LEFT
    from reportlab.lib.pagesizes import A4
    from reportlab.lib.styles import ParagraphStyle, getSampleStyleSheet
    from reportlab.lib.units import mm
    from reportlab.platypus import SimpleDocTemplate, Paragraph, Spacer, Table, TableStyle, PageBreak
    from reportlab.pdfbase import pdfmetrics
    from reportlab.pdfbase.cidfonts import UnicodeCIDFont
    from reportlab.pdfbase.ttfonts import TTFont
except Exception as exc:  # pragma: no cover
    raise SystemExit("ReportLab is required for PDF export. Install with: python3 -m pip install reportlab\n" + str(exc))

REPORT_DIR = "09_reports"
AUDIT_DIR = "10_audit"
SCHEMA = "polik.recovery.session_pdf_export.v2.6"


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


def register_cjk_font() -> str:
    """Register a CJK-capable font and return the ReportLab font name.

    Prefer embedded TrueType fonts so exported PDFs render correctly in common viewers and
    in automated render checks. The script does not ship fonts; it only uses fonts already
    installed on the operator machine. Set POLIK_PDF_FONT=/path/to/font.ttf if needed.
    """
    import os
    candidates = []
    env_font = os.environ.get("POLIK_PDF_FONT")
    if env_font:
        candidates.append(Path(env_font).expanduser())
    candidates.extend([
        # macOS common locations
        Path("/Library/Fonts/Arial Unicode.ttf"),
        Path("/System/Library/Fonts/Supplemental/Arial Unicode.ttf"),
        Path("/System/Library/Fonts/Supplemental/Songti.ttc"),
        Path("/System/Library/Fonts/Supplemental/Heiti SC.ttc"),
        Path("/System/Library/Fonts/PingFang.ttc"),
        # Linux CI / Codex / common package locations
        Path("/usr/share/fonts/truetype/arphic/uming.ttc"),
        Path("/usr/share/fonts/truetype/wqy/wqy-microhei.ttc"),
        Path("/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf"),
    ])
    for path in candidates:
        if not path.exists():
            continue
        for subfont in [0, 1, 2, 3, 4, 5]:
            try:
                name = f"PolikCJK{subfont}"
                pdfmetrics.registerFont(TTFont(name, str(path), subfontIndex=subfont))
                return name
            except Exception:
                continue
    # Last resort: built-in CID fonts. Some non-macOS renderers may lack CMaps, so
    # validation should catch this and prompt the operator to set POLIK_PDF_FONT.
    for name in ["MSung-Light", "STSong-Light", "HeiseiMin-W3"]:
        try:
            pdfmetrics.registerFont(UnicodeCIDFont(name))
            return name
        except Exception:
            continue
    return "Helvetica"


FONT_NAME = register_cjk_font()


def make_styles() -> dict[str, ParagraphStyle]:
    base = getSampleStyleSheet()
    return {
        "title": ParagraphStyle("PolikTitle", parent=base["Title"], fontName=FONT_NAME, fontSize=20, leading=26, spaceAfter=12, alignment=TA_LEFT),
        "h1": ParagraphStyle("PolikH1", parent=base["Heading1"], fontName=FONT_NAME, fontSize=15, leading=20, spaceBefore=10, spaceAfter=6),
        "h2": ParagraphStyle("PolikH2", parent=base["Heading2"], fontName=FONT_NAME, fontSize=12, leading=16, spaceBefore=8, spaceAfter=4),
        "body": ParagraphStyle("PolikBody", parent=base["BodyText"], fontName=FONT_NAME, fontSize=10, leading=15, spaceAfter=5),
        "small": ParagraphStyle("PolikSmall", parent=base["BodyText"], fontName=FONT_NAME, fontSize=8, leading=12, textColor=colors.HexColor("#475569")),
        "code": ParagraphStyle("PolikCode", parent=base["Code"], fontName=FONT_NAME, fontSize=8, leading=11, backColor=colors.HexColor("#f1f5f9")),
    }


def para(text: Any, style: ParagraphStyle) -> Paragraph:
    text = "" if text is None else str(text)
    text = text.replace("&", "&amp;").replace("<", "&lt;").replace(">", "&gt;").replace("\n", "<br/>")
    return Paragraph(text, style)


def table(rows: list[list[Any]], widths: list[float] | None = None) -> Table:
    styles = make_styles()
    data = [[para(cell, styles["small"] if r else styles["body"]) for cell in row] for r, row in enumerate(rows)]
    t = Table(data, colWidths=widths, repeatRows=1)
    t.setStyle(TableStyle([
        ("BACKGROUND", (0, 0), (-1, 0), colors.HexColor("#e2e8f0")),
        ("TEXTCOLOR", (0, 0), (-1, 0), colors.HexColor("#0f172a")),
        ("FONTNAME", (0, 0), (-1, -1), FONT_NAME),
        ("GRID", (0, 0), (-1, -1), 0.25, colors.HexColor("#cbd5e1")),
        ("VALIGN", (0, 0), (-1, -1), "TOP"),
        ("LEFTPADDING", (0, 0), (-1, -1), 6),
        ("RIGHTPADDING", (0, 0), (-1, -1), 6),
        ("TOPPADDING", (0, 0), (-1, -1), 5),
        ("BOTTOMPADDING", (0, 0), (-1, -1), 5),
    ]))
    return t


def load_inputs(session: Path) -> dict[str, Any]:
    reports = session / REPORT_DIR
    audit = session / AUDIT_DIR
    manifest = load_json(session / "session_manifest.json") or {}
    customer = load_json(reports / "customer_summary.json") or {}
    engineer = load_json(reports / "engineer_handoff.json") or {}
    audit_json = load_json(audit / "sample_audit.json") or {}
    return {"manifest": manifest, "customer": customer, "engineer": engineer, "audit": audit_json}


def doc_template(path: Path, title: str) -> SimpleDocTemplate:
    path.parent.mkdir(parents=True, exist_ok=True)
    return SimpleDocTemplate(str(path), pagesize=A4, rightMargin=15*mm, leftMargin=15*mm, topMargin=14*mm, bottomMargin=14*mm, title=title)


def customer_story(session: Path, inputs: dict[str, Any]) -> list[Any]:
    styles = make_styles()
    customer = inputs["customer"]
    audit = inputs["audit"]
    counts = customer.get("counts") or customer.get("summary_counts") or {}
    outcome = customer.get("outcome", {}) if isinstance(customer.get("outcome"), dict) else {"label": customer.get("outcome", "")}
    audit_summary = audit.get("summary", {}) if isinstance(audit.get("summary"), dict) else {}
    story: list[Any] = []
    story.append(para("資料救援案件摘要（客戶版）", styles["title"]))
    story.append(para(f"產生時間：{now_iso()}", styles["small"]))
    story.append(Spacer(1, 4*mm))
    story.append(table([
        ["項目", "內容"],
        ["案件編號", customer.get("case_id") or inputs["manifest"].get("case_id", session.name)],
        ["目前判定", outcome.get("label", "")],
        ["完整檔案", f"{counts.get('complete_files', 0)} 個 / {human_bytes(counts.get('complete_bytes', 0))}"],
        ["可疑或部分檔案", f"{counts.get('suspect_files', 0)} 個 / {human_bytes(counts.get('suspect_bytes', 0))}"],
        ["略過項目", counts.get("skipped_files", 0)],
        ["掃描項目", counts.get("scan_entries", counts.get("scan_reports", 0))],
    ], [38*mm, 132*mm]))
    story.append(para("重要說明", styles["h1"]))
    notes = customer.get("plain_language_notes") or customer.get("notes") or []
    if not notes:
        notes = ["完整檔案與可疑檔案已分流；可疑/partial 檔案不等於完整成功。"]
    for note in notes:
        story.append(para(f"• {note}", styles["body"]))
    story.append(para("人工抽查紀錄", styles["h1"]))
    if audit_summary:
        story.append(table([
            ["項目", "數量"],
            ["抽查樣本", audit_summary.get("total", 0)],
            ["已抽查", audit_summary.get("checked", 0)],
            ["可正常開啟", audit_summary.get("opens_ok", 0)],
            ["可開啟但有警告", audit_summary.get("warning", 0)],
            ["無法開啟/類型錯誤", audit_summary.get("failed", 0)],
            ["尚未抽查", audit_summary.get("pending", 0)],
        ], [60*mm, 110*mm]))
        if int(audit_summary.get("pending", 0) or 0) > 0:
            story.append(para("提醒：仍有檔案尚未人工抽查，交付前建議至少抽查完整區與可疑區樣本。", styles["body"]))
    else:
        story.append(para("尚未產生抽查紀錄。建議先執行 make session-audit，再依抽查結果產生 PDF。", styles["body"]))
    story.append(para("建議下一步", styles["h1"]))
    for step in customer.get("next_steps", []) or ["請抽查完整檔案是否能正常開啟，並保留 session 資料夾作為交付紀錄。"]:
        story.append(para(f"• {step}", styles["body"]))
    story.append(Spacer(1, 5*mm))
    story.append(para("本報告由 Polik Recovery Session v2.6 產生；PDF 是交付摘要，JSON/HTML 與原始 Session 仍為工程追蹤依據。", styles["small"]))
    return story


def engineer_story(session: Path, inputs: dict[str, Any]) -> list[Any]:
    styles = make_styles()
    engineer = inputs["engineer"]
    manifest = inputs["manifest"]
    audit = inputs["audit"]
    counts = engineer.get("summary_counts") or engineer.get("counts") or {}
    story: list[Any] = []
    story.append(para("Polik Recovery 工程交接報告", styles["title"]))
    story.append(para(f"產生時間：{now_iso()}", styles["small"]))
    story.append(table([
        ["欄位", "內容"],
        ["Case ID", engineer.get("case_id") or manifest.get("case_id", session.name)],
        ["Session", engineer.get("session_dir") or str(session)],
        ["Source", engineer.get("source") or manifest.get("source", "")],
        ["Mode", engineer.get("mode") or manifest.get("mode", "")],
        ["Outcome", (engineer.get("outcome") or {}).get("label") if isinstance(engineer.get("outcome"), dict) else engineer.get("outcome", "")],
        ["Complete", f"{counts.get('complete_files', 0)} / {human_bytes(counts.get('complete_bytes', 0))}"],
        ["Suspect", f"{counts.get('suspect_files', 0)} / {human_bytes(counts.get('suspect_bytes', 0))}"],
        ["Quality", f"PASS {counts.get('quality_pass', 0)} / FAIL {counts.get('quality_fail', 0)}"],
    ], [38*mm, 132*mm]))
    story.append(para("人工抽查摘要", styles["h1"]))
    summary = audit.get("summary", {}) if isinstance(audit.get("summary"), dict) else {}
    story.append(table([
        ["項目", "數量"],
        ["樣本", summary.get("total", 0)],
        ["已檢查", summary.get("checked", 0)],
        ["可正常開啟", summary.get("opens_ok", 0)],
        ["有警告", summary.get("warning", 0)],
        ["失敗", summary.get("failed", 0)],
        ["待抽查", summary.get("pending", 0)],
    ], [50*mm, 120*mm]))
    items = [i for i in audit.get("items", []) if isinstance(i, dict)][:24]
    if items:
        story.append(para("抽查明細（最多前 24 筆）", styles["h2"]))
        rows = [["類別", "檔案", "大小", "結果", "備註"]]
        for item in items:
            rows.append([item.get("category", ""), item.get("relative_path", ""), item.get("size_human", ""), item.get("result_label", item.get("result", "")), item.get("notes", "")])
        story.append(table(rows, [22*mm, 65*mm, 22*mm, 32*mm, 29*mm]))
    story.append(PageBreak())
    story.append(para("macOS + Codex 待驗證", styles["h1"]))
    validation = engineer.get("macos_codex_validation", {}) if isinstance(engineer.get("macos_codex_validation"), dict) else {}
    note = validation.get("note") or engineer.get("note") or "此專案曾在非 macOS、無 Rust/Cargo/Tauri CLI 的環境修改；請於 macOS + Codex 執行實機驗證。"
    story.append(para(note, styles["body"]))
    commands = validation.get("commands") if isinstance(validation, dict) else engineer.get("macos_codex_validation")
    if not commands:
        commands = ["make codex-bootstrap", "make codex-validation-quick", "make codex-validation-full", "make quality-gate-all", "cargo tauri build"]
    for cmd in commands:
        story.append(para(str(cmd), styles["code"]))
    checklist = engineer.get("engineering_checklist", [])
    if checklist:
        story.append(para("工程檢查清單", styles["h1"]))
        rows = [["項目", "狀態", "動作"]]
        for item in checklist:
            rows.append([item.get("id", ""), "OK" if item.get("ok") else "待處理", item.get("action", "")])
        story.append(table(rows, [40*mm, 25*mm, 105*mm]))
    story.append(para("PDF 交付提醒", styles["h1"]))
    story.append(para("PDF 是快照；工程判斷仍應以 session_manifest.json、dashboard、timeline、sample_audit.json 與原始報告為準。", styles["body"]))
    return story


def export_pdf(session: Path, audience: str) -> dict[str, str]:
    reports = session / REPORT_DIR
    reports.mkdir(parents=True, exist_ok=True)
    inputs = load_inputs(session)
    written: dict[str, str] = {}
    if audience in {"customer", "both"}:
        path = reports / "customer_summary.pdf"
        doc = doc_template(path, "資料救援案件摘要（客戶版）")
        doc.build(customer_story(session, inputs))
        written["customer_pdf"] = str(path)
    if audience in {"engineer", "both"}:
        path = reports / "engineer_handoff.pdf"
        doc = doc_template(path, "Polik Recovery 工程交接報告")
        doc.build(engineer_story(session, inputs))
        written["engineer_pdf"] = str(path)
    update_manifest(session, written)
    return written


def update_manifest(session: Path, written: dict[str, str]) -> None:
    manifest_path = session / "session_manifest.json"
    manifest = load_json(manifest_path)
    if not isinstance(manifest, dict):
        return
    manifest["schema"] = "polik.recovery.session.v2.6"
    manifest["version"] = "2.6-report-pdf-sample-audit"
    manifest.setdefault("folders", {})["reports"] = str(session / REPORT_DIR)
    manifest["pdf_reports"] = {"generated_at": now_iso(), "files": written, "font": FONT_NAME}
    timeline = manifest.setdefault("timeline", [])
    if isinstance(timeline, list):
        timeline.append({
            "event_id": f"pdf_report_{int(dt.datetime.now().timestamp())}",
            "event_type": "pdf_report_generated",
            "title": "產生 PDF 交付報告",
            "status": "success",
            "summary": f"已產生 PDF：{', '.join(written.keys())}",
            "created_at": now_iso(),
            "actor": "session_pdf_export.py",
            "artifacts": list(written.values()),
        })
    write_json(manifest_path, manifest)
    try:
        from session_timeline import write_timeline_files
        write_timeline_files(session, manifest)
    except Exception:
        pass


def main() -> int:
    parser = argparse.ArgumentParser(description="Export Polik Recovery Session handoff reports to PDF")
    parser.add_argument("--session", type=Path, required=True)
    parser.add_argument("--audience", choices=["customer", "engineer", "both"], default="both")
    parser.add_argument("--ensure-audit", action="store_true", help="Create sample audit if 10_audit/sample_audit.json does not exist")
    args = parser.parse_args()
    session = args.session.expanduser().resolve()
    if not (session / "session_manifest.json").exists():
        raise SystemExit(f"Missing session_manifest.json: {session}")
    if args.ensure_audit and not (session / AUDIT_DIR / "sample_audit.json").exists():
        from session_sample_audit import init_or_update_audit
        init_or_update_audit(session, sample_size=5)
    written = export_pdf(session, args.audience)
    print(json.dumps({"ok": True, "schema": SCHEMA, "session": str(session), "written": written, "font": FONT_NAME}, ensure_ascii=False, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
