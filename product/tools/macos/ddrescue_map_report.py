#!/usr/bin/env python3
"""Parse GNU ddrescue map files and produce JSON/HTML recovery imaging reports.

The map file is the most important artifact for failing disks: it tells us which
ranges were copied, which ranges are still not tried, and which ranges are known
bad. This tool turns that map into something a user, Codex, or a technician can
read before deciding whether to continue, retry, or stop.
"""
from __future__ import annotations

import argparse
import datetime as dt
import html
import json
import math
import os
from pathlib import Path
from typing import Any

STATUS_META = {
    "+": {"label": "已完成", "class": "finished", "severity": "good", "meaning": "ddrescue 已成功複製這段資料。"},
    "-": {"label": "壞區", "class": "bad", "severity": "danger", "meaning": "這段資料目前讀不到，通常代表壞軌或讀取失敗。"},
    "?": {"label": "未嘗試", "class": "nontried", "severity": "warn", "meaning": "這段尚未讀取，通常可續跑。"},
    "*": {"label": "待刮取", "class": "nonscraped", "severity": "warn", "meaning": "ddrescue 尚未進入細緻刮取階段，建議視磁碟狀況再 retry。"},
    "/": {"label": "待修剪", "class": "nontrimmed", "severity": "warn", "meaning": "尚未 trim，通常代表第一輪尚未完整整理壞區邊界。"},
}


def now_iso() -> str:
    return dt.datetime.now(dt.timezone.utc).astimezone().isoformat(timespec="seconds")


def parse_int(value: str) -> int:
    value = value.strip()
    if value.lower().startswith("0x"):
        return int(value, 16)
    return int(value, 10)


def human_size(n: int) -> str:
    if n < 1024:
        return f"{n} B"
    units = ["KB", "MB", "GB", "TB", "PB"]
    value = float(n)
    for unit in units:
        value /= 1024.0
        if abs(value) < 1024.0:
            return f"{value:.2f} {unit}"
    return f"{value:.2f} EB"


def shell_quote(value: str) -> str:
    return "'" + value.replace("'", "'\"'\"'") + "'"


def parse_map(map_path: Path) -> dict[str, Any]:
    segments: list[dict[str, Any]] = []
    comments: list[str] = []
    current_status: dict[str, Any] | None = None

    for raw_line in map_path.read_text(encoding="utf-8", errors="replace").splitlines():
        line = raw_line.strip()
        if not line:
            continue
        if line.startswith("#"):
            comments.append(line)
            continue
        parts = line.split()
        if len(parts) < 3:
            continue

        # ddrescue header line looks like: current_pos current_status current_pass
        # Segment lines look like: pos size status, with size being numeric.
        try:
            first = parse_int(parts[0])
        except ValueError:
            continue

        try:
            second = parse_int(parts[1])
            status = parts[2][0]
            if status not in STATUS_META:
                continue
            segments.append({
                "offset": first,
                "size": second,
                "end": first + second,
                "status": status,
                "status_label": STATUS_META[status]["label"],
            })
        except ValueError:
            # Most likely the current status line.
            current_status = {
                "current_position": first,
                "current_status": parts[1],
                "current_pass": parts[2],
            }

    total = sum(s["size"] for s in segments)
    by_status: dict[str, int] = {k: 0 for k in STATUS_META}
    for s in segments:
        by_status[s["status"]] = by_status.get(s["status"], 0) + int(s["size"])

    stats = []
    for status, meta in STATUS_META.items():
        size = by_status.get(status, 0)
        stats.append({
            "status": status,
            "label": meta["label"],
            "severity": meta["severity"],
            "size_bytes": size,
            "size_human": human_size(size),
            "percent": round((size / total * 100.0), 6) if total else 0.0,
            "meaning": meta["meaning"],
        })

    bad = by_status.get("-", 0)
    pending = by_status.get("?", 0) + by_status.get("*", 0) + by_status.get("/", 0)
    finished = by_status.get("+", 0)
    if total == 0:
        health = "unknown"
        recommendation = "map 檔沒有可解析區段，請確認這是 GNU ddrescue mapfile。"
    elif bad == 0 and pending == 0:
        health = "complete"
        recommendation = "成像看起來完整。建議先驗證映像檔，再掃描 .img，不要再反覆讀原始磁碟。"
    elif pending > 0 and bad == 0:
        health = "resume_recommended"
        recommendation = "還有未嘗試/未刮取區段。建議先續跑 ddrescue，不要直接進入深度掃描。"
    elif bad > 0 and finished / total >= 0.98:
        health = "mostly_imaged_with_bad_areas"
        recommendation = "大部分已完成，但仍有壞區。建議視資料重要性做有限 retry，避免過度折磨原碟。"
    else:
        health = "high_risk"
        recommendation = "壞區或未完成比例偏高。建議先檢查線材、供電與外接盒，再用 ddrescue 續跑；不要把這個 image 當完整成功。"

    return {
        "schema": "polik.recovery.ddrescue_map_report.v1",
        "created_at": now_iso(),
        "map_file": str(map_path),
        "comments": comments[:50],
        "current_status": current_status,
        "total_bytes": total,
        "total_human": human_size(total),
        "finished_bytes": finished,
        "finished_human": human_size(finished),
        "bad_bytes": bad,
        "bad_human": human_size(bad),
        "pending_bytes": pending,
        "pending_human": human_size(pending),
        "finished_percent": round((finished / total * 100.0), 6) if total else 0.0,
        "bad_percent": round((bad / total * 100.0), 6) if total else 0.0,
        "pending_percent": round((pending / total * 100.0), 6) if total else 0.0,
        "stats": stats,
        "segment_count": len(segments),
        "segments": segments,
        "largest_bad_ranges": sorted([s for s in segments if s["status"] == "-"], key=lambda x: x["size"], reverse=True)[:50],
        "health": health,
        "recommendation": recommendation,
    }


def compress_segments_for_html(segments: list[dict[str, Any]], max_items: int = 420) -> list[dict[str, Any]]:
    if len(segments) <= max_items:
        return segments
    total = sum(int(s["size"]) for s in segments)
    if total <= 0:
        return segments[:max_items]
    bucket_size = max(1, math.ceil(total / max_items))
    buckets: list[dict[str, Any]] = []
    current = {"offset": 0, "size": 0, "end": 0, "status": "+", "status_label": "已完成", "weights": {}}

    for seg in segments:
        remaining = int(seg["size"])
        seg_offset = int(seg["offset"])
        while remaining > 0:
            room = bucket_size - current["size"]
            take = min(room, remaining)
            if current["size"] == 0:
                current["offset"] = seg_offset
            current["size"] += take
            current["end"] = seg_offset + take
            current["weights"][seg["status"]] = current["weights"].get(seg["status"], 0) + take
            remaining -= take
            seg_offset += take
            if current["size"] >= bucket_size:
                status = max(current["weights"], key=current["weights"].get)
                current["status"] = status
                current["status_label"] = STATUS_META[status]["label"]
                buckets.append(current)
                current = {"offset": 0, "size": 0, "end": 0, "status": "+", "status_label": "已完成", "weights": {}}
    if current["size"] > 0:
        status = max(current["weights"], key=current["weights"].get)
        current["status"] = status
        current["status_label"] = STATUS_META[status]["label"]
        buckets.append(current)
    return buckets


def build_commands(manifest: dict[str, Any] | None, map_path: Path) -> dict[str, str]:
    if not manifest:
        return {}
    manifest_path = Path(str(manifest.get("manifest") or "")).expanduser()
    if not manifest_path.exists():
        # If manifest did not store its own path, use map sibling guess only for display.
        manifest_path = map_path.with_suffix(".manifest.json")
    m = shell_quote(str(manifest_path))
    return {
        "refresh_map_report": f"python3 tools/macos/ddrescue_map_report.py --manifest {m}",
        "resume_fast_pass": f"python3 tools/macos/create_source_image.py --resume-manifest {m} --method ddrescue",
        "retry_bad_areas": f"python3 tools/macos/create_source_image.py --resume-manifest {m} --method ddrescue --retry-bad --retries 3",
        "retry_bad_areas_reverse": f"python3 tools/macos/create_source_image.py --resume-manifest {m} --method ddrescue --retry-bad --reverse --retries 3",
        "retry_direct_io": f"python3 tools/macos/create_source_image.py --resume-manifest {m} --method ddrescue --retry-bad --direct --retries 3",
        "verify_after_retry": f"python3 tools/macos/verify_source_image.py --manifest {m}",
    }


def write_html(report: dict[str, Any], html_path: Path) -> None:
    stats = report.get("stats", [])
    segments = compress_segments_for_html(report.get("segments", []))
    total = max(1, int(report.get("total_bytes") or 1))
    status_class = {
        "+": "finished",
        "-": "bad",
        "?": "nontried",
        "*": "nonscraped",
        "/": "nontrimmed",
    }
    bar = "".join(
        f"<div class='seg {status_class.get(s['status'], 'unknown')}' title='{html.escape(s['status_label'])} | {human_size(int(s['size']))} | offset {s['offset']}' style='width:{max(0.12, int(s['size']) / total * 100):.6f}%'></div>"
        for s in segments
    )
    stat_cards = "".join(
        f"<div class='card {html.escape(str(x['severity']))}'><div>{html.escape(x['label'])}</div><strong>{x['percent']:.4f}%</strong><small>{html.escape(x['size_human'])}</small><p>{html.escape(x['meaning'])}</p></div>"
        for x in stats
    )
    bad_rows = "".join(
        f"<tr><td>0x{int(r['offset']):x}</td><td>{human_size(int(r['size']))}</td><td>0x{int(r['end']):x}</td></tr>"
        for r in report.get("largest_bad_ranges", [])[:30]
    ) or "<tr><td colspan='3'>沒有壞區紀錄。</td></tr>"
    commands = report.get("commands", {})
    command_rows = "".join(
        f"<tr><th>{html.escape(k)}</th><td><code>{html.escape(v)}</code></td></tr>"
        for k, v in commands.items()
    ) or "<tr><td colspan='2'>沒有 manifest，無法產生續跑指令。</td></tr>"
    raw_summary = {k: v for k, v in report.items() if k not in {"segments", "comments", "largest_bad_ranges"}}
    html_doc = f"""<!doctype html>
<html lang="zh-Hant"><head><meta charset="utf-8"><title>Polik ddrescue Map Report</title>
<style>
body{{font-family:-apple-system,BlinkMacSystemFont,'Noto Sans TC',sans-serif;margin:32px;background:#f8fafc;color:#1a202c;line-height:1.55}}
.badge{{display:inline-block;padding:6px 10px;border-radius:999px;background:#e2e8f0;font-weight:700}}
.badge.complete{{background:#c6f6d5}}.badge.resume_recommended{{background:#fefcbf}}.badge.mostly_imaged_with_bad_areas{{background:#fed7aa}}.badge.high_risk{{background:#fed7d7}}
.notice{{padding:14px 16px;border-radius:12px;background:#fffaf0;border:1px solid #f6ad55;margin:14px 0}}
.grid{{display:grid;grid-template-columns:repeat(auto-fit,minmax(190px,1fr));gap:12px;margin:16px 0}}
.card{{background:#fff;border:1px solid #e2e8f0;border-radius:14px;padding:14px}}.card.good{{border-color:#9ae6b4}}.card.warn{{border-color:#f6e05e}}.card.danger{{border-color:#fc8181}}
.card strong{{display:block;font-size:24px;margin:4px 0}}.card small{{color:#4a5568}}.card p{{font-size:13px;color:#4a5568;margin-bottom:0}}
.mapbar{{display:flex;width:100%;height:36px;overflow:hidden;border-radius:10px;border:1px solid #cbd5e0;background:#fff;margin:12px 0}}
.seg{{height:100%;min-width:1px}}.finished{{background:#48bb78}}.bad{{background:#e53e3e}}.nontried{{background:#a0aec0}}.nonscraped{{background:#ed8936}}.nontrimmed{{background:#ecc94b}}.unknown{{background:#805ad5}}
table{{border-collapse:collapse;width:100%;background:#fff;margin:16px 0}}th,td{{border:1px solid #e2e8f0;padding:10px;text-align:left;vertical-align:top}}th{{background:#edf2f7;width:230px}}code,pre{{font-family:ui-monospace,SFMono-Regular,Menlo,monospace;white-space:pre-wrap;word-break:break-all}}
</style></head><body>
<h1>Polik ddrescue Map Report <span class="badge {html.escape(str(report.get('health')))}">{html.escape(str(report.get('health')))}</span></h1>
<div class="notice"><strong>建議：</strong>{html.escape(str(report.get('recommendation')))}</div>
<div class="grid">{stat_cards}</div>
<h2>成像區段視覺化</h2><div class="mapbar">{bar}</div>
<p>綠色=完成，紅色=壞區，灰色=未嘗試，橘色/黃色=待刮取或待修剪。若紅色很多，請不要一直掃原碟；應先完成/續跑 image，再掃 .img。</p>
<h2>建議續跑 / 重試指令</h2><table>{command_rows}</table>
<h2>最大壞區範圍</h2><table><tr><th>offset</th><th>size</th><th>end</th></tr>{bad_rows}</table>
<h2>摘要 JSON</h2><pre>{html.escape(json.dumps(raw_summary, ensure_ascii=False, indent=2))}</pre>
</body></html>"""
    html_path.write_text(html_doc, encoding="utf-8")


def main() -> int:
    parser = argparse.ArgumentParser(description="Create a readable report from a GNU ddrescue map file.")
    parser.add_argument("--map", dest="map_file", help="ddrescue .map file")
    parser.add_argument("--manifest", help="Polik source image manifest; preferred because it can generate retry commands")
    parser.add_argument("--out", help="JSON output path")
    parser.add_argument("--html", dest="html_path", help="HTML output path")
    parser.add_argument("--report-only", action="store_true", help="Always exit 0 after writing reports, regardless of map health.")
    args = parser.parse_args()

    manifest: dict[str, Any] | None = None
    manifest_path: Path | None = None
    if args.manifest:
        manifest_path = Path(args.manifest).expanduser().resolve()
        if not manifest_path.exists():
            raise SystemExit(f"manifest not found: {manifest_path}")
        manifest = json.loads(manifest_path.read_text(encoding="utf-8"))

    map_path = Path(args.map_file or (manifest or {}).get("map_file") or "").expanduser()
    if not map_path.is_absolute() and manifest_path:
        map_path = (manifest_path.parent / map_path).resolve()
    else:
        map_path = map_path.resolve()
    if not map_path.exists():
        raise SystemExit(f"map file not found: {map_path}")

    report = parse_map(map_path)
    if manifest is not None:
        report["manifest"] = str(manifest_path)
        report["source"] = manifest.get("source")
        report["output_image"] = manifest.get("output_image")
        report["method"] = manifest.get("method")
    report["commands"] = build_commands(manifest, map_path)

    out_path = Path(args.out or (manifest_path.with_suffix(".map_report.json") if manifest_path else map_path.with_suffix(".report.json")))
    html_path = Path(args.html_path or (manifest_path.with_suffix(".map_report.html") if manifest_path else map_path.with_suffix(".report.html")))
    out_path.write_text(json.dumps(report, ensure_ascii=False, indent=2), encoding="utf-8")
    write_html(report, html_path)
    print(json.dumps({k: v for k, v in report.items() if k != "segments"}, ensure_ascii=False, indent=2))
    if args.report_only:
        return 0
    return 0 if report.get("health") in {"complete", "resume_recommended", "mostly_imaged_with_bad_areas"} else 2


if __name__ == "__main__":
    raise SystemExit(main())
