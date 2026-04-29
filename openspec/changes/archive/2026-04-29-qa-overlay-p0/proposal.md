## Why

D-016（Q&A Agent + KB add_to_kb 沉澱機制）把「使用者問問題 → RAG 命中 → 必要時 ReAct 補查 → 答案附引用 → 值得的話新增進 KB」定為 Module 8 P0；對應 `docs/implementation-plan.md` §二第六階段步驟 30「聊天 UI + 引用 panel + KB growth 稽核 tab」。Backend 已完全通電：`POST /qa` + 5 種 SSE 事件（`rag_hits` / `agent_thought` / `agent_action_result` / `kb_growth` / `qa_answer`）+ `add_to_kb` 三段 sanitize / dedup / size 防呆 + `kb_growth.jsonl` 落盤、`qa-agent` capability spec 鎖死所有 invariant；前端 `<QAEntry>` mdc 元件已在 R-01 互動教材內存在但目前是 **placeholder navigation**（`router.push('/qa?prompt=...')`），實際 page 沒做。

Mockup `design/v1/12-qa-drawer.html` 把 Q&A 設為 **drawer overlay**（`<aside class="qa">` 疊在 stage 上、underlay 半透明）而非獨立 page —— 這是刻意設計：Q&A 是 contextual 的（「我在讀這站、要問這段 code」），底層 station 必須仍然可見讓使用者保留閱讀脈絡。session 短命、無 URL 持久化需求（`kb_growth.jsonl` 已是稽核 trail，跨 session 記憶屬 Phase 2 per `docs/qa-agent.md §十`）。本 change 把 Q&A 完整通電：drawer overlay + multi-turn 視覺骨架 + 引用 panel + `kb_growth` AuditPanel tab 接通。

## What Changes

- 新增 module-level singleton composable `web/app/composables/useQaSession.ts` — `useQaSession()` 回 `{ open, close, start(prompt, originatingStationId?), turns, currentTaskId, status, ... }`；start() 內 `POST /qa` 取得 `qa_<8hex>` task_id 後 `useSseTask(task_id)` 開 SSE，把 5 種 event 分派為 turn-shaped reactive state（每筆 question 一個 `QaTurn` bucket：rag hits / react steps / kb growth / answer + citations）；多 turn 累積在 `turns: Ref<QaTurn[]>`；drawer 關閉時 `close()` 清空。
- 新增元件 `web/app/components/qa/QAOverlay.vue` — drawer 元件（`<aside>` 疊 stage 上）；`<QaTurnCard>` 子元件渲染 mockup 12 那 4 phase（user msg / RAG hits / ReAct steps / answer with citations）；底部 composer 含 input + send button + meta strip（`Pass 3 sanitize on` / `session add N/20` / `budget X/10 步 · $cost`）；header 顯示 origin chip（從哪 station_id 召喚）；ESC 關閉、`Cmd+K` / `Ctrl+K` 全域開啟。
- 新增 `web/app/components/qa/QaTurnCard.vue` — 單一 turn 的 4 phase 視覺；`citations` 區塊用 `<QaCitations>` 子元件展示 `file_path:line_start-line_end` + `related_stations` station chip。
- 新增 `web/app/components/qa/QaCitations.vue` — citations 列；station chip 點擊 emit `navigate-to-station(station_id)`，由 caller layout 決定如何處理（drawer 自己不做 router push）。
- **改既有** `web/app/components/content/QAEntry.vue` — 砍掉 `router.push('/qa?prompt=...')` placeholder，改 imperative 呼叫 `useQaSession().start(prompt, currentStationId)`；R-01 station page 順便補一支 expose station_id 給 mdc slot 的方式（暫透過 page-level `provide('currentStationId', stationId)`）。
- 新增 layout-level mount：`web/app/layouts/default.vue` 加 `<QAOverlay />` 與全域 `Cmd+K` listener，drawer 永遠在 layout 樹但靠 composable 的 `open` ref v-if 顯隱（避免每個 page 自己 mount）。
- AuditPanel `kb_growth` tab 接通（reuse `llm-call-inspector-p0` 引入的 `useAuditJsonl(ws, 'kb_growth')`）—— 純 list rendering，**不開 inspector overlay**（rollback / detail 屬 Phase 2 per `docs/qa-agent.md §十「批次 rollback / KB 清理 UI」`）；Live-tail：`useQaSession` 也 watch SSE `kb_growth` event 同步 push 進 audit list。
- `qa-agent` capability 不改 backend Requirements（純消費端）；新增 capability `qa-overlay` 規範前端三件事（drawer overlay 行為 / `useQaSession` composable 契約 / QAEntry 改走 imperative）。
- `frontend-shell` 不改既有 Requirements（七 tab AuditPanel 既有契約、`select-row` emit 已由 `llm-call-inspector-p0` 加好；本 change 只 reuse）。
- 文件：`docs/decisions.md` D-016 後續清單把「前端聊天 UI」打勾、補本 change 連結；`docs/qa-agent.md §八` 補一段「P0 drawer overlay 模式」對應實作；`docs/implementation-plan.md` §二第六階段步驟 30 加註 ✅ landed；`docs/sidecar-api.md` 不動（無新 endpoint）。

## Non-Goals

- **Cross-session memory（記住使用者歷史問題）** — `docs/qa-agent.md §十` 明列 Phase 2，本 change 不做；drawer 關閉即清空 `turns`，下次開是空 session。
- **Multi-question concurrency** — 同一時刻只能有一個 in-flight Q&A task；`POST /qa` backend 走 `TaskRegistry` single-slot 會回 409 `TASK_IN_FLIGHT`。前端 send button 在前一 turn `done` event 收到前 disabled，不靠 backend race。
- **Multi-turn 等於多 POST** — 每筆 user 問句都是獨立 `POST /qa`（backend 無「continue session」endpoint），drawer 只是 visually stitch 多筆結果；不偽造 conversation continuity（system prompt 不會看到前一輪內容，這是 backend 已知限制）。
- **Rollback `kb_growth` entry / KB 清理 UI** — `docs/qa-agent.md §十` Phase 2；mockup 12 line 462 的 `↶ rollback` 按鈕本 P0 不渲染。
- **Citation file open in side panel** — mockup 12 提到「點 file:line 跳 side panel」是 P1（`docs/qa-agent.md §十一` P1 列明），本 P0 citation 只顯示 `file_path:line_start-line_end` + `related_stations` chip；station chip 點擊 emit 給 caller，file:line 點擊不做事（cursor 顯 default、無 hover 樣式）。
- **`add_to_kb` rollback 反向 event** — `kb_growth.jsonl` 雖然支援 rollback event_type，但 P0 永遠是 `"add"`（per CLAUDE.md `七層 Audit JSONL` 表格）；UI 不暴露 rollback button。
- **Drawer 浮動 / resize / multi-instance** — drawer 固定靠右，寬 ~480px，不可拖曳調寬；同一時刻只一個 drawer instance（layout 層 singleton mount）。
- **Q&A inspector overlay**（類似 `LlmCallInspector` 的 detail drawer）— `kb_growth` tab 是 passive list，不開 inspector；Q&A turn detail 已在 drawer 內展開、無第二層 detail。

## Capabilities

### New Capabilities

- `qa-overlay`: 前端 Q&A drawer overlay 與 session 管理 — module-level singleton `useQaSession` composable + `<QAOverlay>` drawer + `<QaTurnCard>` 4-phase 渲染 + `<QaCitations>` 引用列 + `Cmd+K` 全域召喚 / `ESC` 關閉 / station-chip emit；同時規範 `<QAEntry>` mdc 元件從 placeholder route 改 imperative 呼叫；`kb_growth` AuditPanel tab 透過 `useAuditJsonl(ws, 'kb_growth')` 接通 live-tail。

### Modified Capabilities

(none — backend Q&A spec 純消費端、無 contract 改動；frontend-shell AuditPanel 既有契約足夠；R-01 互動教材 mdc 元件契約「dumb + emit pattern」可在不改 prop shape 的前提下換成 imperative call — 改 click handler 內部實作即可，不破現有 Requirement。)

## Impact

- Affected specs:
  - New: openspec/specs/qa-overlay/spec.md
- Affected code:
  - New:
    - web/app/composables/useQaSession.ts
    - web/app/components/qa/QAOverlay.vue
    - web/app/components/qa/QaTurnCard.vue
    - web/app/components/qa/QaCitations.vue
    - web/tests/qa/useQaSession.spec.ts
    - web/tests/qa/QAOverlay.spec.ts
    - web/tests/qa/QaTurnCard.spec.ts
    - web/tests/qa/QaCitations.spec.ts
    - web/tests/qa/qa-overlay-page-integration.spec.ts
    - web/tests/qa/fixtures/qa-stream.json（vitest fixture，仿 `agent-console-p0` 套路；含完整 5 event sequence）
    - web/tests/qa/fixtures/kb-growth.json（kb_growth.jsonl entries fixture for AuditPanel kb_growth tab test）
  - Modified:
    - web/app/components/content/QAEntry.vue（砍掉 `/qa?prompt=...` placeholder route，改 `useQaSession().start(prompt, currentStationId)` 呼叫；保留既有 prop shape 與按鈕外觀）
    - web/app/layouts/default.vue（加 `<QAOverlay />` mount + 全域 `Cmd+K` / `Ctrl+K` listener）
    - web/app/pages/tutorial/[workspace_id]/[station_id].vue（補 `provide('currentStationId', stationId)` 讓 `<QAEntry>` mdc slot 拿得到 originating station_id）
    - web/app/pages/explorer/[task_id].vue（kb_growth tab 接 `useAuditJsonl(ws, 'kb_growth')` rows + 同步把 `useQaSession` 的 SSE `kb_growth` event 餵進去做 live-tail；其他 tab 不影響）
    - docs/decisions.md（D-016 後續清單打勾）
    - docs/qa-agent.md（§八 補 P0 drawer overlay 模式對應實作段）
    - docs/implementation-plan.md（步驟 30 加註 ✅ landed）
- Affected runtime contracts:
  - 不改 `POST /qa` endpoint signature
  - 不改 5 種 SSE event schema
  - 不改 `kb_growth.jsonl` schema（純消費端）
- Affected dependencies:
  - 不引新 npm 套件（vitest infra 已在 `agent-console-p0`、`useAuditJsonl` 與 Tauri `read_audit_jsonl` 已在 `llm-call-inspector-p0`）
  - **依賴前置 change**：`llm-call-inspector-p0` 必須先 apply（提供 `web/app/composables/useAuditJsonl.ts` + Tauri `read_audit_jsonl` IPC + AuditPanel `select-row` emit 雖本 change 不用 select-row，但 useAuditJsonl 與 Tauri command 是 hard dependency）。Apply 順序：`llm-call-inspector-p0` → `qa-overlay-p0`。
