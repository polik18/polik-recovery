# Polik Recovery product workspace

This directory contains the canonical open-source Rust/Tauri product workspace. It was imported from the preserved local delivery snapshot recorded in [`../docs/audit/source-snapshot-sha256.txt`](../docs/audit/source-snapshot-sha256.txt).

## Status

The source is published for audit and continued development under the repository's [MIT License](../LICENSE). It is **not a stable recovery release**. In particular:

- legacy macOS privileged-helper prototypes were removed; no production helper exists yet, so raw-device operations fail closed;
- source/output physical-device isolation has a fail-closed identity contract, but platform resolvers and caller migration are incomplete;
- strict real-image APFS evidence is incomplete;
- the historical DMG is not a build product of this canonical workspace;
- signing, notarization and clean-machine release gates are not complete.

Do not run the current product against real devices or the only copy of important data.

## Layout

- `crates/`: Rust recovery engine, parsers, CLI and platform-device layers.
- `src-tauri/`: desktop application backend and Tauri packaging configuration.
- `apps/desktop-prototype/`: desktop WebView frontend.
- `fixtures/`: small synthetic test inputs and expected manifests.
- `tools/`: fixture, quality, session and macOS workflow tooling.

Historical implementation reports and machine-specific handoff logs were intentionally not imported as canonical documentation because they contain stale claims and local paths. Public capability truth lives in [`../docs/capability-matrix.md`](../docs/capability-matrix.md).

## Toolchain

The workspace pins Rust in `rust-toolchain.toml`. From this directory:

```sh
cargo fmt --all -- --check
cargo check --workspace --all-targets --locked
cargo test --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
```

These commands only compile or run synthetic fixtures. Do not invoke CLI commands with `/dev/disk*`, `/dev/rdisk*`, `\\.\PhysicalDrive*` or another real device.

The four deterministic synthetic quality gates and their current limits are documented in [`../docs/quality/synthetic-corpus.md`](../docs/quality/synthetic-corpus.md). Passing them is regression evidence for those exact fixtures, not a real-world recovery-rate claim.

## Desktop prototype

The desktop source remains an audit target, not a production-ready App. Building a release bundle is intentionally excluded from the initial CI gate until the privileged boundary, CSP/capabilities, product version and signing pipeline satisfy the staged plan.

## Security

See [`../SECURITY.md`](../SECURITY.md). Do not publicly post exploit details for the privileged prototypes. Contributions must not weaken a failed-closed safety gate merely to make a test pass.
