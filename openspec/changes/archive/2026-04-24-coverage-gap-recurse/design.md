## Context

`explorer-react-loop-p0`（2026-04-24 archive）把 `run_explorer` 的 6 步 Think→Act→Observe→Judge→Log→Update 主迴圈做完，並在尾端留下一個 dormant hook：

```python
if _COVERAGE_RECURSION_ENABLED:  # pragma: no cover - intentionally dormant
    _ = coverage  # silences unused-parameter lint in the dormant branch
    raise RuntimeError(
        "coverage recursion is disabled in P0 — lands in coverage-gap-recurse"
    )
```

`CoverageChecker` Protocol 也在同次 landed（`codebus_agent.agent.protocols`），簽名 `async check(state) -> list[Gap]`。P0 的測試用 `_CountingCoverage(gaps=[...])` 驗證「即使 coverage.check 被呼，遞迴絕不觸發」。本 change 負責把這塊通電。

約束：

- **one-shot LLM 規紀**（`docs/agent-core.md §七`）：Coverage 不進 ReAct 子迴圈；一次 LLM call，回一組 `Gap`。跟 Judge 的行為對齊，但**呼叫時機不同** —— Judge 每輪跑、Coverage 只在主迴圈收斂後跑一次。
- **TrackedProvider 紅線**（invariant #4）：所有 outbound LLM 必包 `TrackedProvider` 且 inner class 必在 `ALLOWED_INNER_TYPES` 白名單內。目前白名單是 `{MockProvider, OpenAIEmbeddingProvider, OpenAIChatProvider}` —— Coverage 跟 Judge 同走 `OpenAIChatProvider` + JUDGE role，不需擴白名單。
- **Budget 單一貨幣**（`docs/agent-core.md §五`）：`budget_steps_left` 是目前唯一實作的 budget 貨幣；遞迴共享同一份 state 所以繼承剩餘步數。token-aware budget 留給步驟 21。
- **SSE event 向後相容**（`explorer-sse` capability）：`explorer_loop` 已有 `agent_thought` / `agent_action_result` / `judge_verdict` / `progress` / `usage_delta` / `llm_call`；新增 `coverage_gaps` 要不破壞既有 frontend（目前前端未實作，但格式要跟既有 event envelope 一致）。
- **遞迴安全**（`docs/agent-core.md §九`）：上限 3 層、每層遞減 budget、depth 參數不可對外 public。

## Goals / Non-Goals

**Goals:**

- 讓 Explorer 主迴圈收斂後能「發現 gap → 主動補查」，閉合 Agent 核心 Demo 靈魂。
- Coverage 一次性 LLM call，不牽動現有 Judge / Explorer prompt，新 prompt 模組獨立（`prompts/coverage.py`）。
- reasoning_log 能精準標示「哪段是主 loop、哪段是 gap round」—— 用一行特殊 `Step` 開頭記錄 coverage round，方便 golden-sample replay 驗證遞迴行為。
- SSE emit `coverage_gaps` event 把 gap 資訊帶到前端，format 跟現有 event envelope 對齊（`{"type": str, ...}` 頂層 key）。
- 符合 `docs/agent-core.md §七` 的紀律：Coverage 不改 state、不呼叫 ExplorerTools。

**Non-Goals:**

見 proposal Non-Goals 段（keyword fallback / gap 內嵌遞迴 / gap priority / 前端 UI / Gap schema 擴欄 / coverage golden baseline）。

- 本 change **不** 引入新 budget 貨幣 —— `budget_tokens_left` 仍是欄位存在但未被 enforce，留給步驟 21。
- 本 change **不** 把 `_depth` 參數暴露到 HTTP 層；`POST /explore` 請求 schema 不動。
- 本 change **不** 改 `CoverageChecker` Protocol 簽名（已是 P0 landed 的 `async check(state) -> list[Gap]`）。
- 本 change **不** 改 `Gap` / `CoverageResult` schema（schema 早在 P0 types.py 埋好）。
- 本 change **不** 調整 `_MIN_STATIONS_FOR_CONVERGENCE`（收斂條件保持 3 不動）。

## Decisions

### Decision 1：遞迴 vs iterative loop — 用 tail-recursion

選 tail-recursion（`return await run_explorer(..., _depth=_depth+1)`），不改成 `while run-and-coverage` iterative loop。

理由：

- **可讀性** — 既有 P0 code site 已以 `if _COVERAGE_RECURSION_ENABLED: ...` 形狀寫在迴圈後，tail-recursion 最貼近「主迴圈結束 → 評估 gap → 如需要則重跑」的語意。Iterative loop 會需要把整個 while 用更外層 loop 包住、搬動 `initial_budget_steps` snapshot 等，改動面積大。
- **Depth 3 對 Python recursion 零壓力** —— Python 預設 recursion limit 1000，3 層遞迴 stack 幾 KB。
- **Stack trace 乾淨** —— debug 遞迴時 stack 直接顯示 depth 路徑。

**替代方案**：iterative outer loop。棄用，面積過大且語意不直觀。

### Decision 2：遞迴體重用同一份 `state`

遞迴呼叫共用同一 `ExplorerState` 實例（不 deep-copy）。

理由：

- `state.stations` / `state.visited_files` / `state.messages` / `state.pending_queue` / `state.budget_steps_left` / `state.step_count` 都應該**累積**跨主 loop + gap round。
- `_enqueue_gap_investigation` 往 `state.pending_queue` 塞 gap target、往 `state.messages` 塞「Coverage 回報 N 個 gap」的 user message 後，下一輪 Think 直接看到 gap 指示。
- Deep-copy 反而會讓 gap round 的 reasoning_log 與主 loop 斷開，Replay 會看到兩份獨立 state snapshot。

**推論**：`run_explorer` 是「state-mutating function」—— 與其透過回傳值疊 stations，不如讓 state 本身成為跨 round 的累積器。既有 P0 程式碼也是如此（`state.stations.append(...)` / `state.budget_steps_left -= 1`）。

### Decision 3：`_depth` 參數 vs 全域 counter

選 `_depth: int = 0` keyword-only 參數，底線前綴代表「實作細節、不是 public API」。

理由：

- 不用全域 / `contextvars` —— 這類 hidden state 會讓測試難寫（每個測前要 reset）且遞迴 concurrency 情境下會互擾（即便目前 MVP 單 process 單 explorer run，模式紀律仍照顧）。
- keyword-only 防止呼叫端誤傳 positional。
- 底線前綴符合 Python 慣例「internal-use」標記；HTTP 層的 `run_explorer(...)` 呼叫不傳 `_depth`（讓它 default 0）。

**替代方案**：用 `ContextVar` 記 depth。棄用，hidden state 測試不友善。

### Decision 4：Coverage round 的 reasoning_log Step 表示

在遞迴進入前、SSE emit 後、遞迴呼叫前，寫一行特殊 Step：

```python
logger.write(Step(
    step=state.step_count,
    ts=datetime.now(timezone.utc),
    thought=f"[coverage] round-{_depth + 1} gaps={len(gaps)} will_recurse={will_recurse}",
    tool_calls=[],
    tool_results=[],
    judge_verdict=None,
    tokens_used=0,
    explorer_prompt_version=EXPLORER_PROMPT_VERSION,
    judge_prompt_version=JUDGE_PROMPT_VERSION,
))
```

理由：

- `Step` schema 已有 `judge_verdict: JudgeVerdict | None = None` default，所以 coverage round 可以直接用 `None` —— 不必擴 schema。
- `thought` 欄位塞結構化標記 `[coverage] round-N gaps=K will_recurse=B`，Replay / drift guard 可 regex 抓。
- `step` 繼續用 `state.step_count`（與該時刻的迭代計數一致），保持單調遞增。Coverage round Step 不增 `state.step_count`（它不是一個 iteration），下一輪遞迴的第一個 Explorer iteration Step 會接續。

**替代方案**：擴 `Step` schema 加 `kind: Literal["iteration", "coverage_round"]` 欄位。棄用 —— 既有 reasoning_log.jsonl 的 Replay tooling 都吃現有 schema，擴欄要全面 re-baseline。`[coverage]` 前綴 thought 是更輕量的 marker。

### Decision 5：`coverage_gaps` SSE event 格式

```json
{
  "type": "coverage_gaps",
  "round": 0,
  "gaps": [
    {"description": "string", "suggested_target": "string | null"}
  ],
  "will_recurse": true,
  "skip_reason": null
}
```

理由：

- `round` 用 `_depth`（0-indexed），表示「這是第幾次 coverage check」（0 表示主 loop 後第一次）。
- `gaps` 是 `Gap.model_dump()` 的陣列，跟 `JudgeVerdict` 的 `relevance / reason` 嵌入方式對齊（直接 shallow dump）。
- `will_recurse` bool + `skip_reason` nullable string — 當 `will_recurse=False` 時 `skip_reason` 必為 `"budget_exhausted" | "max_depth_reached" | "no_gaps"` 之一；`will_recurse=True` 時 `skip_reason=None`。前端能用 `skip_reason` 顯示「Agent 看到 gap 但 budget 用完了」這類敘事。

**替代方案**：把 `skip_reason` 用 `null | undefined` 分流表示，棄用（JSON wire 型別不穩、前端要多寫分支）。

### Decision 6：`_enqueue_gap_investigation` 的 pending_queue 與 messages 雙推

遞迴啟動前把 gap 雙推：

```python
def _enqueue_gap_investigation(state: ExplorerState, gaps: list[Gap]) -> None:
    for gap in gaps:
        target = gap.suggested_target or f"gap:{gap.description[:80]}"
        state.pending_queue.append(target)
    summary = "、".join(g.description[:60] for g in gaps[:3])
    if len(gaps) > 3:
        summary += f"（及其他 {len(gaps) - 3} 項）"
    state.messages.append(Message(
        role="user",
        content=f"Coverage 回報 {len(gaps)} 個 gap：{summary}。請優先補查。",
    ))
```

理由：

- **pending_queue** 驅動 Explorer 的下一輪 Think — 有 queue 就不會踩 `queue_empty` 收斂。
- **messages role=user** 讓下一輪 Think prompt 看到 gap 文字指示（Explorer prompt render 會吃 `state.messages`）。Message 是 user 角色、非 system —— 系統 prompt 應該保持穩定，gap 指示是「使用者/上游要求」的 contextual message。
- `suggested_target=None` 時用 `f"gap:{description[:80]}"` placeholder，保留訊號且不會被當成檔案路徑誤解（Explorer 會 `search(keyword)` 或 `trace_import(symbol)` 自己解）。

**替代方案**：只推 `messages`（讓 queue 保持空），但這樣會讓 `_should_stop` 的 `queue_empty` 分支立刻觸發、遞迴 Think 瞬間被中止。棄用。

### Decision 7：`LLMCoverageChecker` 與 Judge 並排的 factory 形狀

- `api/__init__.py::wire_llm_dependencies` 加 `app.state.llm_coverage_provider = _make_chat_provider_factory(default_module="coverage", temperature=0.0)`。
- `api/explore.py` 的 `_require_explore_deps(http_request)` 加 `coverage_factory = getattr(state, "llm_coverage_provider", None)`，跟 reasoning / judge 同級必要；少一個就 503。
- `POST /explore` handler 建 `coverage = LLMCoverageChecker(coverage_factory, workspace_root)` + `coverage.set_emitter(emitter)` 後餵給 `run_explorer`。

理由：

- 對稱 — Judge / Coverage 兩個 one-shot evaluator 同層，工廠形狀一致。
- `default_module="coverage"` 讓 `token_usage.jsonl` / `llm_calls.jsonl` 的 `module` 欄能拆出 coverage vs judge 的呼叫分布（符合 `docs/agent-core.md §十` 的 module 列舉）。
- `temperature=0.0` 低溫，確定性輸出，與 Judge 一致。

### Decision 8：空 gaps 的語意 — 仍發 SSE，不寫 Step

無 gap（`coverage.check(state) == []`）時：

- **發** SSE `coverage_gaps` event（`gaps=[]`, `will_recurse=False`, `skip_reason="no_gaps"`），讓前端能顯示「Coverage 查無 gap，收斂乾淨」敘事。
- **不** 寫 reasoning_log Step（避免噪音 —— 沒 gap 等於沒事）。

理由：

- SSE 是「即時敘事層」，要傳達「有無 gap」資訊；reasoning_log 是「審計層」，沒 gap 代表「沒補查動作」不需記錄。
- 反過來「有 gap 但不遞迴」（budget 耗盡 / 已達 depth 上限）**要寫** Step，因為這是一個「Agent 原本要做但被限制擋下」的決策軌跡，稽核上有意義。

## Risks / Trade-offs

- [LLM 把合法收斂誤判為 gap → 無謂遞迴] → prompt 裡明確指示「只在 stations 明顯遺漏任務關鍵路徑時回 gap」＋ low-temp（0.0）降低隨機性。**Mitigation**：`_COVERAGE_MAX_DEPTH = 3` 硬上限做安全網；連三輪都「Coverage 覺得還有 gap」也只能吃三輪 budget 就停。
- [Coverage prompt 太大 context，一次 call 爆 token] → state.visited_files / state.stations 直接吃進 prompt 的話大 repo 會爆。**Mitigation**：render_coverage_prompt 比照 Judge 做 window —— visited 前 20 條 + stations 全列 + 任務 + `... (N more)`；避免把完整 visited 倒進 prompt。超長 repo 的徹底解由步驟 21 `context 壓縮` 處理。
- [遞迴中 budget 被主 loop 吃光、gap round 什麼也做不出] → `will_recurse=False, skip_reason="budget_exhausted"` 誠實回報；主 loop 初始 budget 建議至少 `_MIN_STATIONS_FOR_CONVERGENCE * 2`（6）以上才留遞迴 margin。這條不在 code 裡 enforce，是 HTTP caller 紀律。**Mitigation**：若 HTTP request 預設 budget 太小，等實際跑 Demo fixture 驗證後再獨立 change 調 default。
- [`_enqueue_gap_investigation` 把 placeholder `gap:<desc>` 推進 queue 讓 Explorer 拿 description 當關鍵字搜] → search 噪音可能高。**Mitigation**：placeholder 截 80 字控 prompt 長度；下輪 Think 本來就會用 LLM 判斷這 keyword 要不要真的 search、Judge 會 filter 掉雜訊 station。
- [遞迴內 LLMJudge 再跑一輪的提示版本漂移] → 遞迴重用同一 judge 實例（已 set_emitter 一次），provider / prompt 都不變；drift 檢測由現有 `explorer-judge-golden` 的 `judge_prompt_version` 欄位覆蓋。
- [HTTP 層多一個 factory slot 沒設好就 503] → `_require_explore_deps` 早期 raise 503，訊息列出缺 slot；保持跟 `llm_reasoning_provider` / `llm_judge_provider` 的錯誤格式一致。
- [SSE event `coverage_gaps` 前端尚未消費] → 前端 Module 7 實作時才接；在那之前事件只落在 reasoning_log 後方的 SSE channel，沒人讀也不會壞 backend。新 event type 不 break 既有前端（它會直接 discard 未知 event）。

## Migration Plan

- 無 schema 破壞 —— `Gap` / `CoverageResult` / `Step` / `Message` 都在 P0 已 landed。
- 無 HTTP API 破壞 —— `POST /explore` request/response 不變。
- 既有 `_NoopCoverage`（若測試裡用）留著，只是 production path 改用 `LLMCoverageChecker`。
- 既有 `test_coverage_recursion_hook_remains_dormant_in_p0` 測試要改寫（已列入 proposal tasks）；golden-sample baseline 若需 re-baseline 照 `explorer-judge-golden` 的 drift-guard 流程處理。

## Open Questions

無。（proposal + design 已覆蓋本 change 的所有決策面。）
