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
- `GET /healthz` → `200 {"ok": true, "version": "..."}`
- Tauri 啟動後輪詢（最多 10s）確認 sidecar ready

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

**Request**（MVP · `workspace_type: "folder"`）
```json
{
  "workspace_type": "folder",
  "workspace_source": { "path": "/abs/path/to/repo" },
  "options": {
    "respect_gitignore": true,
    "max_file_size_kb": 512
  }
}
```

**雙模 schema**（對齊 `authorization.md §一` · D-002）

| `workspace_type` | `workspace_source` 形態 | 何時支援 |
|---|---|---|
| `"folder"` | `{ "path": "<abs_path>" }` | **MVP** |
| `"topic"` | `{ "query": "...", "seed_urls": [...], "domain_allowlist": [...] }` | Phase 2 |

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

大 repo 走 async：先回 `{ "task_id": "scan_abc123" }`，進度走 SSE（見四），最終結果經 `GET /tasks/{id}/result` 取上述完整 `ScanResult`。

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

### `POST /explore`
Explorer Agent 探索（Module 4）。async。

**Request**
```json
{
  "workspace_id": "timeline-gdrive",
  "task": "新增 Google Drive Adapter 同步功能",
  "budget": { "max_steps": 40, "max_tokens": 200000 }
}
```

**Response**
```json
{ "task_id": "explore_def456" }
```

最終結果經 `GET /tasks/{id}/result` 取：
```json
{
  "stations": [ /* route.json 格式 */ ],
  "reasoning_log_path": "workspace/timeline-gdrive/reasoning_log.jsonl"
}
```

---

### `POST /generate`
產出 tutorial.md（Module 5）。async。

**Request**
```json
{
  "workspace_id": "timeline-gdrive",
  "explore_task_id": "explore_def456",
  "mode": "interactive"
}
```

`mode`：`"interactive"`（含 `<Checkpoint>` / `<Quiz>`）或 `"plain"`（純 Markdown）。

---

### `POST /qa`
Q&A Agent 會話（Module 8，D-016）。async。

**Request**
```json
{
  "workspace_id": "timeline-gdrive",
  "question": "PaymentService 怎麼處理退款？",
  "session_id": "qa_sess_abc"
}
```

**Response**
```json
{ "task_id": "qa_task_xyz" }
```

答案與 KB growth 事件走 SSE（見四）。

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
{ "type": "answer_stream", "delta": "PaymentService 使用 ..." }
```

**Usage tracking 事件（D-021）**
```json
{ "type": "usage_delta", "phase": "explore", "module": "explorer", "step": 3, "prompt_tokens": 1240, "completion_tokens": 180, "cost_usd": 0.0042, "session_total_cost_usd": 0.031 }
{ "type": "usage_summary",
  "session_id": "sess_abc",
  "total_tokens": 45200,
  "total_cost_usd": 0.12,
  "by_module": { "explorer": 0.08, "judge": 0.02, "generator": 0.015, "kb_build": 0.005 },
  "by_phase":  { "scan": 0.0, "kb_build": 0.005, "explore": 0.10, "generate": 0.015, "qa": 0.0 }
}
```

- `phase` ∈ `scan / kb_build / explore / generate / qa`（當前呼叫處於哪個生命週期階段）
- `module` ∈ `explorer / judge / coverage / generator / qa / kb_build / embed`（實際打 LLM 的邏輯組件）
- `session_id` = workspace open → close 完整生命週期（scan/kb_build/explore/generate/qa 全部共用）
- 「這條路線跑完花多少錢」= `by_phase` 的 `scan + kb_build + explore + generate` 總和

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
`done` 送出前必先 emit 一筆 `usage_summary`（D-021）。

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
