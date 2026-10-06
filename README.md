# Polik Recovery

Polik Recovery 是資料復原引擎與桌面介面的研究專案。目前公開倉庫處於**研究預覽階段**，不是可用於唯一副本、重要資料或正式鑑識工作的穩定版本。

## 目前公開狀態

| 項目 | 狀態 |
|---|---|
| 公開原始碼 | 已匯入 [`product/`](product/) 並採 MIT；乾淨環境重建驗證進行中 |
| macOS artifact | 歷史研究預覽，僅 Apple Silicon、未有效簽署、未 notarize |
| Intel macOS | 尚未提供 |
| Windows | 尚未提供 |
| privileged helper | 公開 DMG 中沒有 README 過去所稱的 helper |
| 檔案系統能力 | 尚待可重現 corpus 與 release gate 驗證 |

目前不提供建議下載入口。倉庫中的 `polik-recovery-desktop_0.1.0_aarch64.dmg` 只保留作為歷史稽核 artifact，不應用於唯一一份或高價值資料。

- 檔名：`polik-recovery-desktop_0.1.0_aarch64.dmg`
- SHA-256：`897967c1a83071a16e6086ff58ca111c22091430b6c904cd479206540169af30`
- Checksum 只用來核對下載 bytes，不能證明程式安全或復原結果正確。

完整證據與限制請見：

- [目前發布稽核](docs/audit/current-release-inventory.md)
- [能力矩陣](docs/capability-matrix.md)
- [安全政策](SECURITY.md)
- [階段性改善計畫](POLIK_RECOVERY_STAGED_IMPROVEMENT_PLAN.md)
- [目前實施進度](docs/implementation-status.md)

## 安全提醒

- 不要在唯一一份原始資料上測試本預覽版。
- 發生異音、頻繁斷線或大量 I/O error 時應停止直接掃描，優先使用具 mapfile 的工具製作映像，或尋求專業資料復原服務。
- 復原輸出必須放在不同的實體儲存裝置；不同資料夾或不同 APFS volume 不一定代表不同實體磁碟。
- SHA-256 可證明某一份輸出 bytes 未改變，不能證明它與遺失前的原檔完全相同。
- 加密、壓縮、稀疏、碎片化、已刪除及損壞檔案都需要各自的測試證據，不能由「辨識出檔名」推論為可完整復原。

## 能力說明

開發快照包含 NTFS、exFAT、FAT32、APFS、File Carving、VFS 與輸出 manifest 等實作，但目前公開 artifact 尚未通過版本一致的完整驗證。各能力以 [能力矩陣](docs/capability-matrix.md) 為唯一公開依據；沒有測試證據的項目維持 `experimental`、`planned` 或 `unsupported`。

## 原始碼與授權

專案 owner 已在 [ADR-0001](docs/decisions/0001-source-availability.md) 選擇真正開源。canonical Rust／Tauri workspace 位於 [`product/`](product/)，並依 [MIT License](LICENSE) 發布。

來源公開不代表目前版本已可安全處理真實磁碟。舊 delivery snapshot 的危險 privileged prototype 已從 canonical source 移除，但正式 helper 尚未重新實作，輸出隔離也尚未完成全 caller 遷移；正式 artifact 必須由這份 canonical source 在 CI 重建，並通過安全、正確性、簽署與 notarization gate。

## 隱私

目前沒有足夠的 release evidence 可對桌面 App 做絕對的「完全離線」承諾。正式版需要以網路行為測試與權限設定驗證。

行銷網站與桌面 App 是不同範圍：目前網站會載入 Google Fonts 與站點統計腳本，瀏覽頁面時可能向第三方／站點服務送出一般 HTTP 請求；這不代表磁碟內容會被上傳。

## 開發狀態

目前優先工作是修正公開宣稱、建立權威原始碼、重建 privileged boundary、證明來源／輸出位於不同實體裝置，以及建立可重現的檔案系統 corpus。正式下載會等安全、正確性、簽署與發布 gate 全部通過後再恢復。
