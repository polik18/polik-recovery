#!/bin/sh
set -eu

repo_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$repo_root"

failed=0

check_absent() {
  pattern=$1
  description=$2
  if grep -nE -- "$pattern" README.md index.html; then
    printf 'ERROR: %s\n' "$description" >&2
    failed=1
  fi
}

check_absent 'file://' 'public content contains a local file URL'
check_absent '接近 100%|完全開源|絕對不會|無記憶體安全風險' 'public content contains an unsupported absolute claim'
check_absent 'href="[^"]*\.dmg"' 'landing page exposes a direct DMG download before release gates pass'
check_absent 'Windows 版與 Intel macOS 版.*Releases.*下載' 'README claims unavailable platform downloads'

if [ "$failed" -ne 0 ]; then
  exit 1
fi

printf 'Public-content safety checks passed.\n'
