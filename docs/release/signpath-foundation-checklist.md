# SignPath Foundation 免費 OSS 簽署申請清單

> 這是準備清單，不代表 SignPath Foundation 已接受本專案，也不會自動建立帳號或送件。

## 已具備

- [x] Public canonical repository：<https://github.com/polik18/polik-recovery>
- [x] OSI 類型授權：repository root `LICENSE`（MIT）
- [x] 原始碼與 Windows build workflow 可公開檢視
- [x] pinned GitHub Actions 與 locked Rust dependencies
- [x] 自動產生 SHA-256、CycloneDX SBOM、license screen 與 provenance metadata
- [x] `SECURITY.md` 與已知限制文件
- [x] 不把憑證、私鑰或 token 提交到 Git

## Owner 必須補充或確認

- [ ] SignPath 申請聯絡人及公開 maintainer／reviewer 名單
- [ ] 專案下載頁與功能說明符合 SignPath 的「已發布且有文件」資格
- [ ] 全部 runtime／installer component 均為 OSS 或允許的 system library
- [ ] 隱私聲明涵蓋桌面程式的網路行為及第三方元件
- [ ] 確認可接受簽章 publisher 顯示為 SignPath Foundation
- [ ] SignPath policy／artifact configuration 經 owner review

## 接受後才能進行

1. 將 SignPath 提供的 project／organization identifiers 設為 GitHub environment
   configuration；不得寫入 source。
2. 只允許 protected tag、通過 P2–P6 gate 的 commit 進入簽署 policy。
3. 對 EXE／installer 執行 Authenticode 與 timestamp 驗證。
4. 從乾淨 Windows VM 下載實際 artifact，驗證 publisher、hash、安裝及移除。
5. acceptance、簽署與驗證證據完成前，Windows 下載按鈕保持停用。
