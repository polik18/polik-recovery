#!/usr/bin/env python3
"""Validate the desktop WebView's minimum CSP regression contract."""

from __future__ import annotations

import base64
import hashlib
import json
from pathlib import Path
import re
import sys


ROOT = Path(__file__).resolve().parent.parent
CONFIG = ROOT / "product" / "src-tauri" / "tauri.conf.json"
FRONTEND = ROOT / "product" / "apps" / "desktop-prototype" / "index.html"


def csp_hash(content: str) -> str:
    digest = hashlib.sha256(content.encode("utf-8")).digest()
    return "sha256-" + base64.b64encode(digest).decode("ascii")


def main() -> int:
    config = json.loads(CONFIG.read_text(encoding="utf-8"))
    csp = config.get("app", {}).get("security", {}).get("csp")
    errors: list[str] = []
    if not isinstance(csp, str) or not csp.strip():
        errors.append("desktop CSP must be a non-empty string")
        csp = ""
    if "'unsafe-eval'" in csp:
        errors.append("desktop CSP must not allow unsafe-eval")
    script_directive = next(
        (item.strip() for item in csp.split(";") if item.strip().startswith("script-src")),
        "",
    )
    if not script_directive:
        errors.append("desktop CSP must define script-src")
    elif "'unsafe-inline'" in script_directive:
        errors.append("desktop script-src must not allow unsafe-inline")

    html = FRONTEND.read_text(encoding="utf-8")
    if not re.search(r"if\s*\(\s*isRawDevicePath\(fp\)\s*\)", html):
        errors.append("desktop scan path must reject raw-device paths before run_recovery_scan")
    if "isRawDevicePath" not in html or "run_recovery_scan" not in html:
        errors.append("desktop frontend raw-device safety contract is incomplete")
    scripts = re.findall(r"<script(?:\s[^>]*)?>(.*?)</script>", html, flags=re.I | re.S)
    if len(scripts) != 1:
        errors.append(f"expected exactly one inline desktop script, found {len(scripts)}")
    else:
        expected_hash = csp_hash(scripts[0])
        if expected_hash not in script_directive:
            errors.append(f"desktop script CSP hash is stale; expected {expected_hash}")

    if re.search(r"\son[a-z]+\s*=", html, flags=re.I):
        errors.append("desktop frontend must not use inline event-handler attributes")

    if errors:
        for error in errors:
            print(f"ERROR: {error}", file=sys.stderr)
        return 1
    print("Desktop frontend CSP and inline-script hash are consistent.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
