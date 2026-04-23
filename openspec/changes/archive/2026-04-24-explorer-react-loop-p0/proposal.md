## Why

`chat-provider-wiring`(2026-04-23 archived)把 OpenAI chat-ish 三個 role(REASONING / JUDGE / CHAT)接上 `gpt-4o-mini`,但 **Agent 核心本體還是空的** —— `codebus_agent/` 目錄下沒有 `agent/` 子模組,只有 provider wire / scanner / KB builder。Module 4 Explorer Agent(D-012「自寫 ReAct + Instructor 輔助」)是整個產品的 Demo 靈魂(`docs/agent-explorer-spec.md §一`:「RAG 沒有自主決策;Explorer Agent 每步都是決策」),也是 `docs/implementation-plan.md` §四 第四階段 step 16 起的主戰場。

本 change 落地 **Agent 核心 ReAct skeleton**:types + 主迴圈 + 最小 Judge + `reasoning_log.jsonl` writer + `ExplorerTools` / `Judge` / `CoverageChecker` Protocol 抽象。真實工具(`search` / `list_dir` / `read_file` / `mark_station`)、Coverage gap 補查、Context 壓縮 / Budget 控制、SSE emit 留給後續 change——本次聚焦 ReAct 本身的訊號流通,`TrackedProvider` 已能用,只差一個會動的迴圈。

**合併 step 16 + 18 的理由**(見 `docs/implementation-plan.md` §一 五條強制規則):`reasoning_log.jsonl` 寫入規則明文要求「Explorer ReAct loop 第一次跑」前就要到位,因此 skeleton 必須自帶 logger,否則未來 retrofit 要改每個 step 的呼叫點。Judge protocol + 最小 impl 同理——沒有 Judge,ReAct 每輪走完沒有 relevance 訊號,state.stations 無法更新,log 也缺 `judge_verdict` 欄。

對齊 `docs/decisions.md` D-012(自寫 ReAct + Instructor)、D-013(專案組織)、D-021(UsageTracker)、D-022(LLMCallLogger)。

## What Changes

**新增 `codebus_agent.agent` 子套件**(`sidecar/src/codebus_agent/agent/`):

- **`types.py`** — ReAct 迴圈資料結構(Pydantic `BaseModel`):
  - `Message`(role / content / tool_call_id / tool_name)
  - `ToolCall`(id / name / arguments)
  - `ToolResult`(tool_call_id / tool_name / output / raw / error)
  - `Step`(step / ts / thought / tool_calls / tool_results / judge_verdict / tokens_used)—— `reasoning_log.jsonl` 的一行
  - `JudgeVerdict`(relevance / should_follow_imports / should_add_station / reason)
  - `CoverageResult`(gaps list;本 change 不用,先定義好 schema 讓 Coverage Checker change 直接接)
  - `Station`(path / role / relevance / why / depends_on)—— 探索成果
  - `ExplorerState`(task / messages / visited_files / pending_queue / stations / budget_steps_left / budget_tokens_left / step_count)
  - `ExplorerAction`(thought / tool_calls / stop)—— `_think` 的 Instructor `response_model`
  - `ExplorerResult`(stations / log_path / stopped_reason)

- **`protocols.py`** — `typing.Protocol` 抽象(D-012 強調「Phase 1 就要做對」,Topic mode / Q&A 未來用同一個 core):
  - `ExplorerTools`:`primary_search(query) -> list[SearchHit]` / `fetch(target) -> Content` / `follow_reference(symbol) -> list[Target]`(P0 只定介面不實作)
  - `Judge`:`evaluate(state, results) -> JudgeVerdict`
  - `CoverageChecker`:`check(state) -> list[Gap]`
  - 輔助型別:`SearchHit` / `Content` / `Target` / `Gap`

- **`explorer.py`** — 主 ReAct 迴圈(`docs/agent-core.md §四`骨架):
  - `run_explorer(state, provider, tools, judge, coverage, logger)` async entry
  - 內部函式:`_should_stop(state)` / `_think(state, provider, tool_specs)` / `_execute_tools(calls, tools)` / `_append_observations(state, calls, results)` / `_update_state(state, results, verdict)`
  - Cancel 走 `asyncio.Event` 每輪開頭檢查
  - 收斂條件三擇一:budget 用盡 / pending_queue 空 + stations 夠 / cancel 觸發——**Coverage gap 補查遞迴不在本 change 範圍**(step 20)
  - `_think` 用 Instructor 經 `provider.chat(messages, response_model=ExplorerAction)`,接 `TrackedProvider` 回的 validated Pydantic 實例

- **`judge.py`** — Relevance Judge 最小 impl:
  - `LLMJudge(provider_factory)` class 實現 `Judge` Protocol
  - `evaluate()` one-shot call,拿 `llm_judge_provider(ws)` 回的 TrackedProvider(JUDGE role / default_module=`"judge"`),temperature 0.0
  - `response_model=JudgeVerdict`
  - **不進 ReAct 迴圈內部**;是每輪結尾的獨立 call

- **`reasoning_logger.py`** — `reasoning_log.jsonl` writer:
  - `ReasoningLogger(path)` class;`write(step: Step)` 方法 append JSONL
  - path 約束於 workspace(walk through `ensure_in_workspace` 不在本 change 的 sandbox 檢查範圍——logger 只負責寫檔,caller 保證路徑合法)
  - **不含 SSE emit**(step 22 才做);僅本地檔案 I/O
  - 寫入欄位包含 `explorer_prompt_version` 與 `judge_prompt_version` 常數,方便未來對齊 golden sample

- **`prompts/explorer.py`**:
  - `EXPLORER_SYSTEM`(系統 prompt 常數,zh-TW)
  - `EXPLORER_PROMPT_VERSION = "v0-p0"` 常數
  - `render_explorer_prompt(state, tool_specs)` f-string render

- **`prompts/judge.py`**:
  - `JUDGE_SYSTEM` 常數
  - `JUDGE_PROMPT_VERSION = "v0-p0"` 常數
  - `render_judge_prompt(task, results)` f-string render

- **`prompts/__init__.py`** re-export render 函式 + version 常數

**受影響測試(`sidecar/tests/agent/` 新目錄)**:

- `test_types.py` — Pydantic schema 正確性(必填欄位、`ExplorerAction` round-trip)
- `test_reasoning_logger.py` — JSONL 寫入格式、append 行為、路徑驗證
- `test_judge.py` — Judge 回傳 `JudgeVerdict` 實例(用 `MockProvider` 腳本化)
- `test_explorer_loop.py` — ReAct 迴圈 end-to-end(MockProvider 腳本化 `ExplorerAction` + `JudgeVerdict`),驗證:
  1. 每步都呼 `_think`、`_execute_tools`(空 tool list)、`judge.evaluate`、`logger.write`
  2. `reasoning_log.jsonl` 每行對應一個 `Step`
  3. `_should_stop` 在 budget 用盡時觸發
  4. cancel `asyncio.Event` 能中斷
- `test_protocols.py` — Protocol duck-typing 檢查(Mock 類別結構性滿足 `ExplorerTools` / `Judge` / `CoverageChecker`)

**Prompt 的 M2 承諾(本 change 明文)**:Prompt 常數走 git、改動進 PR 有 diff 可 review;`EXPLORER_PROMPT_VERSION` 進 `reasoning_log.jsonl` 每行以方便 golden sample 對齊(`docs/agent-core.md §八`)。

## Non-Goals

明確排除(避免 P0 蔓延;各自是未來獨立 change):

- **真實工具實作**(`search` / `list_dir` / `read_file` / `mark_station` / `trace_import` / `find_callers`)—— 本 change 只定義 `ExplorerTools` Protocol。實作屬 `explorer-tools-p0`(步驟 17 + 19;解鎖 Explorer 跑真 codebase)。`test_explorer_loop.py` 用 `MockTools` 滿足 Protocol(回固定 ToolResult),驗 ReAct 訊號流通即可。
- **Coverage Checker 實作 + gap 補查遞迴** —— 屬步驟 20 的 `coverage-gap-recurse` change。本 change 只定義 `CoverageChecker` Protocol 與 `CoverageResult` / `Gap` schema;`run_explorer` 呼叫 `coverage.check` 的位置 **先註解掉**,等 Coverage change 落地再打開。
- **Context 壓縮 + Budget 控制** —— 屬步驟 21。本 change `ExplorerState.budget_*` 只做簡單遞減(每輪扣 1 step),不做 token-aware 壓縮、也不做 context-window 75% 觸發點。Budget 預設值(步數 / tokens / wall-seconds)寫死常數。
- **SSE emit**(`agent_thought` / `judge_verdict` / `action_result` / `usage_delta`)—— 屬步驟 22 的 `agent-sse-wiring` change。本 change `ReasoningLogger` 只寫檔,不 inject SSEEmitter。
- **HTTP endpoint** —— 本 change 不在 `api/` 新增 `POST /explore` 端點。Explorer 入口只對 Python 層公開(後續 change 接 endpoint + task registry 整合)。測試直接呼 `await run_explorer(...)`。
- **Prompt engineering 調優 / golden sample 首跑** —— 屬步驟 23。本 change 的 `EXPLORER_SYSTEM` / `JUDGE_SYSTEM` 用能跑通的最小版本(能產生有效 `ExplorerAction` / `JudgeVerdict` 就好),不保證 relevance 品質。Golden sample 對齊留給 `explorer-golden-sample-p0` change。
- **Q&A Agent ReAct loop 重用** —— `docs/agent-explorer-spec.md §十二`明文 Explorer 與 Q&A 共用 ReAct core。本 change **確保 protocol 抽象日 1 就做對**(`QATools` 未來可 plug in),但不實作 Q&A 端。
- **Explorer 與 Generator 的交接**(Station → tutorial.md)—— Module 5 範疇(步驟 24)。
- **`TrackedProvider` 改動** —— 不碰;直接用 `chat-provider-wiring` 提供的 `app.state.llm_reasoning_provider(ws)` / `llm_judge_provider(ws)` factory。Explorer 從 caller 拿到已建好的 `TrackedProvider`。

**拒絕的設計**:

- **LangChain / LangGraph 再評估** —— D-012 已定「自寫 ReAct + Instructor 輔助」,不回頭。
- **把 Judge 也塞進 ReAct 迴圈內部當作一個 tool call** —— `docs/agent-core.md §七`明文「Judge 每步都跑,但它自己不是 ReAct」,邏輯分層乾淨。
- **`reasoning_log.jsonl` 用 ORM / DB 取代 JSONL** —— 七層 Audit JSONL 不變式;保持 append-only 純文字。
- **`ExplorerAction` 內嵌 tool call 用 native OpenAI `tools` field** —— D-012 明文「自寫 ReAct,工具 dispatch 由 Agent 層處理,provider 只回 `response_model`」,不碰 native tool_calls。

## Capabilities

### New Capabilities

- `agent-core`:Explorer ReAct 主迴圈 + types(`Step` / `ExplorerState` / `JudgeVerdict` / `ExplorerAction`)+ `Judge` / `ExplorerTools` / `CoverageChecker` Protocol + `reasoning_log.jsonl` writer。支撐 Module 4 Explorer 與未來 Module 8 Q&A Agent 共用的 ReAct 核心(`docs/agent-explorer-spec.md §十二`)。

### Modified Capabilities

(無 —— 本 change 不動既有 capability。既有 llm-provider 的 chat(messages, response_model) 契約已在 chat-provider-wiring 內定義完整;既有 usage-tracking 的 token_usage.jsonl 由 TrackedProvider 自動寫入,Explorer 不另外 record。)

## Impact

**受影響 spec**:

- `openspec/specs/agent-core/spec.md`(新增 capability)—— requirements 含:
  - `ReAct loop executes think-act-observe-judge-log-update each iteration`(end-to-end 行為)
  - `ExplorerAction is validated by Instructor through chat() response_model`(schema 契約)
  - `JudgeVerdict is produced by Judge Protocol impl via llm_judge_provider`
  - `ReasoningLogger writes each Step as one JSONL line with prompt versions`
  - `ExplorerTools / Judge / CoverageChecker are structural Protocols(day-1 abstraction)`
  - `_should_stop enforces budget / queue / cancel three-way convergence`

**受影響 code**:

- `sidecar/src/codebus_agent/agent/__init__.py`(新檔)
- `sidecar/src/codebus_agent/agent/types.py`(新檔)
- `sidecar/src/codebus_agent/agent/protocols.py`(新檔)
- `sidecar/src/codebus_agent/agent/explorer.py`(新檔;主迴圈)
- `sidecar/src/codebus_agent/agent/judge.py`(新檔;`LLMJudge` impl)
- `sidecar/src/codebus_agent/agent/reasoning_logger.py`(新檔)
- `sidecar/src/codebus_agent/agent/prompts/__init__.py`(新檔)
- `sidecar/src/codebus_agent/agent/prompts/explorer.py`(新檔)
- `sidecar/src/codebus_agent/agent/prompts/judge.py`(新檔)

**受影響測試**:

- `sidecar/tests/agent/__init__.py`(新檔)
- `sidecar/tests/agent/test_types.py`(新檔)
- `sidecar/tests/agent/test_protocols.py`(新檔)
- `sidecar/tests/agent/test_reasoning_logger.py`(新檔)
- `sidecar/tests/agent/test_judge.py`(新檔)
- `sidecar/tests/agent/test_explorer_loop.py`(新檔;end-to-end with MockProvider + MockTools)

**受影響文件**:

- `docs/agent-core.md` — 若實作過程有偏離 §三 / §四 骨架的地方,回頭同步(例如欄位命名 / 模組切分定案)
- `CLAUDE.md` §Repo 現況「sidecar」條:archive 時間軸加入本 change;下一步指向 `explorer-tools-p0`(步驟 17)

**無新依賴 / 無新 env var**(`instructor` / `openai` 已在 `pyproject.toml`)。
