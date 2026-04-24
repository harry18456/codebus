## Why

`explorer-react-loop-p0`（2026-04-24 archive）在 `run_explorer` 尾端埋了「Coverage check + 遞迴補查」的 code site，但以 `_COVERAGE_RECURSION_ENABLED = False` 夾住、`CoverageChecker` 只留 Protocol 抽象（`codebus_agent.agent.protocols.CoverageChecker`）。這塊 P0 刻意遞延給本 change（`docs/implementation-plan.md` **步驟 20**，依賴步驟 18 + 19；後者已在 `explorer-tools-p1` 2026-04-26 archive）——也就是 Agent 核心 Demo 靈魂還缺的最後一塊「發現 gap → 主動補查」閉環。

對齊 **D-012**（自寫 ReAct loop + Instructor/Pydantic structured output，Judge / Coverage 一律 one-shot）、**D-011**（所有 outbound LLM 走 TrackedProvider allowlist）、`docs/agent-core.md §七`（Judge / Coverage 同層設計）、`docs/agent-core.md §九`（「Coverage 遞迴過深 → 上限 3 層，超過停止並記 log warning」的錯誤處理策略）。

現況缺口很具體：Explorer budget 跑完 / queue 空但 stations 還稀薄時，loop 只會直接 `return ExplorerResult(...)`，根本沒機會「看一眼有沒有漏追」。Trust Layer 敘事是「像工程師一樣探索」——如果漏 gap 無法補查，Demo 到 Timeline repo 上就會露餡。

## What Changes

**新增 `LLMCoverageChecker`**（類比 `LLMJudge` 的 one-shot 形狀）：

- 新增 `codebus_agent.agent.coverage.LLMCoverageChecker`，建構期吃 `provider_factory: Callable[[Path], TrackedProvider]` + `workspace_root: Path`，內部以 `JUDGE` role 的 provider factory（`app.state.llm_judge_provider`）預先物化 TrackedProvider；`set_emitter(emitter)` 可轉發給內部 provider，讓 coverage-side `usage_delta` / `llm_call` 事件落到同一 SSE channel。
- `async check(state: ExplorerState) -> list[Gap]` — 渲染 coverage prompt、one-shot `provider.chat(messages, response_model=CoverageResult)`、回 `result.gaps`；**不** 進 ReAct 子迴圈、**不** 呼叫 `ExplorerTools`、**不** 變更 state（紀律與 Judge 對齊）。
- 新增 `codebus_agent.agent.prompts.coverage` 模組：`COVERAGE_SYSTEM` / `render_coverage_prompt(state)` / `COVERAGE_PROMPT_VERSION = "2026-04-26-1"`（date-version 格式，與 Judge 對齊）。

**打開 `_COVERAGE_RECURSION_ENABLED` 並填遞迴體**：

- 把 `sidecar/src/codebus_agent/agent/explorer.py` 的 `_COVERAGE_RECURSION_ENABLED = False` 改 `True`，把 `# pragma: no cover - intentionally dormant` 註解撤掉。
- 新常數 `_COVERAGE_MAX_DEPTH: int = 3`（上限 3 層，對齊 `docs/agent-core.md §九`）。
- 新內部函式 `_enqueue_gap_investigation(state, gaps)`：把每個 `Gap` 轉成 `state.pending_queue` 的一筆 entry（Gap.suggested_target 非空用之；空則用 `f"gap:{gap.description[:80]}"` placeholder 保留訊號）。同時把一則 `Message(role="user", content="Coverage 回報 N 個 gap：<摘要>，請優先補查。")` append 進 `state.messages` 讓下一輪 Think 看到 gap 指示。
- `run_explorer` 多一個 keyword-only 參數 `_depth: int = 0`（底線前綴表示實作細節，不是 public API），遞迴時 `_depth + 1`；到 `_COVERAGE_MAX_DEPTH` 時不再遞迴，仍回傳 accumulated stations。
- 遞迴只在「`state.budget_steps_left > 0`」且「`len(gaps) > 0`」且「`_depth < _COVERAGE_MAX_DEPTH`」三條件同時成立時觸發。重入 `run_explorer` 時重用同一份 `state`（budget 已隨前一段扣減）+ 同一 `logger` / `provider` / `tools` / `judge` / `coverage` / `emitter`。
- 遞迴啟動前先寫一行 `Step` 記錄 coverage round：`thought="[coverage] round-<N> gaps=<k>"`、`tool_calls=[]`、`tool_results=[]`、`judge_verdict=None`，用 `logger.write(...)` 落盤；`_think` 與 `_execute_tools` 仍由下一層 `run_explorer` 重新進入。這樣 reasoning_log 能精準 replay「哪一輪是主 loop、哪一輪是 gap 補查」。

**SSE emit `coverage_gaps` 事件**：

- 在 coverage check 之後、遞迴之前，`emitter.emit({"type": "coverage_gaps", "round": _depth, "gaps": [{"description": ..., "suggested_target": ...}, ...], "will_recurse": bool})`；這讓 Trust Layer 前端能顯示「Agent 發現 N 個 gap → 是否補查」敘事。
- 無 gap（`gaps == []`）時仍 emit 一次（`will_recurse=False`），讓前端知道 coverage 已查無 gap，收斂「乾淨結束」。
- 遞迴觸發條件不滿足（budget 耗盡 / 已達 depth 上限）時 emit `will_recurse=False` 並附一個 `skip_reason: "budget_exhausted" | "max_depth_reached" | "no_gaps"`。

**HTTP 層接線**（`sidecar/src/codebus_agent/api/explore.py`）：

- `POST /explore` 呼叫 `run_explorer(...)` 時傳入一個真 CoverageChecker 實例（從新 `app.state.llm_coverage_provider` factory 生出），不再傳 `_NoopCoverage()`。
- `app.state.llm_coverage_provider` 這個新 factory slot 在 `api/__init__.py::wire_llm_dependencies` 與 `llm_judge_provider` 並列建構，共用 `_make_chat_provider_factory(default_module="coverage", temperature=0.0)`；temperature 與 Judge 對齊（low-temp 確定性輸出）。

**MODIFIED 既有 scenario（agent-core capability）**：

- 既有 scenario `Coverage recursion hook remains dormant in P0`（來源：`explorer-react-loop-p0`）改為 `Coverage recursion hook activates up to MAX_DEPTH rounds`，明確陳述新行為（budget > 0 + gaps > 0 + depth < 3 → 遞迴；否則回傳）。
- 既有 scenario `Each iteration writes exactly one Step line`（`ReAct loop executes think-act-observe-judge-log-update each iteration`）維持不動 —— coverage round 的 Step 行不屬於「iteration」，Explorer iteration 定義仍以主迴圈 while body 為準。

**ADDED 新 Requirement（explorer-sse capability）**：

- 新 Requirement `Coverage round emits coverage_gaps SSE event` 描述 `coverage_gaps` event 的 wire schema、emit 時機（coverage.check 完成後、遞迴觸發前）、以及「無 gap」與「遞迴被擋下」情境下的 `skip_reason` 值。

**測試層**：

- `sidecar/tests/agent/test_coverage.py`（新）—— `LLMCoverageChecker` one-shot 合約：`chat(response_model=CoverageResult)` 被呼到、state 不被 mutate、emitter 轉發、空 gaps / 多 gaps 輸出格式。
- `sidecar/tests/agent/test_coverage_recursion.py`（新）—— Explorer 層的遞迴行為：
  - 空 gaps → 不遞迴 + SSE 發 `will_recurse=False` 附 `skip_reason="no_gaps"`
  - 多 gaps + budget 充足 → 遞迴一次，pending_queue 增加、reasoning_log 多一行 coverage round Step
  - depth 到達 3 → 停止，不再遞迴
  - budget_steps_left 耗盡 → 不遞迴（即使 gaps 非空）
  - `_enqueue_gap_investigation` 對 `suggested_target=None` 的 gap 用 description 截 80 字 placeholder
- `sidecar/tests/agent/prompts/test_coverage_prompt.py`（新）—— `render_coverage_prompt(state)` 輸出 deterministic、版本 constant 格式。
- 既有 `sidecar/tests/agent/test_explorer_loop.py` 的 `test_coverage_recursion_hook_remains_dormant_in_p0` 改寫為 `test_coverage_recursion_hook_activates_after_main_loop` 並加 assertion（coverage.check 被呼一次；有 gap 則遞迴；無則不遞迴）。
- 既有 `sidecar/tests/golden/test_explorer_replay.py` fixture 的 `expected.json` 若因 prompt/emit 變動需要 re-baseline 就同步更新，保留 drift guard。

**文件同步**：

- `CLAUDE.md` 加 archive 時間軸、把「下一步」改指向 **步驟 21 `context 壓縮 + token-aware budget`**。
- `docs/agent-core.md §七`（Judge / Coverage Checker）code block 替換成真實落地形狀；§九 錯誤處理表的「Coverage 遞迴過深」行標為 landed。
- `docs/implementation-plan.md` 步驟 20 狀態標為 ✅ landed。

## Non-Goals

明確排除：

- **Coverage gap 的 non-LLM fallback**：不做 keyword-heuristic 偵測（「visited 數 < X 且 task 包含某 keyword 就強塞 gap」）。P0 場景下 LLM 已足夠；lightweight fallback 延到實際看到 provider 不穩才開獨立 change。
- **Gap 內嵌再遞迴**：遞迴體內**不**再跑 coverage check — 一次 gap round 結束就**回傳**（不管本輪 run 有沒有又產生新 gap）。避免遞迴內遞迴讓 depth / budget 帳變形。本 change 的 3 層上限指的是「主 loop + 最多 2 次 gap 補查」。
- **Gap 優先排序**：按 `CoverageResult.gaps` 收到的順序 FIFO 塞進 `state.pending_queue`，不做 relevance-based ranking。
- **前端 UI**：`coverage_gaps` event 格式本 change 落定，Nuxt UI 顯示延到 Module 7 實作期。
- **Gap schema 擴欄**：`Gap(description, suggested_target)` 保持現狀；不加 `severity` / `confidence` 等欄位。追加要獨立 change + bump schema version。
- **Coverage prompt golden baseline**：`explorer-golden` capability 目前只覆蓋 Explorer + Judge 的 baseline；coverage round 的 baseline 延到與 Module 5 Generator 接線後再做（兩者共用同一 demo fixture）。

**拒絕的設計**：

- **「把 coverage 搬進 while 迴圈每輪跑」**：語意層面會和 Judge 重疊（都變成 per-step 檢查），而且每輪一次 LLM call 對 Haiku budget 浪費，違反「Coverage 只在收斂時跑一次」（`docs/agent-core.md §七`）。
- **「用 iterative loop 取代 tail-recursion」**：自呼 `run_explorer` 直觀且 stack trace 乾淨；Python recursion depth 3 層對堆疊壓力零，沒必要搬成 while。
- **「depth 上限做成可注入 config」**：MVP 硬編 3；要調就改 constant + spec scenario 同步。過早 config 化只是打開測試面。

## Capabilities

### New Capabilities

（無 —— 所有改動掛在既有 `agent-core` / `explorer-sse` capability 上）

### Modified Capabilities

- `agent-core`：
  - MODIFIED Requirement `ReAct loop executes think-act-observe-judge-log-update each iteration`（scenario `Coverage recursion hook remains dormant in P0` 改寫）。
  - ADDED Requirement `Coverage-gap recursion runs after main loop convergence`（新增 coverage round 行為規格與四個 scenario：空 gaps / 多 gaps 遞迴 / depth 上限 / budget 耗盡）。
  - ADDED Requirement `LLMCoverageChecker produces one-shot CoverageResult`（類比 `Judge evaluation runs as one-shot call per iteration`：CoverageChecker 的 one-shot 合約、provider_factory 建構、set_emitter 轉發、state 不被 mutate）。

- `explorer-sse`：
  - ADDED Requirement `Coverage round emits coverage_gaps SSE event`（wire schema、emit 時機、`skip_reason` 值集三個 scenario）。

## Impact

**受影響 spec**：

- `openspec/specs/agent-core/spec.md`（MODIFIED — 1 個 Requirement scenario 改寫 + 2 個 Requirement 新增）
- `openspec/specs/explorer-sse/spec.md`（MODIFIED — 1 個 Requirement 新增）

**受影響 code**（必須 touch 的檔案）：

- `sidecar/src/codebus_agent/agent/explorer.py`（翻開 `_COVERAGE_RECURSION_ENABLED`、加 `_COVERAGE_MAX_DEPTH`、`_enqueue_gap_investigation`、`run_explorer` 加 `_depth` 參數與遞迴邏輯、SSE emit）
- `sidecar/src/codebus_agent/agent/coverage.py`（新檔 —— `LLMCoverageChecker`）
- `sidecar/src/codebus_agent/agent/prompts/coverage.py`（新檔 —— `COVERAGE_SYSTEM` / `render_coverage_prompt` / `COVERAGE_PROMPT_VERSION`）
- `sidecar/src/codebus_agent/agent/prompts/__init__.py`（re-export）
- `sidecar/src/codebus_agent/agent/__init__.py`（re-export `LLMCoverageChecker`）
- `sidecar/src/codebus_agent/api/__init__.py`（加 `app.state.llm_coverage_provider` factory slot，沿用 `_make_chat_provider_factory` 形狀）
- `sidecar/src/codebus_agent/api/explore.py`（construct `LLMCoverageChecker` 餵給 `run_explorer`、set_emitter）

**受影響測試**：

- 新：`sidecar/tests/agent/test_coverage.py`
- 新：`sidecar/tests/agent/test_coverage_recursion.py`
- 新：`sidecar/tests/agent/prompts/test_coverage_prompt.py`
- 更新：`sidecar/tests/agent/test_explorer_loop.py`（`test_coverage_recursion_hook_remains_dormant_in_p0` → 改寫）
- 更新：`sidecar/tests/api/test_explore_endpoint.py`（若現有 test 對 `CoverageChecker` 的 Noop 假設需要改成 LLMCoverageChecker mock）
- 潛在更新：`sidecar/tests/golden/test_explorer_replay.py`（若 prompt/emit 改動讓 baseline 偏移就 re-baseline）

**受影響文件**：

- `CLAUDE.md`（archive 時間軸 + 「下一步」導向步驟 21）
- `docs/agent-core.md`（§七 code block 更新、§九 Coverage 遞迴上限行標 landed、§十七 Day 7 對應本 change）
- `docs/implementation-plan.md`（步驟 20 狀態更新）

**無新依賴**（Instructor / Pydantic / TrackedProvider / SSE emitter 皆既有）。

**無 breaking change**（`run_explorer` 新參數 `_depth` 有 default；`ExplorerResult.stopped_reason` 的 Literal 不變；既有呼叫端不動）。
