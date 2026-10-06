# POLIK Recovery 階段性改善計畫

> 狀態：`IMPLEMENTATION_IN_PROGRESS / PUBLIC_RELEASE_BLOCKED`
>
> 實施進度（2026-10-06）：P0、P1 已在工作樹完成；P2 已移除並封鎖舊危險 prototype、CLI raw reads 與通用 file source 的 device fallback，但正式 helper 尚待重新設計；P3 已把 macOS 與 Windows 的桌面、CLI、recovery-plan 接上 fail-closed physical-store preflight，Windows 以 volume GUID／disk extents 綁定實際 opened source/output handles，拒絕 reparse path，並完成 `.partial`、flush、no-replace、disk-full injection 與 data-final/manifest-partial reconciliation。Windows GNU workspace check／Clippy、83 個非桌面 Rust tests、4 個 release-profile 桌面 linked tests 及 release CLI build 已在本機通過；正式 installer、真實裝置／斷電與簽章證據仍待完成。P4 已建立可重現 synthetic corpus gate；P5 已加入桌面 CSP、誠實進度與報告開啟限制；P7 已依 owner 決策限制為 free-only，加入 deterministic SBOM／checksum／license／provenance bundle 與免費 public-repository Sigstore attestation workflow，但 SignPath acceptance、Windows Authenticode 及無免費路徑的 macOS notarization 仍未完成；P8 已完成第一輪公開內容工作。正式發布維持封鎖。
>
> 基準日期：2026-10-02
>
> 公開倉庫基準：`main@42458a4`（`feat: add live site statistics`）
>
> 已檢視公開 DMG：`polik-recovery-desktop_0.1.0_aarch64.dmg`
>
> DMG SHA-256：`897967c1a83071a16e6086ff58ca111c22091430b6c904cd479206540169af30`

## 1. 計畫目標

把 POLIK Recovery 從「技術展示頁面、不可重建的公開二進位檔，以及未納版控的開發快照」，改善成一套可以安全驗證、可追溯發布、對能力邊界誠實，而且不會因權限設計或輸出位置判斷而二次傷害資料的復原產品。

本計畫的優先順序是：

1. 防止安全事故與誤導使用者。
2. 建立唯一、可重建的權威原始碼與版本來源。
3. 證明唯讀與輸出隔離，而非只靠文件宣稱。
4. 用可重現測試證明各檔案系統功能。
5. 完成簽署、notarization、跨平台與可維護的發布流程。
6. 最後才擴張功能、效能與行銷宣稱。

預估投入為 30–50 個 agent-day，另加 Apple／Windows 簽章申請、真實硬體及測試磁碟準備時間。任何涉及真實故障媒體的驗證，必須由人員核准並在硬體寫入阻擋器或等效保護下進行。

## 2. 目前基線與關鍵證據

### 2.1 公開倉庫

公開倉庫目前只有：

- `index.html`：單檔技術文章與互動模擬器，約 1,700 行。
- `README.md`：產品能力與下載說明。
- `polik-recovery-desktop_0.1.0_aarch64.dmg`：直接納入 Git 的 macOS 二進位檔。

目前沒有可建置原始碼、`LICENSE`、測試、CI、發布腳本、SBOM 或可驗證的 release manifest。公開 GitHub Releases 頁面目前也沒有發布項目，但 README 宣稱 Windows 安裝程式可從 Releases 取得。

### 2.2 公開 DMG

DMG 已以唯讀方式掛載檢查，檢查後已卸載；未執行 App，也未讓程式接觸任何真實磁碟。

發現：

- App 只有 arm64 Mach-O，不是 Universal Binary。
- `Info.plist` 宣稱最低 macOS 10.13，但 Mach-O load command 的最低版本是 macOS 11.0。
- App 為 ad-hoc／linker signature，沒有 Developer ID TeamIdentifier。
- `codesign --verify --deep --strict` 失敗。
- `spctl` 拒絕，App 與 DMG 均沒有可用 notarization ticket。
- App bundle 沒有 README 所稱的 `polik-recovery-helper`、LaunchDaemon、XPC service 或其他 privileged helper。
- 二進位中可見 NTFS、exFAT、APFS、VFS、carver、safe-writer 等 Rust 模組痕跡，但這只能證明程式碼被連結，不能證明完整能力或復原正確性。

### 2.3 本機開發快照

另發現未納版控的開發快照：

`/Users/huangjianzhe/Desktop/polik-recovery-v2.7-delivery-package-patch`

它包含 Rust workspace、Tauri 桌面端、多個檔案系統 crate、工具、測試、文件與 CI 設定，但沒有 `.git`，不能視為可追溯的權威來源。其文件同時出現 v2.7、v2.8、v2.9、49／58／67 tests 等不同狀態；本機目前沒有 `cargo`／`rustc`，所以未能重新建置或重跑測試，歷史 log 不算本次驗證證據。

### 2.4 阻擋發布的問題清單

| ID | 等級 | 問題 | 直接風險 | 對應階段 |
|---|---|---|---|---|
| B-01 | Critical | root helper socket 設為 `0666`，沒有 peer credential、code-signature 或可靠身分驗證 | 任意本機程序可能濫用 root raw-disk 能力 | P0、P2 |
| B-02 | Critical | Tauri 原型以 AppleScript 提權，token 由 PID＋時間組成並放在命令列，socket 位於可預測的 `/tmp` 路徑且 `0666` | 權限提升、raw-disk 洩漏、任意路徑輸出風險 | P0、P2 |
| B-03 | Critical | root worker 可依 IPC 請求執行 scan／extract，且可能自動 `diskutil unmountDisk force` | 中斷使用中的系統、擴大 root 攻擊面 | P0、P2 |
| B-04 | Critical | safe-writer 只比較路徑／父目錄，沒有證明來源與輸出位於不同實體磁碟 | 復原輸出可能覆寫來源媒體 | P0、P3 |
| B-05 | High | README、網站、公開 DMG 與本機原始碼版本及能力互相矛盾 | 使用者依錯誤承諾操作故障媒體 | P0、P1、P8 |
| B-06 | High | DMG 未有效簽章、未 notarize，嚴格驗證失敗 | 無法建立發布來源與完整性信任 | P0、P7 |
| B-07 | High | 公開倉庫無可重建原始碼與 LICENSE，網站卻宣稱完全開源 | 授權、透明度與供應鏈風險 | P0、P1 |
| B-08 | High | APFS 真實 OMAP／VFS 嚴格 gate 已知未通過，文件卻有「完整 APFS」語意 | 可能產出錯誤或不完整復原結果 | P0、P4 |
| B-09 | High | 映像工具在 ddrescue 不存在時靜默退回 `dd conv=noerror,sync` | 對故障媒體產生不必要重讀與不透明零填充 | P3 |
| B-10 | Medium | UI 使用模擬進度百分比 | 使用者誤判卡住、完成度或中止時機 | P5 |
| B-11 | Medium | Tauri CSP 為 `null` 且啟用 global Tauri API | WebView 注入後的影響面過大 | P5 |
| B-12 | Medium | 公開頁面含兩個本機 `file://` 連結、外部字型／統計依賴、可及性缺口 | 壞連結、隱私敘述不精確、鍵盤操作障礙 | P0、P8 |

## 3. 不可妥協的安全原則

以下條件在所有階段都適用：

- 未取得人員明確核准，CLI agent 不得讀取、掛載、卸載或掃描 `/dev/disk*`、`/dev/rdisk*` 或 Windows 實體磁碟。
- 自動化測試只能使用程式生成的 sparse image、fixture 複本或明確標示的犧牲磁碟映像。
- 不得啟動現有 root helper，也不得為了「驗證」而降低 socket、Gatekeeper、SIP、權限或簽章限制。
- 在來源與輸出實體裝置無法被可靠辨識時，預設拒絕執行，不能用警告代替 hard fail。
- 不得把 checksum 說成復原正確性的證明。Checksum 只能證明指定 bytes 在某個步驟前後一致。
- 功能宣稱必須由目前版本的自動化 gate 與 release evidence 支持；沒有證據就標為實驗性、部分支援或不支援。
- 對損壞媒體預設採 image-first；直接掃描實體媒體必須是進階模式並清楚呈現風險。
- 不得把憑證、私鑰、notarization credentials、API token 或真實使用者磁碟資料提交到 Git。

## 4. 階段總覽與相依關係

| 階段 | 主題 | 先決條件 | 預估 | 發布門檻 |
|---|---|---|---:|---|
| P0 | 止血、公開資訊與風險封鎖 | 無 | 1–2 天 | 必須完成 |
| P1 | 權威原始碼、授權、版本統一 | P0 | 3–5 天 | 必須完成 |
| P2 | 特權邊界重設計 | P1 | 5–8 天 | 必須完成 |
| P3 | 實體裝置隔離與 image-first | P1、P2 介面確定 | 4–6 天 | 必須完成 |
| P4 | Parser／復原正確性證據 | P1 | 8–12 天 | 依宣稱完成 |
| P5 | 桌面端安全與長任務可靠性 | P2、P3 | 4–7 天 | 必須完成 |
| P6 | macOS／Windows 真機驗證 | P2–P5 | 4–7 天 | 依平台完成 |
| P7 | CI、簽署與發布供應鏈 | P1–P6 | 4–6 天 | 必須完成 |
| P8 | 網站與文件改造 | P0、能力矩陣穩定 | 2–4 天 | 必須完成 |
| P9 | 受控 pilot 與事故處理 | P2–P8 | 3–5 天 | 正式版前完成 |

P2、P3、P4 可以在介面契約穩定後部分平行，但不得跳過 P0 與 P1。P7 不得以簽署包裝尚未通過資料安全 gate 的程式。

## 5. P0：止血與公開資訊修正

### 目標

在尚未改動核心程式前，先阻止使用者把未簽署、能力未驗證的 App 當成可安全處理重要資料的正式產品。

### CLI agent 任務

1. 建立 `docs/audit/current-release-inventory.md`：
   - 記錄公開 commit、DMG 檔名、大小與 SHA-256。
   - 列出 bundle 檔案、架構、最低 OS、簽章與 notarization 結果。
   - 記錄實際 GitHub Releases 是否存在對應資產。
   - 每個結論附上可重跑的唯讀命令。
2. 建立 `docs/capability-matrix.md`，每項能力只允許以下狀態：
   - `verified`：目前 release artifact 通過指定測試。
   - `partial`：只支援明列的子集合。
   - `experimental`：有程式碼，但沒有足夠真實語料驗證。
   - `planned`：尚未交付。
   - `unsupported`：明確拒絕或無法安全處理。
3. 立即修改公開 README／網站：
   - 將目前 DMG 標記為「未簽署的研究預覽版」，或暫時移除主要下載按鈕。
   - 移除 Windows 安裝程式已可由 Releases 下載的敘述，直到 asset 真實存在且通過 gate。
   - 把「完整 APFS」、「接近 100%」、「絕對不會改動」、「無記憶體安全風險」、「完全開源」等絕對敘述改成可驗證的精確文字。
   - 移除或改寫兩個指向開發者桌面的 `file://` 連結。
   - 公開 DMG SHA-256，但明確說明它只用於下載完整性核對。
   - 將 App 離線處理與網站外部 Google Fonts／site statistics 分開描述。
4. 加入醒目的限制說明：
   - 不應直接操作唯一一份重要資料。
   - 故障媒體應先製作映像，必要時尋求專業復原服務。
   - APFS、壓縮、加密、稀疏檔、嚴重損毀的支援程度依矩陣為準。
5. 建立 `SECURITY.md`：
   - 提供私下回報管道。
   - 明確說明目前 preview 不應用於高價值或唯一副本。
   - 不公開利用步驟，但列出已知受影響版本與修復狀態。

### 驗收條件

- README、網站、下載頁與 capability matrix 沒有互相矛盾的支援宣稱。
- 所有公開下載均有 checksum、版本、平台、架構、最低 OS 與簽章狀態。
- `rg 'file://|接近 100%|完全開源|絕對不會|無記憶體安全風險'` 不再出現未加限定的行銷敘述。
- 未通過 P2、P3、P4、P7 前，正式版下載與「production ready」標示保持封鎖。

## 6. P1：建立唯一權威來源、授權與版本

### 目標

讓任一乾淨環境都能從一個有歷史、有授權、有版本規則的 repository 建置同一產品。

### 必須先由 owner 決定的分流

這是唯一不能由 CLI agent 自行猜測的產品決策：

- **真正開源**：對本機快照做 secrets、PII、fixture 與授權稽核後，把可發布原始碼匯入公開 repository，加入真實 LICENSE。
- **維持專有**：另建私有 canonical repository，公開倉庫只保留網站／下載資訊，並移除「完全開源」與 MIT badge／宣稱。

在 owner 選定前，agent 只能產生匯入清單、差異報告與風險報告，不得把桌面快照直接提交至公開 Git。

### CLI agent 任務

1. 保存來源快照證據：
   - 對快照建立唯讀 inventory 與全檔案 SHA-256 manifest。
   - 記錄來源路徑、日期與「沒有 `.git` 歷史」的限制。
   - 不移動、不改寫原始快照。
2. 建立匯入排除清單：
   - 排除 `target/`、`.DS_Store`、log、暫存報告、build artifacts、簽章資料。
   - 掃描 secrets、絕對本機路徑、姓名／學校／客戶資料、真實磁碟映像。
   - 大型 fixture 以可重建 generator 或受控 artifact storage 管理。
3. 依 owner 決策建立 canonical repo：
   - 保留公開網站歷史，產品 source 可放同 repo 的標準目錄，或分成明確的 public-site／private-product repos。
   - 首個 source commit 記錄快照 provenance，不能假造過去 commit history。
4. 若採開源路徑：
   - 加入經 owner 確認的 `LICENSE`。
   - 盤點所有 dependency license 與第三方工具再散布條件。
   - 讓 Cargo metadata、README badge 與 LICENSE 完全一致。
5. 建立唯一版本來源：
   - 建議使用 workspace package version 作為產品版本來源。
   - build script 產生 Tauri config、Info.plist、CLI `--version` 與 release manifest。
   - 消除 `0.1.0`、`2.4.0`、`2.7.0`、文件 v2.8／v2.9 的混雜。
6. 鎖定工具鏈：
   - 加入 `rust-toolchain.toml`，版本以專案實際可建置結果為準。
   - 提交 `Cargo.lock`。
   - 記錄 Node／package manager 版本；禁止 CI 臨時使用未鎖版的 `npx -y`。
7. 建立乾淨環境 bootstrap：
   - `scripts/bootstrap-check.sh` 只檢查，不靜默安裝或提權。
   - 缺少 Rust、Node、平台 SDK 時輸出精確安裝說明。
   - CI 是第一個可信綠燈；歷史文字紀錄不算通過。

### 驗收條件

- 新機 clone 後能依文件完成 reproducible build。
- 所有 crate、桌面 App、CLI、DMG／installer 顯示同一 SemVer 與 commit ID。
- `cargo metadata` 有正確作者、description、repository、license。
- source ownership、公開／私有邊界與 LICENSE 無矛盾。
- source import 沒有 secrets、真實使用者資料、build artifacts 或開發者絕對路徑。

## 7. P2：重建特權與 raw-device 邊界

### 目標

root 元件只做最小、可驗證的唯讀裝置開啟，不能接受任意掃描、任意輸出路徑或由任何本機程序呼叫。

### 立即封鎖

- 將目前 `0666` Unix socket helper 標為 `prototype-only`，release build 必須 compile-time 排除。
- 禁止以 `osascript ... with administrator privileges` 啟動同一 App binary 的 root worker。
- 禁止以 PID＋時間 token、命令列 secret、可預測 `/tmp` socket 作為正式驗證。
- 禁止 root worker 執行完整 parser、carver、extract 或寫入使用者指定路徑。

### 建議架構

```text
一般權限 UI／Core
    │ 受限、具身分驗證的 IPC
    ▼
已簽署 privileged helper
    │ 只驗證並唯讀開啟 allowlisted raw device
    ▼
受限 read-only handle／FD 交回一般權限程序
    │
    ├─ parser／scan：一般權限
    └─ output writer：一般權限、與來源實體磁碟隔離
```

### CLI agent 任務

1. 先撰寫 threat model：
   - 攻擊者包含同機非管理員程序、惡意 WebView 內容、惡意檔名／metadata、被換掉的 device node、symlink race、程序崩潰與拔除裝置。
   - 資產包含 raw disk bytes、其他使用者檔案、root 權限、復原輸出與 manifest。
2. macOS 正式設計使用受支援的 signed helper／XPC 路徑：
   - 優先評估 SMAppService privileged helper 或符合目前 macOS 支援政策的機制。
   - App 與 helper 必須使用同一 Developer ID team 簽署。
   - helper 驗證 calling process 的 audit token 與 designated requirement。
   - IPC endpoint 不得 world-writable；不得只靠可竊取 bearer token。
3. 將 helper API 壓縮至最小：
   - `list_candidate_devices()`：只回傳正規化裝置資訊。
   - `open_read_only_device(stable_id)`：重查 stable identity 後以唯讀方式開啟。
   - `close_handle()`／自動 lifecycle cleanup。
   - 不提供 arbitrary path read、extract、output write、shell command 或 force-unmount API。
4. Device allowlist 與 TOCTOU 防護：
   - stable ID 必須從 IOKit／Disk Arbitration 取得，不能接受 UI 傳入的任意 `/dev/...` 字串。
   - 開啟後用 `fstat`／平台 API 重新確認裝置 identity、類型與唯讀旗標。
   - 預設排除目前系統開機磁碟及其 APFS physical stores。
5. 移除自動 `unmountDisk force`：
   - 預設只提供使用者可理解的正常卸載流程。
   - 正常卸載失敗就停止；進階強制操作須獨立、再次確認且留下 audit event。
   - helper 不得在背景自行決定強制卸載。
6. 加入 resource limits：
   - 單次 read 上限、總量／速率限制、timeout、取消、client disconnect cleanup。
   - 拒絕 overflow、負值轉型、超出裝置大小與未對齊的大量請求。
7. 建立對抗測試：
   - 未簽署 client、不同 Team ID、其他本機使用者、竄改 App、重放 request、猜 socket、client crash。
   - device node 被置換、拔除、重新插入、容量改變、sleep／wake。
   - 驗證任何失敗都不會退回寬鬆模式。

### 驗收條件

- release binary 不含舊 root worker 與 `0666` socket 路徑。
- 未授權本機程序無法列出敏感資訊或讀取任何 raw-disk byte。
- root helper 不含檔案系統 parser、carver、輸出 writer、shell 執行或任意路徑功能。
- helper crash／client crash 後沒有殘留 root process、socket 或開啟的裝置 handle。
- security integration tests 在乾淨 macOS VM 與至少一台真機通過。

## 8. P3：來源／輸出實體隔離與 image-first

### 目標

從「路徑看起來不同」提升為「可證明來源與輸出不共用實體儲存裝置」，並讓故障媒體先成像成為正常流程。

### CLI agent 任務

1. 建立 `StorageIdentity` 抽象：
   - macOS：volume UUID、APFS container、physical store、BSD device、whole-disk identity。
   - Windows：volume GUID、disk number、storage device number、volume extents。
   - image file：host filesystem 與承載它的 physical stores。
2. Preflight 必須解析：
   - source device／image 的穩定身分。
   - output directory 經 canonicalization 後的實際 volume 與 physical device。
   - symlink、bind／mount alias、APFS volume 共享 container、外接磁碟重新掛載後的別名。
3. Hard fail 規則：
   - source 與 output 共用任何 physical store。
   - identity 不完整或平台查詢失敗。
   - output 空間不足以滿足估算＋安全餘量。
   - output path 穿越 symlink 或在寫入期間被替換。
4. 強化 safe-writer：
   - 以 directory handle／平台安全 API 相對開檔，減少 path race。
   - 寫入 `name.partial`，完成後 `fsync` 檔案與父目錄，再 atomic rename。
   - 預設 `create_new`，重名採可預測且不覆蓋的命名規則。
   - 對每個輸出記錄 hash、logical size、actual size、來源 extent 與錯誤區段。
5. Image-first 工作流：
   - UI 首頁優先提供「建立／選擇磁碟映像」，直接掃實體磁碟放在進階模式。
   - 故障／I/O error 媒體必須使用 ddrescue 或等效可恢復、可重試、有 mapfile 的工具。
   - 找不到 ddrescue 時停止並提供安裝／專業服務說明，不得靜默退回 `dd`。
   - 正常卸載失敗時停止；不得自動 force-unmount 後繼續。
6. 成像輸出：
   - 使用 `.partial` 與恢復 mapfile。
   - 記錄來源 stable ID、容量、sector size、開始／完成時間、工具版本、完整命令參數。
   - 精確記錄 unreadable ranges，不能只用零填充而不揭露。
   - full hash 為正常完成 gate；sample hash 只能做快速抽查。
7. Manifest schema：
   - 版本化 JSON schema，固定時間格式與 byte-order 表示。
   - 把 `recovered`、`partial`、`carved`、`metadata-only`、`failed` 分開。
   - 明確區分「輸出 bytes hash」與「已對照原檔真值」。

### 驗收條件

- 同一路徑、不同路徑同 volume、不同 APFS volume 同 container、symlink alias 均會被正確攔截。
- 裝置 identity 不明時必定拒絕，不得出現「仍要繼續」預設按鈕。
- 拔線、磁碟滿、權限改變、程序被 kill 後只留下可辨識 `.partial` 與一致 manifest。
- 對故障 fixture 的 unreadable ranges 能從 image mapfile 一路追到個別復原檔案狀態。

## 9. P4：檔案系統與復原正確性

### 目標

用明確 corpus、oracle 與 byte-level 比對，回答每個檔案系統「哪些情境真的能正確復原」。

### 9.1 先建立能力真值表

每個格式至少拆成以下維度，不得只寫「支援 NTFS／APFS」：

| 維度 | 範例狀態 |
|---|---|
| Volume 辨識 | verified／partial／unsupported |
| 已配置檔案列舉 | 同上 |
| 已刪除檔案列舉 | 同上 |
| 連續／碎片化資料 | 分開記錄 |
| 稀疏檔 | 分開記錄 |
| 壓縮檔 | 演算法與限制 |
| 加密檔 | unlocked-only／rejected |
| ADS／resource fork／xattr | 個別記錄 |
| 目錄結構與 Unicode | 個別記錄 |
| metadata 損壞 | 可降級行為 |
| byte-exact extraction | corpus 通過率 |

APFS 另列 OMAP、B-tree traversal、snapshots、clones、compressed files、FileVault／已解鎖來源。NTFS 另列 attribute list、resident／non-resident、runlist、compression、ADS、MFT mirror。exFAT／FAT32 另列 deleted entry、cluster chain、fragmentation 與長檔名。

### 9.2 Corpus 與 oracle

1. 建立 deterministic image generators：
   - 固定 seed、檔名、內容、fragmentation layout 與預期 hash。
   - 包含空檔、大檔、深層目錄、Unicode／正規化、極長檔名、重名、稀疏、壓縮、已刪除與部分覆寫。
2. 建立 corruption corpus：
   - boot sector、allocation table、MFT record、B-tree node、extent、directory entry 的單點與多點破壞。
   - 截斷映像、sector read error map、宣稱大小超過實際檔案。
3. 建立合法的真實樣本集：
   - 由專案自己產生、沒有個資的小型實體檔案系統映像。
   - 每個樣本附建立腳本、平台版本、工具版本與 ground-truth manifest。
4. Differential validation：
   - 與作業系統正常掛載結果、已知成熟唯讀工具，以及 ground truth 比對。
   - 工具結果不同時保留 evidence，不用多數決掩蓋差異。
5. 指標：
   - allocated file byte accuracy。
   - deleted-file precision／recall。
   - false positive rate（carver）。
   - partial recovery range accuracy。
   - metadata accuracy 與崩潰率。

### 9.3 Parser 安全性

1. 為 boot record、MFT／attribute／runlist、FAT chains、APFS nodes／OMAP、carver header parser 加入 fuzz targets。
2. 所有 size、offset、count、cluster math 使用 checked arithmetic。
3. 限制遞迴深度、node count、單檔 extent count、解壓倍率與總輸出大小。
4. 惡意映像不得造成 panic、OOM、無限迴圈、超界 read 或寫出 sandbox。
5. 對 parser errors 使用結構化類型，不能把所有錯誤吞掉後回傳空清單。

### 9.4 APFS 特別 gate

- 已知 strict real APFS VFS／OMAP gate 未通過，因此「完整 APFS」不得發布。
- 只有當真實、可重建 APFS corpus 的 OMAP、目錄 traversal、extent 與 byte hash 全部通過後，對應子能力才能升為 `verified`。
- clones、snapshots、FileVault、compression 如果沒有各自證據，必須維持 `experimental` 或 `unsupported`。
- 不得為了讓 gate 變綠而降低 assertion、跳過真實 corpus 或改用只驗證「找到一些檔案」的條件。

### 驗收條件

- 每個公開宣稱都能指到 test ID、fixture ID、版本與 CI artifact。
- fuzz smoke test 每次 PR 執行；長時間 fuzz 定期執行並保存 corpus／crash artifact。
- 所有 verified corpus 的輸出 hash 與 ground truth 完全一致。
- 部分復原不會標記為成功完整檔，且 manifest 精確列出缺失 ranges。
- APFS strict gate 未通過時，網站與 UI 不顯示「完整支援」。

## 10. P5：桌面端安全、可理解性與長任務可靠性

### 目標

把 Tauri UI 從原型提升為權限最小、資訊可信、能安全取消與恢復的長任務介面。

### CLI agent 任務

1. WebView hardening：
   - 設定非空 CSP，只允許必要的本機 resource。
   - 移除 `withGlobalTauri`，改用明確 import 與最小 capability。
   - 移除 inline script／inline event handler，避免 CSP 例外。
   - 禁止任意 navigation、外部內容載入與不必要的 network access。
2. IPC command 最小化：
   - UI 使用 opaque session／device／scan IDs，不直接傳任意 raw path 給高權限層。
   - 每個 command 驗證 state transition、session ownership、path scope 與參數上限。
   - capability files 逐項列出可用命令，不使用寬泛 default permission。
3. 輸出安全：
   - 使用者控制的檔名、metadata、錯誤訊息一律以 text node 呈現。
   - 系統化稽核所有 `innerHTML`，只保留完全靜態模板或安全 renderer。
   - 測試 `<script>`、HTML attributes、RTL／Unicode control chars、路徑穿越、保留名稱與超長名稱。
4. 真實進度：
   - 刪除 exFAT、FAT32、APFS、carver 的假百分比 timer。
   - 有可靠 denominator 才顯示百分比；否則顯示 indeterminate、目前階段、已讀 bytes／records 與 elapsed time。
   - UI 清楚區分 scan、index、preview、extract、hash、flush。
5. 任務狀態機：
   - `created → preflighted → running → cancelling → cancelled|failed|completed`。
   - cancellation 必須 cooperative 且有 timeout；不能讓 partial file 假裝完成。
   - crash／重啟後可讀 manifest 恢復狀態，但不得自動重啟 raw-disk 操作。
6. 大資料集效能：
   - tree／table virtualization、分頁查詢、bounded channel、背壓與記憶體上限。
   - 百萬筆 metadata fixture 不得把所有 row 同時塞入 DOM。
7. 可及性與風險文案：
   - 全鍵盤操作、明確 focus、ARIA state、色彩對比。
   - 不只靠顏色表示 complete／partial／failed。
   - 危險動作說明影響對象、可逆性與下一步，而不是只有「發生錯誤」。
8. 外部工具：
   - `diskutil`、`hdiutil`、`sips`、PowerShell、converter 等均使用固定 executable／argument array，禁止 shell interpolation。
   - 加入 timeout、輸出大小上限、exit status 與版本檢查。

### 驗收條件

- CSP 測試證明 inline script、遠端 script 與未授權 navigation 被阻擋。
- 惡意檔名不會形成 DOM／command／path injection。
- UI 顯示的百分比皆可由 core counter 重算；沒有合成進度。
- 取消、磁碟拔除、磁碟滿、App crash 後，沒有假成功與無主輸出。
- 百萬筆 fixture 在定義好的記憶體與互動延遲預算內可瀏覽。

## 11. P6：平台與硬體驗證

### macOS

- 統一 deployment target：Info.plist、Rust target、linker 與實際 API 必須一致。
- 明確選擇 arm64＋x86_64 individual artifacts 或 universal binary。
- 在最低支援 macOS、目前主流版與最新穩定版的乾淨 VM／真機測試。
- 驗證 APFS、HFS+ 承載的 image file、外接 USB／Thunderbolt、sleep／wake、裝置拔除。
- 驗證 helper 安裝、升級、移除、簽章變更與舊 helper cleanup。

### Windows

- 在真實 Windows runner／VM 驗證 `CreateFileW` raw-volume／physical-drive read-only 行為。
- 重新設計 Windows 權限邊界，不能直接照搬 macOS helper。
- 驗證 volume lock／dismount、UAC cancel、BitLocker locked／unlocked、long path 與 reserved filenames。
- 驗證 Authenticode installer 的 install、upgrade、repair、uninstall 與殘留 service。

### 共通

- 對 encrypted volume 採明確策略：只支援由 OS 已解鎖後的 logical source，或直接拒絕；不得暗示能破解加密。
- 建立平台 support matrix，沒有真機／VM gate 的平台不提供正式下載。
- 所有測試先用 generated images；實體媒體只用專用犧牲裝置。

### 驗收條件

- 支援矩陣中的每個 OS／架構都有乾淨環境 install→scan fixture→extract→uninstall evidence。
- 最低 OS 宣稱與 binary load commands 一致。
- Windows 未完成前，README 和網站保持 planned／experimental，沒有空的下載按鈕。

## 12. P7：CI、供應鏈、簽署與發布

### PR gate

- `cargo fmt --check`
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`
- `cargo test --workspace --all-features`
- schema／capability matrix consistency tests
- generated-image integration tests
- fuzz smoke tests
- dependency audit、license policy、SBOM generation
- frontend lint、unit tests、CSP／accessibility／broken-link tests

GitHub Actions 必須 pin 到 commit SHA；工具版本寫入 repository，不得在 CI 臨時下載不固定版本的 executable。

### macOS release gate

1. Release mode build，沒有 prototype helper 或 debug entitlement。
2. App 及正式 helper 使用 Developer ID 簽署。
3. `codesign --verify --deep --strict --verbose=2` 通過。
4. `spctl --assess --type execute` 通過。
5. DMG 簽署、送 notarization、staple，離線 ticket 驗證通過。
6. 在乾淨 Mac 下載實際 release asset，完成首次啟動與 fixture recovery smoke test。

### Windows release gate

1. Binary 與 installer Authenticode 簽章有效。
2. SmartScreen／乾淨 VM 安裝行為記錄。
3. install、upgrade、uninstall、service cleanup 測試通過。
4. 實際 release asset 在乾淨 VM 完成 fixture recovery smoke test。

### 發布產物

每次 release 至少包含：

- 平台／架構清楚的 App artifact。
- `SHA256SUMS` 與其簽章。
- SBOM。
- provenance／build metadata。
- capability matrix snapshot。
- release notes、known limitations、最低 OS、升級／回退說明。
- 對應 tag 與 commit；禁止手動替換同 tag 的資產。

二進位檔不再直接提交 Git。公開下載應指向真實 GitHub Release 或受控 artifact host。發布流程必須能撤回有問題版本、標記 compromised artifact，並保留舊 checksum／公告證據。

### 驗收條件

- 從乾淨 clone 可建立與 release manifest 一致的 artifact。
- macOS／Windows 平台 gate 全綠才建立對應下載按鈕。
- DMG 嚴格 codesign、Gatekeeper、notarization 全部通過。
- release page 真的存在文件宣稱的所有 asset。

## 13. P8：網站與文件

### CLI agent 任務

1. 將 1,700 行單頁拆成可維護的結構，或至少建立 build／lint／test 管線。
2. 下載卡片由 release manifest 產生版本、平台、架構、checksum 與 URL，避免手動漂移。
3. 用 capability matrix 產生支援表，不重複手寫互相矛盾的說明。
4. 移除所有本機 `file://` 路徑與不存在的 Releases 資產連結。
5. 隱私說明分開：
   - Desktop app 是否離線、是否有 telemetry。
   - Marketing site 使用 Google Fonts／site statistics 時會產生哪些第三方請求。
6. 避免絕對保證：
   - 「唯讀」改成說明如何由權限、API 與測試強制。
   - 「100%」改成 corpus 測試結果及版本。
   - 「Rust 無記憶體安全風險」改成 Rust 降低特定風險，但 parser、unsafe、FFI、權限與邏輯錯誤仍須測試。
7. 可及性：
   - SVG／互動元件能用鍵盤操作。
   - mode selector 有正確 button semantics 與 `aria-pressed`。
   - focus、heading、landmark、對比、reduced motion、語言標記通過自動與人工檢查。
8. 安全 header：
   - CSP、`frame-ancestors`、`nosniff`、referrer policy。
   - inline handler 全部移除。

### 驗收條件

- 自動 broken-link checker 通過，沒有 `file://` 或不存在的下載資產。
- 網站版本、checksum、支援能力與 release manifest 一致。
- 首屏清楚標示 preview／stable 與關鍵限制。
- Lighthouse／axe 基線納入 CI，鍵盤人工 smoke test 通過。

## 14. P9：受控 pilot 與正式發布判定

### Pilot 順序

1. 純 generated image corpus。
2. 受控、可重建的小型檔案系統映像。
3. 專用犧牲 USB／SSD，內容有 ground truth。
4. 模擬 I/O error、斷線、磁碟滿與程序中止。
5. 在硬體寫入阻擋器下讀取非敏感測試媒體。
6. 最後才允許自願測試者的副本資料；禁止唯一副本。

### 觀察項目

- source 裝置是否有任何 write attempt。
- helper 安裝／卸載與 privilege audit events。
- 讀取重試與裝置健康惡化趨勢。
- 復原 precision／recall、byte hash、partial ranges。
- App crash、hang、memory growth、取消延遲。
- UI 是否讓使用者誤解成功、完整性或輸出位置。

### 事故手冊

建立 `docs/incident-response.md`，至少涵蓋：

- 發現 source write、錯誤 force-unmount 或同碟輸出。
- helper 被未授權 client 呼叫。
- release asset／簽章遭替換或憑證外洩。
- parser 產生錯誤完整檔、silent corruption 或資料外洩。
- 立即停發、撤回下載、通知使用者、保存證據、修補與重新發布流程。

### 正式版 Go／No-Go

只有以下全部成立才可標 stable：

- Critical／High release blockers 全部關閉，沒有以文件豁免安全缺陷。
- source→output 實體隔離測試通過。
- 正式 helper 身分驗證與 least privilege 對抗測試通過。
- 公開宣稱對應的檔案系統 corpus gate 通過。
- macOS／Windows 對應 artifact 的安裝、簽署、notarization／Authenticode gate 通過。
- 網站、README、UI、capability matrix、release notes 無版本或能力矛盾。
- 有 rollback、security contact、incident response 與已知限制。

## 15. 建議 issue／PR 切分

每個 PR 只處理一個可驗收邊界，建議順序：

1. `audit: record current public artifact and claim inventory`
2. `docs: correct preview, platform and capability claims`
3. `docs: add security policy and capability matrix`
4. `repo: establish canonical source according to owner decision`
5. `build: pin toolchain and unify version metadata`
6. `security: compile out legacy privileged prototypes`
7. `security: introduce authenticated minimal macOS helper`
8. `storage: resolve physical source and output identity`
9. `writer: atomic partial outputs and versioned manifest`
10. `imaging: require resumable image-first workflow`
11. `test: add deterministic filesystem and corruption corpus`
12. `test: add parser fuzzing and resource limits`
13. `apfs: satisfy strict real OMAP/VFS gate`
14. `desktop: enforce CSP and least-privilege commands`
15. `desktop: replace synthetic progress with real counters`
16. `desktop: cancellation, recovery and large-tree bounds`
17. `platform: validate macOS matrix and helper lifecycle`
18. `platform: validate Windows raw-device and installer flow`
19. `release: signing, notarization, SBOM and provenance`
20. `web: derive downloads and capabilities from release manifest`
21. `pilot: execute controlled recovery validation`

每個 PR description 必須包含：威脅／失敗模式、變更範圍、測試證據、未處理限制、rollback 方法，以及是否改變公開 capability matrix。

## 16. CLI agent 執行協議

### 每個任務開始前

1. 讀取 repository 的 `AGENTS.md` 與最新 checkpoint／plan。
2. `git status --short --branch`，保留使用者既有變更。
3. 確認目前 commit、工具鏈與平台，不引用舊 log 代替實跑。
4. 對會接觸裝置的測試列出 fixture 路徑；路徑若為 `/dev/*` 立即停止。
5. 先寫出預期 failure mode 與驗收命令，再改程式。

### 實作規則

- 優先新增小型、可審查的單元與 integration tests。
- 不使用真實個資、學生／學校資料、客戶檔案或唯一磁碟副本。
- 不啟動 root helper、不用 `sudo`、不執行 force-unmount。
- 不把 failed test 改成 skip／ignore 來得到綠燈。
- 不以 mock 單獨證明 raw-device safety；平台整合 gate 必須另跑。
- 不在同一 PR 同時大改 privilege boundary、parser 與 UI。
- 若發現實際能力低於文件，先降低宣稱並建立 blocker，不自行美化結果。

### 每個任務完成時

1. 執行與風險相稱的完整測試；列出沒能執行的項目與原因。
2. 更新 capability matrix、threat model、schema 或 release inventory（若受到影響）。
3. 留下精確 commit／artifact hash 與重現步驟。
4. 確認沒有 `.partial`、測試 image、log、secret、簽章資料被誤提交。
5. `git diff --check`、`git status`，清楚列出修改檔案。

## 17. 第一個可交付 sprint

第一個 sprint 僅做 P0 與 P1 的決策前準備，不實作新功能、不啟動 helper、不匯入未核准原始碼。

### Sprint 1 任務

1. 新增 current release inventory 與可重跑的唯讀 audit commands。
2. 新增逐項 capability matrix，將 APFS、Windows、helper bundling 標成實際證據所支持的狀態。
3. 修正 README 與網站的 Windows Releases、開源、APFS、100%、唯讀與 helper 宣稱。
4. 標記／暫停未簽署 DMG 的正式下載定位，公開 checksum 與限制。
5. 新增 `SECURITY.md` 與安全回報方式。
6. 對本機 source snapshot 產生 inventory、secret／PII／license／artifact 掃描報告，但不修改它。
7. 建立 open-source／proprietary 決策記錄，等待 owner 選擇。
8. 記錄目前環境缺少 Cargo；在 canonical repo 確立後才加入 pinned bootstrap 與 CI。

### Sprint 1 完成定義

- 公開頁面不再引導使用者把 preview 當正式資料救援工具。
- 所有矛盾宣稱有明確修正或 blocker。
- source import 的內容、排除項目、授權與風險已可供 owner 審核。
- 下一個 agent 可以在不猜測產品授權與 repository 邊界的情況下開始 P1。

## 18. 給 CLI agent 的首輪指令範本

```text
在 /Users/huangjianzhe/Git/polik-recovery 執行 POLIK_RECOVERY_STAGED_IMPROVEMENT_PLAN.md 的 Sprint 1。

限制：
- 不執行或安裝目前 DMG/App。
- 不啟動任何 helper、不用 sudo、不存取 /dev/disk* 或 /dev/rdisk*。
- 不修改 /Users/huangjianzhe/Desktop/polik-recovery-v2.7-delivery-package-patch；只做唯讀 inventory 與掃描。
- 在 owner 尚未決定真正開源或維持專有前，不把 source snapshot 匯入公開 Git。
- 保留使用者現有變更；每個 public claim 都必須連到目前可重跑的 evidence。

先提交 current-release-inventory、capability-matrix、SECURITY.md 與 README/index.html 的誠實宣稱修正；完成後執行 link、HTML、文字一致性檢查，回報無法執行的測試與後續 blocker。
```

---

這份計畫本身不是安全認證。正式發布的可信度必須來自可重建原始碼、最小特權架構、實體裝置隔離、可重現 corpus、平台簽署與每次 release 的具體 evidence。
