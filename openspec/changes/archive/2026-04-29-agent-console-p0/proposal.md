## Why

D-008（First-run UX 三個等待點）把 Exploring 階段的 console 定為「最強 demo 資產」——agentic 證據集中、等待時間最長、把 LLM 黑盒打開給使用者看；對應 `docs/implementation-plan.md` §二第六階段步驟 28，也是 `docs/agent-explorer-spec.md` §七「前端視覺化（Demo 核心）」與 §九 P0「reasoning_log + 前端 console（Demo 靈魂）」的交付。

Backend SSE 已通電（`explorer-sse` capability，archive `2026-04-24-agent-sse-wiring`，emit `agent_thought` / `agent_action_result` / `judge_verdict` / `coverage_gaps` / `progress` / `usage_delta` / `llm_call` / `budget_warning` 八種事件），共用骨架 `useSseTask` + `AuditPanel.vue` 已就位（`frontend-shell` archive `2026-04-27`），R-01 互動教材也通電（`interactive-tutorial` archive `2026-04-28`）。本步驟新增 page-level Console 將 SSE 事件以 ReAct 三節拍（Think / Act / Judge）組裝成可走訪的 Step Card 時間軸，是 Trust Layer 敘事核心（O-04 LLM Call Inspector / O-05 Sanitizer Diff 同列）的入口。

## What Changes

- 新增 page route `web/app/pages/explorer/[task_id].vue`：Module 4 Explorer 執行中的即時 console 頁；左側 Step Card timeline，右側既有 `AuditPanel`（`activeTab="reasoning"`）。
- 新增 `web/app/components/console/` 四元件：
  - `ConsoleTimeline.vue` — 以 step 為 key 的 reactive `Map<step, StepBucket>`，每筆 SSE 事件 upsert 對應 bucket 後 v-for 渲染 `StepCard`。
  - `StepCard.vue` — 單一 step 的卡片，依到達順序展示 THINK（thought + action calls）→ ACT（observation per tool result，500 字元截斷標記）→ JUDGE（relevance + reason）三節拍。
  - `ProgressStrip.vue` — step-cell 進度條 + step 計數 + 已標站數，吃 `progress` 事件。
  - `CoverageBanner.vue` — `coverage_gaps` / `budget_warning` 事件的橫幅展示（一次性，不 stack）。
- 新增 composable `web/app/composables/useExplorerStream.ts`：包裝既有 `useSseTask`，把 SSE 事件流分派為 `stepBuckets` / `progress` / `coverageGaps` / `budgetWarning` 四個 reactive surface；emit `error` / `done` 終態給 page 統一處理。
- AuditPanel reasoning tab 接通：透過 `useExplorerStream` 把 reasoning 系列事件（`agent_thought` + `agent_action_result` + `judge_verdict`）轉成 `AuditRow` 餵給既有 `AuditPanel.vue`（行為層綁定，不改 `frontend-shell` capability 的既有 Requirement）。
- vitest fixture：`web/tests/console/fixtures/explorer-stream.json` 從 `tests/golden/demo-synthetic/` 既有 `reasoning_log.jsonl` 改寫一份完整 SSE 序列（含 coverage 與 budget 事件），給 vitest 與 `useExplorerStream` 單元測試使用。
- 文件：
  - `docs/agent-explorer-spec.md` §七「前端視覺化」補一段 Step Card 三節拍 + bucket-fill state 模型對應實作。
  - `docs/decisions.md` D-008 後續清單把「Agent console 元件」打勾，補本 change 連結。

## Non-Goals

- **Slider 回放 / Decision log replay UI**：`agent-explorer-spec.md` §九列為 P2，本 change 不做。
- **Runtime mock event switch（`?mock=1`）**：Backend `/explore` 503 `EXPLORE_NOT_CONFIGURED` 規範下 sidecar 沒有 mock-explore 模式；fixture 只進 vitest，不進 production bundle。
- **MOC empty CTA「執行頁面」**：`pages/tutorial/[workspace_id]/index.vue` 觸發 generate flow 的入口，屬步驟 29 三個介入點範疇。
- **LLM Call Inspector list / detail 分頁**：步驟 28.5 獨立 change，本步驟僅讓 `llm_call` SSE 事件能被既有 AuditPanel llm tab 消費（既有路徑，不擴功能）。
- **Q&A console**：`qa_answer` / `rag_hits` / `kb_growth` 事件屬步驟 30 Q&A UI 範疇；本 change 的 `useExplorerStream` 不處理 Q&A 事件。
- **可暫停 / 停止 explore**：mockup 雖畫有「暫停 / 停止」按鈕，cancel endpoint `POST /tasks/{id}/cancel` 是現有能力但 UI binding 不在本 change 範圍。
- **Stations 總覽 panel / station 數推導**：mockup 顯示「3 stations」是從多個 `judge_verdict` 累計而來；P0 只渲染 `progress.current/total` 文字，不另算 stations 累積（避免重做 `mark_station` 邏輯）。

## Capabilities

### New Capabilities

- `agent-console`: Module 4 Explorer 執行中的 page-level console。將 `explorer-sse` 八種 SSE 事件流組裝為 ReAct 三節拍（Think / Act / Judge）Step Card timeline，搭配 progress 條、coverage banner、budget warning banner，並把 reasoning 系列事件同步餵給既有 `AuditPanel` reasoning tab。

### Modified Capabilities

(none)

## Impact

- Affected specs:
  - New: openspec/specs/agent-console/spec.md
- Affected code:
  - New:
    - web/app/pages/explorer/[task_id].vue
    - web/app/components/console/ConsoleTimeline.vue
    - web/app/components/console/StepCard.vue
    - web/app/components/console/ProgressStrip.vue
    - web/app/components/console/CoverageBanner.vue
    - web/app/composables/useExplorerStream.ts
    - web/vitest.config.ts
    - web/tests/setup.ts
    - web/tests/console/fixtures/explorer-stream.json
    - web/tests/console/useExplorerStream.spec.ts
    - web/tests/console/ConsoleTimeline.spec.ts
    - web/tests/console/StepCard.spec.ts
    - web/tests/console/ProgressStrip.spec.ts
    - web/tests/console/CoverageBanner.spec.ts
    - web/tests/console/explorer-page.spec.ts
  - Modified:
    - web/package.json（補 devDependencies + 把 `test` script 從 stub 改成 `vitest run`）
    - docs/agent-explorer-spec.md
    - docs/decisions.md
    - docs/implementation-plan.md
  - Removed:
    (none)
- Affected dependencies:
  - 新增 devDependencies（純測試 infra，不進 Tauri SPA bundle）：
    - `vitest`（測試 runner）
    - `@vue/test-utils`（Vue 元件 mount API）
    - `happy-dom`（vitest 的 DOM 環境，比 jsdom 啟動快）
    - `@vitest/coverage-v8`（coverage 用 v8 內建，不需額外編譯器）
  - **修正前一版 ingest 的事實錯誤**：`web/package.json` 既有 `npm run test` 是 `echo "deferred to phase B"` 的 stub，本 change 同時負責把它改成 `vitest run`。
- Affected runtime contracts:
  - 不改 `explorer-sse` 既有 SSE wire schema，純消費端。
  - 不改 `frontend-shell` 既有 Requirement（`useSseTask` / `AuditPanel` 七 tab / 設計 token 三條皆原樣套用）。
