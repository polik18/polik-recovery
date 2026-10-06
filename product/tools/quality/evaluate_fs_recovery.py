#!/usr/bin/env python3
"""Evaluate filesystem-level scan/extract outputs for Polik Recovery v0.7."""
from __future__ import annotations

import argparse
import hashlib
import html
import json
from datetime import datetime, timezone
from pathlib import Path
from typing import Any


def load_json(path: Path) -> dict[str, Any]:
    return json.loads(path.read_text(encoding="utf-8"))


def sha256_file(path: Path) -> str:
    h = hashlib.sha256()
    with path.open("rb") as f:
        for chunk in iter(lambda: f.read(1024 * 1024), b""):
            h.update(chunk)
    return h.hexdigest()


def pct(value: float) -> str:
    return f"{value * 100:.1f}%"


def entry_matches(actual: dict[str, Any], expected: dict[str, Any]) -> bool:
    return (
        str(actual.get("path")) == str(expected.get("path"))
        and str(actual.get("status")) == str(expected.get("status"))
        and str(actual.get("kind")) == str(expected.get("kind"))
        and int(actual.get("size") or 0) == int(expected.get("size") or 0)
    )


def evaluate(expected_manifest: dict[str, Any], output_root: Path) -> dict[str, Any]:
    scan_checks: list[dict[str, Any]] = []
    extract_checks: list[dict[str, Any]] = []

    for fixture in expected_manifest.get("fixtures", []):
        fs = fixture["fs"]
        scan_json_path = output_root / "scans" / f"{fs}_scan.json"
        scan_ok = scan_json_path.exists()
        scan_entries = []
        if scan_ok:
            try:
                scan_entries = load_json(scan_json_path).get("entries", [])
            except Exception as exc:  # noqa: BLE001 - report detail
                scan_ok = False
                scan_entries = []
                scan_checks.append({"fs": fs, "passed": False, "reason": f"scan JSON parse failed: {exc}"})

        for expected in fixture.get("expectedEntries", []):
            matched = any(entry_matches(actual, expected) for actual in scan_entries)
            scan_checks.append({
                "fs": fs,
                "path": expected.get("path"),
                "expectedStatus": expected.get("status"),
                "expectedSize": expected.get("size"),
                "passed": bool(scan_ok and matched),
                "reason": "matched" if matched else "missing or mismatched scan entry",
            })

        for item in fixture.get("extracts", []):
            name = item["name"]
            output_file = output_root / "extracts" / fs / name
            exists = output_file.exists()
            actual_sha = sha256_file(output_file) if exists else None
            actual_bytes = output_file.stat().st_size if exists else None
            passed = bool(
                exists
                and actual_sha == item.get("expectedSha256")
                and actual_bytes == item.get("expectedBytes")
            )
            extract_checks.append({
                "fs": fs,
                "name": name,
                "output": str(output_file),
                "expectedSha256": item.get("expectedSha256"),
                "actualSha256": actual_sha,
                "expectedBytes": item.get("expectedBytes"),
                "actualBytes": actual_bytes,
                "passed": passed,
                "reason": "hash and size matched" if passed else "missing output or hash/size mismatch",
            })

    thresholds = expected_manifest.get("thresholds", {})
    scan_passed = sum(1 for item in scan_checks if item.get("passed"))
    extract_passed = sum(1 for item in extract_checks if item.get("passed"))
    scan_ratio = scan_passed / max(1, len(scan_checks))
    extract_ratio = extract_passed / max(1, len(extract_checks))
    scan_failures = len(scan_checks) - scan_passed
    extract_failures = len(extract_checks) - extract_passed

    rules = [
        {
            "name": "scan_entry_match_ratio",
            "actual": scan_ratio,
            "operator": ">=",
            "required": thresholds.get("minimumScanEntryMatchRatio", 1.0),
            "passed": scan_ratio >= thresholds.get("minimumScanEntryMatchRatio", 1.0),
        },
        {
            "name": "extract_hash_match_ratio",
            "actual": extract_ratio,
            "operator": ">=",
            "required": thresholds.get("minimumExtractHashMatchRatio", 1.0),
            "passed": extract_ratio >= thresholds.get("minimumExtractHashMatchRatio", 1.0),
        },
        {
            "name": "scan_failures",
            "actual": scan_failures,
            "operator": "<=",
            "required": thresholds.get("maximumScanFailures", 0),
            "passed": scan_failures <= thresholds.get("maximumScanFailures", 0),
        },
        {
            "name": "extract_failures",
            "actual": extract_failures,
            "operator": "<=",
            "required": thresholds.get("maximumExtractFailures", 0),
            "passed": extract_failures <= thresholds.get("maximumExtractFailures", 0),
        },
    ]

    return {
        "version": "v0.7",
        "evaluatedAt": datetime.now(timezone.utc).isoformat(),
        "summary": {
            "scanChecks": len(scan_checks),
            "scanPassed": scan_passed,
            "scanFailures": scan_failures,
            "scanEntryMatchRatio": scan_ratio,
            "extractChecks": len(extract_checks),
            "extractPassed": extract_passed,
            "extractFailures": extract_failures,
            "extractHashMatchRatio": extract_ratio,
        },
        "rules": rules,
        "scanChecks": scan_checks,
        "extractChecks": extract_checks,
        "passed": all(rule["passed"] for rule in rules),
    }


def write_html(report: dict[str, Any], path: Path) -> None:
    summary = report["summary"]
    def row_rule(rule: dict[str, Any]) -> str:
        actual = rule["actual"]
        required = rule["required"]
        actual_text = pct(actual) if isinstance(actual, float) else str(actual)
        required_text = pct(required) if isinstance(required, float) else str(required)
        cls = "ok" if rule["passed"] else "bad"
        return f"<tr><td>{html.escape(rule['name'])}</td><td>{html.escape(actual_text)}</td><td>{html.escape(rule['operator'])} {html.escape(required_text)}</td><td class='{cls}'>{'PASS' if rule['passed'] else 'FAIL'}</td></tr>"

    def row_scan(item: dict[str, Any]) -> str:
        cls = "ok" if item["passed"] else "bad"
        return f"<tr><td>{html.escape(item['fs'])}</td><td>{html.escape(str(item.get('path')))}</td><td>{html.escape(str(item.get('expectedStatus')))}</td><td>{html.escape(str(item.get('expectedSize')))}</td><td>{html.escape(item['reason'])}</td><td class='{cls}'>{'PASS' if item['passed'] else 'FAIL'}</td></tr>"

    def row_extract(item: dict[str, Any]) -> str:
        cls = "ok" if item["passed"] else "bad"
        return f"<tr><td>{html.escape(item['fs'])}</td><td>{html.escape(item['name'])}</td><td>{html.escape(str(item.get('expectedBytes')))}</td><td>{html.escape(str(item.get('actualBytes')))}</td><td>{html.escape(str(item.get('actualSha256') or '')[:16])}</td><td>{html.escape(item['reason'])}</td><td class='{cls}'>{'PASS' if item['passed'] else 'FAIL'}</td></tr>"

    status = "PASS" if report["passed"] else "FAIL"
    status_cls = "ok" if report["passed"] else "bad"
    doc = f"""<!doctype html>
<html lang="zh-Hant"><meta charset="utf-8"><title>Polik Recovery v0.7 FS Quality Gate</title>
<style>
body{{font-family:-apple-system,BlinkMacSystemFont,'Noto Sans TC',sans-serif;margin:32px;line-height:1.6;color:#1f2937;background:#f9fafb}}
.card{{border:1px solid #e5e7eb;border-radius:16px;padding:18px;margin:0 0 18px;background:#fff}}
table{{border-collapse:collapse;width:100%;background:#fff}}td,th{{border-top:1px solid #e5e7eb;padding:10px;text-align:left;vertical-align:top}}.ok{{color:#047857;font-weight:700}}.bad{{color:#b91c1c;font-weight:700}}code{{background:#f3f4f6;padding:2px 6px;border-radius:6px}}
</style><body>
<h1>Polik Recovery v0.7 檔案系統品質門檻</h1>
<div class="card"><p><b>結果：</b><span class="{status_cls}">{status}</span></p><p><b>評估時間：</b>{html.escape(report['evaluatedAt'])}</p></div>
<div class="card"><h2>摘要</h2><p>掃描檢查：{summary['scanPassed']}/{summary['scanChecks']}，命中率 <b>{pct(summary['scanEntryMatchRatio'])}</b></p><p>還原檢查：{summary['extractPassed']}/{summary['extractChecks']}，hash 命中率 <b>{pct(summary['extractHashMatchRatio'])}</b></p></div>
<div class="card"><h2>門檻</h2><table><thead><tr><th>項目</th><th>實際</th><th>門檻</th><th>結果</th></tr></thead><tbody>{''.join(row_rule(r) for r in report['rules'])}</tbody></table></div>
<div class="card"><h2>掃描項目</h2><table><thead><tr><th>FS</th><th>路徑</th><th>狀態</th><th>大小</th><th>原因</th><th>結果</th></tr></thead><tbody>{''.join(row_scan(i) for i in report['scanChecks'])}</tbody></table></div>
<div class="card"><h2>還原項目</h2><table><thead><tr><th>FS</th><th>檔名</th><th>預期 bytes</th><th>實際 bytes</th><th>實際 SHA256</th><th>原因</th><th>結果</th></tr></thead><tbody>{''.join(row_extract(i) for i in report['extractChecks'])}</tbody></table></div>
</body></html>"""
    path.write_text(doc, encoding="utf-8")


def main() -> int:
    parser = argparse.ArgumentParser(description="Evaluate v0.7 filesystem quality-gate outputs.")
    parser.add_argument("--expected", default="fixtures/fs_quality/expected_fs_manifest.json")
    parser.add_argument("--output-root", default="target/fs-quality")
    parser.add_argument("--out", default="target/fs-quality/fs_quality_report.json")
    parser.add_argument("--html", default="target/fs-quality/fs_quality_report.html")
    args = parser.parse_args()

    expected = load_json(Path(args.expected))
    report = evaluate(expected, Path(args.output_root))
    out_path = Path(args.out)
    html_path = Path(args.html)
    out_path.parent.mkdir(parents=True, exist_ok=True)
    out_path.write_text(json.dumps(report, ensure_ascii=False, indent=2), encoding="utf-8")
    write_html(report, html_path)

    print(f"fs_quality_gate: {'PASS' if report['passed'] else 'FAIL'}")
    print(f"scan_entry_match_ratio: {pct(report['summary']['scanEntryMatchRatio'])}")
    print(f"extract_hash_match_ratio: {pct(report['summary']['extractHashMatchRatio'])}")
    print(f"report_json: {out_path}")
    print(f"report_html: {html_path}")
    return 0 if report["passed"] else 2


if __name__ == "__main__":
    raise SystemExit(main())
