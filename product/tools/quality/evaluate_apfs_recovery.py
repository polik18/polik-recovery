#!/usr/bin/env python3
"""Evaluate APFS scan/extract outputs for Polik Recovery v1.3."""
from __future__ import annotations

import argparse
import hashlib
import html
import json
import re
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


def parse_stdout_fields(path: Path) -> dict[str, str]:
    fields: dict[str, str] = {}
    if not path.exists():
        return fields
    for line in path.read_text(encoding="utf-8", errors="replace").splitlines():
        if ":" not in line:
            continue
        key, value = line.split(":", 1)
        fields[key.strip()] = value.strip()
    return fields


def bool_text(value: object) -> str:
    return "true" if bool(value) else "false"


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
    quality_checks: list[dict[str, Any]] = []
    bytes_lost_checks: list[dict[str, Any]] = []
    encrypted_hint_checks: list[dict[str, Any]] = []

    for fixture_index, fixture in enumerate(expected_manifest.get("fixtures", [])):
        scan_json_path = output_root / "scans" / f"apfs_{fixture_index}_scan.json"
        scan_ok = scan_json_path.exists()
        scan_entries: list[dict[str, Any]] = []
        if scan_ok:
            try:
                scan_entries = load_json(scan_json_path).get("entries", [])
            except Exception as exc:  # noqa: BLE001 - report detail
                scan_ok = False
                scan_checks.append({"fixture": fixture_index, "passed": False, "reason": f"scan JSON parse failed: {exc}"})

        for expected in fixture.get("expectedEntries", []):
            matched = any(entry_matches(actual, expected) for actual in scan_entries)
            scan_checks.append({
                "fixture": fixture_index,
                "path": expected.get("path"),
                "expectedStatus": expected.get("status"),
                "expectedSize": expected.get("size"),
                "passed": bool(scan_ok and matched),
                "reason": "matched" if matched else "missing or mismatched scan entry",
            })

        for item in fixture.get("extracts", []):
            name = item["name"]
            output_file = output_root / "extracts" / f"apfs_{fixture_index}" / name
            stdout_path = output_root / "extracts" / f"apfs_{fixture_index}" / f"{name}.stdout.txt"
            fields = parse_stdout_fields(stdout_path)
            exists = output_file.exists()
            actual_sha = sha256_file(output_file) if exists else None
            actual_bytes = output_file.stat().st_size if exists else None
            hash_passed = bool(
                exists
                and actual_sha == item.get("expectedSha256")
                and actual_bytes == item.get("expectedBytes")
            )
            extract_checks.append({
                "fixture": fixture_index,
                "name": name,
                "output": str(output_file),
                "expectedSha256": item.get("expectedSha256"),
                "actualSha256": actual_sha,
                "expectedBytes": item.get("expectedBytes"),
                "actualBytes": actual_bytes,
                "passed": hash_passed,
                "reason": "hash and size matched" if hash_passed else "missing output or hash/size mismatch",
            })

            actual_quality = fields.get("quality")
            expected_quality = item.get("expectedQuality")
            quality_passed = actual_quality == expected_quality
            quality_checks.append({
                "fixture": fixture_index,
                "name": name,
                "expected": expected_quality,
                "actual": actual_quality,
                "passed": quality_passed,
                "reason": "quality matched" if quality_passed else "quality mismatch or stdout missing",
            })

            actual_bytes_lost = fields.get("bytes_lost")
            expected_bytes_lost = str(item.get("expectedBytesLost", 0))
            bytes_lost_passed = actual_bytes_lost == expected_bytes_lost
            bytes_lost_checks.append({
                "fixture": fixture_index,
                "name": name,
                "expected": expected_bytes_lost,
                "actual": actual_bytes_lost,
                "passed": bytes_lost_passed,
                "reason": "bytes_lost matched" if bytes_lost_passed else "bytes_lost mismatch or stdout missing",
            })

            actual_hint = (fields.get("encrypted_hint") or "").lower()
            expected_hint = bool_text(item.get("expectedEncryptedHint", False))
            encrypted_hint_passed = actual_hint == expected_hint
            encrypted_hint_checks.append({
                "fixture": fixture_index,
                "name": name,
                "expected": expected_hint,
                "actual": actual_hint,
                "passed": encrypted_hint_passed,
                "reason": "encrypted_hint matched" if encrypted_hint_passed else "encrypted_hint mismatch or stdout missing",
            })

    thresholds = expected_manifest.get("thresholds", {})
    scan_passed = sum(1 for item in scan_checks if item.get("passed"))
    extract_passed = sum(1 for item in extract_checks if item.get("passed"))
    quality_failures = sum(1 for item in quality_checks if not item.get("passed"))
    bytes_lost_failures = sum(1 for item in bytes_lost_checks if not item.get("passed"))
    encrypted_hint_failures = sum(1 for item in encrypted_hint_checks if not item.get("passed"))
    scan_ratio = scan_passed / max(1, len(scan_checks))
    extract_ratio = extract_passed / max(1, len(extract_checks))
    scan_failures = len(scan_checks) - scan_passed
    extract_failures = len(extract_checks) - extract_passed

    rules = [
        {
            "name": "apfs_scan_entry_match_ratio",
            "actual": scan_ratio,
            "operator": ">=",
            "required": thresholds.get("minimumScanEntryMatchRatio", 1.0),
            "passed": scan_ratio >= thresholds.get("minimumScanEntryMatchRatio", 1.0),
        },
        {
            "name": "apfs_extract_hash_match_ratio",
            "actual": extract_ratio,
            "operator": ">=",
            "required": thresholds.get("minimumExtractHashMatchRatio", 1.0),
            "passed": extract_ratio >= thresholds.get("minimumExtractHashMatchRatio", 1.0),
        },
        {
            "name": "apfs_scan_failures",
            "actual": scan_failures,
            "operator": "<=",
            "required": thresholds.get("maximumScanFailures", 0),
            "passed": scan_failures <= thresholds.get("maximumScanFailures", 0),
        },
        {
            "name": "apfs_extract_failures",
            "actual": extract_failures,
            "operator": "<=",
            "required": thresholds.get("maximumExtractFailures", 0),
            "passed": extract_failures <= thresholds.get("maximumExtractFailures", 0),
        },
        {
            "name": "apfs_quality_mismatches",
            "actual": quality_failures,
            "operator": "<=",
            "required": thresholds.get("maximumQualityMismatches", 0),
            "passed": quality_failures <= thresholds.get("maximumQualityMismatches", 0),
        },
        {
            "name": "apfs_bytes_lost_mismatches",
            "actual": bytes_lost_failures,
            "operator": "<=",
            "required": thresholds.get("maximumBytesLostMismatches", 0),
            "passed": bytes_lost_failures <= thresholds.get("maximumBytesLostMismatches", 0),
        },
        {
            "name": "apfs_encrypted_hint_mismatches",
            "actual": encrypted_hint_failures,
            "operator": "<=",
            "required": thresholds.get("maximumEncryptedHintMismatches", 0),
            "passed": encrypted_hint_failures <= thresholds.get("maximumEncryptedHintMismatches", 0),
        },
    ]

    return {
        "version": "v1.3",
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
            "qualityFailures": quality_failures,
            "bytesLostFailures": bytes_lost_failures,
            "encryptedHintFailures": encrypted_hint_failures,
        },
        "rules": rules,
        "scanChecks": scan_checks,
        "extractChecks": extract_checks,
        "qualityChecks": quality_checks,
        "bytesLostChecks": bytes_lost_checks,
        "encryptedHintChecks": encrypted_hint_checks,
        "passed": all(rule["passed"] for rule in rules),
    }


def write_html(report: dict[str, Any], path: Path) -> None:
    summary = report["summary"]

    def rule_row(rule: dict[str, Any]) -> str:
        actual = rule["actual"]
        required = rule["required"]
        actual_text = pct(actual) if isinstance(actual, float) else str(actual)
        required_text = pct(required) if isinstance(required, float) else str(required)
        cls = "ok" if rule["passed"] else "bad"
        return f"<tr><td>{html.escape(rule['name'])}</td><td>{html.escape(actual_text)}</td><td>{html.escape(rule['operator'])} {html.escape(required_text)}</td><td class='{cls}'>{'PASS' if rule['passed'] else 'FAIL'}</td></tr>"

    def scan_row(item: dict[str, Any]) -> str:
        cls = "ok" if item["passed"] else "bad"
        return f"<tr><td>{item.get('fixture')}</td><td>{html.escape(str(item.get('path')))}</td><td>{html.escape(str(item.get('expectedStatus')))}</td><td>{html.escape(str(item.get('expectedSize')))}</td><td>{html.escape(str(item.get('reason')))}</td><td class='{cls}'>{'PASS' if item['passed'] else 'FAIL'}</td></tr>"

    def extract_row(item: dict[str, Any]) -> str:
        cls = "ok" if item["passed"] else "bad"
        return f"<tr><td>{item.get('fixture')}</td><td>{html.escape(str(item.get('name')))}</td><td>{html.escape(str(item.get('expectedBytes')))}</td><td>{html.escape(str(item.get('actualBytes')))}</td><td>{html.escape(str(item.get('actualSha256') or '')[:16])}</td><td>{html.escape(str(item.get('reason')))}</td><td class='{cls}'>{'PASS' if item['passed'] else 'FAIL'}</td></tr>"

    def field_row(item: dict[str, Any]) -> str:
        cls = "ok" if item["passed"] else "bad"
        return f"<tr><td>{item.get('fixture')}</td><td>{html.escape(str(item.get('name')))}</td><td>{html.escape(str(item.get('expected')))}</td><td>{html.escape(str(item.get('actual')))}</td><td>{html.escape(str(item.get('reason')))}</td><td class='{cls}'>{'PASS' if item['passed'] else 'FAIL'}</td></tr>"

    status = "PASS" if report["passed"] else "FAIL"
    status_cls = "ok" if report["passed"] else "bad"
    doc = f"""<!doctype html>
<html lang="zh-Hant"><meta charset="utf-8"><title>Polik Recovery v1.3 APFS Quality Gate</title>
<style>
body{{font-family:-apple-system,BlinkMacSystemFont,'Noto Sans TC',sans-serif;margin:32px;line-height:1.6;color:#1f2937;background:#f9fafb}}
.card{{border:1px solid #e5e7eb;border-radius:16px;padding:18px;margin:0 0 18px;background:#fff}}
table{{border-collapse:collapse;width:100%;background:#fff}}td,th{{border-top:1px solid #e5e7eb;padding:10px;text-align:left;vertical-align:top}}.ok{{color:#047857;font-weight:700}}.bad{{color:#b91c1c;font-weight:700}}code{{background:#f3f4f6;padding:2px 6px;border-radius:6px}}
</style><body>
<h1>Polik Recovery v1.3 APFS 品質門檻</h1>
<div class="card"><p><b>結果：</b><span class="{status_cls}">{status}</span></p><p><b>評估時間：</b>{html.escape(report['evaluatedAt'])}</p></div>
<div class="card"><h2>摘要</h2><p>掃描檢查：{summary['scanPassed']}/{summary['scanChecks']}，命中率 <b>{pct(summary['scanEntryMatchRatio'])}</b></p><p>還原檢查：{summary['extractPassed']}/{summary['extractChecks']}，hash 命中率 <b>{pct(summary['extractHashMatchRatio'])}</b></p><p>品質錯誤：{summary['qualityFailures']}，bytes_lost 錯誤：{summary['bytesLostFailures']}，加密提示錯誤：{summary['encryptedHintFailures']}</p></div>
<div class="card"><h2>門檻</h2><table><thead><tr><th>項目</th><th>實際</th><th>門檻</th><th>結果</th></tr></thead><tbody>{''.join(rule_row(r) for r in report['rules'])}</tbody></table></div>
<div class="card"><h2>掃描項目</h2><table><thead><tr><th>Fixture</th><th>路徑</th><th>狀態</th><th>大小</th><th>原因</th><th>結果</th></tr></thead><tbody>{''.join(scan_row(i) for i in report['scanChecks'])}</tbody></table></div>
<div class="card"><h2>還原 Hash / Size</h2><table><thead><tr><th>Fixture</th><th>檔名</th><th>預期 bytes</th><th>實際 bytes</th><th>實際 SHA256</th><th>原因</th><th>結果</th></tr></thead><tbody>{''.join(extract_row(i) for i in report['extractChecks'])}</tbody></table></div>
<div class="card"><h2>品質標記</h2><table><thead><tr><th>Fixture</th><th>檔名</th><th>預期</th><th>實際</th><th>原因</th><th>結果</th></tr></thead><tbody>{''.join(field_row(i) for i in report['qualityChecks'])}</tbody></table></div>
<div class="card"><h2>bytes_lost</h2><table><thead><tr><th>Fixture</th><th>檔名</th><th>預期</th><th>實際</th><th>原因</th><th>結果</th></tr></thead><tbody>{''.join(field_row(i) for i in report['bytesLostChecks'])}</tbody></table></div>
<div class="card"><h2>加密提示</h2><table><thead><tr><th>Fixture</th><th>檔名</th><th>預期</th><th>實際</th><th>原因</th><th>結果</th></tr></thead><tbody>{''.join(field_row(i) for i in report['encryptedHintChecks'])}</tbody></table></div>
</body></html>"""
    path.write_text(doc, encoding="utf-8")


def main() -> int:
    parser = argparse.ArgumentParser(description="Evaluate APFS quality-gate outputs.")
    parser.add_argument("--expected", default="fixtures/apfs_quality/expected_apfs_manifest.json")
    parser.add_argument("--output-root", default="target/apfs-quality")
    parser.add_argument("--out", default="target/apfs-quality/apfs_quality_report.json")
    parser.add_argument("--html", default="target/apfs-quality/apfs_quality_report.html")
    args = parser.parse_args()

    expected = load_json(Path(args.expected))
    report = evaluate(expected, Path(args.output_root))
    out_path = Path(args.out)
    html_path = Path(args.html)
    out_path.parent.mkdir(parents=True, exist_ok=True)
    out_path.write_text(json.dumps(report, ensure_ascii=False, indent=2), encoding="utf-8")
    write_html(report, html_path)

    print(f"apfs_quality_gate: {'PASS' if report['passed'] else 'FAIL'}")
    print(f"scan_entry_match_ratio: {pct(report['summary']['scanEntryMatchRatio'])}")
    print(f"extract_hash_match_ratio: {pct(report['summary']['extractHashMatchRatio'])}")
    print(f"quality_failures: {report['summary']['qualityFailures']}")
    print(f"bytes_lost_failures: {report['summary']['bytesLostFailures']}")
    print(f"encrypted_hint_failures: {report['summary']['encryptedHintFailures']}")
    print(f"report_json: {out_path}")
    print(f"report_html: {html_path}")
    return 0 if report["passed"] else 2


if __name__ == "__main__":
    raise SystemExit(main())
