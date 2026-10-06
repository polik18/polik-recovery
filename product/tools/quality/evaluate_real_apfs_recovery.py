#!/usr/bin/env python3
"""Evaluate real macOS APFS image scan/extract outputs for Polik Recovery v1.4.

This evaluator is intentionally strict for required allocated files, and
informational for deleted APFS files because real APFS deletion metadata is not
stable after unmount, checkpoint rotation, snapshots, or TRIM-like cleanup.
"""
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


def normalize_path(value: str) -> str:
    value = value.replace("\\", "/")
    if not value.startswith("/"):
        value = "/" + value
    while "//" in value:
        value = value.replace("//", "/")
    return value.rstrip("/") or "/"


def path_matches(actual: str, expected: str) -> bool:
    actual_n = normalize_path(actual)
    expected_n = normalize_path(expected)
    return actual_n == expected_n or actual_n.endswith(expected_n)


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


def load_scan_entries(path: Path) -> tuple[list[dict[str, Any]], str | None]:
    if not path.exists():
        return [], f"scan JSON missing: {path}"
    try:
        data = load_json(path)
        return data.get("entries", []), None
    except Exception as exc:  # noqa: BLE001 - report parser failure
        return [], f"scan JSON parse failed: {exc}"


def find_entry(entries: list[dict[str, Any]], expected_path: str) -> dict[str, Any] | None:
    for entry in entries:
        if path_matches(str(entry.get("path", "")), expected_path):
            return entry
    return None


def evaluate(expected_manifest: dict[str, Any], output_root: Path) -> dict[str, Any]:
    scan_entries, scan_error = load_scan_entries(output_root / "scan" / "apfs_scan.json")
    required_files = [item for item in expected_manifest.get("files", []) if item.get("required", True)]
    optional_files = [item for item in expected_manifest.get("files", []) if not item.get("required", True)]

    scan_checks: list[dict[str, Any]] = []
    extract_checks: list[dict[str, Any]] = []
    field_checks: list[dict[str, Any]] = []
    optional_observations: list[dict[str, Any]] = []

    for item in required_files:
        matched = find_entry(scan_entries, item["path"])
        ok = bool(matched and not scan_error)
        scan_checks.append({
            "path": item["path"],
            "expectedBytes": item.get("bytes"),
            "actualPath": matched.get("path") if matched else None,
            "actualBytes": matched.get("size") if matched else None,
            "inode": matched.get("inode", matched.get("id")) if matched else None,
            "passed": ok,
            "reason": "matched" if ok else (scan_error or "missing required APFS entry"),
        })

        name = item.get("name") or Path(item["path"]).name
        out_file = output_root / "extracts" / name
        stdout_path = output_root / "extracts" / f"{name}.stdout.txt"
        fields = parse_stdout_fields(stdout_path)
        exists = out_file.exists()
        actual_sha = sha256_file(out_file) if exists else None
        actual_bytes = out_file.stat().st_size if exists else None
        expected_sha = item.get("sha256")
        expected_bytes = item.get("bytes")
        hash_ok = bool(exists and actual_sha == expected_sha and actual_bytes == expected_bytes)
        extract_checks.append({
            "path": item["path"],
            "output": str(out_file),
            "expectedSha256": expected_sha,
            "actualSha256": actual_sha,
            "expectedBytes": expected_bytes,
            "actualBytes": actual_bytes,
            "passed": hash_ok,
            "reason": "hash and size matched" if hash_ok else "missing output or hash/size mismatch",
        })

        expected_quality = item.get("expectedQuality", "complete")
        actual_quality = fields.get("quality")
        quality_ok = actual_quality == expected_quality
        field_checks.append({
            "path": item["path"],
            "field": "quality",
            "expected": expected_quality,
            "actual": actual_quality,
            "passed": quality_ok,
            "reason": "quality matched" if quality_ok else "quality mismatch or extraction stdout missing",
        })

        expected_bytes_lost = str(item.get("expectedBytesLost", 0))
        actual_bytes_lost = fields.get("bytes_lost")
        bytes_lost_ok = actual_bytes_lost == expected_bytes_lost
        field_checks.append({
            "path": item["path"],
            "field": "bytes_lost",
            "expected": expected_bytes_lost,
            "actual": actual_bytes_lost,
            "passed": bytes_lost_ok,
            "reason": "bytes_lost matched" if bytes_lost_ok else "bytes_lost mismatch or extraction stdout missing",
        })

    for item in optional_files:
        matched = find_entry(scan_entries, item["path"])
        optional_observations.append({
            "path": item["path"],
            "deleted": item.get("deleted", False),
            "actualPath": matched.get("path") if matched else None,
            "inode": matched.get("inode", matched.get("id")) if matched else None,
            "observed": bool(matched),
            "note": item.get("note", "optional APFS observation"),
        })

    scan_passed = sum(1 for item in scan_checks if item["passed"])
    extract_passed = sum(1 for item in extract_checks if item["passed"])
    field_passed = sum(1 for item in field_checks if item["passed"])
    scan_ratio = scan_passed / max(1, len(scan_checks))
    extract_ratio = extract_passed / max(1, len(extract_checks))
    field_ratio = field_passed / max(1, len(field_checks))
    thresholds = expected_manifest.get("thresholds", {})

    rules = [
        {
            "name": "real_apfs_required_scan_match_ratio",
            "actual": scan_ratio,
            "operator": ">=",
            "required": thresholds.get("minimumRequiredScanMatchRatio", 1.0),
            "passed": scan_ratio >= thresholds.get("minimumRequiredScanMatchRatio", 1.0),
        },
        {
            "name": "real_apfs_required_extract_hash_ratio",
            "actual": extract_ratio,
            "operator": ">=",
            "required": thresholds.get("minimumRequiredExtractHashRatio", 1.0),
            "passed": extract_ratio >= thresholds.get("minimumRequiredExtractHashRatio", 1.0),
        },
        {
            "name": "real_apfs_extract_field_match_ratio",
            "actual": field_ratio,
            "operator": ">=",
            "required": thresholds.get("minimumExtractFieldMatchRatio", 1.0),
            "passed": field_ratio >= thresholds.get("minimumExtractFieldMatchRatio", 1.0),
        },
        {
            "name": "real_apfs_required_scan_failures",
            "actual": len(scan_checks) - scan_passed,
            "operator": "<=",
            "required": thresholds.get("maximumRequiredScanFailures", 0),
            "passed": (len(scan_checks) - scan_passed) <= thresholds.get("maximumRequiredScanFailures", 0),
        },
        {
            "name": "real_apfs_required_extract_failures",
            "actual": len(extract_checks) - extract_passed,
            "operator": "<=",
            "required": thresholds.get("maximumRequiredExtractFailures", 0),
            "passed": (len(extract_checks) - extract_passed) <= thresholds.get("maximumRequiredExtractFailures", 0),
        },
    ]

    return {
        "version": "v1.4",
        "kind": "real_macos_apfs_quality_report",
        "evaluatedAt": datetime.now(timezone.utc).isoformat(),
        "fixture": {
            "image": expected_manifest.get("image"),
            "apfsSliceImage": expected_manifest.get("apfsSliceImage"),
            "volumeName": expected_manifest.get("volumeName"),
            "volumeOffset": expected_manifest.get("volumeOffset", 0),
            "createdAt": expected_manifest.get("createdAt"),
        },
        "summary": {
            "scanChecks": len(scan_checks),
            "scanPassed": scan_passed,
            "extractChecks": len(extract_checks),
            "extractPassed": extract_passed,
            "fieldChecks": len(field_checks),
            "fieldPassed": field_passed,
            "optionalObserved": sum(1 for item in optional_observations if item["observed"]),
            "optionalTotal": len(optional_observations),
            "scanEntryMatchRatio": scan_ratio,
            "extractHashMatchRatio": extract_ratio,
            "extractFieldMatchRatio": field_ratio,
        },
        "rules": rules,
        "scanChecks": scan_checks,
        "extractChecks": extract_checks,
        "fieldChecks": field_checks,
        "optionalObservations": optional_observations,
        "scanError": scan_error,
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
        return f"<tr><td>{html.escape(str(item.get('path')))}</td><td>{html.escape(str(item.get('actualPath') or ''))}</td><td>{html.escape(str(item.get('inode') or ''))}</td><td>{html.escape(str(item.get('reason')))}</td><td class='{cls}'>{'PASS' if item['passed'] else 'FAIL'}</td></tr>"

    def extract_row(item: dict[str, Any]) -> str:
        cls = "ok" if item["passed"] else "bad"
        return f"<tr><td>{html.escape(str(item.get('path')))}</td><td>{html.escape(str(item.get('expectedBytes')))}</td><td>{html.escape(str(item.get('actualBytes')))}</td><td>{html.escape(str(item.get('actualSha256') or '')[:16])}</td><td>{html.escape(str(item.get('reason')))}</td><td class='{cls}'>{'PASS' if item['passed'] else 'FAIL'}</td></tr>"

    def field_row(item: dict[str, Any]) -> str:
        cls = "ok" if item["passed"] else "bad"
        return f"<tr><td>{html.escape(str(item.get('path')))}</td><td>{html.escape(str(item.get('field')))}</td><td>{html.escape(str(item.get('expected')))}</td><td>{html.escape(str(item.get('actual')))}</td><td>{html.escape(str(item.get('reason')))}</td><td class='{cls}'>{'PASS' if item['passed'] else 'FAIL'}</td></tr>"

    def optional_row(item: dict[str, Any]) -> str:
        cls = "ok" if item["observed"] else "warn"
        return f"<tr><td>{html.escape(str(item.get('path')))}</td><td>{'yes' if item.get('deleted') else 'no'}</td><td>{html.escape(str(item.get('actualPath') or ''))}</td><td>{html.escape(str(item.get('inode') or ''))}</td><td>{html.escape(str(item.get('note')))}</td><td class='{cls}'>{'OBSERVED' if item['observed'] else 'NOT FOUND'}</td></tr>"

    status = "PASS" if report["passed"] else "FAIL"
    status_cls = "ok" if report["passed"] else "bad"
    doc = f"""<!doctype html>
<html lang="zh-Hant">
<head>
<meta charset="utf-8">
<title>Polik Recovery v1.4 真實 APFS 品質報告</title>
<style>
body{{font-family:-apple-system,BlinkMacSystemFont,'Noto Sans TC','PingFang TC',sans-serif;margin:32px;line-height:1.55;color:#17202a;background:#f7fafc}}
.card{{background:white;border-radius:16px;padding:22px;margin:18px 0;box-shadow:0 8px 26px rgba(15,23,42,.08)}}
table{{border-collapse:collapse;width:100%;background:white}}
th,td{{border-bottom:1px solid #e5e7eb;padding:10px;text-align:left;font-size:14px;vertical-align:top}}
th{{background:#f1f5f9}}
.ok{{color:#047857;font-weight:700}}
.bad{{color:#b91c1c;font-weight:700}}
.warn{{color:#b45309;font-weight:700}}
code{{background:#eef2ff;padding:2px 6px;border-radius:6px}}
</style>
</head>
<body>
<h1>Polik Recovery v1.4 真實 macOS APFS 品質報告</h1>
<div class="card"><h2 class="{status_cls}">{status}</h2>
<p>評估時間：{html.escape(report['evaluatedAt'])}</p>
<p>APFS slice：<code>{html.escape(str(report['fixture'].get('apfsSliceImage') or ''))}</code></p>
<p>掃描檢查：{summary['scanPassed']} / {summary['scanChecks']}，提取檢查：{summary['extractPassed']} / {summary['extractChecks']}，欄位檢查：{summary['fieldPassed']} / {summary['fieldChecks']}</p>
<p>刪除檔觀察：{summary['optionalObserved']} / {summary['optionalTotal']}（APFS deleted record 為資訊性，不列入品質門檻）</p>
</div>
<div class="card"><h2>品質門檻</h2><table><tr><th>規則</th><th>實際</th><th>門檻</th><th>結果</th></tr>{''.join(rule_row(rule) for rule in report['rules'])}</table></div>
<div class="card"><h2>掃描檢查</h2><table><tr><th>預期路徑</th><th>實際路徑</th><th>inode</th><th>原因</th><th>結果</th></tr>{''.join(scan_row(item) for item in report['scanChecks'])}</table></div>
<div class="card"><h2>提取 Hash 檢查</h2><table><tr><th>路徑</th><th>預期 bytes</th><th>實際 bytes</th><th>實際 sha256</th><th>原因</th><th>結果</th></tr>{''.join(extract_row(item) for item in report['extractChecks'])}</table></div>
<div class="card"><h2>提取欄位檢查</h2><table><tr><th>路徑</th><th>欄位</th><th>預期</th><th>實際</th><th>原因</th><th>結果</th></tr>{''.join(field_row(item) for item in report['fieldChecks'])}</table></div>
<div class="card"><h2>刪除檔觀察</h2><p>APFS 的 deleted record 可能因 checkpoint、snapshot、unmount 或空間回收而消失，所以這裡只做觀察，不做硬性 PASS/FAIL。</p><table><tr><th>路徑</th><th>已刪除</th><th>實際路徑</th><th>inode</th><th>說明</th><th>狀態</th></tr>{''.join(optional_row(item) for item in report['optionalObservations'])}</table></div>
</body></html>
"""
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(doc, encoding="utf-8")


def main() -> int:
    parser = argparse.ArgumentParser(description="Evaluate v1.4 real macOS APFS quality-gate outputs.")
    parser.add_argument("--expected", required=True)
    parser.add_argument("--output-root", required=True)
    parser.add_argument("--out", required=True)
    parser.add_argument("--html", required=True)
    args = parser.parse_args()

    expected = load_json(Path(args.expected))
    report = evaluate(expected, Path(args.output_root))
    out = Path(args.out)
    out.parent.mkdir(parents=True, exist_ok=True)
    out.write_text(json.dumps(report, ensure_ascii=False, indent=2), encoding="utf-8")
    write_html(report, Path(args.html))
    print(json.dumps({"passed": report["passed"], "summary": report["summary"]}, ensure_ascii=False, indent=2))
    return 0 if report["passed"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
