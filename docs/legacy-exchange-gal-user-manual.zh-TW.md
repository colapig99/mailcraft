# MailCraft Legacy Exchange GAL — 安裝、使用及維護手冊（繁體中文）

> **適用範圍**：本 Fork 的 \`feature/exchange-legacy-ldap\` 實驗分支，Windows 桌面版本，Exchange 2003 / AD GAL。Exchange 5.5 僅有實驗性 LDAP schema，**尚未在現場完成驗證**。  
> **文件版本**：1.0（2026-11-10）。**只記錄目前的行為，不變更搜尋程式、LDAP Filter、TTL 或安全機制。**  
> **部署狀態**：試行／Draft PR，非正式生產版本。請以實際 Artifact 對應的 Commit 為準。  
> **公開倉庫注意**：本手冊只使用範例主機、範例網域及虛構帳號。實際 PDC IP、內部 DNS、使用者、GAL 清單、Bind DN、密碼和業務資料應另存於受權限保護的內部操作記錄，**不可提交到公開 GitHub**。

## 1. 系統架構與功能界線

本分支使用 IMAP 收信、SMTP 寄信；LDAP 只用於「企業 GAL 通訊錄搜尋」。它不是 EWS、CalDAV、MAPI/RPC，也**不會**透過 LDAP 讀取個人的 Outlook Contacts、Calendar、Tasks 或信箱內容。

| 功能 | 使用的來源／協定 | 狀態 |
| --- | --- | --- |
| 收信、資料夾及離線郵件 | IMAP 與 MailCraft 本機資料檔 | 取決於伺服器支援 |
| 寄信 | SMTP | 取決於伺服器支援 |
| Exchange 2003 GAL | AD LDAP v3；Windows 可使用 Negotiate + Signing/Sealing | 曾於現場成功 |
| Exchange 5.5 GAL | 舊版 Exchange Directory LDAP；schema 可能不同 | 尚未現場驗證 |
| GAL-only 自動完成 | LDAP 目錄搜尋結果 | 只限制建議，不限制手動輸入 |
| 本機 Contacts 與郵件出現過的地址 | MailCraft 本機資料 | 與 GAL 分開 |
| 自動回覆 | ManageSieve（現有郵件模組） | 舊版 Exchange 無此服務時不可用 |
| Exchange 原生 Calendar／Free-Busy／MAPI | 未新增 | 不可視為已支援 |

**目錄主機不等於 Exchange 郵件主機。** 設定 LDAP 時要填實際有 AD 目錄服務的 DC／GC，而不是把 Exchange Server 郵件主機名稱直接當成 LDAP 服務位置。

## 2. 安裝前的檢查清單

1. 試點電腦可連線到內網 DC LDAP Port；以一般網域使用者登入 Windows。
2. Windows PowerShell 5.1 / 系統提供的 \`powershell.exe\` 可執行；有 \`System.DirectoryServices.Protocols\`。
3. Windows 網域登入憑證能以 Negotiate 與 AD 建立支援 Signing + Sealing 的 LDAP 連線。
4. 既有 MailCraft IMAP／SMTP 已能基本收發；帳號與 LDAP 設定應分開驗證。
5. 取得此 Fork 的 **成功建置（Success）的 Windows Artifact**，並保留 Run URL、Commit SHA、SHA-256、安裝日期。
6. 停止舊版 MailCraft，確認工作管理員沒有其他 \`mailcraft.exe\` 正在使用同一資料目錄，備份設定與資料。
7. **不要**將 Domain Administrator 密碼設定為未加密 LDAP Simple Bind；正常 Windows Integrated 模式無需 Bind DN 和 LDAP 密碼。

### 2.1 取得正確的 Windows 測試版

從 Fork 的 [Windows build workflow](../.github/workflows/ldap-windows-build.yml) 及其 GitHub Actions 成功 Run 下載 **該 Run 的** \`mailcraft-legacy-ldap-windows-poc-<run-number>\` Artifact。切勿將較早 Run 的 ZIP 當成新版本。Windows Artifact 為**未簽署、僅供內部試行**的 EXE，並非官方 MailCraft Release；下載並不代表符合正式部署要求。

請記錄：

| 項目 | 值（由技術員填寫，內部保存） |
| --- | --- |
| 測試/部署日期及負責人 |  |
| GitHub Run URL、Git Commit SHA |  |
| 下載 Artifact 名稱、ZIP SHA-256 |  |
| 測試電腦名稱／OS／是否加入網域 |  |
| EXE 版本（執行 \`mailcraft.exe --version\`） |  |
| 使用模式（一般／Portable／--data） |  |
| LDAP 服務、Base DN（不要寫入公開 issue） |  |
| IMAP/SMTP 收發驗收結果 |  |
| GAL 用戶姓名測試（記錄匿名化 ID） |  |

**PowerShell 取得 SHA-256：**

\`\`\`powershell
Get-FileHash "C:\Path\To\Downloaded\mailcraft-poc.zip" -Algorithm SHA256
\`\`\`

可用 \`mailcraft.exe --version\` 確認執行檔版本；若已經有舊程序佔用相同資料目錄，重開另一份 MailCraft 可能只喚醒舊視窗，導致誤判新版本已啟動。

## 3. 安裝模式及資料儲存位置（已核對原始碼）

### 3.1 一般模式（Windows）

**設定根目錄**：\`%APPDATA%\MailCraft\`（Windows 環境變數 APPDATA 的位置，通常在使用者 Roaming AppData）。  
**帳號清單**：\`%APPDATA%\MailCraft\accounts.json\`。內有伺服器位址、帳號、IMAP/SMTP、LDAP URL、Base DN、schema、Windows Integrated 等設定，**不包含 LDAP 或郵箱密碼**。  
**程式日誌**：\`%APPDATA%\MailCraft\mailcraft.log\`；可能包含帳號 ID、錯誤、伺服器名稱、路徑等敏感診斷資料。  
**郵件資料檔（預設）**：Windows 使用者 Documents 目錄下的 \`MailCraft Files\`，例如 \`Documents\MailCraft Files\<account-address>.mailcraft\`。這是以帳號命名的本機資料檔，郵件和索引可能很大，**不是** \`accounts.json\`。  
**已搬移的資料檔**：\`%APPDATA%\MailCraft\location.json\` 可能指向其他絕對路徑；\`move-pending.json\` 表示下次啟動待執行搬移。不要只憑 Documents 是否有檔案認定資料遺失。  
**郵件/LDAP 密碼**：一般模式使用系統鑰匙圈／憑證管理；實際秘密資料不在 \`accounts.json\`。重新登入 Windows 或換電腦之後，可能需要重新輸入或重新授權。Windows Integrated LDAP 使用目前登入的網域身分，不需另存 LDAP 密碼。

### 3.2 Portable 模式

當 EXE 同一目錄存在 \`portable.txt\` **或** \`MailCraft.portable\` 標記檔，且該目錄可寫入時，MailCraft 將資料集中放在 \`<EXE目錄>\MailCraftData\`，包含帳號清單、程式狀態、日誌及帳號郵件資料。**Portable 模式的密碼及登入秘密資訊只保留在記憶體中，重啟需重新輸入**；不依賴系統鑰匙圈作為持久密碼庫。

若標記檔存在但沒有寫入權限，Portable 可能自動失效並改用一般使用者資料目錄；這是支援人員需要優先檢查的情況。以完整路徑執行 EXE 不一定就代表 Portable：必須有標記檔、寫入權限，且未被 \`--data\`／\`MAILCRAFT_DATA\` 覆蓋。

### 3.3 自訂資料位置

\`mailcraft.exe --data <絕對路徑>\` 或環境變數 \`MAILCRAFT_DATA\` 可指定設定根目錄。原始碼中 \`--data\` 優先於環境變數及 Portable 的自動偵測；\`--sample\` 使用獨立的 sample 測試位置。自訂位置不一定與一般 Documents 資料路徑相同，應記錄啟動命令及實際檔案位置。

**Windows 只讀定位命令：**

\`\`\`powershell
$base = Join-Path $env:APPDATA 'MailCraft'
"Config root: $base"
"Accounts file exists: $(Test-Path (Join-Path $base 'accounts.json'))"
"Log exists: $(Test-Path (Join-Path $base 'mailcraft.log'))"
"Moved-mail pointer exists: $(Test-Path (Join-Path $base 'location.json'))"
"MAILCRAFT_DATA: $env:MAILCRAFT_DATA"
Get-ChildItem -LiteralPath $base -Force -ErrorAction SilentlyContinue |
  Select-Object Name, Length, LastWriteTime
\`\`\`

> 此命令只列出檔名與大小，**不要**把整份 \`accounts.json\`、日誌、郵件資料檔或有個資的資料夾清單貼到公開 GitHub Issues。

## 4. Exchange 2003 AD GAL 設定步驟

1. 先在 MailCraft 新增可用的 Exchange 2003 郵箱：依管理員核定的 IMAP 主機、Port、認證方式與 SMTP 主機、Port 設定。
2. 在 Windows 上以**一般網域帳號**登入，確認能連接內網 LDAP。切勿以 Domain Administrator 作為預設測試帳號。
3. 開啟 **File → Info → Account settings → Enterprise directory (LDAP)**（選項名稱可能因語言而異）。
4. 勾選 **Use LDAP organisation directory**。
5. Directory schema 選 **Exchange 2003 / AD**。
6. LDAP URL 輸入 \`ldap://<directory-controller-fqdn-or-ip>:389\`，Base DN 例如 \`DC=example,DC=com\`。須由系統管理員確認這是目錄主機及正確搜尋範圍。
7. 啟用 **Windows Integrated (current domain user, Negotiate + signing/sealing)**。
8. **關閉** Legacy unencrypted anonymous；**Bind DN 留空**；**LDAP 密碼留空**。
9. 儲存後，在撰寫郵件的 To/Cc/Bcc 輸入至少三個字元，停下輸入，等候 GAL 搜尋。
10. 若未找到先檢查「搜尋中／失敗／沒結果」訊息；不要直接推論伺服器沒有該員工。

**注意**：\`ldap://\` 的 Windows Integrated 模式是透過 Windows SSPI Negotiate 並要求 LDAP Signing + Sealing；**不等同於** LDAP 明文 Simple Bind。服務端仍屬舊版，應僅透過公司內部可信網路存取並按管理員政策限制埠。Windows Integrated 不適用於 Linux/macOS。

### 4.1 其他連線模式

| 模式 | 使用條件 | 警告 |
| --- | --- | --- |
| \`ldaps://\` | 有有效伺服器 TLS 憑證 | 驗證憑證、主機名稱及信任鏈 |
| \`ldap://\` + STARTTLS | 目錄服務支援 TLS | 不應在 TLS 失敗時默默降級 |
| \`ldap://\` + Windows Integrated | Windows Domain + SSPI Negotiate + Signing／Sealing | 推薦本環境採用，不需要儲存 LDAP 密碼 |
| \`ldap://\` + Legacy Anonymous | 管理員明確允許匿名搜尋 | 搜尋與回覆可能明文暴露；必須隔離網路 |
| \`ldap://\` + 管理員 Simple Bind | **不支援／不建議** | 禁止明文傳送有權限的密碼 |

### 4.2 Exchange 5.5 特殊注意

Exchange 5.5 使用不同的 Directory Service、Search Base 與欄位命名，可能只支援舊版 LDAP 行為。本分支的 \`Exchange 5.5\` schema **沒有**在現場完成端到端驗證；不能直接複製 AD 的 \`DC=...\` Base DN。應先由管理員以只讀方式核對實際 LDAP Server、Port、Base DN、目錄版本和認證能力，再單獨做相容性測試。

## 5. 收件人自動完成、GAL Only 與來源標示

- **GAL Only 未勾選**：同時建議來源於本機郵件索引、個人 Contacts 與 LDAP GAL。已確認的 GAL 結果會優先列出。
- **GAL Only 勾選**：**只顯示** LDAP GAL 回傳的候選地址；仍能手動輸入任何 SMTP 地址，**不是**收件域名限制或 DLP 安全政策。
- 來源徽記 **GAL** = 這次目錄查詢有匹配；**Contacts** = 儲存的個人聯絡人；**Mail** = 本機郵件紀錄中出現的地址，包括收到的寄件者；**Checking** = 正在確認該候選地址是否也存在於 GAL。
- 即使一筆地址的網域與本公司不同，也可能是 AD 內合法的外部 Mail Contact。**不要用 \`@example.com\` 之類規則推斷 GAL 成員身份。**
- 開始搜尋至少 3 字元；介面在最後一次輸入後約 350 毫秒才送出請求。含中文、別名、姓氏或 SMTP proxy 地址的搜尋結果取決於 AD 欄位內容及實際 Search Base。
- GAL 的來源是可查詢收件者候選集合，**不保證**與 Outlook 自訂 GAL View、Hidden from Address Lists、通訊群組展開及 ACL 顯示完全一致。

## 6. 效能數據、查詢策略與限制（請勿擅自變更）

下列數值核對自本分支原始碼 \`crates/net/src/ldap.rs\`、\`crates/net/src/ldap_windows.ps1\`、\`crates/engine/src/session/pim/directory.rs\` 和 Compose UI；它們是**實作參數，不是服務器實測 QPS**。

| 參數 | 現行數值 | 說明 |
| --- | --- | --- |
| GUI 輸入停止判定 | 350 ms | 合併連續鍵入的查詢 |
| 最短自動搜尋 | 3 字元 | 不含明確強制查詢例外 |
| 查詢長度上限 | 128 字元 | 超出拒絕 |
| 優先查詢 | AD 前綴搜尋，例如 \`displayName=query*\` | 嘗試減少前置萬用字元 |
| 子字串回退 | 前綴搜尋成功但 0 筆，且輸入 **至少 5 字元** | \`*query*\` 可能消耗較多 PDC 資源；中段短字串不一定能找到 |
| 結果上限 | 50 筆／查詢 | 限制傳輸量；不是整份 GAL |
| LDAP 伺服器搜尋逾時 | 每次約 8 秒 | 一次前綴加一次回退可能是兩次伺服器搜尋 |
| Windows helper 回覆逾時 | 約 18 秒 | 超時重建 worker，下次再查 |
| 成功結果快取 | 300 秒 | 同一查詢及帳號不必重複查 |
| 空結果／失敗快取 | 30 秒 | 避免失敗重試風暴 |
| 每 Session 快取鍵上限 | 200 | 滿額淘汰已完成的舊項目 |
| Windows SSPI 連線 | 持續重用（每工作執行緒） | 不應每查詢都啟動 PowerShell、Bind |
| GAL 全量同步／定期大量下載 | **沒有** | 查詢是依使用者輸入按需觸發 |

**實測基線**：現場已使用一般網域帳號從 Exchange 2003 AD 查得已知員工；客戶端也已回報搜尋結果正確。**尚未具備正式 100 人並行壓測、P95 延遲、PDC CPU/磁碟指標的測量紀錄**；不能聲稱 100 人一定安全。

## 7. 升級、備份及還原

### 7.1 升級之前

1. 記錄當前 EXE 的完整版本、Commit、ZIP SHA-256、資料模式和路徑。
2. 從 MailCraft 正常退出，確認沒有同一根目錄的執行個體。
3. 備份整個設定根目錄（例如 \`%APPDATA%\MailCraft\`），**以及**實際郵件資料目錄（通常 Documents 下的 \`MailCraft Files\`；若 \`location.json\` 有設定則依其指向位置）。
4. 備份 Portable 的整個 \`MailCraftData\`，但不要誤以為其中包含持久保存的密碼。
5. 將備份放到公司核准的安全位置；限制讀取權，必要時加密，並按資料保存政策管理。
6. 將新版 ZIP 解壓到新的測試目錄；不覆蓋原本穩定可用的 EXE。使用相同資料目錄前先確保沒有其他 MailCraft 執行。
7. 確認 IMAP／SMTP 功能及 GAL 查詢後才把新版本列為試點標準版。

### 7.2 回復舊版

若新版異常：先退出程式、保留錯誤日誌及問題時間，將 EXE 切回先前通過驗收的版本；若出現資料格式或帳號問題，應先在隔離位置復原備份，不能在未確認資料格式相容性前直接反覆啟動新舊版本共用同一份可寫資料。

**只備份 \`accounts.json\` 不等於備份郵件**；帳號密碼也不會隨 JSON 自動遷移。切勿把備份上傳到公開 GitHub Issue。

## 8. 支援入口與故障排除

請從 [支援 Runbook](./legacy-exchange-gal-support-runbook.zh-TW.md) 依症狀處理。典型分類：

| 症狀 | 第一檢查點 |
| --- | --- |
| 下載了新 ZIP，畫面仍是舊版 | Run/Commit 是否相符、舊 \`mailcraft.exe\` 是否仍執行、資料目錄是否相同 |
| 設定重新安裝後保留／消失 | 一般、Portable、\`--data\`、\`MAILCRAFT_DATA\`、寫入權限 |
| GAL Only 沒結果 | 是否至少 3 字元、等候請求、Base DN、目錄服務、是否查詢失敗 |
| GAL 回報授權失敗 | Windows 網域身份、Signing/Sealing、DNS、AD 權限 |
| 有些地址只標 \`Mail\` | 它可能來自已收到的寄件人或舊郵件索引，不代表一定是 GAL |
| PowerShell 視窗彈出 | 確認使用包含隱藏持久 worker 修正的新版 Artifact |
| 自動回覆顯示 \`no ManageSieve\` | 舊 Exchange 不提供 ManageSieve，不是 GAL 連線失敗 |
| 查詢很慢或 PDC 資源高 | 檢查快取／搜尋條件／模糊回退、同時查詢數、PDC PerfMon |
| 郵件 \`Access denied\` | IMAP 操作、資料夾權限、伺服器拒絕資訊；不要先歸咎 LDAP |

## 9. 變更控制與驗收

**目前搜尋策略保持原狀。** 若日後要修改 3 字元、350ms、300s TTL、50 筆、前綴／回退規則、Windows Worker 行為，必須另開 Issue／PR、記錄原因、回歸結果與比較前後的 PDC 資源使用數據；本文件更新不應修改這些參數。

最低驗收：
- 一般員工 Windows 身分可查 GAL、可見中文與英文名字，無 Administrator LDAP 密碼。
- 混合模式來源標籤與 GAL Only 行為可重現；已知員工查詢成功。
- 連續重複查詢不產生新的 PowerShell 視窗；後續請求重用連線。
- IMAP／SMTP 正常收發；錯誤可與 LDAP 問題分開識別。
- 日誌、帳號設定和郵件資料可以定位、備份、還原。
- Windows CI、Rust 編譯、Clippy、單元測試通過，且測試 EXE 的 Commit 與 CI 一致。
- Exchange 5.5 獨立驗證前不得標示為正式支援。
- 5、20、50 人逐級試點時，收集基準與尖峰數據；不能用單一 Task Manager 截圖當成 100 人負載測試。

**後續文件**：[技術支援、備份還原、試點量測 Runbook](./legacy-exchange-gal-support-runbook.zh-TW.md)；[現有 LDAP 技術設計](./ldap-legacy-exchange.md)。
