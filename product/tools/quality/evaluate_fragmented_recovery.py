#!/usr/bin/env python3
"""Evaluate v0.8 fragmentation/partial-recovery fixture outputs."""
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


def parse_stdout(path: Path) -> dict[str, str]:
    data: dict[str, str] = {}
    if not path.exists():
        return data
    for line in path.read_text(encoding="utf-8", errors="replace").splitlines():
        if ":" in line:
            key, value = line.split(":", 1)
            data[key.strip()] = value.strip()
    return data


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
        fixture_id = fixture["fs"]
        scan_json_path = output_root / "scans" / f"{fixture_id}_scan.json"
        scan_ok = scan_json_path.exists()
        scan_entries: list[dict[str, Any]] = []
        if scan_ok:
            try:
                scan_entries = load_json(scan_json_path).get("entries", [])
            except Exception as exc:  # noqa: BLE001
                scan_ok = False
                scan_checks.append({"fixture": fixture_id, "passed": False, "reason": f"scan JSON parse failed: {exc}"})

        for expected in fixture.get("expectedEntries", []):
            matched = any(entry_matches(actual, expected) for actual in scan_entries)
            scan_checks.append({
                "fixture": fixture_id,
                "path": expected.get("path"),
                "expectedStatus": expected.get("status"),
                "expectedSize": expected.get("size"),
                "passed": bool(scan_ok and matched),
                "reason": "matched" if matched else "missing or mismatched scan entry",
            })

        for item in fixture.get("extracts", []):
            name = item["name"]
            output_file = output_root / "extracts" / fixture_id / name
            stdout_file = output_root / "extracts" / fixture_id / f"{name}.stdout.txt"
            stdout = parse_stdout(stdout_file)
            exists = output_file.exists()
            actual_sha = sha256_file(output_file) if exists else None
            actual_bytes = output_file.stat().st_size if exists else None
            expected_quality = item.get("expectedQuality", "complete")
            expected_mode = str(item.get("expectedMode", ""))
            actual_quality = stdout.get("quality", "")
            actual_mode = stdout.get("recovery_mode", "")
            hash_matches = bool(exists and actual_sha == item.get("expectedSha256") and actual_bytes == item.get("expectedBytes"))
            mode_matches = (not expected_mode) or expected_mode in actual_mode

            if expected_quality == "complete":
                passed = hash_matches and actual_quality == "complete" and mode_matches
                reason = "complete hash/mode matched" if passed else "expected complete extract did not match hash, mode, or quality"
            else:
                # For known unsafe fallback/partial cases, the success condition is honest classification,
                # not a false claim of complete recovery. Hash mismatch is acceptable when allowHashMismatch=true.
                allow_hash_mismatch = bool(item.get("allowHashMismatch"))
                hash_condition = True if allow_hash_mismatch else hash_matches
                passed = exists and actual_quality == expected_quality and mode_matches and hash_condition
                reason = "suspect case honestly classified" if passed else "expected suspect/partial classification was not observed"

            extract_checks.append({
                "fixture": fixture_id,
                "name": name,
                "output": str(output_file),
                "expectedQuality": expected_quality,
                "actualQuality": actual_quality,
                "expectedMode": expected_mode,
                "actualMode": actual_mode,
                "expectedSha256": item.get("expectedSha256"),
                "actualSha256": actual_sha,
                "expectedBytes": item.get("expectedBytes"),
                "actualBytes": actual_bytes,
                "hashMatches": hash_matches,
                "passed": passed,
                "reason": reason,
            })

    thresholds = expected_manifest.get("thresholds", {})
    scan_passed = sum(1 for item in scan_checks if item.get("passed"))
    scan_ratio = scan_passed / max(1, len(scan_checks))
    complete_checks = [item for item in extract_checks if item.get("expectedQuality") == "complete"]
    complete_passed = sum(1 for item in complete_checks if item.get("passed"))
    complete_ratio = complete_passed / max(1, len(complete_checks))
    suspect_checks = [item for item in extract_checks if item.get("expectedQuality") != "complete"]
    suspect_passed = sum(1 for item in suspect_checks if item.get("passed"))
    suspect_ratio = suspect_passed / max(1, len(suspect_checks))
    unexpected_complete = sum(1 for item in suspect_checks if item.get("actualQuality") == "complete")
    scan_failures = len(scan_checks) - scan_passed

    rules = [
        {
            "name": "scan_entry_match_ratio",
            "actual": scan_ratio,
            "operator": ">=",
            "required": thresholds.get("minimumScanEntryMatchRatio", 1.0),
            "passed": scan_ratio >= thresholds.get("minimumScanEntryMatchRatio", 1.0),
        },
        {
            "name": "complete_extract_hash_ratio",
            "actual": complete_ratio,
            "operator": ">=",
            "required": thresholds.get("minimumCompleteExtractHashRatio", 1.0),
            "passed": complete_ratio >= thresholds.get("minimumCompleteExtractHashRatio", 1.0),
        },
        {
            "name": "suspect_classification_ratio",
            "actual": suspect_ratio,
            "operator": ">=",
            "required": thresholds.get("minimumSuspectClassificationRatio", 1.0),
            "passed": suspect_ratio >= thresholds.get("minimumSuspectClassificationRatio", 1.0),
        },
        {
            "name": "unexpected_complete_for_suspect",
            "actual": unexpected_complete,
            "operator": "<=",
            "required": thresholds.get("maximumUnexpectedCompleteForSuspect", 0),
            "passed": unexpected_complete <= thresholds.get("maximumUnexpectedCompleteForSuspect", 0),
        },
        {
            "name": "scan_failures",
            "actual": scan_failures,
            "operator": "<=",
            "required": thresholds.get("maximumScanFailures", 0),
            "passed": scan_failures <= thresholds.get("maximumScanFailures", 0),
        },
    ]

    return {
        "version": "v0.8",
        "evaluatedAt": datetime.now(timezone.utc).isoformat(),
        "summary": {
            "scanChecks": len(scan_checks),
            "scanPassed": scan_passed,
            "scanEntryMatchRatio": scan_ratio,
            "completeExtractChecks": len(complete_checks),
            "completeExtractPassed": complete_passed,
            "completeExtractHashRatio": complete_ratio,
            "suspectChecks": len(suspect_checks),
            "suspectPassed": suspect_passed,
            "suspectClassificationRatio": suspect_ratio,
            "unexpectedCompleteForSuspect": unexpected_complete,
        },
        "rules": rules,
        "scanChecks": scan_checks,
        "extractChecks": extract_checks,
        "passed": all(rule["passed"] for rule in rules),
    }


def write_html(report: dict[str, Any], path: Path) -> None:
    summary = report["summary"]

    def fmt_value(value: Any) -> str:
        return pct(value) if isinstance(value, float) else str(value)

    def row_rule(rule: dict[str, Any]) -> str:
        cls = "ok" if rule["passed"] else "bad"
        return f"<tr><td>{html.escape(rule['name'])}</td><td>{html.escape(fmt_value(rule['actual']))}</td><td>{html.escape(rule['operator'])} {html.escape(fmt_value(rule['required']))}</td><td class='{cls}'>{'PASS' if rule['passed'] else 'FAIL'}</td></tr>"

    def row_scan(item: dict[str, Any]) -> str:
        cls = "ok" if item["passed"] else "bad"
        return f"<tr><td>{html.escape(item['fixture'])}</td><td>{html.escape(str(item.get('path')))}</td><td>{html.escape(str(item.get('expectedStatus')))}</td><td>{html.escape(str(item.get('expectedSize')))}</td><td>{html.escape(item['reason'])}</td><td class='{cls}'>{'PASS' if item['passed'] else 'FAIL'}</td></tr>"

    def row_extract(item: dict[str, Any]) -> str:
        cls = "ok" if item["passed"] else "bad"
        return f"<tr><td>{html.escape(item['fixture'])}</td><td>{html.escape(item['name'])}</td><td>{html.escape(str(item.get('expectedQuality')))}</td><td>{html.escape(str(item.get('actualQuality')))}</td><td>{html.escape(str(item.get('expectedMode')))}</td><td>{html.escape(str(item.get('actualMode')))}</td><td>{html.escape(str(item.get('actualBytes')))}</td><td>{html.escape('yes' if item.get('hashMatches') else 'no')}</td><td>{html.escape(item['reason'])}</td><td class='{cls}'>{'PASS' if item['passed'] else 'FAIL'}</td></tr>"

    status = "PASS" if report["passed"] else "FAIL"
    status_cls = "ok" if report["passed"] else "bad"
    doc = f"""<!doctype html>
<html lang="zh-Hant"><meta charset="utf-8"><title>Polik Recovery v0.8 Fragmentation Quality Gate</title>
<style>
body{{font-family:-apple-system,BlinkMacSystemFont,'Noto Sans TC',sans-serif;margin:32px;line-height:1.6;color:#1f2937;background:#f9fafb}}
.card{{border:1px solid #e5e7eb;border-radius:16px;padding:18px;margin:0 0 18px;background:#fff}}
table{{border-collapse:collapse;width:100%;background:#fff}}td,th{{border-top:1px solid #e5e7eb;padding:10px;text-align:left;vertical-align:top}}.ok{{color:#047857;font-weight:700}}.bad{{color:#b91c1c;font-weight:700}}code{{background:#f3f4f6;padding:2px 6px;border-radius:6px}}
</style><body>
<h1>Polik Recovery v0.8 碎片化與 partial recovery 品質門檻</h1>
<div class="card"><p><b>結果：</b><span class="{status_cls}">{status}</span></p><p><b>評估時間：</b>{html.escape(report['evaluatedAt'])}</p></div>
<div class="card"><h2>摘要</h2><p>掃描命中：{summary['scanPassed']}/{summary['scanChecks']}，命中率 <b>{pct(summary['scanEntryMatchRatio'])}</b></p><p>完整還原：{summary['completeExtractPassed']}/{summary['completeExtractChecks']}，hash 命中率 <b>{pct(summary['completeExtractHashRatio'])}</b></p><p>可疑/partial 誠實標記：{summary['suspectPassed']}/{summary['suspectChecks']}，命中率 <b>{pct(summary['suspectClassificationRatio'])}</b></p></div>
<div class="card"><h2>門檻</h2><table><thead><tr><th>項目</th><th>實際</th><th>門檻</th><th>結果</th></tr></thead><tbody>{''.join(row_rule(r) for r in report['rules'])}</tbody></table></div>
<div class="card"><h2>掃描項目</h2><table><thead><tr><th>Fixture</th><th>路徑</th><th>狀態</th><th>大小</th><th>原因</th><th>結果</th></tr></thead><tbody>{''.join(row_scan(i) for i in report['scanChecks'])}</tbody></table></div>
<div class="card"><h2>還原項目</h2><table><thead><tr><th>Fixture</th><th>檔名</th><th>預期品質</th><th>實際品質</th><th>預期模式</th><th>實際模式</th><th>實際 bytes</th><th>Hash 是否吻合</th><th>原因</th><th>結果</th></tr></thead><tbody>{''.join(row_extract(i) for i in report['extractChecks'])}</tbody></table></div>
</body></html>"""
    path.write_text(doc, encoding="utf-8")


def main() -> int:
    parser = argparse.ArgumentParser(description="Evaluate v0.8 fragmentation quality-gate outputs.")
    parser.add_argument("--expected", default="fixtures/fragmented_quality/expected_fragmented_manifest.json")
    parser.add_argument("--output-root", default="target/fragmented-quality")
    parser.add_argument("--out", default="target/fragmented-quality/fragmented_quality_report.json")
    parser.add_argument("--html", default="target/fragmented-quality/fragmented_quality_report.html")
    args = parser.parse_args()

    expected = load_json(Path(args.expected))
    report = evaluate(expected, Path(args.output_root))
    out_path = Path(args.out)
    html_path = Path(args.html)
    out_path.parent.mkdir(parents=True, exist_ok=True)
    out_path.write_text(json.dumps(report, ensure_ascii=False, indent=2), encoding="utf-8")
    write_html(report, html_path)

    print(f"fragmented_quality_gate: {'PASS' if report['passed'] else 'FAIL'}")
    print(f"scan_entry_match_ratio: {pct(report['summary']['scanEntryMatchRatio'])}")
    print(f"complete_extract_hash_ratio: {pct(report['summary']['completeExtractHashRatio'])}")
    print(f"suspect_classification_ratio: {pct(report['summary']['suspectClassificationRatio'])}")
    print(f"report_json: {out_path}")
    print(f"report_html: {html_path}")
    return 0 if report["passed"] else 2


if __name__ == "__main__":
    raise SystemExit(main())
