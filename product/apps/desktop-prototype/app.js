// ── I18N ──
const I18N = {
  en: {
    appTitle:'Polik Recovery', brandLogo:'P', brandName:'Polik Recovery', brandSubtitle:'Recover important files safely',
    navLabel:'Scan type', mobileModeLabel:'Scan type', sampleLabel:'Sample data', loadSample:'↻ Load selected sample', advancedMode:'⚙️ Advanced mode', sidebarNote:'Read-only disk access. Source data is never modified.',
    workbench:'Recovery Workbench', runtimePreview:'Preview mode', desktopMode:'Desktop App Mode', previewModeMessage:'Preview mode: sample screens are available. Use the desktop app for real scans.', reanalyze:'⟳ Reanalyze', copyCommand:'⇩ Copy command', chooseImage:'📁 Choose Image', startScan:'▶ Start Scan', stop:'⏹ Stop', restoreSelected:'💾 Restore Selected Files',
    steps:{select:'Select disk', scanning:'Scanning', restore:'Select and restore'},
    scanTitle:'Scanning, please wait...', scanSub:'The system is safely reading disk contents.<br>This process will not modify your data.', scanReading:'Reading...', stopCurrent:'⏹ Stop current operation', scanTip:'Large disks can take longer to scan. Stop requests the current read flow to halt and closes the privileged helper in physical disk mode.',
    safe:{readTitle:'Read-only scan', readSub:'No writes to the source disk', saveTitle:'Save elsewhere', saveSub:'Avoid overwriting original data', pickTitle:'Pick files', pickSub:'Select files after scanning', authTitle:'Authorize once', authSub:'No repeated prompts in one run', imageTitle:'Image first', imageSub:'Scan .img instead of stressing the source'},
    recommendedFlow:'Recommended flow', diskPick:'Create an image (recommended) <span class="muted-inline">- direct physical-disk scanning is disabled</span>', detecting:'- Detecting... -', sourcePath:'Source image path', ntfsDepth:'NTFS scan depth',
    ntfsOptions:['Standard: estimate full MFT automatically (recommended)','Quick: scan first 10,000 records','Conservative: scan first 50,000 records'], ntfsHelp:'Use Standard when too few files are found. It reads $MFT size first instead of scanning only a fixed short range.', apfsScope:'APFS scan scope', apfsScopeOptions:['Existing files only (fast)','Also look for deleted files (reads the whole container, slower)'], apfsScopeHelp:'Deleted APFS files come from leftover metadata. Their content cannot be verified and goes to suspect/.',
    outputPath:'Recovery output folder <span class="danger-inline">do not choose the source disk</span>', choose:'📁 Choose', outputHelp:'Save to Desktop, an external disk, or another safe folder to avoid overwriting the disk being recovered.',
    riskTitle:'🟢 Pre-scan risk check', riskCheckLabel:'Pre-scan risk check', riskReady:'Ready. No obvious risk detected.', riskLow:'Low risk', riskInitial:'<strong>Read-only scan.</strong> Source, format, and output location are checked before scanning to reduce overwrite risk.', riskRecommendation:'Recommendation: save recovered files to another disk',
    imageWorkflowTitle:'🧊 v1.9 Image first + ddrescue map', imageWorkflowBadge:'Recommended flow', imageWorkflowSummary:'For physical disks, create a read-only image first, review the ddrescue map for bad or incomplete areas, then scan the .img.', imageWorkflowCommand:'Choose a source to generate imaging, map visualization, verification, and retry commands.', copyImageWorkflowCommand:'Copy image / map / retry command', regenerate:'Regenerate',
    sessionWorkflowTitle:'🗂️ v2.7 Recovery Case Session + Customer Package', sessionWorkflowBadge:'Recommended', sessionWorkflowSummary:'Create one case folder per recovery and keep image, map, scan, recovery, quality, logs, dashboard, customer summary, and engineer handoff together.', activeSessionStatus:'No Session is active. Use Create Session to point recovery output to a case folder automatically.', sessionWorkflowCommand:'Choose a source to generate session creation, indexing, and check commands.',
    sessionButtons:['Create Session','Open Session','Use Session Output','Update Index','Check Session','Open Index','Import Reports / Artifacts','Generate Handoff Report','Open Customer Report','Open Engineer Report','Create Customer ZIP','Open Delivery Folder','Copy Session Command'],
    diagTitle:'🧰 Runtime Environment Check', notChecked:'Not checked', checkAgain:'Check again', startSafeScan:'▶ Start Safe Scan', previewSample:'👀 Preview Sample', emptyHint:'🛡️ Read-only mode: this app will never write to or modify your source disk',
    qualityNote:'✅ By default, only complete or medium-confidence items are auto-selected. Low-confidence fragments are not auto-selected.', foundFiles:'Found files', pathFilterLabel:'Filter results by path', pathFilterPlaceholder:'🔍 Filter by path (e.g. /Users/me/Desktop)', clearFilter:'✕ Clear', quickRecover:'⚡ Recover now', quickRecoverConfirm:'Recover this file now?\n\nFile: {name}\nSize: {size}\nOutput: {path}\n\nClick OK to start.', quickRecoverUnknownSize:'Unknown', filters:['All','🖼 Images','📄 Documents','🎬 Videos','🎵 Audio','🗑️ Deleted','✅ Complete','🟡 Repairable','🧩 Partial','⚠️ Fragments'], selectUsable:'Select usable', clear:'Clear', selectedPrefix:'Selected', itemCount:'items', fileCount:'files', selectedApprox:'Selected {count} files - about {size}', bucketCounts:'Complete {complete} - Repairable {possible} - Partial {partial} - Fragments {fragments} | complete files go to recovered/, others are quarantined to suspect/', emptySelectionSub:'Partial files and fragments are not selected by default. Select them manually only when needed.', selectionMain:'Select files to recover first', selectionSub:'Use Shift for range selection. The destination is shown before restore.', details:'File details', notSelected:'Not selected',
    status:{deleted:'🗑️ Deleted', normal:'Normal'}, qualityLabels:{high:'Complete', medium:'Suspect', low:'Fragment'}, bucketTitles:{complete:'Complete', possible:'Repairable', partial:'Partial recovery', fragments:'Low-confidence fragments'}, bucketHints:{complete:'Selected by default, output to recovered/', possible:'Selected by default, may be quarantined on output', partial:'Manual selection, output to suspect/partial', fragments:'Manual selection, usually for expert review only'}, qualitySummary:'Recovery quality: complete {complete}, repairable {possible}, partial {partial}, low-confidence fragments {fragments}. Low-confidence items are not auto-selected.', qualityEmpty:'No scan result loaded yet. Results are separated into recovered/ and suspect/ after scanning.',
    categoryLabels:{image:'images', doc:'documents', video:'videos', audio:'audio'}, noFiles:'No files found', noFilteredFiles:'No recoverable {type} files were found on this disk.', listHead:['File name','Status','Assessment','Route','Size'], selectAria:'Select {name}', filePath:'File path', detailsPrompt:'Click a file on the left to view details', nodeType:'Node type', allocationStatus:'Allocation status', fileSize:'File size', recoveryConfidence:'Recovery confidence', confidence:'confidence', routeStrategy:'Route strategy', qualityTag:'Quality tag', readMode:'Read mode', lostBytes:'lost {bytes} bytes', outputPreview:'Will restore to', folderNotRestorable:'Folders cannot be restored directly. Select files inside the folder.',
    advancedSettings:'Advanced Settings', advancedNotice:'🛡️ Save recovered files to another disk. Never save them back to the damaged source disk.', generatedCommand:'Generated CLI command', confirm:'Confirm', cancel:'Cancel', close:'Close', sessionPathLabel:'Session folder path', sessionPathPrompt:'Paste the existing Session folder path.', artifactPathLabel:'Artifact file or folder path', artifactPathPrompt:'Paste the file or folder to import.', artifactKindLabel:'Artifact type', artifactKindPrompt:'Use auto or choose a known artifact type.', imageWorkflowDisclosure:'🧊 Image-first workflow', sessionWorkflowDisclosure:'🗂️ Case / session tools', diagDisclosure:'🧰 Runtime diagnostics', recoveryReportTitle:'Recovery Complete Report', recoveryReportNote:'Complete files are placed in recovered/. Suspect, partial, and low-confidence results are quarantined in suspect/.', openHtmlReport:'🌐 Open HTML Report', copySummary:'📋 Copy Summary', collapseReport:'Collapse Report',
    copied:'✓ Copied', commandCopied:'Command copied.', sampleLoaded:'Sample result loaded. Use the desktop app to run real recovery.', metrics:{default:['Records scanned','Files in result','Deleted in result','Visible data size','CLI report output (JSON / Text)','VFS scan result (JSON / Text)'], carver:['Signatures detected','Carved successfully','Confidence','Scanned data','Deep scan log','Carved result list'], apfs:['Superblock status','Object manager OID','Volumes','Block size','APFS superblock metadata','Subvolume structure list']},
    modes:{
      ntfs:{nav:'💾 Disk scan and recovery', title:'Disk Scan and Recovery', badge:'Windows / NTFS', emptyTitle:'Scan a disk and recover lost files', desc:'For Windows disks, SSDs, or NTFS partitions. After scanning, select files from the file tree to restore.', advice:'Best for Windows disks or NTFS partitions. Start with a normal scan, then use Deep Scan only if needed.'},
      exfat:{nav:'📁 exFAT removable recovery', title:'exFAT Removable Recovery', badge:'Removable / exFAT', emptyTitle:'Scan an exFAT USB drive', desc:'For larger USB drives, external disks, and memory cards using exFAT. Save recovered files to the computer, not back to the same drive.', advice:'Best for exFAT USB drives, memory cards, and external storage. Use Deep Scan if the file system is severely damaged.'},
      fat32:{nav:'🧩 FAT32 card recovery', title:'FAT32 Card Recovery', badge:'Memory card / FAT32', emptyTitle:'Scan a FAT32 memory card or older USB drive', desc:'For camera SD cards, older USB drives, and FAT32 media. It reads directory entries and FAT tables before falling back to deeper recovery.', advice:'Best for FAT32 camera cards and older USB drives. Deleted files may need conservative contiguous-cluster recovery if the FAT chain was cleared.'},
      apfs:{nav:'🍎 Mac disk recovery', title:'Mac Disk Recovery', badge:'Mac / APFS', emptyTitle:'Scan a Mac disk or APFS container', desc:'For macOS APFS disks. Unencrypted extents can be extracted; FileVault, compression, clones, and snapshot-only data are marked suspect.', advice:'Best for internal or external APFS disks. If encryption or SSD/TRIM is suspected, image the disk first and set recovery expectations carefully.'},
      carver:{nav:'🔍 Deep scan', title:'Deep Scan', badge:'Deep Scan', emptyTitle:'Search scattered file fragments', desc:'Use when the disk format is unknown, the partition table is damaged, or normal scans cannot find files.', advice:'Best for damaged partitions or unknown formats. Strict format checks reduce junk files and low-confidence results are isolated.'}
    }
  }
};
I18N['zh-Hant'] = {
  appTitle:'波立克資料救援', brandLogo:'波', brandName:'波立克資料救援', brandSubtitle:'安全救援您的重要檔案',
  navLabel:'掃描類型', mobileModeLabel:'掃描類型', sampleLabel:'範例資料', loadSample:'↻ 載入所選模式範例', advancedMode:'⚙️ 進階模式', sidebarNote:'以唯讀模式存取磁碟，絕不修改您的原始資料。',
  workbench:'資料救援工作台', runtimePreview:'預覽模式', desktopMode:'桌面 App 模式', previewModeMessage:'目前是預覽模式：可看範例畫面；真實掃描請用桌面 App 開啟。', reanalyze:'⟳ 重新分析', copyCommand:'⇩ 複製指令', chooseImage:'📁 選擇映像檔', startScan:'▶ 開始掃描', stop:'⏹ 中止', restoreSelected:'💾 還原選取的檔案',
  steps:{select:'選擇磁碟', scanning:'掃描中', restore:'選擇並還原'}, recommendedFlow:'建議流程', choose:'📁 選擇', regenerate:'重新產生', checkAgain:'重新檢查', notChecked:'尚未檢查', startSafeScan:'▶ 開始安全掃描', previewSample:'👀 先看範例結果', clear:'清除', selectUsable:'全選可用檔', selectedPrefix:'已選', details:'檔案詳細資訊', notSelected:'未選取',
  scanTitle:'正在掃描，請稍候...', scanSub:'系統正在安全地讀取磁碟內容，<br>這個過程不會修改您的任何資料。', scanReading:'讀取中...', stopCurrent:'⏹ 中止目前作業', scanTip:'大型磁碟掃描可能需要較久；按中止會要求目前讀取流程停止，實體磁碟模式會同時關閉管理員權限工作站。',
  safe:{readTitle:'唯讀掃描', readSub:'不寫入來源磁碟', saveTitle:'另存救援', saveSub:'避免覆蓋原資料', pickTitle:'勾選還原', pickSub:'掃完再挑要救的檔案', authTitle:'身分檢查', authSub:'來源與輸出必須是不同實體磁碟', imageTitle:'先建映像', imageSub:'掃 .img，不反覆磨原碟'},
  diskPick:'建立映像檔（建議） <span class="muted-inline">— 實體磁碟不可直接掃描</span>', detecting:'— 正在偵測... —', sourcePath:'來源映像檔路徑', ntfsDepth:'NTFS 掃描深度', ntfsOptions:['標準：自動估算完整 MFT（建議）','快速：先掃前 10,000 筆','保守：先掃前 50,000 筆'], ntfsHelp:'救援數量太少時請用「標準」；它會先讀取 $MFT 大小估算總紀錄數，而不是固定只掃一小段。', apfsScope:'APFS 掃描範圍', apfsScopeOptions:['只找現存檔案（快速）','也找已刪除檔（需讀完整個容器，較慢）'], apfsScopeHelp:'APFS 已刪除檔來自殘留的中繼資料，內容無法驗證，還原後會放到 suspect/。',
  outputPath:'還原檔案儲存資料夾 <span class="danger-inline">請勿選來源磁碟</span>', outputHelp:'建議存到桌面、外接硬碟或另一個安全資料夾，避免把新檔案覆蓋到原本要救的磁碟。',
  riskTitle:'🟢 掃描前風險檢查', riskCheckLabel:'掃描前風險檢查', riskReady:'可以開始，尚未偵測到明顯風險。', riskLow:'低風險', riskInitial:'<strong>唯讀掃描。</strong> 開始前會檢查來源、格式與輸出位置，降低覆蓋原資料的風險。', riskRecommendation:'建議：救回檔案存到另一顆磁碟',
  imageWorkflowTitle:'🧊 v1.9 映像檔優先 + ddrescue map', imageWorkflowBadge:'建議流程', imageWorkflowSummary:'如果來源是實體磁碟，建議先建立唯讀映像檔，再用 ddrescue map 報告確認壞區/未完成比例，最後才掃描 .img。', imageWorkflowCommand:'選擇來源後會產生成像、map 視覺化、驗證與 retry 指令。', copyImageWorkflowCommand:'複製成像 / map / retry 指令',
  sessionWorkflowTitle:'🗂️ v2.7 救援案件 Session + 客戶交付包', sessionWorkflowBadge:'建議建立', sessionWorkflowSummary:'每次救援建立一個案件資料夾，把 image、map、scan、recovery、quality、log、Dashboard、客戶版摘要與工程交接報告放在一起。', activeSessionStatus:'尚未啟用 Session。可先按「直接建立 Session」，App 會自動把還原輸出指向案件資料夾。', sessionWorkflowCommand:'選擇來源後會產生 session 建立、索引與檢查指令。',
  sessionButtons:['直接建立 Session','開啟既有 Session','使用 Session 輸出','更新索引','檢查 Session','開啟索引','匯入報告 / Artifacts','產生交接報告','開啟客戶版','開啟工程版','建立客戶交付 ZIP','開啟交付資料夾','複製 Session 指令'],
  diagTitle:'🧰 實機環境檢查', emptyHint:'🛡️ 唯讀模式：本程式絕對不會寫入或修改您的來源磁碟', qualityNote:'✅ 預設只會自動勾選完整或中信心項目；低信心碎片不會自動勾選，避免把垃圾檔混進成功檔。', foundFiles:'找到的檔案', pathFilterPlaceholder:'🔍 篩選路徑（例如 /Users/me/Desktop）', clearFilter:'✕ 清除', quickRecover:'⚡ 立刻救回', quickRecoverConfirm:'立刻救回這個檔案？\n\n檔案：{name}\n大小：{size}\n輸出：{path}\n\n按「確認」開始。', quickRecoverUnknownSize:'未知', filters:['全部','🖼 圖片','📄 文件','🎬 影片','🎵 音訊','🗑️ 已刪除','✅ 完整','🟡 可疑可修','🧩 部分救援','⚠️ 碎片'], itemCount:'個項目', fileCount:'個檔案', selectedApprox:'已選 {count} 個檔案・約 {size}', bucketCounts:'完整 {complete}・可疑 {possible}・部分 {partial}・碎片 {fragments}｜完整檔進 recovered/，其他隔離到 suspect/', emptySelectionSub:'預設不勾選 partial / fragments；真的需要殘檔時再手動勾選。', selectionMain:'先勾選要救回來的檔案', selectionSub:'可用 Shift 連續選取；還原前會顯示儲存位置。',
  status:{deleted:'🗑️ 已刪除', normal:'正常'}, qualityLabels:{high:'完整', medium:'可疑', low:'碎片'}, bucketTitles:{complete:'完整可用', possible:'可疑可修', partial:'部分救援', fragments:'低信心碎片'}, bucketHints:{complete:'預設勾選，輸出到 recovered/', possible:'預設勾選，但輸出時可能隔離', partial:'需手動勾選，輸出到 suspect/partial', fragments:'需手動勾選，通常只供專家檢查'}, qualitySummary:'救援品質分流：完整 {complete}、可疑可修 {possible}、部分救援 {partial}、低信心碎片 {fragments}。低信心項目不會自動勾選，避免垃圾檔混進成功結果。', qualityEmpty:'尚未載入掃描結果。開始掃描後會依品質分成 recovered/ 與 suspect/。', pathFilterLabel:'依路徑篩選結果',
  categoryLabels:{image:'圖片', doc:'文件', video:'影片', audio:'音訊'}, noFiles:'未找到任何檔案', noFilteredFiles:'此磁碟未找到「{type}」類型的可救援檔案', listHead:['檔名','狀態','判定','分流','大小'], selectAria:'選取 {name}', filePath:'檔案路徑', detailsPrompt:'請點擊左側的檔案以查看詳細資訊', nodeType:'節點類型', allocationStatus:'分配狀態', fileSize:'檔案大小', recoveryConfidence:'救援信心', confidence:'信心值', routeStrategy:'分流策略', qualityTag:'品質標記', readMode:'讀取模式', lostBytes:'遺失 {bytes} bytes', outputPreview:'預計還原到', folderNotRestorable:'資料夾本身不可直接還原，請勾選裡面的檔案',
  advancedSettings:'進階設定', advancedNotice:'🛡️ 請將還原的檔案存到其他磁碟，切勿存回損毀的來源磁碟！', generatedCommand:'生成的 CLI 執行指令', confirm:'確認', cancel:'取消', close:'關閉', sessionPathLabel:'Session 資料夾路徑', sessionPathPrompt:'請貼上既有 Session 資料夾路徑。', artifactPathLabel:'Artifact 檔案或資料夾路徑', artifactPathPrompt:'請貼上要匯入的檔案或資料夾路徑。', artifactKindLabel:'Artifact 類型', artifactKindPrompt:'可輸入 auto，或選擇已知的 Artifact 類型。', imageWorkflowDisclosure:'🧊 映像檔優先流程', sessionWorkflowDisclosure:'🗂️ 案件／Session 工具', diagDisclosure:'🧰 實機環境檢查', recoveryReportTitle:'還原完成報告', recoveryReportNote:'完整成功檔會在 recovered/，可疑、部分或低信心結果會隔離到 suspect/。', openHtmlReport:'🌐 開啟 HTML 報告', copySummary:'📋 複製摘要', collapseReport:'收起報告', copied:'✓ 已複製', commandCopied:'指令已複製。', sampleLoaded:'已載入範例結果。正式救援請用桌面 App 開始掃描。',
  metrics:{default:['掃描的記錄數','結果中的檔案','結果中的已刪除檔案','結果資料大小','CLI 報告輸出 (JSON / Text)','VFS 掃描結果 (JSON / Text)'], carver:['偵測到的特徵碼','成功彫刻數','信心值','掃描資料量','深度掃描日誌','彫刻結果清單'], apfs:['超級區塊狀態','物件管理 OID','卷宗個數','Block 大小','APFS 超區元數據','子卷結構列表']},
  modes:{
    ntfs:{nav:'💾 硬碟掃描與救援', title:'硬碟掃描與救援', badge:'Windows / NTFS', emptyTitle:'掃描硬碟，找回遺失檔案', desc:'適合一般 Windows 硬碟、SSD 或 NTFS 分割區。掃描後會列出檔案樹，讓你勾選要還原的項目。', advice:'適合 Windows 硬碟或 NTFS 分割區，建議先跑一般掃描；找不到再改用深度碎塊掃描。'},
    exfat:{nav:'📁 exFAT 隨身碟救援', title:'隨身碟掃描與救援', badge:'隨身碟 / exFAT', emptyTitle:'掃描 exFAT 隨身碟', desc:'適合容量較大的 USB 隨身碟、外接硬碟、記憶卡等 exFAT 裝置。請把救回來的檔案存到電腦，不要存回同一支隨身碟。', advice:'適合 exFAT 隨身碟、記憶卡與外接儲存裝置。若檔案系統損毀嚴重，可改用深度碎塊掃描。'},
    fat32:{nav:'🧩 FAT32 記憶卡救援', title:'FAT32 記憶卡救援', badge:'記憶卡 / FAT32', emptyTitle:'掃描 FAT32 記憶卡或舊隨身碟', desc:'適合相機 SD 卡、舊隨身碟、車機記憶卡等 FAT32 裝置。會讀目錄 entry 與 FAT 表，比純深度掃描更不容易產生垃圾檔。', advice:'適合 FAT32 記憶卡、相機卡與舊隨身碟。刪除檔若 FAT chain 已被清除，會改用連續 cluster 保守還原；碎片化大檔仍可能部分損毀。'},
    apfs:{nav:'🍎 Mac 磁碟掃描與救援', title:'Mac 磁碟掃描與救援', badge:'Mac / APFS', emptyTitle:'掃描 Mac 磁碟或 APFS 容器', desc:'適合 macOS 的 APFS 磁碟。v1.2 已接上未加密卷宗的 extent 提取；FileVault、壓縮、clone 或 snapshot-only 仍會被標記為可疑。', advice:'適合 Mac 內建或外接 APFS 磁碟，會優先讀取容器、卷宗與 extent。若疑似加密或 SSD/TRIM，建議先做映像檔並降低成功率預期。'},
    carver:{nav:'🔍 深度碎塊掃描 (Deep Scan)', title:'深度碎塊掃描 (Deep Scan)', badge:'深度掃描', emptyTitle:'深度搜尋散落的檔案碎塊', desc:'當磁碟格式看不出來、分割表毀損，或一般掃描找不到檔案時使用。v1.2 保留嚴格格式驗證、風險提示，並新增 APFS extent 提取路徑。', advice:'適合分割表毀損、格式未知或一般掃描失敗的狀況。嚴格格式驗證會減少垃圾檔，低信心結果會隔離顯示。'}
  }
};
Object.assign(I18N, {
  'zh-Hans': { appTitle:'Polik 数据恢复', brandLogo:'P', brandName:'Polik 数据恢复', brandSubtitle:'安全恢复重要文件', navLabel:'扫描类型', sampleLabel:'示例数据', loadSample:'↻ 加载所选示例', advancedMode:'⚙️ 高级模式', sidebarNote:'只读访问磁盘，不修改源数据。', workbench:'数据恢复工作台', runtimePreview:'预览模式', reanalyze:'⟳ 重新分析', copyCommand:'⇩ 复制命令', startScan:'▶ 开始扫描', stop:'⏹ 停止', restoreSelected:'💾 恢复所选文件', recommendedFlow:'建议流程', choose:'📁 选择', regenerate:'重新生成', checkAgain:'重新检查', notChecked:'尚未检查', startSafeScan:'▶ 开始安全扫描', previewSample:'👀 查看示例', clear:'清除', selectUsable:'选择可用文件', selectedPrefix:'已选', details:'文件详细信息', notSelected:'未选择', sessionWorkflowTitle:'🗂️ v2.7 恢复案例 Session + 客户交付包', sessionWorkflowBadge:'建议建立', sessionWorkflowSummary:'每次恢复建立一个案例文件夹，统一存放 image、map、scan、recovery、quality、log、Dashboard、客户摘要与工程交接报告。', activeSessionStatus:'尚未激活 Session。点击「直接建立 Session」，App 会自动将恢复输出指向案例文件夹。', sessionWorkflowCommand:'选择来源后生成 session 建立、索引与检查命令。', sessionButtons:['直接建立 Session','打开既有 Session','使用 Session 输出','更新索引','检查 Session','打开索引','导入报告 / Artifacts','生成交接报告','打开客户版','打开工程版','建立客户交付 ZIP','打开交付文件夹','复制 Session 命令'], pathFilterPlaceholder:'🔍 按路径筛选（例如 /Users/me/Desktop）', clearFilter:'✕ 清除', quickRecover:'⚡ 立即恢复', quickRecoverConfirm:'立即恢复此文件？\n\n文件：{name}\n大小：{size}\n输出：{path}\n\n点击确定开始。', quickRecoverUnknownSize:'未知', modes:{ntfs:{nav:'💾 磁盘扫描与恢复',title:'磁盘扫描与恢复',badge:'Windows / NTFS',emptyTitle:'扫描磁盘并恢复丢失文件',desc:'适用于 Windows 磁盘、SSD 或 NTFS 分区。扫描后可从文件树选择要恢复的项目。',advice:'适合 Windows 磁盘或 NTFS 分区。先普通扫描，找不到再用深度扫描。'},exfat:{nav:'📁 exFAT U 盘恢复',title:'exFAT 可移动磁盘恢复',badge:'可移动磁盘 / exFAT',emptyTitle:'扫描 exFAT U 盘',desc:'适用于较大的 USB、外接磁盘和 exFAT 存储卡。请将恢复文件保存到电脑或其他磁盘。',advice:'适合 exFAT USB、存储卡与外接设备。文件系统严重损坏时改用深度扫描。'},fat32:{nav:'🧩 FAT32 存储卡恢复',title:'FAT32 存储卡恢复',badge:'存储卡 / FAT32',emptyTitle:'扫描 FAT32 存储卡或旧 U 盘',desc:'适用于相机 SD 卡、旧 U 盘与 FAT32 设备。',advice:'适合 FAT32 相机卡与旧 U 盘。删除文件可能需要保守连续簇恢复。'},apfs:{nav:'🍎 Mac 磁盘恢复',title:'Mac 磁盘恢复',badge:'Mac / APFS',emptyTitle:'扫描 Mac 磁盘或 APFS 容器',desc:'适用于 macOS APFS 磁盘。未加密 extent 可提取；加密、压缩、clone 或 snapshot-only 会标记为可疑。',advice:'适合内置或外接 APFS 磁盘。疑似加密或 SSD/TRIM 时请先制作映像。'},carver:{nav:'🔍 深度扫描',title:'深度扫描',badge:'深度扫描',emptyTitle:'搜索散落文件碎片',desc:'磁盘格式未知、分区表损坏或普通扫描找不到文件时使用。',advice:'适合损坏分区或未知格式；严格格式检查会减少垃圾文件。'}} },
  ja: { appTitle:'Polik Recovery', brandSubtitle:'重要なファイルを安全に復旧', navLabel:'スキャン種類', sampleLabel:'サンプルデータ', loadSample:'↻ 選択中のサンプルを読み込む', advancedMode:'⚙️ 詳細モード', sidebarNote:'読み取り専用でアクセスし、元ディスクは変更しません。', workbench:'復旧ワークベンチ', runtimePreview:'プレビューモード', reanalyze:'⟳ 再解析', copyCommand:'⇩ コマンドをコピー', startScan:'▶ スキャン開始', stop:'⏹ 停止', restoreSelected:'💾 選択ファイルを復元', recommendedFlow:'推奨フロー', choose:'📁 選択', regenerate:'再生成', checkAgain:'再チェック', notChecked:'未チェック', startSafeScan:'▶ 安全スキャン開始', previewSample:'👀 サンプル表示', clear:'クリア', selectUsable:'復元可能を選択', selectedPrefix:'選択済み', details:'ファイル詳細', notSelected:'未選択', sessionWorkflowTitle:'🗂️ v2.7 復旧ケース Session + 顧客パッケージ', sessionWorkflowBadge:'推奨', sessionWorkflowSummary:'復旧ごとにケースフォルダを作り、image・map・scan・recovery・quality・log・Dashboard・顧客向け報告・エンジニア引き継ぎ資料をまとめて管理します。', activeSessionStatus:'Session は未アクティブです。「Session 作成」をクリックすると、復旧出力が自動的にケースフォルダに向きます。', sessionWorkflowCommand:'ソースを選択すると、session 作成・索引・確認コマンドを生成します。', sessionButtons:['Session を作成','既存 Session を開く','Session 出力を使用','索引を更新','Session を確認','索引を開く','レポート / Artifacts をインポート','引き継ぎレポートを生成','顧客版を開く','エンジニア版を開く','顧客配布 ZIP を作成','配布フォルダを開く','Session コマンドをコピー'], pathFilterPlaceholder:'🔍 パスで絞り込み（例：/Users/me/Desktop）', clearFilter:'✕ クリア', quickRecover:'⚡ すぐに復元', quickRecoverConfirm:'このファイルをすぐに復元しますか？\n\nファイル：{name}\nサイズ：{size}\n出力先：{path}\n\nOKをクリックして開始。', quickRecoverUnknownSize:'不明', modes:{ntfs:{nav:'💾 ディスクスキャンと復旧',title:'ディスクスキャンと復旧',badge:'Windows / NTFS',emptyTitle:'ディスクをスキャンして失われたファイルを復旧',desc:'Windows ディスク、SSD、NTFS パーティション向けです。',advice:'まず通常スキャンを実行し、見つからない場合は Deep Scan を使います。'},exfat:{nav:'📁 exFAT リムーバブル復旧',title:'exFAT リムーバブル復旧',badge:'リムーバブル / exFAT',emptyTitle:'exFAT USB ドライブをスキャン',desc:'USB、外付けディスク、メモリカード向けです。',advice:'exFAT ストレージ向け。破損が大きい場合は Deep Scan を使います。'},fat32:{nav:'🧩 FAT32 カード復旧',title:'FAT32 カード復旧',badge:'メモリカード / FAT32',emptyTitle:'FAT32 カードをスキャン',desc:'カメラ SD カードや古い USB 向けです。',advice:'FAT32 メディア向け。削除済みファイルは保守的に復旧します。'},apfs:{nav:'🍎 Mac ディスク復旧',title:'Mac ディスク復旧',badge:'Mac / APFS',emptyTitle:'Mac ディスクまたは APFS コンテナをスキャン',desc:'macOS APFS ディスク向けです。',advice:'暗号化や SSD/TRIM が疑われる場合は先にイメージを作成してください。'},carver:{nav:'🔍 Deep Scan',title:'Deep Scan',badge:'Deep Scan',emptyTitle:'散在するファイル断片を検索',desc:'形式不明やパーティション破損時に使用します。',advice:'厳格な形式チェックで不要ファイルを減らします。'}} },
  ko: { appTitle:'Polik Recovery', brandSubtitle:'중요 파일을 안전하게 복구', navLabel:'스캔 유형', sampleLabel:'샘플 데이터', loadSample:'↻ 선택한 샘플 불러오기', advancedMode:'⚙️ 고급 모드', sidebarNote:'읽기 전용으로 접근하며 원본 디스크를 수정하지 않습니다.', workbench:'복구 작업대', runtimePreview:'미리보기 모드', reanalyze:'⟳ 다시 분석', copyCommand:'⇩ 명령 복사', startScan:'▶ 스캔 시작', stop:'⏹ 중지', restoreSelected:'💾 선택 파일 복원', recommendedFlow:'권장 흐름', choose:'📁 선택', regenerate:'다시 생성', checkAgain:'다시 검사', notChecked:'검사 안 함', startSafeScan:'▶ 안전 스캔 시작', previewSample:'👀 샘플 보기', clear:'지우기', selectUsable:'사용 가능 선택', selectedPrefix:'선택됨', details:'파일 세부 정보', notSelected:'선택 안 됨', sessionWorkflowTitle:'🗂️ v2.7 복구 케이스 Session + 고객 패키지', sessionWorkflowBadge:'권장', sessionWorkflowSummary:'복구마다 케이스 폴더를 만들어 image, map, scan, recovery, quality, log, Dashboard, 고객 요약 및 엔지니어 인수인계 자료를 한곳에 관리합니다.', activeSessionStatus:'Session이 활성화되지 않았습니다. 「Session 생성」을 클릭하면 복구 출력이 자동으로 케이스 폴더로 지정됩니다.', sessionWorkflowCommand:'소스를 선택하면 session 생성, 인덱싱, 확인 명령을 생성합니다.', sessionButtons:['Session 생성','기존 Session 열기','Session 출력 사용','인덱스 업데이트','Session 확인','인덱스 열기','보고서 / Artifacts 가져오기','인계 보고서 생성','고객 버전 열기','엔지니어 버전 열기','고객 배포 ZIP 생성','배포 폴더 열기','Session 명령 복사'], pathFilterPlaceholder:'🔍 경로로 필터링 (예: /Users/me/Desktop)', clearFilter:'✕ 지우기', quickRecover:'⚡ 바로 복원', quickRecoverConfirm:'이 파일을 바로 복원하시겠습니까?\n\n파일: {name}\n크기: {size}\n출력: {path}\n\n확인을 클릭하여 시작하세요.', quickRecoverUnknownSize:'알 수 없음', modes:{ntfs:{nav:'💾 디스크 스캔 및 복구',title:'디스크 스캔 및 복구',badge:'Windows / NTFS',emptyTitle:'디스크를 스캔해 손실 파일 복구',desc:'Windows 디스크, SSD, NTFS 파티션용입니다.',advice:'일반 스캔 후 필요하면 Deep Scan을 사용하세요.'},exfat:{nav:'📁 exFAT 이동식 복구',title:'exFAT 이동식 복구',badge:'이동식 / exFAT',emptyTitle:'exFAT USB 드라이브 스캔',desc:'USB, 외장 디스크, 메모리 카드용입니다.',advice:'손상이 심하면 Deep Scan을 사용하세요.'},fat32:{nav:'🧩 FAT32 카드 복구',title:'FAT32 카드 복구',badge:'메모리 카드 / FAT32',emptyTitle:'FAT32 카드 또는 오래된 USB 스캔',desc:'카메라 SD 카드와 FAT32 장치용입니다.',advice:'삭제 파일은 보수적으로 복구됩니다.'},apfs:{nav:'🍎 Mac 디스크 복구',title:'Mac 디스크 복구',badge:'Mac / APFS',emptyTitle:'Mac 디스크 또는 APFS 컨테이너 스캔',desc:'macOS APFS 디스크용입니다.',advice:'암호화나 SSD/TRIM이 의심되면 먼저 이미지를 만드세요.'},carver:{nav:'🔍 Deep Scan',title:'Deep Scan',badge:'Deep Scan',emptyTitle:'흩어진 파일 조각 검색',desc:'형식 불명 또는 파티션 손상 시 사용합니다.',advice:'엄격한 형식 검사로 불필요한 결과를 줄입니다.'}} },
  es: { appTitle:'Polik Recovery', brandSubtitle:'Recupera archivos importantes con seguridad', navLabel:'Tipo de escaneo', sampleLabel:'Datos de muestra', loadSample:'↻ Cargar muestra seleccionada', advancedMode:'⚙️ Modo avanzado', sidebarNote:'Acceso de solo lectura. El disco origen no se modifica.', workbench:'Panel de recuperación', runtimePreview:'Modo vista previa', reanalyze:'⟳ Reanalizar', copyCommand:'⇩ Copiar comando', startScan:'▶ Iniciar escaneo', stop:'⏹ Detener', restoreSelected:'💾 Restaurar seleccionados', recommendedFlow:'Flujo recomendado', choose:'📁 Elegir', regenerate:'Regenerar', checkAgain:'Comprobar de nuevo', notChecked:'Sin comprobar', startSafeScan:'▶ Escaneo seguro', previewSample:'👀 Ver muestra', clear:'Limpiar', selectUsable:'Seleccionar útiles', selectedPrefix:'Seleccionados', details:'Detalles del archivo', notSelected:'Sin selección', sessionWorkflowTitle:'🗂️ v2.7 Caso de recuperación Session + Paquete cliente', sessionWorkflowBadge:'Recomendado', sessionWorkflowSummary:'Crea una carpeta por recuperación con imagen, mapa, escaneo, recuperación, calidad, log, Dashboard, resumen para el cliente y traspaso al ingeniero.', activeSessionStatus:'No hay Session activa. Usa «Crear Session» para que la salida se dirija automáticamente a la carpeta del caso.', sessionWorkflowCommand:'Selecciona un origen para generar los comandos de creación, indexación y verificación de la Session.', sessionButtons:['Crear Session','Abrir Session existente','Usar salida de Session','Actualizar índice','Verificar Session','Abrir índice','Importar informes / Artifacts','Generar informe de traspaso','Abrir versión cliente','Abrir versión ingeniero','Crear ZIP para cliente','Abrir carpeta de entrega','Copiar comando de Session'], pathFilterPlaceholder:'🔍 Filtrar por ruta (ej. /Users/me/Escritorio)', clearFilter:'✕ Limpiar', quickRecover:'⚡ Recuperar ahora', quickRecoverConfirm:'¿Recuperar este archivo ahora?\n\nArchivo: {name}\nTamaño: {size}\nDestino: {path}\n\nHaz clic en Aceptar para comenzar.', quickRecoverUnknownSize:'Desconocido', modes:{ntfs:{nav:'💾 Escaneo y recuperación de disco',title:'Escaneo y recuperación de disco',badge:'Windows / NTFS',emptyTitle:'Escanea un disco y recupera archivos perdidos',desc:'Para discos Windows, SSD o particiones NTFS.',advice:'Empieza con un escaneo normal y usa Deep Scan si hace falta.'},exfat:{nav:'📁 Recuperación exFAT',title:'Recuperación exFAT',badge:'Extraíble / exFAT',emptyTitle:'Escanear unidad USB exFAT',desc:'Para USB, discos externos y tarjetas exFAT.',advice:'Usa Deep Scan si el sistema de archivos está muy dañado.'},fat32:{nav:'🧩 Recuperación FAT32',title:'Recuperación de tarjeta FAT32',badge:'Tarjeta / FAT32',emptyTitle:'Escanear tarjeta FAT32',desc:'Para tarjetas SD de cámara y USB antiguos.',advice:'Los borrados pueden requerir recuperación conservadora.'},apfs:{nav:'🍎 Recuperación de disco Mac',title:'Recuperación de disco Mac',badge:'Mac / APFS',emptyTitle:'Escanear disco Mac o contenedor APFS',desc:'Para discos APFS de macOS.',advice:'Si hay cifrado o SSD/TRIM, crea una imagen primero.'},carver:{nav:'🔍 Deep Scan',title:'Deep Scan',badge:'Deep Scan',emptyTitle:'Buscar fragmentos de archivos',desc:'Para formatos desconocidos o particiones dañadas.',advice:'Las validaciones estrictas reducen archivos basura.'}} },
  fr: { appTitle:'Polik Recovery', brandSubtitle:'Récupérez vos fichiers importants en sécurité', navLabel:'Type d’analyse', sampleLabel:'Données exemple', loadSample:'↻ Charger l’exemple sélectionné', advancedMode:'⚙️ Mode avancé', sidebarNote:'Accès en lecture seule. Le disque source n’est pas modifié.', workbench:'Atelier de récupération', runtimePreview:'Mode aperçu', desktopMode:'Mode application bureau', reanalyze:'⟳ Réanalyser', copyCommand:'⇩ Copier la commande', startScan:'▶ Lancer l’analyse', stop:'⏹ Arrêter', restoreSelected:'💾 Restaurer la sélection', recommendedFlow:'Flux recommandé', choose:'📁 Choisir', regenerate:'Régénérer', checkAgain:'Revérifier', notChecked:'Non vérifié', startSafeScan:'▶ Analyse sûre', previewSample:'👀 Voir exemple', clear:'Effacer', selectUsable:'Sélectionner utilisables', selectedPrefix:'Sélectionnés', details:'Détails du fichier', notSelected:'Non sélectionné', sessionWorkflowTitle:'🗂️ v2.7 Session de cas de récupération + Package client', sessionWorkflowBadge:'Recommandé', sessionWorkflowSummary:'Créez un dossier par récupération pour regrouper l’image, la carte, le scan, la récupération, la qualité, les logs, le Dashboard, le résumé client et la passation ingénieur.', activeSessionStatus:'Aucune Session active. Cliquez sur « Créer Session » pour que la sortie soit automatiquement dirigée vers le dossier du cas.', sessionWorkflowCommand:'Sélectionnez une source pour générer les commandes de création, d’indexation et de vérification de la Session.', sessionButtons:['Créer Session','Ouvrir Session existante','Utiliser sortie Session','Mettre à jour l’index','Vérifier Session','Ouvrir index','Importer rapports / Artifacts','Générer rapport de passation','Ouvrir version client','Ouvrir version ingénieur','Créer ZIP client','Ouvrir dossier livraison','Copier commande Session'], pathFilterPlaceholder:'🔍 Filtrer par chemin (ex. /Users/moi/Bureau)', clearFilter:'✕ Effacer', quickRecover:'⚡ Récupérer maintenant', quickRecoverConfirm:'Récupérer ce fichier maintenant ?\n\nFichier : {name}\nTaille : {size}\nDestination : {path}\n\nCliquez sur OK pour commencer.', quickRecoverUnknownSize:'Inconnu', modes:{ntfs:{nav:'💾 Analyse et récupération disque',title:'Analyse et récupération disque',badge:'Windows / NTFS',emptyTitle:'Analyser un disque et récupérer les fichiers perdus',desc:'Pour disques Windows, SSD ou partitions NTFS.',advice:'Commencez par une analyse normale, puis Deep Scan si besoin.'},exfat:{nav:'📁 Récupération exFAT',title:'Récupération exFAT',badge:'Amovible / exFAT',emptyTitle:'Analyser une clé USB exFAT',desc:'Pour clés USB, disques externes et cartes exFAT.',advice:'Utilisez Deep Scan si le système est très endommagé.'},fat32:{nav:'🧩 Récupération FAT32',title:'Récupération carte FAT32',badge:'Carte / FAT32',emptyTitle:'Analyser une carte FAT32',desc:'Pour cartes SD et anciens USB FAT32.',advice:'Les fichiers supprimés peuvent nécessiter une récupération prudente.'},apfs:{nav:'🍎 Récupération disque Mac',title:'Récupération disque Mac',badge:'Mac / APFS',emptyTitle:'Analyser un disque Mac ou conteneur APFS',desc:'Pour disques APFS macOS.',advice:'Créez d’abord une image en cas de chiffrement ou SSD/TRIM.'},carver:{nav:'🔍 Deep Scan',title:'Deep Scan',badge:'Deep Scan',emptyTitle:'Rechercher des fragments de fichiers',desc:'Pour formats inconnus ou partitions endommagées.',advice:'Des contrôles stricts réduisent les faux fichiers.'}} },
  de: { appTitle:'Polik Recovery', brandSubtitle:'Wichtige Dateien sicher wiederherstellen', navLabel:'Scan-Typ', sampleLabel:'Beispieldaten', loadSample:'↻ Ausgewähltes Beispiel laden', advancedMode:'⚙️ Expertenmodus', sidebarNote:'Nur-Lese-Zugriff. Das Quelllaufwerk wird nicht verändert.', workbench:'Recovery Workbench', runtimePreview:'Vorschaumodus', reanalyze:'⟳ Neu analysieren', copyCommand:'⇩ Befehl kopieren', startScan:'▶ Scan starten', stop:'⏹ Stoppen', restoreSelected:'💾 Ausgewählte Dateien wiederherstellen', recommendedFlow:'Empfohlener Ablauf', choose:'📁 Wählen', regenerate:'Neu erzeugen', checkAgain:'Erneut prüfen', notChecked:'Nicht geprüft', startSafeScan:'▶ Sicheren Scan starten', previewSample:'👀 Beispiel anzeigen', clear:'Leeren', selectUsable:'Brauchbare wählen', selectedPrefix:'Ausgewählt', details:'Dateidetails', notSelected:'Nicht ausgewählt', sessionWorkflowTitle:'🗂️ v2.7 Recovery-Fall Session + Kundenpake', sessionWorkflowBadge:'Empfohlen', sessionWorkflowSummary:'Legen Sie pro Wiederherstellung einen Fallordner an, um Image, Map, Scan, Recovery, Qualität, Log, Dashboard, Kundenzusammenfassung und Ingenieur-Übergabe an einem Ort zu bündeln.', activeSessionStatus:'Keine Session aktiv. Klicken Sie auf „Session erstellen", damit die Ausgabe automatisch in den Fallordner geleitet wird.', sessionWorkflowCommand:'Wählen Sie eine Quelle, um Session-Erstellungs-, Index- und Prüfbefehle zu erzeugen.', sessionButtons:['Session erstellen','Vorhandene Session öffnen','Session-Ausgabe verwenden','Index aktualisieren','Session prüfen','Index öffnen','Berichte / Artifacts importieren','Übergabebericht erstellen','Kundenversion öffnen','Ingenieurversion öffnen','Kunden-ZIP erstellen','Lieferordner öffnen','Session-Befehl kopieren'], pathFilterPlaceholder:'🔍 Nach Pfad filtern (z. B. /Users/ich/Schreibtisch)', clearFilter:'✕ Leeren', quickRecover:'⚡ Jetzt wiederherstellen', quickRecoverConfirm:'Diese Datei jetzt wiederherstellen?\n\nDatei: {name}\nGröße: {size}\nZiel: {path}\n\nOK klicken zum Starten.', quickRecoverUnknownSize:'Unbekannt', modes:{ntfs:{nav:'💾 Datenträger-Scan und Recovery',title:'Datenträger-Scan und Recovery',badge:'Windows / NTFS',emptyTitle:'Datenträger scannen und verlorene Dateien retten',desc:'Für Windows-Datenträger, SSDs oder NTFS-Partitionen.',advice:'Zuerst normal scannen, danach bei Bedarf Deep Scan.'},exfat:{nav:'📁 exFAT-Wechseldatenträger',title:'exFAT-Wiederherstellung',badge:'Wechselmedium / exFAT',emptyTitle:'exFAT-USB-Laufwerk scannen',desc:'Für USB, externe Datenträger und exFAT-Karten.',advice:'Bei starken Schäden Deep Scan verwenden.'},fat32:{nav:'🧩 FAT32-Kartenrettung',title:'FAT32-Kartenrettung',badge:'Speicherkarte / FAT32',emptyTitle:'FAT32-Karte scannen',desc:'Für Kamera-SD-Karten und ältere USB-Laufwerke.',advice:'Gelöschte Dateien werden konservativ wiederhergestellt.'},apfs:{nav:'🍎 Mac-Datenträgerrettung',title:'Mac-Datenträgerrettung',badge:'Mac / APFS',emptyTitle:'Mac-Datenträger oder APFS-Container scannen',desc:'Für macOS-APFS-Datenträger.',advice:'Bei Verschlüsselung oder SSD/TRIM zuerst ein Image erstellen.'},carver:{nav:'🔍 Deep Scan',title:'Deep Scan',badge:'Deep Scan',emptyTitle:'Verstreute Dateifragmente suchen',desc:'Für unbekannte Formate oder beschädigte Partitionen.',advice:'Strenge Formatprüfungen reduzieren Müll-Dateien.'}} },
  'pt-BR': { appTitle:'Polik Recovery', brandSubtitle:'Recupere arquivos importantes com segurança', navLabel:'Tipo de varredura', sampleLabel:'Dados de exemplo', loadSample:'↻ Carregar exemplo selecionado', advancedMode:'⚙️ Modo avançado', sidebarNote:'Acesso somente leitura. O disco de origem não é modificado.', workbench:'Bancada de recuperação', runtimePreview:'Modo de prévia', reanalyze:'⟳ Reanalisar', copyCommand:'⇩ Copiar comando', startScan:'▶ Iniciar varredura', stop:'⏹ Parar', restoreSelected:'💾 Restaurar selecionados', recommendedFlow:'Fluxo recomendado', choose:'📁 Escolher', regenerate:'Gerar novamente', checkAgain:'Verificar novamente', notChecked:'Não verificado', startSafeScan:'▶ Varredura segura', previewSample:'👀 Ver exemplo', clear:'Limpar', selectUsable:'Selecionar úteis', selectedPrefix:'Selecionados', details:'Detalhes do arquivo', notSelected:'Não selecionado', sessionWorkflowTitle:'🗂️ v2.7 Caso de recuperação Session + Pacote cliente', sessionWorkflowBadge:'Recomendado', sessionWorkflowSummary:'Crie uma pasta por recuperação com imagem, mapa, varredura, recuperação, qualidade, log, Dashboard, resumo do cliente e entrega ao engenheiro, tudo em um lugar.', activeSessionStatus:'Nenhuma Session ativa. Clique em «Criar Session» para direcionar automaticamente a saída para a pasta do caso.', sessionWorkflowCommand:'Selecione uma fonte para gerar comandos de criação, indexação e verificação da Session.', sessionButtons:['Criar Session','Abrir Session existente','Usar saída da Session','Atualizar índice','Verificar Session','Abrir índice','Importar relatórios / Artifacts','Gerar relatório de entrega','Abrir versão cliente','Abrir versão engenheiro','Criar ZIP do cliente','Abrir pasta de entrega','Copiar comando da Session'], pathFilterPlaceholder:'🔍 Filtrar por caminho (ex. /Users/eu/Desktop)', clearFilter:'✕ Limpar', quickRecover:'⚡ Recuperar agora', quickRecoverConfirm:'Recuperar este arquivo agora?\n\nArquivo: {name}\nTamanho: {size}\nDestino: {path}\n\nClique em OK para começar.', quickRecoverUnknownSize:'Desconhecido', modes:{ntfs:{nav:'💾 Varredura e recuperação de disco',title:'Varredura e recuperação de disco',badge:'Windows / NTFS',emptyTitle:'Varra um disco e recupere arquivos perdidos',desc:'Para discos Windows, SSDs ou partições NTFS.',advice:'Comece com varredura normal e use Deep Scan se necessário.'},exfat:{nav:'📁 Recuperação exFAT',title:'Recuperação exFAT',badge:'Removível / exFAT',emptyTitle:'Varrer unidade USB exFAT',desc:'Para USB, discos externos e cartões exFAT.',advice:'Use Deep Scan se o sistema estiver muito danificado.'},fat32:{nav:'🧩 Recuperação FAT32',title:'Recuperação de cartão FAT32',badge:'Cartão / FAT32',emptyTitle:'Varrer cartão FAT32',desc:'Para cartões SD de câmera e USB antigos.',advice:'Arquivos excluídos podem exigir recuperação conservadora.'},apfs:{nav:'🍎 Recuperação de disco Mac',title:'Recuperação de disco Mac',badge:'Mac / APFS',emptyTitle:'Varrer disco Mac ou contêiner APFS',desc:'Para discos APFS do macOS.',advice:'Se houver criptografia ou SSD/TRIM, crie uma imagem primeiro.'},carver:{nav:'🔍 Deep Scan',title:'Deep Scan',badge:'Deep Scan',emptyTitle:'Procurar fragmentos de arquivos',desc:'Para formatos desconhecidos ou partições danificadas.',advice:'Verificações estritas reduzem arquivos inúteis.'}} },
  id: { appTitle:'Polik Recovery', brandSubtitle:'Pulihkan file penting dengan aman', navLabel:'Jenis pemindaian', sampleLabel:'Data contoh', loadSample:'↻ Muat contoh terpilih', advancedMode:'⚙️ Mode lanjutan', sidebarNote:'Akses hanya-baca. Disk sumber tidak diubah.', workbench:'Meja kerja pemulihan', runtimePreview:'Mode pratinjau', reanalyze:'⟳ Analisis ulang', copyCommand:'⇩ Salin perintah', startScan:'▶ Mulai Pindai', stop:'⏹ Hentikan', restoreSelected:'💾 Pulihkan File Terpilih', recommendedFlow:'Alur disarankan', choose:'📁 Pilih', regenerate:'Buat ulang', checkAgain:'Periksa lagi', notChecked:'Belum diperiksa', startSafeScan:'▶ Mulai Pindai Aman', previewSample:'👀 Lihat contoh', clear:'Bersihkan', selectUsable:'Pilih yang bisa dipakai', selectedPrefix:'Dipilih', details:'Detail file', notSelected:'Belum dipilih', sessionWorkflowTitle:'🗂️ v2.7 Kasus pemulihan Session + Paket pelanggan', sessionWorkflowBadge:'Disarankan', sessionWorkflowSummary:'Buat satu folder kasus per pemulihan untuk menyimpan image, peta, pemindaian, pemulihan, kualitas, log, Dashboard, ringkasan pelanggan, dan serah terima ke insinyur.', activeSessionStatus:'Tidak ada Session aktif. Klik «Buat Session» agar output pemulihan otomatis diarahkan ke folder kasus.', sessionWorkflowCommand:'Pilih sumber untuk menghasilkan perintah pembuatan session, pengindeksan, dan pemeriksaan.', sessionButtons:['Buat Session','Buka Session yang ada','Gunakan output Session','Perbarui indeks','Periksa Session','Buka indeks','Impor laporan / Artifacts','Buat laporan serah terima','Buka versi pelanggan','Buka versi insinyur','Buat ZIP pelanggan','Buka folder pengiriman','Salin perintah Session'], pathFilterPlaceholder:'🔍 Filter berdasarkan jalur (mis. /Users/saya/Desktop)', clearFilter:'✕ Bersihkan', quickRecover:'⚡ Pulihkan sekarang', quickRecoverConfirm:'Pulihkan file ini sekarang?\n\nFile: {name}\nUkuran: {size}\nOutput: {path}\n\nKlik OK untuk mulai.', quickRecoverUnknownSize:'Tidak diketahui', modes:{ntfs:{nav:'💾 Pindai dan pulihkan disk',title:'Pindai dan Pulihkan Disk',badge:'Windows / NTFS',emptyTitle:'Pindai disk dan pulihkan file hilang',desc:'Untuk disk Windows, SSD, atau partisi NTFS.',advice:'Mulai dengan pemindaian normal, lalu Deep Scan jika perlu.'},exfat:{nav:'📁 Pemulihan exFAT',title:'Pemulihan exFAT',badge:'Removable / exFAT',emptyTitle:'Pindai drive USB exFAT',desc:'Untuk USB, disk eksternal, dan kartu exFAT.',advice:'Gunakan Deep Scan jika sistem file rusak berat.'},fat32:{nav:'🧩 Pemulihan kartu FAT32',title:'Pemulihan Kartu FAT32',badge:'Kartu / FAT32',emptyTitle:'Pindai kartu FAT32',desc:'Untuk kartu SD kamera dan USB lama.',advice:'File terhapus mungkin perlu pemulihan konservatif.'},apfs:{nav:'🍎 Pemulihan disk Mac',title:'Pemulihan Disk Mac',badge:'Mac / APFS',emptyTitle:'Pindai disk Mac atau kontainer APFS',desc:'Untuk disk APFS macOS.',advice:'Jika ada enkripsi atau SSD/TRIM, buat image terlebih dahulu.'},carver:{nav:'🔍 Deep Scan',title:'Deep Scan',badge:'Deep Scan',emptyTitle:'Cari fragmen file tersebar',desc:'Untuk format tidak dikenal atau partisi rusak.',advice:'Pemeriksaan format ketat mengurangi file sampah.'}} }
});

// ── Sample data ──
const samples = {
  ntfs: {
    title:"硬碟掃描與救援",
    source:"fixtures/images/ntfs_volume_extract.bin",output:"/Users/Shared/已救援",
    report:`NTFS Scan Report\nbytes_per_sector: 512\nsectors_per_cluster: 8\ncluster_size: 4096\ntotal_sectors: 320\ntotal_size_bytes: 163840\nmft_offset: 4096\nfile_record_size: 1024\nrecords_scanned: 16\nfiles: 12\ndeleted: 3\nextractable_files: 1`,
    vfs:`/root\tDirectory\tAllocated\tsize=-\tid=5\n/root/report.txt\tFile\tDeleted\tsize=20\tid=0\n/root/hello.bin\tFile\tAllocated\tsize=10240\tid=1\n/root/system.sys\tFile\tAllocated\tsize=40960\tid=2`,
    diskBar:`<div class="disk-seg boot">Boot (Sector 0)</div><div class="disk-seg mft">MFT Zone (Cluster 1)</div><div class="disk-seg data">Data (Cluster 2+)</div>`
  },
  exfat: {
    title:"隨身碟掃描與救援",
    source:"fixtures/images/exfat_image.img",output:"/Users/Shared/已救援",
    report:`exFAT Volume Report\nbytes_per_sector: 512\nsectors_per_cluster: 8\ncluster_size: 4096\nfat_offset: 32\nfat_length: 32\ncluster_heap_offset: 128\ncluster_count: 100\nrecords_scanned: 10\nfiles: 8\ndeleted: 2\nextractable_files: 8`,
    vfs:`/hello.txt\tFile\tAllocated\tsize=15\tid=65536\n/docs\tDirectory\tAllocated\tsize=-\tid=73728\n/docs/readme.md\tFile\tAllocated\tsize=40\tid=81920\n/deleted.bin\tFile\tDeleted\tsize=20000\tid=65600`,
    diskBar:`<div class="disk-seg boot">Boot & VBR</div><div class="disk-seg mft">FAT Table</div><div class="disk-seg data">Cluster Heap</div>`
  },
  fat32: {
    title:"FAT32 記憶卡救援",
    source:"fixtures/images/fat32_image.img",output:"/Users/Shared/已救援",
    report:`FAT32 Volume Report
bytes_per_sector: 512
sectors_per_cluster: 8
cluster_size: 4096
reserved_sectors: 32
fat_count: 2
fat_size: 128
root_cluster: 2
cluster_count: 2048
records_scanned: 12
files: 9
deleted: 3
extractable_files: 9
quality_note: FAT32 已刪除檔案若 FAT chain 被清除，會以 first_cluster 連續讀取作為保守 fallback；碎片化刪除檔仍可能只救回部分資料。`,
    vfs:`/DCIM	Directory	Allocated	size=-	id=65536
/DCIM/IMG_0001.JPG	File	Allocated	size=204800	id=65600
/DCIM/_MG_0002.JPG	File	Deleted	size=185300	id=65632	tier=medium	quality=FAT32 已刪除檔：可嘗試還原但需驗證內容
/DCIM/_MG_BAD.JPG	File	Deleted	size=185300	id=65648	tier=low	quality=suspect_partial：FAT chain 缺失，可能只剩部分資料
/PRIVATE	Directory	Allocated	size=-	id=69632
/PRIVATE/VIDEO.MOV	File	Deleted	size=5242880	id=69664	tier=low	quality=suspect_partial：影片大檔碎片化風險高，會隔離到 suspect/partial`,
    diskBar:`<div class="disk-seg boot">Boot Sector</div><div class="disk-seg mft">FAT 1 / FAT 2</div><div class="disk-seg data">Data Clusters</div>`
  },
  apfs: {
    title:"Mac 磁碟掃描與救援",
    source:"fixtures/images/apfs_container.img",output:"/Users/Shared/已救援",
    report:`APFS Container Superblock
block_size: 4096
block_count: 244140625
checksum_status: OK (Fletcher64 Match)
spaceman_oid: 1005
omap_oid: 1010
reaper_oid: 1015
max_file_systems: 2
entries_read: 4
volume_0_oid: 1020 role=1 root_tree_oid=2050 meta_crypto=0 encrypted_hint=false
extract_note: v1.2 已接上未加密 APFS extent 提取；FileVault/加密、壓縮、clone/snapshot-only 與缺失 extent 會改列 suspect/partial。
encrypted_hint: false`,
    vfs:`/Users	Directory	Allocated	size=-	id=101
/Users/adam	Directory	Allocated	size=-	id=102
/Users/adam/Desktop	Directory	Allocated	size=-	id=103
/Users/adam/Desktop/report.pdf	File	Allocated	size=245760	id=202	tier=medium	quality=APFS v1.2：未加密 extent 可嘗試提取，輸出前仍會驗證內容
/Users/adam/Pictures/photo.heic	File	Allocated	size=3840000	id=203	tier=medium	quality=APFS v1.2：若 HEIC 結構完整可輸出 recovered/，缺 extent 會進 suspect/partial
/Users/adam/Movies/large.mov	File	Allocated	size=524288000	id=204	tier=medium	quality=APFS 大檔：可能含 clone/壓縮/snapshot 風險，提取後依 bytes_lost 判定`,
    diskBar:`<div class="disk-seg mft disk-span-2">Container Superblock</div><div class="disk-seg boot">Object Map</div><div class="disk-seg data disk-span-3">Checkpoint / Volumes / Extents</div>`
  },
  carver: {
    title:"深度碎塊掃描 (Deep Scan)",
    source:"fixtures/images/damaged_disk.bin",output:"/Users/Shared/已救援",
    report:`Carver Scan Complete\ntotal_bytes_scanned: 1048576\nsignatures_detected: 5\npng_carved: 1\njpeg_carved: 1\npdf_carved: 1\nzip_carved: 1\nmp4_carved: 0\nheic_carved: 1\nconfidence_average: 0.91\nquality_note: v0.6 使用格式驗證加品質門檻，可搭配 tools/quality 產生成功率與垃圾檔比例報告。`,
    vfs:`/carved_100_108.png\tFile\tAllocated\tsize=108\tid=100\tconfidence=0.98\tquality=PNG chunk 與 CRC 驗證通過\ttier=high\n/carved_300_72.jpg\tFile\tAllocated\tsize=72\tid=300\tconfidence=0.94\tquality=JPEG marker、尺寸與 SOS/EOI 驗證通過\ttier=high\n/carved_500_12340.pdf\tFile\tAllocated\tsize=12340\tid=500\tconfidence=0.70\tquality=PDF EOF 與 startxref 存在，但交叉參照需修復\ttier=medium\n/carved_700_8800.docx\tFile\tAllocated\tsize=8800\tid=700\tconfidence=0.93\tquality=ZIP/Office 中央目錄與本機檔頭驗證通過\ttier=high\n/carved_20480_523040.heic\tFile\tAllocated\tsize=523040\tid=20480\tconfidence=0.88\tquality=HEIC/HEIF ftyp/meta box 結構通過\ttier=high`,
    diskBar:`<div class="disk-seg disk-span-3 disk-unknown">損毀分割區表 / 未知檔案系統</div><div class="disk-seg data disk-span-3">Raw Sectors（發現特徵碼）</div>`
  }
};

const modeMeta = {
  ntfs: { icon:'💾', badge:'Windows / NTFS', emptyTitle:'掃描硬碟，找回遺失檔案', desc:'適合一般 Windows 硬碟、SSD 或 NTFS 分割區。掃描後會列出檔案樹，讓你勾選要還原的項目。', advice:'適合 Windows 硬碟或 NTFS 分割區，建議先跑一般掃描；找不到再改用深度碎塊掃描。' },
  exfat:{ icon:'📁', badge:'隨身碟 / exFAT', emptyTitle:'掃描 exFAT 隨身碟', desc:'適合容量較大的 USB 隨身碟、外接硬碟、記憶卡等 exFAT 裝置。請把救回來的檔案存到電腦，不要存回同一支隨身碟。', advice:'適合 exFAT 隨身碟、記憶卡與外接儲存裝置。若檔案系統損毀嚴重，可改用深度碎塊掃描。' },
  fat32:{ icon:'🧩', badge:'記憶卡 / FAT32', emptyTitle:'掃描 FAT32 記憶卡或舊隨身碟', desc:'適合相機 SD 卡、舊隨身碟、車機記憶卡等 FAT32 裝置。會讀目錄 entry 與 FAT 表，比純深度掃描更不容易產生垃圾檔。', advice:'適合 FAT32 記憶卡、相機卡與舊隨身碟。刪除檔若 FAT chain 已被清除，會改用連續 cluster 保守還原；碎片化大檔仍可能部分損毀。' },
  apfs: { icon:'🍎', badge:'Mac / APFS', emptyTitle:'掃描 Mac 磁碟或 APFS 容器', desc:'適合 macOS 的 APFS 磁碟。v1.2 已接上未加密卷宗的 extent 提取；FileVault、壓縮、clone 或 snapshot-only 仍會被標記為可疑。', advice:'適合 Mac 內建或外接 APFS 磁碟，會優先讀取容器、卷宗與 extent。若疑似加密或 SSD/TRIM，建議先做映像檔並降低成功率預期。' },
  carver:{ icon:'🔍', badge:'深度掃描', emptyTitle:'深度搜尋散落的檔案碎塊', desc:'當磁碟格式看不出來、分割表毀損，或一般掃描找不到檔案時使用。v1.2 保留嚴格格式驗證、風險提示，並新增 APFS extent 提取路徑。', advice:'適合分割表毀損、格式未知或一般掃描失敗的狀況。為了減少垃圾檔，v1.2 會先提示深度掃描的假陽性風險，再檢查 PNG CRC、PDF startxref、ZIP/Office 中央目錄、MP4/MOV/HEIC box tree，並把低信心結果隔離顯示。' }
};

// ── State ──
let activeMode = "ntfs";
let activeFilter = "all";
let isGrid = false;
let wizStep = 1;
let allDisks = [];
let detectedPartitions = [];
let selectedPartitionOffset = 0;
let defaultRecoveryDir = '';
let progressListenersReady = false;
let hasTauriRuntime = false;
let operationInProgress = false;
let currentJobId = '';
let lastImagingPlan = null;
let lastSessionPlan = null;
let activeSession = null;
let outputIsolation = {pending:false, ok:false, message:'尚未完成來源／輸出 physical store 檢查。', sourceStores:[], outputStores:[]};
let outputIsolationTicket = 0;
let latestStructuredEntries = null;
let scanPageTicket = 0;
// 縮圖快取：Map<entryId, base64String | 'loading' | 'error'>
const thumbCache = new Map();
let thumbObserver = null;
const VIRTUAL_ROW_HEIGHT = 44;
const VIRTUAL_GRID_ROW_HEIGHT = 92;
const VIRTUAL_OVERSCAN = 8;
const visibleEntryById = new Map();
let visibleListEntries = [];
let visibleGridEntries = [];
const thumbnailQueue = [];
let thumbnailInFlight = 0;
const THUMBNAIL_CONCURRENCY = 4;
const state = { report:{}, entries:[], selected:null, selectedIds:new Set(), lastSelectedId:null, lastRecoveryReport:null };

// ── Helpers ──
const $ = id => document.getElementById(id);
const $$ = sel => Array.from(document.querySelectorAll(sel));
const getTauri = () => window.__TAURI__ || null;
const isRawDevicePath = value => /^\/dev\//.test(value) || /^\\\\\.\\/.test(value);
const scanButtons = () => [$('runRealScan'), $('runRealScanTop')].filter(Boolean);
const extractButtons = () => [$('runRealExtract'), $('ctaBtn')].filter(Boolean);
const supportedLocales = Object.keys(I18N);
let currentLocale = localStorage.getItem('polik_locale') || 'en';
if(!supportedLocales.includes(currentLocale)) currentLocale = 'en';

function i18nLookup(locale, path){
  return path.split('.').reduce((acc,key)=>acc && acc[key] !== undefined ? acc[key] : undefined, I18N[locale]);
}
function tr(path, fallback=''){
  const local = i18nLookup(currentLocale, path);
  if(local !== undefined) return local;
  const en = i18nLookup('en', path);
  return en !== undefined ? en : fallback;
}
function trObj(path){
  return i18nLookup(currentLocale, path) || i18nLookup('en', path) || {};
}
function trf(path, values={}, fallback=''){
  return String(tr(path, fallback)).replace(/\{(\w+)\}/g, (_,key)=>values[key] ?? '');
}
function setText(id, path, fallback=''){
  const el=$(id);
  if(el) el.textContent=tr(path, fallback || el.textContent);
}
function setHtml(id, path, fallback=''){
  const el=$(id);
  if(el) el.innerHTML=tr(path, fallback || el.innerHTML);
}
function setButtonList(ids, values){
  ids.forEach((id,idx)=>{ if($(id) && values[idx]) $(id).textContent=values[idx]; });
}
function applyMetricLabels(kind='default'){
  const labels = trObj(`metrics.${kind}`);
  const fallback = trObj('metrics.default');
  const v = Array.isArray(labels) ? labels : fallback;
  [['lbRec',0],['lbFil',1],['lbDel',2],['lbExt',3],['lbRptIn',4],['lbVfsIn',5]].forEach(([id,idx])=>{
    if($(id)) $(id).textContent = v[idx] || fallback[idx] || '';
  });
}
function applyI18n(){
  document.documentElement.lang=currentLocale;
  document.title=tr('appTitle','Polik Recovery');
  setText('brandLogo','brandLogo','P');
  setText('brandName','brandName','Polik Recovery');
  setText('brandSubtitle','brandSubtitle');
  setText('navLabel','navLabel');
  setText('mobileModeLabel','mobileModeLabel');
  setText('sampleLabel','sampleLabel');
  setText('loadSampleData','loadSample');
  setText('advancedModeLabel','advancedMode');
  setText('imageWorkflowDisclosure','imageWorkflowDisclosure');
  setText('sessionWorkflowDisclosure','sessionWorkflowDisclosure');
  setText('diagDisclosure','diagDisclosure');
  setText('sidebarNote','sidebarNote');
  setText('modeKicker','workbench');
  setText('runtimeBadge','runtimePreview');
  setText('parseInputs','reanalyze');
  setText('copyCommand','copyCommand');
  setText('runRealScanTop','startScan');
  setText('cancelOperationTop','stop');
  setText('runRealExtract','restoreSelected');
  setText('ctaBtn','restoreSelected');
  setText('wl1','steps.select');
  setText('wl2','steps.scanning');
  setText('wl3','steps.restore');
  setText('scanTitle','scanTitle');
  setHtml('scanSub','scanSub');
  setText('scanTxt','scanReading');
  setText('cancelOperationBtn','stopCurrent');
  setText('scanTip','scanTip');
  setText('safeReadTitle','safe.readTitle');
  setText('safeReadSub','safe.readSub');
  setText('safeSaveTitle','safe.saveTitle');
  setText('safeSaveSub','safe.saveSub');
  setText('safePickTitle','safe.pickTitle');
  setText('safePickSub','safe.pickSub');
  setText('safeAuthTitle','safe.authTitle');
  setText('safeAuthSub','safe.authSub');
  setText('safeImageTitle','safe.imageTitle');
  setText('safeImageSub','safe.imageSub');
  setText('modeAdviceTitle','recommendedFlow');
  setHtml('diskPickLabel','diskPick');
  const firstOpt=$('diskSel')?.querySelector('option[value=""]');
  if(firstOpt) firstOpt.textContent=tr('detecting');
  setText('sourcePathLabel','sourcePath');
  setText('ntfsProfileLabel','ntfsDepth');
  const ntfsOpts=tr('ntfsOptions',[]);
  if(Array.isArray(ntfsOpts)) Array.from($('ntfsScanProfile')?.options || []).forEach((opt,idx)=>{ if(ntfsOpts[idx]) opt.textContent=ntfsOpts[idx]; });
  setText('ntfsProfileHelp','ntfsHelp');
  setText('apfsScopeLabel','apfsScope');
  const apfsOpts=tr('apfsScopeOptions',[]);
  if(Array.isArray(apfsOpts)) Array.from($('apfsScanScope')?.options || []).forEach((opt,idx)=>{ if(apfsOpts[idx]) opt.textContent=apfsOpts[idx]; });
  setText('apfsScopeHelp','apfsScopeHelp');
  setHtml('outputPathLabel','outputPath');
  setText('browseOutput','choose');
  setText('outputPathHelp','outputHelp');
  setText('riskTitle','riskTitle');
  setText('riskScore','riskLow');
  setHtml('riskInitial','riskInitial');
  setText('riskRecommendation','riskRecommendation');
  setText('imageWorkflowTitle','imageWorkflowTitle');
  setText('imageWorkflowBadge','imageWorkflowBadge');
  setText('imageWorkflowSummary','imageWorkflowSummary');
  setText('imageWorkflowCommand','imageWorkflowCommand');
  setText('copyImageWorkflowCommand','copyImageWorkflowCommand');
  setText('refreshImageWorkflow','regenerate');
  setText('sessionWorkflowTitle','sessionWorkflowTitle');
  setText('sessionWorkflowBadge','sessionWorkflowBadge');
  setText('sessionWorkflowSummary','sessionWorkflowSummary');
  if($('activeSessionStatus')?.classList.contains('empty')) setText('activeSessionStatus','activeSessionStatus');
  setText('sessionWorkflowCommand','sessionWorkflowCommand');
  setButtonList(['createDesktopSession','openDesktopSession','useSessionOutput','refreshDesktopSessionIndex','runDesktopSessionDoctor','openDesktopSessionIndex','importSessionArtifact','generateSessionReport','openCustomerReport','openEngineerReport','generateDeliveryPackage','openDeliveryFolder','copySessionWorkflowCommand'], tr('sessionButtons',[]));
  setText('refreshSessionWorkflow','regenerate');
  setText('diagTitle','diagTitle');
  setText('diagVersion','notChecked');
  setText('refreshDiagnostics','checkAgain');
  setText('runRealScan','startSafeScan');
  setText('previewSampleBtn','previewSample');
  setText('emptyHint','emptyHint');
  setText('qualityNote','qualityNote');
  setText('foundFilesTitle','foundFiles');
  const filterLabels=tr('filters',[]);
  if(Array.isArray(filterLabels)) $$('.filter-bar .fb[data-f]').forEach((btn,idx)=>{ if(filterLabels[idx]) btn.textContent=filterLabels[idx]; });
  setText('selectAllFiles','selectUsable');
  setText('clearSelection','clear');
  setText('selectionMain','selectionMain');
  setText('selectionSub','selectionSub');
  const pf=$('pathFilter'); if(pf) pf.placeholder=tr('pathFilterPlaceholder','🔍 Filter by path');
  setText('clearPathFilter','clearFilter');
  setText('detailsTitle','details');
  setText('selStatus','notSelected');
  setText('advancedSettingsTitle','advancedSettings');
  setText('advancedNotice','advancedNotice');
  setText('generatedCommandLabel','generatedCommand');
  setText('recoveryReportTitle','recoveryReportTitle');
  setText('recoveryReportNote','recoveryReportNote');
  setText('openHtmlReport','openHtmlReport');
  setText('copyRecoverySummary','copySummary');
  setText('hideRecoveryReport','collapseReport');
  const lang=$('languageSelect');
  if(lang) lang.value=currentLocale;
  setButtonList(['btnModeNtfs','btnModeExfat','btnModeFat32','btnModeApfs','btnModeCarver'], ['ntfs','exfat','fat32','apfs','carver'].map(m=>tr(`modes.${m}.nav`)));
  const mobileMode=$('mobileModeSelect');
  if(mobileMode){
    ['ntfs','exfat','fat32','apfs','carver'].forEach((mode,idx)=>{
      const option=mobileMode.options[idx];
      if(option) option.textContent=tr(`modes.${mode}.nav`);
    });
    mobileMode.value=activeMode;
  }
  $$('.fb[data-f]').forEach(btn=>btn.setAttribute('aria-pressed', String(btn.classList.contains('act'))));
  setText('pathFilterLabel','pathFilterLabel','Filter by path');
  applyMetricLabels(activeMode==='carver'?'carver':activeMode==='apfs'?'apfs':'default');
}
function setLocale(locale){
  if(!supportedLocales.includes(locale)) locale='en';
  currentLocale=locale;
  localStorage.setItem('polik_locale', currentLocale);
  applyI18n();
  setModeConfig(activeMode,{loadSample:false});
  render();
}

const EXT_MAP = {
  image:['.png','.jpg','.jpeg','.gif','.bmp','.webp','.tiff','.heic','.svg'],
  doc:  ['.pdf','.doc','.docx','.txt','.md','.xls','.xlsx','.ppt','.pptx','.rtf','.csv','.odt'],
  video:['.mp4','.avi','.mov','.mkv','.wmv','.flv','.m4v','.webm'],
  audio:['.mp3','.wav','.flac','.aac','.m4a','.ogg','.wma']
};
const CAT_ICON = { image:'🖼', doc:'📄', video:'🎬', audio:'🎵', Directory:'📁', other:'📦' };

function getCat(path) {
  const ext = path.toLowerCase().slice(path.lastIndexOf('.'));
  for (const [c,exts] of Object.entries(EXT_MAP)) if (exts.includes(ext)) return c;
  return 'other';
}
function fname(path){ return path.split('/').pop() || path; }
function fext(path){ const n=fname(path);const d=n.lastIndexOf('.');return d<0?'':n.slice(d+1).toUpperCase().slice(0,4); }
function isSampleOutput(path){
  return !path || path.startsWith('/tmp/') || path.startsWith('/Users/Shared/已救援') || path.includes('/recovered_data');
}
function safeFilename(name, fallback){
  const clean = String(name || '').split('/').pop().replace(/[\\/:*?"<>|]/g,'_').trim();
  return clean || fallback;
}
function looksLikeFilePath(path){
  const base = fname(path || '');
  return /\.[A-Za-z0-9]{1,8}$/.test(base);
}
function normalizeOutputDir(path){
  return (path || defaultRecoveryDir || '/Users/Shared/已救援').replace(/\/+$/,'');
}
function outputDirProblem(){
  const raw=$('outputPath').value.trim();
  if(!raw) return '請先選擇還原檔案要存放的資料夾。';
  if(raw.startsWith('/dev/')) return '儲存位置不能是磁碟裝置，請選擇資料夾。';
  if(looksLikeFilePath(raw)) return '儲存位置必須是資料夾，不能是單一檔案路徑。請按「選擇」挑資料夾。';
  const source=$('sourcePath').value.trim();
  if(source && !source.startsWith('/dev/')){
    const sourceDir=source.split('/').slice(0,-1).join('/');
    if(sourceDir && raw.startsWith(sourceDir)) return '儲存資料夾看起來和來源映像檔在同一個位置，為避免覆蓋原資料，請改選其他磁碟或桌面資料夾。';
  }
  const isolationProblem=outputIsolationProblem();
  if(isolationProblem) return isolationProblem;
  return '';
}

function outputIsolationProblem(){
  if(!getTauri()) return '';
  const source=($('sourcePath')?.value || '').trim();
  const output=($('outputPath')?.value || '').trim();
  if(!source || isRawDevicePath(source) || !output) return '';
  if(outputIsolation.pending) return '正在檢查來源與輸出是否位於不同實體磁碟，完成前不能開始。';
  return outputIsolation.ok ? '' : (outputIsolation.message || '輸出位置尚未通過 physical store 隔離檢查。');
}

async function refreshOutputIsolation(){
  const core=getTauri()?.core;
  const source=($('sourcePath')?.value || '').trim();
  const output=($('outputPath')?.value || '').trim();
  const ticket=++outputIsolationTicket;
  if(!core || !source || isRawDevicePath(source) || !output){
    outputIsolation={pending:false,ok:false,message:'尚未完成來源／輸出 physical store 檢查。',sourceStores:[],outputStores:[]};
    updatePrimaryActions();
    return;
  }
  outputIsolation={...outputIsolation,pending:true,ok:false,message:'正在檢查來源與輸出是否位於不同實體磁碟。'};
  updatePrimaryActions();
  try{
    const result=await core.invoke('check_output_isolation',{sourcePath:source,outputDir:output});
    if(ticket!==outputIsolationTicket) return;
    outputIsolation={pending:false,ok:Boolean(result?.ok),message:result?.message || '輸出位置尚未通過 physical store 隔離檢查。',sourceStores:result?.sourceStores || [],outputStores:result?.outputStores || []};
    setAS(outputIsolation.message,outputIsolation.ok?'ok':'error');
  }catch(err){
    if(ticket!==outputIsolationTicket) return;
    outputIsolation={pending:false,ok:false,message:`輸出位置安全檢查失敗：${err}` ,sourceStores:[],outputStores:[]};
    setAS(outputIsolation.message,'error');
  }
  updatePrimaryActions();
  renderRiskPanel();
}

function selectedDiskInfo(){
  const val = $('diskSel')?.value || $('sourcePath')?.value || '';
  return allDisks.find(d=>d.path===val) || null;
}
function riskLevelFromScore(score){
  if(score >= 70) return 'high';
  if(score >= 38) return 'medium';
  return 'low';
}
function riskLevelLabel(level){
  const labels={low:['低風險','Low risk'],medium:['中等風險','Medium risk'],high:['高風險','High risk']};
  const pair=labels[level] || ['未知','Unknown'];
  return currentLocale==='zh-Hant' ? pair[0] : pair[1];
}
function riskIcon(level){
  return {low:'🟢',medium:'🟡',high:'🔴'}[level] || '⚪';
}
const RISK_EN = {
  '尚未選擇來源磁碟':'No source selected',
  '請先選擇要救援的磁碟或映像檔。':'Choose the disk image to scan first.',
  '實體磁碟掃描':'Physical-disk scan',
  '第一次讀取可能需要管理員授權；請確認不要把救回檔案存回同一顆磁碟。':'The first read may require authorization; never save recovered files back to the same disk.',
  '實體磁碟：請先建立映像檔':'Physical disk: create an image first',
  '映像檔掃描':'Image scan',
  '使用磁碟映像檔最安全，原始磁碟不會被二次寫入。':'A disk image is safest; the original disk is not read repeatedly.',
  '映像檔：較安全':'Image: safer',
  'SSD / TRIM 風險':'SSD / TRIM risk',
  'SSD 或 Mac 內建磁碟刪除後可能已被 TRIM 清掉；掃得到不代表資料區塊仍存在。':'SSD or an internal Mac disk may have discarded deleted blocks through TRIM; a scan cannot guarantee the data remains.',
  'SSD/TRIM：成功率可能偏低':'SSD/TRIM: success rate may be lower',
  '建議先建立 .img':'Create an .img first',
  'APFS v1.2 限制':'APFS limitations',
  '已支援未加密 APFS 的 extent 提取，但 FileVault/加密、壓縮檔、clone、snapshot-only 或 TRIM 後資料仍可能只進 suspect/partial。':'Unencrypted APFS extents are supported; FileVault, encryption, compression, clones, snapshots, and TRIM-affected data may remain suspect or partial.',
  'APFS：未加密 extent 可嘗試提取':'APFS: unencrypted extents can be attempted',
  '加密磁碟':'Encrypted disk',
  '疑似加密或 FileVault，未解鎖時深度掃描只會看到加密亂碼。':'The disk may be encrypted or FileVault-protected; without unlocking, deep scan sees encrypted bytes.',
  '疑似加密':'Possibly encrypted',
  '深度碎塊掃描':'Deep fragment scan',
  '適合格式損毀或找不到檔案系統；但碎片化大檔可能只能輸出 suspect/，垃圾比例會比檔案系統掃描高。':'Useful when the filesystem is damaged or unknown; fragmented files may be suspect and junk results are more likely.',
  'Deep Scan：假陽性較高':'Deep Scan: higher false-positive risk',
  '快速掃描範圍有限':'Quick scan has limited coverage',
  '只掃前 10,000 筆 MFT，速度快但可能漏掉大量檔案；救少時請改標準完整 MFT。':'Only the first 10,000 MFT records are scanned; it is faster but can miss files. Use the standard full MFT scan when recovery is sparse.',
  'NTFS 快速：可能漏救':'NTFS quick: may miss files',
  'NTFS 標準掃描':'NTFS standard scan',
  '會依 $MFT 大小估算掃描範圍，通常比固定筆數更不容易漏檔。':'The scan range is estimated from $MFT size, which is less likely to miss files than a fixed record count.',
  'NTFS 標準：建議':'NTFS standard: recommended',
  '隨身碟 / 記憶卡刪除檔':'Removable-media deleted files',
  '刪除後 FAT chain 可能被清掉；若只能連續讀取，程式會標記為可疑，不會假裝完整成功。':'The FAT chain may be cleared after deletion; contiguous-only recovery is marked suspect instead of being presented as complete.',
  '輸出位置需要調整':'Output location needs attention',
  '請先選擇還原檔案要存放的資料夾。':'Choose a folder for recovered files first.',
  '儲存位置不能是磁碟裝置，請選擇資料夾。':'The output cannot be a disk device; choose a folder.',
  '儲存位置必須是資料夾，不能是單一檔案路徑。請按「選擇」挑資料夾。':'The output must be a folder, not a file path. Use Choose to select a folder.',
  '儲存資料夾看起來和來源映像檔在同一個位置，為避免覆蓋原資料，請改選其他磁碟或桌面資料夾。':'The output appears to share a location with the source image. Choose another disk or a desktop folder to avoid overwriting source data.',
  '正在檢查來源與輸出是否位於不同實體磁碟，完成前不能開始。':'Checking whether source and output are on different physical disks; scanning is blocked until it finishes.',
  '輸出位置尚未通過 physical store 隔離檢查。':'The output has not passed the physical-store isolation check.',
  '輸出位置有風險':'Output location has a risk',
  '輸出位置通過基本檢查':'Output location passed the basic check',
  '目前看起來是資料夾路徑；仍建議使用另一顆外接硬碟或桌面資料夾。':'This looks like a folder path; another external disk or a desktop folder is still recommended.',
  '輸出資料夾 OK':'Output folder OK',
  '外接裝置':'External device',
  '來源是外接裝置，較容易避免系統背景寫入；掃描前請先停止繼續拍照、下載或複製檔案。':'The source is removable; stop taking photos, downloading, or copying files before scanning.',
  '內建磁碟':'Internal disk',
  '內建系統碟容易被背景程序寫入，刪除資料可能更快被覆蓋。':'Background processes may write to an internal system disk and overwrite deleted data sooner.',
  '內建磁碟：覆蓋風險較高':'Internal disk: higher overwrite risk',
  '先建 image 再掃描':'Create an image before scanning',
  '建議：救回檔案存到另一顆磁碟':'Recommendation: save recovered files to another disk'
};
function riskText(text){ return currentLocale==='zh-Hant' ? text : (RISK_EN[text] || text); }
function diskLooksLikeSsd(disk){
  const text = `${disk?.label||''} ${disk?.fs_type||''}`.toLowerCase();
  return !!disk?.solid_state || text.includes('ssd') || text.includes('solid state') || text.includes('apple ssd') || (!disk?.removable && activeMode==='apfs');
}
function diskLooksEncrypted(disk){
  const text = `${disk?.label||''} ${disk?.fs_type||''}`.toLowerCase();
  return !!disk?.encrypted || text.includes('encrypted') || text.includes('filevault') || text.includes('crypto') || text.includes('apfs encrypted');
}
function currentRiskAssessment(){
  const disk=selectedDiskInfo();
  const source=($('sourcePath')?.value || '').trim();
  const out=($('outputPath')?.value || '').trim();
  const rows=[];
  const chips=[];
  let score=10;
  const add=(severity, title, detail, points=0)=>{
    rows.push({severity,title,detail});
    score += points;
  };
  const addChip=(cls,text)=>chips.push({cls,text});

  if(!source){
    add('danger','尚未選擇來源磁碟','請先選擇要救援的磁碟或映像檔。',30);
  } else if(source.startsWith('/dev/')){
    add('warn','實體磁碟掃描','第一次讀取可能需要管理員授權；請確認不要把救回檔案存回同一顆磁碟。',10);
    addChip('warn','實體磁碟：請先建立映像檔');
  } else {
    add('good','映像檔掃描','使用磁碟映像檔最安全，原始磁碟不會被二次寫入。',0);
    addChip('good','映像檔：較安全');
  }

  if(diskLooksLikeSsd(disk)){
    add('danger','SSD / TRIM 風險','SSD 或 Mac 內建磁碟刪除後可能已被 TRIM 清掉；掃得到不代表資料區塊仍存在。',28);
    addChip('danger','SSD/TRIM：成功率可能偏低');
    addChip('warn','建議先建立 .img');
  }

  if(activeMode==='apfs'){
    add('warn','APFS v1.2 限制','已支援未加密 APFS 的 extent 提取，但 FileVault/加密、壓縮檔、clone、snapshot-only 或 TRIM 後資料仍可能只進 suspect/partial。',18);
    addChip('warn','APFS：未加密 extent 可嘗試提取');
  }
  if(diskLooksEncrypted(disk)){
    add('danger','加密磁碟','疑似加密或 FileVault，未解鎖時深度掃描只會看到加密亂碼。',35);
    addChip('danger','疑似加密');
  }
  if(activeMode==='carver'){
    add('warn','深度碎塊掃描','適合格式損毀或找不到檔案系統；但碎片化大檔可能只能輸出 suspect/，垃圾比例會比檔案系統掃描高。',18);
    addChip('warn','Deep Scan：假陽性較高');
  }
  if(activeMode==='ntfs' && $('ntfsScanProfile')?.value==='quick'){
    add('warn','快速掃描範圍有限','只掃前 10,000 筆 MFT，速度快但可能漏掉大量檔案；救少時請改標準完整 MFT。',12);
    addChip('warn','NTFS 快速：可能漏救');
  } else if(activeMode==='ntfs'){
    add('good','NTFS 標準掃描','會依 $MFT 大小估算掃描範圍，通常比固定筆數更不容易漏檔。',0);
    addChip('good','NTFS 標準：建議');
  }
  if(activeMode==='fat32' || activeMode==='exfat'){
    add('warn','隨身碟 / 記憶卡刪除檔','刪除後 FAT chain 可能被清掉；若只能連續讀取，程式會標記為可疑，不會假裝完整成功。',8);
  }

  const outProblem=outputDirProblem();
  if(outProblem){
    add('danger','輸出位置需要調整',outProblem,35);
    addChip('danger','輸出位置有風險');
  } else if(out){
    add('good','輸出位置通過基本檢查','目前看起來是資料夾路徑；仍建議使用另一顆外接硬碟或桌面資料夾。',0);
    addChip('good','輸出資料夾 OK');
  }

  if(disk && disk.removable){
    add('good','外接裝置','來源是外接裝置，較容易避免系統背景寫入；掃描前請先停止繼續拍照、下載或複製檔案。',0);
    addChip('good','外接裝置');
  } else if(disk && !disk.removable){
    add('warn','內建磁碟','內建系統碟容易被背景程序寫入，刪除資料可能更快被覆蓋。',15);
    addChip('warn','內建磁碟：覆蓋風險較高');
    addChip('warn','先建 image 再掃描');
  }

  const level=riskLevelFromScore(Math.min(score,100));
  return {level,score:Math.min(score,100),rows,chips};
}
function renderRiskPanel(){
  const panel=$('riskPanel');
  if(!panel) return;
  const risk=currentRiskAssessment();
  panel.className=`risk-panel ${risk.level}`;
  $('riskTitle').textContent=`${riskIcon(risk.level)} ${tr('riskCheckLabel','Pre-scan risk check')}`;
  $('riskScore').textContent=`${riskLevelLabel(risk.level)}｜${risk.score}/100`;
  $('riskBody').innerHTML=risk.rows.map(r=>{
    const icon=r.severity==='danger'?'⚠️':r.severity==='warn'?'🟡':'✅';
    const cls=r.severity==='danger'?'danger':r.severity==='warn'?'warn':'';
    return `<div class="risk-row ${cls}"><span>${icon}</span><span><strong>${esc(riskText(r.title))}。</strong> ${esc(riskText(r.detail))}</span></div>`;
  }).join('') || `<div class="risk-row"><span>✅</span><span><strong>${esc(tr('riskReady','Ready.'))}</strong></span></div>`;
  $('riskActions').innerHTML=(risk.chips.length?risk.chips:[{cls:'good',text:tr('riskRecommendation','Recommendation: save recovered files to another disk')}])
    .map(c=>`<span class="risk-chip ${c.cls}">${esc(riskText(c.text))}</span>`).join('');
}

function imageOutputDir(){
  const out = normalizeOutputDir($('outputPath')?.value || defaultRecoveryDir || '/Users/Shared/已救援');
  return `${out}/source-images`;
}
function sessionRootDir(){
  if(activeSession?.sessionDir){
    const parts=String(activeSession.sessionDir).split('/');
    parts.pop();
    return parts.join('/') || String(activeSession.sessionDir);
  }
  const out = normalizeOutputDir($('outputPath')?.value || defaultRecoveryDir || '/Users/Shared/已救援');
  if(out.endsWith('/03_recovery_output')){
    const parts=out.split('/');
    parts.pop();
    return parts.join('/');
  }
  return `${out}/sessions`;
}
function localImagingPlan(){
  const source = ($('sourcePath')?.value || '').trim();
  const out = imageOutputDir();
  const slug = (source || 'source').replace(/^\/dev\//,'').replace(/[^A-Za-z0-9._-]+/g,'_') || 'source';
  const prefix = `polik-image-${slug}-<timestamp>`;
  return {
    recommended: source.startsWith('/dev/'),
    source,
    outputDir: out,
    imagePath: `${out}/${prefix}.img`,
    manifestPath: `${out}/${prefix}.manifest.json`,
    createCommand: `python3 tools/macos/create_source_image.py --source '${source || '/dev/diskX'}' --out-dir '${out}'`,
    verifyCommand: `python3 tools/macos/verify_source_image.py --manifest '${out}/${prefix}.manifest.json'`,
    mapReportCommand: `python3 tools/macos/ddrescue_map_report.py --manifest '${out}/${prefix}.manifest.json'`,
    resumeCommand: `python3 tools/macos/create_source_image.py --resume-manifest '${out}/${prefix}.manifest.json' --method ddrescue`,
    retryCommand: `python3 tools/macos/create_source_image.py --resume-manifest '${out}/${prefix}.manifest.json' --method ddrescue --retry-bad --retries 3`,
    retryReverseCommand: `python3 tools/macos/create_source_image.py --resume-manifest '${out}/${prefix}.manifest.json' --method ddrescue --retry-bad --reverse --retries 3`,
    scanNextStep: `建立並驗證完成後，把來源改成 ${out}/${prefix}.img 再掃描。`,
    warnings: [source.startsWith('/dev/') ? '來源是實體磁碟：建議先建立映像檔。' : '來源看起來已是映像檔：可以直接掃描。'],
    steps: ['停止使用來源磁碟', '把映像檔存到另一顆磁碟', '查看 ddrescue map 報告', '必要時續跑或 retry', '驗證後掃描 .img']
  };
}
async function refreshImageWorkflowPlan(){
  const panel=$('imageWorkflowPanel');
  if(!panel) return;
  const source=($('sourcePath')?.value || '').trim();
  if(!source){
    $('imageWorkflowBadge').textContent='等待來源';
    $('imageWorkflowSummary').textContent='選擇來源磁碟後，這裡會產生「先建立映像檔，再掃描映像檔」的建議流程。';
    $('imageWorkflowCommand').textContent='尚未選擇來源。';
    return;
  }
  const core=getTauri()?.core;
  let plan=null;
  if(core){
    try{ plan=await core.invoke('get_imaging_workflow_plan',{sourcePath:source,outputDir:imageOutputDir()}); }catch(_){ plan=null; }
  }
  lastImagingPlan = plan || localImagingPlan();
  $('imageWorkflowBadge').textContent = lastImagingPlan.recommended ? '強烈建議' : '可直接掃描';
  $('imageWorkflowSummary').innerHTML = `${lastImagingPlan.recommended ? '來源是實體磁碟，建議先做 image，並用 ddrescue map 看壞區/未完成比例。' : '來源看起來已是映像檔/一般檔案，可直接掃描；仍請保留原檔。'}<br>${esc(lastImagingPlan.scanNextStep || '')}`;
  const commands = [
    '# 1. 建立或續跑來源映像檔',
    lastImagingPlan.createCommand,
    '# 2. 產生 ddrescue map 視覺化報告',
    lastImagingPlan.mapReportCommand,
    '# 3. 驗證映像檔',
    lastImagingPlan.verifyCommand,
    '# 4. 若 map 顯示未完成，可先續跑',
    lastImagingPlan.resumeCommand,
    '# 5. 若 map 顯示壞區，可有限次 retry；不要無限重試折磨原碟',
    lastImagingPlan.retryCommand,
    lastImagingPlan.retryReverseCommand
  ].filter(Boolean);
  $('imageWorkflowCommand').textContent = commands.join('\n');
}
async function copyImageWorkflowCommand(){
  const text=$('imageWorkflowCommand')?.textContent || '';
  try{
    if(navigator.clipboard?.writeText) await navigator.clipboard.writeText(text);
    else{const t=document.createElement('textarea');t.value=text;t.style.cssText='position:fixed;left:-9999px;';document.body.appendChild(t);t.focus();t.select();document.execCommand('copy');t.remove();}
    setAS('已複製成像、map、驗證與 retry 指令。','ok');
  }catch(err){ setAS(`複製成像 / map / retry 指令失敗：${err}`,'error'); }
}

function localSessionPlan(){
  const source = ($('sourcePath')?.value || '').trim();
  const root = sessionRootDir();
  const slug = (source || 'source').replace(/^\/dev\//,'').replace(/[^A-Za-z0-9._-]+/g,'_') || 'source';
  const caseId = `polik-session-${slug}-<timestamp>`;
  const sessionDir = `${root}/${caseId}`;
  return {
    recommended:true,
    caseId,
    sessionRoot:root,
    sessionDir,
    createCommand:`python3 tools/session/create_recovery_session.py --source '${source || '/dev/diskX'}' --mode '${activeMode || 'unknown'}' --out-root '${root}'`,
    indexCommand:`python3 tools/session/create_recovery_session.py --session '${sessionDir}'`,
    doctorCommand:`python3 tools/session/session_doctor.py --session '${sessionDir}'`,
    imageManifestTarget:`${sessionDir}/01_source_image/`,
    scanReportTarget:`${sessionDir}/02_scan_reports/`,
    recoveryOutputDir:`${sessionDir}/03_recovery_output`,
    handoffDir:`${sessionDir}/06_handoff/`,
    nextStep:`建立 session 後，建議把還原輸出資料夾改成 ${sessionDir}/03_recovery_output`,
    warnings:['建立案件資料夾後，image、map、scan、recovery、quality、log 都放同一個 session。'],
    steps:['建立 session','來源若是實體磁碟，先做 image','掃描 image 並保存報告','還原到 03_recovery_output','重新產生 session_index.html']
  };
}
async function refreshSessionWorkflowPlan(){
  const panel=$('sessionWorkflowPanel');
  if(!panel) return;
  const source=($('sourcePath')?.value || '').trim();
  const core=getTauri()?.core;
  let plan=null;
  if(core){
    try{ plan=await core.invoke('get_session_workflow_plan',{sourcePath:source,outputDir:sessionRootDir(),mode:activeMode}); }catch(_){ plan=null; }
  }
  lastSessionPlan = plan || localSessionPlan();
  $('sessionWorkflowBadge').textContent = '案件資料夾';
  $('sessionWorkflowSummary').innerHTML = `${esc(lastSessionPlan.nextStep || '建立 session 後，將本次救援所有報告與輸出集中保存。')}<br>建議輸出：<code>${esc(lastSessionPlan.recoveryOutputDir || '')}</code>`;
  const commands=[
    '# 1. 建立救援 session / 案件資料夾',
    lastSessionPlan.createCommand,
    '# 2. 成像完成後，把 manifest/map/scan/recovery/quality/log 匯入對應資料夾',
    `# image manifest → ${lastSessionPlan.imageManifestTarget}`,
    `# scan report    → ${lastSessionPlan.scanReportTarget}`,
    `# recovery output → ${lastSessionPlan.recoveryOutputDir}`,
    '# 3. 匯入外部報告 / artifacts（範例）',
    `python3 tools/session/import_session_artifacts.py --session '${lastSessionPlan.sessionDir}' --kind auto <report-or-log-path>`,
    '# 4. 重新產生 session_index.html',
    lastSessionPlan.indexCommand,
    '# 5. 檢查 session 是否完整',
    lastSessionPlan.doctorCommand
  ].filter(Boolean);
  $('sessionWorkflowCommand').textContent = commands.join('\n');
}
async function copySessionWorkflowCommand(){
  const text=$('sessionWorkflowCommand')?.textContent || '';
  try{
    if(navigator.clipboard?.writeText) await navigator.clipboard.writeText(text);
    else{const t=document.createElement('textarea');t.value=text;t.style.cssText='position:fixed;left:-9999px;';document.body.appendChild(t);t.focus();t.select();document.execCommand('copy');t.remove();}
    setAS('已複製 Session 建立、索引與檢查指令。','ok');
  }catch(err){ setAS(`複製 Session 指令失敗：${err}`,'error'); }
}

function renderActiveSession(extra=''){
  const box=$('activeSessionStatus');
  if(!box) return;
  if(!activeSession){
    box.className='session-status empty';
    box.innerHTML='尚未啟用 Session。可先按「直接建立 Session」，App 會自動把還原輸出指向案件資料夾。';
    return;
  }
  box.className='session-status';
  const warn=(activeSession.warnings||[]).slice(0,2).map(w=>`<div>🟡 ${esc(w)}</div>`).join('');
  box.innerHTML=`<strong>✅ 已啟用 Session：${esc(activeSession.caseId||'未命名案件')}</strong>
    <div class="session-grid">
      <div>案件資料夾：<code>${esc(activeSession.sessionDir||'')}</code></div>
      <div>還原輸出：<code>${esc(activeSession.recoveryOutputDir||'')}</code></div>
      <div>完整檔案：<code>${esc(activeSession.recoveredDir||'')}</code></div>
      <div>可疑檔案：<code>${esc(activeSession.suspectDir||'')}</code></div>
      ${activeSession.artifactCount!=null?`<div>Artifacts：<code>${esc(String(activeSession.artifactCount))}</code></div>`:''}
      ${activeSession.customerReportHtml?`<div>客戶版報告：<code>${esc(activeSession.customerReportHtml)}</code></div>`:''}
      ${activeSession.engineerReportHtml?`<div>工程交接版：<code>${esc(activeSession.engineerReportHtml)}</code></div>`:''}
      ${warn}
      ${extra?`<div class="session-checks">${esc(extra)}</div>`:''}
    </div>`;
}
function applySession(session, updateOutput=true){
  activeSession=session;
  if(session?.recoveryOutputDir && updateOutput){
    $('outputPath').value=session.recoveryOutputDir;
  }
  renderActiveSession();
  renderRiskPanel();
  renderCommand();
  refreshImageWorkflowPlan();
  refreshSessionWorkflowPlan();
}
async function createDesktopSession(){
  const core=getTauri()?.core;
  if(!core){ await copySessionWorkflowCommand(); return; }
  const source=($('sourcePath')?.value||'').trim();
  const root=sessionRootDir();
  try{
    setAS('正在建立 Session 案件資料夾...');
    const session=await core.invoke('create_recovery_session_direct',{sourcePath:source,mode:activeMode,outputRoot:root,caseId:null});
    applySession(session,true);
    setAS(`已建立 Session：${session.caseId}，還原輸出已切到案件資料夾。`,'ok');
  }catch(err){ showFriendlyError('建立 Session 失敗', err, 'Session'); }
}
async function openDesktopSession(){
  const core=getTauri()?.core;
  const def=activeSession?.sessionDir || lastSessionPlan?.sessionDir || sessionRootDir();
  const dir=await requestAppInput(tr('sessionPathLabel','Session folder path'), tr('sessionPathPrompt','Paste the existing Session folder path.'), def, {label:tr('sessionPathLabel','Session folder path')});
  if(!dir) return;
  if(!core){ setAS('目前不是 Tauri App 模式，請使用複製指令方式開啟 Session。','error'); return; }
  try{
    const session=await core.invoke('open_recovery_session_direct',{sessionDir:dir.trim()});
    applySession(session,true);
    setAS(`已開啟 Session：${session.caseId}，還原輸出已切到案件資料夾。`,'ok');
  }catch(err){ showFriendlyError('開啟 Session 失敗', err, 'Session'); }
}
function useSessionOutput(){
  if(!activeSession?.recoveryOutputDir){ setAS('尚未啟用 Session。請先建立或開啟 Session。','error'); return; }
  $('outputPath').value=activeSession.recoveryOutputDir;
  renderRiskPanel(); renderCommand(); refreshImageWorkflowPlan(); refreshSessionWorkflowPlan();
  setAS('已將還原輸出資料夾切到目前 Session 的 03_recovery_output/。','ok');
}
async function refreshDesktopSessionIndex(){
  if(!activeSession?.sessionDir){ setAS('尚未啟用 Session。','error'); return; }
  const core=getTauri()?.core;
  if(!core){ setAS('目前不是 Tauri App 模式，請使用 session-index 指令。','error'); return; }
  try{
    const session=await core.invoke('refresh_recovery_session_index',{sessionDir:activeSession.sessionDir});
    applySession(session,false);
    setAS('已重新產生 session_index.html。','ok');
  }catch(err){ showFriendlyError('更新 Session 索引失敗', err, 'Session'); }
}
async function runDesktopSessionDoctor(){
  if(!activeSession?.sessionDir){ setAS('尚未啟用 Session。','error'); return; }
  const core=getTauri()?.core;
  if(!core){ setAS('目前不是 Tauri App 模式，請使用 session-doctor 指令。','error'); return; }
  try{
    const report=await core.invoke('get_session_doctor_report_direct',{sessionDir:activeSession.sessionDir});
    const checks=(report.checks||[]).map(c=>`${c.ok?'✅':'❌'} ${c.id} — ${c.detail}`).join('\n');
    const warnings=(report.warnings||[]).map(w=>`🟡 ${w}`).join('\n');
    renderActiveSession(`${report.ok?'PASS':'需要處理'}\n${checks}${warnings?'\n\n'+warnings:''}`);
    setAS(report.ok?'Session 檢查通過。':'Session 檢查完成，有項目需要處理。', report.ok?'ok':'error');
  }catch(err){ showFriendlyError('Session 檢查失敗', err, 'Session'); }
}
async function openDesktopSessionIndex(){
  if(!activeSession?.sessionDir){ setAS('尚未啟用 Session。','error'); return; }
  const core=getTauri()?.core;
  if(!core){ setAS('目前不是 Tauri App 模式，請手動開啟 session_index.html。','error'); return; }
  try{
    await core.invoke('open_session_index_direct',{sessionDir:activeSession.sessionDir});
    setAS('已要求系統開啟 session_index.html。','ok');
  }catch(err){ showFriendlyError('開啟 Session 索引失敗', err, 'Session'); }
}


async function importSessionArtifact(){
  if(!activeSession?.sessionDir){ setAS('尚未啟用 Session。請先建立或開啟 Session。','error'); return; }
  const core=getTauri()?.core;
  const source=await requestAppInput(tr('artifactPathLabel','Artifact file or folder path'), tr('artifactPathPrompt','Paste the file or folder to import.\nExamples: image manifest, ddrescue .map, scan report, recovery_report.html, quality_report.json, log, or artifacts zip.'), '', {label:tr('artifactPathLabel','Artifact file or folder path')});
  if(!source || !source.trim()) return;
  const kind=await requestAppInput(tr('artifactKindLabel','Artifact type'), `${tr('artifactKindPrompt','Use auto or choose a known artifact type.')}\nauto / source_image / ddrescue_map / scan_report / recovery_report / quality_report / log / handoff / intake`, 'auto', {label:tr('artifactKindLabel','Artifact type')}) || 'auto';
  if(!core){
    const cmd=`python3 tools/session/import_session_artifacts.py --session '${activeSession.sessionDir}' --kind '${kind}' '${source.trim()}'`;
    try{ if(navigator.clipboard?.writeText) await navigator.clipboard.writeText(cmd); }catch(_){ }
    setAS('目前不是 Tauri App 模式，已產生匯入指令，可在終端機或 Codex 執行。','ok');
    renderActiveSession(cmd);
    return;
  }
  try{
    setAS('正在匯入 artifact 到目前 Session...');
    const result=await core.invoke('import_session_artifact_direct',{sessionDir:activeSession.sessionDir,sourcePath:source.trim(),artifactKind:kind.trim()||'auto'});
    const refreshed=await core.invoke('refresh_recovery_session_index',{sessionDir:activeSession.sessionDir});
    applySession(refreshed,false);
    setAS(`已匯入 ${result.artifactKind} → ${result.targetFolder}`,'ok');
    renderActiveSession(`已匯入：${result.artifactKind}
來源：${result.sourcePath}
目標：${result.destinationPath}`);
  }catch(err){ showFriendlyError('匯入 Session Artifact 失敗', err, 'Session artifact'); }
}

async function generateSessionReport(){
  if(!activeSession?.sessionDir){ setAS('尚未啟用 Session。請先建立或開啟 Session。','error'); return; }
  const core=getTauri()?.core;
  const fallback=`python3 tools/session/session_handoff_report.py --session '${activeSession.sessionDir}' --audience both`;
  if(!core){ await navigator.clipboard?.writeText(fallback).catch(()=>{}); renderActiveSession(fallback); setAS('目前不是 Tauri App 模式，已顯示/複製產生交接報告指令。','ok'); return; }
  try{
    setAS('正在產生 v2.5 客戶版摘要與工程交接報告...');
    const result=await core.invoke('generate_session_handoff_reports_direct',{sessionDir:activeSession.sessionDir});
    const refreshed=await core.invoke('refresh_recovery_session_index',{sessionDir:activeSession.sessionDir});
    applySession(refreshed,false);
    renderActiveSession(`已產生交接報告\n客戶版：${result.customerHtml}\n工程版：${result.engineerHtml}`);
    setAS('已產生 v2.5 交接摘要 / 客戶版報告。','ok');
  }catch(err){ showFriendlyError('產生 Session 交接報告失敗', err, 'Session reports'); }
}
async function openCustomerReport(){
  if(!activeSession?.sessionDir){ setAS('尚未啟用 Session。','error'); return; }
  const core=getTauri()?.core;
  if(!core){ renderActiveSession(`客戶版報告：${activeSession.customerReportHtml||activeSession.sessionDir+'/09_reports/customer_summary.html'}`); return; }
  try{ await core.invoke('open_session_customer_report_direct',{sessionDir:activeSession.sessionDir}); }catch(err){ showFriendlyError('開啟客戶版報告失敗', err, 'Session reports'); }
}
async function openEngineerReport(){
  if(!activeSession?.sessionDir){ setAS('尚未啟用 Session。','error'); return; }
  const core=getTauri()?.core;
  if(!core){ renderActiveSession(`工程交接報告：${activeSession.engineerReportHtml||activeSession.sessionDir+'/09_reports/engineer_handoff.html'}`); return; }
  try{ await core.invoke('open_session_engineer_report_direct',{sessionDir:activeSession.sessionDir}); }catch(err){ showFriendlyError('開啟工程交接報告失敗', err, 'Session reports'); }
}
async function generateDeliveryPackage(){
  if(!activeSession?.sessionDir){ setAS('尚未啟用 Session。請先建立或開啟 Session。','error'); return; }
  const includeSuspect = await requestAppConfirmation(`客戶交付 ZIP 預設只包含 recovered/ 完整區。\n\n是否也納入 suspect/ 可疑與 partial 檔案？\n\n一般交付建議選「取消」，除非客戶明確需要可疑殘檔。`, '確認交付內容');
  const cmd=`python3 tools/session/session_delivery_package.py --session ${shellQ(activeSession.sessionDir)}${includeSuspect?' --include-all-suspect':''}`;
  try{ await navigator.clipboard?.writeText(cmd); }catch(_){ }
  renderActiveSession(`${cmd}

執行後客戶交付包會在：
${activeSession.sessionDir}/11_delivery/packages/

此 ZIP 預設排除 raw image、ddrescue map、工程 log、工程版報告與敏感路徑。`);
  setAS('已產生 v2.7 客戶交付 ZIP 指令並嘗試複製到剪貼簿。請在 macOS/Codex 終端機執行。','ok');
}
async function openDeliveryFolder(){
  if(!activeSession?.sessionDir){ setAS('尚未啟用 Session。','error'); return; }
  const core=getTauri()?.core;
  const target=`${activeSession.sessionDir}/11_delivery/packages`;
  if(!core){ renderActiveSession(`交付資料夾：${target}`); return; }
  try{ await core.invoke('open_report_file',{path:target}); setAS('已要求系統開啟客戶交付包資料夾。','ok'); }
  catch(err){ showFriendlyError('開啟交付資料夾失敗', err, 'Session delivery'); }
}
function showAppDialog(title, message, options={}){
  const {confirm=false, confirmLabel=tr('confirm','Confirm'), cancelLabel=tr('cancel','Cancel'), input=null}=options;
  return new Promise(resolve=>{
    const previousFocus=document.activeElement instanceof HTMLElement ? document.activeElement : null;
    const suffix=`${Date.now()}-${Math.random().toString(16).slice(2)}`;
    const titleId=`appDialogTitle-${suffix}`;
    const messageId=`appDialogMessage-${suffix}`;
    const inputId=`appDialogInput-${suffix}`;
    const backdrop=document.createElement('div');
    backdrop.className='app-dialog-backdrop';
    backdrop.innerHTML=`<div class="app-dialog" role="dialog" aria-modal="true" aria-labelledby="${titleId}" aria-describedby="${messageId}">
      <div id="${titleId}" class="app-dialog-title">${esc(title)}</div>
      <div id="${messageId}" class="app-dialog-message">${esc(message)}</div>
      ${input?`<label class="app-dialog-label" for="${inputId}">${esc(input.label||'Value')}</label><input id="${inputId}" class="app-dialog-input" type="${esc(input.type||'text')}" value="${esc(input.value||'')}" placeholder="${esc(input.placeholder||'')}" autocomplete="off">`:''}
      <div class="app-dialog-actions">
        <button type="button" class="ghost" data-dialog-cancel>${esc(confirm ? cancelLabel : tr('close','Close'))}</button>
        ${confirm?`<button type="button" class="primary" data-dialog-confirm>${esc(confirmLabel)}</button>`:''}
      </div>
    </div>`;
    document.body.appendChild(backdrop);
    const dialog=backdrop.querySelector('.app-dialog');
    const field=input?backdrop.querySelector('.app-dialog-input'):null;
    let closed=false;
    const finish=value=>{
      if(closed) return;
      closed=true;
      backdrop.removeEventListener('keydown',onKeyDown);
      backdrop.remove();
      previousFocus?.focus?.({preventScroll:true});
      resolve(value);
    };
    const onKeyDown=event=>{
      if(event.key==='Escape'){ event.preventDefault(); finish(input?null:false); return; }
      if(event.key==='Enter' && confirm && (event.target===field || event.target?.matches?.('[data-dialog-confirm]'))){
        event.preventDefault();
        finish(input ? field.value : true);
        return;
      }
      if(event.key!=='Tab') return;
      const focusable=[...dialog.querySelectorAll('button,input,textarea,select,[href],[tabindex]:not([tabindex="-1"])')].filter(el=>!el.disabled);
      if(!focusable.length) return;
      const first=focusable[0], last=focusable[focusable.length-1];
      if(event.shiftKey && document.activeElement===first){ event.preventDefault(); last.focus(); }
      else if(!event.shiftKey && document.activeElement===last){ event.preventDefault(); first.focus(); }
    };
    backdrop.addEventListener('keydown',onKeyDown);
    backdrop.querySelector('[data-dialog-cancel]').addEventListener('click',()=>finish(input?null:false));
    backdrop.querySelector('[data-dialog-confirm]')?.addEventListener('click',()=>finish(input ? field.value : true));
    backdrop.addEventListener('click',event=>{ if(event.target===backdrop) finish(input?null:false); });
    (field || backdrop.querySelector(confirm?'[data-dialog-confirm]':'[data-dialog-cancel]'))?.focus();
    field?.select();
  });
}
function showAppNotice(title, message){ return showAppDialog(title, message); }
function requestAppConfirmation(message, title='請確認'){ return showAppDialog(title, message, {confirm:true}); }
function requestAppInput(title, message, value='', options={}){
  return showAppDialog(title, message, {confirm:true, input:{value, ...options}});
}
async function confirmRiskBeforeScan(){
  const risk=currentRiskAssessment();
  const hardProblem=outputDirProblem();
  if(hardProblem){ await showAppNotice('掃描前需要處理', '⚠️ 掃描前請先處理：\n'+hardProblem); return false; }
  if(risk.level==='high'){
    const top=risk.rows.filter(r=>r.severity==='danger').slice(0,4).map(r=>`・${r.title}：${r.detail}`).join('\n');
    return requestAppConfirmation(`這次救援判定為高風險（${risk.score}/100）。\n\n${top}\n\n建議先停止使用來源磁碟，最好先做映像檔再掃。仍要開始掃描嗎？`, '高風險掃描確認');
  }
  return true;
}
function buildOutputPath(basePath, entry){
  const base = normalizeOutputDir(basePath);
  const id = entry?.id || Date.now();
  const sourceName = safeFilename(entry?.path, `recovered_${id}.bin`);
  return `${base}/recovered/${sourceName}`;
}
function esc(v){ return String(v).replace(/[&<>"']/g,c=>({'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;',"'":'&#39;'})[c]); }

function parseFriendlyError(raw, context='操作'){
  const text=String(raw||'未知錯誤');
  try{
    const parsed=JSON.parse(text);
    if(parsed&&parsed.code){
      const reason=parsed.messageZh||parsed.message_zh||'程式在這一步遇到錯誤。';
      return {code:String(parsed.code),reason,action:'請依照詳細資訊處理；若是真實磁碟，建議先建立映像檔再掃描。',target:parsed.target||$('sourcePath')?.value||'-',tech:parsed.detail||text,context,text};
    }
  }catch(_){ /* legacy plain-text error */ }
  const get=(label)=>{
    const m=text.match(new RegExp(label+'：([^\n]+)'));
    return m?m[1].trim():'';
  };
  const code=get('錯誤代碼') || 'RECOVERY_ERROR';
  const reason=get('可能原因') || '程式在這一步遇到錯誤。';
  const action=get('建議處理') || '請確認來源磁碟、輸出資料夾與權限；若是真實磁碟，建議先建立映像檔再掃描。';
  const target=get('目標') || $('sourcePath')?.value || '-';
  const tech=get('技術訊息') || text;
  return {code,reason,action,target,tech,context,text};
}
function showFriendlyError(title, raw, context='操作'){
  const e=parseFriendlyError(raw, context);
  const drawer=$('errorDrawer');
  if(!drawer){ void showAppNotice(title, String(raw)); return; }
  $('errorTitle').textContent=`❌ ${title}`;
  $('errorSub').textContent=`${e.code}｜${e.context}`;
  $('errorBody').innerHTML=`
    <div class="error-block"><label>可能原因</label><div>${esc(e.reason)}</div></div>
    <div class="error-block"><label>建議處理</label><div>${esc(e.action)}</div></div>
    <div class="error-block"><label>目標</label><div>${esc(e.target)}</div></div>
    <div class="error-block"><label>技術訊息</label><pre>${esc(e.tech)}</pre></div>`;
  drawer.dataset.copyText=`${title}\n錯誤代碼：${e.code}\n可能原因：${e.reason}\n建議處理：${e.action}\n目標：${e.target}\n技術訊息：${e.tech}`;
  drawer.classList.add('show');
  setAS(`${title}：${e.code}，請看右下角處理建議。`,'error');
}
function hideFriendlyError(){ $('errorDrawer')?.classList.remove('show'); }
async function copyErrorText(){
  const txt=$('errorDrawer')?.dataset.copyText||'';
  try{ await navigator.clipboard.writeText(txt); setAS('錯誤內容已複製。','ok'); }
  catch(_){ setAS('無法使用剪貼簿，請手動複製錯誤內容。','error'); }
}
function diagStatusIcon(s){ return s==='ok'?'✅':s==='error'?'❌':'🟡'; }
async function refreshDiagnostics(){
  const core=getTauri()?.core;
  const panel=$('diagPanel');
  if(!core||!panel) return;
  panel.classList.add('show');
  $('diagVersion').textContent='檢查中...';
  $('diagList').innerHTML='<div class="diag-item warn"><strong>正在檢查實機環境...</strong><p>正在確認 App 路徑、輸出權限、diskutil 與 hdiutil。</p></div>';
  try{
    const res=await core.invoke('get_runtime_diagnostics');
    $('diagVersion').textContent=`${res.platform||'unknown'}｜${res.appVersion||''}`;
    $('diagList').innerHTML=(res.checks||[]).map(c=>`<div class="diag-item ${esc(c.status||'warn')}"><strong>${diagStatusIcon(c.status)} ${esc(c.title)}</strong><p>${esc(c.detail)}</p><p><b>處理：</b>${esc(c.action)}</p></div>`).join('') || '<div class="diag-item warn"><strong>沒有取得檢查結果</strong><p>請改用 make dev-doctor。</p></div>';
  }catch(err){
    $('diagVersion').textContent='檢查失敗';
    $('diagList').innerHTML=`<div class="diag-item error"><strong>❌ 無法執行環境檢查</strong><p>${esc(err)}</p></div>`;
  }
}
function fmtBytes(v){
  if(!Number.isFinite(v)) return '-';
  if(v<1024) return `${v} B`;
  const u=['KB','MB','GB','TB'];let s=v/1024,i=0;
  while(s>=1024&&i<u.length-1){s/=1024;i++;}
  return `${s.toFixed(s>=10?1:2)} ${u[i]}`;
}
function shellQ(v){ return /^[A-Za-z0-9_./:-]+$/.test(v)?v:`'${v.replace(/'/g,"'\\''")}'`; }
function stLbl(s){ return s.toLowerCase()==='deleted'?tr('status.deleted'):tr('status.normal'); }
function stCls(s){ return s.toLowerCase()==='deleted'?'del':'ok'; }
function selectedTotalBytes(){
  return selectedFiles().reduce((sum,e)=>sum+(Number.isFinite(+e.size)?+e.size:0),0);
}
function qualityTier(e){
  if(e.kind==='Directory') return '';
  if(e.tier) return String(e.tier).toLowerCase();
  const c=parseFloat(e.confidence);
  if(Number.isFinite(c)){
    if(c>=0.85) return 'high';
    if(c>=0.60) return 'medium';
    return 'low';
  }
  if(String(e.quality||'').includes('可疑')) return 'low';
  return 'high';
}
function qualityLabel(e){
  const t=qualityTier(e);
  if(t==='high') return tr('qualityLabels.high');
  if(t==='medium') return tr('qualityLabels.medium');
  if(t==='low') return tr('qualityLabels.low');
  return '-';
}
function qualityClass(e){
  const t=qualityTier(e);
  return t ? `q-${t}` : '';
}

function qualityBucket(e){
  if(e.kind==='Directory') return '';
  const raw = `${e.tier||''} ${e.quality||''} ${e.recoveryMode||''} ${e.reason||''}`.toLowerCase();
  if(raw.includes('partial') || raw.includes('bytes_lost') || raw.includes('部分')) return 'partial';
  const t=qualityTier(e);
  if(t==='high') return 'complete';
  if(t==='medium') return 'possible';
  return 'fragments';
}
function routeLabel(e){
  const b=qualityBucket(e);
  return {complete:'recovered/',possible:'suspect/possible',partial:'suspect/partial',fragments:'suspect/fragments'}[b] || '-';
}
function routeClass(e){
  const b=qualityBucket(e);
  return b ? `route-${b==='complete'?'recovered':b}` : '';
}
function bucketTitle(bucket){
  return tr(`bucketTitles.${bucket}`, bucket);
}
function bucketHint(bucket){
  return tr(`bucketHints.${bucket}`, '');
}
function qualityStats(entries=state.entries){
  const stats={complete:0,possible:0,partial:0,fragments:0,bytes:{complete:0,possible:0,partial:0,fragments:0}};
  for(const e of entries){
    if(e.kind!=='File') continue;
    const b=qualityBucket(e);
    if(!stats[b] && stats[b]!==0) continue;
    stats[b]++;
    const size=+e.size;
    if(Number.isFinite(size)) stats.bytes[b]+=size;
  }
  return stats;
}
function selectedBucketStats(){
  return qualityStats(selectedFiles());
}
function shouldAutoSelect(e){
  return e.kind==='File' && ['complete','possible'].includes(qualityBucket(e));
}
function predictedOutputPath(basePath, entry){
  const base = normalizeOutputDir(basePath);
  const id = entry?.id || Date.now();
  const sourceName = safeFilename(entry?.path, `recovered_${id}.bin`);
  const bucket = qualityBucket(entry);
  const folder = bucket==='complete' ? 'recovered' : bucket==='possible' ? 'suspect/possible' : bucket==='partial' ? 'suspect/partial' : 'suspect/fragments';
  return `${base}/${folder}/${sourceName}`;
}

function extractField(text, label){
  const line = String(text||'').split(/\r?\n/).find(x=>x.startsWith(label));
  if(!line) return '';
  return line.slice(label.length).trim();
}
function classifyRecoveryMessage(msg){
  const text = String(msg||'');
  if(text.startsWith('✓')) return {type:'success', label:'完整成功', cls:'ok'};
  if(text.startsWith('📦')) return {type:'quarantine', label:'隔離輸出', cls:'quarantine'};
  if(text.startsWith('📄')) return {type:'report', label:'報告', cls:'report'};
  if(text.startsWith('⚠️')) return {type:'skipped', label:'已略過', cls:'skip'};
  return {type:'skipped', label:'訊息', cls:'skip'};
}
function parseRecoveryResult(messages, selectedItems){
  const rows = [];
  let reportHtml = '';
  for(const msg of messages || []){
    const c = classifyRecoveryMessage(msg);
    const outPath = extractField(msg, '輸出路徑：') || extractField(msg, '隔離路徑：') || '';
    if(c.type==='report'){
      reportHtml = msg.replace(/^📄\s*已建立還原報告：\s*/, '').trim();
    }
    rows.push({message:String(msg), ...c, outputPath:outPath});
  }
  const success = rows.filter(r=>r.type==='success').length;
  const quarantine = rows.filter(r=>r.type==='quarantine').length;
  const skipped = rows.filter(r=>r.type==='skipped').length;
  const reportRows = rows.filter(r=>r.type==='report').length;
  return {
    generatedAt:new Date(),
    source:$('sourcePath')?.value || '',
    outputDir:normalizeOutputDir($('outputPath')?.value || ''),
    mode:activeMode,
    requested:(selectedItems||[]).length,
    success, quarantine, skipped, reportRows,
    reportHtml,
    rows
  };
}
function recoverySummaryText(report){
  if(!report) return '';
  return [
    '波立克資料救援｜還原完成報告',
    `模式：${report.mode}`,
    `來源：${report.source}`,
    `輸出資料夾：${report.outputDir}`,
    `選取：${report.requested} 個`,
    `完整成功：${report.success} 個`,
    `隔離輸出：${report.quarantine} 個`,
    `略過/警告：${report.skipped} 個`,
    report.reportHtml ? `HTML 報告：${report.reportHtml}` : '',
  ].filter(Boolean).join('\n');
}
function renderRecoveryReport(report){
  state.lastRecoveryReport = report;
  const drawer = $('recoveryDrawer');
  if(!drawer) return;
  $('recoveryReportTitle').textContent = `還原完成：成功 ${report.success}・隔離 ${report.quarantine}・略過 ${report.skipped}`;
  $('recoverySummaryCards').innerHTML = `
    <div class="rcard total"><span>選取</span><strong>${report.requested}</strong></div>
    <div class="rcard ok"><span>完整成功</span><strong>${report.success}</strong></div>
    <div class="rcard q"><span>隔離輸出</span><strong>${report.quarantine}</strong></div>
    <div class="rcard skip"><span>略過/警告</span><strong>${report.skipped}</strong></div>`;
  const htmlBtn = $('openHtmlReport');
  if(report.reportHtml){
    $('recoveryReportPath').textContent = `報告位置：${report.reportHtml}`;
    htmlBtn.style.display = 'inline-flex';
  } else {
    $('recoveryReportPath').textContent = '尚未取得 HTML 報告路徑；可到輸出資料夾查看 recovery_report.html / recovery_report.json。';
    htmlBtn.style.display = 'none';
  }
  $('recoveryReportRows').innerHTML = report.rows.map(r=>`
    <tr>
      <td><span class="result-status ${r.cls}">${esc(r.label)}</span></td>
      <td><div class="result-detail">${esc(r.message)}</div></td>
    </tr>`).join('') || '<tr><td colspan="2">沒有回傳任何還原訊息。</td></tr>';
  drawer.classList.add('show');
}
async function openRecoveryHtmlReport(){
  const report = state.lastRecoveryReport;
  if(!report?.reportHtml) return;
  const core=getTauri()?.core;
  if(core){
    try{ await core.invoke('open_report_file',{path:report.reportHtml}); setAS('已開啟 HTML 報告。','ok'); return; }
    catch(err){ setAS(`開啟報告失敗：${err}`,'error'); }
  }
}
async function copyRecoverySummary(){
  const text = recoverySummaryText(state.lastRecoveryReport);
  if(!text) return;
  try{ await navigator.clipboard.writeText(text); setAS('已複製還原摘要。','ok'); }
  catch(_){ setAS('無法使用剪貼簿，請手動複製報告內容。','error'); }
}
function hideRecoveryReport(){ $('recoveryDrawer')?.classList.remove('show'); }

function updatePrimaryActions(){
  const scanVisible = hasTauriRuntime && wizStep===1;
  const source=($('sourcePath')?.value || '').trim();
  const sourceReady=Boolean(source) && !isRawDevicePath(source) && !source.startsWith('fixtures/');
  const scanBlocked=!sourceReady || outputIsolation.pending || !outputIsolation.ok;
  scanButtons().forEach(btn=>{
    btn.style.display = scanVisible ? 'inline-flex' : 'none';
    btn.disabled = btn.id==='runRealScanTop' && !sourceReady ? false : scanBlocked;
    if(btn.id==='runRealScanTop') btn.textContent=tr(sourceReady?'startScan':'chooseImage');
  });
  const filesCount = selectedFiles().length;
  const canExtract = hasTauriRuntime && wizStep===3 && filesCount>0;
  const topExtract = $('runRealExtract');
  if(topExtract){
    topExtract.style.display = hasTauriRuntime && wizStep===3 ? 'inline-flex' : 'none';
    topExtract.disabled = filesCount===0;
    topExtract.title = filesCount===0 ? '請先勾選要還原的檔案' : `準備還原 ${filesCount} 個檔案`;
  }
  const cta = $('ctaBtn');
  if(cta) cta.classList.toggle('vis', canExtract);
}

// ── Wizard ──
function setStep(n){
  wizStep = n;
  const dots=[[$('wd1'),$('wl1')],[$('wd2'),$('wl2')],[$('wd3'),$('wl3')]];
  dots.forEach(([d,l],i)=>{
    const idx=i+1;
    d.className='wdot'+(idx<n?' dn':idx===n?' act':'');
    l.className='wlbl'+(idx<n?' dn':idx===n?' act':'');
  });
  $('wln1').className='wline'+(n>1?' dn':'');
  $('wln2').className='wline'+(n>2?' dn':'');
  $('scanView').classList.toggle('act', n===2);
  $('workspace').style.display = n===2 ? 'none' : '';
  $('emptyState').style.display = n===1 ? 'flex' : 'none';
  $('s3').style.display = n===3 ? 'flex' : 'none';
  updatePrimaryActions();
}

// ── Advanced mode ──
function setAdv(on){
  document.body.classList.toggle('adv', on);
  $('togSwitch').classList.toggle('on', on);
  $('advRow')?.setAttribute('aria-expanded', String(on));
  $('advRow')?.setAttribute('aria-pressed', String(on));
}
$('advRow').addEventListener('click', ()=>setAdv(!document.body.classList.contains('adv')));

// ── Status ──
function setAS(msg,kind=''){
  const el=$('actionStatus');
  el.textContent=msg||'';
  el.className='as'+(kind?' '+kind:'');
}

function setScanProgress(payload){
  // payload 可以是舊字串，也可以是後端的 ScanProgressPayload 結構化物件
  let pct = null;
  let text = '';
  if(payload && typeof payload === 'object'){
    pct = typeof payload.pct === 'number' ? payload.pct : null;
    if(payload.msg) text = payload.msg;
    // 掃描中：顯示動態計數
    if(payload.scanned > 0 && payload.total > 0){
      const title = $('scanTitle');
      if(title) title.textContent = tr('scanTitle','掃描中，請稍候...');
      const sub = $('scanSub');
      if(sub) sub.innerHTML =
        `已掃 <strong>${payload.scanned.toLocaleString()}</strong> / ${payload.total.toLocaleString()} 筆記錄` +
        (payload.filesFound > 0 ? `，找到 <strong>${payload.filesFound.toLocaleString()}</strong> 個檔案` : '');
    }
    // 還原中：顯示 done/total + 當前檔名
    if(payload.done != null && payload.total > 0){
      const title = $('scanTitle');
      if(title) title.textContent = `還原中 ${payload.done} / ${payload.total}`;
      if(payload.currentFile){
        const sub = $('scanSub');
        if(sub) sub.innerHTML = `目前：<strong>${esc(payload.currentFile)}</strong>`;
        text = `還原 ${payload.done}/${payload.total}・${payload.currentFile}`;
      }
    }
  } else {
    text = String(payload || '處理中...');
    // 舊字串模式：解析 "N%" 格式
    const m = text.match(/(\d{1,3})\s*%/);
    if(m) pct = Math.max(0, Math.min(100, Number(m[1])));
  }
  const txt = $('scanTxt');
  if(txt && text) txt.textContent = text;
  const fill = $('scanFill');
  if(!fill) return;
  if(pct != null){
    fill.classList.remove('indeterminate');
    fill.style.width = Math.max(0, Math.min(100, pct)) + '%';
    fill.style.marginLeft = '0';
  } else {
    fill.classList.add('indeterminate');
    fill.style.width = '';
    fill.style.marginLeft = '';
  }
}
function setExtractProgress(payload){
  if(!payload || typeof payload !== 'object'){ setAS(String(payload||''), ''); return; }
  const done = payload.done ?? 0;
  const total = payload.total ?? 0;
  const file = payload.currentFile || '';
  const pct = payload.pct ?? (total > 0 ? Math.round(done * 100 / total) : 0);
  const msg = total > 0
    ? `還原 ${done} / ${total}${file ? '・' + file : ''}`
    : (file || '還原中...');
  setAS(msg, '');
  setScanProgress({pct, done, total, currentFile: file, msg});
}
function setOperationBusy(on, label='處理中'){
  operationInProgress = !!on;
  const cancelBtn = $('cancelOperationBtn');
  if(cancelBtn){
    cancelBtn.disabled = !on;
    cancelBtn.textContent = tr('stopCurrent');
  }
  const cancelTop = $('cancelOperationTop');
  if(cancelTop){
    cancelTop.style.display = on ? 'inline-flex' : 'none';
    cancelTop.disabled = !on;
  }
  document.body.classList.toggle('operation-busy', !!on);
  if(on) setScanProgress(label);
}
function isUserCancelError(err){
  return String(err || '').includes('中止') || String(err || '').toLowerCase().includes('cancel');
}
function makeJobId(kind){
  const entropy = globalThis.crypto?.randomUUID?.() || `${Date.now()}-${Math.random().toString(16).slice(2)}`;
  return `${kind}-${entropy}`;
}
async function cancelCurrentOperation(){
  const core = getTauri()?.core;
  if(!core || !operationInProgress || !currentJobId){ return; }
  try{
    setAS('正在送出中止要求...', 'error');
    setScanProgress('正在中止目前作業...');
    const msg = await core.invoke('cancel_current_operation',{jobId:currentJobId});
    setAS(msg || '已送出中止要求。', 'error');
  }catch(err){
    setAS(`中止作業失敗：${err}`, 'error');
  }
}
async function showTauri(){
  if(!getTauri()) return false;
  hasTauriRuntime = true;
  $('runtimeBadge').textContent=tr('desktopMode');
  $('runtimeBadge').className='status-chip safe';
  updatePrimaryActions();
  $('browseOutput').style.display='inline-flex';
  $('browseSource').style.display='inline-flex';
  $('diskPickGroup').style.display='';
  $('partitionPickGroup').style.display='';
  loadDisks();
  loadPartitions();
  refreshDiagnostics();
  // 預設輸出路徑：桌面的「已救援」資料夾
  const core = getTauri()?.core;
  if(core && !defaultRecoveryDir) {
    try {
      defaultRecoveryDir = await core.invoke('get_recovery_default_dir');
    } catch (_) {
      defaultRecoveryDir = '/Users/Shared/已救援';
    }
  }
  if(defaultRecoveryDir && isSampleOutput($('outputPath').value)) {
    $('outputPath').value = defaultRecoveryDir;
    renderCommand();
  }
  refreshOutputIsolation();
  // 監聽掃描/還原進度事件
  if(window.__TAURI__?.event && !progressListenersReady) {
    progressListenersReady = true;
    window.__TAURI__.event.listen('scan-progress', e=>{
      setScanProgress(e.payload);
    });
    window.__TAURI__.event.listen('extract-progress', e=>{
      setExtractProgress(e.payload);
    });
  }
  return true;
}

async function loadDisks(){
  const core=getTauri()?.core;if(!core)return;
  const sel=$('diskSel');
  sel.innerHTML='<option value="">— 偵測中... —</option>';
  try{
    allDisks=await core.invoke('list_disks');
    applyDiskFilter();
  }catch(err){
    sel.innerHTML='<option value="">— 偵測失敗 —</option>';
    setAS(`磁碟偵測失敗：${err}`,'error');
  }
}

async function loadPartitions(){
  const core=getTauri()?.core;
  const group=$('partitionPickGroup');
  const sel=$('partitionSel');
  const source=($('sourcePath')?.value || '').trim();
  if(!core || !group || !sel || !source) return;
  if(isRawDevicePath(source)){
    detectedPartitions=[];
    selectedPartitionOffset=0;
    sel.innerHTML='<option value="0">請先建立唯讀映像檔，再選擇 .img</option>';
    return;
  }
  sel.innerHTML='<option value="0">— 偵測分割區中... —</option>';
  selectedPartitionOffset=0;
  try{
    const result=await core.invoke('detect_partitions',{filePath:source});
    detectedPartitions=result.partitions || [];
    sel.innerHTML='<option value="0">整個映像檔（offset 0）</option>';
    detectedPartitions.forEach(partition=>{
      const option=document.createElement('option');
      option.value=String(partition.offset);
      option.textContent=`${result.table} #${partition.index} · ${partition.filesystem||'unknown'} · offset ${partition.offset} · ${fmtBytes(partition.length)}${partition.name?` · ${partition.name}`:''}`;
      sel.appendChild(option);
    });
    if(detectedPartitions.length===1){
      sel.value=String(detectedPartitions[0].offset);
      selectedPartitionOffset=detectedPartitions[0].offset;
    }
  }catch(_){
    detectedPartitions=[];
    sel.innerHTML='<option value="0">未偵測到有效 MBR/GPT（offset 0）</option>';
  }
}

function applyDiskFilter(){
  const sel=$('diskSel');
  const prev=sel.value;
  // exFAT = 隨身碟 → 只顯示外接；APFS = Mac 磁碟 → 只顯示內建；其他 → 全部
  let list=allDisks;
  if(activeMode==='exfat' || activeMode==='fat32') list=allDisks.filter(d=>d.removable);
  else if(activeMode==='apfs') list=allDisks.filter(d=>!d.removable);
  if(!list.length){
    sel.innerHTML='<option value="">— 未偵測到符合的磁碟 —</option>';
    return;
  }
  sel.innerHTML='<option value="">— 選擇磁碟 —</option>';
  for(const d of list){
    const o=document.createElement('option');
    o.value=d.path;o.textContent=d.label;
    if(d.removable)o.className='ext';
    sel.appendChild(o);
  }
  // 恢復上次選擇；否則自動選第一個外接（或第一個）
  if(prev&&list.find(d=>d.path===prev)){
    sel.value=prev;
  } else {
    const pref=list.find(d=>d.removable)||list[0];
    if(pref){
      sel.value=pref.path;
      if(isRawDevicePath(pref.path)){
        $('sourcePath').value='';
        selectedPartitionOffset=0;
        setAS('已偵測到實體磁碟。請先建立唯讀映像檔，再選擇 .img。','error');
      } else {
        $('sourcePath').value=pref.path;
        loadPartitions();
      }
      renderCommand();renderRiskPanel();updatePrimaryActions();
    }
  }
  renderRiskPanel();
}
function tauriAPI(ns){
  const api=getTauri()?.[ns];
  if(!api){
    const m=`Tauri ${ns} API 尚未載入。請開啟桌面 App；若用瀏覽器開啟 HTML，真實掃描不可用。`;
    setAS(m,'error'); void showAppNotice('需要桌面 App', m); return null;
  }
  return api;
}

// ── Mode config (labels + inputs, no parse) ──
function setModeConfig(mode, {loadSample=true}={}){
  activeMode = mode;
  document.querySelectorAll('.nav button').forEach(b=>b.classList.remove('active'));
  const ids={ntfs:'btnModeNtfs',exfat:'btnModeExfat',fat32:'btnModeFat32',apfs:'btnModeApfs',carver:'btnModeCarver'};
  $(ids[mode]).classList.add('active');
  if($('mobileModeSelect')) $('mobileModeSelect').value=mode;
  const cfg=samples[mode];
  const meta=modeMeta[mode];
  const localized=trObj(`modes.${mode}`);
  $('modeTitle').textContent=localized.title || cfg.title;
  $('modeBadge').textContent=localized.badge || meta.badge;
  $('emptyIcon').textContent=meta.icon;
  $('emptyTitle').textContent=localized.emptyTitle || meta.emptyTitle;
  $('modeDesc').textContent=localized.desc || meta.desc;
  $('modeAdvice').textContent=localized.advice || meta.advice;
  $('modeAdviceIcon').textContent=meta.icon;
  if($('ntfsScanProfileGroup')) $('ntfsScanProfileGroup').style.display = mode==='ntfs' ? '' : 'none';
  if($('apfsScanScopeGroup')) $('apfsScanScopeGroup').style.display = mode==='apfs' ? '' : 'none';
  if(loadSample && isSampleOutput($('outputPath').value)) {
    $('outputPath').value=defaultRecoveryDir || cfg.output;
  }
  applyDiskFilter();
  if(loadSample){
    if(!$('diskSel')?.value) $('sourcePath').value=cfg.source;
    $('reportInput').value=cfg.report;
    $('vfsInput').value=cfg.vfs;
    $('diskBar').innerHTML=cfg.diskBar;
  }
  applyMetricLabels(mode==='carver'?'carver':mode==='apfs'?'apfs':'default');
  renderCommand();
  renderRiskPanel();
  refreshImageWorkflowPlan();
  refreshSessionWorkflowPlan();
}

function resetResultsForModeChange(){
  scanPageTicket++;
  thumbnailQueue.length=0;
  thumbCache.clear();
  if(thumbObserver){ thumbObserver.disconnect(); thumbObserver=null; }
  latestStructuredEntries=null;
  state.report={};
  state.entries=[];
  state.selected=null;
  state.selectedIds.clear();
  state.lastSelectedId=null;
  state.lastRecoveryReport=null;
  activeFilter='all';
  $$('.fb[data-f]').forEach(btn=>btn.classList.toggle('act', btn.dataset.f==='all'));
}

function switchMode(mode){
  if(operationInProgress){
    setAS('目前有作業進行中，請先中止或等待完成後再切換掃描類型。','error');
    return;
  }
  if(mode!==activeMode) resetResultsForModeChange();
  setModeConfig(mode);
  setStep(1);
  render();
}

// setMode = setModeConfig + parse (used by load-samples)
function setMode(mode){ setModeConfig(mode); parseInputs(); }

// ── Parse ──
function parseReport(txt){
  const o={};
  for(const ln of txt.split(/\r?\n/)){const i=ln.indexOf(':');if(i<0)continue;o[ln.slice(0,i).trim()]=ln.slice(i+1).trim();}
  return o;
}
function parseVfs(txt){
  return txt.split(/\r?\n/).map(l=>l.trim()).filter(Boolean)
    .filter(l=>!['source:','volume_offset:','mft_offset:','file_record_size:','records_read:'].some(p=>l.startsWith(p)))
    .map((ln,idx)=>{
      const p=ln.split(/\t/);
      const attrs=Object.fromEntries(p.slice(3).map(x=>{const i=x.indexOf('=');return i<0?[x,'']:[x.slice(0,i),x.slice(i+1)];}));
      return{path:p[0]||'/unknown',kind:p[1]||'Unknown',status:p[2]||'Unknown',size:attrs.size||'-',id:attrs.id||String(idx),confidence:attrs.confidence||'',quality:attrs.quality||'',tier:attrs.tier||'',recoveryMode:attrs.recovery_mode||attrs.mode||'',bytesLost:attrs.bytes_lost||'',bytesRead:attrs.bytes_read||'',reason:attrs.reason||''};
    });
}
function parseInputs(structuredEntries=null){
  scanPageTicket++;
  thumbnailQueue.length=0;
  thumbCache.clear(); // 新掃描時清除縮圖快取
  if(thumbObserver){ thumbObserver.disconnect(); thumbObserver=null; }
  state.report=parseReport($('reportInput').value);
  latestStructuredEntries = Array.isArray(structuredEntries) ? structuredEntries : null;
  state.entries=latestStructuredEntries || parseVfs($('vfsInput').value);
  state.selected=state.entries.find(e=>e.kind==='File')||state.entries[0]||null;
  const autoSelected = state.entries.filter(shouldAutoSelect).map(e=>e.id);
  state.selectedIds=new Set(autoSelected.length ? autoSelected : (state.selected && state.selected.kind==='File' ? [state.selected.id] : []));
  state.lastSelectedId=state.selected?.id || null;
  if(state.entries.length>0) setStep(3); else setStep(1);
  render();
}

async function loadRemainingScanPages(firstPage){
  if(!firstPage?.scanId || firstPage.nextCursor == null) return;
  const core=getTauri()?.core;
  if(!core) return;
  const ticket=++scanPageTicket;
  const merged=Array.isArray(firstPage.entries) ? firstPage.entries.slice() : [];
  let cursor=firstPage.nextCursor;
  try{
    while(cursor != null){
      const page=await core.invoke('get_scan_page',{scanId:firstPage.scanId,cursor,limit:500});
      if(ticket!==scanPageTicket) return;
      merged.push(...(Array.isArray(page.entries) ? page.entries : []));
      latestStructuredEntries=merged;
      state.entries=merged;
      cursor=page.nextCursor;
      render();
    }
  }catch(err){
    if(ticket===scanPageTicket) setAS(`後續掃描結果載入失敗：${err}`,'error');
  }
}

// ── Filter ──
function pathFilterValue(){
  return ($('pathFilter')?.value || '').trim().toLowerCase();
}
function filtered(){
  const pathF = pathFilterValue();
  let base;
  if(activeFilter==='all') base = state.entries;
  else if(activeFilter==='deleted') base = state.entries.filter(e=>e.kind==='File' && e.status.toLowerCase()==='deleted');
  else if(['complete','possible','partial','fragments'].includes(activeFilter)) base = state.entries.filter(e=>e.kind==='File' && qualityBucket(e)===activeFilter);
  else base = state.entries.filter(e=>{ if(e.kind==='Directory') return false; return getCat(e.path)===activeFilter; });
  if(!pathF) return base;
  return base.filter(e=>e.path.toLowerCase().includes(pathF));
}

function selectedFiles(){
  return state.entries.filter(e=>e.kind==='File' && state.selectedIds.has(e.id));
}

function selectableInCurrentView(){
  return filtered().filter(e=>e.kind==='File');
}

function chooseRange(toEntry){
  const list=selectableInCurrentView();
  const to=list.findIndex(e=>e.id===toEntry.id);
  const from=list.findIndex(e=>e.id===state.lastSelectedId);
  if(from < 0 || to < 0) {
    toggleEntry(toEntry, true);
    return;
  }
  const [start,end]=from<to?[from,to]:[to,from];
  for(const e of list.slice(start,end+1)) state.selectedIds.add(e.id);
  state.selected=toEntry;
  state.lastSelectedId=toEntry.id;
  render();
}

function toggleEntry(e, force){
  if(e.kind==='Directory') {
    state.selected=e;
    render();
    return;
  }
  const on = force ?? !state.selectedIds.has(e.id);
  if(on) state.selectedIds.add(e.id);
  else state.selectedIds.delete(e.id);
  state.selected=e;
  state.lastSelectedId=e.id;
  render();
}

function handleEntryClick(e, ev, force){
  if(ev?.shiftKey && e.kind==='File') chooseRange(e);
  else toggleEntry(e, force);
}

// ── Render ──
function render(){
  // Metrics
  if(activeMode==='carver'){
    $('mRec').textContent=state.report.signatures_detected||'0';
    $('mFil').textContent=state.report.png_carved?
      [state.report.png_carved,state.report.jpeg_carved,state.report.pdf_carved,state.report.zip_carved,state.report.mp4_carved,state.report.heic_carved].reduce((a,v)=>a+parseInt(v||0),0):'0';
    $('mDel').textContent=state.report.confidence_average||'1.0';
    $('mExt').textContent=state.report.total_bytes_scanned?fmtBytes(+state.report.total_bytes_scanned):'0 B';
    $('volLbl').textContent=tr('modes.carver.title');$('mftLbl').textContent=tr('noValidPartition','No valid partition table');
  } else if(activeMode==='apfs'){
    $('mRec').textContent=state.report.checksum_status||'OK';
    $('mFil').textContent=state.report.spaceman_oid||'0';
    $('mDel').textContent=state.report.max_file_systems||'0';
    $('mExt').textContent=state.report.block_size?fmtBytes(+state.report.block_size):'0 B';
    $('volLbl').textContent='APFS Container';
    $('mftLbl').textContent=state.report.omap_oid?`Object Map OID ${state.report.omap_oid}`:'';
  } else {
    const fileEntries=state.entries.filter(e=>e.kind==='File');
    const deletedEntries=fileEntries.filter(e=>String(e.status).toLowerCase()==='deleted');
    const visibleBytes=fileEntries.reduce((total,e)=>{
      const size=Number(e.size);
      return Number.isFinite(size) && size>0 ? total+size : total;
    },0);
    $('mRec').textContent=state.report.records_scanned||'0';
    $('mFil').textContent=String(fileEntries.length);
    $('mDel').textContent=String(deletedEntries.length);
    $('mExt').textContent=visibleBytes?fmtBytes(visibleBytes):'0 B';
    $('volLbl').textContent=state.report.total_size_bytes?`Disk size: ${fmtBytes(+state.report.total_size_bytes)}`:'Disk not loaded';
    $('mftLbl').textContent=state.report.mft_offset?`Address offset ${state.report.mft_offset}`:'';
  }

  const list=filtered();
  renderQualitySummary();
  $('entryCount').textContent=`${list.length} ${tr('itemCount')}`;
  const selFiles=selectedFiles();
  const selStats=selectedBucketStats();
  $('selectedCount').textContent=`${tr('selectedPrefix')} ${selFiles.length}`;
  $('selectionStrip').innerHTML = selFiles.length
    ? `<div class="selection-main">${trf('selectedApprox',{count:selFiles.length,size:fmtBytes(selectedTotalBytes())})}</div><div class="selection-sub">${trf('bucketCounts', selStats)}</div>`
    : `<div class="selection-main">${tr('selectionMain')}</div><div class="selection-sub">${tr('emptySelectionSub')}</div>`;
  renderList(list);
  renderGrid(list);
  renderDetails();
  renderCommand();
  if(isGrid) setupThumbObserver();
  updatePrimaryActions();
}

function renderQualitySummary(){
  const stats=qualityStats();
  const total=stats.complete+stats.possible+stats.partial+stats.fragments;
  const selected=selectedBucketStats();
  const cards=[
    ['complete','✅',stats.complete,stats.bytes.complete,selected.complete],
    ['possible','🟡',stats.possible,stats.bytes.possible,selected.possible],
    ['partial','🧩',stats.partial,stats.bytes.partial,selected.partial],
    ['fragments','⚠️',stats.fragments,stats.bytes.fragments,selected.fragments],
  ];
  $('qualitySummary').innerHTML = cards.map(([bucket,icon,count,bytes,sel])=>`
    <button type="button" class="qcard ${bucket}" data-qf="${bucket}" title="${bucketTitle(bucket)}" aria-pressed="${activeFilter===bucket}">
      <span>${icon} ${bucketTitle(bucket)}</span>
      <strong>${count}</strong>
      <small>${fmtBytes(bytes)} | ${tr('selectedPrefix')} ${sel}<br>${bucketHint(bucket)}</small>
    </button>
  `).join('');
  $('qualityNote').textContent = total
    ? trf('qualitySummary', stats)
    : tr('qualityEmpty');
  $$('.qcard').forEach(card=>card.addEventListener('click',()=>{
    activeFilter=card.dataset.qf;
    $$('.fb').forEach(b=>b.classList.toggle('act', b.dataset.f===activeFilter));
    $$('.fb[data-f]').forEach(b=>b.setAttribute('aria-pressed', String(b.dataset.f===activeFilter)));
    render();
  }));
}

function renderList(entries){
  const body=$('treeBody'); body.innerHTML='';
  visibleListEntries=entries;
  visibleEntryById.clear();
  for(const entry of entries) visibleEntryById.set(String(entry.id),entry);
  if(!entries.length){
    const labels=trObj('categoryLabels');
    const hint = activeFilter==='all'
      ? tr('noFiles')
      : trf('noFilteredFiles',{type:labels[activeFilter]||activeFilter});
    body.innerHTML=`<div class="empty-hint">${hint}</div>`;
    return;
  }
  const heads=tr('listHead',[]);
  body.insertAdjacentHTML('beforeend',`<div class="list-head"><span></span><span>${heads[0]}</span><span>${heads[1]}</span><span>${heads[2]}</span><span>${heads[3]}</span><span class="list-head-right">${heads[4]}</span></div><div class="virtual-list"></div>`);
  body.querySelector('.virtual-list').style.setProperty('--virtual-height', `${entries.length*VIRTUAL_ROW_HEIGHT}px`);
  ensureListVirtualHandlers(body);
  renderVirtualListWindow();
}

function ensureListVirtualHandlers(body){
  if(body.dataset.virtualBound==='1') return;
  body.dataset.virtualBound='1';
  body.addEventListener('scroll',renderVirtualListWindow,{passive:true});
  body.addEventListener('change',ev=>{
    const pick=ev.target.closest?.('.pick');
    if(!pick) return;
    const row=pick.closest('[data-entry-id]');
    const entry=row && visibleEntryById.get(String(row.dataset.entryId));
    if(entry) handleEntryClick(entry,ev,pick.checked);
  });
  body.addEventListener('click',ev=>{
    if(ev.target.closest?.('.pick')) return;
    const row=ev.target.closest?.('[data-entry-id]');
    const entry=row && visibleEntryById.get(String(row.dataset.entryId));
    if(!entry) return;
    if(ev.target.closest?.('.quick-rec')){
      ev.stopPropagation();
      quickRecover(entry,row);
      return;
    }
    handleEntryClick(entry,ev);
  });
}

function makeListRow(e){
  const isSel=state.selectedIds.has(e.id);
  const cat=e.kind==='Directory'?'Directory':getCat(e.path);
  const row=document.createElement('div');
  row.className='tree-row virtual-row'+(isSel?' sel':'');
  row.dataset.entryId=String(e.id);
  row.innerHTML=`<input class="pick" type="checkbox" ${isSel?'checked':''} ${e.kind==='Directory'?'disabled':''} aria-label="${esc(trf('selectAria',{name:fname(e.path)}))}"><div class="tree-name"><span class="ico">${CAT_ICON[cat]||CAT_ICON.other}</span><span title="${esc(e.path)}">${esc(fname(e.path))}</span></div><span class="pill ${stCls(e.status)}">${stLbl(e.status)}</span><span class="pill ${qualityClass(e)}">${e.kind==='File'?qualityLabel(e):'-'}</span><span class="pill ${routeClass(e)}">${e.kind==='File'?routeLabel(e):'-'}</span><span class="entry-size">${e.size==='-'?'-':fmtBytes(+e.size)}</span>${e.kind==='File'?`<button class="quick-rec" type="button" aria-label="${esc(tr('quickRecover'))}">${esc(tr('quickRecover'))}</button>`:''}`;
  return row;
}

function renderVirtualListWindow(){
  const body=$('treeBody');
  const canvas=body?.querySelector('.virtual-list');
  if(!body || !canvas || !visibleListEntries.length) return;
  const header=body.querySelector('.list-head');
  const headerHeight=header?.offsetHeight || 32;
  const top=Math.max(0,body.scrollTop-headerHeight);
  const first=Math.max(0,Math.floor(top/VIRTUAL_ROW_HEIGHT)-VIRTUAL_OVERSCAN);
  const last=Math.min(visibleListEntries.length,Math.ceil((top+body.clientHeight)/VIRTUAL_ROW_HEIGHT)+VIRTUAL_OVERSCAN);
  canvas.replaceChildren();
  for(let index=first;index<last;index++){
    const row=makeListRow(visibleListEntries[index]);
    row.style.transform=`translateY(${index*VIRTUAL_ROW_HEIGHT}px)`;
    canvas.appendChild(row);
  }
}

function renderGrid(entries){
  const body=$('gridBody'); body.innerHTML='';
  visibleGridEntries=entries;
  for(const entry of entries) visibleEntryById.set(String(entry.id),entry);
  body.classList.add('virtual-grid');
  body.style.height=`${Math.max(1,Math.ceil(entries.length/4)*VIRTUAL_GRID_ROW_HEIGHT)}px`;
  ensureGridVirtualHandlers(body);
  renderVirtualGridWindow();
}

function ensureGridVirtualHandlers(body){
  if(body.dataset.virtualBound==='1') return;
  body.dataset.virtualBound='1';
  $('gridWrap').addEventListener('scroll',renderVirtualGridWindow,{passive:true});
  body.addEventListener('change',ev=>{
    const pick=ev.target.closest?.('.pick');
    if(!pick) return;
    const item=pick.closest('[data-entry-id]');
    const entry=item && visibleEntryById.get(String(item.dataset.entryId));
    if(entry) handleEntryClick(entry,ev,pick.checked);
  });
  body.addEventListener('click',ev=>{
    if(ev.target.closest?.('.pick')) return;
    const item=ev.target.closest?.('[data-entry-id]');
    const entry=item && visibleEntryById.get(String(item.dataset.entryId));
    if(!entry) return;
    if(ev.target.closest?.('.quick-rec-g')){
      ev.stopPropagation();
      quickRecover(entry,item);
      return;
    }
    handleEntryClick(entry,ev);
  });
}

function drainThumbnailQueue(){
  while(thumbnailInFlight<THUMBNAIL_CONCURRENCY && thumbnailQueue.length){
    const {el,entryId}=thumbnailQueue.shift();
    thumbnailInFlight++;
    const entry=state.entries.find(item=>String(item.id)===entryId);
    const core=getTauri()?.core;
    if(!entry || !core){
      thumbnailInFlight--;
      continue;
    }
    core.invoke('read_file_thumbnail',{
      filePath:$('sourcePath').value.trim(),
      mode:activeMode,
      selectedId:entryId,
      offset:selectedPartitionOffset,
    }).then(b64=>{
      thumbCache.set(entryId,b64);
      const thumb=el.isConnected && el.querySelector('.gi-thumb');
      if(!thumb) return;
      const badge=thumb.querySelector('.gi-badge');
      const miniQ=thumb.querySelector('.mini-q');
      thumb.innerHTML='';
      if(miniQ) thumb.appendChild(miniQ.cloneNode(true));
      const img=document.createElement('img');
      img.src=`data:image/jpeg;base64,${b64}`;
      img.style.cssText='width:100%;height:100%;object-fit:cover;border-radius:7px;';
      img.alt='';
      thumb.appendChild(img);
      if(badge) thumb.appendChild(badge.cloneNode(true));
    }).catch(()=>{
      thumbCache.set(entryId,'error');
    }).finally(()=>{
      thumbnailInFlight--;
      drainThumbnailQueue();
    });
  }
}

function queueThumbnail(el,entryId){
  if(!entryId || thumbCache.has(entryId)) return;
  thumbCache.set(entryId,'loading');
  thumbnailQueue.push({el,entryId});
  drainThumbnailQueue();
}

function makeGridItem(e){
  const isSel=state.selectedIds.has(e.id);
  const isDel=e.status.toLowerCase()==='deleted';
  const cat=e.kind==='Directory'?'Directory':getCat(e.path);
  const ext=fext(e.path);
  const isImg=e.kind==='File' && cat==='image';
  const cachedThumb=thumbCache.get(String(e.id));
  const thumbContent=isImg && cachedThumb && cachedThumb!=='loading' && cachedThumb!=='error'
    ? `<img src="data:image/jpeg;base64,${cachedThumb}" class="thumb-image" alt="">`
    : (CAT_ICON[cat]||CAT_ICON.other);
  const item=document.createElement('div');
  item.className='gi'+(isSel?' sel':'')+(isDel?' del':'');
  item.dataset.entryId=String(e.id);
  if(isImg) item.dataset.thumbEntryId=String(e.id);
  item.innerHTML=`<input class="pick" type="checkbox" ${isSel?'checked':''} ${e.kind==='Directory'?'disabled':''} aria-label="${esc(trf('selectAria',{name:fname(e.path)}))}"><div class="gi-thumb">${e.kind==='File'?`<span class="mini-q ${qualityBucket(e)}"></span>`:''}${thumbContent}${ext?`<span class="gi-badge${isDel?' del':''}">${ext}</span>`:''}</div><div class="gi-lbl" title="${esc(e.path)}">${esc(fname(e.path))}</div>${e.kind==='File'?`<button class="quick-rec-g" type="button" aria-label="${esc(tr('quickRecover'))}">${esc(tr('quickRecover'))}</button>`:''}`;
  return item;
}

function renderVirtualGridWindow(){
  const body=$('gridBody');
  if(!body || !visibleGridEntries.length) return;
  const width=Math.max(1,body.clientWidth || $('gridWrap').clientWidth || 320);
  const columns=Math.max(1,Math.floor((width+7)/81));
  const rowCount=Math.ceil(visibleGridEntries.length/columns);
  body.style.height=`${Math.max(1,rowCount*VIRTUAL_GRID_ROW_HEIGHT)}px`;
  const scrollTop=$('gridWrap').scrollTop;
  const firstRow=Math.max(0,Math.floor(scrollTop/VIRTUAL_GRID_ROW_HEIGHT)-VIRTUAL_OVERSCAN);
  const lastRow=Math.ceil((scrollTop+$('gridWrap').clientHeight)/VIRTUAL_GRID_ROW_HEIGHT)+VIRTUAL_OVERSCAN;
  const first=firstRow*columns;
  const last=Math.min(visibleGridEntries.length,lastRow*columns);
  body.replaceChildren();
  for(let index=first;index<last;index++){
    const item=makeGridItem(visibleGridEntries[index]);
    const column=index%columns;
    const row=Math.floor(index/columns);
    item.style.left=`${column*100/columns}%`;
    item.style.width=`${100/columns}%`;
    item.style.top=`${row*VIRTUAL_GRID_ROW_HEIGHT}px`;
    body.appendChild(item);
  }
  if(isGrid) setupThumbObserver();
}

function setupThumbObserver(){
  if(!hasTauriRuntime) return;
  if(thumbObserver) thumbObserver.disconnect();
  thumbObserver = new IntersectionObserver(entries=>{
    for(const obs of entries){
      if(!obs.isIntersecting) continue;
      const el = obs.target;
      const entryId = el.dataset.thumbEntryId;
      if(!entryId) continue;
      if(thumbCache.has(entryId)) continue; // 已有快取
      thumbObserver.unobserve(el);
      queueThumbnail(el,entryId);
    }
  },{root: $('gridWrap'), rootMargin:'80px', threshold:0.1});
  $$('#gridBody [data-thumb-entry-id]').forEach(el=>thumbObserver.observe(el));
}

function renderDetails(){
  const body=$('detailsBody'), stat=$('selStatus');
  if(!state.selected){
    stat.textContent=tr('notSelected'); stat.className='pill';
    body.innerHTML=`<div class="field"><label>${tr('filePath')}</label><div class="val">${tr('detailsPrompt')}</div></div>`;
    return;
  }
  const e=state.selected;
  stat.textContent=stLbl(e.status); stat.className='pill '+stCls(e.status);
  const idLbl={ntfs:'MFT 紀錄編號',exfat:'目錄項目偏移',fat32:'FAT32 目錄項目偏移',carver:'特徵碼起始位置',apfs:'APFS inode / object id'}[activeMode]||'ID';
  const outPreview = e.kind==='File' ? predictedOutputPath($('outputPath').value, e) : tr('folderNotRestorable');
  const qualityRows = `${e.kind==='File'?`<div class="field"><label>${tr('recoveryConfidence')}</label><div class="val">${esc(qualityLabel(e))}${e.confidence?` | ${tr('confidence')} ${esc(e.confidence)}`:''}</div></div><div class="field"><label>${tr('routeStrategy')}</label><div class="val">${esc(routeLabel(e))} | ${esc(bucketHint(qualityBucket(e)))}</div></div>`:''}${e.quality?`<div class="field"><label>${tr('qualityTag')}</label><div class="val">${esc(e.quality)}</div></div>`:''}${e.recoveryMode?`<div class="field"><label>${tr('readMode')}</label><div class="val">${esc(e.recoveryMode)}${e.bytesLost?` | ${esc(trf('lostBytes',{bytes:e.bytesLost}))}`:''}</div></div>`:''}`;
  body.innerHTML=`<div class="field"><label>${tr('filePath')}</label><div class="val">${esc(e.path)}</div></div><div class="field"><label>${idLbl}</label><div class="val">${esc(e.id)}</div></div><div class="field"><label>${tr('nodeType')}</label><div class="val">${esc(e.kind)}</div></div><div class="field"><label>${tr('allocationStatus')}</label><div class="val">${stLbl(e.status)}</div></div><div class="field"><label>${tr('fileSize')}</label><div class="val">${e.size==='-'?'-':fmtBytes(+e.size)}</div></div>${qualityRows}<div class="field"><label>${tr('outputPreview')}</label><div class="val preview">${esc(outPreview)}</div></div>`;
}

function renderCommand(){
  const sel=state.selected||state.entries.find(e=>e.kind==='File')||{id:'0',size:'0'};
  const outputPath = buildOutputPath($('outputPath').value, sel);
  let cmd='';
  if(activeMode==='exfat') cmd=`cargo run -q -p data-recovery-cli -- exfat-extract --file ${shellQ($('sourcePath').value)} --entry-id ${sel.id} --output ${shellQ(outputPath)}`;
  else if(activeMode==='fat32') cmd=`cargo run -q -p data-recovery-cli -- fat32-extract --file ${shellQ($('sourcePath').value)} --entry-id ${sel.id} --output ${shellQ(outputPath)}`;
  else if(activeMode==='apfs') cmd=`cargo run -q -p data-recovery-cli -- apfs-info --file ${shellQ($('sourcePath').value)}`;
  else if(activeMode==='carver') cmd=`cargo run -q -p data-recovery-cli -- carve --file ${shellQ($('sourcePath').value)} --output-dir ${shellQ(normalizeOutputDir($('outputPath').value))} --format text`;
  else cmd=`cargo run -q -p data-recovery-cli -- ntfs-extract --file ${shellQ($('sourcePath').value)} --record ${sel.id} --output ${shellQ(outputPath)}`;
  $('commandPreview').textContent=cmd;
}

// ── View toggle ──
$('btnList').addEventListener('click',()=>{
  isGrid=false;
  $('treeBody').classList.remove('hidden');
  $('gridWrap').classList.remove('act');
  $('btnList').classList.add('act');
  $('btnGrid').classList.remove('act');
  $('btnList').setAttribute('aria-pressed','true');
  $('btnGrid').setAttribute('aria-pressed','false');
});
$('btnGrid').addEventListener('click',()=>{
  isGrid=true;
  $('treeBody').classList.add('hidden');
  $('gridWrap').classList.add('act');
  $('btnGrid').classList.add('act');
  $('btnList').classList.remove('act');
  $('btnList').setAttribute('aria-pressed','false');
  $('btnGrid').setAttribute('aria-pressed','true');
  renderVirtualGridWindow(); // 只建立可見格子並啟動縮圖懶加載
});

// ── Filter buttons ──
document.querySelectorAll('.fb').forEach(btn=>{
  btn.addEventListener('click',()=>{
    if(!btn.dataset.f) return;
    document.querySelectorAll('.fb').forEach(b=>b.classList.remove('act'));
    btn.classList.add('act');
    document.querySelectorAll('.fb[data-f]').forEach(b=>b.setAttribute('aria-pressed', String(b===btn)));
    activeFilter=btn.dataset.f;
    render();
  });
});
$('selectAllFiles').addEventListener('click',()=>{
  for(const e of state.entries) if(shouldAutoSelect(e)) state.selectedIds.add(e.id);
  state.selected=selectedFiles()[0]||state.selected;
  state.lastSelectedId=state.selected?.id || state.lastSelectedId;
  render();
});
$('clearSelection').addEventListener('click',()=>{
  state.selectedIds.clear();
  state.lastSelectedId=null;
  render();
});

// ── Path filter ──
$('pathFilter')?.addEventListener('input', ()=>{ render(); });
$('clearPathFilter')?.addEventListener('click',()=>{ if($('pathFilter')) $('pathFilter').value=''; render(); });

// ── Nav ──
$('btnModeNtfs').addEventListener('click',()=>switchMode('ntfs'));
$('btnModeExfat').addEventListener('click',()=>switchMode('exfat'));
$('btnModeFat32').addEventListener('click',()=>switchMode('fat32'));
$('btnModeApfs').addEventListener('click',()=>switchMode('apfs'));
$('btnModeCarver').addEventListener('click',()=>switchMode('carver'));
$('mobileModeSelect')?.addEventListener('change',event=>switchMode(event.target.value));
$('loadSampleData').addEventListener('click',()=>setMode(activeMode));
$('languageSelect')?.addEventListener('change',e=>setLocale(e.target.value));
$('previewSampleBtn').addEventListener('click',()=>{ setMode(activeMode); setAS(tr('sampleLoaded'),'ok'); });
$('parseInputs').addEventListener('click',parseInputs);

$('copyCommand').addEventListener('click',async()=>{
  const txt=$('commandPreview').textContent;
  try{
    if(navigator.clipboard?.writeText) await navigator.clipboard.writeText(txt);
    else{const t=document.createElement('textarea');t.value=txt;t.style.cssText='position:fixed;left:-9999px;';document.body.appendChild(t);t.focus();t.select();document.execCommand('copy');t.remove();}
    const orig=$('copyCommand').textContent;
    $('copyCommand').textContent=tr('copied');setAS(tr('commandCopied'),'ok');
    setTimeout(()=>$('copyCommand').textContent=orig,1500);
  }catch(err){setAS(`複製失敗：${err}`,'error');showFriendlyError('複製指令失敗', err, '複製指令');}
});

$('sourcePath').addEventListener('input',()=>{
  const source=$('sourcePath').value.trim();
  if(isRawDevicePath(source)){
    detectedPartitions=[];
    selectedPartitionOffset=0;
    setAS('實體磁碟不可直接掃描，請先建立唯讀映像檔。','error');
  }
  renderCommand(); renderRiskPanel(); refreshImageWorkflowPlan(); refreshSessionWorkflowPlan(); updatePrimaryActions(); refreshOutputIsolation();
});
$('sourcePath').addEventListener('change',()=>{ loadPartitions(); refreshOutputIsolation(); });
$('outputPath').addEventListener('input',()=>{ const p=outputDirProblem(); setAS(p,p?'error':''); renderCommand(); renderRiskPanel(); refreshImageWorkflowPlan(); refreshSessionWorkflowPlan(); refreshOutputIsolation(); });
$('ntfsScanProfile')?.addEventListener('change',()=>{ renderCommand(); renderRiskPanel(); setAS($('ntfsScanProfile').value==='auto'?'NTFS 將自動估算完整 MFT 筆數。':'已切換 NTFS 掃描深度。',''); });
function autoModeFromFs(fsType){
  const f=(fsType||'').toLowerCase();
  if(f.includes('exfat')) return 'exfat';
  if(f.includes('fat32') || f.includes('ms-dos fat32') || f.includes('msdos') || f.includes('ms-dos fat')) return 'fat32';
  if(f.includes('ntfs')||f.includes('windows_ntfs')) return 'ntfs';
  if(f.includes('apfs')) return 'apfs';
  return 'carver'; // Linux / unknown → deep scan
}
$('diskSel').addEventListener('change',()=>{
  const v=$('diskSel').value;
  if(!v) return;
  if(isRawDevicePath(v)){
    $('sourcePath').value='';
    selectedPartitionOffset=0;
    setAS('已選擇實體磁碟。請先建立唯讀映像檔，再選擇 .img。','error');
    renderCommand(); renderRiskPanel(); refreshImageWorkflowPlan(); refreshSessionWorkflowPlan(); updatePrimaryActions(); refreshOutputIsolation();
    return;
  }
  $('sourcePath').value=v;
  const disk=allDisks.find(d=>d.path===v);
  if(disk){
    const suggested=autoModeFromFs(disk.fs_type);
    if(suggested!==activeMode) switchMode(suggested);
    if(suggested==='carver') setAS('此分割區格式（'+disk.fs_type+'）沒有專屬解析器，已切換至深度碎塊掃描。','');
  }
  loadPartitions();
  renderCommand();
  renderRiskPanel();
  refreshImageWorkflowPlan();
  refreshSessionWorkflowPlan();
  refreshOutputIsolation();
});
$('btnRefreshDisks').addEventListener('click',loadDisks);
$('partitionSel')?.addEventListener('change',()=>{
  selectedPartitionOffset=Number($('partitionSel').value||0);
  renderCommand();
  renderRiskPanel();
});

// ── Tauri ──
showTauri();
setTimeout(showTauri,100);
setTimeout(showTauri,500);
setTimeout(refreshImageWorkflowPlan,600);
setTimeout(refreshSessionWorkflowPlan,650);
setTimeout(()=>{ if(!hasTauriRuntime){ setAS(tr('previewModeMessage')); updatePrimaryActions(); } },700);

$('browseSource').addEventListener('click',async()=>{
  const d=tauriAPI('dialog');if(!d)return;
  try{
    const sel=await d.open({title:'選擇來源磁碟映像或設備',multiple:false});
    if(sel){
      const chosen=Array.isArray(sel)?sel[0]:sel;
      if(isRawDevicePath(chosen)){
        $('sourcePath').value='';
        selectedPartitionOffset=0;
        setAS('實體磁碟不可直接掃描，請先建立唯讀映像檔。','error');
      } else {
        $('sourcePath').value=chosen;
        loadPartitions();
        setAS('已選擇來源映像檔。','ok');
      }
      renderCommand();renderRiskPanel();refreshImageWorkflowPlan();refreshSessionWorkflowPlan();updatePrimaryActions();refreshOutputIsolation();
    }
  }catch(err){setAS(`選擇失敗：${err}`,'error');showFriendlyError('選擇來源失敗', err, '選擇來源');}
});
$('browseOutput').addEventListener('click',async()=>{
  const d=tauriAPI('dialog');if(!d)return;
  try{
    const sel=await d.open({title:'選擇還原檔案要存放的資料夾',directory:true,multiple:false,defaultPath:normalizeOutputDir($('outputPath').value)});
    if(sel){$('outputPath').value=sel;renderCommand();renderRiskPanel();refreshImageWorkflowPlan();refreshSessionWorkflowPlan();refreshOutputIsolation();setAS('已選擇儲存位置。','ok');}
  }catch(err){setAS(`選擇失敗：${err}`,'error');showFriendlyError('選擇輸出位置失敗', err, '選擇輸出位置');}
});

function ntfsScanCount(){
  const profile = $('ntfsScanProfile')?.value || 'auto';
  if(profile === 'quick') return 10000;
  if(profile === 'fallback') return 50000;
  return 0; // 0 = 後端自動從 $MFT 大小估算完整筆數
}

async function doScan(){
  const core=tauriAPI('core');if(!core)return;
  const fp=$('sourcePath').value.trim();
  renderRiskPanel();
  if(!fp || fp.startsWith('fixtures/')){
    $('browseSource')?.click();
    return;
  }
  if(isRawDevicePath(fp)){
    setAS('實體磁碟不可直接掃描，請先建立唯讀映像檔。','error');
    void showAppNotice('需要先建立映像檔', '為保護原始磁碟，請先用 ddrescue 或 hdiutil 建立映像檔，再選擇 .img。');
    return;
  }
  const isolationProblem=outputIsolationProblem();
  if(isolationProblem){
    setAS(isolationProblem,'error');
    void showAppNotice('輸出位置需要調整', isolationProblem);
    return;
  }
  if(!(await confirmRiskBeforeScan())) return;
  const jobId=makeJobId('scan');
  currentJobId=jobId;
  const btns=scanButtons();
  const orig=btns.map(b=>[b,b.textContent]);
  btns.forEach(b=>{b.textContent='⏳ 掃描中...';b.disabled=true;});
  setStep(2);setOperationBusy(true, '正在準備掃描...');setAS('正在檢查映像檔與輸出實體磁碟身分...');
  try{
    const scanCount = activeMode==='ntfs' ? ntfsScanCount() : 0;
    const includeDeleted = activeMode==='apfs' && $('apfsScanScope')?.value==='deleted';
    const res=await core.invoke('run_recovery_scan',{filePath:fp,mode:activeMode,offset:selectedPartitionOffset,count:scanCount,jobId,includeDeleted});
    $('reportInput').value=res.report;$('vfsInput').value=res.vfs;
    parseInputs(res.entries);void loadRemainingScanPages(res);setAS(activeMode==='carver'?'掃描成功：已自動過濾未通過檔案結構檢查的碎片。':activeMode==='apfs'?(includeDeleted?'APFS 掃描成功（含已刪除檔）：已刪除檔內容無法驗證，還原後會放到 suspect/。':'APFS 掃描成功：未加密 extent 可嘗試提取；partial 會隔離到 suspect/。'):'掃描成功，請勾選要還原的檔案。','ok');
  }catch(err){
    setStep(1);
    if(isUserCancelError(err)){ setAS('已中止掃描。','error'); }
    else { setAS(`掃描失敗：${err}`,'error');showFriendlyError('掃描失敗', err, '掃描'); }
  }finally{
    if(currentJobId===jobId) currentJobId='';
    setOperationBusy(false);
    orig.forEach(([b,t])=>{b.textContent=t;b.disabled=false;});
    updatePrimaryActions();
  }
}
scanButtons().forEach(btn=>btn.addEventListener('click',doScan));

// ── 單一檔案快速救回 ──
async function quickRecover(entry, el){
  const core=getTauri()?.core;
  if(!core){ setAS('此功能需要桌面 App 模式。','error'); return; }
  if(operationInProgress){ setAS('已有另一個掃描／還原作業進行中，請先等待或取消。','error'); return; }
  if(entry.kind==='Directory'){ setAS('資料夾無法單獨救回，請展開後選擇檔案。','error'); return; }
  const outDir = normalizeOutputDir($('outputPath').value.trim() || defaultRecoveryDir);
  const outPath = buildOutputPath(outDir, entry);
  // 顯示確認對話框，預填輸出路徑
  const confirmed = await requestAppConfirmation(
    trf('quickRecoverConfirm',{name:fname(entry.path), size:entry.size==='-'?tr('quickRecoverUnknownSize'):fmtBytes(+entry.size), path:outPath},
      `立刻救回這個檔案？\n\n檔案：${fname(entry.path)}\n大小：${entry.size==='-'?'未知':fmtBytes(+entry.size)}\n輸出：${outPath}\n\n按「確認」開始。`)
  , '快速救回確認');
  if(!confirmed) return;
  // 按鈕改為進行中狀態
  const btns = el.querySelectorAll('.quick-rec, .quick-rec-g');
  btns.forEach(b=>{ b.textContent='⏳'; b.disabled=true; });
  const jobId=makeJobId('extract');
  currentJobId=jobId;
  setOperationBusy(true, '正在準備單一檔案救回...');
  setAS(`正在救回 ${fname(entry.path)}...`);
  try{
    const msg = await core.invoke('run_recovery_extract',{
      filePath: $('sourcePath').value.trim(),
      mode: activeMode,
      offset: selectedPartitionOffset,
      selectedId: String(entry.id),
      outputPath: outPath,
      jobId,
    });
    el.classList.add('rec-ok');
    btns.forEach(b=>{ b.textContent='✅'; b.disabled=false; });
    setAS(`已救回：${fname(entry.path)} → ${outPath}`,'ok');
  }catch(err){
    btns.forEach(b=>{ b.textContent='⚡ 立刻救回'; b.disabled=false; });
    showFriendlyError(`救回 ${fname(entry.path)} 失敗`, err, '單一救回');
  }finally{
    if(currentJobId===jobId) currentJobId='';
    setOperationBusy(false);
  }
}

async function doExtract(){
  const core=tauriAPI('core');if(!core)return;
  const files=selectedFiles();
  if(!files.length){await showAppNotice('尚未選擇檔案', '請先從清單中勾選要還原的檔案！');return;}
  const selectedStats=selectedBucketStats();
  if((selectedStats.partial+selectedStats.fragments)>0){
    const ok=await requestAppConfirmation(`你選了 ${selectedStats.partial} 個部分救援、${selectedStats.fragments} 個低信心碎片。\n\n這些檔案很可能打不開，程式會隔離到 suspect/，不會放進 recovered/。仍要繼續嗎？`, '低信心檔案還原確認');
    if(!ok) return;
  }
  const fp=$('sourcePath').value.trim();
  const outProblem=outputDirProblem();
  if(outProblem){ await showAppNotice('儲存位置需要調整', '⚠️ 儲存位置需要調整：\n'+outProblem); setAS(outProblem,'error'); return; }
  if(operationInProgress){ setAS('已有另一個掃描／還原作業進行中，請先等待或取消。','error'); return; }
  const jobId=makeJobId('extract');
  currentJobId=jobId;
  extractButtons().forEach(b=>{b.textContent='⏳ 還原中...';b.disabled=true;});
  (async()=>{
    setOperationBusy(true, '正在準備批次還原...');
    setAS(`正在準備批次還原 ${files.length} 個檔案；若已授權過，這裡不會再要求密碼...`);
    const items=files.map(entry=>({
      selectedId:String(entry.id),
      outputPath:buildOutputPath($('outputPath').value.trim(), entry)
    }));
    const done=await core.invoke('run_recovery_extract_batch',{filePath:fp,mode:activeMode,offset:selectedPartitionOffset,items,jobId});
    const report=parseRecoveryResult(done, files);
    setAS(`100% - 還原完成：完整成功 ${report.success} 個，隔離 ${report.quarantine} 個，略過 ${report.skipped} 個`,'ok');
    renderRecoveryReport(report);
  })()
    .catch(err=>{
      if(isUserCancelError(err)){ setAS('已中止還原。','error'); }
      else { setAS(`還原失敗：${err}`,'error');showFriendlyError('還原失敗', err, '批次還原'); }
    })
    .finally(()=>{
      if(currentJobId===jobId) currentJobId='';
      setOperationBusy(false);
      extractButtons().forEach(b=>{b.textContent='💾 還原選取的檔案';b.disabled=false;});
      updatePrimaryActions();
    });
}
$('runRealExtract').addEventListener('click',doExtract);
$('ctaBtn').addEventListener('click',doExtract);
$('cancelOperationBtn')?.addEventListener('click', cancelCurrentOperation);
$('cancelOperationTop')?.addEventListener('click', cancelCurrentOperation);


$('closeErrorDrawer')?.addEventListener('click', hideFriendlyError);
$('copyErrorText')?.addEventListener('click', copyErrorText);
$('runDiagnosticsFromError')?.addEventListener('click', refreshDiagnostics);
$('refreshDiagnostics')?.addEventListener('click', refreshDiagnostics);
$('refreshImageWorkflow')?.addEventListener('click', refreshImageWorkflowPlan);
$('copyImageWorkflowCommand')?.addEventListener('click', copyImageWorkflowCommand);
$('refreshSessionWorkflow')?.addEventListener('click', refreshSessionWorkflowPlan);
$('copySessionWorkflowCommand')?.addEventListener('click', copySessionWorkflowCommand);
$('createDesktopSession')?.addEventListener('click', createDesktopSession);
$('openDesktopSession')?.addEventListener('click', openDesktopSession);
$('useSessionOutput')?.addEventListener('click', useSessionOutput);
$('refreshDesktopSessionIndex')?.addEventListener('click', refreshDesktopSessionIndex);
$('runDesktopSessionDoctor')?.addEventListener('click', runDesktopSessionDoctor);
$('openDesktopSessionIndex')?.addEventListener('click', openDesktopSessionIndex);
$('importSessionArtifact')?.addEventListener('click', importSessionArtifact);
$('generateSessionReport')?.addEventListener('click', generateSessionReport);
$('openCustomerReport')?.addEventListener('click', openCustomerReport);
$('openEngineerReport')?.addEventListener('click', openEngineerReport);
$('generateDeliveryPackage')?.addEventListener('click', generateDeliveryPackage);
$('openDeliveryFolder')?.addEventListener('click', openDeliveryFolder);

$('closeRecoveryReport')?.addEventListener('click', hideRecoveryReport);
$('hideRecoveryReport')?.addEventListener('click', hideRecoveryReport);
$('openHtmlReport')?.addEventListener('click', openRecoveryHtmlReport);
$('copyRecoverySummary')?.addEventListener('click', copyRecoverySummary);

// ── Boot ──
try{
  applyI18n();
  setModeConfig('ntfs');
  setStep(1);
  renderRiskPanel();
  renderActiveSession();
}catch(err){
  setAS(`介面初始化失敗：${err}`,'error');
  void showAppNotice('介面初始化失敗', String(err));
}
