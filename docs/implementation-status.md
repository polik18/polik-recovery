# Implementation Status

> Updated: 2026-11-21
> Working baseline: `wp/P3-ntfs-batched-read@5a3e937`（usability checkpoint revision 35）
> Release status: blocked

## Stage checkpoint

| Stage | Status | Evidence / next gate |
|---|---|---|
| P0 — public-risk containment | `completed in working tree` | README/site corrected; download CTA removed; audit, capability matrix and security policy added |
| P1 — canonical source/license/version | `completed in working tree` | curated source, provenance manifest, MIT, `0.2.0-alpha.1`, Rust 1.85.1 and local build/test/lint evidence are present |
| P2 — privileged boundary | `prototype quarantined; redesign pending` | unsafe server/helper modes and CLI raw-read implementations removed; desktop, CLI and file-backed core reject raw-device paths; a signed, authenticated least-privilege helper is not implemented |
| P3 — physical-device isolation | `regular-image contract implemented` | macOS and Windows callers resolve physical stores and bind live source/output handles; disk-full injection and the valid data/manifest commit-gap reconciliation pass. Raw-device and abrupt-power-loss platform evidence remain blocked |
| P4 — parser/recovery evidence | `partial` | four deterministic synthetic corpus gates pass and run in CI; independent real-image, malformed-input and broader feature matrices remain |
| P5 — desktop reliability/security | `in progress` | P4/P5 usability work-package commits add spawn_blocking, structured scan pages, virtualized results and bounded thumbnail concurrency; global Tauri API, inline styles, broader path authority and job-token cancellation remain |
| P6 — platform validation | `Windows source gate partial` | Windows GNU workspace check/Clippy, 83 non-desktop tests and 4 release-profile desktop linked tests pass; release CLI builds. Formal MSVC, installer, VM matrix, signing and removable-media evidence remain |
| P7 — release supply chain | `free-only implementation in progress` | deterministic SHA-256/CycloneDX/license/provenance evidence generator and verifier added; manual public-repository GitHub/Sigstore attestation workflow is pinned and refuses private repositories. SignPath acceptance, Authenticode, installer evidence and macOS notarization remain blocked |
| P8 — website/docs | `first pass completed` | claims, CSP, external JS/CSS, keyboard/ARIA and local validators added; visual/browser audit remains |
| P9 — controlled pilot | `not started` | requires P2–P8 product gates |

## Completed changes

- Replaced unsupported public claims with evidence-scoped research-preview wording.
- Removed the primary direct DMG download link while preserving artifact identity and checksum for audit.
- Added release inventory, capability matrix, security policy and machine-readable release manifest.
- Recorded a read-only pre-import audit of the unversioned local source snapshot.
- Added ADR-0001 to prevent accidental public source disclosure before the owner decides the source model.
- Added a non-empty public-website CSP without `unsafe-inline` or `unsafe-eval`.
- Moved JavaScript and CSS out of the HTML document.
- Removed inline event handlers, inline styles, `innerHTML` use and local `file://` links.
- Added keyboard and ARIA behavior for reading modes, tabs and SVG mockup controls.
- Added local validation scripts and a commit-SHA-pinned GitHub Actions gate.
- Accepted ADR-0001 Option A, added the MIT License and imported the curated canonical workspace under `product/`.
- Preserved the untouched delivery snapshot with a complete SHA-256 provenance manifest.
- Unified Cargo/Tauri source metadata on `0.2.0-alpha.1` and pinned Rust 1.85.1.
- Removed the legacy standalone root helper, AppleScript elevation server, predictable socket/token flow, world-writable socket mode and automatic force-unmount path.
- Added a source-security regression gate and a commit-SHA-pinned macOS product CI workflow.
- Passed local workspace formatting, locked compilation and 109 Rust tests; the product toolchain remains pinned to Rust 1.85.1, while the latest local run used Rust 1.99.0. Python storage-parser and full workspace Clippy evidence is recorded per checkpoint and must be rerun under the pinned toolchain for release claims.
- Added a fail-closed physical-store identity contract that rejects unknown identities and any shared physical store before a verified writer call.
- Removed dd fallback and force-unmount controls; macOS imaging now stops before creating output when `diskutil` cannot prove source/output physical-store separation.
- Made all four synthetic fixture generators byte-reproducible and added a double-generation regression check.
- Passed carving, filesystem, fragmented/partial and APFS quality gates; the exact metrics and limitations are recorded in `docs/quality/synthetic-corpus.md`.
- Replaced the desktop App's null CSP with a restrictive baseline and a checked hash for its single inline script; `unsafe-eval` and inline script execution are disallowed.
- Removed timer-driven fake scan percentages; only NTFS record counts and post-discovery validation use determinate progress, while opaque parser work is explicitly indeterminate.
- Constrained the generic report opener to report/archive extensions or ordinary directories, rejected application bundles/executables, and removed the Windows `cmd /C start` shell path.
- Migrated every desktop NTFS/exFAT/FAT32/APFS/carving output branch to the verified writer; macOS resolves the hosting device with `df -P`, expands APFS physical stores with `diskutil -plist`, and checks isolation before creating output directories.
- Removed the obsolete scan/extract/batch helper CLI entry points, removed stale AppleScript diagnostics, and added tests proving desktop raw-device paths are rejected before open.
- Hardened recovered-file commits with destination-capacity preflight (exact bytes plus 5%, minimum 64 MiB, and manifest reserve), deterministic `.partial` staging, `create_new`, file sync, no-replace commit and Unix parent-directory sync; deterministic disk-full injection and data/manifest crash-gap reconciliation are now covered, while abrupt-power-loss testing remains a release blocker.
- Bound Unix/macOS recovered-file creation, no-replace commit and directory sync to one verified directory handle; reject symlink components, compare the opened directory's device/inode to the canonical target, and test that path replacement cannot redirect writes. The equivalent regular-image Windows handle binding and no-replace commit path is now implemented and tested.
- Consolidated the desktop's duplicated macOS storage resolver into safe-writer, tagged identities by trusted provenance, rejected caller-asserted store strings at the public writer boundary, and used `fstatfs` plus `diskutil` to require the opened output handle's physical-store set to equal its preflight identity. Windows now resolves opened handles through volume GUIDs and physical disk extents; raw-device evidence remains blocked on the P2 privileged-helper design.
- Opened regular image sources with `O_NOFOLLOW`, verified their post-open file type, device/inode and physical-store set, and made every public writer call require a `VerifiedSourceStorage` token tied to the parser's live `FileBlockSource`. CLI, desktop and recovery-plan callers now use the same verified handle that supplies recovered bytes.
- Removed CLI `raw-info`/`raw-hash` implementations and the generic file source's raw-device ioctl fallback; regular image-file checks now reject `/dev/*`, Windows device namespaces and non-regular files before open.
- Migrated CLI NTFS/exFAT/FAT32/APFS/carving and recovery-plan output to verified storage identities. Production macOS builds reject same-store output; the debug-only quality override is path-confined to `product/fixtures` → `product/target` and is unavailable in release builds.
- Added Windows opened-handle storage resolution using volume GUIDs plus `IOCTL_VOLUME_GET_VOLUME_DISK_EXTENTS`, source volume-serial/file-index binding, reparse-point rejection and output-parent identity checks before recovered bytes are written.
- Added Windows `CREATE_NEW` partial staging, file flush, no-replace `SetFileInformationByHandle` commit and post-commit parent-handle verification; desktop Windows extraction now uses the canonical resolver.
- Added deterministic disk-full injection and strict reconciliation for the valid data-final/manifest-partial crash gap; mismatched or ambiguous states remain untouched and fail closed.
- Established a free/open-source Windows validation route with Rust 1.85.1 GNU plus LLVM-MinGW; workspace check and Clippy pass, 83 non-desktop Rust tests plus 4 release-profile desktop linked tests pass, and an optimized CLI is buildable. The GNU debug desktop cdylib still exceeds the PE export limit, while formal MSVC and installer validation remain release gates.
- Accepted the free-only P7 supply-chain decision; added deterministic release evidence generation and verification for artifact SHA-256, CycloneDX 1.5 SBOM, Cargo license screening, capability snapshot, build metadata and unsigned in-toto/SLSA-formatted provenance.
- Added a manual, non-publishing Windows evidence workflow pinned to exact action commits. It refuses non-public repositories before build, runs the MSVC source gates, creates a free Sigstore public-good attestation, verifies the evidence bundle and uploads only a short-lived workflow artifact.

## Verification commands

```sh
./scripts/check-public-content.sh
python3 scripts/validate-public-files.py
python3 scripts/check-product-security.py
python3 scripts/check-product-frontend.py
node --check assets/site.js
sh -n scripts/check-public-content.sh
cd product
cargo fmt --all -- --check
cargo check --workspace --all-targets --locked
cargo test --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
python3 tools/quality/check_fixture_reproducibility.py
cargo build --locked -p polik-recovery-cli
python3 tools/quality/run_quality_gate.py
python3 tools/quality/run_fs_quality_gate.py
python3 tools/quality/run_fragmented_quality_gate.py
python3 tools/quality/run_apfs_quality_gate.py
cargo build --release --locked -p polik-recovery-cli
# Verify release raw-device, same-store and fixture-override requests fail closed.
git diff --check
```

These checks validate public content, local links, CSP policy, release-manifest consistency, historical DMG size/SHA-256, quarantined security patterns, JavaScript syntax, Rust compilation, synthetic tests and lint. They do not validate a signed desktop artifact, real-device behavior or recovery correctness on an independent corpus.

On Windows, run `powershell -File scripts/check-windows-product.ps1`; it only uses the pinned Rust toolchain and an already-extracted free/open-source LLVM-MinGW installation and never opens a raw device.

## Current blocker

P1 and the P3 regular-image Windows parity work are complete in their work-package commits. Release remains blocked on the P2 production privilege design, P3 raw-device/abrupt-power-loss evidence, P4 correctness corpus, P5 desktop hardening, P6 formal MSVC/installer/platform evidence, and P7 signed/notarized release automation.
