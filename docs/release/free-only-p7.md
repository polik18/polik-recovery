# P7 免費供應鏈路徑

POLIK Recovery 的 P7 自本文件起採 **free-only**：自動化不得要求付費
憑證、付費 CI 或付費簽署服務。免費不代表降低發布門檻；沒有可信簽署的
平台產物必須繼續標示為不可公開發布。

## 可立即執行

- Rust、Cargo、Python 與 Git 產生 release artifact、CycloneDX SBOM、
  SHA-256、license screen、capability snapshot 及 in-toto/SLSA 格式 metadata。
- `cargo-deny 0.20.2` 檢查 RustSec advisories、SPDX licenses、registry／Git
  sources、wildcard dependencies 與重複版本；CI action 固定到完整 commit SHA。
- 公開 GitHub repository 使用免費的 GitHub Actions 與 keyless artifact
  attestation。Actions 必須 pin 到完整 commit SHA。
- Windows 開源專案可申請 SignPath Foundation 免費簽署；是否接受由該基金會
  決定，申請中或遭拒絕時不得宣稱已有 Authenticode 信任。

## 明確限制

- 自簽憑證只可測試，不能滿足公開 Windows Authenticode gate。
- 一般開發者沒有免費的 Apple Developer ID/notarization 路徑。因此 macOS
  可完成建置、SBOM、checksum 與本地 ad-hoc 測試，但正式下載必須保持封鎖。
- SHA-256 只證明 byte identity；本地 unsigned provenance 不是可信 attestation。
- 免費服務若改為收費，workflow 必須 fail closed，不得自動升級付費方案。

## 本機產生證據

```powershell
python scripts/check-free-p7.py
python scripts/release/generate_release_evidence.py `
  --artifact product/target/release/polik-recovery.exe `
  --output product/target/release-evidence/windows-x86_64
python scripts/release/verify_release_evidence.py `
  product/target/release-evidence/windows-x86_64
```

輸出中的 `signing-status.json` 會誠實記錄簽署與 attestation 狀態。工作樹不乾淨、
license screen 失敗或沒有外部 attestation 時，`public_release_eligible` 必須是
`false`。

## 尚需人工處理

1. 將目前未提交的 canonical source 正式 review、commit、push。
2. 確認 repository 維持公開，避免 GitHub-hosted runner／attestation 轉成付費用量。
3. 依 SignPath Foundation 條件準備申請；不得把 acceptance 當成既成事實。
4. macOS 正式發布持續封鎖，除非未來出現符合 free-only 的可信 notarization 路徑。
