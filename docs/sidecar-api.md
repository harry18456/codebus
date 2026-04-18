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
掃描資料夾（Module 1）。同步呼叫，小資料夾秒回；大資料夾走 async 版（見下）。

**Request**
```json
{
  "path": "/abs/path/to/repo",
  "options": {
    "respect_gitignore": true,
    "max_file_size_kb": 512
  }
}
```

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
