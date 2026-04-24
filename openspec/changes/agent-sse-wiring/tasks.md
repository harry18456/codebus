## 1. Scaffolding

- [x] 1.1 建立 `sidecar/src/codebus_agent/agent/emitter.py`（空 stub 含 `__all__` + `NotImplementedError`）與 `sidecar/src/codebus_agent/agent/context_vars.py`（空 stub）
- [x] 1.2 建立 `sidecar/src/codebus_agent/api/explore.py`（空 stub 含 `router = APIRouter()` 即可；endpoint handler 空 return）
- [x] 1.3 建 test scaffolding：`sidecar/tests/agent/test_sse_emitter.py` 占位；`sidecar/tests/providers/test_tracked_provider_sse.py` 占位；`sidecar/tests/providers/test_llm_call_logger_sse.py` 占位；`sidecar/tests/api/test_explore_endpoint.py` 占位

## 2. RED — `SSEEmitter is an opt-in runtime-checkable Protocol`

對應 spec `explorer-sse / SSEEmitter is an opt-in runtime-checkable Protocol`。

- [x] 2.1 [P] `tests/agent/test_sse_emitter.py` 加 `test_null_emitter_satisfies_protocol`(落實 spec Requirement `SSEEmitter is an opt-in runtime-checkable Protocol`)—— `isinstance(NullEmitter(), SSEEmitter)` True；呼 `.emit({...})` 不 raise、不寫任何 side effect
- [x] 2.2 [P] `test_sse_emitter.py` 加 `test_task_handle_emitter_fans_out` —— `TaskHandleEmitter(handle).emit({"type": "progress"})` 後，`handle` 的每個 subscriber queue 都收到該 event
- [x] 2.3 [P] `test_sse_emitter.py` 加 `test_custom_impl_without_inherit_satisfies_protocol` —— 純結構 `class MyEmitter: def emit(self, event): ...` 經 `isinstance(MyEmitter(), SSEEmitter)` 回 True（驗 `@runtime_checkable`）

## 3. GREEN — 實作 `agent/emitter.py`

- [x] 3.1 `agent/emitter.py` 定義 `SSEEmitter(Protocol)` + `@runtime_checkable`；單方法 `emit(event: dict) -> None`
- [x] 3.2 `agent/emitter.py` 實作 `NullEmitter` class（`emit` no-op）與 `TaskHandleEmitter(handle: TaskHandle)` class（`emit` delegate 回 `handle.emit(event)`）
- [x] 3.3 執行 `uv run pytest sidecar/tests/agent/test_sse_emitter.py` 確認 2.1 ~ 2.3 全綠

## 4. RED — `Explorer loop emits agent_thought / agent_action_result / judge_verdict events`

對應 spec `explorer-sse / Explorer loop emits agent_thought / agent_action_result / judge_verdict events`。

- [x] 4.1 [P] `tests/agent/test_explorer_loop_sse.py`（新檔）加 `test_three_event_types_fire_per_iteration_in_order`(落實 spec Requirement `Explorer loop emits agent_thought / agent_action_result / judge_verdict events`)—— `run_explorer(..., emitter=spy)` 跑一輪帶 non-empty tool_calls → spy 收到 `agent_thought` → `agent_action_result` → `judge_verdict` 三 event、都同 step、每 event 的 step 值等於 `state.step_count` at iteration start
- [x] 4.2 [P] `test_explorer_loop_sse.py` 加 `test_missing_emitter_preserves_legacy_behavior` —— `run_explorer(...)` 不傳 emitter，既有測試 `test_each_iteration_executes_think_act_observe_judge_log_update` style 行為不變；return value / logger.writes 皆相同
- [x] 4.3 [P] `test_explorer_loop_sse.py` 加 `test_observation_truncation_bounds_channel_payload` —— tool 回 10_000-char output；對應 `agent_action_result.observation` 欄位 length ≤ 500 + 截斷 marker；`reasoning_log.jsonl` 的 `tool_results[0].output` 仍完整
- [x] 4.4 [P] `test_explorer_loop_sse.py` 加 `test_progress_event_also_fires_each_iteration` —— 每輪一個 `{"type": "progress", "phase": "exploring", "current": step, "total": budget}` event 與其他三個共存

## 5. GREEN — 擴 `run_explorer`

- [x] 5.1 `agent/explorer.py` `run_explorer` 加 `emitter: SSEEmitter | None = None` kwarg；內部做 `_emitter = emitter or NullEmitter()` 以免熱路徑寫 `if emitter is not None`
- [x] 5.2 在 Think 結束後 emit `{"type": "agent_thought", "step": state.step_count, "thought": thought, "action": [c.model_dump() for c in tool_calls]}`
- [x] 5.3 在 `_execute_tools` 回 list 後 emit N 個 `{"type": "agent_action_result", "step": state.step_count, "tool": r.tool_name, "observation": r.output[:500] or f"ERROR: {r.error}"[:500], "tokens_used": 0}`（P0 `tokens_used=0`，真實值延後）
- [x] 5.4 在 Judge 結束後 emit `{"type": "judge_verdict", "step": state.step_count, "relevance": verdict.relevance, "reason": verdict.reason}`
- [x] 5.5 每輪結尾 emit `{"type": "progress", "phase": "exploring", "current": state.step_count, "total": initial_budget_steps}`（`initial_budget_steps` 在迴圈前 snapshot）
- [x] 5.6 執行 `uv run pytest sidecar/tests/agent/test_explorer_loop_sse.py sidecar/tests/agent/test_explorer_loop.py` 確認新測全綠且舊測不 regression

## 6. RED — `TrackedProvider emits usage_delta on every completed call`

對應 spec `explorer-sse / TrackedProvider emits usage_delta on every completed call`。

- [x] 6.1 [P] `tests/providers/test_tracked_provider_sse.py` 加 `test_emitter_fires_after_token_usage_jsonl_write`(落實 spec Requirement `TrackedProvider emits usage_delta on every completed call`)—— `TrackedProvider(MockProvider(), ..., emitter=spy).chat(...)` 成功 → spy 收到 1 筆 `type=usage_delta`、`module` 等於構造時的 `default_module`、`prompt_tokens` / `completion_tokens` 非 None
- [x] 6.2 [P] `test_tracked_provider_sse.py` 加 `test_failed_call_suppresses_usage_delta` —— inner `chat` raise；spy 沒收到 `usage_delta`；`llm_calls.jsonl` 的 failure 行仍寫
- [x] 6.3 [P] `test_tracked_provider_sse.py` 加 `test_omitting_emitter_preserves_existing_behavior` —— 不傳 emitter，既有 `token_usage.jsonl` / `llm_calls.jsonl` 行為不變；`tests/providers/test_default_module.py` 既有測試需照舊通過
- [x] 6.4 [P] `test_tracked_provider_sse.py` 加 `test_context_var_scopes_phase_and_step` —— 用 `current_phase.set("explore")` + `current_step.set(3)`，event 的 `phase == "explore"`、`step == 3`；未 set 時 `phase`、`step` 為 None

## 7. GREEN — `agent/context_vars.py` + 擴 `TrackedProvider`

- [x] 7.1 `agent/context_vars.py` 定義三個 `contextvars.ContextVar`：`current_phase: ContextVar[str | None]`（default None）/ `current_step: ContextVar[int | None]` / `current_session: ContextVar[str | None]`；每個曝露 getter helper
- [x] 7.2 `providers/tracked.py` 加 `emitter: SSEEmitter | None = None` kwarg；內部 `self._emitter = emitter` 保留
- [x] 7.3 `TrackedProvider.chat` / `embed` 成功 path 在 `self._tracker.record(...)` 之後 emit（失敗 path 不 emit）；event payload 照 spec 塞 `phase` / `step` from context var、`session_total_cost_usd` from `self._tracker.session_total()`
- [x] 7.4 注意 `session_total()` 若 `UsageTracker` API 尚未曝露，此任務需同步加最小 read helper 或用累加器本地計算
- [x] 7.5 執行 `uv run pytest sidecar/tests/providers/test_tracked_provider_sse.py sidecar/tests/providers/test_tracked_provider.py sidecar/tests/providers/test_default_module.py` 確認全綠

## 8. RED — `LLMCallLogger emits llm_call event carrying preview`

對應 spec `explorer-sse / LLMCallLogger emits llm_call event carrying preview`。

- [x] 8.1 [P] `tests/providers/test_llm_call_logger_sse.py` 加 `test_successful_call_emits_llm_call_event`(落實 spec Requirement `LLMCallLogger emits llm_call event carrying preview`)—— `LLMCallLogger(..., emitter=spy).log(...)` 成功後 spy 收到 `type=llm_call`、`preview` 長度 ≤ 200、`llm_calls.jsonl` 行照舊寫
- [x] 8.2 [P] `test_llm_call_logger_sse.py` 加 `test_failed_call_still_emits_llm_call_event` —— `log_failure(...)` 後 spy 也收 `llm_call` event、`request_id` / `module` / `model` 欄位存在
- [x] 8.3 [P] `test_llm_call_logger_sse.py` 加 `test_omitted_emitter_preserves_file_only_behavior` —— emitter 缺席時 `tests/providers/test_llm_call_logger.py` 既有測試全過

## 9. GREEN — 擴 `LLMCallLogger`

- [x] 9.1 `providers/llm_call_logger.py` 加 `emitter: SSEEmitter | None = None` 參數
- [x] 9.2 `log` 與 `log_failure` 成功寫檔後 emit `{"type": "llm_call", ...}`；`preview` 從 `request["messages"]` 第一個 `role=="user"` message 的 `content[:200]` 取，無則 empty string
- [x] 9.3 執行 `uv run pytest sidecar/tests/providers/test_llm_call_logger_sse.py sidecar/tests/providers/test_llm_call_logger.py` 確認全綠

## 10. RED — `POST /explore endpoint spawns Explorer under task registry`

對應 spec `explorer-sse / POST /explore endpoint spawns Explorer under task registry`。

- [x] 10.1 [P] `tests/api/test_explore_endpoint.py` 加 `test_happy_path_returns_202_with_task_id`(落實 spec Requirement `POST /explore endpoint spawns Explorer under task registry`)—— 起 app with fake `llm_judge_provider` / `llm_reasoning_provider` factory；POST valid body；status 202；`task_id` 符 regex `^explore_[0-9a-f]{8}$`
- [x] 10.2 [P] `test_explore_endpoint.py` 加 `test_concurrent_task_rejected_409` —— 先佔 `TaskRegistry` 一個 scan running task，再 POST `/explore` → 409 / `TASK_IN_FLIGHT`
- [x] 10.3 [P] `test_explore_endpoint.py` 加 `test_missing_workspace_root_rejected` —— POST with `workspace_root="/does/not/exist"` → 400 或 404；`TaskRegistry.current_running()` 仍 None
- [x] 10.4 [P] `test_explore_endpoint.py` 加 `test_bearer_authentication_enforced` —— 不帶 `Authorization` header → 401

## 11. RED — `task_id format`(modified: explore kind)

對應 spec `sidecar-runtime / task_id format`。

- [x] 11.1 [P] `tests/api/test_task_registry.py`（既有檔）加 `test_explore_kind_follows_same_shape`(落實 spec Requirement `task_id format` 的 modified：加 explore kind)—— `registry.create("explore")` 回 `TaskHandle`，`id` 符 `^explore_[0-9a-f]{8}$`；同時一個 scan running → 再 `create("explore")` 回 None
- [x] 11.2 [P] `tests/api/test_task_registry.py` 加 `test_invalid_kind_other_than_explore_still_rejected` —— `registry.create("weird")` 必 raise `ValueError`，regex 擴張不應放行無關字串

## 12. GREEN — 擴 `TaskKind` + `api/explore.py`

- [x] 12.1 `api/tasks.py` `TaskKind = Literal["scan", "kb", "explore"]`；`_VALID_KINDS` 加 `"explore"`；既有測試不需改動
- [x] 12.2 `api/explore.py` 實作 endpoint：parse body → validate workspace_root (`Path.exists()` + `is_dir()`) → `registry.create("explore")` → 不 None 時 spawn background coroutine；coroutine 內構 `ToolContext` / `FolderTools` / `LLMJudge` / `ReasoningLogger` / `TaskHandleEmitter` 後呼 `run_explorer(..., emitter=emitter)`；走 `_run_background_task` 包裝
- [x] 12.3 `api/__init__.py` 註冊 `explore_router`（bearer middleware 下）；與 scan / kb / tasks router 同層；啟動 factory 依賴（reasoning / judge / kb provider）延用 `wire_kb_dependencies` 的 pattern
- [x] 12.4 執行 `uv run pytest sidecar/tests/api/test_explore_endpoint.py sidecar/tests/api/test_task_registry.py sidecar/tests/api/test_tasks_sse.py` 確認全綠

## 13. Integration — end-to-end SSE stream from `POST /explore`

- [x] 13.1 `tests/api/test_explore_sse_integration.py` 加 `test_explore_endpoint_emits_full_event_sequence` —— `httpx.AsyncClient` POST `/explore` 後訂閱 `/tasks/{id}/events`；用 MockProvider script 餵 3 個 `ExplorerAction`；驗 event stream 包含 `agent_thought` / `agent_action_result` / `judge_verdict` / `usage_delta` / `llm_call` / `progress` / `done`；各 event 的 `step` 在合理範圍
- [x] 13.2 執行 `uv run pytest sidecar/tests/api/test_explore_sse_integration.py` 確認全綠

## 14. 文件 + Repo metadata 更新

- [x] 14.1 `CLAUDE.md` archive 時間軸加入本 change；「下一步」指向步驟 18 `explorer-judge-golden`（Judge prompt 調 + golden sample 首跑）或步驟 19 `explorer-tools-p1`（`trace_import` / `find_callers`）
- [x] 14.2 `docs/sidecar-api.md §三` 加入 `POST /explore` endpoint 條目（request body / response shape）；`§四` 的 event schema 已完整不動
- [x] 14.3 `docs/agent-core.md §十二` 拿掉「SSE 由 ReasoningLogger 直發」過時描述；改為「由 `SSEEmitter` 注入，Explorer loop 呼 + TrackedProvider / LLMCallLogger 同步 emit」

## 15. 驗證與 commit gate

- [x] 15.1 執行 `uv run pytest sidecar/tests/agent/test_sse_emitter.py sidecar/tests/agent/test_explorer_loop_sse.py sidecar/tests/providers/test_tracked_provider_sse.py sidecar/tests/providers/test_llm_call_logger_sse.py sidecar/tests/api/test_explore_endpoint.py sidecar/tests/api/test_explore_sse_integration.py` 確認新增測試層全綠
- [x] 15.2 執行 `uv run pytest sidecar/tests/` 完整 suite 無 regression
- [x] 15.3 執行 `pre-commit run --all-files` 全綠
