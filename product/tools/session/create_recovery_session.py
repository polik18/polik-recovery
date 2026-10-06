#!/usr/bin/env python3
"""Create or update a Polik Recovery case/session folder.

The goal is to keep one recovery case self-contained:
- source/intake notes
- image manifest and ddrescue map report
- scan reports
- recovery output
- quality/recovery reports
- logs and Codex handoff artifacts
"""
from __future__ import annotations

import argparse
import datetime as dt
import html
import json
import os
import platform
import re
import shutil
from pathlib import Path
from typing import Any

try:
    from session_timeline import append_event as append_timeline_event, write_timeline_files
except Exception:  # pragma: no cover - keeps older standalone use resilient
    append_timeline_event = None
    write_timeline_files = None

try:
    from session_dashboard import build_dashboard as build_session_dashboard, write_dashboard as save_session_dashboard
except Exception:  # pragma: no cover
    build_session_dashboard = None
    save_session_dashboard = None

ROOT = Path(__file__).resolve().parents[2]

DIRS = {
    "intake": "00_intake",
    "source_image": "01_source_image",
    "scan_reports": "02_scan_reports",
    "recovery_output": "03_recovery_output",
    "quality_reports": "04_quality_reports",
    "logs": "05_logs",
    "handoff": "06_handoff",
    "timeline": "07_timeline",
    "dashboard": "08_dashboard",
    "reports": "09_reports",
    "audit": "10_audit",
    "delivery": "11_delivery",
}

RECOVERY_SUBDIRS = [
    "recovered",
    "suspect/possible",
    "suspect/partial",
    "suspect/fragments",
    "skipped",
]


def now_iso() -> str:
    return dt.datetime.now(dt.timezone.utc).astimezone().isoformat(timespec="seconds")


def stamp() -> str:
    return dt.datetime.now().strftime("%Y%m%d-%H%M%S")


def slug(value: str) -> str:
    value = value.strip() or "case"
    value = value.replace("/dev/", "")
    value = re.sub(r"[^A-Za-z0-9._\-\u4e00-\u9fff]+", "_", value)
    return value.strip("._-") or "case"


def default_out_root() -> Path:
    home = Path.home()
    desktop = home / "Desktop"
    return (desktop if desktop.exists() else home) / "Polik-Recovery-Sessions"


def rel_or_abs(path: Path, base: Path) -> str:
    try:
        return str(path.resolve().relative_to(base.resolve()))
    except Exception:
        return str(path)


def copy_if_present(src: str | None, dst_dir: Path, *, label: str, copied: list[dict[str, Any]], warnings: list[str]) -> str | None:
    if not src:
        return None
    p = Path(src).expanduser()
    if not p.exists():
        warnings.append(f"{label} 不存在，未匯入：{p}")
        return None
    dst_dir.mkdir(parents=True, exist_ok=True)
    dst = dst_dir / p.name
    if p.resolve() != dst.resolve():
        shutil.copy2(p, dst)
    copied.append({"label": label, "source": str(p), "stored_as": str(dst)})
    return str(dst)


def detect_existing_artifacts(session_dir: Path) -> dict[str, list[str]]:
    patterns = {
        "image_manifests": "01_source_image/**/*.manifest.json",
        "map_reports": "01_source_image/**/*map_report*.json",
        "scan_reports": "02_scan_reports/**/*",
        "recovery_reports": "03_recovery_output/**/recovery_report.*",
        "quality_reports": "04_quality_reports/**/*quality_report.*",
        "logs": "05_logs/**/*.log",
        "handoff_artifacts": "06_handoff/**/*.zip",
    }
    result: dict[str, list[str]] = {}
    for key, pattern in patterns.items():
        result[key] = [rel_or_abs(p, session_dir) for p in sorted(session_dir.glob(pattern)) if p.is_file()]
    return result


def write_readme(session_dir: Path, manifest: dict[str, Any]) -> None:
    text = f"""# Polik Recovery Session

此資料夾是一個完整救援案件資料夾，請盡量把同一次救援的 image、map、scan、recovery、quality、log 與 Codex 驗證輸出都放在這裡。

## Case

- Case ID：`{manifest['case_id']}`
- Created：`{manifest['created_at']}`
- Source：`{manifest.get('source') or '-'}`
- Mode：`{manifest.get('mode') or '-'}`
- Operator：`{manifest.get('operator') or '-'}`

## Folder Layout

```text
00_intake/             來源資訊、客戶/案件備註、風險評估
01_source_image/       .img、.manifest.json、ddrescue .map、map report
02_scan_reports/       掃描輸出、VFS、CLI JSON/TXT
03_recovery_output/    recovered/ 與 suspect/ 隔離結果
04_quality_reports/    quality gate 報告與評估結果
05_logs/               CLI、Tauri、Codex、ddrescue log
06_handoff/            給 Codex/工程師/客戶的交接 zip 與摘要
07_timeline/           操作時間線 JSON/HTML，記錄建立、匯入、掃描、還原、驗證、交接
08_dashboard/          Session Dashboard JSON/HTML，集中顯示 image、map、scan、recovery、quality 狀態
09_reports/            v2.5 客戶版摘要與工程交接報告，v2.6 PDF 交付報告
10_audit/              v2.6 人工抽查紀錄 JSON/HTML/CSV/Markdown
11_delivery/           v2.7 客戶交付包、清單、checksum 與 zip
```

## Suggested Next Steps

1. 若來源是 `/dev/disk*`，先建立 image，不要反覆掃原碟。
2. 產生 ddrescue map report，確認壞區與未完成比例。
3. 驗證 image 後，把掃描來源改成 `.img`。
4. 還原結果只把完整檔放 `recovered/`，可疑結果放 `suspect/`。
5. 每次掃描、還原、匯入、驗證後重新產生 `session_index.html`、`07_timeline/session_timeline.html`、`08_dashboard/session_dashboard.html`，最後產出 `09_reports/customer_summary.html`、`09_reports/customer_summary.pdf`、`10_audit/sample_audit.html`，並用 v2.7 建立 `11_delivery/` 客戶交付 zip。

## Session Commands

```bash
make session-index SESSION={session_dir}
make session-dashboard SESSION={session_dir}
make codex-artifacts SESSION={session_dir}
```
"""
    (session_dir / "SESSION_README.md").write_text(text, encoding="utf-8")


def write_html(session_dir: Path, manifest: dict[str, Any]) -> None:
    artifacts = manifest.get("artifacts", {})
    rows = []
    for key, value in manifest.items():
        if key == "artifacts":
            continue
        if isinstance(value, (dict, list)):
            value = json.dumps(value, ensure_ascii=False, indent=2)
        rows.append(f"<tr><th>{html.escape(str(key))}</th><td><pre>{html.escape(str(value))}</pre></td></tr>")

    artifact_blocks = []
    if isinstance(artifacts, list):
        rows2 = []
        for item in artifacts[-30:]:
            if isinstance(item, dict):
                rows2.append(f"<tr><td>{html.escape(str(item.get('kind','')))}</td><td><code>{html.escape(str(item.get('destination_path','')))}</code></td><td>{html.escape(str(item.get('imported_at_unix','')))}</td></tr>")
        artifact_blocks.append("<section><h2>imported_artifacts</h2><table><tr><th>kind</th><th>destination</th><th>imported_at</th></tr>" + ("".join(rows2) or "<tr><td colspan='3' class='muted'>尚無</td></tr>") + "</table></section>")
    elif isinstance(artifacts, dict):
        for key, values in artifacts.items():
            lis = "".join(f"<li><code>{html.escape(str(v))}</code></li>" for v in values) or "<li class='muted'>尚無</li>"
            artifact_blocks.append(f"<section><h2>{html.escape(key)}</h2><ul>{lis}</ul></section>")

    timeline = manifest.get("timeline", []) if isinstance(manifest.get("timeline"), list) else []
    timeline_rows = []
    for item in timeline[-20:]:
        if isinstance(item, dict):
            timeline_rows.append(f"<tr><td>{html.escape(str(item.get('created_at','')))}</td><td>{html.escape(str(item.get('status','')))}</td><td>{html.escape(str(item.get('title','')))}</td><td>{html.escape(str(item.get('summary','')))}</td></tr>")
    artifact_blocks.append("<section><h2>timeline</h2><p><a href='07_timeline/session_timeline.html'>開啟完整時間線</a></p><table><tr><th>time</th><th>status</th><th>title</th><th>summary</th></tr>" + ("".join(timeline_rows) or "<tr><td colspan='4' class='muted'>尚無</td></tr>") + "</table></section>")
    artifact_blocks.append("<section><h2>dashboard</h2><p><a href='08_dashboard/session_dashboard.html'>開啟案件總覽 Dashboard</a></p><p class='muted'>v2.4 會集中顯示 image/map、scan、recovery、quality 與最近 timeline。</p></section>"
                           "<section><h2>handoff_reports</h2><p><a href='09_reports/customer_summary.html'>客戶版摘要</a> ｜ <a href='09_reports/engineer_handoff.html'>工程交接版</a> ｜ <a href='09_reports/customer_summary.pdf'>客戶版 PDF</a></p><p class='muted'>v2.5 會把 Dashboard、timeline 與還原結果整理成可交付報告；v2.6 可輸出 PDF。</p></section><section><h2>sample_audit</h2><p><a href='10_audit/sample_audit.html'>開啟人工抽查紀錄</a></p><p class='muted'>v2.6 記錄抽查哪些檔案、能否開啟、備註與 hash。</p></section>")

    doc = f"""<!doctype html>
<html lang="zh-Hant"><head><meta charset="utf-8"><title>Polik Recovery Session {html.escape(manifest['case_id'])}</title>
<style>
body{{font-family:-apple-system,BlinkMacSystemFont,'Noto Sans TC',sans-serif;line-height:1.6;margin:32px;background:#f8fafc;color:#1a202c}}
.card,section{{background:#fff;border:1px solid #e2e8f0;border-radius:14px;padding:16px;margin:14px 0;box-shadow:0 6px 20px rgba(15,23,42,.06)}}
h1{{margin:0 0 8px}}.muted{{color:#718096}}table{{border-collapse:collapse;width:100%;background:#fff}}th,td{{border:1px solid #e2e8f0;padding:10px;text-align:left;vertical-align:top}}th{{width:220px;background:#edf2f7}}pre,code{{font-family:ui-monospace,SFMono-Regular,Menlo,monospace;white-space:pre-wrap}}.ok{{color:#2f855a}}.warn{{color:#b7791f}}
</style></head><body>
<h1>Polik Recovery Session</h1>
<p class="muted">案件資料夾索引。請把同一次救援的 image、map、scan、recovery、quality、log 都收進這裡。</p>
<div class="card"><table>{''.join(rows)}</table></div>
{''.join(artifact_blocks)}
</body></html>"""
    (session_dir / "session_index.html").write_text(doc, encoding="utf-8")


def build_manifest(args: argparse.Namespace, session_dir: Path, copied: list[dict[str, Any]], warnings: list[str]) -> dict[str, Any]:
    manifest_path = session_dir / "session_manifest.json"
    existing: dict[str, Any] = {}
    if manifest_path.exists() and not args.reset:
        try:
            existing = json.loads(manifest_path.read_text(encoding="utf-8"))
        except Exception:
            warnings.append("既有 session_manifest.json 無法解析，已重新建立。")

    case_id = existing.get("case_id") or args.case_id or session_dir.name
    existing_artifacts = existing.get("artifacts") if isinstance(existing.get("artifacts"), list) else []
    detected_inventory = detect_existing_artifacts(session_dir)
    manifest = {
        "schema": "polik.recovery.session.v2.7",
        "case_id": case_id,
        "created_at": existing.get("created_at") or now_iso(),
        "updated_at": now_iso(),
        "version": "2.7-delivery-package",
        "platform": platform.platform(),
        "operator": args.operator or existing.get("operator") or "",
        "source": args.source or existing.get("source") or "",
        "mode": args.mode or existing.get("mode") or "",
        "session_dir": str(session_dir),
        "notes": args.notes or existing.get("notes") or "",
        "environment_note": "目前此專案曾在非 macOS、無 Rust/Cargo/Tauri CLI 的環境修改；實機驗證需依 docs/plans/macos-codex-validation-plan.md 於 macOS + Codex 執行。",
        "folders": {key: str(session_dir / name) for key, name in DIRS.items()},
        "copied_artifacts": (existing.get("copied_artifacts") or []) + copied,
        "warnings": (existing.get("warnings") or []) + warnings,
        "artifacts": existing_artifacts,
        "timeline": existing.get("timeline") if isinstance(existing.get("timeline"), list) else [],
        "timeline_count": len(existing.get("timeline", [])) if isinstance(existing.get("timeline"), list) else 0,
        "dashboard": existing.get("dashboard", {}),
        "artifact_inventory": detected_inventory,
    }
    return manifest


def main() -> int:
    parser = argparse.ArgumentParser(description="Create/update a Polik Recovery session folder.")
    parser.add_argument("--out-root", type=Path, default=default_out_root(), help="案件根目錄，例如 /Volumes/SAFE/Polik-Sessions")
    parser.add_argument("--case-id", help="案件代號；預設依時間與來源產生")
    parser.add_argument("--session", type=Path, help="既有 session 資料夾；提供後會更新索引")
    parser.add_argument("--source", help="來源磁碟或 image 路徑")
    parser.add_argument("--mode", help="預計掃描模式，例如 ntfs/exfat/fat32/apfs/carver")
    parser.add_argument("--operator", help="操作者/交接者")
    parser.add_argument("--notes", help="案件備註")
    parser.add_argument("--image-manifest", help="匯入來源 image manifest json")
    parser.add_argument("--map-report", help="匯入 ddrescue map report json/html")
    parser.add_argument("--scan-report", action="append", default=[], help="匯入掃描報告，可重複")
    parser.add_argument("--recovery-report", action="append", default=[], help="匯入還原報告，可重複")
    parser.add_argument("--quality-report", action="append", default=[], help="匯入品質報告，可重複")
    parser.add_argument("--log", action="append", default=[], help="匯入 log，可重複")
    parser.add_argument("--handoff", action="append", default=[], help="匯入交接 zip/檔案，可重複")
    parser.add_argument("--reset", action="store_true", help="忽略既有 manifest 重新建立")
    args = parser.parse_args()

    if args.session:
        session_dir = args.session.expanduser().resolve()
    else:
        base = slug(args.case_id or f"polik-session-{slug(args.source or 'source')}-{stamp()}")
        session_dir = args.out_root.expanduser().resolve() / base

    for name in DIRS.values():
        (session_dir / name).mkdir(parents=True, exist_ok=True)
    for sub in RECOVERY_SUBDIRS:
        (session_dir / DIRS["recovery_output"] / sub).mkdir(parents=True, exist_ok=True)

    copied: list[dict[str, Any]] = []
    warnings: list[str] = []
    copy_if_present(args.image_manifest, session_dir / DIRS["source_image"], label="image_manifest", copied=copied, warnings=warnings)
    copy_if_present(args.map_report, session_dir / DIRS["source_image"], label="map_report", copied=copied, warnings=warnings)
    for item in args.scan_report:
        copy_if_present(item, session_dir / DIRS["scan_reports"], label="scan_report", copied=copied, warnings=warnings)
    for item in args.recovery_report:
        copy_if_present(item, session_dir / DIRS["recovery_output"], label="recovery_report", copied=copied, warnings=warnings)
    for item in args.quality_report:
        copy_if_present(item, session_dir / DIRS["quality_reports"], label="quality_report", copied=copied, warnings=warnings)
    for item in args.log:
        copy_if_present(item, session_dir / DIRS["logs"], label="log", copied=copied, warnings=warnings)
    for item in args.handoff:
        copy_if_present(item, session_dir / DIRS["handoff"], label="handoff", copied=copied, warnings=warnings)

    is_new_session = not (session_dir / "session_manifest.json").exists() or args.reset
    manifest = build_manifest(args, session_dir, copied, warnings)
    (session_dir / "session_manifest.json").write_text(json.dumps(manifest, ensure_ascii=False, indent=2), encoding="utf-8")
    if append_timeline_event and (is_new_session or not manifest.get("timeline")):
        append_timeline_event(
            session_dir,
            event_type="session_created",
            title="建立救援 Session",
            status="success",
            summary="建立案件資料夾與基本目錄結構。",
            actor=args.operator or "create_recovery_session.py",
            details={"source": args.source or "", "mode": args.mode or ""},
        )
        manifest = json.loads((session_dir / "session_manifest.json").read_text(encoding="utf-8"))
    elif write_timeline_files:
        write_timeline_files(session_dir, manifest)
    if build_session_dashboard and save_session_dashboard:
        try:
            save_session_dashboard(session_dir, build_session_dashboard(session_dir))
            manifest = json.loads((session_dir / "session_manifest.json").read_text(encoding="utf-8"))
        except Exception as exc:
            warnings.append(f"dashboard 產生失敗：{exc}")
    write_readme(session_dir, manifest)
    write_html(session_dir, manifest)

    print(str(session_dir))
    print(str(session_dir / "session_index.html"))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
