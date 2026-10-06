#!/usr/bin/env python3
"""Validate static public files without network access or third-party packages."""

from __future__ import annotations

import hashlib
from html.parser import HTMLParser
import json
from pathlib import Path
import re
import sys
from urllib.parse import unquote, urlsplit


ROOT = Path(__file__).resolve().parent.parent
INDEX = ROOT / "index.html"
README = ROOT / "README.md"
MANIFEST = ROOT / "docs" / "release-manifest.json"


class PublicHTMLParser(HTMLParser):
    def __init__(self) -> None:
        super().__init__(convert_charrefs=True)
        self.ids: list[str] = []
        self.references: list[tuple[str, str]] = []
        self.inline_handlers: list[tuple[str, str]] = []
        self.inline_styles: list[str] = []
        self.inline_scripts = 0
        self.csp: str | None = None

    def handle_starttag(self, tag: str, attrs: list[tuple[str, str | None]]) -> None:
        values = dict(attrs)
        if element_id := values.get("id"):
            self.ids.append(element_id)

        for name, value in attrs:
            if name.startswith("on"):
                self.inline_handlers.append((tag, name))
            if name == "style":
                self.inline_styles.append(tag)
            if name in {"href", "src"} and value:
                self.references.append((name, value))

        if tag == "script" and not values.get("src"):
            self.inline_scripts += 1

        if (
            tag == "meta"
            and values.get("http-equiv", "").lower() == "content-security-policy"
        ):
            self.csp = values.get("content")


def fail(errors: list[str], message: str) -> None:
    errors.append(message)


def validate_html(errors: list[str]) -> None:
    parser = PublicHTMLParser()
    parser.feed(INDEX.read_text(encoding="utf-8"))

    duplicates = sorted({item for item in parser.ids if parser.ids.count(item) > 1})
    if duplicates:
        fail(errors, f"duplicate HTML ids: {', '.join(duplicates)}")

    if parser.inline_handlers:
        rendered = ", ".join(f"<{tag} {attr}>" for tag, attr in parser.inline_handlers)
        fail(errors, f"inline event handlers are forbidden: {rendered}")

    if parser.inline_styles:
        fail(errors, f"inline style attributes are forbidden: {len(parser.inline_styles)} found")

    if parser.inline_scripts:
        fail(errors, f"inline scripts are forbidden: {parser.inline_scripts} found")

    if not parser.csp:
        fail(errors, "Content-Security-Policy meta element is missing")
    else:
        if "script-src 'self'" not in parser.csp:
            fail(errors, "CSP must restrict scripts to self")
        if "'unsafe-eval'" in parser.csp:
            fail(errors, "CSP must not allow unsafe-eval")
        if "'unsafe-inline'" in parser.csp:
            fail(errors, "CSP must not allow unsafe-inline")

    id_set = set(parser.ids)
    for attribute, reference in parser.references:
        parsed = urlsplit(reference)
        if parsed.scheme in {"http", "https", "mailto"} or reference.startswith("//"):
            continue
        if parsed.scheme:
            fail(errors, f"unsupported public URL scheme in {attribute}: {reference}")
            continue
        if parsed.fragment and not parsed.path and parsed.fragment not in id_set:
            fail(errors, f"missing fragment target: #{parsed.fragment}")
        if parsed.path:
            target = (ROOT / unquote(parsed.path)).resolve()
            try:
                target.relative_to(ROOT.resolve())
            except ValueError:
                fail(errors, f"local reference escapes repository: {reference}")
                continue
            if not target.exists():
                fail(errors, f"missing local {attribute} target: {reference}")


def validate_markdown_links(errors: list[str]) -> None:
    link_pattern = re.compile(r"\[[^]]+\]\(([^)]+)\)")
    for source in (README, ROOT / "SECURITY.md"):
        for raw_target in link_pattern.findall(source.read_text(encoding="utf-8")):
            target_text = raw_target.split("#", 1)[0]
            parsed = urlsplit(target_text)
            if not target_text or parsed.scheme in {"http", "https", "mailto"}:
                continue
            target = (source.parent / unquote(parsed.path)).resolve()
            if not target.exists():
                fail(errors, f"{source.name}: missing Markdown target {raw_target}")


def validate_manifest(errors: list[str]) -> None:
    data = json.loads(MANIFEST.read_text(encoding="utf-8"))
    if data.get("public_release_enabled") is not False:
        fail(errors, "research preview must keep public_release_enabled=false")

    artifacts = data.get("artifacts")
    if not isinstance(artifacts, list) or len(artifacts) != 1:
        fail(errors, "release manifest must contain exactly one audited historical artifact")
        return

    entry = artifacts[0]
    artifact = ROOT / str(entry.get("filename", ""))
    if not artifact.is_file():
        fail(errors, f"manifest artifact is missing: {artifact.name}")
        return

    if artifact.stat().st_size != entry.get("bytes"):
        fail(errors, f"artifact size does not match release manifest: {artifact.name}")

    digest = hashlib.sha256()
    with artifact.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    if digest.hexdigest() != entry.get("sha256"):
        fail(errors, f"artifact SHA-256 does not match release manifest: {artifact.name}")

    if entry.get("download_recommended") is not False:
        fail(errors, "historical unsigned artifact must not be marked as recommended")


def main() -> int:
    errors: list[str] = []
    validate_html(errors)
    validate_markdown_links(errors)
    validate_manifest(errors)

    if errors:
        for error in errors:
            print(f"ERROR: {error}", file=sys.stderr)
        return 1

    print("Public files, links, CSP and release manifest are consistent.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
