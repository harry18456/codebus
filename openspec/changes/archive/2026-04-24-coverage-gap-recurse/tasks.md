## 1. Scaffolding

- [x] 1.1 建立 test scaffolding：`sidecar/tests/agent/test_coverage.py`、`sidecar/tests/agent/test_coverage_recursion.py`、`sidecar/tests/agent/prompts/test_coverage_prompt.py` 三個占位檔（`from __future__ import annotations` + module docstring 指向 spec requirement + 空 TODO 佔位）
- [x] 1.2 在 `sidecar/tests/agent/conftest.py` 補共用 fixture：`scripted_coverage_checker`（可受控回傳 gap 清單的 test double 類比 P0 `_CountingCoverage`）、`captured_coverage_provider`（用 MockProvider + `MockScript` 推入 `CoverageResult` payload）

## 2. RED — `LLMCoverageChecker produces one-shot CoverageResult`

對應 spec requirement `LLMCoverageChecker produces one-shot CoverageResult`（含四個 scenario：one-shot 合約 / 不 mutate state / set_emitter 轉發 / prompt 版本 constant）。覆蓋 design 決策「Decision 7：LLMCoverageChecker 與 Judge 並排的 factory 形狀」。

- [x] 2.1 [P] `test_coverage.py::test_check_issues_one_shot_structured_call`（落實 spec requirement `LLMCoverageChecker produces one-shot CoverageResult`）—— push `CoverageResult(gaps=[Gap(...), Gap(...)])` 進 MockProvider script、呼 `check(state)`、assert `provider.chat` 被呼一次且 `response_model == CoverageResult`
- [x] 2.2 [P] `test_coverage.py::test_check_does_not_mutate_explorer_state` —— 建 `ExplorerState` snapshot（stations / visited_files / pending_queue / messages / step_count / budget_steps_left），跑 `check`，assert 六欄位全等
- [x] 2.3 [P] `test_coverage.py::test_set_emitter_propagates_to_tracked_provider` —— 用 spy emitter 餵 `LLMCoverageChecker.set_emitter`，assert 後續 `check` 觸發 `usage_delta` / `llm_call` event 到同 spy
- [x] 2.4 [P] `test_coverage_prompt.py::test_render_coverage_prompt_is_deterministic_on_sorted_state` —— 給定固定 state，連跑兩次 `render_coverage_prompt(state)`，assert 輸出位元一致
- [x] 2.5 [P] `test_coverage_prompt.py::test_render_coverage_prompt_windows_visited_at_20_with_more_footer` —— state.visited_files 塞 30 項，assert 輸出含前 20 條 + `... (10 more)` footer
- [x] 2.6 [P] `test_coverage_prompt.py::test_coverage_prompt_version_is_date_version_format` —— import `COVERAGE_PROMPT_VERSION`，assert match `r"^\d{4}-\d{2}-\d{2}-\d+$"`

## 3. GREEN — 實作 LLMCoverageChecker 與 prompt 模組

對應 design 決策「Decision 7」與 spec requirement `LLMCoverageChecker produces one-shot CoverageResult`。

- [x] 3.1 新 `sidecar/src/codebus_agent/agent/prompts/coverage.py` —— `COVERAGE_SYSTEM`（角色邊界 / Gap 判準 / 輸出格式三段式，與 Judge prompt 同結構）、`render_coverage_prompt(state)`（visited window 20 + stations 列表 + 任務）、`COVERAGE_PROMPT_VERSION = "2026-04-26-1"`
- [x] 3.2 在 `sidecar/src/codebus_agent/agent/prompts/__init__.py` re-export `COVERAGE_SYSTEM` / `render_coverage_prompt` / `COVERAGE_PROMPT_VERSION`
- [x] 3.3 新 `sidecar/src/codebus_agent/agent/coverage.py`：`LLMCoverageChecker(provider_factory, workspace_root)`、`set_emitter`、`async check(state) -> list[Gap]`（one-shot `chat(response_model=CoverageResult)`、回 `result.gaps`）
- [x] 3.4 `sidecar/src/codebus_agent/agent/__init__.py` re-export `LLMCoverageChecker`
- [x] 3.5 執行 `uv run pytest sidecar/tests/agent/test_coverage.py sidecar/tests/agent/prompts/test_coverage_prompt.py` 直到 2.x 全綠

## 4. RED — `Coverage-gap recursion runs after main loop convergence`

對應 spec requirement `Coverage-gap recursion runs after main loop convergence`（四個 scenario：empty gaps / gaps with budget / max depth / budget exhausted）。覆蓋 design 決策「Decision 1：tail-recursion」「Decision 2：重用同一份 state」「Decision 3：`_depth` 參數」「Decision 4：Coverage round Step 格式」「Decision 6：`_enqueue_gap_investigation` 雙推」。

- [x] 4.1 [P] `test_coverage_recursion.py::test_empty_gaps_terminate_without_recursion`（落實 spec requirement `Coverage-gap recursion runs after main loop convergence`）—— `scripted_coverage_checker` 回 `[]`，assert `run_explorer` 不重入、不寫 coverage Step、`_enqueue_gap_investigation` spy 沒被呼
- [x] 4.2 [P] `test_coverage_recursion.py::test_gaps_with_budget_trigger_one_recursion_round` —— 回 2 個 gap + budget=5，assert pending_queue 多 2 條、messages 多 1 條 role="user"、reasoning_log 多一行 `[coverage] round-1 gaps=2 will_recurse=True` Step、recursive call `_depth=1`
- [x] 4.3 [P] `test_coverage_recursion.py::test_max_depth_halts_further_recursion` —— `_depth=2` 進入、coverage 回 1 gap、assert 不遞迴、Step `[coverage] round-3 gaps=1 will_recurse=False` 寫入
- [x] 4.4 [P] `test_coverage_recursion.py::test_budget_exhaustion_halts_recursion_even_with_gaps` —— `budget_steps_left=0` + 1 gap、assert 不遞迴、`_enqueue_gap_investigation` 不被呼、Step `will_recurse=False` 寫入
- [x] 4.5 [P] `test_coverage_recursion.py::test_enqueue_gap_investigation_uses_placeholder_when_suggested_target_is_none` —— `Gap(description="...", suggested_target=None)`、assert `state.pending_queue[-1] == f"gap:{desc[:80]}"`
- [x] 4.6 [P] `test_coverage_recursion.py::test_stopped_reason_propagates_through_recursion` —— 遞迴最內層 budget 耗盡收斂，assert 最外層 `ExplorerResult.stopped_reason == "budget_exhausted"`
- [x] 4.7 [P] 改寫 `test_explorer_loop.py::test_coverage_recursion_hook_remains_dormant_in_p0` → `test_coverage_recursion_hook_activates_after_main_loop_convergence`（配合 MODIFIED requirement `ReAct loop executes think-act-observe-judge-log-update each iteration` scenario `Coverage recursion hook activates after main loop convergence`）：用可控 coverage spy + 空 gap 驗證會呼 `coverage.check` 一次但不遞迴

## 5. GREEN — 實作 `_enqueue_gap_investigation` 與 run_explorer 遞迴體

對應 design 決策 1-6 與 spec requirement `Coverage-gap recursion runs after main loop convergence`。

- [x] 5.1 `sidecar/src/codebus_agent/agent/explorer.py` 新 `_COVERAGE_MAX_DEPTH: int = 3` module constant；保留 `_COVERAGE_RECURSION_ENABLED` 暫作為 feature flag（值翻 `True`）
- [x] 5.2 實作 `_enqueue_gap_investigation(state, gaps)`（對應 design 決策「Decision 6：`_enqueue_gap_investigation` 的 pending_queue 與 messages 雙推」）：placeholder rule `gap:{desc[:80]}`、messages 用「Coverage 回報 {N} 個 gap：{summary}。請優先補查。」模板、gap description 摘要最多取前 3 條
- [x] 5.3 `run_explorer` 新 keyword-only 參數 `_depth: int = 0`（對應 design 決策「Decision 1：遞迴 vs iterative loop — 用 tail-recursion」、「Decision 2：遞迴體重用同一份 `state`」、「Decision 3：`_depth` 參數 vs 全域 counter」）；在 main while 退出後呼 `coverage.check(state)` 一次，判 `(len(gaps) > 0, budget > 0, _depth < _COVERAGE_MAX_DEPTH)` 三條件；滿足時呼 `_enqueue_gap_investigation` + `logger.write(coverage Step)` + `return await run_explorer(..., _depth=_depth+1)`
- [x] 5.4 coverage Step 用 `Step(step=state.step_count, ts=now, thought=f"[coverage] round-{_depth+1} gaps={len(gaps)} will_recurse={will_recurse}", tool_calls=[], tool_results=[], judge_verdict=None, tokens_used=0, explorer_prompt_version=EXPLORER_PROMPT_VERSION, judge_prompt_version=JUDGE_PROMPT_VERSION)`（對應 design 決策「Decision 4：Coverage round 的 reasoning_log Step 表示」）；**不** 增 `state.step_count`（coverage round 不是 iteration）
- [x] 5.5 不寫 coverage Step 當 `len(gaps) == 0`（對應 design 決策「Decision 8：空 gaps 的語意 — 仍發 SSE，不寫 Step」）
- [x] 5.6 執行 `uv run pytest sidecar/tests/agent/test_coverage_recursion.py sidecar/tests/agent/test_explorer_loop.py` 直到 4.x 全綠

## 6. RED — `Coverage round emits coverage_gaps SSE event`

對應 spec requirement `Coverage round emits coverage_gaps SSE event`（五個 scenario：非空 gaps + 遞迴 / 空 gaps / 預算耗盡 / max depth / 無 emitter）。覆蓋 design 決策「Decision 5：`coverage_gaps` SSE event 格式」。

- [x] 6.1 [P] `test_coverage_recursion.py::test_coverage_gaps_event_fires_before_recursion`（落實 spec requirement `Coverage round emits coverage_gaps SSE event`）—— 用 spy emitter、gaps 非空 + budget 充足，assert emit 收到 `{"type":"coverage_gaps","round":0,"gaps":[...],"will_recurse":true,"skip_reason":null}`、event 早於 recursive `run_explorer` 呼叫
- [x] 6.2 [P] `test_coverage_recursion.py::test_coverage_gaps_event_no_gaps_skip_reason` —— empty gaps，assert emit `skip_reason="no_gaps"`、`will_recurse=false`
- [x] 6.3 [P] `test_coverage_recursion.py::test_coverage_gaps_event_budget_exhausted_skip_reason` —— budget=0 + 1 gap，assert `skip_reason="budget_exhausted"`
- [x] 6.4 [P] `test_coverage_recursion.py::test_coverage_gaps_event_max_depth_skip_reason` —— `_depth=2` + 1 gap + budget>0，assert `skip_reason="max_depth_reached"`
- [x] 6.5 [P] `test_coverage_recursion.py::test_coverage_gaps_event_suppressed_when_emitter_none` —— emitter=None，assert 遞迴行為等同 emitter-set 情境；spy 完全不被呼

## 7. GREEN — SSE emit 實作與 skip_reason 優先序

- [x] 7.1 在 `run_explorer` 的 coverage 區段實作 `_coverage_skip_reason(gaps, budget_ok, depth_ok) -> str | None`：按 `no_gaps > max_depth_reached > budget_exhausted` 優先序回 reason 字串，遞迴觸發時回 None
- [x] 7.2 emit `coverage_gaps` event（`_emitter.emit({...})`）塞在「coverage.check 回傳後、`logger.write(coverage Step)` 之前」的位置；gaps 欄位用 `[g.model_dump() for g in gaps]`
- [x] 7.3 執行 `uv run pytest sidecar/tests/agent/test_coverage_recursion.py` 直到 6.x 全綠

## 8. HTTP 層接線與 LLM factory slot

對應 design 決策「Decision 7：LLMCoverageChecker 與 Judge 並排的 factory 形狀」。

- [x] 8.1 [P] `sidecar/tests/api/test_explore_endpoint.py` 加 `test_explore_endpoint_requires_llm_coverage_provider` —— 拔掉 `app.state.llm_coverage_provider`、送 `POST /explore`，assert 回 503 + error message 列出 `llm_coverage_provider`
- [x] 8.2 `sidecar/src/codebus_agent/api/__init__.py::wire_llm_dependencies`（或對應 DI 函式）加 `app.state.llm_coverage_provider = _make_chat_provider_factory(default_module="coverage", temperature=0.0)`（OpenAI key 未設時跟 judge/reasoning 一樣設 `None`）；對應 design 決策「Decision 7：`LLMCoverageChecker` 與 Judge 並排的 factory 形狀」
- [x] 8.3 `sidecar/src/codebus_agent/api/explore.py::_require_explore_deps` 加 `coverage_factory = getattr(state, "llm_coverage_provider", None)` 的必要 slot 檢查，缺 slot → 503 + error message 同 judge / reasoning 格式
- [x] 8.4 `POST /explore` handler：`coverage = LLMCoverageChecker(coverage_factory, workspace_root)` + `coverage.set_emitter(emitter)`、餵給 `run_explorer(...)`（取代任何 `_NoopCoverage()` 佔位）

## 9. 文件與 repo metadata 更新

- [x] 9.1 `CLAUDE.md` archive 時間軸加入本 change；「下一步」改指向 **步驟 21 `context 壓縮 + token-aware budget`**
- [x] 9.2 `docs/agent-core.md §七` Judge / Coverage Checker code block 替換成真實落地形狀（`LLMCoverageChecker` 類 + `check` 方法 + `_COVERAGE_MAX_DEPTH` 常數）；§九 錯誤處理表「Coverage 遞迴過深」行標為 `✅ 步驟 20 landed（本 change）`；§五 update-state 區段補「coverage round Step 不增 step_count」註記
- [x] 9.3 `docs/implementation-plan.md` 步驟 20 狀態 `⏳` → `✅ landed（coverage-gap-recurse）`

## 10. 驗證與 commit gate

- [x] 10.1 執行 `uv run pytest sidecar/tests/agent/` 無 regression（Coverage recursion / Explorer loop / Judge / 工具全綠）
- [x] 10.2 執行 `uv run pytest sidecar/tests/api/test_explore_endpoint.py` 全綠（503 coverage_provider + happy-path recursion wiring）
- [x] 10.3 執行 `uv run pytest sidecar/tests/` 完整 suite 無 regression（golden-sample replay 必要時 re-baseline 並更新 `tests/golden/demo-synthetic/expected.json` 的 stopped_reason / step_count 欄位）
- [x] 10.4 執行 `pre-commit run --all-files` 全綠
