#!/usr/bin/env python3
"""Evaluate a Polik Recovery carve_manifest.json against a fixture manifest."""
from __future__ import annotations

import argparse
import html
import json
import sys
from datetime import datetime, timezone
from pathlib import Path
from typing import Any


def load_json(path: Path) -> dict[str, Any]:
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except Exception as exc:  # noqa: BLE001 - user-facing utility
        raise SystemExit(f"無法讀取 JSON：{path}\n{exc}")


def pct(value: float) -> str:
    return f"{value * 100:.1f}%"


def item_key(item: dict[str, Any]) -> str:
    return str(item.get("sha256") or "")


def evaluate(expected_manifest: dict[str, Any], carve_manifest: dict[str, Any]) -> dict[str, Any]:
    expected = expected_manifest.get("expectedRecoverable", [])
    invalid = expected_manifest.get("expectedInvalidOrSkipped", [])
    thresholds = expected_manifest.get("thresholds", {})
    carved = carve_manifest.get("carved_items", [])

    expected_by_sha = {item_key(item): item for item in expected if item_key(item)}
    invalid_by_sha = {item_key(item): item for item in invalid if item_key(item)}
    carved_by_sha = {item_key(item): item for item in carved if item_key(item)}

    found_expected = [sha for sha in expected_by_sha if sha in carved_by_sha]
    missed_expected = [expected_by_sha[sha] for sha in expected_by_sha if sha not in carved_by_sha]
    false_positives = [item for sha, item in carved_by_sha.items() if sha not in expected_by_sha]
    invalid_recovered = [item for sha, item in carved_by_sha.items() if sha in invalid_by_sha]
    high_conf = [item for item in carved if item.get("tier") == "high"]
    high_expected = [item for item in high_conf if item_key(item) in expected_by_sha]

    expected_found_ratio = len(found_expected) / max(1, len(expected_by_sha))
    high_usable_ratio = len(high_expected) / max(1, len(high_conf))

    rules = [
        {
            "name": "expected_found_ratio",
            "actual": expected_found_ratio,
            "required": thresholds.get("minimumExpectedFoundRatio", 0.75),
            "operator": ">=",
            "passed": expected_found_ratio >= thresholds.get("minimumExpectedFoundRatio", 0.75),
        },
        {
            "name": "high_confidence_usable_ratio",
            "actual": high_usable_ratio,
            "required": thresholds.get("minimumHighConfidenceUsableRatio", 0.80),
            "operator": ">=",
            "passed": high_usable_ratio >= thresholds.get("minimumHighConfidenceUsableRatio", 0.80),
        },
        {
            "name": "false_positive_count",
            "actual": len(false_positives),
            "required": thresholds.get("maximumFalsePositiveCount", 2),
            "operator": "<=",
            "passed": len(false_positives) <= thresholds.get("maximumFalsePositiveCount", 2),
        },
        {
            "name": "missed_expected_count",
            "actual": len(missed_expected),
            "required": thresholds.get("maximumSkippedExpectedCount", 2),
            "operator": "<=",
            "passed": len(missed_expected) <= thresholds.get("maximumSkippedExpectedCount", 2),
        },
        {
            "name": "known_invalid_recovered_count",
            "actual": len(invalid_recovered),
            "required": 0,
            "operator": "==",
            "passed": len(invalid_recovered) == 0,
        },
    ]

    return {
        "version": "v0.6",
        "evaluatedAt": datetime.now(timezone.utc).isoformat(),
        "source": carve_manifest.get("source"),
        "summary": {
            "expectedRecoverable": len(expected_by_sha),
            "carvedItems": len(carved),
            "foundExpected": len(found_expected),
            "missedExpected": len(missed_expected),
            "falsePositiveCount": len(false_positives),
            "knownInvalidRecoveredCount": len(invalid_recovered),
            "highConfidenceCount": len(high_conf),
            "highConfidenceExpectedCount": len(high_expected),
            "expectedFoundRatio": expected_found_ratio,
            "highConfidenceUsableRatio": high_usable_ratio,
        },
        "rules": rules,
        "passed": all(rule["passed"] for rule in rules),
        "missedExpected": missed_expected,
        "falsePositives": false_positives,
        "invalidRecovered": invalid_recovered,
    }


def write_html(report: dict[str, Any], path: Path) -> None:
    summary = report["summary"]
    rule_rows = []
    for rule in report["rules"]:
        actual = rule["actual"]
        required = rule["required"]
        actual_text = pct(actual) if isinstance(actual, float) else str(actual)
        required_text = pct(required) if isinstance(required, float) else str(required)
        cls = "ok" if rule["passed"] else "bad"
        rule_rows.append(
            f"<tr><td>{html.escape(rule['name'])}</td><td>{html.escape(actual_text)}</td>"
            f"<td>{html.escape(rule['operator'])} {html.escape(required_text)}</td><td class='{cls}'>{'PASS' if rule['passed'] else 'FAIL'}</td></tr>"
        )
    missed_rows = "".join(
        f"<li>{html.escape(str(item.get('name')))} · {html.escape(str(item.get('expectedExtension', '')))} · {html.escape(str(item.get('sha256', ''))[:16])}</li>"
        for item in report.get("missedExpected", [])
    ) or "<li>無</li>"
    fp_rows = "".join(
        f"<li>{html.escape(str(item.get('output_path', item.get('outputPath', ''))))} · tier={html.escape(str(item.get('tier', '')))} · {html.escape(str(item.get('sha256', ''))[:16])}</li>"
        for item in report.get("falsePositives", [])
    ) or "<li>無</li>"
    invalid_rows = "".join(
        f"<li>{html.escape(str(item.get('output_path', item.get('outputPath', ''))))} · {html.escape(str(item.get('sha256', ''))[:16])}</li>"
        for item in report.get("invalidRecovered", [])
    ) or "<li>無</li>"
    status = "PASS" if report["passed"] else "FAIL"
    status_cls = "ok" if report["passed"] else "bad"
    doc = f"""<!doctype html>
<html lang="zh-Hant"><meta charset="utf-8"><title>Polik Recovery Quality Gate</title>
<style>
body{{font-family:-apple-system,BlinkMacSystemFont,'Noto Sans TC',sans-serif;margin:32px;line-height:1.6;color:#1f2937}}
.card{{border:1px solid #e5e7eb;border-radius:16px;padding:18px;margin:0 0 18px;background:#fff}}
table{{border-collapse:collapse;width:100%}}td,th{{border-top:1px solid #e5e7eb;padding:10px;text-align:left}}.ok{{color:#047857;font-weight:700}}.bad{{color:#b91c1c;font-weight:700}}code{{background:#f3f4f6;padding:2px 6px;border-radius:6px}}
</style><body>
<h1>Polik Recovery v0.6 品質門檻報告</h1>
<div class="card"><p><b>結果：</b><span class="{status_cls}">{status}</span></p><p><b>來源：</b>{html.escape(str(report.get('source') or ''))}</p><p><b>評估時間：</b>{html.escape(report['evaluatedAt'])}</p></div>
<div class="card"><h2>摘要</h2><p>預期可救：{summary['expectedRecoverable']}　實際 carved：{summary['carvedItems']}　命中預期：{summary['foundExpected']}　漏掉：{summary['missedExpected']}　假陽性：{summary['falsePositiveCount']}</p><p>預期命中率：<b>{pct(summary['expectedFoundRatio'])}</b>　高信心可用率：<b>{pct(summary['highConfidenceUsableRatio'])}</b></p></div>
<div class="card"><h2>門檻</h2><table><thead><tr><th>項目</th><th>實際</th><th>門檻</th><th>結果</th></tr></thead><tbody>{''.join(rule_rows)}</tbody></table></div>
<div class="card"><h2>漏掉的預期檔案</h2><ul>{missed_rows}</ul></div>
<div class="card"><h2>假陽性輸出</h2><ul>{fp_rows}</ul></div>
<div class="card"><h2>已知壞檔被錯救</h2><ul>{invalid_rows}</ul></div>
</body></html>"""
    path.write_text(doc, encoding="utf-8")


def main() -> int:
    parser = argparse.ArgumentParser(description="Evaluate Polik Recovery output against v0.6 fixture expectations.")
    parser.add_argument("--expected", default="fixtures/quality/carver_baseline/expected_manifest.json")
    parser.add_argument("--manifest", required=True, help="Path to carve_manifest.json generated by polik-recovery carve")
    parser.add_argument("--out", default="quality_report.json")
    parser.add_argument("--html", default="quality_report.html")
    args = parser.parse_args()

    expected = load_json(Path(args.expected))
    manifest = load_json(Path(args.manifest))
    report = evaluate(expected, manifest)
    Path(args.out).write_text(json.dumps(report, ensure_ascii=False, indent=2), encoding="utf-8")
    write_html(report, Path(args.html))

    summary = report["summary"]
    print(f"quality_gate: {'PASS' if report['passed'] else 'FAIL'}")
    print(f"expected_found_ratio: {pct(summary['expectedFoundRatio'])}")
    print(f"high_confidence_usable_ratio: {pct(summary['highConfidenceUsableRatio'])}")
    print(f"false_positive_count: {summary['falsePositiveCount']}")
    print(f"missed_expected_count: {summary['missedExpected']}")
    print(f"report_json: {args.out}")
    print(f"report_html: {args.html}")
    return 0 if report["passed"] else 2


if __name__ == "__main__":
    raise SystemExit(main())
