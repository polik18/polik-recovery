# Polik Recovery

Polik Recovery 是一套以 Rust 開發的唯讀資料救援工具，提供 macOS 桌面版安裝檔與中文教學頁面。

## 下載

- macOS Apple Silicon: http://polik18.github.io/polik-recovery/polik-recovery-desktop_0.1.0_aarch64.dmg
- 使用教學: `index.html`

Windows 版安裝檔尚待在 Windows 電腦本地編譯後加入此發佈目錄。

## macOS 使用方式

1. 下載並開啟 http://polik18.github.io/polik-recovery/polik-recovery-desktop_0.1.0_aarch64.dmg。
2. 將 Polik Recovery 拖曳到 Applications。
3. 啟動應用程式後，選擇要掃描的來源磁碟或映像檔。
4. 請將救援輸出位置選在不同磁碟或外接儲存裝置，避免覆寫來源資料。

## 安全原則

- 掃描與讀取來源磁碟時採唯讀模式。
- 救援輸出會產生 SHA-256 稽核資訊。
- macOS Raw Disk 存取需要系統權限；若直接讀取被拒，應用程式可透過本機 helper 進行受限唯讀讀取。

## 發佈內容

此目錄只放公開發佈用檔案，不包含 Rust 原始碼、內部文件或開發設定。
