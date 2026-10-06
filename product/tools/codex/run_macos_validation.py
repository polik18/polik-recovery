#!/usr/bin/env python3
"""Run macOS/Codex validation commands and produce JSON/HTML logs.

This tool is intentionally conservative: it does not try to fix failures.
It records commands, exit codes, durations, and log files so Codex or a
human developer can diagnose build/test/runtime problems on macOS.
"""
from __future__ import annotations

import argparse
import html
import json
import os
import platform
import shlex
import shutil
import subprocess
import sys
import time
from datetime import datetime, timezone
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[2]
DEFAULT_OUT = ROOT / "target" / "codex-validation"

COMMAND_GROUPS: dict[str, list[list[str]]] = {
    "doctor": [
        ["make", "dev-doctor"],
        ["make", "macos-doctor"],
    ],
    "rust": [
        ["cargo", "fmt", "--check"],
        ["cargo", "check", "--workspace"],
        ["cargo", "test"],
        ["cargo", "build", "--release", "-p", "polik-recovery-cli"],
    ],
    "quality": [
        ["make", "quality-gate-all"],
    ],
    "real-apfs": [
        ["make", "real-apfs-metadata-gate"],
    ],
    "image-workflow": [
        ["make", "macos-image-workflow"],
        ["make", "macos-image-plan", "SOURCE=/dev/diskX", "OUT=target/codex-validation/polik-images"],
        ["python3", "tools/macos/ddrescue_map_report.py", "--map", "fixtures/ddrescue_map/sample.map", "--out", "target/codex-validation/sample_map_report.json", "--html", "target/codex-validation/sample_map_report.html", "--report-only"],
    ],
    "session-workflow": [
        ["make", "session-workflow"],
        ["python3", "tools/session/create_recovery_session.py", "--source", "/dev/diskX", "--mode", "apfs", "--out-root", "target/codex-validation/session-smoke"],
        ["bash", "-lc", "SESSION=$(find target/codex-validation/session-smoke -maxdepth 1 -type d -name 'polik-session-*' | head -1); mkdir -p target/codex-validation; echo '{\"ok\":true,\"kind\":\"session-import-smoke\"}' > target/codex-validation/session-import-smoke.json; python3 tools/session/import_session_artifacts.py --session \"$SESSION\" --kind quality_report target/codex-validation/session-import-smoke.json"],
        ["bash", "-lc", "SESSION=$(find target/codex-validation/session-smoke -maxdepth 1 -type d -name 'polik-session-*' | head -1); mkdir -p \"$SESSION/03_recovery_output/recovered\" \"$SESSION/09_reports\" \"$SESSION/10_audit\"; echo recovered > \"$SESSION/03_recovery_output/recovered/smoke.txt\"; echo '<html>customer</html>' > \"$SESSION/09_reports/customer_summary.html\"; echo '{\\\"summary\\\":{\\\"total\\\":0}}' > \"$SESSION/10_audit/sample_audit.json\"; python3 tools/session/session_delivery_package.py --session \"$SESSION\" --package-id codex-session-smoke"],
    ],
    "tauri": [
        ["cargo", "tauri", "build", "--bundles", "app"],
    ],
}

PHASES: dict[str, list[str]] = {
    "quick": ["doctor", "rust"],
    "quality": ["doctor", "rust", "quality"],
    "full": ["doctor", "rust", "quality", "image-workflow", "session-workflow", "real-apfs", "tauri"],
    "image-workflow": ["doctor", "image-workflow"],
    "session-workflow": ["doctor", "session-workflow"],
    "real-apfs": ["doctor", "rust", "real-apfs"],
    "tauri": ["doctor", "rust", "tauri"],
}


def now_iso() -> str:
    return datetime.now(timezone.utc).astimezone().isoformat(timespec="seconds")


def check_command_available(cmd: str) -> bool:
    return shutil.which(cmd) is not None


def write_html_report(summary: dict[str, Any], out_path: Path) -> None:
    status = "PASS" if summary["passed"] else "FAIL"
    rows = []
    for item in summary["results"]:
        code = item.get("exit_code")
        badge = "PASS" if code == 0 else "FAIL"
        rows.append(
            "<tr>"
            f"<td>{html.escape(item['group'])}</td>"
            f"<td><code>{html.escape(item['command_display'])}</code></td>"
            f"<td class='{badge.lower()}'>{badge}</td>"
            f"<td>{html.escape(str(code))}</td>"
            f"<td>{item['duration_seconds']:.1f}s</td>"
            f"<td><code>{html.escape(item['log_file'])}</code></td>"
            "</tr>"
        )
    skipped_rows = []
    for item in summary.get("skipped", []):
        skipped_rows.append(
            "<tr>"
            f"<td>{html.escape(item['group'])}</td>"
            f"<td><code>{html.escape(item['command_display'])}</code></td>"
            f"<td>{html.escape(item['reason'])}</td>"
            "</tr>"
        )
    body = f"""
<!doctype html>
<html lang="zh-Hant">
<head>
<meta charset="utf-8" />
<title>Polik Recovery macOS/Codex Validation Report</title>
<style>
body {{ font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', sans-serif; margin: 32px; line-height: 1.6; color: #1f2937; }}
code {{ background: #f3f4f6; padding: 2px 5px; border-radius: 5px; }}
table {{ border-collapse: collapse; width: 100%; margin-top: 16px; }}
th, td {{ border: 1px solid #d1d5db; padding: 8px; text-align: left; vertical-align: top; }}
th {{ background: #f9fafb; }}
.pass {{ color: #047857; font-weight: 700; }}
.fail {{ color: #b91c1c; font-weight: 700; }}
.summary {{ padding: 16px; border-radius: 12px; background: {'#ecfdf5' if summary['passed'] else '#fef2f2'}; border: 1px solid {'#a7f3d0' if summary['passed'] else '#fecaca'}; }}
</style>
</head>
<body>
<h1>Polik Recovery macOS/Codex Validation Report</h1>
<div class="summary">
<strong>Overall:</strong> <span class="{status.lower()}">{status}</span><br />
<strong>Started:</strong> {html.escape(summary['started_at'])}<br />
<strong>Finished:</strong> {html.escape(summary['finished_at'])}<br />
<strong>Phase:</strong> {html.escape(summary['phase'])}<br />
<strong>Host:</strong> {html.escape(summary['environment']['platform'])}<br />
</div>
<h2>Command Results</h2>
<table>
<thead><tr><th>Group</th><th>Command</th><th>Status</th><th>Exit</th><th>Duration</th><th>Log</th></tr></thead>
<tbody>{''.join(rows)}</tbody>
</table>
<h2>Skipped Commands</h2>
<table>
<thead><tr><th>Group</th><th>Command</th><th>Reason</th></tr></thead>
<tbody>{''.join(skipped_rows) if skipped_rows else '<tr><td colspan="3">None</td></tr>'}</tbody>
</table>
<h2>Next Step</h2>
<p>把本報告、所有 log、以及 <code>target/*quality*/</code> 報告交給 Codex 或工程師處理。修復順序：先編譯，再測試，再實機 APFS，再 Tauri 打包。</p>
</body>
</html>
"""
    out_path.write_text(body, encoding="utf-8")


def run_command(command: list[str], group: str, out_dir: Path, timeout: int | None) -> dict[str, Any]:
    display = " ".join(shlex.quote(part) for part in command)
    safe_name = "_".join(part.replace("/", "-").replace(" ", "_") for part in command[:4])[:120]
    log_path = out_dir / "logs" / f"{int(time.time())}_{group}_{safe_name}.log"
    log_path.parent.mkdir(parents=True, exist_ok=True)
    start = time.monotonic()
    with log_path.open("w", encoding="utf-8", errors="replace") as log:
        log.write(f"$ {display}\n")
        log.write(f"started_at={now_iso()}\n")
        log.write(f"cwd={ROOT}\n\n")
        try:
            proc = subprocess.run(
                command,
                cwd=ROOT,
                stdout=subprocess.PIPE,
                stderr=subprocess.STDOUT,
                text=True,
                timeout=timeout,
                env=os.environ.copy(),
            )
            output = proc.stdout or ""
            log.write(output)
            exit_code = proc.returncode
        except subprocess.TimeoutExpired as exc:
            partial = exc.stdout if isinstance(exc.stdout, str) else ""
            log.write(partial or "")
            log.write(f"\nTIMEOUT after {timeout} seconds\n")
            exit_code = 124
        except FileNotFoundError as exc:
            log.write(f"COMMAND_NOT_FOUND: {exc}\n")
            exit_code = 127
    duration = time.monotonic() - start
    return {
        "group": group,
        "command": command,
        "command_display": display,
        "exit_code": exit_code,
        "duration_seconds": duration,
        "log_file": str(log_path.relative_to(ROOT)),
    }


def build_command_plan(phase: str, include: list[str], exclude: list[str]) -> list[tuple[str, list[str]]]:
    groups = list(PHASES[phase])
    for group in include:
        if group not in COMMAND_GROUPS:
            raise SystemExit(f"Unknown include group: {group}")
        if group not in groups:
            groups.append(group)
    groups = [group for group in groups if group not in set(exclude)]
    plan: list[tuple[str, list[str]]] = []
    for group in groups:
        for command in COMMAND_GROUPS[group]:
            plan.append((group, command))
    return plan


def main() -> int:
    parser = argparse.ArgumentParser(description="Run macOS/Codex validation commands")
    parser.add_argument("--phase", choices=sorted(PHASES), default="quick", help="Validation phase to run")
    parser.add_argument("--out", type=Path, default=DEFAULT_OUT, help="Output directory")
    parser.add_argument("--timeout", type=int, default=1800, help="Per-command timeout in seconds")
    parser.add_argument("--continue-on-error", action="store_true", help="Continue after command failures")
    parser.add_argument("--include", action="append", default=[], help="Additional command group to include")
    parser.add_argument("--exclude", action="append", default=[], help="Command group to exclude")
    parser.add_argument("--dry-run", action="store_true", help="Print command plan without running")
    args = parser.parse_args()

    out_dir = args.out.resolve()
    out_dir.mkdir(parents=True, exist_ok=True)
    plan = build_command_plan(args.phase, args.include, args.exclude)

    if args.dry_run:
        print(f"phase={args.phase}")
        for group, command in plan:
            print(f"[{group}] {' '.join(shlex.quote(part) for part in command)}")
        return 0

    summary: dict[str, Any] = {
        "schema": "polik.codex.validation.v1",
        "phase": args.phase,
        "started_at": now_iso(),
        "finished_at": None,
        "passed": True,
        "environment": {
            "platform": platform.platform(),
            "machine": platform.machine(),
            "python": sys.version.split()[0],
            "root": str(ROOT),
        },
        "results": [],
        "skipped": [],
    }

    for group, command in plan:
        executable = command[0]
        display = " ".join(shlex.quote(part) for part in command)
        if not check_command_available(executable):
            summary["passed"] = False
            summary["skipped"].append({
                "group": group,
                "command": command,
                "command_display": display,
                "reason": f"command not found: {executable}",
            })
            if not args.continue_on_error:
                break
            continue
        result = run_command(command, group, out_dir, args.timeout)
        summary["results"].append(result)
        if result["exit_code"] != 0:
            summary["passed"] = False
            if not args.continue_on_error:
                break

    summary["finished_at"] = now_iso()
    json_path = out_dir / "validation_summary.json"
    html_path = out_dir / "validation_summary.html"
    json_path.write_text(json.dumps(summary, ensure_ascii=False, indent=2), encoding="utf-8")
    write_html_report(summary, html_path)

    print(f"Validation summary: {json_path}")
    print(f"HTML report: {html_path}")
    return 0 if summary["passed"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
