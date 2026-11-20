# POLIK Recovery Capability Matrix

> Applies to public artifact `polik-recovery-desktop_0.1.0_aarch64.dmg`  
> SHA-256: `897967c1a83071a16e6086ff58ca111c22091430b6c904cd479206540169af30`  
> Updated: 2026-11-21
> Canonical alpha checkpoint: `wp/U4-structured-errors@2d5c8ef`（usability checkpoint revision 42；P2/P3/P4/P5/U1/U2/U3/U4 已完成，S2 遠端 CI 仍待 owner 授權）

## Status definitions

- `verified`: the exact public artifact passed a reproducible version-matched gate.
- `partial`: a defined subset passed, with explicit exclusions.
- `experimental`: implementation evidence exists, but correctness or packaging is not sufficiently validated.
- `planned`: not present in the public release.
- `unsupported`: known absent or deliberately rejected.
- `unverified`: insufficient evidence to make a safety claim.

## Platform and distribution

| Capability | Status | Evidence / limitation |
|---|---|---|
| macOS Apple Silicon bundle exists | `experimental` | arm64 binary is present; App was inspected but not executed |
| Intel macOS | `planned` | no x86_64 or universal artifact |
| Windows desktop | `planned` | canonical alpha now type-checks and passes Clippy on Windows, but no MSI/EXE, signed desktop candidate or GitHub Release exists |
| Valid Developer ID signature | `unsupported` | strict codesign verification fails |
| Apple notarization | `unsupported` | no stapled ticket |
| Rebuild from public source | `partial` | canonical source is in `product/`; product pins Rust 1.85.1 and checkpoint 42 records 109 local Rust tests plus locked workspace checks, but remote CI and release artifact reproduction are not yet evidenced |
| Bundled privileged helper | `unsupported` | helper is absent from 0.1.0 bundle |

## Input and safety boundary

| Capability | Status | Evidence / limitation |
|---|---|---|
| Scan a selected image file | `experimental` | recovery modules are linked; no artifact-matched end-to-end corpus |
| Read a raw macOS device | `unsupported` | 0.2.0-alpha.1 desktop, CLI and generic file source intentionally fail closed after unsafe prototypes and CLI raw reads were removed; no production helper exists |
| Source opened read-only | `partial` | public artifact lacks a version-matched gate; canonical alpha regular-image sources use `O_NOFOLLOW` on Unix or `FILE_FLAG_OPEN_REPARSE_POINT` on Windows, post-open regular-file checks and live-handle file/physical-store identity binding |
| Refuse output on source physical disk | `partial` | canonical macOS and Windows desktop/CLI/recovery-plan regular-image paths resolve every physical store and reject overlap/unknown identity; public artifact and raw-device helper paths remain unverified/unsupported |
| Image-first workflow | `experimental` | canonical alpha desktop flow is fail-closed for raw devices and requires a regular image; not verified in public artifact |
| Failing-media resumable imaging | `experimental` | ddrescue workflow is not artifact-matched and fallback behavior needs correction |
| Complete offline desktop operation | `unverified` | no version-matched network-behavior test |

## File systems

| Capability | Status | Evidence / limitation |
|---|---|---|
| NTFS volume/MFT parsing | `experimental` | canonical source passes a small deterministic scan/extract fixture at 100%; no artifact-matched or independent real-image corpus |
| NTFS resident/non-resident extraction | `experimental` | fragmented/compressed/sparse cases need independent gates |
| NTFS compressed files | `experimental` | no artifact-matched byte-exact corpus |
| NTFS ADS | `experimental` | no artifact-matched byte-exact corpus |
| exFAT volume/directory parsing | `experimental` | deterministic allocated/deleted and fragmented fixture gates pass at 100%; real-media breadth is unproven |
| exFAT deleted-file recovery | `experimental` | fragmentation and partial overwrite need ground truth |
| FAT32 parsing/recovery | `experimental` | deterministic allocated/deleted and fragmented fixture gates pass at 100%; real-media breadth is unproven |
| APFS container/volume parsing | `experimental` | deterministic synthetic complete/partial/crypto-hint gate passes at 100%; fixtures are APFS-like parser regressions, not full macOS volumes |
| APFS complete OMAP/VFS traversal | `experimental` | strict real-image OMAP/VFS gate is not proven |
| APFS extraction | `experimental` | clones, compression, snapshots and extents need separate gates |
| FileVault decryption | `unsupported` | no claim of decrypting locked volumes |

## File carving and output

| Capability | Status | Evidence / limitation |
|---|---|---|
| PNG/JPEG/PDF/ZIP carving | `experimental` | deterministic synthetic gate finds 6/8 expected items (75%) with 0 false positives; the historical F3 random-256 MiB corpus produced 11 false JPEGs before C1, and the corrected gate now records 0; independent precision/recall corpus remains absent |
| MP4 box-walking carving | `experimental` | fragmented files and false positives are not quantified |
| OLE/CFBF carving | `experimental` | no published precision/recall corpus |
| SHA-256 output manifest | `experimental` | hash can verify output bytes, not original-file correctness |
| Partial recovery reporting | `experimental` | exact unreadable ranges and zero-filled regions need schema-level evidence |
| Atomic output / crash recovery | `experimental` | canonical alpha checks capacity; macOS uses handle-relative staging and Windows binds reparse-safe opened file/directory handles to physical extents before flush/no-replace commit. Deterministic disk-full injection and the valid data-final/manifest-partial reconciliation path pass locally; abrupt power loss and exact release-artifact evidence remain absent |

## Promotion rule

An item may only move to `verified` when the exact release commit and artifact have:

1. a deterministic fixture or legally controlled real-image corpus;
2. expected metadata and byte hashes;
3. a reproducible test command;
4. CI or signed release evidence;
5. explicit failure and unsupported-case tests.

Finding a module name in a binary, compiling a crate, or listing some files is not sufficient.
