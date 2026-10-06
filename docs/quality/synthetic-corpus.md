# Synthetic Recovery Corpus

> Last local run: 2026-10-02 with Rust 1.85.1 and canonical source `0.2.0-alpha.1`.

## Reproducibility

All generators use fixed metadata timestamps, deterministic input bytes and fixed ZIP entry metadata. `tools/quality/check_fixture_reproducibility.py` generates every corpus twice in the same temporary tree and compares SHA-256 for every file. The current run reports 19 byte-identical files.

Generated fixtures contain no user disk data. Build and report output goes under ignored `product/target/`.

## Current gates

| Gate | Scope | Current result |
|---|---|---|
| Carving | PNG, JPEG, PDF, Office ZIP, MP4 and HEIC signatures plus malformed/padding negatives | PASS: 6 of 8 expected items found (75%), 100% high-confidence usability, 0 false positives, 2 misses |
| Filesystem | Small NTFS, FAT32 and exFAT allocated/deleted scan and extraction cases | PASS: 100% expected scan entries and 100% extracted byte hashes |
| Fragmented/partial | FAT32 and exFAT fragmented chains and deliberately partial cases | PASS: 100% scan entries, complete hashes and suspect classifications |
| APFS-like | Synthetic complete, missing-range partial and encrypted-metadata-hint records | PASS: 100% scan entries, extract hashes, quality labels, byte-loss fields and encryption hints |

The carving gate deliberately records its two current misses instead of describing the feature as complete. Its 75% threshold is a regression floor, not an acceptable production recovery rate.

## Reproduction

From `product/`:

```sh
python3 tools/quality/check_fixture_reproducibility.py
cargo build --locked -p polik-recovery-cli
python3 tools/quality/run_quality_gate.py
python3 tools/quality/run_fs_quality_gate.py
python3 tools/quality/run_fragmented_quality_gate.py
python3 tools/quality/run_apfs_quality_gate.py
cargo build --release --locked -p polik-recovery-cli
python3 tools/quality/check_release_fail_closed.py
```

The extraction gates intentionally use a debug-only storage identity pair after canonicalizing and confining the source to `product/fixtures/` and output to `product/target/`. This permits deterministic same-host fixture tests without weakening production isolation. Release builds reject that option; the release safety gate separately proves raw commands, same-physical-store extraction and the fixture override all fail before output creation.

The product CI runs the same gates and checks that regeneration does not modify committed fixtures.

## Non-claims and next corpus work

These results do not establish success rates on real damaged media, prove the historical DMG, or validate every parser branch. They do not cover full APFS OMAP/VFS traversal, snapshots, clones, compression, FileVault, broad NTFS compression/sparse/encryption cases, arbitrary corruption, fuzzing, memory pressure, unplug events or hostile input security.

Promotion beyond `experimental` requires independently sourced, legally controlled images; per-feature ground truth; byte-exact expected outputs; malformed/truncated adversarial corpora; sanitizer/fuzz evidence; and artifact-matched platform runs.
