## Context

Module 4 Explorer（agent-core / explorer-tools / explorer-sse）執行時會在 `<ws>/.codebus/reasoning_log.jsonl` 落盤每步 ReAct 紀錄，同時透過 `GET /tasks/{id}/events` SSE 通道把 `agent_thought` / `agent_action_result` / `judge_verdict` / `coverage_gaps` / `progress` / `usage_delta` / `llm_call` / `budget_warning` 八種事件即時推給前端（schema 見 `docs/sidecar-api.md` §四、`openspec/specs/explorer-sse/spec.md` Requirement 2-6）。

`frontend-shell` archive 已交付 `useSseTask` composable（含 1000-event FIFO 上限、exponential backoff `[1s, 2s, 4s, 8s, 16s, 30s]`、bearer 透過 `useSidecar` 帶 query param）與七 tab `AuditPanel.vue`（`reasoning` 是其中一 tab）。

Phase A 設計 mockup `design/v1/07-explorer-react.html` 以 ReAct 三節拍（Think / Act / Judge）的「Step Card」為視覺主軸，配合 5 格 step-cell 進度條 + `cov-warn` 黃色 banner。`agent-explorer-spec.md` §七「視覺化設計重點」要求：每步動畫展開、`💭` thought 是 agentic 證據、失敗的嘗試也要顯示；§九 P0 列為「reasoning_log + 前端 console（Demo 靈魂）」。

R-01 互動教材 archive `2026-04-28-r-01-station-board` 確立的 page-level shell 模式（`pages/tutorial/[workspace_id]/[station_id].vue` + `useTutorialFiles` / `useStationRoute` / `useTutorialProgress` composable trio）是本步驟可參考的同層架構樣本。

## Goals / Non-Goals

**Goals:**

- 把八種 SSE 事件即時組裝成 ReAct 三節拍 Step Card timeline，提供 Demo 「打開 LLM 黑盒」的 agentic 證據（D-008）。
- 讓 `useExplorerStream` 成為**唯一**的 SSE 事件分派入口；page 元件、ConsoleTimeline、AuditPanel reasoning tab 共享同一份 reactive state。
- 失敗的工具呼叫（`ToolResult.error is not None`）必須跟成功 ToolResult 同樣顯示在 Step Card ACT 區，符合「失敗的嘗試也要顯示」的 spec 要求。
- vitest 對 fixture event sequence 重放可驗證 timeline / progress / banner 渲染結果，不需 sidecar 上線即可開發。
- Reconnect 中斷後的補齊（`GET /reasoning?after_step_id=`）由 P1 處理，但 P0 的 bucket-fill model 必須能無痛接上補齊事件（events upsert idempotent on `step` key）。

**Non-Goals:**

- Slider 回放 / Decision log replay UI（`agent-explorer-spec.md` §九 P2，本 change 不做）。
- Runtime mock event switch（fixture 只進 vitest，不進 production bundle）。
- LLM Call Inspector list / detail 分頁（步驟 28.5 獨立 change）。
- Q&A console（步驟 30 獨立 change，本 change 不處理 `qa_answer` / `rag_hits` / `kb_growth` 事件）。
- Pause / Stop explore UI binding（mockup 雖畫但 cancel endpoint 接線屬步驟 29 介入點）。
- Stations 累計推算（mockup「3 stations」）— P0 只渲染 `progress.current/total`，避免重做 `mark_station` 邏輯。

## Decisions

### Page route 採 `/explorer/[task_id]` 而非掛在 R-01 tutorial 路徑下

Explorer 任務的 task_id 命名空間與 sidecar 的 `TaskRegistry`（`explore_<8-hex>`）對齊（`sidecar-api.md` §三-bis）；R-01 的 `tutorial/[workspace_id]/[station_id]` 是 generator 產出後的學習路徑，兩者敘事階段不同（exploring vs learning）。獨立 route 也讓 SSE 連線生命週期單純（`onMounted` 開、`onBeforeUnmount` 關），不必擔心 R-01 station-page 切換時誤關 explorer SSE。

**Alternatives considered**：把 console 塞進 `pages/tutorial/[workspace_id]/explorer.vue`（rejected — workspace_id 跟 task_id 不是 1:1，且 R-01 是後讀檔，explorer 是先 stream，混在一起會讓 layout 切換邏輯複雜）。

### `useExplorerStream` 是唯一的 SSE 事件分派入口

Composable 層暴露四個 reactive surface 給上層消費：

```ts
interface UseExplorerStreamApi {
  stepBuckets: Ref<Map<number, StepBucket>>   // ConsoleTimeline 用，按 step asc 排序渲染
  progress: Ref<ProgressSnapshot | null>      // ProgressStrip 用
  coverageBanner: Ref<CoverageBanner | null>  // CoverageBanner 用，latest-only
  budgetBanner: Ref<BudgetBanner | null>      // CoverageBanner 用，per-kind latched
  auditRows: Ref<AuditRow[]>                  // AuditPanel reasoning tab 用，rolling window
  status: Ref<SseStatus>                      // 直透 useSseTask
  done: Ref<boolean>
  error: Ref<Error | null>
}

interface StepBucket {
  step: number
  thought?: { text: string; actions: ToolCall[] }
  actions: Array<{ tool: string; observation: string; tokens_used: number; isError: boolean }>
  judge?: { relevance: number; reason: string }
}
```

Composable 內 `watch(useSseTask().events, ...)` 把每筆新事件分派到對應 bucket / banner / row 串列。Bucket key 是 `event.data.step`（int），upsert 邏輯：thought/judge 各只有一筆（後到覆蓋），actions 累加（per tool call）。`auditRows` 是把 thought/action/judge 直接 stringify 成 `AuditRow.body`，提供 audit-panel 七 tab 同步。

**Alternatives considered**：分四個獨立 composable（`useStepTimeline` / `useProgress` / `useCoverageBanner` / `useReasoningRows`）—— rejected：四份各自開 EventSource 會讓 sidecar 收到四條重複連線；改用一份 composable + 多個 derived ref 是 Vue 慣用做法。

### Bucket-fill 而非 flat event array

每個 SSE 事件帶 `step: int`。把它們往 `Map<step, StepBucket>` upsert 後 v-for，timeline 渲染天然依 step asc 排序，不需要客戶端再做事件 group-by；新事件晚到也只更新對應 bucket，不會重排。對未來「reconnect 補齊」（`GET /reasoning?after_step_id=`）友善 —— 補齊事件直接 upsert，與即時事件邏輯共用。

**Alternatives considered**：flat `events: SseEvent[]` + 渲染前 `useMemo` group-by step（rejected：每筆新事件都要重算整份 group 結果，1000-event 上限下無感但設計上是浪費；另 v-for over Map 比 over groupBy 結果更好做 `:key`）。

### Coverage banner 與 budget warning 的展示策略

`coverage_gaps` 一個 explore 跑會發多筆（每輪一筆），但畫面「同時只顯示一條」就夠 —— 採 latest-only：新事件覆寫舊 `coverageBanner.value`。`budget_warning` 同個 `kind`（tokens / steps）一次性（spec.md Req 6 規定 `Each kind MUST be suppressed after a single successful emit`），但兩種 kind 可能各打一次：用 `budgetBanner: { tokens?: BudgetWarning; steps?: BudgetWarning }` 對應 mockup「黃色 cov-warn 同時顯示一個」的設計，UI 層決定要不要顯示哪個（優先級：steps > tokens；可調）。

**Alternatives considered**：banner 堆疊（rejected：mockup 沒這設計，且兩個橫幅同時出現會壓縮 timeline 視覺）。

### Vitest infra 在本 change 內 bootstrap（apply 階段 ingest 補錄）

Proposal 原宣稱「vitest 已在 `web/` 內」是事實錯誤；實際上 `web/package.json` 既無 vitest 依賴、`npm run test` 是 placeholder echo。本 change 順帶把 vitest 架起：

- 採 `vitest` + `@vue/test-utils` + `happy-dom` + `@vitest/coverage-v8` 純 devDeps；不引 jsdom（happy-dom 啟動快、API 對 DOM 觀察行為足夠）。
- `vitest.config.ts` 設 `environment: 'happy-dom'`、`globals: true`（讓 `describe` / `it` / `expect` 不需 import）、`setupFiles: ['tests/setup.ts']`、`include: ['tests/**/*.spec.ts']`。
- `web/tests/setup.ts` 註冊全域 `EventSource` mock（vitest 的 happy-dom 不提供）—— 用 `class FakeEventSource` 暴露 `_emit(type, data)` 給測試手動觸發。
- `package.json` 的 `test` script 改成 `vitest run`（CI 友善：non-watch 模式）；保留 watch mode 給開發者 `vitest watch`。
- 不引 `@nuxt/test-utils`（會把 Nuxt runtime 整個拉起，超 P0 範圍）；composable / 元件單元測手動 mock `useSidecar`，page 整合測直接掛載元件樹不過 Nuxt router。

**Alternatives considered**：等 phase B「測試 hook」獨立架設（rejected — 本 change TDD 是必須）；用 jest 而非 vitest（rejected — Nuxt 4 / Vite 生態以 vitest 為主，jest 需 babel transform 拖慢且 ESM 問題多）。

### Vitest fixture 採 JSON array 而非 JSONL

`web/tests/console/fixtures/explorer-stream.json` 用 JSON array `[{ "type": "...", "data": { ... } }, ...]` 而非 JSONL 字串。理由：
1. vitest 本就吃 JSON import（`import fixture from './fixtures/explorer-stream.json'`），不需手動 split lines。
2. Fixture 是 SSE event envelope（`{ type, data }`），不是 sidecar 那邊的 reasoning_log.jsonl 原始落盤格式 —— 兩者語意不同。
3. 從 `tests/golden/demo-synthetic/reasoning_log.jsonl` 寫一個 build script 一次性轉換（含補 `progress` / `coverage_gaps` / `budget_warning` 等非 reasoning_log 來源的事件）。

**Alternatives considered**：JSONL 字串 + 在測試裡 split（rejected：增加 boilerplate）；mock fastify EventSource server（rejected：超 P0 範圍）。

### 失敗的工具呼叫與成功者一視同仁進入 ACT 區

`agent_action_result` 事件的 `observation` 欄在 `ToolResult.error is not None` 時是被截斷的錯誤訊息（`explorer-sse` Req 2 規定）；StepCard ACT 區依 `data.observation` 是否含截斷標記判斷顯示樣式（紅框 vs 一般），但**仍照常渲染**，符合 `agent-explorer-spec.md` §七「失敗的嘗試也要顯示」要求。前端不額外判斷 isError 旗標 —— sidecar 不在 envelope 中區分 success/error，因此 P0 採文字啟發式（observation 含 `error:` 前綴或 `traceback` 視為 isError）；後續若 sidecar 加 `is_error: bool` 欄再切換至 explicit flag。

### `tokens_used` 為 0 時 UI 顯示「—」而非 `0 tokens`

`explorer-sse` Req 2 規定 `tokens_used: 0` 是 P0 placeholder（per-tool 計帳尚未上）。UI 顯示 `0 tokens · $0.0000` 會誤導使用者以為「真的沒花 token」；P0 採 `tokens_used > 0 ? formatTokens(...) : '—'` 顯示。後續真實計帳 wire 上後此 fallback 自然消失。

## Risks / Trade-offs

- **[Risk] 1000-event FIFO 上限可能截掉 Step 1 的 thought**：useSseTask 寫死 `EVENTS_CAP = 1000`，長 explore 會把舊事件擠掉。Mitigation：`useExplorerStream` 直接 watch `useSseTask().events` 並 fork 進 stepBuckets（永不過期）；FIFO 只影響 raw events 陣列，不影響 timeline。`auditRows` 採 rolling window（最後 N=200）符合 audit-panel 既有設計。
- **[Risk] Reconnect 期間漏事件**：`useSseTask` exponential backoff 重連，但 P0 不接 `GET /reasoning?after_step_id=` 補齊端點。Mitigation：bucket-fill 對補齊事件天然友善（upsert by step key），P1 直接掛上即可，本 change 不做 backfill 但設計不擋路。
- **[Trade-off] Explorer page 切換中途破連線**：使用者離開 `/explorer/[task_id]` 就斷 SSE，回頭時不重連（重新進頁會建新 EventSource）。這是刻意的：Explorer 任務是 short-lived（Module 4 budget_steps 預設 ≤ 30 步），長度 < 5 分鐘，不必持久連線。後續若改 long-running 任務再評估 background subscription 機制。
- **[Trade-off] `tokens_used: 0` placeholder 顯示「—」造成 demo 期間看不到 per-tool token**：D-021 / D-022 雙線記帳是 session-level，per-tool 攤提非 P0；demo 時 `usage_delta` 事件帶的 session_total_cost_usd 已能展示「整條路線跑完花多少錢」，per-tool 細粒度暫時用 `—` 占位不影響整體 demo 訊息。
- **[Risk] mockup 的 Step Card 視覺密度高（token / cost / 多個 tool row / observation 截斷）會讓 vitest snapshot 維護成本高**：採 unit test + 行為斷言（has-element queries），不做 snapshot 全量比對。

## Open Questions

(無 — backend SSE 已固定 contract，frontend 純消費；剩下均為實作細節，apply 階段處理。)
