#!/usr/bin/env python3
"""Verify a Polik Recovery source image manifest."""
from __future__ import annotations

import argparse
import datetime as dt
import hashlib
import html
import json
from pathlib import Path
from typing import Any


def now_iso() -> str:
    return dt.datetime.now(dt.timezone.utc).astimezone().isoformat(timespec="seconds")


def sha256_file(path: Path, chunk_size: int = 1024 * 1024 * 8) -> str:
    h = hashlib.sha256()
    with path.open("rb") as f:
        for chunk in iter(lambda: f.read(chunk_size), b""):
            h.update(chunk)
    return h.hexdigest()


def sample_sha256(path: Path, sample_size: int = 1024 * 1024) -> dict[str, Any]:
    size = path.stat().st_size
    ranges: list[tuple[str, int, int]] = []
    if size <= sample_size * 3:
        ranges.append(("full", 0, size))
    else:
        ranges.append(("head", 0, sample_size))
        mid = max(0, (size // 2) - (sample_size // 2))
        ranges.append(("middle", mid, sample_size))
        ranges.append(("tail", max(0, size - sample_size), min(sample_size, size)))
    result = {"size_bytes": size, "sample_size": sample_size, "ranges": []}
    with path.open("rb") as f:
        for label, offset, length in ranges:
            f.seek(offset)
            data = f.read(length)
            result["ranges"].append({
                "label": label,
                "offset": offset,
                "length": len(data),
                "sha256": hashlib.sha256(data).hexdigest(),
            })
    return result


def write_html(report: dict[str, Any], html_path: Path) -> None:
    status = "PASS" if report["pass"] else "REVIEW"
    rows = []
    for key, value in report.items():
        if isinstance(value, (dict, list)):
            value = json.dumps(value, ensure_ascii=False, indent=2)
        rows.append(f"<tr><th>{html.escape(str(key))}</th><td><pre>{html.escape(str(value))}</pre></td></tr>")
    html_doc = f"""<!doctype html><html lang="zh-Hant"><head><meta charset="utf-8"><title>Polik Image Verification</title>
<style>body{{font-family:-apple-system,BlinkMacSystemFont,'Noto Sans TC',sans-serif;margin:32px;background:#f8fafc;color:#1a202c}}.status{{display:inline-block;padding:6px 10px;border-radius:999px;background:{'#c6f6d5' if report['pass'] else '#fed7d7'};font-weight:700}}table{{border-collapse:collapse;width:100%;background:#fff;margin-top:16px}}th,td{{border:1px solid #e2e8f0;padding:10px;text-align:left;vertical-align:top}}th{{width:220px;background:#edf2f7}}pre{{white-space:pre-wrap;margin:0;font-family:ui-monospace,SFMono-Regular,Menlo,monospace}}</style></head>
<body><h1>Polik Image Verification <span class="status">{status}</span></h1><table>{''.join(rows)}</table></body></html>"""
    html_path.write_text(html_doc, encoding="utf-8")


def main() -> int:
    parser = argparse.ArgumentParser(description="Verify a Polik Recovery source image.")
    parser.add_argument("--manifest", required=True, help="create_source_image.py 產生的 manifest.json")
    parser.add_argument("--image", help="覆寫 manifest 裡的 image 路徑")
    parser.add_argument("--out", help="驗證 JSON 輸出路徑")
    parser.add_argument("--html", help="驗證 HTML 輸出路徑")
    parser.add_argument("--full-sha256", action="store_true", help="計算完整 SHA-256；大映像檔會很久")
    args = parser.parse_args()

    manifest_path = Path(args.manifest).expanduser().resolve()
    if not manifest_path.exists():
        print(f"manifest not found: {manifest_path}", file=__import__('sys').stderr)
        return 2
    manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
    image_path = Path(args.image or manifest.get("output_image", "")).expanduser()
    if not image_path.is_absolute():
        image_path = (manifest_path.parent / image_path).resolve()

    checks: list[dict[str, Any]] = []
    def check(name: str, ok: bool, detail: str) -> None:
        checks.append({"name": name, "ok": ok, "detail": detail})

    exists = image_path.exists()
    check("image_exists", exists, str(image_path))
    image_size = image_path.stat().st_size if exists else 0
    expected_size = manifest.get("source_size_bytes_before")
    if expected_size:
        check("size_matches_source", image_size >= int(expected_size), f"image={image_size}, source={expected_size}")
    else:
        check("size_known", image_size > 0, f"image_size={image_size}")

    sample = sample_sha256(image_path) if exists and image_size > 0 else None
    prior_sample = manifest.get("sample_sha256")
    if sample and prior_sample:
        check("sample_sha256_matches_manifest", sample == prior_sample, "sample hash comparison")
    elif sample:
        check("sample_sha256_created", True, "manifest had no prior sample hash")

    full_hash = None
    if args.full_sha256 and exists and image_size > 0:
        full_hash = sha256_file(image_path)
        prior = manifest.get("sha256")
        if prior:
            check("full_sha256_matches_manifest", full_hash == prior, "full hash comparison")
        else:
            check("full_sha256_created", True, full_hash)

    passed = all(c["ok"] for c in checks if c["name"] != "size_matches_source")
    if expected_size and image_size < int(expected_size):
        passed = False

    out_path = Path(args.out or manifest_path.with_suffix(".verification.json"))
    html_path = Path(args.html or manifest_path.with_suffix(".verification.html"))
    report = {
        "schema": "polik.recovery.source_image_verification.v1",
        "created_at": now_iso(),
        "manifest": str(manifest_path),
        "image": str(image_path),
        "image_size_bytes": image_size,
        "expected_source_size_bytes": expected_size,
        "sample_sha256": sample,
        "sha256": full_hash,
        "checks": checks,
        "pass": passed,
        "next_step": "掃描這個 .img 映像檔，不要反覆掃描原始磁碟。" if passed else "先檢查成像是否中斷；若是壞軌，優先用 ddrescue map 續跑。",
    }
    out_path.write_text(json.dumps(report, ensure_ascii=False, indent=2), encoding="utf-8")
    write_html(report, html_path)
    print(json.dumps(report, ensure_ascii=False, indent=2))
    return 0 if passed else 2


if __name__ == "__main__":
    raise SystemExit(main())
