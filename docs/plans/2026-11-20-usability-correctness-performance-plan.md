# Polik Recovery 易用性／正確性／效能改善計畫（AI Agent 交接版）

> 建立：2026-11-20（Claude Code，Opus 5.5）
> 基準：`main@2283db2`
> 進度：2026-11-21 全部工作包（含新增 UX1）完成並合併至 `main`，Product CI 首次全綠（run 44850879036）。
> 狀態機器檔：[`2026-11-20-improvement-checkpoint.json`](2026-11-20-improvement-checkpoint.json)（**任務 owner／進度以它為準**，本文件只寫背景、規格與驗收）
> 與既有計畫的關係：[`POLIK_RECOVERY_STAGED_IMPROVEMENT_PLAN.md`](../../POLIK_RECOVERY_STAGED_IMPROVEMENT_PLAN.md) 處理「安全邊界與發布」（P0–P9）；本計畫處理使用者回報的「不好用、錯誤多、效能不好」，兩者不衝突。本計畫的 WP 不得削弱任何 fail-closed 安全門禁。

---

## 0. 接手 Agent 先讀（5 分鐘）

1. 讀 [`product/AGENTS.md`](../../product/AGENTS.md) 的安全規則：**禁止碰 `/dev/disk*`、`/dev/rdisk*`、`\\.\PhysicalDrive*`，禁止 sudo、禁止啟動 helper**。所有測試只用 `product/fixtures/` 或新產生的拋棄式映像。
2. 開 checkpoint JSON，找 `status: "todo"` 且 `depends_on` 全為 `done` 的 WP；把 `owner`、`started_at` 填上並 commit（或在共同大腦記錄）後才開工。
3. 工具鏈：本機 `cargo` 不在 PATH，要先：
   ```sh
   export PATH="$HOME/.rustup/toolchains/1.85.1-aarch64-apple-darwin/bin:$PATH"
   cd product
   ```
4. 一個 WP 一個分支：`wp/<ID>-<slug>`（例 `wp/C1-jpeg-false-positive`）。完成時在 checkpoint 填 `evidence`（指令＋結果摘要），不要只寫「已完成」。
5. 歷史 log、舊報告**不算證據**；每次都要重跑指令。

---

## 1. 研究結論摘要（2026-11-20 實測）

| # | 類別 | 發現 | 證據 |
|---|---|---|---|
| F1 | 錯誤／安全 | `cargo test --workspace` **失敗 1 項**：`safe-writer::tests::rejects_symlinked_output_parent`，寫入穿越 symlink 目錄成功（應拒絕） | 93 passed / 1 failed；`crates/safe-writer/src/lib.rs:2246`。推論根因：output 路徑先被 `canonicalize`（約 `lib.rs:463-500`、`1214`），之後 `reject_symlink_components`（`lib.rs:1524`）檢查的已是解析後路徑，永遠看不到 symlink |
| F2 | 錯誤 | **Product CI 自 2026-10-06 起從未綠過**：`EmbarkStudios/cargo-deny-action` 是 container action，`macos-14` runner 不支援 → 後面的 test 步驟根本沒跑，F1 因此沒被發現 | `gh run view 37535238407`：`Container action is only supported on Linux` |
| F3 | 錯誤（結果品質） | **純隨機 256 MiB 資料被 carve 出 11 個「JPEG」，信心 0.94**，大小 1–8 MB，且彼此區段重疊 | scratch benchmark：`rand256.img → found 11`，全為 JPEG conf=0.94 |
| F4 | 效能 | carver 單執行緒逐 byte 比對 6 種特徵：page cache 下僅 **240–294 MiB/s**（NVMe 可供 2–3 GB/s，CPU 成瓶頸；1 TB 映像約 1 小時） | zero256 294 MiB/s、rand256 240 MiB/s |
| F5 | 效能 | 每遇一個特徵就 `read_window` 一次配置並讀取上限視窗（PDF 80 MB、ZIP 120 MB、MP4 250 MB）。64 MiB 映像內僅 64 個 `PK\x03\x04` → **降到 12.8 MiB/s（慢 20 倍）**、RSS 134 MB。真實磁碟中每個 docx/jar/apk/zip 內部的每個檔頭都是 `PK\x03\x04`，會造成災難性變慢 | `pk64.img 4.99s`；`crates/file-carver/src/lib.rs:263-280`、`:126` |
| F6 | 效能／正確 | 命中後 `i += len` 只在當前 256 KB chunk 內有效；下一 chunk 從固定位移重掃，大檔內部會被重複偵測（再讀一次大視窗），最後只靠 SHA-256 去重，**重疊區段不會被去除** | `file-carver/src/lib.rs:190`、`:194-201` |
| F7 | 正確 | `FileBlockSource::read_at` 只呼叫一次 `file.read()`，未處理 short read；且以 `Mutex<File>`＋seek 序列化所有讀取，無法並行 | `crates/core/src/block.rs:288-296` |
| F8 | 正確 | 任何一次 I/O error 會讓整個 carve 以 `?` 中止（壞軌媒體映像最常見情況） | `file-carver/src/lib.rs:49` |
| F9 | 正確 | NTFS 掃描以 `mft_offset + index*record_size` 讀取，**假設 $MFT 連續**；長期使用的磁碟 $MFT 常碎片化 → 後段紀錄遺漏或讀到垃圾 | `src-tauri/src/lib.rs:1969-1975` |
| F10 | 易用性 | 桌面 App 主流程是「選磁碟 → 掃描」，磁碟清單列出 `/dev/diskN` 分割區，但 raw device 已 fail-closed（helper 已移除），**使用者照預設流程操作必然失敗** | `list_disks_impl`（`lib.rs:1490`）、`open_regular_file_source` 拒絕 `/dev/` |
| F11 | 易用性 | 只有一顆內建磁碟的 Mac，輸出到任何資料夾都被拒（`source and output share physical storage: disk0`）——安全上正確，但 UI 沒有在掃描前就檢查並說明「需外接磁碟」 | release CLI 實測 |
| F12 | 易用性 | 取消是全域 `AtomicBool`，每個新作業都 `reset`（縮圖、第二個作業會把取消旗標清掉）；carver 內部完全不檢查取消 → 掃描中按取消無效 | `lib.rs:12-33`、`carve_source` 無 cancel 參數 |
| F13 | 易用性 | 沒有 MBR/GPT 解析；整碟映像（`.img`/`.dmg` raw）必須手動算 partition offset；縮圖 `offset: 0` 寫死，分割區映像的縮圖一定錯 | `grep -ri gpt crates` 僅 device-windows；`index.html:2282-2287` |
| F14 | 效能 | Tauri `async fn` 內直接跑同步重工作（未 `spawn_blocking`）；掃描結果以一整段 TSV 文字回傳；前端每次 `render()` 以 `innerHTML` 重建全部列（無虛擬化），縮圖每次 `state.entries.find()`（O(n)）且後端每張縮圖重開來源、重讀 boot sector | `lib.rs:3909-3943`、`format_vfs_text`、`index.html:2211-2235`、`2280` |
| F15 | 可維護性 | `src-tauri/src/lib.rs` 4,469 行、前端單檔 2,603 行；`run_privileged_*`、`is_perm_denied`、`/dev/` 分支等已無作用的殘留碼；`withGlobalTauri: true` 仍開；378 處 `unwrap/expect`、48 處 `unsafe` | `wc -l`、`grep` |
| F16 | 文件 | `docs/implementation-status.md` 停在 2026-10-06，仍寫「uncommitted working tree」、「91 tests」，與現況不符 | 檔案內容 |
| F17 | 網站 | 網站本身正常（200、42 KB，CSP 與連結驗證通過）；使用者給的 `polik18.github.io/polik-recover` 少一個 y 為 404。載入 3 組 Google Fonts，可縮減 | `curl`、`scripts/validate-public-files.py` |

通過的部分：`cargo fmt` OK、`cargo clippy -D warnings` OK、公開內容／前端 CSP／安全模式檢查腳本全部 OK、四個 synthetic quality gate 的既有設計合理。

---

## 2. 工作包（WP）

格式：**ID｜標題｜優先級｜依賴**，接著是「修改範圍」「規格」「驗收（必須全部可重跑）」。
優先級：P0 = 立刻；P1 = 本輪核心；P2 = 之後。

### 階段 S：止血與基線（先做，讓後續每一步都有可信的回饋）

#### S1｜修正 symlink 輸出防護回歸｜P0｜無
- 範圍：`product/crates/safe-writer/src/lib.rs`
- 規格：在任何 `canonicalize(output…)` **之前**，對使用者提供的原始 output 路徑執行 `reject_symlink_components`；或改為逐段 `openat(O_NOFOLLOW|O_DIRECTORY)` 走訪。Windows 端 `reject_windows_reparse_components` 同樣檢查原始路徑。不得以修改測試期望來讓測試通過。
- 驗收：
  - `cargo test --locked -p data_recovery_safe_writer` 全綠（含 `rejects_symlinked_output_parent`、`directory_handle_stays_bound_after_path_replacement`）。
  - 新增測試：中間層 symlink（`a/link/b/out.txt`）與最後一層 symlink 都被拒。

#### S2｜讓 Product CI 真的會跑而且是綠的｜P0｜S1
- 範圍：`.github/workflows/product-ci.yml`
- 規格：把 cargo-deny 移到獨立 `ubuntu-latest` job（container action 只支援 Linux），或在 macOS job 改用 `cargo install --locked cargo-deny@<pinned>` 直接執行；保持 action 以 commit SHA 釘選。macOS job 必須跑到 fmt/check/test/clippy/quality gates。
- 驗收：push 到分支後 `gh run list --workflow product-ci.yml -L1` 為 `success`；log 中可見 `cargo test` 實際執行及 test result 行數。

#### S3｜建立效能與誤判基準（bench + 負樣本 corpus）｜P0｜無
- 範圍：新增 `product/tools/bench/`（產生器＋執行器）或 `crates/file-carver/benches/`；fixture 不入 git（太大），以固定 seed 產生於 `product/target/bench/`。
- 規格：至少四種映像：全 0、固定 seed 隨機、每 1 MiB 一個 `PK\x03\x04`、每 1 MiB 一個 `ftyp`；各 256 MiB／64 MiB。輸出 JSON：吞吐量 MiB/s、峰值 RSS、carved 數、各類型數。
- 2026-11-20 基線（Apple Silicon、page cache、release）：

  | 映像 | MiB/s | RSS | carved |
  |---|---|---|---|
  | zero 256 MiB | 294 | 2.8 MB | 0 |
  | random 256 MiB | 240 | 63 MB | **11（全誤判）** |
  | PK-dense 64 MiB | **12.8** | 134 MB | 0 |
  | ftyp-dense 64 MiB | 135 | 134 MB | 0 |

- 驗收：`python3 tools/bench/run_carver_bench.py` 可重跑並輸出上表格式；CI 加入「隨機映像 carved 必須 = 0」的硬門禁（吞吐量只記錄不設門檻，避免 runner 抖動）。

#### S4｜文件對齊現況｜P1｜S1、S2
- 範圍：`docs/implementation-status.md`、`docs/capability-matrix.md`、`docs/quality/synthetic-corpus.md`、`product/AGENTS.md`
- 規格：更新日期、基準 commit、實際測試數（目前 94）、CI 狀態；capability matrix 的 carving 一列補上 F3 誤判事實（維持 `experimental`）。
- 驗收：`python3 scripts/validate-public-files.py` 通過；文件中不再出現「uncommitted working tree」。

### 階段 C：正確性（「錯誤多」）

#### C1｜JPEG carving 誤判｜P0｜S3
- 範圍：`crates/file-carver/src/lib.rs`（`find_jpeg_length`、`valid_jpeg_bytes_strict`）
- 規格：要求合法結構才接受：SOI → 至少一個 DQT、DHT（或標準 Huffman 的 baseline 例外）、合法 SOF（寬高 > 0、元件數 1/3/4、取樣因子合法）、SOS 的元件對應 SOF；熵編碼區內 `0xFF` 後只能是 `0x00`、`RST0-7` 或 EOI；長度合理（≥ 某最小值、與寬高大致相稱）。信心不得因只看到標記就給 0.94。
- 驗收：S3 隨機映像 carved = 0；既有 `run_quality_gate.py` 不退步（仍 ≥ 6/8 且 0 誤判）；新增單元測試：隨機 bytes 前綴 `FFD8FF` 不得被接受。

#### C2｜carving 區段不重疊、跨 chunk 正確跳過｜P1｜S3
- 規格：維護 `next_scan_offset`，接受一個檔案後從其結尾繼續（跨 chunk 生效）；結果以區段去重（後找到且被前者完全覆蓋者丟棄，除非是明確的內嵌縮圖策略並在 manifest 標註）。
- 驗收：新增測試：一個 ZIP 內含 3 個 JPEG → 預設只產出 1 個 ZIP；任意兩個輸出區段不重疊（測試中斷言）。

#### C3｜BlockSource 讀取契約｜P1｜無
- 範圍：`crates/core/src/block.rs`
- 規格：改用 positional read（Unix `std::os::unix::fs::FileExt::read_at`、Windows `seek_read`），迴圈讀滿或到 EOF，移除 `Mutex`；在 trait 文件明訂「回傳 bytes_read < len 只代表 EOF」。保留 `O_NOFOLLOW`、regular-file 與身分綁定檢查不變。
- 驗收：新增 short-read 模擬測試（自訂 BlockSource）；`cargo test --workspace` 全綠；safe-writer 的 `VerifiedSourceStorage` 綁定測試不退步。

#### C4｜壞區容錯｜P1｜C3
- 規格：carver 與 parser 遇到 I/O error 時記錄 `unreadable_ranges`（offset/length/errno），以磁區為單位跳過繼續；manifest／報告呈現壞區清單。
- 驗收：測試用 BlockSource 在指定區段回傳 EIO，carve 仍完成且報告列出該區段。

#### C5｜NTFS $MFT 依 data runs 讀取｜P1｜無
- 範圍：`crates/fs-ntfs`、`src-tauri/src/lib.rs` 的 `read_ntfs_records_tolerant`、CLI 對應指令
- 規格：先讀 MFT record 0 的 `$DATA` runlist，建立 `index → 實體位移` 對照；所有讀 MFT 紀錄的路徑（掃描、單檔還原、縮圖）共用此對照。
- 驗收：fixture 產生器新增「$MFT 分成 3 段」的 NTFS 映像（固定 seed、可重現）；`run_fs_quality_gate.py` 對該映像 100% 找到並還原；`check_fixture_reproducibility.py` 仍通過。

#### C6｜MBR／GPT 自動偵測與 offset｜P1｜無
- 規格：新增 `crates/partition`（或放 core）解析 MBR 與 GPT（含 protective MBR）；CLI 新增 `partitions --file`；桌面 App 選檔後列出分割區讓使用者選，自動帶入 offset 與檔案系統類型；縮圖、還原沿用同一 offset（修 `index.html` 寫死 `offset: 0`）。
- 驗收：新 fixture：GPT 整碟映像含 FAT32 + exFAT 兩分割區，CLI 與 App 皆能不手動輸入 offset 完成掃描與還原；單元測試涵蓋損壞 GPT header（CRC 錯）時退回 backup header 或明確報錯。

### 階段 P：效能

#### P1｜carver 特徵前置過濾＋小視窗驗證｜P0｜S3、C1
- 規格：
  1. 用 `memchr::memmem`／`aho-corasick`（兩者已在 Cargo.lock，MIT/Unlicense，符合 free-only 政策，需跑 `cargo deny`）一次找出所有候選位置，取代逐 byte 6 次比較。
  2. 候選先用 ≤ 64 KB 的小讀取做檔頭結構驗證，失敗即丟棄；通過才進入長度判定。
  3. 長度判定改為串流／分段讀取（逐 box、逐 chunk、逐 marker），**不再一次配置 max_size 視窗**；全程重用 buffer。
  4. 長度判定與 SHA-256 共用同一次讀取，不要讀兩次（現況 `find_*_length` 讀一次、`read_file_data` 再讀一次）。
- 目標（S3 bench、page cache）：random ≥ 1,000 MiB/s；PK-dense ≥ 200 MiB/s；所有映像峰值 RSS < 64 MB。
- 驗收：S3 bench JSON 達標；quality gate 不退步。

#### P2｜carver 串流輸出與並行｜P1｜P1、C3
- 規格：`carve_source` 改為 callback／iterator（`FnMut(CarvedFile) -> ControlFlow`），可邊找邊寫、邊回報進度、可取消；以 chunk 分段 + 邊界重疊用 `rayon`（若新增依賴需過 cargo deny）或 `std::thread::scope` 並行掃描，最後依 offset 排序並套用 C2 的區段規則。
- 驗收：大於 RAM 的映像（例如以 sparse file 產生 32 GiB）記憶體維持平穩；進度事件至少每秒 1 次。

#### P3｜NTFS 批次讀取｜P2｜C3、C5
- 規格：MFT 以 1–4 MiB 為單位批次讀入再切分紀錄，取代每筆 1 KB 一次 syscall。
- 驗收：S3 擴充一個 100 萬筆 MFT 的合成映像，掃描時間記錄於 bench JSON，相較改前至少快 3 倍。

#### P4｜桌面後端不阻塞、分頁回傳｜P1｜無
- 範圍：`src-tauri/src/lib.rs`
- 規格：所有重工作改 `tauri::async_runtime::spawn_blocking`；掃描結果改結構化 JSON（serde struct），並支援分頁／串流事件；縮圖指令快取已開啟的來源與已解析的 boot sector／MFT 對照（以 source path + offset 為 key，LRU）。
- 驗收：掃描進行中 UI 仍可操作（取消按鈕 < 200 ms 回應）；10 萬筆結果的首次可見時間 < 1 s（在 dev 機以合成資料量測並記錄）。

#### P5｜前端清單虛擬化｜P1｜P4
- 範圍：`apps/desktop-prototype/index.html`（或拆檔後的 JS）
- 規格：清單／格狀檢視只渲染可見列（固定列高虛擬捲動）；`id → entry` 使用 `Map`；事件用委派而非每列 addEventListener；縮圖請求併發上限（例如 4）。
- 驗收：10 萬筆合成資料下捲動不卡（Performance 面板無 > 50 ms long task 連續出現）；`python3 scripts/check-product-frontend.py` 通過（CSP hash 若變更要同步更新）。

### 階段 U：易用性

#### U1｜主流程改為「映像檔優先」｜P0｜無
- 規格：首頁主按鈕改為「選擇映像檔」；磁碟清單改名為「建立映像（建議）」，只提供 ddrescue／`hdiutil` 指引與 dry-run 計畫，不再提供直接掃描 `/dev/disk*` 的入口；`diskutil list` 改用 `-plist` 解析（取代目前脆弱的純文字解析）。
- 驗收：前端不存在可對 `/dev/` 路徑觸發 `run_recovery_scan` 的 UI 路徑（新增前端靜態檢查或 Rust 測試）；首次開啟流程截圖附在 PR。

#### U2｜輸出位置事前檢查｜P0｜無
- 規格：新增 Tauri 指令 `check_output_isolation(source, output_dir)`，在使用者選完來源與輸出資料夾時立即呼叫 safe-writer 既有的 physical-store 解析；不合格就在畫面上顯示「輸出必須在另一顆實體磁碟（例如外接 USB 硬碟）」與偵測到的 store，並停用「開始」按鈕。
- 驗收：單碟 Mac 上選內建資料夾 → 開始鈕停用且訊息正確；選外接磁碟 → 可開始。不得新增任何繞過開關。

#### U3｜可靠的取消機制｜P1｜P2、P4
- 規格：以 job id + `Arc<AtomicBool>`（或 `CancellationToken`）取代全域旗標；cancel token 傳入 carver、各 parser 迴圈與批次還原；同時只允許一個重作業，縮圖等輕作業不得重設取消狀態。
- 驗收：測試：carve 64 MiB 映像中途取消，1 秒內回傳 `Cancelled` 且無殘留 `.partial`（或依 safe-writer 規則保留並標註）。

#### U4｜錯誤訊息結構化、移除殘留碼｜P1｜U1
- 規格：後端錯誤改為 `{ code, message_zh, detail }`（enum），前端依 code 顯示對應說明與下一步；以 App 內對話框取代 `alert/confirm`；刪除 `run_privileged_*`、`shutdown_privileged_server_for_cancel`、`is_perm_denied` 與 `/dev/` 分支；評估關閉 `withGlobalTauri`（改 import `@tauri-apps/api` 或受控 bridge）。
- 驗收：`grep -n "run_privileged_\|alert(\|confirm(" src-tauri apps` 無結果；`check-product-security.py` 通過。

#### U5｜拆分巨型檔案｜P2｜P4、U4
- 規格：`src-tauri/src/lib.rs` 拆為 `session.rs`、`scan.rs`、`extract.rs`、`disks.rs`、`diagnostics.rs`、`thumbnail.rs`、`errors.rs`；前端拆為 `app.js`／`views/*.js`／`i18n.js`／`style.css`（同時移除 `style-src 'unsafe-inline'`）。純搬移的 PR 與行為變更 PR 分開。
- 驗收：搬移 PR 的 `cargo test` 結果與搬移前相同數量；CSP 不再含 `unsafe-inline`。

### 階段 W：網站（低優先）

#### W1｜網站小修｜P2｜S4
- 規格：字型縮減為 1–2 組或自架子集；同步更新能力敘述（引用 S4 後的 capability matrix）；考慮在 `polik18.github.io` 主站加 `polik-recover/` → `polik-recovery/` 的轉址頁（需另一 repo，先問擁有者）。
- 驗收：`./scripts/check-public-content.sh`、`python3 scripts/validate-public-files.py` 通過；Lighthouse 行動版效能分數記錄於 PR。

---

## 3. 建議執行順序與並行

```
S1 ─┬─> S2 ─> S4
    │
S3 ─┼─> C1 ─> P1 ─> P2 ─> U3
    └─> C2
C3 ─> C4,  C3+C5 ─> P3
C6（獨立）   U1、U2、P4（獨立，可平行）─> P5、U4 ─> U5 ─> W1
```

- 可同時給不同 agent 的組合（檔案不衝突）：〔S1/S2〕、〔S3→C1→P1〕（file-carver）、〔C5〕（fs-ntfs）、〔C6〕（新 crate）、〔U1/U2〕（前端＋tauri 新指令）。
- `src-tauri/src/lib.rs` 是衝突熱點：P4、U3、U4、U5 同一時間只能一個 owner。U5（純搬移）排在最後，避免大量 rebase。

## 4. 每個 WP 的「完成」定義

1. 該 WP 驗收項全部有可重跑指令與實際輸出摘要（寫入 checkpoint `evidence`）。
2. 最低門禁全綠（在 `product/`）：
   ```sh
   cargo fmt --all -- --check
   cargo check --workspace --all-targets --locked
   cargo test --workspace --locked --no-fail-fast
   cargo clippy --workspace --all-targets --locked -- -D warnings
   python3 tools/quality/check_fixture_reproducibility.py
   cargo build --locked -p polik-recovery-cli
   python3 tools/quality/run_quality_gate.py
   python3 tools/quality/run_fs_quality_gate.py
   python3 tools/quality/run_fragmented_quality_gate.py
   python3 tools/quality/run_apfs_quality_gate.py
   make release-safety-gate
   ```
   以及 repo 根目錄 `python3 scripts/check-product-security.py`、`python3 scripts/check-product-frontend.py`。
3. 能力宣稱只能因版本對應的 gate 通過而升級；本計畫不會讓任何項目升到 `verified`。
4. 不 push、不合併到 `main`，除非擁有者明確授權。

## 5. 交接協定

- **接手**：讀本文件 §0 → 讀 checkpoint → 選 WP → 填 `owner`／`started_at`／`branch`。若某 WP 已有 owner 且 `updated_at` 在 24 小時內，不得接手；超過 24 小時無更新，在 `notes` 記錄後才可接管。
- **中途交接**：在 checkpoint 該 WP 填 `status: "in_progress"`、`next_step`（一句話、可直接執行）、`blockers`；分支要 push 到本機或遠端可取得的位置。
- **完成**：`status: "done"`、`evidence`、`finished_at`；若發現新問題，新增 WP（ID 依階段字母遞增）而不是塞進既有 WP。
- **共同大腦**：重大里程碑（S 階段完成、C1+P1 完成、U1+U2 完成）在 `_AI_Brain_History.md` 留一行摘要並指回本文件。

## 6. 研究時使用的指令（可重現）

```sh
# 工具鏈
export PATH="$HOME/.rustup/toolchains/1.85.1-aarch64-apple-darwin/bin:$PATH"
cd product
cargo test --workspace --locked --no-fail-fast   # 93 passed / 1 failed（S1）
cargo clippy --workspace --all-targets --locked -- -D warnings   # OK

# CI 狀態
gh run list -L 10
gh run view 37535238407 --log-failed   # cargo-deny container action on macOS

# carver bench：在暫存目錄建立獨立 cargo 專案，path 依賴
# data_recovery_file_carver / data_recovery_core，複製 product/Cargo.lock 以符合 rustc 1.85.1，
# 對 zero/random/PK-dense/ftyp-dense 映像呼叫 carve_source(CarverConfig::default())
# 並以 /usr/bin/time -l 量測峰值 RSS。S3 會把它正式化進 repo。
```
