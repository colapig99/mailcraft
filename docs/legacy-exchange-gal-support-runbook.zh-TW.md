# MailCraft Legacy Exchange GAL — 技術支援、部署與效能試點 Runbook

> **繁體中文／內部技術支援操作基準**。適用於本 Fork 的 \`feature/exchange-legacy-ldap\`，不代表上游官方支援。  
> **版本：2026-11-10，v1.0。** 本文件只增加操作說明，**沒有修改 LDAP 搜尋或快取程式**。  
> 務必使用受限制的公司內部表單保存：機器名稱、PDC IP、內部 Base DN、密碼、員工資料、完整 mailcraft.log、郵件和備份。公開 GitHub Issue 僅填去識別化結果。

## A. 支援台快速處理流程

1. 先問清楚是 **MailCraft 啟動／資料位置**、**IMAP 收件**、**SMTP 發信**、**LDAP GAL**、**收件人來源**、還是**不支援的 ManageSieve 自動回覆**。不要把所有錯誤混成「Exchange 連線問題」。
2. 核對版本：\`mailcraft.exe --version\`、下載 Artifact 的 GitHub Run URL、Commit SHA、ZIP SHA-256。不要使用只看檔名、不看 Run 的「最新」判定。
3. 判斷資料模式：是否有 \`portable.txt\`／\`MailCraft.portable\`、\`MAILCRAFT_DATA\` 或 \`--data\`。確認執行檔目前讀取哪個 \`accounts.json\`。
4. 如果是 GAL：確認 Windows 登入者已加入網域，使用 Windows Integrated 且沒有填 LDAP Bind DN／Password。
5. 在 MailCraft 新郵件 To 欄查 **已知存在** 的完整英文姓名、Alias、中文名稱；至少 3 字元，停止輸入以待查詢。核對「搜尋中／沒有結果／查詢失敗」及來源標示。
6. 對照單獨的只讀 LDAP 探針 \`scripts/Test-LegacyGal.ps1\`（隔離公司內網、一般員工權限），以區分伺服器／AD 搜尋與 MailCraft UI／子程序問題。
7. 如果仍有問題，從**正確的資料根目錄**收集 \`mailcraft.log\` 的相關時段，移除姓名、Email、主機/IP、帳號、任何憑證後才提供外部開發支援。
8. 任何修正先在試點電腦驗證，不在生產 PDC 修改 Directory Schema、全域 LDAP 設定或安全策略。

## B. 常用檢查命令（Windows PowerShell）

### B1. 版本、進程、資料目錄（安全只讀）

\`\`\`powershell
# 在 mailcraft.exe 的資料夾中執行（不保證 GUI 版本含 Git Commit；另記 Run URL）
.\mailcraft.exe --version

Get-Process mailcraft -ErrorAction SilentlyContinue |
  Select-Object Id, ProcessName, StartTime, Path

$base = Join-Path $env:APPDATA "MailCraft"
"Default app folder: $base"
"MAILCRAFT_DATA override: $env:MAILCRAFT_DATA"
"Accounts: $(Test-Path (Join-Path $base 'accounts.json'))"
"Log: $(Test-Path (Join-Path $base 'mailcraft.log'))"
"Moved mailbox pointer: $(Test-Path (Join-Path $base 'location.json'))"
Get-ChildItem -LiteralPath $base -Force -ErrorAction SilentlyContinue |
  Select-Object Name,Length,LastWriteTime
\`\`\`

注意：若程序以 \`--data\` 啟動或 Portable marker 生效，上述預設 \`$base\` **可能不是實際根目錄**。應先用啟動捷徑／命令列及 EXE 旁的 \`MailCraftData\` 核對。不要直接複製整份 \`accounts.json\` 到公網。

### B2. 內網連通性與基本環境

\`\`\`powershell
whoami
# 在有 Test-NetConnection Cmdlet 的較新 Windows／PowerShell 上可使用（Windows Server 2003 沒有）
Test-NetConnection -ComputerName directory.example.com -Port 389
# 若較舊電腦沒有此 Cmdlet，改用公司核准的 TCP 連接工具；
# 這個命令只測 TCP Port，不代表 LDAP Bind 已成功。
\`\`\`

請注意舊版 Windows 沒有 \`Test-NetConnection\`，不可把「命令不存在」當成 Port 被防火牆阻擋。

### B3. AD LDAP 可用性（一般域用戶，唯讀）

從 Fork 的 \`scripts/Test-LegacyGal.ps1\` 檢視腳本後放在內部電腦，在 PowerShell 用一行執行：

\`\`\`powershell
powershell.exe -NoProfile -File "$HOME\Test-LegacyGal.ps1" -Server directory.example.com -Port 389 -BaseDn "dc=example,dc=com" -Query "knownemployee"
\`\`\`

腳本檢查匿名和目前登入 Windows 帳號的 Negotiate/Signing/Sealing；**匿名失敗可以是正常現象**，重點是 \`WindowsIntegrated: result=Success, count=...\`。請只回報 result、count、時間與去識別化錯誤，不貼人名／Email。

### B4. 區分 GAL 與本機郵件資料

| MailCraft 顯示 | 意義及操作 |
| --- | --- |
| \`Searching GAL…\` | LDAP 請求尚未完成；停止輸入、查看進程／網路，避免大量重試 |
| \`No GAL results...\` | 該次查詢完成且結果為空；試其他姓名、Alias、Base DN |
| \`GAL query failed: ...\` | 客戶端、LDAP 連線或權限錯誤；記錄可安全分享的類別 |
| \`GAL\` | 目前查詢對應到 LDAP 候選地址 |
| \`Contacts\` | 本機個人通訊錄來源 |
| \`Mail\` | 郵件中曾出現的地址，包括收件者和來信寄件者 |
| \`Checking\` | 尚待該次 GAL 判定 |
| \`no ManageSieve\` | 伺服器不提供該自動回覆介面，與 GAL 無直接關係 |
| \`server refused: Access denied\` | 需調查當時的 IMAP 或其他服務操作；不能僅憑這句認定 LDAP 失效 |

## C. 設定及存儲排查

### C1. Windows 一般模式檔案表

| 項目 | 預設或相對路徑 | 用途及支援注意 |
| --- | --- | --- |
| 設定根目錄 | \`%APPDATA%\MailCraft\` | 不等同 EXE 所在目錄 |
| 帳號資料 | \`accounts.json\` | LDAP URL、Base DN、schema、windowsIntegrated、IMAP/SMTP；**不含密碼** |
| 日誌 | \`mailcraft.log\` | 排查所需，含敏感診斷資料 |
| 郵件儲存位置指標 | \`location.json\` | 如果郵件資料已搬走，記錄目的地 |
| 未完成搬移 | \`move-pending.json\` | 下次啟動才執行，勿手動刪除 |
| 郵件資料 | \`Documents\MailCraft Files\*.mailcraft\` | 離線郵件及帳號資料，可能很大 |
| 一般模式機密資料 | Windows 憑證儲存 | 不能靠備份 \`accounts.json\` 移植密碼 |

### C2. Portable 與自訂目錄的特殊情況

- EXE 旁有 \`portable.txt\` 或 \`MailCraft.portable\`，且目錄可寫入：所有資料改存 \`<EXE>\MailCraftData\`。
- 若 Portable 標記旁目錄不可寫：程式可能回退到一般模式；先查 \`mailcraft.log\` 中的 portable 訊息。
- \`--data C:\Path\To\Data\`／\`MAILCRAFT_DATA\` 會覆蓋自動偵測，請記下啟動命令。
- Portable 密碼不持久保存；重啟重新登入／輸入密碼是預期現象。
- 一般模式換新版 EXE 不一定重置設定；如果要隔離測試，**使用獨立的資料目錄**，避免污染現有郵件與帳號配置。

## D. 備份／升級／還原標準作業程序（SOP）

### D1. 升級前備份

1. 取得使用者授權，記錄原 Artifact、Commit、SHA-256 及資料模式。
2. 讓使用者關閉 MailCraft。以 \`Get-Process mailcraft\` 確認該使用者沒有活動進程；避免資料庫正在寫入時拷貝。
3. 分別備份**設定根目錄**與**實際郵件資料夾**。如果 \`location.json\` 指向已搬動的路徑，另備份該資料夾。
4. Portable 模式備份整個 \`MailCraftData\`；不能假設備份包含密碼。
5. 將備份存入存取受控、加密且有版本日期的公司儲存區；不要放在共用公開網盤。
6. 標記資料備份完成時間，執行安全的完整性檢查（檔案數、容量、SHA-256）。

**可靠複製方式**（請在已確認 MailCraft 完全關閉後操作）：

\`\`\`powershell
$source = Join-Path $env:APPDATA 'MailCraft'
$destination = 'D:\ApprovedBackup\MailCraft-YYYYMMDD\settings'
New-Item -ItemType Directory -Path $destination -Force | Out-Null
Get-ChildItem -LiteralPath $source -Force | ForEach-Object {
    Copy-Item -LiteralPath $_.FullName -Destination $destination -Recurse -Force
}
# 另以相同方式備份實際郵件資料目錄；先確認 Documents 的重定向和 location.json。
\`\`\`

### D2. 換版測試

- 不覆蓋原 EXE，將最新版測試 ZIP 解壓到新位置。
- 核對該 EXE 的 GitHub Run SHA 與預期 Commit，避免下載前一次成功的舊包。
- 用同一 Windows 使用者登入，核對資料模式及既有 \`accounts.json\`。
- 開啟 MailCraft：測試 IMAP 收件、SMTP 寄信、GAL-only、混合來源標示、連續 3 次查詢和中文顯示。
- 檢查工作管理員：沒有反覆跳出 PowerShell 視窗；既有持續性背景 PowerShell 可能存在（帳號 worker 存活期間），不應每次查詢都新增。
- 記錄新舊版 P50/P95 延遲與 PDC 使用率；若變差先停止擴大部署。

### D3. 回復／事故處理

- **先保存**故障發生時間和刪除敏感內容後的日誌。
- 退出 MailCraft，將 EXE 指回舊版 Artifact；若新版改變資料格式，先在**隔離位置**測試舊版與備份的相容性。
- 不直接刪 \`accounts.json\`、\`MailCraftData\`、\`.mailcraft\`、\`location.json\`。
- 不把密碼／完整郵件／內部 GAL 匯出寫進公開 GitHub PR。
- 若問題是 AD、DNS 或 Windows 登入服務變慢，優先回退擴大部署／暫停 GAL 測試，保護既有生產服務。

## E. 5 → 20 → 50 → 100 人的試點負載與測量

**這是建議試行流程，並非既定的性能保證。** 依公司允許的維護時段及監控能力安排，不要在生產 PDC 上做大量不受限制的壓力產生器。

### E1. PDC 觀察項目

- Windows Server 2003 PerfMon：\`Processor(_Total)\% Processor Time\`、\`Process(lsass)\% Processor Time\`（在該版本實際可用時）。
- \`Memory\Available KBytes\`、\`Memory\Pages/sec\`（需連續時間序列，單一尖峰可能正常）。
- \`PhysicalDisk\Avg. Disk sec/Read\`、\`Avg. Disk Queue Length\`（若可用）。
- NTDS／LDAP 搜尋速率及連線數計數器：**僅使用該 Windows 2003 主機實際提供的計數器**，避免套用現代版本不支援的項目。
- DNS、WINS、Windows 網域登入是否變慢；服務中斷／帳戶鎖定／異常事件。
- Client：第一次查詢時間、第二個不同名字查詢時間、相同名字再次查詢時間、PowerShell 數量、GUI 是否卡頓、錯誤率。

### E2. 實施方法

1. 維持現有 PDC 服務，先在沒有 MailCraft GAL 測試時連續記錄基線。
2. 5 名一般員工各測數個**已有的**姓名、Alias；每人重複相同查詢。
3. 擴至 20 人，記錄工作時間中的尖峰及低峰；查同一名字命中本機快取不會再向 PDC 發問，換不同 Client 不共用快取。
4. 記錄可觀察的實際 LDAP 工作量；正常一次前綴搜尋成功應為一次搜尋，未命中且至少 5 字元可能有**第二次**子字串搜尋。
5. 如 PDC CPU/磁碟／網域登入延遲明顯惡化，立即停止擴大並回退部署；**不**以修改 AD Registry、Schema 或伺服器認證政策作為第一處理方式。
6. 50 人試點通過後，才能評估 100 人的部署節奏。若容量不足，另行評估中央 Proxy 或設計變更；不在本文偷偷修改現有搜尋功能。

### E3. 每次測試記錄表（內部使用）

| 指標 | 基線 | 5 人 | 20 人 | 50 人 | 100 人 |
| --- | --- | --- | --- | --- | --- |
| 開始／結束時間、Build SHA | | | | | |
| PDC 平均／尖峰 CPU % | | | | | |
| lsass CPU % | | | | | |
| Available KBytes 最小值 | | | | | |
| Memory Pages/sec P95 | | | | | |
| Disk 延遲／佇列 | | | | | |
| LDAP request/search rate | | | | | |
| 客戶端首次查詢 P50/P95 | | | | | |
| 第二次不同姓名 P50/P95 | | | | | |
| 重複同姓名（快取）P50/P95 | | | | | |
| 查詢失敗率 % | | | | | |
| DNS／登入有無異常 | | | | | |
| 問題／處置／批准人 | | | | | |

### E4. 工作量情境表（**假設值，非 PDC 實測**）

| 每人每天不同姓名查詢 | 100 人的每日 Client 查詢 | 每日最少 LDAP 搜尋（假定全部快取未命中且前綴查有結果） | 未命中時可能多出的回退 |
| --- | --- | --- | --- |
| 5 | 500 | 500 | 視是否符合至少 5 字元及前綴 0 筆 |
| 20 | 2,000 | 2,000 | 同上 |
| 50 | 5,000 | 5,000 | 同上 |
| 100 | 10,000 | 10,000 | 同上 |

上述模型不計算本機快取節省的請求，也不把 8 小時平均化當成尖峰承載保證。舊 PDC 不可因平均每秒查詢低而跳過峰值測試。

## F. 常見問題對照

| 現象 | 可能原因 | 安全處理 |
| --- | --- | --- |
| \`GAL only\` 沒有員工 | 查詢未完成、alias 不同、Filter／Base DN、資料未納入 AD、LDAP 錯誤 | 先以已知名字執行讀取測試及探針，勿隨意放寬 AD 權限 |
| \`GAL search failed\` | Negotiate、網域登入、連線、PowerShell helper JSON/UTF-8、LDAP 回覆錯誤 | 記錄安全錯誤碼，檢查根目錄 log，分離 CLI／LDAP Probe |
| 只標 \`Mail\` | 本機歷史寄件者／收件者存在 | 這不是 GAL 已確認的證據 |
| 中文人名亂碼或搜尋無結果 | 編碼或資料來源欄位差異 | 核對 EXE Commit、PowerShell UTF-8、AD 回傳；勿更改 AD 內容 |
| PowerShell 視窗不停彈出 | 執行舊版 EXE 或 worker 管理錯誤 | 核對 Run/Commit，檢查是否有舊程序佔用資料目錄 |
| 密碼更新後失效 | 系統鑰匙圈或登入授權不同步 | 驗證 Windows Integrated 模式沒有 Bind DN；勿提交密碼截圖 |
| 重新下載 EXE 仍保留帳號 | 一般模式 \`%APPDATA%\MailCraft\` 設定延用 | 預期；必要時使用隔離測試資料目錄 |
| 重啟 Portable 後要求密碼 | Portable 機密資料只存記憶體 | 預期；不應改成明文密碼保存在 JSON |
| 自動回覆不支援 | 無 ManageSieve | 與 LDAP GAL 無直接關係 |
| \`Access denied\` | IMAP 權限或其他服務回覆 | 核對具體操作及日誌，勿先把 LDAP 當成原因 |
| GitHub 上編譯 warning | 第三方 Actions Node runtime 相容提示 | 檢查是否是 **Success**、Artifact 與 SHA 是否一致；與已建置 EXE 功能分離 |
| PDC CPU／DNS 反應變差 | LDAP 搜尋尖峰、子字串回退、原有系統負載 | 停止擴大試點、保存 PerfMon、按變更程序決定回退 |

## G. 回報給開發者的最小資料集

**公開 GitHub Issue：** 填功能分類、匿名化重現步驟、是否可重現、OS 主要版本、MailCraft Build/Commit、公開 GitHub CI Run URL、Safe LDAP Error Category、Count 或毫秒數等。

**只在內部工單／受控渠道提供：** 真正的 LDAP IP/主機、內網 DNS、Base DN、使用者帳號、員工名、Email、log 原文、醫療或商業內容、憑證、資料檔、備份。

### 回報模板

\`\`\`text
Issue: [啟動/郵件/LDAP/GAL-only/效能/自動回覆]
Build: [Git Commit / Run URL / ZIP SHA256]
Windows: [OS 版本 / 是否 domain-joined]
Mode: [一般/Portable/--data]（請勿列出實際內網路徑）
Steps: [1...2...3]
Expected / Actual:
LDAP mode: [WindowsIntegrated / TLS / Anonymous]
GAL query length and category: [例如 5-char alias，勿提供姓名]
LDAP result: [Success/count 或安全錯誤類別]
P50 / P95 elapsed ms (若測量):
mailcraft.log: [已去識別化的相關行，禁止憑證]
PDC metrics: [CPU/Memory/Disk/時間區間，不附內網識別資訊]
Impact: [單一使用者/5/20/50/100 pilot]
Rollback status:
\`\`\`

## H. 來源與維護責任

本 Runbook 的路徑與參數以原始碼為準：
- \`crates/store/src/location.rs\`：一般資料根目錄、Documents 位置及搬移指標。
- \`apps/mailcraft/src/portable.rs\`、\`apps/mailcraft/src/main.rs\`：Portable/override/日誌/啟動與實例控制。
- \`crates/store/src/lib.rs\`：\`accounts.json\`、帳號資料。
- \`crates/model/src/lib.rs\`、\`crates/ui-egui/src/dialogs/ldap.rs\`：LDAP 設定欄位與 UI。
- \`crates/net/src/ldap.rs\`、\`crates/net/src/ldap_windows.ps1\`：LDAP Filter、簽章封裝、Windows SSPI/helper 與查詢限制。
- \`crates/engine/src/session/pim/directory.rs\`：每帳號查詢快取與佇列。
- \`crates/ui-egui/src/reading/compose.rs\`：收件人搜尋、來源標籤與 GAL Only。
- \`.github/workflows/ldap-check.yml\`、\`.github/workflows/ldap-windows-build.yml\`：建置、測試及 Artifact。

文件應隨功能變更同步更新；若 GitHub Actions 未對最新 Commit 成功建置，不可將試驗性修改標示為正式支援。

另見：[使用及安裝手冊](./legacy-exchange-gal-user-manual.zh-TW.md)／[技術設計文件](./ldap-legacy-exchange.md)。
