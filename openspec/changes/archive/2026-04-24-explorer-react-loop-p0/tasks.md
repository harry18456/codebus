## 1. Scaffolding

- [x] 1.1 建立 `sidecar/src/codebus_agent/agent/` 套件目錄與 `__init__.py`(re-export 主要 symbol)；加入 `sidecar/src/codebus_agent/agent/prompts/` 子目錄與 `__init__.py`
- [x] 1.2 建空 stub：`agent/types.py` / `agent/protocols.py` / `agent/explorer.py` / `agent/judge.py` / `agent/reasoning_logger.py` / `agent/prompts/explorer.py` / `agent/prompts/judge.py`(各含 `__all__` + `NotImplementedError` 讓 Section 3+ 的 RED 測試失敗時有訊號)
- [x] 1.3 建 test 目錄 `sidecar/tests/agent/__init__.py`；建 conftest `sidecar/tests/agent/conftest.py` 提供 `workspace_dir` / `mock_reasoning_provider` / `mock_judge_provider` 等共用 fixture

## 2. RED — `Agent-core types are Pydantic BaseModels with stable JSON serialization`

對應 spec `agent-core / Agent-core types are Pydantic BaseModels with stable JSON serialization`。

- [x] 2.1 [P] `tests/agent/test_types.py` 加 `test_explorer_action_round_trips`(落實 spec Requirement `Agent-core types are Pydantic BaseModels with stable JSON serialization`)— raw JSON `{"thought": "...", "tool_calls": [], "stop": false}` 經 `ExplorerAction.model_validate_json` → `model_dump_json()` 再 `json.loads` 應與原 object 等值
- [x] 2.2 [P] `test_types.py` 加 `test_step_round_trips_with_nested_verdict` — `Step` with 嵌套 `JudgeVerdict` / `ToolResult` round-trip 不失資料
- [x] 2.3 [P] `test_types.py` 加 `test_judge_verdict_rejects_out_of_range_relevance` — payload `{"relevance": 1.5, ...}` 過 `JudgeVerdict.model_validate_json` 必 raise `ValidationError`(bounds 0..1)
- [x] 2.4 [P] `test_types.py` 加 `test_explorer_state_required_fields` — 缺 `task` / `budget_steps_left` / `budget_tokens_left` 必 raise `ValidationError`

## 3. GREEN — 實作 `agent/types.py`

- [x] 3.1 `agent/types.py` 定義 `Message`(role / content / tool_call_id / tool_name)
- [x] 3.2 `agent/types.py` 定義 `ToolCall`(id / name / arguments: `dict[str, Any]`) 與 `ToolResult`(tool_call_id / tool_name / output / raw / error)
- [x] 3.3 `agent/types.py` 定義 `JudgeVerdict`(relevance: `Field(ge=0, le=1)` / should_follow_imports / should_add_station / reason)、`CoverageResult`(gaps list) 與 `Gap`(schema stub 供未來 Coverage change)
- [x] 3.4 `agent/types.py` 定義 `Station`(path / role / relevance / why / depends_on) 與 `ExplorerState`(task / messages / visited_files: `set[str]` / pending_queue / stations / budget_steps_left / budget_tokens_left / step_count)
- [x] 3.5 `agent/types.py` 定義 `ExplorerAction`(thought / tool_calls / stop) 與 `ExplorerResult`(stations / log_path / stopped_reason: Literal)
- [x] 3.6 `agent/types.py` 定義 `Step`(step / ts / thought / tool_calls / tool_results / judge_verdict / tokens_used / `explorer_prompt_version` / `judge_prompt_version`)—— 多的兩個 version 欄位就是 `ReasoningLogger` Requirement 要寫進 JSONL 的那兩個
- [x] 3.7 執行 `uv run pytest sidecar/tests/agent/test_types.py` 確認 2.1 ~ 2.4 全綠

## 4. RED — `ExplorerTools, Judge, and CoverageChecker are structural Protocols`

對應 spec `agent-core / ExplorerTools, Judge, and CoverageChecker are structural Protocols`。

- [x] 4.1 [P] `tests/agent/test_protocols.py` 加 `test_mock_tools_satisfies_explorer_tools_protocol`(落實 spec Requirement `ExplorerTools, Judge, and CoverageChecker are structural Protocols`)— 純結構(無 inherit)的 `MockTools` 類別實作 `primary_search` / `fetch` / `follow_reference` 三個 coroutine 即 `isinstance(mock, ExplorerTools) is True`
- [x] 4.2 [P] `test_protocols.py` 加 `test_mock_judge_satisfies_judge_protocol` 與 `test_mock_coverage_satisfies_coverage_checker_protocol`(同 pattern)
- [x] 4.3 [P] `test_protocols.py` 加 `test_protocols_do_not_bind_folder_mode_types` — 用 `inspect.signature` 驗 `ExplorerTools.primary_search` 的參數是抽象 `str -> list[SearchHit]`,非 folder-specific 型別(例如不直接拿 `Path`)

## 5. GREEN — 實作 `agent/protocols.py`

- [x] 5.1 `agent/protocols.py` 定義輔助 Pydantic 型別 `SearchHit`(path / snippet / score)、`Content`(path / text / lines_range)、`Target`(kind / args / priority)、`Gap`(description / suggested_target)
- [x] 5.2 `agent/protocols.py` 定義 `ExplorerTools(Protocol)` + `@runtime_checkable`,三個 coroutine method 照 `docs/agent-explorer-spec.md §十二.7` 設計
- [x] 5.3 `agent/protocols.py` 定義 `Judge(Protocol)` + `@runtime_checkable`,`evaluate(state, results) -> JudgeVerdict`
- [x] 5.4 `agent/protocols.py` 定義 `CoverageChecker(Protocol)` + `@runtime_checkable`,`check(state) -> list[Gap]`
- [x] 5.5 執行 `uv run pytest sidecar/tests/agent/test_protocols.py` 確認 4.1 ~ 4.3 全綠

## 6. RED — `ReasoningLogger appends one JSONL line per Step to workspace path`

對應 spec `agent-core / ReasoningLogger appends one JSONL line per Step to workspace path`。

- [x] 6.1 [P] `tests/agent/test_reasoning_logger.py` 加 `test_each_write_appends_one_jsonl_line`(落實 spec Requirement `ReasoningLogger appends one JSONL line per Step to workspace path`)— `ReasoningLogger(path).write(step)` 連呼 K 次 → 檔案恰 K 行,每行以 `\n` 收尾,每行可經 `Step.model_validate_json` 還原
- [x] 6.2 [P] `test_reasoning_logger.py` 加 `test_prompt_version_columns_present` — JSON 物件必含 `explorer_prompt_version` 與 `judge_prompt_version` 字串欄位,值等於當下 module-level 常數
- [x] 6.3 [P] `test_reasoning_logger.py` 加 `test_write_failure_propagates` — 把 workspace 設成 read-only 或路徑不可寫,`write()` 必 raise,不得靜默丟失

## 7. GREEN — 實作 `agent/reasoning_logger.py` + prompt 模組

- [x] 7.1 `agent/prompts/explorer.py` 定義 `EXPLORER_SYSTEM` 字串常數(zh-TW)、`EXPLORER_PROMPT_VERSION = "v0-p0"` 常數、`render_explorer_prompt(state, tool_specs) -> str`
- [x] 7.2 `agent/prompts/judge.py` 定義 `JUDGE_SYSTEM` 字串常數、`JUDGE_PROMPT_VERSION = "v0-p0"` 常數、`render_judge_prompt(task, results) -> str`
- [x] 7.3 `agent/prompts/__init__.py` re-export 上述 render 與 version 常數
- [x] 7.4 `agent/reasoning_logger.py` 實作 `ReasoningLogger(path: Path)`:`write(step: Step)` 內建 `Step.model_copy(update={"explorer_prompt_version": EXPLORER_PROMPT_VERSION, "judge_prompt_version": JUDGE_PROMPT_VERSION})` 後 append `model_dump_json() + "\n"`;write 失敗不 catch,直接往上丟
- [x] 7.5 執行 `uv run pytest sidecar/tests/agent/test_reasoning_logger.py` 確認 6.1 ~ 6.3 全綠

## 8. RED — `Judge evaluation runs as one-shot call per iteration`

對應 spec `agent-core / Judge evaluation runs as one-shot call per iteration`。

- [x] 8.1 [P] `tests/agent/test_judge.py` 加 `test_llm_judge_returns_validated_verdict`(落實 spec Requirement `Judge evaluation runs as one-shot call per iteration`)— `LLMJudge(provider_factory)` with `MockProvider` 腳本回 `JudgeVerdict(relevance=0.8, ...)` → `evaluate(state, results)` 回同型 instance
- [x] 8.2 [P] `test_judge.py` 加 `test_judge_is_stateless_with_respect_to_state` — 呼叫前後 `state.stations` / `state.visited_files` / `state.step_count` 不變
- [x] 8.3 [P] `test_judge.py` 加 `test_judge_provider_role_is_judge_not_reasoning` — 注入的 factory 產出的 TrackedProvider `role == ProviderRole.JUDGE` 且 `_default_module == "judge"`
- [x] 8.4 [P] `test_judge.py` 加 `test_judge_does_not_invoke_explorer_tools` — 一個監控用的 spy `ExplorerTools` 在 Judge 呼叫中 primary_search / fetch / follow_reference 都不應被觸發(one-shot 不進 ReAct sub-loop)

## 9. GREEN — 實作 `agent/judge.py`

- [x] 9.1 `agent/judge.py` 實作 `LLMJudge(provider_factory: Callable[[Path], TrackedProvider])`,`evaluate(state, results)` 內構 messages → `await provider.chat(messages, response_model=JudgeVerdict)` → return
- [x] 9.2 `agent/judge.py` `LLMJudge` 不得 import `ExplorerTools` 具體類;不得讀寫 `state` 欄位
- [x] 9.3 執行 `uv run pytest sidecar/tests/agent/test_judge.py` 確認 8.1 ~ 8.4 全綠

## 10. RED — ReAct 主迴圈(三個 Requirement 一組)

對應三個 Requirement:
- `ReAct loop executes think-act-observe-judge-log-update each iteration`
- `Explorer Think step validates ExplorerAction via Instructor`
- `Explorer loop stops on budget exhaustion, empty queue, or cancel signal`

- [x] 10.1 [P] `tests/agent/test_explorer_loop.py` 加 `test_each_iteration_executes_think_act_observe_judge_log_update` — run_explorer 跑 N 輪,驗 `_think` 呼 N 次、`judge.evaluate` 呼 N 次、`logger.write` 呼 N 次,每個 `Step.step` 等於 0..N-1
- [x] 10.2 [P] `test_explorer_loop.py` 加 `test_explorer_think_validates_explorer_action_via_instructor` — MockProvider 腳本回 `ExplorerAction(thought="x", tool_calls=[], stop=False)` → `_think` 回 `(thought, tool_calls)` tuple,且內部 `provider.chat` 必帶 `response_model=ExplorerAction`
- [x] 10.3 [P] `test_explorer_loop.py` 加 `test_observations_feed_forward_into_next_think` — 第 K 輪 tool 結果 R1 / R2 → 第 K+1 輪 `_think` 的 messages 必含 role=`"tool"` 且 content 反映 R1.output / R2.output
- [x] 10.4 [P] `test_explorer_loop.py` 加 `test_tool_errors_do_not_crash_loop` — Spy `ExplorerTools.fetch` 故意 raise,`ToolResult.error` 必記下,loop 繼續 Judge + Log + Update,`reasoning_log.jsonl` 的該行含失敗 `ToolResult`
- [x] 10.5 [P] `test_explorer_loop.py` 加 `test_coverage_recursion_hook_remains_dormant_in_p0` — 即使 `CoverageChecker.check` 回大量 gaps,`run_explorer` 也**不得**遞迴呼自己(用 counter spy 驗)
- [x] 10.6 [P] `test_explorer_loop.py` 加 `test_budget_exhaustion_terminates_loop` — `state.budget_steps_left = 0` 進 run_explorer → `_think` 一次都不呼,`ExplorerResult.stopped_reason == "budget_exhausted"`
- [x] 10.7 [P] `test_explorer_loop.py` 加 `test_cancel_event_short_circuits_mid_run` — 跑到 K 輪後 set `asyncio.Event`,K+1 輪的 `_think` / `_execute_tools` / `judge.evaluate` 都不觸發,結果 `stopped_reason == "cancelled"`,`stations` 保留 K 輪前的內容
- [x] 10.8 [P] `test_explorer_loop.py` 加 `test_queue_empty_with_enough_stations_terminates_cleanly` — 某輪後 `pending_queue == []` 且 `len(stations) >= _MIN_STATIONS_FOR_CONVERGENCE` → 下一輪 `_should_stop` True,`stopped_reason == "queue_empty"`

## 11. GREEN — 實作 `agent/explorer.py`

- [x] 11.1 `agent/explorer.py` 定義 `_MIN_STATIONS_FOR_CONVERGENCE = 3` 常數(對齊 spec Requirement `Explorer loop stops on budget exhaustion, empty queue, or cancel signal` 提到的 sensible P0 default)
- [x] 11.2 `agent/explorer.py` 實作 `_should_stop(state, cancel_event)` — 三分支判斷;cancel 分支用 `asyncio.Event.is_set()` 非同步安全
- [x] 11.3 `agent/explorer.py` 實作 `_think(state, provider, tool_specs)` — render prompt → `await provider.chat(state.messages + [user_msg], response_model=ExplorerAction)` → return `(action.thought, action.tool_calls)`(落實 `Explorer Think step validates ExplorerAction via Instructor` 的 single-call + TrackedProvider-only 契約)
- [x] 11.4 `agent/explorer.py` 實作 `_execute_tools(calls, tools)` — `asyncio.gather` 並行 `_execute_one(call, tools)`;單一 tool 失敗包 `ToolResult.error`,不往外拋(落實 `ReAct loop executes think-act-observe-judge-log-update each iteration` 的 tool-error scenario)
- [x] 11.5 `agent/explorer.py` 實作 `_append_observations(state, calls, results)` — 把每個 result 轉 `Message(role="tool", content=result.output, tool_call_id=call.id, tool_name=call.name)` append 到 `state.messages`
- [x] 11.6 `agent/explorer.py` 實作 `_update_state(state, results, verdict)` — 依 `verdict.should_add_station` 追加 `Station`、`should_follow_imports` 追加 `pending_queue`、`results[*].tool_name == "read_file"` 追加 `visited_files`(簡化版 — 本 P0 不做 Judge 細節只做欄位 fold)
- [x] 11.7 `agent/explorer.py` 實作 `run_explorer(state, provider, tools, judge, coverage, logger, cancel_event=None)` 主迴圈 — 照 spec Requirement `ReAct loop executes think-act-observe-judge-log-update each iteration` 六步序;迴圈頭 `_should_stop` 檢查;終止產出 `ExplorerResult`;Coverage 遞迴 hook 明文 `if False:  # enabled by coverage-gap-recurse change` 夾住不跑
- [x] 11.8 執行 `uv run pytest sidecar/tests/agent/test_explorer_loop.py` 確認 10.1 ~ 10.8 全綠

## 12. 文件 + Repo metadata 更新

- [x] 12.1 於 `CLAUDE.md` §Repo 現況 的 archive 時間軸加入本 change(Module 4 Explorer ReAct skeleton);「目前沒有 in-progress change」句子相應更新到下一步指向 `explorer-tools-p0`(步驟 17)
- [x] 12.2 檢查 `docs/agent-core.md` §三(資料結構)§四(主 ReAct 迴圈)§七(Judge / Coverage)§十二(reasoning_log)與實作是否對齊,有漂移的欄位命名 / 模組切分 / Protocol 簽章回頭同步該文件;本 change 不動該文件的整體結構

## 13. 驗證與 commit gate

- [x] 13.1 執行 `uv run pytest sidecar/tests/agent/` 確認 agent 層全綠
- [x] 13.2 執行 `uv run pytest sidecar/tests/` 完整 suite 無 regression
- [x] 13.3 執行 `pre-commit run --all-files` 全綠
