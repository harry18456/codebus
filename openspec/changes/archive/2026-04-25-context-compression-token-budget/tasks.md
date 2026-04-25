## 1. Scaffolding

- [x] 1.1 建立 test scaffolding：`sidecar/tests/agent/test_budget_probe.py`、`sidecar/tests/agent/test_token_budget_enforcement.py`、`sidecar/tests/agent/test_message_rolling_window.py`、`sidecar/tests/agent/test_budget_warning_event.py` 四個占位檔（`from __future__ import annotations` + module docstring 指向 spec requirement + 空 TODO 佔位）
- [x] 1.2 `sidecar/tests/agent/conftest.py` 補共用 fixture：`scripted_token_probe`（可受控回傳 total 的 test double，類比 P0 `_CountingCoverage` 模式）

## 2. RED — `TrackedProvider exposes session token counters`

對應 spec requirement `TrackedProvider exposes session token counters`（五 scenario：初始零、成功 chat 前進、失敗不動、embed 僅加 prompt、per-instance 獨立）。覆蓋 design Decision 1：TrackedProvider 加 session token counters（對稱於 cost）。

- [x] 2.1 [P] `test_tracked_provider_tokens.py::test_session_token_counters_start_at_zero`（落實 spec requirement `TrackedProvider exposes session token counters`）—— 純 construct、assert `session_prompt_tokens` / `session_completion_tokens` / `session_total_tokens` 都 `== 0`
- [x] 2.2 [P] `test_tracked_provider_tokens.py::test_successful_chat_advances_both_counters` —— push `CoverageResult` / `JudgeVerdict` 進 MockScript、assert `session_prompt_tokens` 增量 = `prompt_tokens`、`session_completion_tokens` 增量 = `completion_tokens`、`session_total_tokens` = 兩者和
- [x] 2.3 [P] `test_tracked_provider_tokens.py::test_failed_chat_leaves_counters_unchanged` —— MockProvider raise（用 sentinel exception）、assert counters 前後相等、`llm_calls.jsonl` 仍有 failure record
- [x] 2.4 [P] `test_tracked_provider_tokens.py::test_embed_advances_prompt_counter_only` —— 成功 embed、assert `session_prompt_tokens += embed_tokens`、`session_completion_tokens` 不動
- [x] 2.5 [P] `test_tracked_provider_tokens.py::test_counters_are_per_instance_not_shared` —— 兩個 TrackedProvider 實例、一個跑 chat、assert 另一個 counters 維持 `0`

## 3. GREEN — 實作 TrackedProvider session token counters

對應 design Decision 1：TrackedProvider 加 session token counters（對稱於 cost）與 spec requirement `TrackedProvider exposes session token counters`。

- [x] 3.1 `sidecar/src/codebus_agent/providers/tracked.py` 加三個 instance counter：`self._session_prompt_tokens: int = 0` / `self._session_completion_tokens: int = 0`；在 `_emit_usage_delta` 前呼的成功 path（`chat` / `embed` 都會走到）加 `self._session_prompt_tokens += prompt_tokens`、`self._session_completion_tokens += completion_tokens`（失敗 path 不動 — 不走 `_emit_usage_delta` 路徑，天然隔離）
- [x] 3.2 新三個 read-only property：`session_prompt_tokens` / `session_completion_tokens` / `session_total_tokens`（= `_session_prompt_tokens + _session_completion_tokens`）
- [x] 3.3 執行 `uv run pytest sidecar/tests/providers/test_tracked_provider_tokens.py` 直到 2.x 全綠

## 4. RED — `TokenBudgetProbe` Protocol + `AggregatedTokenProbe` 聚合

對應 design Decision 2：TokenBudgetProbe Protocol + AggregatedTokenProbe 具體 impl。

- [x] 4.1 [P] `test_budget_probe.py::test_token_budget_probe_is_runtime_checkable_protocol` —— 建一個 duck-typed class with `total() -> int`、assert `isinstance(x, TokenBudgetProbe)` 成立
- [x] 4.2 [P] `test_budget_probe.py::test_aggregated_probe_sums_across_providers` —— 三個 TrackedProvider 各跑一次 chat（不同 token 量）、`AggregatedTokenProbe([p1, p2, p3]).total()` 應等於三者 `session_total_tokens` 之和
- [x] 4.3 [P] `test_budget_probe.py::test_aggregated_probe_requires_at_least_one_provider` —— `AggregatedTokenProbe([])` 構造應 raise `ValueError`（避免 silent 0）
- [x] 4.4 [P] `test_budget_probe.py::test_aggregated_probe_is_zero_for_fresh_providers` —— 三個 fresh TrackedProvider（沒跑過 chat）、`total()` 應為 `0`

## 5. GREEN — 實作 `codebus_agent.agent.budget`

對應 design Decision 2：TokenBudgetProbe Protocol + AggregatedTokenProbe 具體 impl。

- [x] 5.1 新 `sidecar/src/codebus_agent/agent/budget.py`：`@runtime_checkable class TokenBudgetProbe(Protocol)` 單方法 `total() -> int`；`class AggregatedTokenProbe` 建構期吃 `providers: Sequence[TrackedProvider]` 並 assert `len >= 1`、`total()` 回 `sum(p.session_total_tokens for p in providers)`
- [x] 5.2 `sidecar/src/codebus_agent/agent/__init__.py` re-export `AggregatedTokenProbe` / `TokenBudgetProbe`
- [x] 5.3 執行 `uv run pytest sidecar/tests/agent/test_budget_probe.py` 直到 4.x 全綠

## 6. RED — `Explorer loop stops on budget exhaustion, empty queue, or cancel signal`（MODIFIED）

對應 MODIFIED spec requirement `Explorer loop stops on budget exhaustion, empty queue, or cancel signal`（擴 `stopped_reason` Literal 到四值 + 兩個新 scenario）。覆蓋 design Decision 3：`_should_stop` 分支優先序：cancel > budget_tokens > budget_steps > queue_empty。

- [x] 6.1 [P] `test_token_budget_enforcement.py::test_token_budget_exhaustion_terminates_loop` —— `scripted_token_probe(total=5000)` + `state.budget_tokens_left=5000`（= 達標）、assert `_think` 第一輪不執行、`stopped_reason == "budget_tokens_exhausted"`
- [x] 6.2 [P] `test_token_budget_enforcement.py::test_missing_token_probe_leaves_budget_unenforced` —— 不傳 `token_probe`、`state.budget_tokens_left=1`（極小值）、push ExplorerActions、assert loop 正常跑到 budget_steps_exhausted、`stopped_reason != "budget_tokens_exhausted"`
- [x] 6.3 [P] `test_token_budget_enforcement.py::test_precedence_token_budget_over_step_budget` —— token 與 step 同輪同時觸發、assert `stopped_reason == "budget_tokens_exhausted"`（design Decision 3 precedence）
- [x] 6.4 [P] `test_token_budget_enforcement.py::test_cancel_still_wins_over_token_budget` —— cancel_event 已 set + token 亦達標、assert `stopped_reason == "cancelled"`
- [x] 6.5 [P] `test_token_budget_enforcement.py::test_stopped_reason_propagates_through_coverage_recursion` —— 外層 queue_empty 收斂、coverage 觸發遞迴、內層 token 耗盡、assert 最外層 `result.stopped_reason == "budget_tokens_exhausted"`（tail-recursion propagation，`coverage-gap-recurse` landed 形狀）
- [x] 6.6 [P] `sidecar/tests/agent/test_types.py` 新 `test_explorer_result_stopped_reason_literal_includes_budget_tokens_exhausted` —— import `ExplorerResult`、assert Literal 值集為 `{"budget_exhausted", "queue_empty", "cancelled", "budget_tokens_exhausted"}`

## 7. GREEN — `_should_stop` 第四分支 + Literal 擴值

對應 design Decision 3：`_should_stop` 分支優先序：cancel > budget_tokens > budget_steps > queue_empty 與 MODIFIED spec requirement `Explorer loop stops on budget exhaustion, empty queue, or cancel signal`。

- [x] 7.1 `sidecar/src/codebus_agent/agent/types.py` 擴 `ExplorerResult.stopped_reason: Literal[...]` 加第四值 `"budget_tokens_exhausted"`（additive）
- [x] 7.2 `sidecar/src/codebus_agent/agent/explorer.py` 加 `_should_stop(state, cancel_event, token_probe) -> tuple[bool, str | None]`（signature 擴第三參數、default None）；順序：cancel → token（`token_probe is not None AND token_probe.total() >= state.budget_tokens_left` → `"budget_tokens_exhausted"`）→ budget_steps → queue_empty
- [x] 7.3 `run_explorer(..., token_probe: TokenBudgetProbe | None = None)` 加新 keyword-only 參數；所有 `_should_stop(state, cancel_event)` 呼叫改成 `_should_stop(state, cancel_event, token_probe)`；tail-recursion 的 `return await run_explorer(...)` 加 `token_probe=token_probe` 保遞迴沿用
- [x] 7.4 執行 `uv run pytest sidecar/tests/agent/test_token_budget_enforcement.py sidecar/tests/agent/test_types.py` 直到 6.x 全綠

## 8. RED — `Explorer applies rolling message window before each Think call`（ADDED）

對應 ADDED spec requirement `Explorer applies rolling message window before each Think call`。覆蓋 design Decision 4：`_MESSAGE_ROLLING_WINDOW = 16` 固定窗口，不 token-aware 與 Decision 5：Rolling window 只影響 wire prompt，不動 state.messages。

- [x] 8.1 [P] `test_message_rolling_window.py::test_think_receives_at_most_window_size_messages_when_state_grew_larger` —— 手動 populate `state.messages` 到 20 條、spy `inner.chat` messages 長度、assert 送進 provider 的是 16 + system + user = 18 條
- [x] 8.2 [P] `test_message_rolling_window.py::test_think_preserves_all_state_messages_when_below_window` —— `state.messages` 放 5 條、assert wire prompt 是 5 + system + user = 7 條
- [x] 8.3 [P] `test_message_rolling_window.py::test_rolling_window_does_not_mutate_state_messages` —— populate 20 條、跑 N 輪、assert `state.messages` 長度 `>= 20` 且前 20 條原封不動
- [x] 8.4 [P] `test_message_rolling_window.py::test_reasoning_log_records_full_iteration_history_despite_windowing` —— 長 run、assert 每行 Step 的 `tool_results` 完整（不被 window 切）
- [x] 8.5 [P] `test_message_rolling_window.py::test_coverage_gap_recursion_frame_respects_same_window` —— 透過 scripted_coverage_checker 觸發 recursion、inner frame `_enqueue_gap_investigation` 新塞的 user 訊息 assert 在 windowed slice 最後一筆

## 9. GREEN — `_think` rolling window 實作

對應 design Decision 4：`_MESSAGE_ROLLING_WINDOW = 16` 固定窗口，不 token-aware 與 Decision 5：Rolling window 只影響 wire prompt，不動 state.messages 與 ADDED spec requirement `Explorer applies rolling message window before each Think call`。

- [x] 9.1 `sidecar/src/codebus_agent/agent/explorer.py` 新 module constant `_MESSAGE_ROLLING_WINDOW: int = 16`
- [x] 9.2 `_think(state, provider, tool_specs)` 在既有 `_to_provider_messages(state.messages) + [...]` 拼接前，先 `windowed = state.messages[-_MESSAGE_ROLLING_WINDOW:]`，把後續用的 `state.messages` 全換成 `windowed`（注意只動 _think 裡的 local、不動 state 本尊；`_append_observations` / `_update_state` 等 state mutator 不變）
- [x] 9.3 執行 `uv run pytest sidecar/tests/agent/test_message_rolling_window.py` 直到 8.x 全綠

## 10. RED — `Explorer emits budget_warning SSE event at 80% threshold`（ADDED）

對應 ADDED spec requirement `Explorer emits budget_warning SSE event at 80% threshold`。覆蓋 design Decision 6：`_BUDGET_WARNING_PCT = 0.8` + per-kind once semantic 與 Decision 8：`budget_warning` SSE event 觸發點—— Update 之後、progress 之前。

- [x] 10.1 [P] `test_budget_warning_event.py::test_first_iteration_crossing_step_threshold_emits_warning` —— `initial_budget_steps=5`、push 4 個 actions、spy emitter 捕 `budget_warning` kind=steps、`current=4` `budget=5` `pct=0.8`、於該輪 progress event 之前
- [x] 10.2 [P] `test_budget_warning_event.py::test_token_budget_crosses_threshold_before_step_budget` —— `scripted_token_probe(total 階梯式：0→4001 在第二輪)`、`budget_tokens_left=5000`、assert emit kind=tokens、沒有 kind=steps
- [x] 10.3 [P] `test_budget_warning_event.py::test_both_thresholds_cross_emit_once_per_kind` —— 長 run、tokens 與 steps 都會跨過 0.8、assert exactly one kind=tokens + exactly one kind=steps（per-kind once）
- [x] 10.4 [P] `test_budget_warning_event.py::test_missing_emitter_suppresses_all_warnings` —— `emitter=None`、assert spy（未 wire）依舊 empty、result terminal 行為一致
- [x] 10.5 [P] `test_budget_warning_event.py::test_missing_token_probe_suppresses_tokens_warning_only` —— `token_probe=None`、steps threshold 仍跨過、assert kind=steps emit、kind=tokens 絕無

## 11. GREEN — `budget_warning` SSE emit + `usage_delta` 擴欄

對應 design Decision 6：`_BUDGET_WARNING_PCT = 0.8` + per-kind once semantic、Decision 8：`budget_warning` SSE event 觸發點—— Update 之後、progress 之前、MODIFIED spec requirement `TrackedProvider emits usage_delta on every completed call`（加 session_total_tokens 欄位）、ADDED spec requirement `Explorer emits budget_warning SSE event at 80% threshold`。

- [x] 11.1 `sidecar/src/codebus_agent/agent/explorer.py` 新 module constant `_BUDGET_WARNING_PCT: float = 0.8`；新小 `@dataclass class _BudgetWarningState: warned_tokens: bool = False; warned_steps: bool = False`
- [x] 11.2 `run_explorer` 在主 while 進入前建一個 `warning_state = _BudgetWarningState()`；在 Update 步驟後、`progress` emit 前呼 `_maybe_emit_budget_warning(emitter, warning_state, state, initial_budget_steps, token_probe)`
- [x] 11.3 新 `_maybe_emit_budget_warning(...)`：`kind="tokens"` 檢查 `token_probe is not None AND not warning_state.warned_tokens AND token_probe.total() / state.budget_tokens_left >= _BUDGET_WARNING_PCT` → emit + flip `warned_tokens=True`；`kind="steps"` 類比用 `(initial_budget_steps - state.budget_steps_left) / initial_budget_steps`
- [x] 11.4 `sidecar/src/codebus_agent/providers/tracked.py::_emit_usage_delta` 加 `"session_total_tokens": int(self.session_total_tokens)` 欄位（additive，既有 event key 不動；對應 MODIFIED `TrackedProvider emits usage_delta on every completed call`）
- [x] 11.5 執行 `uv run pytest sidecar/tests/agent/test_budget_warning_event.py sidecar/tests/providers/test_tracked_provider_sse.py` 直到 10.x 全綠且既有 tracked-sse 測試擴欄後不破

## 12. GREEN — Judge / Coverage 暴露 `provider` property

對應 design Decision 7：Judge / Coverage 暴露 `provider` property，不暴露整個 `_provider`。

- [x] 12.1 `sidecar/src/codebus_agent/agent/judge.py::LLMJudge` 加 `@property def provider(self) -> TrackedProvider: return self._provider`
- [x] 12.2 `sidecar/src/codebus_agent/agent/coverage.py::LLMCoverageChecker` 加同形狀 `provider` property
- [x] 12.3 新 `sidecar/tests/agent/test_evaluator_provider_property.py` 雙測：`test_llm_judge_exposes_provider_property` / `test_llm_coverage_checker_exposes_provider_property`（回的是同一顆 TrackedProvider 實例）

## 13. HTTP 層接線：aggregator 餵進 `run_explorer`

對應 MODIFIED spec requirement `Explorer loop stops on budget exhaustion, empty queue, or cancel signal` 與 design Decision 2：TokenBudgetProbe Protocol + AggregatedTokenProbe 具體 impl、Decision 7：Judge / Coverage 暴露 `provider` property。

- [x] 13.1 [P] `sidecar/tests/api/test_explore_endpoint.py` 新 `test_explore_endpoint_wires_aggregated_token_probe` —— spy `run_explorer`（monkeypatch）、assert 被呼時 `token_probe` 參數為 `AggregatedTokenProbe` 實例且 `providers` 長度 = 3（reasoning + judge + coverage）
- [x] 13.2 `sidecar/src/codebus_agent/api/explore.py::explore_endpoint` 在建完 `reasoning_provider` / `judge` / `coverage` 三元組後組 `token_probe = AggregatedTokenProbe([reasoning_provider, judge.provider, coverage.provider])`、餵給 `run_explorer(..., token_probe=token_probe)`

## 14. 完整 SSE integration 測試擴欄

對應 MODIFIED spec requirement `TrackedProvider emits usage_delta on every completed call`（session_total_tokens 欄位）與 ADDED spec requirement `Explorer emits budget_warning SSE event at 80% threshold`。

- [x] 14.1 `sidecar/tests/api/test_explore_sse_integration.py` 把 `usage_delta` assertion 擴 `session_total_tokens: int >= 0` 欄位存在檢查
- [x] 14.2 `sidecar/tests/api/test_explore_sse_integration.py` 加 `budget_warning` 至 **可選** event 集（short run 不保證觸發，故進 optional；若觸發則 schema 檢查 `kind in {"tokens", "steps"}`）

## 15. 文件與 repo metadata 更新

- [x] 15.1 `CLAUDE.md` archive 時間軸加入本 change；「下一步」改指向 **步驟 22 `前端 Agent console 消費 SSE`**（SSE emit 已 landed，剩前端接）
- [x] 15.2 `docs/agent-core.md §十` Context 壓縮 改成真實落地形狀（`_MESSAGE_ROLLING_WINDOW=16` + state.messages 不變 + wire prompt 窗化）；§十一 Budget 控制改「token budget 已 landed、wall-clock 延後」；§四 `_should_stop` precedence 表改四值 + §十二 SSE event 清單加 `budget_warning` + `usage_delta.session_total_tokens` 欄位
- [x] 15.3 `docs/implementation-plan.md` 步驟 21 狀態 `⏳` → `✅ landed（context-compression-token-budget）`

## 16. 驗證與 commit gate

- [x] 16.1 執行 `uv run pytest sidecar/tests/agent/` 無 regression（Budget probe / token budget / rolling window / warning event / 既有 Explorer-loop / coverage-recursion 全綠）
- [x] 16.2 執行 `uv run pytest sidecar/tests/providers/` 無 regression（TrackedProvider session counters + usage_delta 擴欄 + 既有 cost 測試全綠）
- [x] 16.3 執行 `uv run pytest sidecar/tests/api/test_explore_endpoint.py sidecar/tests/api/test_explore_sse_integration.py` 全綠（token_probe wiring + session_total_tokens SSE 欄位 + optional budget_warning）
- [x] 16.4 執行 `uv run pytest sidecar/tests/` 完整 suite 無 regression（golden-sample replay 的 `stopped_reason="budget_exhausted"` drift guard 不觸發，因 replay 不傳 token_probe、token branch 不觸發）
- [x] 16.5 執行 `pre-commit run --all-files` 全綠
