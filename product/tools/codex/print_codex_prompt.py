#!/usr/bin/env python3
"""Print the recommended Codex handoff prompt."""
from __future__ import annotations

from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
PROMPT = ROOT / "docs" / "plans" / "codex-agent-prompt.md"


def main() -> int:
    print(PROMPT.read_text(encoding="utf-8"))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
