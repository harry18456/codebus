# Authorization Spec — 授權 Modal 與稽核

> O-01 Authorization Modal 的 spec canonical。
> 關聯決策：D-008（First-run UX）、D-011（資安）、D-015（Sanitizer）。
> 對應 UI：`design/O-01*.html`（由 Claude Design 產出）。

---

## 一、範圍

O-01 Authorization Modal 負責**使用者對 CodeBus 授權敏感操作前的明確同意**，涵蓋：

1. **Scope 授權** — 依 `workspace_type` 不同語意（見下）：
   - `folder`（**MVP 唯一支援**）：workspace_root 路徑授權（對齊 `tool-sandbox.md §五`）
   - `topic`（Phase 2）：URL 清單 / topic 關鍵字 + 爬取 sources 授權（見 `agent-explorer-spec.md §十二`）
2. **LLM provider 授權** — code 片段允許送往哪個 provider / model
3. **Sanitizer 類別 ack** — 使用者明確確認了解哪些類別會被去識別化
4. **Audit trail** — grant / deny / revoke 三類事件寫 `authorization_audit.jsonl`

**雙模對齊（D-002）**：`workspace_type` 作為 discriminator 欄位從 day 1 寫入 schema，MVP 只支援 `"folder"`，Phase 2 加 `"topic"` 不需 schema breaking change。與 `ExplorerTools` trait 抽象策略一致。

**不在範圍內**（避免 modal 膨脹）：
- Tool 白名單（runtime inline prompt，非 O-01）
- Budget ceiling（Settings 頁，非信任決策）
- 高風險 raw 解鎖（O-05 的 audit unlock，另一條路徑）

**定位**：O-01 = 授權 + 承諾書 + 能力預告三合一，不只是「同意 checkbox」。是 **Trust Layer 敘事的起點**（Act 1 第一幕）。

---

## 二、三種觸發場景

同一 `<AuthorizationModal>` 組件，情境靠 props 切。**不做三個 component**。

| 場景 | 觸發條件 | Modal 形態 |
|---|---|---|
| **(a) 首次冷啟動** | workspace 從未授權過 | **完整版**：信任劇場全秀（4 類 sanitizer + provider + 承諾條款全展開） |
| **(b) 換資料夾 / scope 升級** | workspace 已授權過、重開或切換 | **精簡版** / **完整版**（依分級規則，見 §三） |
| **(c) Revoke / rules version 升級** | 使用者主動 revoke、或 sanitizer rules major bump | **變體版**：紅框強調、單 CTA「review & re-grant」 |

### 情境文案差異

**(a) 首次冷啟動** · 完整內文
> 歡迎使用 CodeBus。以下是本次工作階段的授權範圍…

**(b) 換資料夾** · 精簡版
> 您先前授權過 CodeBus。此 workspace 的 sanitizer 命中皆屬已 ack 類別，請確認範圍。

**(c) 重新授權** · 變體
> Sanitizer rules 已更新至 v1.2.0 / 此 workspace 偵測到新類別。請 review 授權範圍。

---

## 三、Scope 升級分級規則（承 F 題對齊）

**核心比對邏輯**

```
acked_kinds   = { kinds from last grant's user_ack }
new_kinds     = { kinds detected in new workspace by Sanitizer dry-run }
rules_version = sanitizer rules version at current boot

if rules_version is MAJOR bump from last grant:
    → scenario (c) variant · 紅框重授權
    ∧ if new_kinds ⊄ acked_kinds: 合併 highlight 新類別（不連彈兩次 modal）

elif new_kinds ⊆ acked_kinds:
    → scenario (b) 精簡版 · 同類別、volume 變化不觸發完整版

elif new_kinds ⊄ acked_kinds:
    → scenario (b) 完整版 · 但只針對 `new_kinds \ acked_kinds` 要求明確勾選
       舊類別不要求 re-ack
```

**具體分級表**

| 變化 | 處理 | 原因 |
|---|---|---|
| 同類別 + 數量增加 | 精簡版 | Volume 變化不是新信任負擔，已 ack |
| 新類別出現（例 首次無 secret、新 folder 有）| 完整版 · 新類別 highlight · 舊類別不 re-ack | 最少必要變化 = 最小摩擦 × 最大稽核價值 |
| 同類別 + 數量**大幅**爆增（× 10 以上）| 精簡版 | 仍無新信任負擔；過度 gating 會稀釋敘事主軸 |
| Rules patch / minor bump | 不觸發 modal | 沒有新信任決策（見 §六） |
| Rules major bump | (c) 變體 | 有新信任決策（新 kind / 語意變更） |
| Provider 變更 | (c) 變體 | 資料流向改變，必重授 |
| 同時 rules major + new kind | 合併 (c) 變體 · 雙重 highlight | 避免連彈兩次 modal，一次收 |

---

## 四、Modal 結構（信任劇場第一張卡）

### 版面分區

```
┌──────────────────────────────────────────────────┐
│ Header: 授權 CodeBus · 開始 {workspace_name}     │
├──────────────────────────────────────────────────┤
│ Body:                                            │
│   [1] Scope 摘要（workspace path + 檔案數）        │
│   [2] Sanitizer 類別預告（4 色 · 預設展開）       │
│   [3] Hero line（加粗 + icon · 不進 LLM / KB）   │
│   [4] LLM Provider 行（基本 · 進階可展開）        │
│   [5] 三條承諾 checkbox（user_ack 對應）          │
├──────────────────────────────────────────────────┤
│ Footer: [先不啟用此 workspace]  [授權並開始]     │
└──────────────────────────────────────────────────┘
```

### 1. Scope 摘要
- `workspace_path`（絕對路徑，縮寫 home → `~`）
- `file_count` + `dominant_languages`（取自 `POST /scan` dry-run）

### 2. Sanitizer 類別預告 ⭐ 核心信任劇場
- **預設展開**（不摺疊 — 評審 30 秒要看完）
- 4 色分類顯示：🔴 secret (N rules) / 🟠 pii (N rules) / 🟡 internal (N rules) / ⚫ other (N rules)
- Hover 每類 → 顯示 rule_id 清單（對齊 `sanitizer.md §十一`），不展開 regex
- 小字連結：「了解 sanitizer 如何運作 → 稽核頁」（連 O-05，demo 時可順勢跳轉）

### 3. Hero Line
> **🛡️ 原值留在本機 sidecar，不進 LLM、不寫進 KB**

加粗 + icon，整個 Trust Layer 敘事的**一句話總結**，版面份量最重。

### 4. LLM Provider 行

**基本行**（預設顯示）
```
Provider: Anthropic · Claude Haiku 4.5 · outbound HTTPS → api.anthropic.com
```

**進階展開**（點「進階 ⌄」）
- API key 來源（env / keychain）
- Region / Endpoint 細節
- Outbound 網域白名單

**不顯示**：est cost（冷啟動沒 scan 過，估不準）、rate limit（非信任決策）

### 5. 三條承諾 Checkbox

對應 `user_ack` 的三個基礎 flag：

| Checkbox 文案 | user_ack flag |
|---|---|
| ☐ 我了解**原值留在本機**，不會離開這台機器 | `raw_stays_local` |
| ☐ 我了解**清理後內容進 KB**，原值**不寫 KB** | `no_kb_persist` |
| ☐ 我同意 CodeBus 送已清理內容到 `{provider}` | `outbound_to_{provider}` |

三項**全勾**才 enable「授權並開始」按鈕。

**新類別補 ack**（(b) 完整版）
- 頂部多一行紅字：「⚠️ 偵測到新類別：🔴 secret」
- 對應多一個 checkbox：「☐ 我了解此 workspace 含 **secret** 類內容，將被替換」
- 對應 flag：`new_kind:secret`

**Rules version bump 補 ack**（(c) 變體）
- 對應 flag：`rules_version:v1.2.0`

---

## 五、Event Schema · `authorization_audit.jsonl`

**位置**：`~/.codebus/authorization_audit.jsonl`（跨 workspace，App-level audit log）

**為何獨立 log 檔**：
- 與 `sanitize_audit.jsonl`（workspace-level）語意不同
- 授權事件跨 workspace（使用者在一個 App 授權多個 workspace）
- 稽核篩選（「這個 App 的所有授權紀錄」）走單一檔更乾淨

### 三事件

#### `grant_issued`

```json
{
  "ts": "2026-04-19T10:30:00Z",
  "event": "grant_issued",
  "session_id": "sess_abc",
  "workspace_id": "ws_timeline",
  "workspace_type": "folder",
  "workspace_source": {
    "path": "~/projects/timeline"
  },
  "scenario": "first_run",
  "scope": {
    "llm_provider": "anthropic",
    "llm_model": "claude-haiku-4.5",
    "outbound_endpoint": "api.anthropic.com"
  },
  "sanitizer_rules_version": "v1.2.0",
  "user_ack": [
    "raw_stays_local",
    "no_kb_persist",
    "outbound_to_anthropic"
  ]
}
```

**`workspace_type` 與 `workspace_source` 語意**

| `workspace_type` | `workspace_source` 形態 | 何時支援 |
|---|---|---|
| `"folder"` | `{ "path": "<abs_path>" }` | **MVP** |
| `"topic"` | `{ "query": "...", "seed_urls": [...], "domain_allowlist": [...] }` | Phase 2（D-002） |

**Phase 2 新增 topic 模式時**，新增 flag 範例：`outbound_to_topic_domains`（對應使用者明確 ack 外部爬取）、`topic_crawl_bounded`（明確 ack 爬蟲受 domain_allowlist 限制）。schema 層不需 breaking change。

**`scenario` 列舉**（P0 closed set）：`first_run` / `scope_reconfirm` / `scope_upgrade_new_kind`

> P1 deferred：`rules_version_bump`（rules major bump 觸發的單獨變體）/ `combined_version_and_kind`（合併情境）。在 P0 階段不接收這兩個值；引入時須走新 change 一併補對應 modal 變體與比對邏輯。

**新類別場景**（`scenario: "scope_upgrade_new_kind"`）
```json
{
  ...,
  "user_ack": [
    "raw_stays_local",
    "no_kb_persist",
    "outbound_to_anthropic",
    "new_kind:secret"
  ],
  "previous_acked_kinds": ["email", "internal_domain"],
  "new_kinds_introduced": ["secret"]
}
```

#### `grant_denied`

```json
{
  "ts": "...",
  "event": "grant_denied",
  "session_id": "sess_abc",
  "workspace_type": "folder",
  "workspace_source": { "path": "~/projects/timeline" },
  "scenario": "first_run",
  "reason": "user_cancelled"
}
```

**`reason` 列舉**：`user_cancelled`（點「先不啟用此 workspace」）/ `app_closed`（關 App 未決定）/ `dialog_dismissed`（ESC 等異常退出，MVP 不支援視為 app_closed）

#### `grant_revoked`

```json
{
  "ts": "...",
  "event": "grant_revoked",
  "session_id": "sess_abc",
  "workspace_id": "ws_timeline",
  "grant_ts": "2026-04-19T10:30:00Z",
  "trigger": "settings_revoke"
}
```

**`trigger` 列舉**：
- `settings_revoke` — 使用者從 Settings 主動撤回
- `rules_version_bump` — sanitizer rules major bump，舊 grant 自動作廢
- `provider_change` — provider 切換，舊 grant 作廢
- `workspace_deleted` — workspace 目錄不存在，grant 清理

---

## 六、`sanitizer_rules_version` 語意

**格式**：opaque 字串。**P0 不規範格式語意**——`grant_issued.sanitizer_rules_version` 一律 verbatim 等於呼叫時 `codebus_agent.sanitizer.RULES_VERSION` 的當下值。目前該常數為 `YYYY-MM-DD-N` date format（例：`2026-04-20-1`），由 `sanitizer/config.py` 集中決定，rules 改動時 bump（CLAUDE.md 不變式 9）。

**P0 行為**
- `grant_issued.sanitizer_rules_version` 只是審計欄位，**不做版本比對 / 不寫 meta.json / 不檢查 `last_acked_version`**
- 三個 P0 scenario（`first_run` / `scope_reconfirm` / `scope_upgrade_new_kind`）的觸發條件全部基於「current scan kinds vs last acked kinds」，與 `rules_version` 變化無關
- audit log 需要時可讀回 `sanitizer_rules_version` 字串做 forensics（純字面比對；不解析 semver / date）

**P1 deferred**（後續 change 落地時統一決定）
- 是否從 date format 切 semver `vMAJOR.MINOR.PATCH`、是否要寫 migration 把既有 audit log 的 date 字串轉換
- `~/.codebus/sanitizer_rules_meta.json` 記錄 `last_acked_version` 的機制
- 啟動時比對 `last_acked_version` vs `current_version` → 觸發 `rules_version_bump` modal 變體（對應 §五 P1 deferred 的兩個 scenario 值）
- 版本升級觸發策略表（PATCH / MINOR / MAJOR 對應觸發行為）

> 原則不變：使用者 ack 的是「**我同意這些 kind 類別會被替換**」。不改 kind 語意的 rules 升級 = 沒有新信任決策。但此原則的**自動觸發**邏輯整段 P1 才落地——P0 階段使用者必須透過 `scope_upgrade_new_kind`（kind 變化）或 Settings 主動 revoke（trigger=`settings_revoke`）才會重授。

---

## 七、Cancel / Deny 的 UX 規則

**Cancel 按鈕**：`先不啟用此 workspace · 返回選擇頁`

**行為**：
- 寫 `grant_denied` event (`reason: "user_cancelled"`)
- 關閉 O-01 modal
- **返回 R-00 Start Page**（見 `workspace-lifecycle.md §六`）— 不停留在 disabled 狀態
- 使用者可隨時重開 O-01 再試

**不做的事**：
- ❌ Blocking 不能 dismiss（過於強硬，無路可走）
- ❌ Workspace disabled 狀態（使用者會以為 App 壞了）
- ❌ 離線 demo 模式（精力不值得，屬於另一個產品）

**類比**：VS Code workspace trust — 不信任就退回選 workspace。

---

## 八、Sidecar Endpoints

✅ 已落實（`auth-flow` change，2026-04-27 起跑）。完整 request / response schema、status code、error code 對照見 [`docs/sidecar-api.md §三`](./sidecar-api.md)。本節保留歷史摘要供快速 lookup：

### `POST /auth/grant`
前端收到 modal 確認後呼叫。Sidecar 寫 `grant_issued`、依 `workspace_type` 初始化 ToolContext、return `session_id`。

```json
// Request (MVP · workspace_type: "folder")
{
  "workspace_type": "folder",
  "workspace_source": { "path": "~/projects/timeline" },
  "scenario": "first_run",
  "scope": { "llm_provider": "anthropic", "llm_model": "claude-haiku-4.5" },
  "sanitizer_rules_version": "v1.2.0",
  "user_ack": ["raw_stays_local", "no_kb_persist", "outbound_to_anthropic"]
}

// Response
{ "session_id": "sess_abc", "workspace_id": "ws_timeline", "granted_at": "..." }
```

**ToolContext 初始化差異**
- `folder` → set `ToolContext.workspace_root = <resolved abs path>`
- `topic`（Phase 2）→ set `ToolContext.workspace_topic = {seed_urls, domain_allowlist}`（新欄位，屆時補 `tool-sandbox.md §五`）

### `POST /auth/deny`
使用者點 Cancel 時呼叫。Sidecar 寫 `grant_denied`，不 spawn session。

### `POST /auth/revoke`
從 Settings 撤回授權。Sidecar 寫 `grant_revoked`、tear down session。

### `GET /auth/status`
查詢當前 session 是否有效授權。前端啟動時輪詢。

```json
{
  "has_active_grant": true,
  "session_id": "sess_abc",
  "workspace_id": "ws_timeline",
  "sanitizer_rules_version": "v1.2.0"
}
```

---

## 九、分工對齊（其他 spec 的職責）

| 職責 | 主 spec | 本文件 |
|---|---|---|
| Sanitizer 規則 / placeholder 格式 | `sanitizer.md` | 僅引用 rule_id 做 UI 顯示 |
| `workspace_root` / ToolContext | `tool-sandbox.md §五` | 引用其不可變約束 |
| LLM provider 選型 / 呼叫 | `llm-provider.md` | 引用 provider 名顯示 |
| 六層 audit log 全景 | `security.md §3` | `authorization_audit.jsonl` 為第七層，補進 |
| Sidecar endpoints 格式 | `sidecar-api.md` | §八 placeholder，實作期同步 |
| 視覺稿 | `design/O-01*.html` | 文案對齊，版面細節由 Design 主導 |

---

## 十、MVP 明確不做

- **Topic mode 授權**（`workspace_type: "topic"`）— schema 預留，Phase 2 實作，見 D-002
- Multi-user 授權（單使用者 local app）
- Role-based 授權（admin / viewer）
- 動態權限升降（Agent 臨時要求更高權限）— 需更多 UX 設計
- 遠端 grant 管理（企業中控台）— 與 local-first 衝突
- 授權過期自動 revoke（MVP 授權與 App session 綁）
- Biometric / hardware token 二次確認

---

## 十一、實作順序

對齊 `auth-flow` change（2026-04-27 起跑）的 P0 範圍與本 spec §五 / §六 校正：

| 優先 | 項目 | 工期 | 依賴 |
|---|---|---|---|
| P0 | `AuthorizationAuditLogger`（第七層 audit 唯一 writer，三事件 method） | 0.5d | — |
| P0 | 四個 sidecar endpoints（`POST /auth/grant` / `POST /auth/deny` / `POST /auth/revoke` / `GET /auth/status`） | 1d | audit writer |
| P0 | Scope 比對邏輯（current scan kinds vs `find_last_grant_for_workspace` 的 acked_kinds；不依賴 rules_version 變化） | 0.5d | Sanitizer dry-run（既有 `POST /scan` 的 `files[*].sanitize_stats`） |
| P0 | O-01 Vue 組件（3 P0 情境：`first_run` / `scope_reconfirm` / `scope_upgrade_new_kind`，共用 component + props 切換） | 1.5d | 視覺稿（`design/v1/03-grant.html` 已備） |
| P1 | Settings 頁 revoke 入口（endpoint 已是 P0；UI 入口屬 P1） | 0.5d | endpoints |
| P1 | Sanitizer rules version 偵測與 `~/.codebus/sanitizer_rules_meta.json` 機制 | 0.5d | sanitizer 模組 |
| P1 | Rules major bump 自動 trigger 邏輯（啟動時比對 `last_acked_version` → 觸發 `rules_version_bump` modal 變體） | 0.5d | version 偵測 |
| P1 | `combined_version_and_kind` 合併情境 modal 變體 | 0.5d | rules version 比對 |
| P1 | `RevokeRequest.trigger` 擴增 P1 值（`rules_version_bump` / `provider_change` / `workspace_deleted`） | 0.5d | rules version 比對 |

**合計 P0**：約 4 天（對齊本 change 工期）；**P0+P1**：約 6.5 天。

**Scope 比對邏輯細節（P0）**

```
acked_kinds = { flag.removeprefix("new_kind:")
                for flag in last_grant.user_ack
                if flag.startswith("new_kind:") }
new_kinds_in_request = { flag.removeprefix("new_kind:")
                         for flag in current_request.user_ack
                         if flag.startswith("new_kind:") }
new_kinds_diff = new_kinds_in_request - acked_kinds

if scenario == "first_run":
    requires last_grant is None
elif scenario == "scope_reconfirm":
    requires last_grant is not None AND new_kinds_diff is empty
elif scenario == "scope_upgrade_new_kind":
    requires last_grant is not None AND new_kinds_diff is non-empty
```

任何違反 → 400 `AUTH_INVALID_REQUEST`。詳見 `openspec/changes/auth-flow/specs/authorization-audit/spec.md::Requirement: scope upgrade detection reads the latest grant from audit log`。

---

## 十二、後續

- [x] 本 spec 建立（2026-04-19）
- [ ] O-01 視覺稿定稿（Claude Design · 額度恢復後）
- [ ] 四個 sidecar endpoints 補進 `sidecar-api.md`（實作期）
- [ ] `security.md §3.8` 加指向本文件的連結（現有 First-run 授權段為舊簡版）
- [ ] `sanitizer.md §七` 舊「首次授權 modal」段加指向本文件的連結
- [ ] `decisions.md` 新增 D-023 交叉引用（或擴充 D-008）
