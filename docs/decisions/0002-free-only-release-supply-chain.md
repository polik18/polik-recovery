# ADR-0002: Free-only release supply chain

- Status: `accepted`
- Date: 2026-10-06
- Owner: project owner（使用者要求所有作法必須免費）

## Decision

P7 的建置、驗證、SBOM、provenance、attestation 與平台簽署只能採用不收費
方案。開源方案優先，但免費的 proprietary platform SDK／runner 可在無自動付費或
升級風險時使用。

1. 本機證據只使用 repository code、Python、Git、Rust 與 Cargo。
2. 公開 repository 可使用 GitHub standard hosted runners 與 Sigstore public-good
   artifact attestation；workflow 在 repository 非公開時立即失敗，且不得建立 release。
3. Windows 正式 Authenticode 僅考慮 SignPath Foundation 免費 OSS 路徑；在外部
   acceptance 前保持 unsigned／release blocked。
4. 自簽憑證不得當成公開信任證據。
5. macOS 沒有一般可用的免費 Developer ID/notarization 路徑，因此正式 macOS
   發布保持封鎖，不以 ad-hoc 簽署冒充完成。
6. 免費服務若改變資格或價格，流程必須 fail closed，不得自動接受付費方案。

## Consequences

- 可以免費完成 checksum、CycloneDX SBOM、license screen、in-toto/SLSA metadata
  與公開 repository 的 keyless attestation。
- Windows 是否能免費完成平台信任取決於 SignPath Foundation 的審核結果。
- free-only 限制下，P7 的 macOS notarization 驗收條件目前無法關閉。
- artifact attestation 不等於 Authenticode 或 Apple notarization，文件與 UI 必須區分。
