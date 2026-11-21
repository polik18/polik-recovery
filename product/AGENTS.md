# Product workspace instructions

Use Traditional Chinese when communicating with the project owner.

Before changing product code, read:

1. `../POLIK_RECOVERY_STAGED_IMPROVEMENT_PLAN.md`
2. `../docs/implementation-status.md`
3. `../docs/capability-matrix.md`
4. `../SECURITY.md`
5. `../docs/plans/2026-11-20-usability-correctness-performance-plan.md` and its checkpoint JSON (current usability/correctness/performance work packages; claim a WP there before starting)

Safety rules:

- Automated work may only use files under `fixtures/` or newly generated disposable images.
- Never access `/dev/disk*`, `/dev/rdisk*`, `\\.\PhysicalDrive*` or another real device without explicit human approval for that exact device.
- Approved exception (owner, 2026-11-21): `tools/macos/run_real_fs_image_gate.py --with-deleted` may attach the disposable `hdiutil` images it has just created under `fixtures/real_fs_generated/`, only at its private mount points under `target/real-fs-gate/mnt/`, delete its own test files there and detach by mount point. This covers local runs and the ephemeral Product CI runner; it does not extend to any other image, device or mount point.
- Never start, install or elevate a privileged helper during automated validation.
- Never use `sudo`, force-unmount a disk, or bypass source/output isolation.
- Keep unsupported claims at `experimental`, `planned`, `unsupported` or `unverified` until a version-matched gate passes.
- Preserve the original delivery snapshot; this workspace is the canonical, reviewable successor.

Minimum code gate:

```sh
cargo fmt --check
cargo check --workspace --all-targets
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

If the pinned toolchain is unavailable, report that exact limitation; historical logs are not test evidence.
