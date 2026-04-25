## Why

`coverage-gap-recurse`（2026-04-26 archive）補齊 Module 4 Explorer 的收斂閉環後，下一塊靈魂缺口是「**長跑不炸**」—— `docs/agent-core.md` §十 Context 壓縮 與 §十一 Budget 控制 在 spec 裡寫死多時，但兩塊都還沒落地：

- **`state.messages` 無上限累積**：Explorer loop 每輪把 N 個 ToolResult 以 `role="tool"` 塞進 `state.messages`（`_append_observations`），下一輪 `_think` 以 `_to_provider_messages(state.messages)` 全量送進 provider。20 輪後就是 20 + 份 tool observation 每輪都上網——Timeline 這種大 repo 跑到第 15 步就會撞 context window。
- **`budget_tokens_left` 是死欄位**：`ExplorerState` 一開始就有這欄（M1 埋的），但 `_should_stop` 只看 `budget_steps_left`。token 用爆的會 provider 層拋 `OPENAI_CONTEXT_EXCEEDED`，變成 SSE `error` event 炸給前端——不是 partial-stations 收斂。
- **評審 Demo 風險**：Trust Layer R-01 / O-04 示範要跑 8–15 步起跳；context 爆或 token 爆都會讓 Agent console 顯示「error」而不是「budget_tokens_exhausted」的誠實收斂敘事，直接破壞「像工程師一樣探索」的 narrative。

對齊 **D-012**（自寫 ReAct loop + budget/compression 是核心層責任）、**D-021**（`token_usage.jsonl` 是 UsageTracker 唯一寫入路徑；本 change 從 `TrackedProvider` 記憶體總量讀，不再開第二條）、**D-022**（`llm_calls.jsonl` wire payload 不變）、`docs/implementation-plan.md` **步驟 21**（Context 壓縮 + Budget 控制，1d，依賴步驟 16）、`docs/agent-core.md` §十 / §十一。

本 change 是 step 21 最小可用版：只做「rolling window + token budget enforcement」，把 wall-clock timeout / tool-output 壓縮 / state snapshot injection 這些週邊策略留給後續獨立 change。

## What Changes

**A. `TrackedProvider` 暴露 session token 累計** —— 既有 `session_total_cost_usd` 的對稱擴充：

- 新 `session_prompt_tokens: int` / `session_completion_tokens: int` 兩個記憶體 running counter，每次 `chat` / `embed` 成功後由既有 `_emit_usage_delta` 路徑同步累計（失敗不累計，跟 cost 對齊）。
- 新 property `session_total_tokens: int`（= prompt + completion）供 Explorer 讀取。
- **不**改動 `token_usage.jsonl` / `llm_calls.jsonl` wire schema（D-021 / D-022 不變），只是新增 in-memory 讀取面；`usage_delta` SSE event 多 `session_total_tokens` 欄位（嚴格 additive，不改既有 key）。

**B. `TokenBudgetProbe` Protocol + 多 provider 聚合** —— 跨 reasoning / judge / coverage 三個 provider 的加總鉤：

- 新 `codebus_agent.agent.budget.TokenBudgetProbe`（`@runtime_checkable` Protocol，單方法 `total() -> int`）。
- 新具體 impl `AggregatedTokenProbe(providers: list[TrackedProvider])`：`total()` 回 `sum(p.session_total_tokens for p in self._providers)`。
- 新 `LLMJudge.provider` / `LLMCoverageChecker.provider` 唯讀 property，暴露內部 `TrackedProvider` 給 endpoint 組 probe 用；不改 evaluate / check API。

**C. `run_explorer` 執行 token budget** —— `_should_stop` 第四分支：

- `run_explorer(..., token_probe: TokenBudgetProbe | None = None)`：新 keyword-only，default None 保持既有測試相容；None 時視同 token budget 永不耗盡。
- `_should_stop` 新分支 `"budget_tokens_exhausted"`（第四種 convergence，precedence: cancel > budget_tokens > budget_steps > queue_empty；token 在 steps 之前查，因為 token 用光前 steps 可能還有 5 格）：當 `token_probe is not None AND token_probe.total() >= state.budget_tokens_left` → 收斂。
- `ExplorerResult.stopped_reason` Literal 擴充到四值：`"budget_exhausted" | "queue_empty" | "cancelled" | "budget_tokens_exhausted"`（**additive**，不改既有 Literal 值；四值同為 str，既有 match-case 預設分支仍舊 fall-through）。
- 遞迴 propagation 沿用既有 tail-recursion 原則（innermost stopped_reason 原樣回傳 — `coverage-gap-recurse` 已落地）。

**D. `_think` 套 rolling window** —— 降低 wire prompt 長度：

- 新常數 `_MESSAGE_ROLLING_WINDOW: int = 16`（可用單次 Explorer run 內 N 輪 tool observation 做滑動窗；16 代表 `[hint... last 16 messages]`）。
- `_think` 在 `_to_provider_messages(state.messages)` 之前先 `state.messages[-_MESSAGE_ROLLING_WINDOW:]` slice：只送最近 16 條進 provider，歷史仍完整保留在 `state.messages`（`reasoning_log.jsonl` 不變）。
- 注意 **只窗化 provider wire prompt**，不窗化 state。`state.visited_files` / `state.stations` 仍是無損累積器；下一輪 `_think` 透過 `render_explorer_prompt(state, tool_specs)` 拿 state summary 補回上下文感。
- **不**做 summary compression（不把 dropped messages 摘要回去塞 system prompt）——MVP 走最簡單的 FIFO slice；未來若 Judge 需要完整歷史再開新 change 做摘要。

**E. SSE `budget_warning` event（explorer-sse）** ——

- 新常數 `_BUDGET_WARNING_PCT: float = 0.8`；新 `_BudgetWarningState` 小類，per-run 記 `warned_tokens: bool` / `warned_steps: bool` 避免重複 emit（一次 run 內每種 budget 最多 emit 一次）。
- 觸發條件：
  - `token_probe.total() / state.budget_tokens_left >= 0.8` 且未 warned → emit `{"type": "budget_warning", "kind": "tokens", "current": int, "budget": int, "pct": float}`
  - `(initial_budget_steps - state.budget_steps_left) / initial_budget_steps >= 0.8` 且未 warned → emit `{"type": "budget_warning", "kind": "steps", ...}`
- 觸發點：每輪 Update 後、progress emit 之前（一個清楚的 per-iter checkpoint）。
- 無 `emitter` / `token_probe` 為 None 時不 emit（file-only / in-process 測試相容）。

**F. HTTP 層接線（`api/explore.py`）** ——

- `explore_endpoint` 建好 `reasoning_provider` / `judge` / `coverage` 三元組後，新組 `AggregatedTokenProbe([reasoning_provider, judge.provider, coverage.provider])`。
- 餵給 `run_explorer(..., token_probe=aggregator)`。
- 既有 `POST /explore` request schema 不動（`budget_tokens` 欄早在 ExploreRequest 裡，只是沒被用）。

**G. 文件同步** ——

- `CLAUDE.md` archive 時間軸加入本 change + 「下一步」改指向 **步驟 22 `SSE emit` 後續**（Explorer → 前端 Agent console 消費，雖然 SSE emit 早在 agent-sse-wiring 已落地、步驟 22 主要是驗前端接通）。
- `docs/agent-core.md §十` / §十一 兩段從「策略描述」改「✅ landed 形狀」，連帶 `stopped_reason` 表改四值。
- `docs/implementation-plan.md` 步驟 21 狀態 `⏳` → `✅ landed（context-compression-token-budget）`。

## Non-Goals

- **Wall-clock timeout（`max_wall_seconds=600` 硬上限）**：§十一 列了 10 分鐘上限，本 change 不做；D-007 cost benchmark 還沒跑出實際 Explorer 耗時，過早寫死反而把正常 run 卡死。留給 `wall-clock-budget` 後續 change 帶 benchmark 一起落。
- **Summary compression**（dropped messages 摘要成 system hint）：Rolling window 單純 FIFO slice 已足以解 context 爆炸；若 Judge 在長 run 後誤判，再開新 change 做摘要。過早壓縮是 guesswork。
- **Tool output truncation 二度加工**：既有 `_truncate_observation` 對 SSE `agent_action_result.observation` 截 500 字、`render_judge_prompt` 對每條 tool output 截 800 字、`render_explorer_prompt` 對 state summary 窗口 20 entries——三段 truncation 已覆蓋 provider wire 實況。本 change 只加 messages window，不動既有 truncation 常數。
- **State snapshot injection 覆寫**：§十 原本寫「visited / stations / pending_queue[:5] 寫 system prompt」——目前 `render_explorer_prompt` 已經把這些 fold 進 user prompt（每輪 re-render）。所以 system prompt 保持穩定、user prompt 裝 state summary 是比 spec 更乾淨的分層，本 change 不回頭改 system prompt。
- **`budget_tokens_left` 成為唯一 budget**：本 change 把 token budget 變成 steps 之外的**第二**收斂條件（additive），不取代 `budget_steps_left`。兩個都在，先到先 stop。
- **改 `token_usage.jsonl` / `llm_calls.jsonl` schema**：D-021 / D-022 既有 record schema 不動，只擴 TrackedProvider in-memory 讀取面與 `usage_delta` SSE event（後者嚴格 additive）。
- **前端 `budget_warning` UI**：event 格式本 change 落定；Nuxt UI 顯示延到 Module 7 實作期（Agent console）。

**拒絕的設計**：

- **「用 UsageTracker 讀 `token_usage.jsonl` 反推 session 總量」**：每輪跑 re-parse 一份 append-only JSONL 非常浪費；`TrackedProvider` 本來就在記憶體累 cost（為了 `usage_delta` 的 `session_total_cost_usd`），token 多累兩個整數是 zero cost。
- **「把 `budget_tokens_left` 直接改成每輪遞減 token 估計值」**：既有 budget_steps_left 的遞減模式是「每輪扣 1」可預期；token 用量每輪差異巨大（單一次 chat 1k–50k tokens 都可能），把 `budget_tokens_left` 當成 field 每輪扣既不準又會讓 state.model_dump() 亂跳。改「比對 `token_probe.total() >= state.budget_tokens_left`」是精確 snapshot 比對。
- **「rolling window 用 token count 門檻（如 >4096 tokens 才 slice）」**：每輪要估 token 數浪費 CPU；N=16 固定窗口夠保守，若 reasoning prompt 要更大窗口（例如 Q&A Module 8 需要完整歷史），該換的是 `_MESSAGE_ROLLING_WINDOW` 常數本身，不是加 token-aware 邏輯。
- **「Judge / Coverage 也套 rolling window」**：Judge / Coverage 都是 one-shot（一次 call，無歷史 messages），`render_judge_prompt` / `render_coverage_prompt` 已經把 state windows 在 prompt 內做了。rolling window 只對「跨輪累積 messages 的 ReAct loop 本尊」有意義。

## Capabilities

### New Capabilities

（無 —— 所有改動掛在既有 `agent-core` / `explorer-sse` / `llm-provider` capability 上）

### Modified Capabilities

- `agent-core`：
  - MODIFIED Requirement `Explorer loop stops on budget exhaustion, empty queue, or cancel signal`（擴 `stopped_reason` Literal 到四值，加 scenario `Token budget exhaustion terminates loop`）。
  - ADDED Requirement `Explorer applies rolling message window before each Think call`（視窗大小 = 16；window 只作用於 provider wire prompt，`state.messages` 不變）。
- `explorer-sse`：
  - ADDED Requirement `Explorer emits budget_warning SSE event at 80% threshold`（per-kind 最多 emit 一次的 wire schema + 觸發時機）。
  - MODIFIED Requirement `TrackedProvider emits usage_delta on every completed call`（event 新增 `session_total_tokens` 欄位，嚴格 additive）。
- `usage-tracking`：
  - ADDED Requirement `TrackedProvider exposes session token counters`（記憶體 counter `session_prompt_tokens` / `session_completion_tokens` / `session_total_tokens`；`chat` / `embed` 成功 path 累計、失敗 path 不累計）。

## Impact

**受影響 spec**：

- `openspec/specs/agent-core/spec.md`（MODIFIED — 1 Requirement 擴 Literal + scenario；1 Requirement 新增 rolling window）
- `openspec/specs/explorer-sse/spec.md`（MODIFIED — 1 Requirement 新增 budget_warning event）
- `openspec/specs/usage-tracking/spec.md`（MODIFIED — 1 ADDED Requirement 載明 TrackedProvider 記憶體 session token counters）

**受影響 code**：

- `sidecar/src/codebus_agent/providers/tracked.py`（加 `session_prompt_tokens` / `session_completion_tokens` / `session_total_tokens` + `_emit_usage_delta` 路徑累計）
- `sidecar/src/codebus_agent/agent/budget.py`（**新檔** —— `TokenBudgetProbe` Protocol + `AggregatedTokenProbe` 實作）
- `sidecar/src/codebus_agent/agent/explorer.py`（`run_explorer` 加 `token_probe`、`_should_stop` 加 `budget_tokens_exhausted` 分支、`_think` 加 rolling window、新 `_BUDGET_WARNING_PCT` / `_MESSAGE_ROLLING_WINDOW` 常數、emit `budget_warning`）
- `sidecar/src/codebus_agent/agent/types.py`（`ExplorerResult.stopped_reason` Literal 擴四值）
- `sidecar/src/codebus_agent/agent/judge.py`（加 `provider` property）
- `sidecar/src/codebus_agent/agent/coverage.py`（加 `provider` property）
- `sidecar/src/codebus_agent/agent/__init__.py`（re-export `AggregatedTokenProbe` / `TokenBudgetProbe`）
- `sidecar/src/codebus_agent/api/explore.py`（組 `AggregatedTokenProbe` 並餵給 `run_explorer`）

**受影響測試**：

- 新：`sidecar/tests/agent/test_budget_probe.py`（`TokenBudgetProbe` Protocol shape + `AggregatedTokenProbe` 聚合）
- 新：`sidecar/tests/agent/test_token_budget_enforcement.py`（Explorer token 耗盡收斂 + `stopped_reason="budget_tokens_exhausted"` + innermost propagation 穿 coverage 遞迴）
- 新：`sidecar/tests/agent/test_message_rolling_window.py`（`_think` 只送 last 16；`state.messages` 不被改）
- 新：`sidecar/tests/agent/test_budget_warning_event.py`（80% 觸發、kind=tokens / steps 分流、per-run 最多一次）
- 更新：`sidecar/tests/providers/test_tracked_provider_usage.py`（session token counters getter 合約）
- 更新：`sidecar/tests/providers/test_tracked_provider_sse.py`（`usage_delta` 帶 `session_total_tokens` 欄位）
- 更新：`sidecar/tests/api/test_explore_endpoint.py`（happy-path 驗 token_probe 有被 wire）
- 更新：`sidecar/tests/api/test_explore_sse_integration.py`（event 集合加 `budget_warning`）

**受影響文件**：

- `CLAUDE.md`（archive 時間軸 + 「下一步」導向步驟 22）
- `docs/agent-core.md §十` / §十一（landed 形狀）、§四 `stopped_reason` 四值、§十二 SSE event 清單加 `budget_warning`
- `docs/implementation-plan.md`（步驟 21 狀態 → ✅ landed）

**無新依賴**（Pydantic / Protocol / TrackedProvider / SSE emitter 皆既有）。

**無 breaking change**（`ExplorerResult.stopped_reason` Literal 擴 additive；`run_explorer` 新參數 `token_probe` 有 default `None`；`usage_delta` SSE event 新欄位 additive）。
