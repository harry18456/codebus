# Python Sidecar HTTP API Spec

> Tauri (Rust) ↔ Python Sidecar 的 IPC 契約。
> 關聯決策：D-001（混合架構）、D-008（三階段進度）、D-011（Sidecar 資安）。

---

## 一、傳輸與啟動

### 啟動流程（Tauri 端）
1. App 啟動時 spawn Python sidecar process
2. Sidecar 啟動時：
   - 隨機選 ephemeral port（`bind(0)` 取）
   - 生成 32-byte random token
   - 只 listen on `127.0.0.1`（禁對外）
   - 把 `{port, token}` 寫到 stdout 第一行 JSON
3. Tauri 讀 stdout 拿到 port + token，存 Tauri state
4. 後續所有 request header 帶 `Authorization: Bearer {token}`
5. App 關閉時 Tauri 發 `POST /shutdown` 再 SIGTERM，10s 沒退就 kill

### 為何不用 Unix socket / named pipe
跨平台一致（Windows 無 UDS）；localhost TCP + token 足夠，且 debug 簡單（curl 可打）。

### Health check
- `GET /healthz` → `200 {"status": "ok|degraded", "dependencies": {...}}`
- Tauri 啟動後輪詢（最多 10s）確認 sidecar ready
- `dependencies` map（change `kb-build-production-wiring`, D-032 後；`chat-provider-wiring` 再補 `openai_chat`）：
  - `qdrant`：Qdrant 連線探測（不可達 → `ok=false`）
  - `openai_embedding`：KB embedding provider 狀態,`status` 有三值
    - `"ok"`：`CODEBUS_OPENAI_API_KEY` 設 + 啟動時 smoke embed 通
    - `"degraded"`：env 設但 smoke embed 失敗(auth / rate limit / network)
    - `"not-configured"`：env 未設（`ok=true` 因為是**預期的**降級,非故障）
  - `openai_chat`：chat / reasoning / judge 三個 role 共用的 provider 狀態,`status` 一樣三態
    - `"ok"`：env 設 + 啟動時 smoke chat（打 `gpt-4o-mini` with `response_model=_ChatProbeModel`）通
    - `"degraded"`：env 設但 smoke chat 失敗
    - `"not-configured"`：env 未設
- Smoke probe 在 sidecar 啟動時各跑一次,結果分別 cache 於 `app.state.openai_embedding_probe` / `app.state.openai_chat_probe`；`/healthz` 不每次都打 OpenAI。兩個 probe 都走 **raw** provider(不經 TrackedProvider)——健康檢查不是 production traffic,不污染任何 workspace audit trail。一個 `openai_chat` probe 覆蓋三個 chat-ish role 因為它們共用同一把 API key 與同一個 OpenAI chat endpoint

---

## 二、錯誤格式

所有非 2xx response 統一：

```json
{
  "error": {
    "code": "SCANNER_ENCODING_FAIL",
    "message": "Cannot decode file foo.txt as utf-8/big5/gbk",
    "details": { "path": "foo.txt" }
  }
}
```

`code` 穩定列舉；`message` 可變。前端用 `code` 分支，不要 parse `message`。

---

## 三、REST endpoints

### `POST /scan`
掃描 workspace 內容（Module 1）。同步呼叫，小資料夾秒回；大資料夾走 async 版（見下）。

> **Skeleton 範圍註記（`scanner-skeleton`）**：
>
> - `/scan` 目前為**同步單 body JSON response**，尚無 async + SSE 路徑；response 一次回完整 `ScanResult`（Response example 如下所列）。
> - Request 使用**骨架簡化 schema**：`{ workspace_type, workspace_root }`，扁平化於頂層；原 spec 設計的 `workspace_source: { path }` wrapper 與 `options` 物件（`respect_gitignore` / `max_file_size_kb`）**未實作**，留待後續 change 疊加（屆時以新增欄位為主，不破壞現有 caller）。
> - 必經 bearer 中介層：無 `Authorization: Bearer <token>` → `401`，不露 sidecar 內部（對齊 §五）。
> - `workspace_type: "topic"` → `HTTPException 501 Not Implemented`（非 `400`）with `detail="workspace_type='topic' not implemented in MVP"`（對齊 D-002「discriminator day 1」：schema 吃得到但功能未實作）。
> - 未知 `workspace_type`（不是 `"folder"` / `"topic"`）→ `422 Unprocessable Entity`，由 Pydantic discriminated union 驗證擋下。
> - `workspace_root` 不存在或非目錄 → `400` with `detail={ "code": "SCANNER_WORKSPACE_INVALID", "message": "..." }`（對齊 `module-1-scanner.md §十二`）。
> - Response 中 `git` 永遠 `null`、`is_monorepo=false` / `monorepo_type=null` / `sub_packages=[]`（詳 `module-1-scanner.md §十一` Skeleton 註記）；`FileEntry.sanitize_stats` 為 Pass 1 sanitize 後真實 kind→count（無命中時 `{}`）、`stats.quarantined_count` 為 Pass 1 失敗檔數（正常情況為 `0`）。

**Request**（Skeleton · `workspace_type: "folder"`）
```json
{
  "workspace_type": "folder",
  "workspace_root": "/abs/path/to/repo"
}
```

**雙模 schema**（對齊 `authorization.md §一` · D-002）

| `workspace_type` | Skeleton 行為 | 何時完整支援 |
|---|---|---|
| `"folder"` | 走 `scan()` pipeline，回完整 `ScanResult` | **Skeleton（本 change）** |
| `"topic"` | 回 `501 Not Implemented`（schema 已吃，功能未實作）| Phase 2 |

`workspace_type` discriminator 從 day 1 寫入，Phase 2 加 topic 不需 schema breaking change。

**Response**（對齊 `module-1-scanner.md` §十一 `ScanResult` schema）
```json
{
  "task_id": "scan_abc123",
  "workspace_root": "/abs/path/to/repo",
  "scan_started_at": "2026-04-17T10:30:00Z",
  "scan_completed_at": "2026-04-17T10:30:18Z",
  "files": [
    {
      "path": "src/main.py",
      "size": 1204,
      "kind": "text",
      "language": "python",
      "language_confidence": "extension",
      "encoding": "utf-8",
      "content": "... (已 sanitize)",
      "sanitize_stats": { "email": 1, "secret": 0 }
    },
    {
      "path": "dist/bundle.js",
      "size": 48210,
      "kind": "generated",
      "language": "javascript",
      "content": null
    }
  ],
  "symlinks": [],
  "is_monorepo": true,
  "monorepo_type": "pnpm",
  "sub_packages": [
    { "path": "packages/core", "name": "@foo/core" }
  ],
  "git": {
    "head": "abc123...",
    "branch": "main",
    "remote_url": "<REDACTED:internal-domain>/org/repo.git",
    "recent_commits": [ /* ... */ ],
    "file_activity": { /* ... */ },
    "blame": { /* top-N 檔案 */ }
  },
  "content_summary": {
    "total_files": 128,
    "kind_counts": { "text": 94, "binary": 12, "lockfile": 3, "generated": 15, "oversized": 4 },
    "language_counts": { "python": 40, "typescript": 35, "markdown": 19 },
    "category_counts": { "code": 75, "docs": 15, "config": 4, "test": 20 },
    "dominant_category": "code",
    "dominant_languages": ["python", "typescript", "markdown"],
    "has_tests": true,
    "has_docs": true,
    "is_monorepo": true
  },
  "stats": {
    "total_files_walked": 312,
    "total_files_included": 94,
    "total_bytes_read": 482103,
    "duration_seconds": 17.8,
    "quarantined_count": 0,
    "skipped_count": 34
  },
  "warnings": []
}
```

> **Skeleton 行為差異**：上方 Response example 展示**完整 spec 目標**（含 `git` metadata、`is_monorepo=true` 的 monorepo 結果）。目前 `scanner-skeleton` + `scanner-sanitizer-orchestration` 階段實際回的是同一 schema 但部分子系統仍為 stub——`git` 為 `null`、`is_monorepo=false` / `monorepo_type=null` / `sub_packages=[]`、`FileEntry.oversized_preview=null`、`task_id` 未採用（同步 response 不需）。`FileEntry.sanitize_stats` 與 `stats.quarantined_count` 已由 Pass 1 Sanitizer 驅動，回實際值（無命中時 `{}` / `0`）。

大 repo 走 async（先回 `{ "task_id": "..." }` + SSE 進度 + `GET /tasks/{id}/result` 取結果）為後續 change 目標，`scanner-skeleton` 未實作——所有 repo 都走同步 body。

---

### `POST /kb/build`
建知識庫（Module 2）。async，回 `task_id`，進度走 SSE。

**Request**
```json
{
  "workspace_id": "timeline-gdrive",
  "files": ["..."],
  "options": { "chunk_size": 500, "overlap": 50 }
}
```

**Response**
```json
{ "task_id": "kb_xyz789" }
```

---

### `POST /kb/query`（change `kb-query-endpoint`）
查 KB(Module 2)。**同步** JSON,非 SSE(query 通常 < 1s)。

**Request**
```json
{
  "workspace_root": "/abs/path/to/workspace",
  "text": "storage adapter",
  "top_k": 8,
  "filter_path": "src/storage/types.ts",
  "filter_source_kind": ["code"]
}
```

| 欄位 | 規範 |
|---|---|
| `workspace_root` | abs path,sidecar 用此導出 collection name 與 audit log 路徑 |
| `text` | 必填、非空字串;sidecar 會 embed 後做 cosine search |
| `top_k` | 預設 8,範圍 `1..50`,超出回 422 |
| `filter_path` | 選填;只回此 file_path 的 hits |
| `filter_source_kind` | 選填;list of `"code"` / `"doc"` / `"skeleton"` 等 SourceKind |

**Response**(200 OK)
```json
{
  "hits": [
    {
      "point_id": "...",
      "score": 0.87,
      "payload": { "file_path": "src/foo.py", "source_kind": "code", "...": "..." }
    }
  ]
}
```

`hits` 依 score 遞減排序。Empty workspace / 未 build 的 collection → 200 `{"hits": []}`(非 404,讓 caller 邏輯單一)。

**錯誤回應**

| Status | Body | 觸發條件 |
|---|---|---|
| 401 | bearer 拒絕 | 無 / bad `Authorization` header |
| 422 | Pydantic validation | 缺欄位、`top_k <= 0` 或 `> 50`、`text` 空字串 |
| 503 | `{"detail": {"code": "KB_NOT_CONFIGURED", "missing": [...]}}` | sidecar 啟動時無 `CODEBUS_OPENAI_API_KEY`(query 也需要 embed text 成向量) |

**Cost 帳分離**:query 路徑的 embed call 在 `<workspace>/token_usage.jsonl` 標 `module="kb_query"`(非 `"kb_build"`),由 `app.state.kb_query_provider` factory 內的 TrackedProvider `default_module="kb_query"` 自動套用——詳見 `module-2-kb-builder.md §七` Production wiring 段。

---

### `POST /explore`
Explorer Agent 探索（Module 4）。async，落地於 change `agent-sse-wiring`。

**Request**
```json
{
  "workspace_root": "/abs/path/to/workspace",
  "task": "trace how storage is wired",
  "budget_steps": 10,
  "budget_tokens": 50000
}
```

欄位：
- `workspace_root` (string, required) — 絕對路徑，必須存在且是資料夾；不存在或非目錄 → 400 `EXPLORE_WORKSPACE_INVALID`。
- `task` (string, required, `min_length=1`) — 使用者下的探索 prompt。
- `budget_steps` (int, default `10`, `0 ≤ n ≤ 200`) — ReAct 迴圈上限。
- `budget_tokens` (int, default `50000`, `≥ 0`) — 預留欄位，P0 暫未強制；真實 token accounting 由步驟 21 接手。

**Response**
- `202 Accepted`：`{ "task_id": "explore_<8-hex>" }` — `TaskRegistry.create("explore")` 成功。
- `409 Conflict`：`{ "detail": { "code": "TASK_IN_FLIGHT", "running_task_id": "..." } }` — 單槽已被其他 task（含 scan / kb / explore）佔用。
- `503 Service Unavailable`：`{ "detail": { "code": "EXPLORE_NOT_CONFIGURED", "missing": [...] } }` — `llm_reasoning_provider` / `llm_judge_provider` 未注入（`CODEBUS_OPENAI_API_KEY` 未設時會命中）。
- `401 Unauthorized` / `400 Bad Request` — 按 middleware 與 Pydantic 驗證。

訂閱 `GET /tasks/{task_id}/events` 取 SSE stream（見 §四）。最終結果經 `GET /tasks/{id}/result`：
```json
{
  "stations": [ /* route.json 格式，Station list */ ],
  "log_path": "/abs/path/to/workspace/reasoning_log.jsonl",
  "stopped_reason": "budget_exhausted" | "queue_empty" | "cancelled"
}
```

---

### `POST /generate`
產出 tutorial 多檔（Module 5；`module-5-generator-p0` 落地）。async；走 single-slot `TaskRegistry`（409 `TASK_IN_FLIGHT` 同 `/explore` / `/qa`）。

**Request**（對齊 `sidecar/src/codebus_agent/api/generate.py::GenerateRequest`，`extra="forbid"`）
```json
{
  "workspace_root": "/abs/path/to/workspace",
  "task": "trace how storage is wired",
  "stations": [
    { "path": "src/storage/types.ts", "role": "interface", "why": "..." }
  ],
  "options": {}
}
```

| 欄位 | 規範 |
|---|---|
| `workspace_root` | 絕對路徑，必須存在且是資料夾；不存在或非目錄 → `400 GENERATE_WORKSPACE_INVALID`。 |
| `task` | 必填、`min_length=1`；使用者下的教材主題 prompt。 |
| `stations` | `Station` list（空陣列亦可）；schema 對齊 `codebus_agent.agent.types.Station`，`module-5-generator.md §三 / §八` 列欄位定義。 |
| `options` | `GeneratorOptions`（預設值即可）；對應 `codebus_agent.generator.types.GeneratorOptions`，詳 `module-5-generator.md §六`。 |

未列於上表的欄位一律 `422 Unprocessable Entity`（`extra="forbid"` 拒絕）。

**Response**

- `202 Accepted`：`{ "task_id": "generate_<8-hex>" }` — `TaskRegistry.create("generate")` 成功，背景跑 `run_generator`。
- `400 Bad Request`：`{ "detail": { "code": "GENERATE_WORKSPACE_INVALID", "message": "..." } }` — `workspace_root` 不存在或非目錄。
- `409 Conflict`：`{ "detail": { "code": "TASK_IN_FLIGHT", "running_task_id": "..." } }` — 單槽已被佔用（含 scan / kb / explore / generate / qa）。
- `503 Service Unavailable`：`{ "detail": { "code": "GENERATE_NOT_CONFIGURED", "missing": ["llm_generate_provider", ...] } }` — `app.state.llm_generate_provider` factory 未注入（`CODEBUS_OPENAI_API_KEY` 未設時會命中）。
- `401 Unauthorized` / `422 Unprocessable Entity` — 按 bearer middleware 與 Pydantic 驗證。

訂閱 `GET /tasks/{task_id}/events` 取 SSE stream（見 §四）。背景任務若拋未分類例外，wrapper emit `{"type": "error", "code": "GENERATE_FAILED", ...}`（per-station validator 失敗走 degraded fallback、不走 error event）。

`task_id` 過 `^generate_[0-9a-f]{8}$`。

---

### `POST /qa`
Q&A Agent 會話（Module 8，D-016；`module-8-qa-p0` 2026-04-26 落地）。async；走 single-slot `TaskRegistry`（409 `TASK_IN_FLIGHT` 同 `/explore` / `/generate`）。

**Request**
```json
{
  "workspace_root": "/abs/path/to/workspace",
  "question": "PaymentService 怎麼處理退款？",
  "originating_station_id": "s02-payment"
}
```

驗證：`question` ≤ 4000 chars 非空（strip 後）；`originating_station_id` 給定時必符合 `^s\d{2}-[a-z0-9-]{1,40}(-\d+)?$`；任何違規回 422。

**Dependency check**：`kb_provider` / `kb_query_provider` / `kb_growth_logger_factory` / `llm_chat_provider` / `llm_judge_provider` 任一缺即 503 `QA_NOT_CONFIGURED`，`detail` 列出缺哪些 slot。

**Response（202）**
```json
{ "task_id": "qa_a1b2c3d4" }
```

`task_id` 過 `^qa_[0-9a-f]{8}$`。錯誤碼表：`QA_FAILED`（loop 例外）/ `QA_NOT_CONFIGURED`（503）/ `TASK_IN_FLIGHT`（409）/ `QA_WORKSPACE_INVALID`（400）。

**SSE 事件序列**（見 §四）：`rag_hits`（一次，初始 KB 探查後）→ confident path：直接 `qa_answer` 收尾；non-confident path：`agent_thought` / `agent_action_result` 多輪 → `kb_growth`（每筆 add_to_kb 新點，dedup 不 emit）→ `qa_answer`（一次）→ `done`。`usage_delta` / `llm_call` 由 TrackedProvider 自動 emit，`module="qa_agent"`。

---

### `POST /tasks/{id}/cancel`
取消 in-flight task（D-008）。冪等。

**Response** `202 Accepted`（sidecar 盡快中止，不保證即時）。

---

### `GET /tasks/{id}/llm_calls`（D-022）
列出 session 的 LLM call 摘要（list view 資料源）。
```json
{
  "calls": [
    { "request_id": "llm_abc123", "ts": "...", "module": "explorer", "step_id": 3,
      "model": "gpt-4o", "call_type": "chat_structured",
      "tokens": { "prompt": 1240, "completion": 180 },
      "cost_usd": 0.0042, "latency_ms": 1842, "preview": "...", "error": null }
  ],
  "total": 42,
  "session_total_tokens": 45200,
  "session_total_cost_usd": 0.12
}
```
支援 query：`?module=explorer&step=3&model=gpt-4o&limit=50&offset=0`。

---

### `GET /tasks/{id}/llm_calls/{request_id}`（D-022）
取單一 LLM call 的完整 payload（detail modal 資料源）。
```json
{
  "request_id": "llm_abc123",
  "ts": "2026-04-18T10:00:01Z",
  "module": "explorer", "step_id": 3,
  "provider": "contest-openai", "model": "gpt-4o",
  "call_type": "chat_structured",
  "request": { "messages": [...], "tools": [...], "temperature": 0.2, "response_format": {...} },
  "response": { "content": {...}, "tool_calls": [...], "finish_reason": "stop" },
  "usage": { "prompt_tokens": 1240, "completion_tokens": 180, "cost_usd": 0.0042 },
  "latency_ms": 1842, "truncated": false, "error": null
}
```
**注意**：`request.messages` 為 post-Sanitizer Pass 2 版本（實際 wire payload），不會還原 pre-sanitize 原文（D-022）。

---

### `GET /reasoning?after_step_id=`（IA §13 · SSE reconnect 支援）

Agent console 的 SSE 斷線重連用。前端記最後收到的 `step_id`，reconnect 時打這個 endpoint 取漏收的步驟一次補齊，再重新訂閱 `GET /tasks/{id}/events` 接續新事件。

**Request query**
- `task_id`（必要）：要取哪個 task 的 reasoning log
- `after_step_id`（必要）：從這個 step_id 之後的開始回（不含此 id）
- `limit`（選用，預設 100）：上限筆數，防瀏覽器塞爆

**Response**
```json
{
  "task_id": "explore_def456",
  "entries": [
    {
      "step_id": 7,
      "ts": "...",
      "phase": "explore",
      "thought": "...",
      "tool_call": { "tool": "find_callers", "args": {...} },
      "tool_result": { "observation": "...", "tokens_used": 1240 },
      "judge_verdict": { "relevance": 0.92, "reason": "..." },
      "usage": { "prompt_tokens": 1240, "completion_tokens": 180 }
    }
  ],
  "has_more": false,
  "latest_step_id": 14
}
```

**語意**
- `entries` 按 `step_id` 升序排列；前端 append 到 console，不清空舊的
- `has_more: true` 時 UI 繼續用最新 `step_id` 再打一次
- 資料源為 `reasoning_log.jsonl`（D-017）— reconnect 時不重跑 Agent，純讀檔
- `step_id` = 0 時回整個 task 的 log（冷啟進 audit tab 看歷史用）

---

### `GET /tasks/{id}/status`
```json
{
  "task_id": "explore_def456",
  "state": "running",
  "phase": "exploring",
  "progress": 0.42
}
```

`state`：`queued` / `running` / `done` / `failed` / `cancelled`

---

### Audit Mode endpoints（C+ · O-05 Sanitizer Diff 支援）

稽核解鎖是「看 raw 原文」的唯一合法路徑。未解鎖時 diff endpoint 的 `raw` 欄位為 null，UI 顯示遮罩。

#### `POST /audit/unlock`
使用者在 `/audit` route 點「🔓 解鎖原值」觸發。寫 `sanitize_audit.jsonl` 的 `audit_unlock` event，回傳 `audit_session_id`（後續 diff 請求帶此 id）。
```json
// Response
{
  "audit_session_id": "auds_9f2e",
  "unlocked_at": "2026-04-18T10:22:00Z",
  "timeout_sec": 900
}
```

#### `POST /audit/relock`
使用者點「🔒 重新鎖定」或離開 route 時觸發。寫配對的 `audit_relock` event。
```json
// Request
{ "audit_session_id": "auds_9f2e", "trigger": "user_manual_button" }
// Response
{ "relocked_at": "...", "duration_sec": 142 }
```
`trigger`：`user_manual_button` / `route_left` / `timeout`

#### `GET /audit/sanitize/files`
O-05 左欄檔案樹資料源。列出所有有 sanitize 動作的檔案，依嚴重度降序 + kind 分組。
```json
{
  "files": [
    { "path": "src/config.py", "kinds": { "secret": 2, "email": 1 },
      "severity": "high", "last_scanned": "..." },
    { "path": "src/adapters/s3.ts", "kinds": { "email": 3, "domain": 1 },
      "severity": "medium", "last_scanned": "..." }
  ]
}
```

#### `GET /audit/sanitize/diff`
O-05 右上 diff view 資料源。依 `audit_session_id` 的有效性與 scope 回三態（`raw` / `raw_masked` / 都 null）。

**三態語意**（LEFT pane 渲染邏輯）

| 狀態 | `audit_session_id` | scope 是否涵蓋此檔 | `raw` | `raw_masked` |
|---|---|---|---|---|
| **LOCKED** | 無 / 過期 / `relock` 已寫 | — | `null` | 非 null（結構保留遮罩） |
| **UNLOCKED · in-scope** | 有效 | 是（`file` scope 指向本檔，或 `all_placeholders`） | 完整原文 | `null` |
| **UNLOCKED · out-of-scope** | 有效 | 否（`file` scope 指向別檔） | `null` | 非 null（等同 LOCKED 對此檔） |

```json
// Request query: ?file=src/adapters/s3.ts&audit_session_id=auds_9f2e
{
  "file": "src/adapters/s3.ts",
  "scrubbed": "// Copyright ... <REDACTED:email#1> ...",
  "raw": "// Copyright ... john@example.com ...",
  "raw_masked": null,
  "placeholders": [
    { "id": "email#1", "kind": "email", "line": 2, "col": 35,
      "rule_id": "pii_email_v1", "offset_raw": [150, 168] }
  ],
  "rule_stats": {
    "pii_email_v1": { "matched": 23, "flagged": 0 },
    "aws_access_key": { "matched": 1, "flagged": 0 }
  },
  "timeline": [
    { "ts": "...", "rule_id": "pii_email_v1", "kind": "email",
      "placeholder_id": "email#1", "pass": "scanner" }
  ]
}
```

**`raw_masked` 生成規則**
- 由 sidecar 以 `placeholders[].offset_raw` 為界產生，非 placeholder 區段保留原字元（註解、import、語法結構可讀），placeholder 區段替為 `░` × `round(len × 0.75)`，下限 4 字元
- 生成後 `raw` 資料不得殘留在同一 response — 互斥
- 前端永遠直接 render `raw ?? raw_masked`，不在 client 端做遮罩運算（避免客端程式碼意外接觸原文）

**`rule_stats` 語意**
- key 為 `rule_id`，對應 RIGHT pane card 的 `matched: N · flagged: M` 顯示
- `matched` = **本 session（workspace open→close 內）此規則共命中幾次**，跨檔加總（見 `sanitizer.md §十一`）
- `flagged` = 使用者於稽核頁面標記「這筆不該替換」的反饋數；MVP 恆為 `0`（反饋回路留 post-MVP）

**Security 對齊**
- `raw` 欄位是前端唯一合法取得原文的路徑，回傳前必驗 `audit_session_id` 有效且未 expire 且 scope 涵蓋
- `raw_masked` 為結構保留遮罩，不洩漏原值任一字元（非 placeholder 區段才可見）
- Timeline entry **不含** `matched_substring`（原值片段只走 `raw` 欄位，不雙通道洩漏）
- sidecar 內部 `raw` 從**檔案系統即時 re-read**，不從 KB 或 log 拉（KB/log 都是 scrubbed 版本）

---

## 三-bis、Task lifecycle（單槽 in-memory registry，change `sse-progress-skeleton`）

非同步任務以單槽 in-memory `TaskRegistry` 管理（`app.state.tasks`）：同一時刻只允許一個 `running` task；終止後（`done` / `error`）handle 仍留在 slot 直到下次 `create()` 覆寫。所有 task endpoint 走同一條 bearer middleware。

### Endpoints

| 方法 | Path | 說明 |
| --- | --- | --- |
| `POST` | `/scan?stream=true` | opt-in async scan；回 `{"task_id": "scan_<hex8>"}`。無 `?stream=true` 則保留同步契約 |
| `POST` | `/kb/build` | 預設 async；body `{workspace_root, scan_result}`；回 `{"task_id": "kb_<hex8>"}` |
| `GET`  | `/tasks/{id}/events` | SSE 訂閱，見「四、Progress Event」 |
| `GET`  | `/tasks/{id}/result` | terminal payload。`200` = done、`409` = 仍 running、`404` = unknown |

### task_id format
`^(scan|kb|explore|generate|qa)_[0-9a-f]{8}$`。8 位 hex 由 `secrets.token_hex(4)` 產出，足夠單槽 store 不撞號。`scan` / `kb` 自 `sse-progress-skeleton`，`explore` 自 `agent-sse-wiring`，`generate` 自 `module-5-generator-p0`，`qa` 自 `module-8-qa-p0`。

### 409 / TASK_IN_FLIGHT
`POST /scan?stream=true` 或 `POST /kb/build` 若遇上現有 running task：
```json
{"code": "TASK_IN_FLIGHT", "running_task_id": "scan_abcd1234"}
```
不會 spawn 新背景 task。

### 409 / TASK_NOT_DONE
`GET /tasks/{id}/result` 若 task 仍 `running`：
```json
{"code": "TASK_NOT_DONE", "task_id": "scan_abcd1234", "status": "running"}
```

### Background error containment
背景 coroutine 任何例外都被 `_run_background_task` wrapper 收斂：
- 對訂閱者 emit 一筆 `{"type": "error", "code": <ERROR_CODES>, "message": <safe>}`
- 完整 traceback 只進 sidecar logger，不上 wire
- 後加入的訂閱者也會看到 cached error event（不會永遠 hang）

`code` 限定於下表 10 個（對齊 `sidecar/src/codebus_agent/api/tasks.py::ERROR_CODES` frozenset；新增 task kind 必須走 Spectra change 同步補入）：

| Code | 觸發場景 | `message`（`_safe_error_message` 真值） |
|---|---|---|
| `SCAN_FAILED` | `/scan?stream=true` 背景 scan 例外 | `scan task failed` |
| `KB_BUILD_FAILED` | `/kb/build` 背景 KB build 例外 | `knowledge-base build failed` |
| `EXPLORE_FAILED` | `/explore` 背景 Explorer loop 例外 | `explore task failed` |
| `GENERATE_FAILED` | `/generate` 背景 Generator 例外 | `tutorial generation failed` |
| `QA_FAILED` | `/qa` 背景 Q&A loop 例外 | `Q&A task failed` |
| `OPENAI_AUTH_FAILED` | OpenAI key 失敗 | `OpenAI authentication failed; verify CODEBUS_OPENAI_API_KEY` |
| `OPENAI_RATE_LIMITED` | OpenAI rate limit | `OpenAI rate limit exceeded; try again later` |
| `OPENAI_CONTEXT_EXCEEDED` | prompt 超 model context | `LLM context window exceeded for the chosen model` |
| `KB_DIM_MISMATCH` | KB collection dim 不符 | `knowledge-base collection vector dimension mismatch`（D-032） |
| `INTERNAL_ERROR` | 未分類例外 catch-all | `internal sidecar error` |

詳細不變式見 `openspec/specs/sidecar-runtime/spec.md` 的 `Background task error containment` Requirement。

---

## 四、Progress Event (SSE)

### Endpoint
`GET /tasks/{id}/events`（Server-Sent Events，`text/event-stream`）

### Event schema
每筆一行 JSON，分類：

**Phase progress（三階段通用）**
```json
{
  "type": "progress",
  "phase": "embedding",
  "current": 42,
  "total": 94,
  "current_file": "src/services/LocalFileAdapter.ts"
}
```

`phase` ∈ `scanning` / `embedding` / `exploring` / `generating`

**Agent thought（Explorer 階段，D-008 的 demo 神器）**
```json
{
  "type": "agent_thought",
  "step": 7,
  "thought": "看到 IStorageService interface，決定 trace 所有實作",
  "action": { "tool": "find_callers", "args": { "symbol": "IStorageService" } }
}
```

**Agent action result**
```json
{
  "type": "agent_action_result",
  "step": 7,
  "observation": "找到 2 個實作: MockStorageAdapter, LocalFileAdapter",
  "tokens_used": 1240
}
```

**Judge verdict**
```json
{
  "type": "judge_verdict",
  "step": 7,
  "relevance": 0.92,
  "reason": "Interface 定義是加新 Adapter 的必經站"
}
```

**Q&A 專屬事件（D-016）**
```json
{ "type": "rag_hits", "hits": [{"source": "src/foo.py:42", "score": 0.87, "snippet": "..."}] }
{ "type": "kb_growth", "entry_id": "qdrant-id-xyz", "source": "src/services/payment.ts:120-180", "reason": "..." }
{ "type": "qa_answer",
  "answer": "PaymentService 使用 ...",
  "citations": [
    { "file_path": "src/services/payment.ts", "line_start": 120, "line_end": 180, "related_stations": ["s02-payment"] }
  ]
}
```

> P0 為一次性 non-streaming `qa_answer`（spec capability `qa-agent`）。欄位級 streaming（chunk delta）為 P1 reserved，屆時新事件名 + spec 同步 bump。

**Usage tracking 事件（D-021）**
```json
{ "type": "usage_delta", "phase": "explore", "module": "explorer", "step": 3, "prompt_tokens": 1240, "completion_tokens": 180, "cost_usd": 0.0042, "session_total_cost_usd": 0.031, "session_total_tokens": 45200 }
```

- `phase` ∈ `scan / kb_build / explore / generate / qa`（當前呼叫處於哪個生命週期階段）
- `module` ∈ `explorer / judge / coverage / generator / qa / kb_build / embed`（實際打 LLM 的邏輯組件）
- 每筆 `usage_delta` 都帶 `session_total_cost_usd` + `session_total_tokens`，client-side 不需另一個 summary 事件即可即時渲染 cost panel
- 「這條路線跑完花多少錢」由 client 端累積最後一筆 `usage_delta.session_total_cost_usd` 取得

詳見 `agent-core.md §十三 UsageTracker`。

**LLM Call Inspector 事件（D-022）**
```json
{ "type": "llm_call", "request_id": "llm_abc123", "module": "explorer", "step_id": 3, "model": "gpt-4o", "call_type": "chat_structured", "latency_ms": 1842, "tokens": { "prompt": 1240, "completion": 180 }, "cost_usd": 0.0042, "preview": "task: 新增 GoogleDrive Adapter..." }
```
UI list view 渲染用；detail 由下方 HTTP endpoint 拿完整 payload。詳見 `agent-core.md §十三.2 LLMCallLogger`。

**Final done**
```json
{ "type": "done", "result_url": "/tasks/explore_def456/result" }
```
`done` 送出前不需任何前置 event；client-side 由累積 `usage_delta` 算 session total（每筆 `usage_delta` 都帶 `session_total_cost_usd` + `session_total_tokens`，D-021）。

**Error**
```json
{ "type": "error", "code": "LLM_RATE_LIMIT", "message": "..." }
```

### 前端處理
- Nuxt 端用 `EventSource` 或 fetch reader 接
- `type: progress` → 進度條；`type: agent_thought/action_result/judge_verdict` → Agent console stream；`type: done` → 跳結果頁

---

## 五、資安要求（連動 D-011）

- [x] bind `127.0.0.1` only
- [x] token auth（每次啟動隨機）
- [x] 所有 path 參數白名單檢查（只能在使用者授權的資料夾或 workspace 內）
- [x] 不 log token 或完整 prompt 到 stdout/stderr
- [ ] request timeout（預設 60s，長任務走 async + SSE）
- [ ] 無 token 或錯 token → `401`，不揭露 sidecar 內部

---

## 六、打包（連動 D-001 / D-014）

- 開發環境：`uv sync --frozen` + `uv run python -m codebus_agent.api`
- 打包：`uv run pyinstaller` 把 sidecar 與依賴打成單一 executable
- Tauri `tauri.conf.json` 設 `bundle.externalBin` 指向該 executable
- Tauri runtime 自動選對應平台的 binary spawn
- 驗證點：Linux / Windows 各跑一次 E2E，`tauri build` 後 installer 內有 sidecar binary

---

## 七、未列入 MVP

- gRPC / WebSocket（JSON over HTTP + SSE 已足）
- 多 workspace 並發 task（queue 簡化成 FIFO，一次一個）
- 分散式 sidecar / 遠端 sidecar（本機 only）
