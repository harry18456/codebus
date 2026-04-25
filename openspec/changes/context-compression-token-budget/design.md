## Context

`coverage-gap-recurse`（2026-04-26 archive）把 Module 4 Explorer 的收斂閉環補齊，但 `docs/agent-core.md` §十 Context 壓縮 與 §十一 Budget 控制 兩段 spec 到這個 change 為止還沒落地。目前實況：

```python
# sidecar/src/codebus_agent/agent/explorer.py（現況）
def _should_stop(state, cancel_event):
    if cancel_event is not None and cancel_event.is_set():
        return True, "cancelled"
    if state.budget_steps_left <= 0:
        return True, "budget_exhausted"
    if not state.pending_queue and len(state.stations) >= _MIN_STATIONS_FOR_CONVERGENCE:
        return True, "queue_empty"
    return False, None  # budget_tokens_left 從來沒被看過

async def _think(state, provider, tool_specs):
    messages = _to_provider_messages(state.messages) + [...]  # state.messages 全量送
    action = await provider.chat(messages, response_model=ExplorerAction)
```

三個事實觸發本 change：

1. **`state.budget_tokens_left` 是死欄位**：M1 的 ExplorerState schema 早就埋好，但 `_should_stop` 從來沒查過它。token 用爆是 provider 層 `OPENAI_CONTEXT_EXCEEDED` 噴 `_run_background_task` 收殘的 SSE `error` event，不是 partial-stations 收斂。
2. **`state.messages` 單向成長**：每輪 `_append_observations` push N 條 `role="tool"`，第 15 輪 provider 每次都看 15×N 份 observation。Timeline 這種中型 repo 會在 20 輪左右撞 context window。
3. **`docs/agent-core.md` §十一 明定** tokens 從 `UsageTracker.session_total()` 即時讀取，Explorer 不內部估算。但 UsageTracker 只會 append JSONL、不暴露 session total 讀取 API；要嘛回去讀檔（慢 + race condition），要嘛在 TrackedProvider 記憶體累（現成路徑）。

約束：

- **TrackedProvider 唯一 outbound 路徑**（invariant #4）：所有 chat/embed 都經 TrackedProvider，它已經在記憶體累 `session_total_cost_usd`（給 `usage_delta` SSE 用）。token 累計是 cost 累計的直接鄰居。
- **D-021**：`token_usage.jsonl` 只由 TrackedProvider 寫（append-only、不改既有 schema）。session-total 讀取用記憶體，不去 parse 那個檔。
- **D-022**：`llm_calls.jsonl` 記 wire payload（post-Sanitizer Pass 2）；本 change 不動這塊。
- **additive discipline**：`ExplorerResult.stopped_reason` Literal 擴展 additive（新值加在既有值後）；`usage_delta` SSE 欄位嚴格 additive；既有測試 / golden fixture 不破壞。
- **單模 provider 暫時合宜**：當前 chat-ish provider allowlist 是 `{MockProvider, OpenAIChatProvider}`（+ `OpenAIEmbeddingProvider`）；未來加 Ollama / Anthropic 時兩個 counter 跟 cost 一樣走同一 hook。
- **golden baseline 不動 stopped_reason**：`tests/golden/demo-synthetic/expected.json` 目前值是 `"budget_exhausted"`；本 change 不改 fixture 讓 golden replay 繼續跑，token budget 設大到 replay 不會觸發新 branch。

## Goals / Non-Goals

**Goals:**

- `state.budget_tokens_left` 從「死欄位」變成 `_should_stop` 的第四分支收斂貨幣。
- `state.messages` 跨輪成長不再把 provider prompt 撐爆；Explorer 每輪 wire prompt 被窗化到最近 16 條 tool / assistant observation。
- Token 總量是跨 reasoning / judge / coverage 三 provider 的**合計**，不是單一 provider 計算（三者共吃同一個 `budget_tokens_left`）。
- SSE 80% 預警事件讓前端能顯示「budget 快用完」敘事，同 kind 每 run 最多一次避免刷屏。
- 既有 13 個 Explorer 舊測 + 20 個 coverage-gap-recurse 測 + 7 個 explorer-sse 測 + golden replay 全綠（additive 手術）。

**Non-Goals:**

見 proposal Non-Goals 段（wall-clock timeout / summary compression / 二度 tool-output truncation / state snapshot 覆寫 / budget_tokens_left 成為唯一 budget / schema 變動 / 前端 UI）。

- 本 change **不**暴露 `token_probe` 到 `POST /explore` request schema：budget 門檻由 caller（HTTP layer）依 `app.state` DI 組好，handler 透明地組 `AggregatedTokenProbe` 塞 `run_explorer`。
- 本 change **不**做 SSE event `budget_warning` 的前端消費；event 格式落定、Module 7 才接 UI。
- 本 change **不**動 `_MIN_STATIONS_FOR_CONVERGENCE`（仍然 3）、**不**動 `_COVERAGE_MAX_DEPTH`（仍然 3）。
- 本 change **不**把 token 預算寫進 Judge / Coverage 自己的一次 call 裡（它們是 one-shot，單次 over-budget 就讓 provider 拋；本 change 只卡 Explorer main loop 的下一輪 Think）。

## Decisions

### Decision 1：TrackedProvider 加 session token counters（對稱於 cost）

選 TrackedProvider 記憶體累計 prompt / completion tokens，不從 `token_usage.jsonl` 讀。

理由：

- **記憶體對稱 cost**：既有 `_session_total_cost_usd` 在 `_emit_usage_delta` 成功 path 累計；token 放同一路徑，failure path 同樣不累（D-022 scenario `usage_delta on success only` 一脈相承）。
- **zero-cost 讀取**：Explorer 每輪 `_should_stop` 查一次 `sum(p.session_total_tokens for p in probes)`——三個整數加法，比 `Path.open() → for line in f: json.loads(...)` 反推一份 append-only JSONL 快幾個數量級。
- **Race-free**：單 asyncio loop 裡順序執行，不用鎖；未來若跨 task 共享 provider，記憶體 counter 在 TrackedProvider 實例 scope，天然隔離（per-workspace-per-task）。

**替代方案 A**：`UsageTracker.session_total()` 讀 JSONL。棄用——每輪 parse 一整份 append-only 檔，浪費 IO；且 `token_usage.jsonl` 在 workspace-root 但 Explorer 也 append `reasoning_log.jsonl` 到同處，某些邊界條件下要考慮 flush 時序，徒增複雜度。

**替代方案 B**：新 `SessionTotalTokens` shared counter 物件，構造時塞進 TrackedProvider。棄用——多一個層抽象，但 cost 已經是 per-provider 記憶體累計，token 走同路徑才一致；要 shared counter 哪天再說（例如 Q&A Module 8 會想要 cross-task session total，那時再開新 change 引入）。

### Decision 2：TokenBudgetProbe Protocol + AggregatedTokenProbe 具體 impl

選 `@runtime_checkable` Protocol `total() -> int` + 一個 `AggregatedTokenProbe(providers: list[TrackedProvider])` 實作。

理由：

- **對稱現有抽象**：Explorer 已用 `ExplorerTools` / `Judge` / `CoverageChecker` / `SSEEmitter` 四個 Protocol 做邊界；新增 `TokenBudgetProbe` 延續同風格。Protocol 讓測試寫 `_ScriptedProbe(total=1000)` 之類 inline spy，不需 instantiate 整套 TrackedProvider。
- **Aggregation 責任放 endpoint 層**：`run_explorer` 不該知道 reasoning / judge / coverage 三 provider 各自是誰；它只看 `token_probe.total()`。
- **默認 `None` 向後相容**：`run_explorer(..., token_probe=None)` → 永不觸發 token 收斂，既有 in-process 測試 / golden replay 一行都不改。

**替代方案 A**：`run_explorer(..., providers: list[TrackedProvider])`。棄用——把 provider 具體型別綁進 loop，既破壞「loop 只認 Protocol」的 §四 設計紀律，也讓測試必須餵真 TrackedProvider。

**替代方案 B**：`Callable[[], int]` 當 probe。功能上等價於單方法 Protocol 但少自己文件化 + 少 runtime_checkable。選 Protocol 是為了 doc + `isinstance` 診斷。

### Decision 3：`_should_stop` 分支優先序：cancel > budget_tokens > budget_steps > queue_empty

選在既有順序「cancel → budget_steps → queue_empty」前插入 `budget_tokens`，放在 `budget_steps` **之前**。

理由：

- **usability**：token 用完比 step 用完更「硬」——step 用完可能 agent 只跑幾步就卡（例如預設 budget_steps=10 太保守），但 token 用完代表 provider 真的沒法再回；兩個同時都超時也要以 token 為收斂 reason（幫前端 debug 更準）。
- **穩定性**：cancel 永遠第一優先不變；queue_empty 依賴 `_MIN_STATIONS_FOR_CONVERGENCE`，屬於「正面收斂」，擺最後最保險。
- **可讀性**：`_should_stop` 實作順序就是程式碼讀起來的 precedence，不用額外註解。

**替代方案**：`budget_steps > budget_tokens`（現有順序之後插）。功能上差別只在**同時耗盡**時回哪個 reason；本 change 選 tokens 優先是因為它是更少見（需要真的跑 OpenAI）的 branch，優先出它有診斷價值。

### Decision 4：`_MESSAGE_ROLLING_WINDOW = 16` 固定窗口，不 token-aware

選硬 code window size 16 條 messages，不動態按 token count 切。

理由：

- **`render_explorer_prompt` 已在 user prompt 塞 state summary**（`visited_files` 前 20 條、`stations` / `pending_queue` 摘要）——即使丟掉舊 tool messages，Explorer 還是能從 user prompt 看到「之前去過哪」。
- **Token 計數是估算遊戲**：Instructor wraps chat；精確 token 數只能向 OpenAI API 問 pre-flight，打 API 前算 token 反而要跑一次 `tiktoken` encode（又多一個依賴）。固定 16 條裡每條是 `_truncate_observation` 截 500 字的 tool output，上限 16 × 500 = 8000 字 ≈ 2000 tokens，遠低於 128k context window。
- **可調**：如果未來 Q&A Module 8 或更長探索需要更大窗口，改一個常數比改 token-count 邏輯容易。

**替代方案**：`summary compress`（把 dropped messages 摘成 system hint）。棄用——MVP 先做 FIFO slice，等實際跑長 run 再看要不要補摘要；Non-Goals 已列。

### Decision 5：Rolling window 只影響 wire prompt，不動 state.messages

`state.messages` 仍然無損累積（作為 reasoning 審計），rolling window 只在 `_think` 的 `_to_provider_messages(state.messages)` 上切 `[-16:]`。

理由：

- **state 仍是真相來源**：`reasoning_log.jsonl` 每行 Step 含 `tool_results`（由 `_update_state` / logger 寫）；state.messages 是「發生過什麼」的記錄，不應該被「送給 LLM 的 prompt 策略」覆蓋。
- **Observable**：測試可以比對「state.messages 長度 = iter 次數 × tool_count」來驗 loop 邏輯；若 state 本身被切，邏輯測試就糊了。
- **Compression 是 view 層責任**：類比 DB 正規化——原始事實獨立存，view（=prompt）可以任意抽。

### Decision 6：`_BUDGET_WARNING_PCT = 0.8` + per-kind once semantic

80% 寫死；用一個小 mutable 狀態（`_BudgetWarningState(warned_tokens=bool, warned_steps=bool)`）避免刷屏。

理由：

- **80% 符合業界通用**：慢性常識值；若 Demo 跑出來覺得 70% 更好，一個常數改掉。
- **per-kind 只 emit 一次**：token 與 steps 是兩個獨立貨幣，各 emit 一次總量上限 2；避免「每輪都觸發 80%」對前端灌 SSE。
- **per-run scope**：mutable state 是 function-local，每 `run_explorer` call 一份；遞迴（coverage round）也是同一個 mutable，因為遞迴重用同 state——如果外層已 warned_tokens=True，遞迴也不會再 emit。這是正確行為（用戶看到一次 warning 就夠）。

**替代方案 A**：每輪都 emit `budget_update` event（連續進度）。棄用——太吵；`usage_delta` / `progress` 每輪都 emit 已經提供進度，warning 應該是「檻觸發 → 提醒」semantic。

**替代方案 B**：per-warning state 用 `list[str]` 存 kind。功能一樣；選 `@dataclass` 是為了 field 嚴格化（`warned_tokens: bool` / `warned_steps: bool`）不會被誤塞別值。

### Decision 7：Judge / Coverage 暴露 `provider` property，不暴露整個 `_provider`

選加兩個 read-only property，不解 underscore 前綴。

理由：

- **抽象洩漏最小化**：Endpoint 需要 aggregator input，唯一合理接口是「給我你那顆 TrackedProvider」；暴露 property 比暴露整個 Python convention underscore 名稱乾淨。
- **type-safe**：`@property def provider(self) -> TrackedProvider` 有靜態型別；`self._provider` 是 conventional hidden，mypy 會 warning。
- **不擴大 API surface**：Judge / Coverage 仍只有 evaluate / check / set_emitter 三個公開方法；`provider` 是唯讀 getter，相對輕量。

### Decision 8：`budget_warning` SSE event 觸發點—— Update 之後、progress 之前

在既有 `progress` emit 的**前一行**插 warning check。

理由：

- **時序正確**：這一輪的 token consumption 已經發生（Think 呼 chat）、state.budget_steps_left 已扣 1，此時查 probe 得到本輪後的真實 token 總量與剩餘 step。
- **清楚的 checkpoint**：`progress` 已經是每輪的「tick 點」，warning 緊貼它最符合直覺。
- **不影響 emit 順序測試**：既有 `test_explorer_loop_sse.py` 期待的 `agent_thought → agent_action_result* → judge_verdict → progress` 順序不變，本 change 在 progress 前多一個 optional emit（既有測試不斷言「progress 前沒東西」所以不破壞）。

## Risks / Trade-offs

- **[rolling window 丟掉的 observation 可能對 Judge 有用]** → Judge 每輪只看 `results`（當輪 ToolResult），不直接看 `state.messages`，所以 Judge 側不受影響。Think 側 LLM 本來就只需要「近期任務動態」—— state summary（visited / stations）已在 user prompt 每輪 re-render 回補上下文。
- **[token 總量與實際 OpenAI billing 有誤差]** → `TrackedProvider._estimate_tokens(text)` 是「每 4 字元 ≈ 1 token」粗估（D-021 允許 `estimated=True`）。cost benchmark 後可換 `tiktoken` 精算；目前估法傾向「估得比實際多」，保守 stops earlier，反而是 safer 方向。
- **[aggregator 抓不到 coverage provider 時 silent under-count]** → Explorer endpoint 在三者 factory 都 wire 完後才組 aggregator，任何一個缺就已經在 `_require_explore_deps` 擋 503；進 `run_explorer` 一定三 provider 齊全。保險：aggregator `__init__` 對空 list 留 `assert len(providers) >= 1` 避免誤傳。
- **[token_probe=None 時的 budget 語意漂移]** → 既有測試都不傳 probe，所以 `budget_tokens_left` 永不耗盡——`run_explorer` 行為與目前一致。endpoint 真跑時一定傳 probe，生產 path 被覆蓋。
- **[SSE `budget_warning` 前端未消費]** → 同 `coverage_gaps` 策略：事件格式先落定，Module 7 前端上線時再接。既有前端未實作，event 不會讓任何東西壞掉。
- **[rolling window 把 tool observation 丟太快讓 Agent 卡在 loop]** → N=16 對 budget_steps 預設 10–50 的範圍足夠；如果真看到 agent 卡住，`_MESSAGE_ROLLING_WINDOW` 是常數一行改。
- **[`session_total_tokens` 累積在 TrackedProvider 實例 scope，跨 task 不共用]** → 這是 feature 不是 bug：每 task 一個新 provider 實例（`_factory(workspace_root)` 每次 re-construct），token 預算自動 per-task 獨立；若未來 Q&A Module 8 要跨 task 累積，重組 aggregator 就可。
- **[stopped_reason Literal 擴四值可能破 golden fixture]** → fixture `expected.json` 目前 `stopped_reason="budget_exhausted"`；golden replay 給的 budget_tokens 設足夠大（MockProvider 每次 `_estimate_tokens("... pinned ExplorerAction json")` 小 << 10_000）所以 token branch 永不觸發。Drift guard 在 `tests/golden/test_explorer_replay.py` 會驗精確字串等於 `"budget_exhausted"`——**如果**未來真要改 fixture 值，要 re-baseline，本 change 不做。

## Migration Plan

- 無 schema 破壞 —— `ExplorerState.budget_tokens_left` 本來就在；`ExplorerResult.stopped_reason` Literal 擴值嚴格 additive。
- 無 HTTP API 破壞 —— `POST /explore` request/response 不動。
- 既有單測的 `run_explorer(..., token_probe=None)` default 保持 legacy（不觸發 token 收斂）；新測專注驗 probe 非 None 時的新分支。
- Golden baseline 不重開：`expected.json` 的 `stopped_reason="budget_exhausted"` 不改，由 golden replay path 不餵 `token_probe` 保證。
- `test_explore_sse_integration.py` 需要放寬 `types_seen` 的必要集——`budget_warning` 不一定每次都觸發（Short run 可能 budget_tokens > 80% 還沒到），所以它進「optional」而不是「required」event types。

## Open Questions

無。（proposal + design 已覆蓋所有決策面；實作細節見 `tasks.md`。）
