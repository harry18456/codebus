# CodeBus 資安與合規 Spec

> 對應：一般性 Agentic AI 安全規範（內部測試、不觸及外部系統、API 資料政策、稽核 trail）。
> 本文件是**實作期 checklist**，不是產品文案。實作時每項都要能 tick。

---

## 一、設計原則

1. **Local-first**：code 與產出均本地處理
2. **Least privilege**：Tauri fs scope 限縮到使用者選定資料夾
3. **Defense in depth**：多層防護（scanner 過濾 + sidecar 限制 + log 過濾）
4. **Default deny**：PII / secret 預設跳過，使用者可白名單放行
5. **Kill switch**：任何時候使用者可中止

---

## 二、合規對照表

> 左欄條目為一般性 Agentic AI 安全要求；右欄為 CodeBus 的實作對應。

| 類別 | 條文 | CodeBus 對應 |
|---|---|---|
| Do | 完全區隔環境 / 隔離網路 | 桌面 App 本機執行，僅外連 LLM API |
| Do | 低權限沙箱 | Tauri fs scope 限縮 + **D-017 Tool Sandbox**（`ensure_in_workspace`、無寫 tool、無 shell/exec）；Python sidecar 只處理指定資料夾 |
| Do | 完全模擬資料 | Demo repo 須為個人 / OSS 專案，與使用者所屬組織無關 |
| Do | 測試專用 API Key | 走環境變數，不寫死；實作時使用測試用 Key |
| Do | 遠端熔斷機制 | Cancel 按鈕 + `kill_switch.json` config 可禁 Agent |
| Do | 重要 OS 行為人為同意 | 選資料夾走 OS dialog；第一次啟動顯示授權說明 |
| Do | 受保護環境 + 防毒 | 使用者自負；PyInstaller 打包前測防毒誤報 |
| Don't | 不接觸組織內部系統/資料 | UI 警告「勿選組織內部機密 code」；demo 環境隔離 |
| Don't | 不探索/試探/攻擊網路 | Phase 1 只打 LLM API；Phase 2 Topic mode 要 robots.txt + URL 白名單 |
| Don't | 不用生產 API Key | 強制環境變數，從 `.env.example` 管理 |
| Don't | 不開放服務埠對外 | Sidecar bind `127.0.0.1` + 隨機 port + token auth |
| Don't | 不觸發防毒 | PyInstaller 測試 + 考慮 code signing |
| 惡意行為禁制 | 禁惡意程式 / DoS / 滲透 | 設計無此功能 |
| 惡意行為禁制 | 禁 Deepfake / 社交工程 | N/A |
| 惡意行為禁制 | 遵守相關法律（如刑法 358-362） | Topic mode 禁破付費牆、禁規避驗證 |
| 資料保護 | PII 去識別化 / 不送公有雲 LLM | **D-015 Sanitizer 三段防線**：Scanner 入庫 + Provider pre-flight + Q&A `add_to_kb`；placeholder 統一 `<REDACTED:kind#N>`（詳見 `sanitizer.md`） |
| 資料保護 | 稽核 trail 可供查驗 | **七層 JSONL 稽核** = workspace-level 六層（`sanitize_audit.jsonl` / `tool_audit.jsonl` D-017 / `kb_growth.jsonl` D-016 / `reasoning_log.jsonl` / `token_usage.jsonl` D-021 / `llm_calls.jsonl` D-022，完整 request/response post-sanitize）+ App-level 一層（`~/.codebus/authorization_audit.jsonl`，授權 D-008 + workspace 生命週期事件 D-025：`workspace_path_updated` / `registry_rebuilt` / `audit_log_initialized` / `workspace_tombstoned` / `workspace_deleted`）；UI 稽核 tab 六分頁（workspace-level），App-level 走 Settings → Audit |
| 資料保護 | 合法資料來源 | Phase 2 Topic mode 遵守 robots.txt |
| 資料保護 | API Key 保密 + 使用後銷毀 | 使用者端管理，專案結束由使用者清除 |
| 資料保護 | 限用模擬資料、不涉機敏名稱 | Demo repo 選擇時確認 |

---

## 三、實作要求

### 3.1 敏感資料處理（D-015 Sanitizer）

權威實作 spec 在 `sanitizer.md`。本節摘要重點讓合規 review 可快速對照：

**偵測範圍**
- **Secret**：`detect-secrets`（Yelp）+ 自刻 regex（AWS / GitHub / OpenAI / JWT / PEM / Generic API key）
- **PII**：Email、台灣手機（`09XX-XXX-XXX`）、台灣身分證（`[A-Z][12]\d{8}`）
- **內部識別符**：RFC1918 / RFC4193 / link-local IP、`.local` / `.internal` / `.corp` / `.lan` TLD
- **公司特定清單**：使用者 `~/.codebus/sanitizer.local.yaml`（不進 repo、不進對話）

**處理策略**
- **統一 placeholder**：`<REDACTED:kind#N>`，`kind` ∈ `email/phone/id/secret/ip/internal-domain/jwt/private-key/credential/suspect`；`N` 為**單檔 scope** index（同檔同值共用、跨檔不共用）
- **不做整檔跳過**（pre-D-015 設計已廢）：即使 `.env` 也是**內容過 Sanitizer 後進 KB**——高熵 value 會被替成 `<REDACTED:secret#N>`，教學仍能引用結構
- **不存 reverse mapping**：替換即不可逆
- **KB / reasoning_log / tutorial.md**：全存清理版

**觸發點（三段防禦）**
1. **Scanner 入庫前**（第一段，Module 1 執行）
2. **Provider pre-flight**（第二段，每次 LLM call 前）
3. **Q&A `add_to_kb` 寫入前**（第三段，D-016 連動）

**白名單**：`sanitizer.local.yaml` 有 `path_allowlist` / `filename_allowlist` / `pattern_allowlist` 三層，白名單命中仍記 audit，只是 pass_through 不替換。

**稽核**：`sanitize_audit.jsonl` 記類別與數量，**不記原文**。

**授權 modal**：首次選資料夾彈 modal 告知替換範圍，同意才 spawn Scanner（`sanitizer.md` §七）。

**測試 fixture**：`tests/fixtures/sanitizer/`（`real_secrets.txt` / `false_positives.txt` / `tw_pii.txt` / `mixed.py`）CI 必跑。

### 3.2 Agent 工具執行邊界（D-017 Tool Sandbox）

權威實作 spec 在 `tool-sandbox.md`。本節摘要：

- **檔案讀取**：限 `workspace_root` 子樹 + `.git/`（只讀），統一走 `ensure_in_workspace()` helper（resolve + `is_relative_to` 雙檢查）
- **檔案寫入**：**完全禁止**——tool registry 沒有寫 filesystem 的 tool
- **KB 寫入**：Agent 唯一能寫的是本地 Qdrant（`add_to_kb`，走 client 不透過 path）
- **Shell / exec / subprocess**：完全禁止；git metadata 用 `pygit2`（C binding，不 spawn subprocess）
- **網路**：只准走 Provider 層 LLM API + localhost Qdrant
- **稽核**：`tool_audit.jsonl` 記每次 tool 呼叫（成功 / path_escape / 參數），UI 稽核 tab 🔒 Tool Sandbox 分頁顯示
- **熔斷**：同 session 5 次 path_escape → 迴圈提早收斂 + UI 警告
- **Red team**：`tests/sandbox/attacks/` 每個 escape path 必被 `PathEscapeError` 擋下，進 CI

### 3.3 使用者白名單機制（Sanitizer / Sandbox 共用原則）

- 使用者可在 `~/.codebus/sanitizer.local.yaml` opt-in 白名單（path / filename / pattern 三層）
- 白名單命中**仍寫 audit log**（透明度），只是 pass_through 不替換
- UI 顯示「已 pass_through 的命中」清單，可隨時撤回
- Sandbox 層無白名單機制（workspace_root 是唯一可讀區，不開 opt-in）

### 3.4 Python Sidecar 安全

**網路**
- Bind `127.0.0.1` only，**不可** `0.0.0.0` / `::`
- 啟動時隨機 port（`0` 讓 OS 分配）
- Port 透過 Tauri command 回傳給前端，不寫 config 檔

**認證**
- 啟動時生成 random token（32 bytes hex）
- Token 透過環境變數傳給 sidecar（不寫檔）
- 前端每個 request 帶 `Authorization: Bearer <token>`
- Token 驗證失敗 → 拒絕 + log + 計數，達閾值 kill sidecar

**生命週期**
- Tauri App 關閉時 SIGTERM sidecar；5 秒後還在就 SIGKILL
- 啟動時先 health check，失敗重試 3 次然後放棄
- Crash 時前端顯示「Agent 服務異常」，不自動重啟（避免 loop）

**IPC Schema**
- JSON 明確定義，Pydantic 驗證
- 拒絕未知欄位（`extra="forbid"`）
- Request / Response 有 schema version

### 3.5 Tauri fs scope

**`tauri.conf.json` 設定範例**
```json
"fs": {
  "scope": {
    "allow": ["$APPDATA/codebus/**"],
    "deny": ["$HOME/.ssh/**", "$HOME/.aws/**"]
  }
}
```

- 使用者選定資料夾後**動態加入 scope**（Tauri 2.0 支援 runtime scope）
- 前端 JS 永遠不能直接讀系統檔，統一走 invoke command
- Command 裡再 validate 路徑是否在 scope 內

### 3.6 API Key 管理

**儲存**
- 第一次啟動要求使用者輸入 LLM 供應商 API Key
- 存 OS keychain（Tauri 有 plugin-stronghold 或 plugin-keychain）
- **絕不**存純文字 config / .env

**使用**
- Sidecar 啟動時從 Tauri pass 進去（環境變數）
- 記憶體中使用完儘快清（避免 memory dump）
- log 任何地方都不能出現 key

**Repo 規範**
- `.gitignore` 列入 `.env` / `secrets.*` / `*.key`
- `pre-commit` hook 掃 git diff 防止誤 commit

### 3.7 Kill Switch（遠端熔斷對應）

**三層機制**

1. **UI Cancel**（D-008）：使用者隨時可中斷當前 Agent 執行
2. **Config kill switch**：`~/.codebus/kill_switch.json`
   ```json
   { "disabled": true, "reason": "incident-response" }
   ```
   啟動時讀，為 true 就拒啟 sidecar
3. **Hard kill**：使用者直接關 App → Tauri 確保 sidecar 跟著死

**文件化**：README 或 help 說明「如何緊急關閉」

### 3.8 First-run 授權

首次啟動顯示 modal：

```
CodeBus 會：
  ✓ 讀取你選定的資料夾內容（不讀其他位置）
  ✓ 透過 LLM API 呼叫 LLM / Embedding 服務
    （你的 code 片段會傳到 LLM 供應商）
  ✓ 將產出教材、進度、知識庫索引存本地

CodeBus 不會：
  ✗ 上傳完整 codebase 到雲端
  ✗ 讀取你指定以外的資料夾
  ✗ 送敏感檔案（.env / 金鑰 / PII）— 這類會自動過濾

[我了解並同意]  [取消]
```

- 同意才能繼續
- 同意紀錄本地存檔（ts + version）
- 未來版本若擴充權限範圍，重新 prompt

### 3.9 Topic Mode（Phase 2）合規

延後 Phase 2 實作時，補上：

- `robots.txt` 遵守（Python `urllib.robotparser`）
- URL 白名單（初始：官方文件站、Stack Overflow、GitHub、MDN、維基、主流部落格）
- 禁破付費牆 / 規避 login
- User-Agent 明示是教育用途
- Rate limiting（全域每秒 N req）
- 存檔時只存 URL 與摘要，不整頁快取

### 3.10 防毒誤報

**PyInstaller 打包**
- 使用 `--onefile` + `--noupx`（UPX 壓縮常被誤報）
- 每次 release build 先上 VirusTotal 掃，超過 3 家誤報就找 root cause
- 保留 un-packed build 當 fallback

**Code Signing**
- MVP 不做（成本高、耗時）
- 給使用者 README 說明「這是 dev build，未簽章屬正常」

### 3.11 Log 與審計

**log 過濾**
- `reasoning_log.jsonl` 寫入前走 PII/secret 過濾
- log 內的 file content snippet 截短（前 200 char）並遮蔽敏感

**審計 trail（Phase 2）**
- 每次 LLM 呼叫記錄：ts / model / token 數 / request hash（不存原文）
- 可 export 給資安單位核查

---

## 四、Demo / 提交前 Checklist

### 程式
- [ ] Demo repo 確認無機敏痕跡（名稱、註解、資料、git log）
- [ ] `.env` / `secrets.*` 加入 `.gitignore`
- [ ] 沒有 API Key / token 寫死在 repo
- [ ] Sanitizer 對 `tests/fixtures/sanitizer/` 四份 fixture 全過（CI 跑）
- [ ] Sandbox red team：`tests/sandbox/attacks/` 全部 escape path 被 `PathEscapeError` 擋下
- [ ] Sidecar bind 驗證：`netstat` 確認只監聽 `127.0.0.1`
- [ ] Token auth 測試：無 token request 被拒
- [ ] Kill switch 測試：config 設 disabled 時 App 不啟 sidecar
- [ ] First-run modal 出現且必須同意才繼續
- [ ] Cancel 按鈕可中斷探索

### 文件
- [ ] README 資安章節指向本文件
- [ ] 使用者手冊說明敏感檔會被跳過
- [ ] API 資料政策註記（LLM 供應商 API 的條款摘要）

### 打包
- [ ] PyInstaller 產物過 VirusTotal
- [ ] Installer 不含 API Key / 個人資料
- [ ] 移除 debug symbol / dev-only log

---

## 五、常見尖銳問答

**Q：敏感檔案怎麼防止送 LLM？**
A：D-015 Sanitizer 三段防線 — Scanner 入庫前 + Provider pre-flight + Q&A `add_to_kb` 寫入前。偵測到 secret / PII / 內部識別符統一替換成 `<REDACTED:kind#N>` 再進 KB / LLM 請求；**不做整檔跳過**（pre-D-015 的整檔 skip 設計已廢），即使 `.env` 內容也是過完 Sanitizer 後才入庫，高熵 value 被替掉、結構仍可引用。使用者在 UI 稽核 tab 可看到「每檔的替換統計」。

**Q：如果我選的資料夾含 `.env` 會怎樣？**
A：Scanner 會讀取並過 Sanitizer，`SECRET=xxxxx` 這類 value 被替成 `<REDACTED:secret#N>`，key 名與結構保留，教材可以說明「這裡應該設定 API key」而不洩漏值。想完全不讀某檔案，使用者可在 `.gitignore` / `sanitizer.local.yaml` 的 `path_blocklist` 排除；想讓某檔原樣入庫（如 sample config），走 `path_allowlist` 白名單，仍會寫 audit。

**Q：Python sidecar 會被攻擊嗎？**
A：Bind 127.0.0.1 random port + token auth，同機其他 process 也要知道 token 才能打。Token 只在記憶體，不寫檔。

**Q：LLM API 的 code 片段會被拿去訓練嗎？**
A：依 LLM API 供應商條款為準（README 會註明）。我們送的內容已經過 secret + PII 過濾，降低外洩風險。

**Q：萬一出事怎麼緊急停止？**
A：三層 kill switch — UI cancel / config 檔 disable / 直接關 App。有明文 incident response 步驟。
