# Sanitizer Spec — 敏感資料去識別化

> Module 1 Scanner 與 Provider pre-flight 共用的「清洗器」。
> 關聯決策：D-011（資安與合規）、D-003（Provider 抽象）。
> 對應 `docs/security.md` 的實作層。

---

## 一、範圍

Sanitizer 負責偵測並替換以下三類敏感內容：**Secret / PII / 內部識別符**。所有送往 LLM API（chat 或 embedding）的文字**必須**先過 Sanitizer。

### 偵測類別（MVP）

| 類別 | 項目 | 來源 |
|---|---|---|
| **Secret** | API key、JWT、PEM / SSH private key、DB 連線字串含密碼、`.env` 風格 KEY=value（高熵 value）、高熵字串 | 內建 `detect-secrets` + 自刻 regex |
| **PII** | Email、台灣手機（`09XX-XXX-XXX`）、台灣身分證（`[A-Z][12]\d{8}`） | 自刻 regex |
| **內部識別符** | RFC1918 / RFC4193 / link-local IP、通用可疑 TLD（`.local` / `.internal` / `.corp` / `.lan`） | 內建 regex |
| **內部識別符（使用者 config）** | 公司特定域名、hostname pattern、額外 secret pattern | 使用者 `sanitizer.local.yaml` |

### 不做（MVP）
- 中文姓名偵測（誤殺高）
- 信用卡、地址、員工編號（需人工清單）
- 產品代號 / 專案代號（需人工清單）
- ML-based PII（如 Presidio）— 依賴重，延後評估

---

## 二、工具選型（A2）

| 層級 | 工具 | 理由 |
|---|---|---|
| Secret | **`detect-secrets`**（Yelp） | 成熟、規則多、entropy + pattern 雙層，Python 原生 |
| PII（基礎） | 自刻 regex | 需求明確，pattern 短，Python `re` 夠用 |
| 內部識別符 | 自刻 regex + 使用者 config | 公開規則內建，公司特定規則走 config |
| 高熵字串通用偵測 | detect-secrets 的 `Base64HighEntropyString` / `HexHighEntropyString` plugin | **開但走「suspect」等級**（見四） |

**不用**：gitleaks（Go binary 整合複雜）、Presidio（ML 模型重）、scrubadub（功能不及自刻）。

---

## 三、架構與觸發點

### 三段式 sanitize（B1）

```
┌──────────────┐    ┌──────────────┐    ┌──────┐
│ 檔案原文     │───▶│ Sanitizer    │───▶│ KB   │
│ (Scanner)    │    │ pass 1       │    │ 清理版│
└──────────────┘    └──────────────┘    └──────┘
                          │
                          ▼
                    sanitize_audit.jsonl

                    ┌──────────────┐    ┌──────────────┐    ┌─────────┐
Agent / Generator ─▶│ Messages     │───▶│ Sanitizer    │───▶│ LLM API │
                    │ (含 prompt)  │    │ pre-flight   │    │         │
                    └──────────────┘    └──────────────┘    └─────────┘

                    ┌──────────────┐    ┌──────────────┐    ┌──────┐
Q&A Agent add_to_kb▶│ 新 chunk     │───▶│ Sanitizer    │───▶│ KB   │
                    │ (補查所得)   │    │ pass 3       │    │ 清理版│
                    └──────────────┘    └──────────────┘    └──────┘
```

- **Pass 1（Scanner 階段）**：檔案一進知識庫前全掃，替換後存清理版到 KB
- **Pass 2（Provider pre-flight）**：每次 LLM call 前再掃一次 messages（包含 Agent 產出的中間輸出、使用者輸入的 task 字串）
- **Pass 3（Q&A `add_to_kb` 寫入前，連動 D-016）**：Q&A Agent 要沉澱新 chunk 進 KB 時走同一層 sanitize，替換後才 embed + upsert
- 三段重複掃但 CPU 便宜，防禦層級加倍

### 儲存策略（B2）

| 資料 | 存什麼 |
|---|---|
| KB (Qdrant)（embedding + chunk） | **清理版**（永遠不含敏感內容） |
| `reasoning_log.jsonl` | **清理版**（Agent 看到什麼就記什麼） |
| `tutorial.md`（教材產出） | **清理版**（LLM 生成時只看到清理版，自然產出清理版） |
| 原始檔案 | **不額外儲存**，使用者原檔在原處；教材內要顯示 snippet 時走清理版 |
| Reverse mapping（placeholder → 原值） | **不存**，一旦替換即不可逆 |

**例外**：使用者在 UI 點「看原檔」按鈕時，App 直接開本機原檔（不經 LLM、不經 Sanitizer），因為是使用者看自己的資料，不算外洩。

### 稽核 log（B3）

`{workspace}/sanitize_audit.jsonl` 每行一 entry：

```json
{
  "ts": "2026-04-17T10:30:00Z",
  "pass": "scanner",
  "file": "src/config.py",
  "category": "secret",
  "subtype": "api_key_openai",
  "count": 2,
  "placeholder_ids": ["secret#1", "secret#2"]
}
```

**不記原文、不記 reverse mapping**。只給使用者 / stakeholder 看「替換了什麼類別、幾筆」。

Pre-flight pass 另記：

```json
{
  "ts": "...",
  "pass": "preflight",
  "provider": "contest",
  "operation": "chat",
  "bytes_in": 12400,
  "redactions": { "email": 3, "secret": 0, "internal_ip": 1 }
}
```

### Audit Mode 解鎖/鎖定事件（C+ · O-05 支援）

`/audit` 路由的「🔓 解鎖原值」按鈕觸發 — 使用者在稽核頁面主動解鎖才能看 raw diff。解鎖/鎖定成對寫 `sanitize_audit.jsonl`：

```json
// 主動解鎖
{
  "ts": "...",
  "pass": "audit",
  "event": "audit_unlock",
  "session_id": "sess_...",
  "audit_session_id": "auds_...",
  "actor": "local_user",
  "scope": "all_placeholders",
  "reason": "user_clicked_unlock"
}

// 配對的 re-lock
{
  "ts": "...",
  "pass": "audit",
  "event": "audit_relock",
  "session_id": "sess_...",
  "audit_session_id": "auds_...",
  "trigger": "user_manual_button",
  "duration_sec": 142
}
```

規則：
- `audit_session_id` 綁一次解鎖週期，`unlock` / `relock` 事件成對出現
- **連稽核事件本身都不記原值**、不記解鎖期間看了哪些 placeholder（只記解鎖/關閉事件本身）
- `trigger` 三種：`user_manual_button`（點 🔒 重新鎖定按鈕）/ `route_left`（切走 audit route 自動）/ `timeout`（預設 15 分鐘無操作，可 config）
- UI 對應：O-05 topbar banner「🔓 Audit mode · unlocked at 10:22 · duration 2m 22s · [🔒 重新鎖定]」的 duration 從 `audit_unlock.ts` 算起
- Audit mode 解鎖時，`GET /audit/sanitize/diff` 才會回 raw 內容；未解鎖時 raw 欄位為 null

---

## 四、處理策略

### Placeholder 格式（C1）

`<REDACTED:kind#index>`

- `kind` ∈ `email / phone / id / secret / ip / internal-domain / jwt / private-key / credential / suspect`
- `index` 在**單一檔案 scope 內**累增，讓同檔同值替成同個 id（例如同檔出現兩次 `john@example.com` 都變 `<REDACTED:email#1>`）
- 跨檔不共用 index（避免跨檔關聯）

**Suspect 等級**（高熵字串通用偵測）
- 不直接 redact 替換成 `<REDACTED:suspect#N>`
- 但在 audit log 標 `"subtype": "high_entropy_suspect"`
- UI 的稽核報告列出讓使用者 review
- **理由**：避免誤殺 UUID / commit hash / base64 asset

### Agent 對 placeholder 的理解（C2）

所有 Agent / Generator 的 system prompt 加這段：

```
你處理的內容可能包含 <REDACTED:kind#N> 格式的佔位符，
代表原本是敏感資料已被去識別化。規則：
1. 視其為不透明 placeholder，不要推測原值
2. 教材輸出應保留原樣，不要還原
3. 同檔內同 id（如 email#1）指同一實體，可用於邏輯推理
4. 如果決策依賴具體值，回報「需要原值」讓使用者介入
```

### 白名單（C3）

`sanitizer.local.yaml` 三種白名單：

```yaml
# 路徑白名單（glob）
path_allowlist:
  - "tests/fixtures/**"
  - "**/*.example.env"
  - "docs/examples/**"

# 檔名白名單
filename_allowlist:
  - ".env.example"
  - ".env.sample"
  - "*.fixture.*"

# Pattern 白名單（使用者知道這個字串不是 secret）
pattern_allowlist:
  - pattern: "^FAKE_API_KEY_FOR_TESTS_"
    reason: "測試 fixture 假 key"
```

**重要**：白名單內的命中**仍寫 audit log**（透明度），只是不做 redact。

### Test 檔降級策略（A1-3）

**預設：不跳過，一視同仁掃**。但透過上面的 `path_allowlist` 機制，使用者可宣告「這個 fixture 目錄的命中都是假資料」。不自動判斷 `.test.` / `__tests__/`，避免真 secret 漏接。

---

## 五、Config 管理（第 6 職責）

### 檔案位置

```
~/.codebus/sanitizer.local.yaml          # 全域預設
{workspace}/sanitizer.local.yaml         # 專案覆蓋（選用）
```

### Schema（完整）

```yaml
internal_domains:
  - "*.example.com"
  - "corp.example"

internal_hostname_patterns:
  - "^hq-.*$"

extra_secret_patterns:
  - name: "custom-token"
    regex: "^CT-[A-Z0-9]{32}$"
    category: "secret"

path_allowlist: [...]
filename_allowlist: [...]
pattern_allowlist: [...]

options:
  enable_entropy_suspect: true      # 開高熵偵測
  max_file_size_kb: 512             # 超過跳過並警告
  regex_timeout_ms: 5000            # 單檔正則逾時上限
```

### 載入行為
- App 啟動時載入 → 驗 schema（Pydantic）→ 失敗則 log 警告並用內建預設
- 使用者 config 與 App code 分離，git-ignore
- App 提供 UI「編輯內部清單」入口

---

## 六、失敗處理（D3）

| 情況 | 處理 |
|---|---|
| 正則逾時（單檔 > `regex_timeout_ms`） | 該檔標記失敗 → 進**檢疫區**（不進 KB、不送 LLM）→ UI 警告 |
| Config 讀不到 / schema 錯 | 用內建預設 → UI 警告 |
| 單檔命中超過 100 筆 | 視為可疑 → 標記但不擋 → 稽核頁高亮讓使用者 review |
| Pre-flight 抓到 Scanner 漏掉 | 替換 + log warning「Scanner pass 漏接：{file}」→ 繼續送 |
| 使用者 pattern regex 編譯錯 | UI 錯誤提示 → 該規則不載入 → 其他規則照常跑 |

**原則**：永遠偏保守（擋住比漏送安全）；漏掉時至少記 log。

---

## 七、使用者側（D1 / D2）

### 首次授權 modal（D1）

選資料夾後彈出：

```
┌─────────────────────────────────────────────────┐
│ CodeBus 將掃描：/path/to/repo                    │
│                                                 │
│ 送往 LLM 的內容會先經 Sanitizer 去識別化：       │
│   ✓ API keys / tokens / private keys            │
│   ✓ Email / 手機 / 身分證                        │
│   ✓ 內部 IP / 域名（依你的 config）              │
│                                                 │
│ 稽核報告會記錄每次替換的類別與數量（不記原文）。 │
│                                                 │
│ [ ] 我已閱讀並同意                              │
│                                                 │
│ [編輯內部清單] [開始掃描] [取消]                │
└─────────────────────────────────────────────────┘
```

使用者確認後才 spawn Scanner。

### Demo 稽核頁（D2）

UI 有「🛡️ 稽核報告」tab，顯示：

```
本次 session 總覽
  Scanner pass: 替換 42 筆
    Secret: 3 (api_key_openai × 2, jwt × 1)
    Email: 12
    Internal IP: 27
  Pre-flight pass: 補抓 1 筆（email）

每檔明細
  src/config.py          secret × 2, email × 1
  src/mqtt/client.py     internal_ip × 5
  ...（可展開）

[匯出 audit.jsonl]
```

Demo 時打開這頁 = 觀眾看到「這層真的在跑」。

---

## 八、效能與測試

### 效能目標
- 單檔 ≤ 100KB 在 50ms 內處理完
- 大型 repo（5000 檔，總 50MB）scanner pass 目標 < 30s
- 正則用 `re2`-like 避免 catastrophic backtracking（Python `re` 已夠，但 config pattern 要驗）

### 測試 fixture
`tests/fixtures/sanitizer/` 下準備：
- `real_secrets.txt`（內嵌真格式 fake value，驗偵測）
- `false_positives.txt`（UUID / commit hash / base64 asset，驗不誤殺）
- `tw_pii.txt`（台灣電話、身分證）
- `mixed.py`（綜合情境）

### Golden regression
Sanitizer 規則改動 → 跑所有 fixture → 對比預期 placeholder 輸出，diff 有變需人工 review。

---

## 九、實作順序（工期估）

| 優先 | 項目 | 工期 |
|---|---|---|
| P0 | Secret 偵測（detect-secrets 整合） | 0.5d |
| P0 | PII 基礎（email / 台灣手機 / 身分證） | 0.5d |
| P0 | 內建 IP / TLD regex | 0.5d |
| P0 | Placeholder 替換 + audit log 寫入 | 0.5d |
| P0 | Provider pre-flight 掛點 | 0.5d |
| P0 | Config 載入 + schema 驗證 | 0.5d |
| P1 | 路徑 / 檔名 / pattern 白名單 | 1d |
| P1 | 首次授權 modal（前端 + Tauri） | 1d |
| P1 | 稽核報告 UI | 1.5d |
| P1 | 失敗處理 + 檢疫區機制 | 0.5d |
| P2 | 高熵 suspect 等級 + UI review | 1d |

**合計 P0**：約 3 天；**P0+P1**：約 7 天。

---

## 十、MVP 明確不做

- 中文姓名偵測
- 信用卡、地址、員工編號
- 產品代號（unreleased product 命名清單）
- ML-based PII（Presidio）
- Reverse mapping（placeholder → 原值）
- 多 workspace 共用 config（每個 workspace 獨立）
- Sanitizer 規則熱更新（改 config 要重啟 App）

---

## 十一、Rule 統計與反饋欄位（O-05 RIGHT pane 支援）

O-05 placeholder card 展開後顯示 `matched: 23 · 0 flagged`，資料來源為 sidecar 的 rule-level in-memory counter（每 `session_id` 獨立）。

### 欄位定義

| 欄位 | 型別 | 語意 |
|---|---|---|
| `rule_id` | string | 規則穩定識別碼（如 `pii_email_v1` / `aws_access_key`），版本改動升號 |
| `matched` | int | 本 session 此規則總命中次數（Pass 1 / 2 / 3 合計，**跨檔加總**） |
| `flagged` | int | 使用者於稽核頁標記「這筆不該替換」的次數（反饋回路） |
| `last_matched_at` | string (ISO) | 最近一次命中時間（可選，UI 暫未使用） |

### Counter 更新時機
- 每次 Sanitizer 替換字串成功後，`matched += 命中次數`（同檔同值多次命中累加）
- `flagged` 欄位 MVP **恆為 0**；反饋 UI（使用者點「這不該替換」）留 post-MVP
- Workspace 關閉時 counter 清零（per-session）；不跨 session 累計

### 傳輸路徑
- `GET /audit/sanitize/diff` response 的 `rule_stats` 欄位直接回
- 不獨立 endpoint（避免前端雙打），因為 O-05 RIGHT pane 永遠與 diff view 綁同一個 file scope

### 為何不做跨 session 累計
- 避免儲存「這個 workspace 歷史上有過什麼 secret」的 metadata — 一旦外洩仍算資料暴露
- Counter 僅作為稽核頁的當次 demo / 說服力素材，不需要跨 session 一致性
