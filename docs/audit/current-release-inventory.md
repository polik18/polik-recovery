# Current Public Release Inventory

> Audit date: 2026-10-02  
> Repository baseline: `42458a4c9782b3920426a7c98a2447e9dfdc0c47`  
> Status: `PUBLIC RELEASE BLOCKED`

## Repository contents at baseline

公開倉庫只有網站、README 與一個 DMG，沒有產品原始碼、LICENSE、CI、測試或 release build scripts。因此目前不能從公開 commit 重建或獨立驗證 App。

## Artifact identity

| Field | Observed value |
|---|---|
| File | `polik-recovery-desktop_0.1.0_aarch64.dmg` |
| Size | 3,647,430 bytes |
| SHA-256 | `897967c1a83071a16e6086ff58ca111c22091430b6c904cd479206540169af30` |
| Container | UDZO read-only compressed HFS+ |
| App architecture | arm64 only |
| Bundle identifier | `com.polik.recovery` |
| Bundle version | `0.1.0` |
| Info.plist minimum OS | 10.13 |
| Mach-O minimum OS | 11.0 |
| Public GitHub release | None observed |

## Bundle observations

DMG 只包含 App、`/Applications` symlink 與 volume icon。App bundle 只有 `Info.plist`、icon 與主程式：

- 沒有 `polik-recovery-helper`。
- 沒有 LaunchDaemon、XPC service、provisioning profile 或獨立 entitlements file。
- 主程式可見 Tauri／Rust 與多個復原模組的 symbols／strings，但 presence 不能證明功能正確。

## Signature and Gatekeeper

- 主程式只有 ad-hoc／linker signature，沒有 Developer ID TeamIdentifier。
- `codesign --verify --deep --strict` 失敗：bundle 沒有 resources，但 signature 宣稱應有 sealed resources。
- `spctl` 拒絕 App。
- DMG 沒有可用簽章。
- App 與 DMG 都沒有 stapled notarization ticket。

因此這個 artifact 只能標為歷史研究預覽，不能標為正式安全發布。

## Read-only reproduction commands

以下命令只用於 artifact 稽核。不要啟動 App，也不要讓它接觸真實裝置。

```sh
git rev-parse HEAD
stat -f '%z bytes' polik-recovery-desktop_0.1.0_aarch64.dmg
shasum -a 256 polik-recovery-desktop_0.1.0_aarch64.dmg
hdiutil imageinfo polik-recovery-desktop_0.1.0_aarch64.dmg
```

若需要檢查 bundle，應在隔離測試環境以 read-only、no-auto-open 方式掛載至 `mktemp -d` 建立的專用目錄，再執行：

```sh
file '<mount>/polik-recovery-desktop.app/Contents/MacOS/app'
codesign -dvvv '<mount>/polik-recovery-desktop.app'
codesign --verify --deep --strict --verbose=2 '<mount>/polik-recovery-desktop.app'
spctl --assess --type execute --verbose=4 '<mount>/polik-recovery-desktop.app'
xcrun stapler validate '<mount>/polik-recovery-desktop.app'
xcrun stapler validate polik-recovery-desktop_0.1.0_aarch64.dmg
```

掛載後必須用 `hdiutil detach` 卸載。禁止用 `open` 啟動 App；禁止使用 `sudo` 或指定 `/dev/disk*` 作為測試來源。

## Claim mismatches corrected by Sprint 1

| Previous public claim | Evidence | Current public status |
|---|---|---|
| Windows installer is available in Releases | Releases page has no release | `planned` |
| Intel macOS build is available in Releases | No release or artifact | `planned` |
| DMG bundles `polik-recovery-helper` | No helper in bundle | `unsupported in 0.1.0` |
| Complete APFS VFS/extraction | Strict real APFS evidence is not available for artifact | `experimental` |
| Fully open source / MIT | No public product source or LICENSE | `undecided` |
| Source disk can never be modified | Architecture and artifact do not provide sufficient release evidence | `unverified; do not rely on this preview` |

## Release-blocking follow-up

The next release needs a canonical source repository, unified version, authenticated least-privilege helper, physical-device output isolation, version-matched corpus evidence, Developer ID signing, notarization and an actual immutable GitHub Release.
