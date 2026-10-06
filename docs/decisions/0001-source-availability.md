# ADR-0001: Canonical Source Availability and License

- Status: `accepted — Option A`
- Date: 2026-10-02
- Owner: project owner（使用者於 2026-10-02 選擇 A）

## Context

The public repository currently contains a landing page, documentation and a historical DMG, but no product source or LICENSE. A separate local delivery snapshot exists without `.git` history. Historical public copy claimed that the whole engine, CLI and desktop App were open source and MIT licensed; the public evidence does not support that claim.

Publishing the snapshot without review could expose local paths, personal data, stale reports, insecure privileged prototypes or material whose license has not been confirmed. Keeping it private while continuing to claim full open source would also be misleading.

## Option A — genuinely open source

1. Preserve a hash inventory of the original snapshot.
2. Audit secrets, personal data, fixtures, third-party licenses and build artifacts.
3. Remove local-only reports and unsafe release prototypes as appropriate, without falsifying provenance.
4. Import the audited source into a canonical public repository.
5. Add an owner-approved LICENSE and dependency license policy.
6. Make public CI the authoritative build/test evidence.

## Option B — proprietary source

1. Create a private canonical repository with access controls and history.
2. Keep this public repository for the website, public documentation and signed releases only.
3. Remove open-source and MIT claims everywhere.
4. Publish required notices, third-party licenses, SBOM and security contact without publishing proprietary source.

## Decision

採用 **Option A — genuinely open source**：

- canonical source 匯入本 repository 的 `product/`，保留網站根目錄與既有公開歷史；
- 採用來源 workspace 已宣告的 MIT license，正式 `LICENSE` 位於 repository 根目錄；
- 原始 delivery snapshot 保持唯讀，完整 SHA-256 provenance manifest 存於 `docs/audit/source-snapshot-sha256.txt`；
- `target/`、`.DS_Store`、`__pycache__`、`.pyc`、舊工作日誌、交接報告與未鎖版 release workflow 不匯入；
- 高風險 privileged prototypes 可供安全稽核，但阻擋任何正式發布，直到 P2 完成。

## Acceptance record

Canonical repository: <https://github.com/polik18/polik-recovery>  
Canonical product workspace: `product/`
