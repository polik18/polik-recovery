# Decision 0003 — Accept quick-xml / time advisories for P7 free-only release

- **Status:** Accepted
- **Date:** 2026-10-06
- **Component:** `product/deny.toml` `[advisories] ignore`
- **Related:** D0002 (free-only supply chain), D0001 (MSRV 1.85.1)

## Context

`cargo-deny check` reports three advisories, all pulled in **transitively** through the
Tauri desktop stack, not through any recovery logic in our crates:

| RUSTSEC | Crate | Severity | Triggered by |
|---|---|---|---|
| RUSTSEC-2026-0194 | quick-xml 0.38.4 | High (Quadratic run time) | Duplicate attribute-name check in XML parsing |
| RUSTSEC-2026-0195 | quick-xml 0.38.4 | High | Unbounded namespace-declaration allocation in `NsReader` |
| RUSTSEC-2026-0009 | time 0.3.45 | High | RFC 2822 stack-exhaustion DoS |

Dependency chains:

```
plist 1.7.4 -> quick-xml 0.38.4
tauri-utils 2.9.3 -> tauri 2.11.2 -> polik-recovery-desktop (plist + time/cookie)
```

Our recovery crates (`data_recovery_*`) do **not** use quick-xml or time directly; the
vulnerability surface is entirely within Tauri's plist/XML handling and cookie date parsing.

## Options considered

### Option A — Accept and document (chosen)
Keep MSRV pinned at 1.85.1 (the P7 release requirement). Add the three advisories to
`deny.toml` `[advisories] ignore` with justification.

### Option B — Bump MSRV to 1.88
`cargo update -p plist --precise 1.10.0` + `cargo update -p quick-xml --precise 0.41.0`
clears quick-xml. **Blocking issue:** plist 1.9+ requires Rust 1.88.0, and time 0.3.47+
requires Rust 1.86.0 — both exceed our pinned MSRV 1.85.1. There is no plist version that
relaxes the quick-xml `^0.38.0` pin while staying under Rust 1.85.1. This would force a
release-specification change (lower MSRV floor, CI/MSRV gate, docs), which is a product
decision, not a dependency fix.

## Decision

Choose **Option A**. Rationale:

1. **MSRV contract is unchanged.** The P7 free-only release still targets Rust 1.85.1.
   Bumping to 1.88 is a release-specification change that must be decided separately.
2. **Not in our attack surface.** quick-xml parses plist/XML inside Tauri; time parses
   RFC 2822 dates inside Tauri's cookie handling. Neither is exercised by our recovery
   logic. The advisories are DoS-type and require crafted input (repeated attribute names,
   adversarial RFC 2822 strings) that our code paths do not feed.
3. **No safe upgrade path under MSRV 1.85.** See Option B — the pin chain is a hard deadlock
   until MSRV is raised.
4. **Free-only constraint respected.** No paid tools, signing, or CI were used. Ignoring
   advisories is a free, in-repo policy decision.

## Mitigations

- The three advisories are explicitly named in `deny.toml` with full justification so the
  ignore is auditable, not a blind blanket.
- Revisit when MSRV is next raised (planned Option B path) — quick-xml 0.41.0 (MSRV 1.79)
  and time 0.3.47 (MSRV 1.86) will then clear automatically.

## Verification

```bash
cargo-deny check   # advisories: PASS (3 ignored with justification)
```
