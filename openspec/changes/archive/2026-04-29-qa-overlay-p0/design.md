## Context

Q&A backend stack（D-016 + qa-agent capability + kb-growth capability）已通電：

- `POST /qa` 收 `{workspace_root, question, originating_station_id?}` 後 `TaskRegistry.create("qa")` → 回 `{"task_id": "qa_<8hex>"}`；429 等同 explore 走 409 `TASK_IN_FLIGHT`。
- `run_qa` ReAct loop emit 5 種 SSE event（`rag_hits` 一次 → 必要時 `agent_thought` / `agent_action_result` 多次 + `kb_growth` 多次 → `qa_answer` 一次 → `done`）。
- `add_to_kb` 三段防呆（Pass 3 sanitize / dedup by cosine ≥ 0.85 / `_QA_MAX_CHUNK_SIZE_CHARS` cap 4000）+ `_QA_MAX_ADD_TO_KB_PER_QUESTION` (5) / `_QA_MAX_ADD_TO_KB_PER_SESSION` (20) budget。
- `kb_growth.jsonl` 寫 add event（P0 永遠 `event_type: "add"`），含 `entry_id` / `source` / `related_stations` / `originating_station_id` / `question`。

前端側 R-01 互動教材已落地（archive `2026-04-28-r-01-station-board`）：station markdown 以 `@nuxtjs/mdc` 渲染，`<QAEntry>` 是其中一個 mdc 元件，目前 `handleClick` 是 `router.push('/qa?prompt=...')` placeholder（TS 註記寫明「P0 placeholder route — the Q&A page wiring lands in step 30」）。`useSseTask` 既有實作支援 `qa_*` task id（regex 已含 `qa_` prefix）。

`agent-console-p0` archive（2026-04-29）建立的 `useExplorerStream` bucket-fill state model 是本 change `useQaSession` 的最佳模板：以 `step` 為 key 的 `Map<step, StepBucket>`、reactive surface 分派、auditRows rolling window。Q&A 換成以 `turn` 為 key（每 user 問句一個 turn），結構同款但語意不同。

`llm-call-inspector-p0` propose（2026-04-29 parked）引入 `useAuditJsonl(ws, kind)` composable + Tauri `read_audit_jsonl` IPC — 本 change 對 `kb_growth` AuditPanel tab 的接通直接 reuse，apply 順序鎖死「`llm-call-inspector-p0` → `qa-overlay-p0`」。

Mockup `design/v1/12-qa-drawer.html` 是 drawer overlay 設計，搭配 stage-dim 半透明 underlay、`Cmd+K` 召喚、ESC 收起；底部 composer 含 input + send + meta strip；header 顯示 origin chip 標明從哪 station 召喚。

## Goals / Non-Goals

**Goals:**

- Q&A drawer overlay 完整通電：使用者從 R-01 station 點 `<QAEntry>` 或按 `Cmd+K` → drawer 滑出 → 問題 → RAG 命中 → ReAct（必要時）→ 答案 + 引用 + KB growth 顯眼可見；station 底層保持可見不蓋住閱讀脈絡。
- `useQaSession` 是**唯一** Q&A SSE 分派入口（與 `useExplorerStream` 同款 invariant），module-level singleton；任何 caller 拿到的都是同一份 ref，不允許多 instance。
- AuditPanel `kb_growth` tab 接通 live-tail：drawer 內新增的 KB entry 即時出現在右側 audit panel，使用者直接看到 KB 在長。
- `<QAEntry>` mdc 元件改 imperative 後 R-01 既有契約不破（dumb + emit pattern 改成 dumb + imperative call，prop shape 不變）。

**Non-Goals:**

- Cross-session memory（drawer 關閉清空）— Phase 2
- Citation file:line click → side panel — Phase 2
- KB growth rollback button — Phase 2
- Drawer resize / drag / multi-instance — fixed 480px width singleton
- Q&A inspector overlay（kb_growth row 點擊開 detail）— P0 不做、與 D-022 LLM Inspector 同款延後

## Decisions

### `useQaSession` 採 module-level singleton 而非 per-mount instance

**Approach**：composable 內部 `const _state = { open: ref(false), turns: ref([]), ... }` 在 module scope；`useQaSession()` 直接 return `_state` 引用。任何 caller 拿到的都是同一份 ref。Layout 層 `<QAOverlay />` 唯一 mount、靠 `_state.open` v-if 顯隱。`<QAEntry>` 與 `Cmd+K` 都呼叫同一個 `useQaSession().open()`。

**Alternatives considered**：
- per-component instance（每個用 `useQaSession` 的 caller 各自 `ref`）— rejected：drawer 是全 app 唯一 UI element，多 instance 會多開 SSE / 多筆 turns 失同步；同 `useSidecar` 已採 module-level pattern。
- Pinia store — rejected：本專案無 Pinia，引入只為一個 store 開銷大；module-level singleton 是 Vue 慣用 pattern。

### Drawer 不走 vue-router、無 URL 表達

**Approach**：drawer 開合純靠 `useQaSession()._state.open` ref；瀏覽器 URL 完全不變。送出問題不改 URL、收起 drawer 不留 history entry。`<QAEntry>` 從 `router.push('/qa?...')` 改成 `useQaSession().start(prompt, currentStationId)`。

**Alternatives considered**：
- `/qa/[task_id]` page route（與 `/explorer/[task_id]` 對稱）— rejected：Q&A 是 contextual on top of station，URL 換成 `/qa/...` 會讓使用者「失去學習脈絡」（station 看不到了），違反 mockup 12 設計意圖。
- query param `?qa=qa_abc12345` 表達 drawer 開狀態 — rejected：無 URL 表達需求（session 短命、`kb_growth.jsonl` 已是稽核 trail），加 query 只會讓 page 切換時 race。

### Multi-turn 視覺骨架但每筆 question 是獨立 POST /qa

**Approach**：`turns: Ref<QaTurn[]>` 為陣列，每筆 user question 對應一個 `QaTurn`：

```ts
interface QaTurn {
  id: string                    // turn_<timestamp_ms>
  question: string
  originatingStationId: string | null
  taskId: string | null         // qa_<8hex>，pending 時 null
  ragHits: RagHit[] | null
  reactSteps: { step: number; thought?: string; actions: ActionEntry[] }[]
  kbGrowth: KbGrowthEvent[]
  answer: { text: string; citations: Citation[] } | null
  status: 'pending' | 'streaming' | 'done' | 'error'
  error?: { code: string; message: string }
}
```

每筆新 question `start()` 時：append 一個 `QaTurn` 到 `turns.value`（status `pending`）→ `POST /qa` → 收 task_id → `useSseTask(task_id)` → 5 種 event 各自 upsert 進當前 turn → `done` event 翻 status `'done'`。**前一 turn 必須 status='done' 或 'error' 才能送下一筆**（送 button disabled）。

**Alternatives considered**：
- 一個 session 內 backend 維持上下文（continue endpoint）— rejected：backend 沒這 endpoint，Phase 2 才有；P0 偽造 continuity 等於說謊。
- ONE turn per drawer instance（送下一題就清空前一題）— rejected：mockup 12 顯式有 `Turn 1` divider 暗示多 turn UX，且使用者問題 chain 是 demo 看點。

### `<QAEntry>` mdc 改 imperative 但保留 mdc 元件契約

**Approach**：`QAEntry.vue` 內 `handleClick` 從 `router.push('/qa?prompt=...')` 改 `useQaSession().start(props.prompt, inject<string | null>('currentStationId', null))`。Prop shape `{ prompt: string }` 不變、按鈕視覺不變、mdc auto-import contract 不變、`emit` 不變（既有 dumb + emit pattern 嚴格說沒有 emit，按鈕內呼 imperative function 算是「dumb + side effect」變體，仍符合 R-01 archive `Three mdc interactive components with strict prop contracts` 既有 Requirement「QAEntry MUST NOT itself fetch any sidecar endpoint; it is a navigation trigger only」—imperative 呼叫的 composable 才 fetch，元件本身仍只是 trigger）。

**Alternatives considered**：
- emit `'open-qa'` 給 caller、caller 自己去呼 composable — rejected：mdc auto-import 沒有 caller 介入空間（mdc 直接展開到 markdown 樹），用 imperative 是唯一可行路徑。
- 在 QAEntry 內直接 inline composable 邏輯 — rejected：違反 dumb pattern，會讓 mdc 元件依賴 SSE / Tauri stack。

### `currentStationId` 透過 page-level `provide` 注入而非 prop drill

**Approach**：R-01 station page `pages/tutorial/[workspace_id]/[station_id].vue` `setup()` 內 `provide('currentStationId', stationId.value)`；`QAEntry.vue` `inject<string | null>('currentStationId', null)`。MOC 首頁與 explorer 等其他 page 不 provide → inject 拿到 null → `start(prompt, null)`，backend `originating_station_id` 為 null（QARequest 允許）。

**Alternatives considered**：
- 在 QAEntry 上加 `:station-id` prop — rejected：mdc 元件 prop 由 markdown frontmatter / inline 寫入，station_id 是 routing context 不適合放 markdown。
- 讓 useQaSession 自己讀 `useRoute()` 解 station_id — rejected：composable 層耦合 routing 細節（R-01 path schema 是 `/tutorial/<wsid>/<stationid>`，未來改路徑 composable 跟著改），provide 把 routing 翻譯責任留在 page。

### `kb_growth` AuditPanel tab 採 dual-source merge：disk read + SSE live-tail

**Approach**：page `/explorer/[task_id].vue` 與其他 page 都用 `useAuditJsonl(ws_path, 'kb_growth')` 載 disk 既有 entries；同時 `useQaSession()` 內部 watch SSE `kb_growth` event 把新 entry append 進 audit list。dedup key 用 `entry_id`。Live-tail 機制與 `useAuditJsonl` 對 llm 那條同款（`liveTailFromExplorerStream` 換成 `liveTailFromQaSession`，行為 mirror）。

**Alternatives considered**：
- 純 disk re-read polling — rejected：理由同 `llm-call-inspector-p0` 同款決策，不重複。
- Q&A drawer 自帶 KB growth 區塊、不接 AuditPanel — rejected：drawer 內已有 `<QaTurnCard>` 的 KB growth event 段（per-turn），但 AuditPanel `kb_growth` tab 是**全 workspace 累積視圖**（含過去 session 與其他 task），兩者並存才完整。

### `useAuditJsonl` 擴充：第二個 live-tail kind

**Approach**：`useAuditJsonl(ws, kind, opts?)` 的 `opts` 既有 `liveTailFromExplorerStream?: UseExplorerStreamApi`；本 change 新增 `liveTailFromQaSession?: UseQaSessionApi`，當 `kind === 'kb_growth'` 且傳入時，watch QA SSE event 並 dedup append。**這是 `llm-call-inspector-p0` spec 的擴充而非 modify**——本 change 的 spec delta 是 ADDED Requirement「`useAuditJsonl` supports kb_growth live-tail from useQaSession」，不破既有 llm 那條 Requirement。

**Alternatives considered**：
- 為 `kb_growth` 寫獨立 `useKbGrowthAudit` composable — rejected：違反「audit JSONL 只有一個 reader pattern」原則，七層應該共用 `useAuditJsonl`。
- `useAuditJsonl` 內部直接 import useQaSession — rejected：會造成循環依賴（useQaSession 也想記 kb_growth 進 turn 內），且耦合過度；用 `opts` 注入是反向控制原則。

### Drawer width 固定 480px、不可拖曳

**Approach**：`<QAOverlay>` Tailwind class `w-[480px]`，無 resize handler；位置固定 `right-0 top-0 bottom-0`；底層 `<div class="absolute inset-0 bg-surface-0/60 backdrop-blur-sm" />` 半透明遮罩 + `@click="useQaSession().close()"` 點外關閉。

**Alternatives considered**：
- vue-resizable / 自寫 drag handle — rejected：超 P0 範圍；mockup 12 也是固定寬。
- 全屏 modal — rejected：違反「保留底層 station 脈絡」設計意圖。

### `Cmd+K` / `Ctrl+K` 全域召喚

**Approach**：`layouts/default.vue` `setup()` 內 `onMounted` 加 `window.addEventListener('keydown', handler)`、`onBeforeUnmount` removeListener；handler 偵測 `(e.metaKey || e.ctrlKey) && e.key === 'k'` → `e.preventDefault()` + `useQaSession().open(prompt='', stationId=inject('currentStationId') || null)`。已開狀態下再按 Cmd+K 不切換、不關（避免誤觸）；ESC 才關。

**Alternatives considered**：
- 用 vueuse `useMagicKeys` — rejected：本 change 不引新依賴；手寫 listener 約 15 行不複雜。
- Cmd+K toggle（已開再按關閉）— rejected：mockup 12 顯示 Cmd+K 是「召喚」單向動作；ESC 才是收起對位。

### 前一 turn 未 done 不允許送下一筆，靠前端 send button disabled 而非 backend race

**Approach**：composer send button `:disabled` 綁 `lastTurn?.status !== 'done' && lastTurn?.status !== 'error'`（pending / streaming 都 disabled）；同時送出時前端再檢查一次 status（避免 race），若 in-flight 就 abort 並 toast「請等前一題回覆完」。

**Alternatives considered**：
- 不做前端 gate、依賴 backend 409 TASK_IN_FLIGHT — rejected：UX 差（送出後才知失敗）；前端 gate 是 first line of defense。
- 中途允許 cancel 前一 turn 再送下一筆 — rejected：超 P0 範圍（cancel UI 屬步驟 29 介入點）。

## Risks / Trade-offs

- **[Risk] `useQaSession` module-level singleton 在 vitest 多測之間殘留 state**：vitest 預設 module 是隔離的（每測重新 import），但 caching 行為可能讓 test order 影響結果。Mitigation：每測 `beforeEach` 內 `useQaSession().close()` + `turns.value = []` reset；提供 `_resetForTest()` debug-only export。
- **[Risk] `<QAEntry>` 改 imperative 後若 R-01 archive 既有測試假設 `router.push` 行為會 fail**：archive `r-01-station-board` 的 mdc 元件契約測可能 mock `useRouter().push` 並 assert call。Mitigation：先 read archive 既有測 → 若有 router-push assertion 改 expect `useQaSession().start` mock 被呼叫；同時保留 `<QAEntry>` 的 button shape + click handler exists 兩條既有 scenario。
- **[Risk] `Cmd+K` listener 與其他 page 既有快捷鍵衝突**：目前 R-01 / explorer / audit page 都沒裝快捷鍵 listener，但未來可能加。Mitigation：listener 用 `{ capture: false }`，event 不 stopPropagation；未來頁面要 override 自己 preventDefault 即可。文件記在 `docs/qa-agent.md §八` 補充段。
- **[Risk] 多 turn 累積導致 drawer 滾動很長 + 記憶體佔用增加**：sessions 短命但好奇心使用者可能連問 20 題。Mitigation：`turns.value` 寫一個 cap 50 的 FIFO（超過自動 shift 老的）；mockup 12 沒考慮這點，文件補一個說明。
- **[Trade-off] Origin station chip 點擊 emit 給 caller 而非 drawer 自己 router-push**：保留架構彈性（caller 可決定走 vue-router 或關 drawer 才跳），但 caller wiring 多一行；drawer 直接 navigate 會更簡單但耦合 routing。選 emit 是**純 UI 元件、navigation 由 page 決策**這條 R-01 既有原則的延伸。
- **[Trade-off] `useAuditJsonl` 加第二個 live-tail option 把 composable 變 fat**：`opts.liveTailFromExplorerStream` + `opts.liveTailFromQaSession` 並存看起來像 OR-pattern；如果未來有第三、第四種 stream（Generator? Q&A multi-turn?）會更醜。接受 — Phase 2 若真演化成多 source 再 refactor 成 plugin pattern。

## Open Questions

- `lastTurn?.status === 'pending'` 時若 `useSseTask` 進入 `reconnecting` 狀態，UI 是否要顯示 reconnecting 提示？傾向 yes（在 send button 旁加小 spinner + 「重連中…」字），實作時定。
- `kb_growth` audit tab 的 row click 在本 P0 是 no-op；但 AuditPanel `select-row` emit 仍會 fire（本 page 不綁）。若使用者點到沒任何反應會困惑，是否要 visually disable row hover？傾向 no — 與其他 audit tab 行為一致，差異反而困惑；apply 階段視 demo 反應再調。
