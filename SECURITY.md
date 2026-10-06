# Security Policy

## Supported versions

目前沒有標示為 stable 或 production-ready 的支援版本。公開倉庫中的 `0.1.0` DMG 是未有效簽署、未 notarize 的歷史研究預覽，不建議用於重要資料或唯一副本。

## Reporting a vulnerability

請不要在公開 issue 張貼 exploit、憑證、個資、磁碟內容或可直接重現權限提升的完整步驟。

優先使用 GitHub repository 的 **Security → Report a vulnerability** 私下提交報告：

<https://github.com/polik18/polik-recovery/security/advisories/new>

若 private reporting 無法使用，可先建立不含技術細節的 issue，僅說明需要安全聯絡方式；maintainer 建立私下管道後再提供細節。

報告請包含：

- 受影響版本、commit 或 artifact SHA-256。
- 作業系統與硬體架構。
- 影響範圍與必要先決條件。
- 使用人工產生 fixture 的最小重現方式。
- 是否涉及 root helper、raw device、來源寫入、同碟輸出或 silent corruption。

請勿附上真實使用者資料、完整磁碟映像、私鑰或存取 token。

## Current release blockers

以下項目在修復與重新驗證前阻擋正式發布：

- canonical source 已移除舊快照中的 world-writable socket、AppleScript 提權、可預測 socket／token 與 force-unmount prototype；但正式版所需的 signed、authenticated、least-privilege helper 尚未實作與稽核。
- macOS imaging、desktop、CLI 與 recovery-plan 已拒絕來源／輸出共用 physical store 或身分不明；regular source 以 `O_NOFOLLOW` 開啟並核對 device/inode 與 physical stores，writer 也會核對實際 opened output handle，再以同一 handle 建立、提交與同步輸出。Windows resolver／handle 與平台整合仍未完成。
- 公開 DMG 簽章／Gatekeeper 驗證失敗且沒有 notarization ticket。
- 檔案系統能力尚未由目前版本的可重現 corpus 完整驗證。

為避免增加利用風險，本文件不包含 exploit 細節。修復工作依 [階段性改善計畫](POLIK_RECOVERY_STAGED_IMPROVEMENT_PLAN.md) 執行。

## Scope

安全範圍包括桌面 App、CLI、檔案系統 parser、privileged helper、映像建立工具、輸出 writer、release artifact 與本網站。第三方工具本身的漏洞應同時回報其上游專案。現行特權邊界與禁止回歸項目詳見 [privileged boundary](docs/security/privileged-boundary.md)。
