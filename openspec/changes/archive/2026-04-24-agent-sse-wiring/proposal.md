## Why

`sse-progress-skeleton`（2026-04-22 archived）已把 **SSE 基建**立起來了 —— `TaskRegistry` 單槽、`TaskHandle.subscribe() / emit() / close_subscribers()`、`GET /tasks/{id}/events` SSE endpoint、錯誤收斂。`explorer-react-loop-p0`（2026-04-24）補了 ReAct 迴圈骨架；`explorer-tools-p0`（2026-04-24）補了四個真工具。**但 Explorer 目前沒有 HTTP 入口，也沒有 emit SSE 事件**——整個 Agent 跑在 Python in-process 層，前端看不到任何即時進度。

`docs/implementation-plan.md §第四階段 步驟 22` 的重點：把 Explorer 接上 `POST /explore` + SSE，讓 `docs/sidecar-api.md §四` 早就定義好的事件 schema（`agent_thought` / `agent_action_result` / `judge_verdict` / `usage_delta` / `llm_call`）真的能流到前端。**這是 Demo 靈魂** —— 講者能指著螢幕說「你看 Agent 第 7 步決定追 IStorageService 的所有實作」是本 change 要做到的事，沒做完前端 Agent console 空蕩蕩的。

本 change 只做 **MVP SSE emit**：Explorer / Judge / Tracker / LLMCallLogger 端的 wire，加 `POST /explore` 入口 + `explore` task kind；**SSE reconnect replay (`GET /reasoning?after_step_id=`)** 延到 polish 期（`docs/sidecar-api.md §337`），MVP 靠前端重連直接訂閱新事件，漏接舊事件由 `reasoning_log.jsonl` tail 補（M2 已可行）。

對齊 `docs/decisions.md` D-012（自寫 ReAct）、D-021（UsageTracker）、D-022（LLMCallLogger）；**不破任何既有 invariant**。

## What Changes

**新增 `POST /explore` endpoint**（`sidecar/src/codebus_agent/api/explore.py`）：

- Request body: `{ "workspace_root": str, "task": str, "budget_steps": int = 10, "budget_tokens": int = 50000 }`
- Response: `202 Accepted` with `{ "task_id": "explore_<8-hex>" }`；`TaskRegistry.create(kind="explore")` 單槽 enforce（其他 task 跑時回 409 `TASK_IN_FLIGHT`）
- Background coroutine：建 `FolderTools(ctx, state)` + `LLMJudge(factory, ws)` + `ReasoningLogger(path)` 後呼 `run_explorer(...)` 配上 **SSEEmitter**（見下）；完成或 error 走既有 `_run_background_task` 包裝，確保 `done` / `error` 收斂
- path 授權：`workspace_root` 必須是已授權的資料夾（此 change **先假設 authorization middleware 由後續 change 實作**；目前只做 `ensure_in_workspace` 式的 root-validate — 存在、是目錄、可 enter）

**新增 `codebus_agent.agent.emitter` 子模組** — `SSEEmitter` abstract interface + `TaskHandleEmitter` impl：

- `SSEEmitter` Protocol：`emit(event: dict) -> None` 單方法 + `@runtime_checkable`；沒注入時（in-process 測試）走 `NullEmitter`（no-op）
- `TaskHandleEmitter(handle: TaskHandle)` — 實際 impl，`emit(event)` 直接 fan-out `handle.emit(event)`
- Explorer loop / LLMJudge / TrackedProvider / LLMCallLogger 都接 optional `emitter: SSEEmitter | None = None`；`None` 等於舊行為（不 emit，對現有測試兼容）

**擴充 `run_explorer` + Explorer loop 內 emit**：

- Think 完成 → `{"type": "agent_thought", "step": N, "thought": "...", "action": [{"tool": "search", "args": {...}}]}`
- 每次 `_execute_one` 結束 → `{"type": "agent_action_result", "step": N, "tool": "search", "observation": "<first 500 chars>", "tokens_used": ...}`（`observation` 取 `ToolResult.output[:500]` 截斷；error 則塞 error 欄）
- Judge 完成 → `{"type": "judge_verdict", "step": N, "relevance": 0.82, "reason": "..."}`
- 每輪結尾 → `{"type": "progress", "phase": "exploring", "current": step_count, "total": budget_steps_init, "current_file": ""}`（補既有 `progress` schema）

**擴充 `TrackedProvider` + `LLMCallLogger` emit**：

- `TrackedProvider.__init__` 增 optional `emitter: SSEEmitter | None = None`
- 每次 `chat` / `embed` 完成 → emit `{"type": "usage_delta", "phase": ..., "module": self._default_module, "step": ?, "prompt_tokens": ..., "completion_tokens": ..., "cost_usd": ..., "session_total_cost_usd": ...}`
  - `phase` / `step` 走 context var（`current_phase()` / `current_step()`），未設時 `phase=None`、`step=None`
  - `session_total_cost_usd` 從 `UsageTracker.session_total()` 讀（`usage-tracker-dedup` 已有）
- `LLMCallLogger.log` 同時 emit `{"type": "llm_call", "request_id": ..., "module": ..., "step_id": ..., "model": ..., "call_type": ..., "latency_ms": ..., "tokens": {...}, "cost_usd": ..., "preview": "<first 200 chars of first user message>"}`

**新增 `codebus_agent.agent.context_vars`**（新檔）：`current_phase` / `current_step` / `current_session` 三個 `contextvars.ContextVar`；Explorer loop 每輪 `_think` 前 `set(step_count)`、`run_explorer` 入口 `set("explore")`；KB build 端入口 `set("kb_build")`。既有 TrackedProvider 的 `module` 欄位走 `default_module`（已有），phase 走 context var。

**擴充 `TaskKind`**（`api/tasks.py`）：
- `TaskKind = Literal["scan", "kb", "explore"]`
- `_VALID_KINDS` 加 `"explore"`
- `_generate_task_id` 格式 `explore_<8-hex>`；spec `task_id format` regex 改 `^(scan|kb|explore)_[0-9a-f]{8}$`
- `ERROR_CODES` 新增 `EXPLORE_FAILED`（兜底）+ `EXPLORE_BUDGET_EXHAUSTED`（可選；若 stopped_reason == budget_exhausted 時**不當作 error** — 走 `done` 路徑帶 partial result 更合適，對齊 `docs/agent-core.md §九` 「不往上崩，最後至少給 partial result」）

**受影響 spec**：

- **新增 `explorer-sse` capability**（新檔 `openspec/specs/explorer-sse/spec.md`）：
  - Requirement: `POST /explore endpoint spawns Explorer under task registry`
  - Requirement: `Explorer loop emits agent_thought / agent_action_result / judge_verdict events`
  - Requirement: `TrackedProvider emits usage_delta on every completed call`
  - Requirement: `LLMCallLogger emits llm_call event carrying preview`
  - Requirement: `SSEEmitter is an opt-in runtime-checkable Protocol`

- **修改 `sidecar-runtime`**：`task_id format` Requirement 加入 `explore` 前綴（regex `^(scan|kb|explore)_[0-9a-f]{8}$`）

（`agent-core` / `llm-provider` / `usage-tracking` 三支 spec **不動** — emitter 以 `None` default 相容保留，新行為都在 `explorer-sse` capability 裡。）

**受影響測試**：

- `sidecar/tests/api/test_explore_endpoint.py`（新檔）— `POST /explore` happy path + 409 / 404 / bearer 檢查
- `sidecar/tests/agent/test_sse_emitter.py`（新檔）— `NullEmitter` no-op + `TaskHandleEmitter` fan-out；Explorer loop with `TaskHandleEmitter` 驗每輪 emit 三個 event type
- `sidecar/tests/providers/test_tracked_provider_sse.py`（新檔）— TrackedProvider with emitter 每次 chat/embed 後 emit `usage_delta`
- `sidecar/tests/providers/test_llm_call_logger_sse.py`（新檔）— LLMCallLogger.log 同時 emit `llm_call` + 寫 `llm_calls.jsonl`（雙軌）
- 既有 `tests/agent/test_explorer_loop.py` / `test_explorer_loop_with_real_tools.py` 不變（emitter=None，行為相容）
- 既有 `tests/providers/test_tracked_provider.py` / `test_default_module.py` 不變（同上相容）
- `tests/api/test_tasks.py` 既有 `explore_*` task_id format 若有 scan/kb assertion 需擴充

## Non-Goals

明確排除：

- **`GET /reasoning?after_step_id=` reconnect replay** — `docs/sidecar-api.md §337` 的完整 IA §13 spec 延到 polish 期；MVP 斷線重連靠前端從 `reasoning_log.jsonl` tail 補（sidecar 已落檔，`/tasks/{id}/events` 重新訂閱只接新事件）
- **`usage_summary` aggregate event** — 可從 `usage_delta` client-side aggregate；`emit_summary()` / `done` 前的 summary 留到 Module 5 / Q&A 需要時再加
- **Q&A 專屬事件**（`rag_hits` / `kb_growth` / `answer_stream`）— Module 8 Q&A P0 範疇（步驟 25）
- **Sanitize audit SSE event** — `sanitize_audit.jsonl` 已落檔；UI 要的是 O-05 LOCKED/UNLOCKED 畫面靠 file tail，不走 SSE（避免 Pass 1/2 大量命中塞爆頻道）
- **前端 Agent console 實作** — 屬前端階段（步驟 26+）；本 change 只確保 sidecar 端 emit 正確，前端接收用現有 `EventSource` 即可驗
- **Authorization middleware for `POST /explore`** — `POST /scan` 的 root validate 模式在 M2 還沒統一成 middleware；本 change 延用「endpoint 內 `Path.exists() + is_dir()`」的最小檢查，authorization-audit 正式寫入延到獨立 `authorization-middleware` change
- **SSE event 統一 schema validator** — 每個 event type 形狀目前散在 `docs/sidecar-api.md §四`；pydantic schema 落地是 polish 期事，MVP 走 dict

**拒絕的設計**：

- **把 SSE emit 直接硬編到 `run_explorer` / `TrackedProvider`**（不經 Emitter abstraction）—— 破壞 in-process 測試（Explorer 單測不該被 SSE 需求綁住）；Emitter + `None` default 是唯一乾淨解
- **每個 tool method 自呼 `emitter.emit`** —— 責任該在 loop 層（`_execute_one` 拿到 `ToolResult` 後 emit `agent_action_result`），tool method 不該知道 SSE 存在
- **用 WebSocket 取代 SSE** —— D-008 / `docs/sidecar-api.md §641` 已定 SSE over HTTP 夠用；WebSocket 需雙向但 Agent 只單向 emit

## Capabilities

### New Capabilities

- `explorer-sse`：`POST /explore` endpoint + SSEEmitter abstraction + Explorer loop / Judge / TrackedProvider / LLMCallLogger 的 emit wiring；支撐前端 Agent console 即時進度（`docs/sidecar-api.md §四`）。

### Modified Capabilities

- `sidecar-runtime`：`task_id format` 加 `explore` 分支（regex extend）

設計決策：Explorer loop 的 emitter 注入、TrackedProvider 與 LLMCallLogger 的 SSE emit，全部寫進新 `explorer-sse` capability 的 ADDED Requirements，不去修既有的 agent-core / llm-provider / usage-tracking 三支 spec。優點：既有 capability 描述保持乾淨；未來擴 event type 可集中在 explorer-sse；emitter 以未注入為預設值保守相容，既有 Requirement Scenarios 全部仍成立。

## Impact

**受影響 spec**：
- `openspec/specs/explorer-sse/spec.md`（新建）
- `openspec/specs/sidecar-runtime/spec.md`（modify）
- `openspec/specs/agent-core/spec.md`（modify）
- `openspec/specs/llm-provider/spec.md`（modify）
- `openspec/specs/usage-tracking/spec.md`（modify）

**受影響 code**：
- `sidecar/src/codebus_agent/api/explore.py`（新檔）
- `sidecar/src/codebus_agent/api/__init__.py`（註冊 explore router；可能也 wire judge/reasoning-logger factory）
- `sidecar/src/codebus_agent/api/tasks.py`（擴 TaskKind / ERROR_CODES / classify）
- `sidecar/src/codebus_agent/agent/emitter.py`（新檔）
- `sidecar/src/codebus_agent/agent/context_vars.py`（新檔）
- `sidecar/src/codebus_agent/agent/explorer.py`（加 emitter 參數 + 三個 emit 點）
- `sidecar/src/codebus_agent/agent/judge.py`（不動 — Judge 呼 `await provider.chat`，emit 由 TrackedProvider 層發）
- `sidecar/src/codebus_agent/providers/tracked.py`（加 emitter 參數 + usage_delta emit）
- `sidecar/src/codebus_agent/providers/llm_call_logger.py`（加 emitter 參數 + llm_call emit）

**受影響測試**：
- 新：`tests/api/test_explore_endpoint.py` / `tests/agent/test_sse_emitter.py` / `tests/providers/test_tracked_provider_sse.py` / `tests/providers/test_llm_call_logger_sse.py`
- 既有（emitter=None 保持相容）：`tests/agent/test_explorer_loop.py` / `tests/agent/test_explorer_loop_with_real_tools.py` / `tests/providers/test_tracked_provider.py` / `tests/providers/test_default_module.py` — 不需改動

**受影響文件**：
- `docs/sidecar-api.md §三` 加 `POST /explore`
- `docs/sidecar-api.md §四` 的 event schema 目前完整；本 change 不增 event type
- `docs/agent-core.md §十二` 拿掉「SSE 由 ReasoningLogger 直發」過時描述；改為「由 `SSEEmitter` 注入，Explorer loop 呼」
- `CLAUDE.md` archive 時間軸

**無新依賴**（`sse-starlette` 已在；`contextvars` 是 stdlib）。
