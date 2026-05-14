# v3-app-workspace-goal 技術設計

## Context

C 是 v3-app roadmap 第三條 GUI change（foundation → A → B → chat 已 archive）。把 WorkspaceStub.tsx 「Coming soon」 替換成真內容：使用者開 vault 進入後能看到 Goals overview、新增 goal、即時看 stream、瀏覽 wiki。

完整 pre-discuss 結論見 `docs/2026-05-12-v3-app-workspace-goal-discussion.md`。本 design.md 補上 chat-verb 之後追加的「Goals 只顯 mode=goal」filter 決策、以及 layout 細節調整（Wiki 不三欄 / Quiz 留 placeholder / stream history collapse 砍掉 / 空狀態加 hint）。

依賴的既有 capability：

- **app-shell** (foundation)：Tauri shell、IPC Command Registry、AppError、AppState、Lobby、Settings、WorkspaceStub。本 change 把 stub 替換成 real workspace、IPC Registry 從 5 個 commands 擴成 11 個。
- **verb-library** (A archived)：`codebus_core::verb::goal::run_goal(repo, options, on_event, cancel)` library function — Tauri IPC `spawn_goal` 把 GUI 端的 closure 與 cancel signal 注進來 spawn background thread 跑 goal。
- **run-log** (B archived)：`<vault>/.codebus/log/runs-*.jsonl` 是 Goals overview 的資料源；`outcome` 欄位 + `mode` 欄位用來 filter goal-only + 標 cancelled。
- **events-log** (B archived)：`<vault>/.codebus/log/events-*.jsonl` 持久化 per-run stream events，是 Run detail timeline 的資料源；也用來偵測 interrupted run（events 有 / RunLog 無）。
- **agent-stream-rendering** (already shipped)：StreamEvent closed enum 透過 caller closure 消費；GUI 的 closure 把每個 event 透過 Tauri event bus emit 給 frontend。

## Goals / Non-Goals

### Goals

- 取代 WorkspaceStub 為完整 Workspace（sidebar 3 tabs + 主區依 tab 切換）
- Goals tab：runs list（filter `mode=goal`、含 interrupted 偵測）+ New Goal modal + 4 種 run detail view
- Wiki tab：file tree（collapsible panel）+ Milkdown 唯讀預覽 + 自訂 wikilink plugin
- Quiz tab：v1 顯示 "Coming soon" placeholder
- Tauri IPC 加 6 個 commands（spawn / cancel / list_runs / get_run_detail / list_wiki_pages / read_wiki_page）
- Stream events 透過 Tauri event channel `goal-stream` 即時推給 frontend
- Frontend state stores：`useGoalsStore` + `useWikiStore`
- Interrupted run 偵測在 workspace mount 時跑、virtual outcome 不寫 disk
- 一個時間最多一個 active goal run（v1 invariant）

### Non-Goals

- Chat 整合（Cmd+K overlay）— D `v3-app-chat-cmdk` scope
- Quiz 實作邏輯 — E `v3-app-quiz` scope
- Wiki edit / rename / delete in GUI — goal flow 是寫 wiki 唯一管道
- Reset button、Continue button — pre-discuss 已 ruled out
- Multi-vault switcher in workspace — 回 lobby 才切
- Commit history surface — Done detail 顯示 commit hash 即可
- 同時跑多個 goal — v1 enforce one active run
- Real-time collaborative editing
- 修改 `agent-stream-rendering` 既有 closed enum — GUI 純 consumer
- 修改 `events-log` / `run-log` schema — GUI 純 reader

## Decisions

### Goals tab 只顯示 `mode=goal` row

chat-verb ship 後 runs-*.jsonl 內可能有 4 種 `mode`：goal / query / fix / chat。Goals tab 嚴格 filter `mode=goal`，理由：

- "Goals" tab 名字對應 user mental model「寫 wiki 的進度」，goal 是唯一改 wiki 的 verb
- chat / query / fix 各自有自己的 surface：chat 是 D Cmd+K overlay 範圍、query 與 fix 是 CLI-only v1
- 不混顯避免 user 困惑「為什麼我剛剛的 chat session 在這裡」

考慮過：(a) 顯示全部、icon 區分 mode；(b) 只顯示 goal + fix（兩者都改 wiki）。否決 (a) — 違反 Goals tab 命名語意；否決 (b) — fix 是維護動作非建設動作，CLI 已夠用、不需要 GUI 推進。

### IPC 6 個 commands、Tauri event 1 個 channel

Rust 端 `src-tauri/src/ipc/goals.rs` + `src-tauri/src/ipc/wiki.rs` 分檔。

| Command | 簽名 | 用途 |
|---|---|---|
| `spawn_goal` | `(vault_path: String, goal_text: String) -> Result<RunId, AppError>` | 起 std::thread 跑 `run_goal`，hold cancel Arc 進 AppState.active_runs map（key: RunId），每個 StreamEvent / VerbEvent emit Tauri event `goal-stream` payload `{ run_id, event }`，thread 結束時 active_runs remove |
| `cancel_goal` | `(run_id: RunId) -> Result<(), AppError>` | active_runs lookup → cancel flag store(true)，best-effort（thread 自會觀察 + kill child） |
| `list_runs` | `(vault_path: String, mode_filter: ModeFilter) -> Result<Vec<RunLogSummary>, AppError>` | 開啟 vault 內最新 N 個 runs-*.jsonl 讀回 RunLog 行，反向時間排序；同時掃 events-*.jsonl 偵測 interrupted（events file 有 / 對應 RunLog row 無 → virtual entry outcome=interrupted）；ModeFilter 接 `Goal` / `All` |
| `get_run_detail` | `(vault_path: String, run_id: RunId) -> Result<RunDetail, AppError>` | RunLog row + 對應 events-*.jsonl tail-replay 成 Vec<RecordedEvent> |
| `list_wiki_pages` | `(vault_path: String) -> Result<Vec<WikiPageMeta>, AppError>` | Glob vault `wiki/**/*.md`、parse 每個 file 的 frontmatter `title`、回 `{slug, path, title}` |
| `read_wiki_page` | `(vault_path: String, page_slug: String) -> Result<String, AppError>` | 讀對應 file body（去 frontmatter）回 raw markdown |

Event channel：`goal-stream` 一個 channel 就夠，payload 用 `run_id` 區分屬於哪個 run（v1 雖然一個時間只一個 active，但設計成多 run 並存友好以便 v2 擴展）。

考慮過：(a) 每個 RunId 一個 event channel；(b) frontend 用 polling 拉 active stream。否決 (a) — 動態 channel 名要登記管理、複雜；否決 (b) — Tauri event bus 設計就是 push-based，polling 是退化。

### RunId 用 timestamp slug，不引入 UUID

RunId 等於 RunLog row 的 `started_at` 欄位的 jsonl-filename slug（如 `2026-05-13T14-56-21Z`）。Rationale：

- B `v3-run-log-events` 已用 `started_at` 派生 events-*.jsonl 檔名 slug
- frontend 不需要自己生 ID — spawn_goal 回傳 IPC 端從 InvokeReport.started_at 派生
- 跨 process 唯一性：vault-scoped + 秒級 timestamp + Rust spawn 是 single-threaded 進程內、撞 ID 機率 0

考慮過：UUID v4。否決：多餘抽象、會在 RunLog 加新欄位（違反「不改 schema」non-goal）。

### Active runs 狀態存 `AppState.active_runs: Mutex<HashMap<RunId, Arc<AtomicBool>>>`

Spawn 時 insert、cancel 時 lookup、thread 結束時 remove。Lock granularity 整個 map 一個 Mutex（HashMap operation 都是微秒級，contention 極低）。

考慮過：DashMap（lock-free concurrent map）。否決：依賴成本不值；v1 一個時間 ≤ 1 active run，contention 0。

### Frontend 用 2 個 Zustand store + 既有 Vault scope

`useGoalsStore`：

```ts
{
  runs: RunLogSummary[]      // list view 用
  activeRun: ActiveRunState | null  // running detail 用，含 streamed events buffer
  spawnGoal(text): Promise<RunId>
  cancelGoal(runId): Promise<void>
  refreshRuns(): Promise<void>
  // event handler 訂閱 Tauri 'goal-stream'、push 到 activeRun.events
}
```

`useWikiStore`：

```ts
{
  pages: WikiPageIndex       // {[slug]: {path, title}}
  currentPath: string | null  // 目前 Milkdown 預覽的 page slug
  body: string | null         // currentPath 對應 markdown body cache
  loadPage(slug): Promise<void>
  listPages(): Promise<void>
}
```

兩個 store 各自 own 對應 IPC subset；既有 `useRouteStore` 只改 RouteState union（`workspace-stub` → `workspace`），不擴功能。

Workspace mount 時：

1. `useGoalsStore.refreshRuns(vault.path)` 拉 runs list
2. `useWikiStore.listPages(vault.path)` 拉 wiki page index
3. 訂閱 Tauri event channel `goal-stream`、handler 累積到 activeRun.events
4. Unmount 時 unsubscribe + clear stores

### Wiki tab layout — file tree 是 collapsible panel 不是常駐 column

Wiki tab 主區一個 component `WikiTab.tsx` 內：

```
┌──────────────────────────────────────────────────────┐
│ [📁 Pages]                                  ▼ index   │   ← top bar, file tree toggle button
├────────┬─────────────────────────────────────────────┤
│        │                                              │
│ uv-lib │  # uv-lib                                   │
│ uv-...│                                              │
│ cache  │  Library entry point of the `uv` binary..  │
│ ...    │                                              │
│        │  See [[uv-child]] for sub-process signal... │
└────────┴─────────────────────────────────────────────┘
   ↑ collapsible (icon click 切顯隱)
```

預設 file tree 展開 ~180px 寬、Milkdown body 佔剩餘寬度。Pages icon 按一下可隱藏 file tree（再按展開）。理由：user 進 Wiki tab 多半是要 browse 頁面、不是只讀一頁；展開預設讓使用者立刻看到 vault 有哪些 wiki page（apply 階段 user feedback 調整 2026-05-14）。

### Milkdown 唯讀 + 自訂 wikilink plugin

依賴：`@milkdown/core` + `@milkdown/preset-commonmark`（最小 set，不引入 GFM extra plugins 直到有需要）。Editor 初始化 `editable: () => false`。

`[[wikilink]]` plugin：自訂 ProseMirror node + paste rule。

- Render：lookup `useWikiStore.pages[slug]`，存在 → 顯示 link style + onClick → `useWikiStore.loadPage(slug)`；不存在 → 顯示 disabled style（dimmed + hover 顯示 "Page not found"）
- Wikilink 文字格式：spec 與 fix verb 都用 `[[slug]]`，slug 是 filename minus extension（例 `[[uv-lib]]` → `wiki/modules/uv-lib.md`），跨 folder lookup by filename（既有 vault 慣例）

考慮過：(a) `react-markdown` + 自訂 renderer；(b) `tiptap`。選 Milkdown 理由：pre-discuss doc 已決定 + 第三方 wikilink plugin 生態完整 + 跟 ProseMirror 同 ast 較成熟。

### Interrupted run 偵測 — events 有 / RunLog 無

`list_runs` 內邏輯：

1. Scan `<vault>/.codebus/log/runs-*.jsonl` 反序讀回所有 RunLog rows（按 started_at 反向排序）
2. Scan `<vault>/.codebus/log/events-*.jsonl` 列出所有檔的 started_at slug set
3. 對 events slug set 中**沒對應 RunLog row** 的 → 構造 virtual `RunLogSummary { outcome: "interrupted", started_at: <from-slug>, goal: <from-first-user-event-in-events.jsonl>, ... }`
4. Virtual + real merge、按 started_at 反向排序回給 frontend

`outcome="interrupted"` 是 GUI-side virtual value，不寫進 jsonl（保持 `run-log` schema 不擴）。RunDetail 對 interrupted 跑同樣的 events.jsonl tail-replay、frontend render `RunDetailInterrupted` component。

### Stream rendering 在 frontend、不在 Rust

`run_goal` 的 `on_event` closure 在 IPC 端是 `move |event: VerbEvent| { app_handle.emit_to(...) }`，不做任何 render / format。VerbEvent enum serde-serialize 直接送 frontend。Frontend `useGoalsStore` 訂閱 channel、push event 到 buffer，components 用 selector 派生 view-model（tool_use 一行 summary / thinking 段落 buffered / banner 標題切換）。

考慮過：Rust 端 pre-format 成 string 推 frontend（CLI render style）。否決：(a) CLI 跟 GUI render 風格不同（CLI 用 emoji line、GUI 用 React component）、(b) Rust 字串化後 frontend 還要 parse 不對稱、(c) i18n 在 frontend 更靈活。

### Forbidden Behaviors 表 — minor 措辭更新、不擴 forbidden 範圍

`app-shell` 內 `Forbidden Behaviors in v1` 段：

- 「Quest banner...in the Lobby or Workspace stub」→ 「in the Lobby or Workspace」（刪 stub 字、scope 不變）
- 加新 forbidden：「Same-window multi-active-goal — at most one goal run SHALL be active at any time」（v1 explicit）

其他 forbidden 條目（theme toggle / language switcher / Recent Pages panel / Graph view / Chat-mode Cmd+K precursor / direct LLM API）全部沿用。

## Implementation Contract

### Observable behaviors

- Vault 從 lobby 開後，1s 內 Workspace 顯示，預設選 Goals tab；Goals overview 列表顯示 runs（包括 interrupted virtual entry）
- 點 [+ New goal] → modal 開、focus 在 textarea；輸入 goal text 後點 [Run] → modal 關 + Goals overview 多出 running row + 切到 Running detail view
- Running detail view 即時更新 activity stream（每個 tool_use ≤ 100ms render latency）
- 點 [⏹ Cancel] → 500ms 內 row 變 ⏹ Cancelled、view 切 Cancelled detail
- Goal 完成（agent natural exit + lint+fix + auto-commit）→ row 變 ✓ Done、view 切 Done detail；如同時 user 已離開 detail view，row 在 Goals overview 仍會更新狀態
- 點 [Retry with same goal] → pre-fill New Goal modal 並開啟（user 仍要點 [Run] 才 actual spawn）
- Wiki tab 切 → 顯示 currentPath 的 markdown body；點 `[[wikilink]]` → loadPage 切；點 disabled wikilink → 無事發生（hover tooltip "Page not found"）
- Quiz tab 切 → 顯示 centered placeholder「Coming soon — quiz flow ships in v3-app-quiz」
- Workspace ← back to Lobby → useRouteStore 切到 lobby、unmount Workspace；active run 持續在 background（goal flow 不 cancel），下次再進 vault 可看到 running row

### Library / IPC surface

新加 Rust：

- `src-tauri/src/ipc/goals.rs`：
  - `pub async fn spawn_goal(state, vault_path, goal_text) -> Result<String, AppError>`（RunId 是 String）
  - `pub async fn cancel_goal(state, run_id) -> Result<(), AppError>`
  - `pub async fn list_runs(state, vault_path, mode_filter) -> Result<Vec<RunLogSummary>, AppError>`
  - `pub async fn get_run_detail(state, vault_path, run_id) -> Result<RunDetail, AppError>`
  - structs: `RunLogSummary { run_id, mode, goal, started_at, finished_at, tokens, outcome, wiki_changed, lint_error_count, lint_warn_count }`、`RunDetail { summary, events: Vec<RecordedEvent> }`、`ModeFilter::Goal | ::All`
- `src-tauri/src/ipc/wiki.rs`：
  - `pub async fn list_wiki_pages(vault_path) -> Result<Vec<WikiPageMeta>, AppError>`
  - `pub async fn read_wiki_page(vault_path, page_slug) -> Result<String, AppError>`
  - structs: `WikiPageMeta { slug, path, title }`
- `src-tauri/src/state/active_runs.rs`：`ActiveRuns(Mutex<HashMap<String, Arc<AtomicBool>>>)`，由 `AppState` own

新加 React：

- `src/store/goals.ts`：Zustand store 定義（runs / activeRun / actions），訂閱 Tauri 'goal-stream' event 在 store 初始化時
- `src/store/wiki.ts`：Zustand store 定義（pages / currentPath / body / actions）
- `src/components/workspace/Workspace.tsx`：top-level workspace shell（sidebar + 主區），mount/unmount lifecycle
- `src/components/workspace/{GoalsTab,WikiTab,QuizTab}.tsx`：tab 主區 component
- `src/components/workspace/{NewGoalModal,RunListItem,RunDetailRunning,RunDetailDone,RunDetailCancelled,RunDetailInterrupted}.tsx`：Goals tab 子元件
- `src/components/workspace/{WikiTree,WikiPreview,ActivityStreamItem}.tsx`：Wiki tab + RunDetail 子元件
- `src/lib/milkdown-wikilink.ts`：自訂 ProseMirror node + paste rule

修改：

- `src/App.tsx`：route switch 從 `workspace-stub` 改 `workspace`
- `src/store/route.ts`：RouteState union 改 `kind: "workspace"`
- `src/lib/ipc.ts`：加 6 個 IPC wrapper function
- `src/i18n/messages.ts`：加 workspace + tab labels + run detail strings 共約 30 條 key
- `src-tauri/src/state/app_state.rs`：加 `pub active_runs: ActiveRuns` field
- `src-tauri/src/ipc/mod.rs`：register 6 個 new commands、設定 Tauri event emit binding
- `src-tauri/src/lib.rs`：mount IPC module + state init

刪除：

- `src/components/workspace/WorkspaceStub.tsx` 與 `.test.tsx`

### Acceptance criteria

- `pnpm typecheck` + `pnpm test`（既有 Vitest suite）通過、`cargo build -p codebus-app-tauri` 通過
- Vitest 新加 component-level tests cover：Workspace mount routing、NewGoalModal 開關、RunListItem outcome icon 切換、WikiPreview render with wikilinks
- Rust IPC tests cover：spawn_goal happy path、cancel_goal flips flag、list_runs sort + interrupted virtual entry、list_wiki_pages frontmatter parse、ModeFilter::Goal 排除非 goal rows
- Tauri dev mode手動 happy path：開 lobby → click vault card → workspace 載入 → click + New goal → 輸 "describe X module" → Run → 看 activity stream → goal done → Wiki tab 點新 page → 看到剛產生的 wiki
- 既有 27+ goal_flow / cli_routing / vault_init / scoped_env_injection / chat_flow / chat_cancel tests 全綠不破（cargo test --workspace 0 failed）

### Scope boundaries

In scope:

- 取代 WorkspaceStub 為 Workspace.tsx 真內容（3 tabs + Goals overview + Goal flow + Wiki preview + Quiz placeholder）
- 6 個新 Tauri IPC commands + `goal-stream` event channel
- 2 個新 Zustand store
- Milkdown read-only + wikilink plugin
- Interrupted run virtual outcome at workspace mount
- ActiveRuns state map in Rust
- v1 invariant: one active goal run at a time

Out of scope:

- Chat 整合（Cmd+K overlay）
- Quiz 實作邏輯
- Wiki edit / rename / delete / image upload
- Reset / Continue buttons
- Multi-vault switcher
- Commit history UI
- Multi-active-goal
- Modifying agent-stream-rendering / events-log / run-log existing requirements

## Risks / Trade-offs

- [Milkdown + 自訂 ProseMirror node 學習曲線] → Mitigation: 留 spike 時間在 propose 內保留 1 task 「驗證 wikilink plugin demo 跑得起來」；如 Milkdown integration 卡住、fallback 用 `react-markdown` + 自訂 `[[...]]` regex renderer（spec 不寫死 Milkdown）
- [v1 一個時間 1 active run 的限制] → user 點 New Goal 時若已有 active run、modal 顯示 disabled state + 提示「Wait for current run to finish or cancel it」。Spec scenario 寫進去
- [Tauri event channel 一個 channel 多 run payload — v2 多 active run 時 frontend 需 demux by run_id] → v1 一個 active run 就足夠；future change 加 channel-per-run 是 forward-compat 不破 v1 行為
- [events-*.jsonl tail-replay 在 large run（10k+ events）讀檔慢] → Mitigation: `get_run_detail` 一次性 read + parse、不串流 replay；real-world v1 預期單 run < 200 events、檔案 < 500KB，single-shot read 可接受
- [Interrupted run 偵測 false-positive — events 寫到一半 disk 掛掉 / system crash] → 與真 interrupted 同樣處理（virtual outcome=interrupted + [Retry]），user 體感無差別
- [Wiki page slug 衝突 — 不同 folder 同 filename] → 既有 vault 規範 slug 唯一，schema lint rule 強制（`broken-wikilink-related` 等規則）。若仍衝突，`useWikiStore.pages[slug]` 後寫覆蓋前者 — flag 為 spec scenario「duplicate slug behavior」
- [Long-running goal flow 在 Workspace ← back 後仍跑] → v1 維持，未來如需要「leave kill」可加 setting；目前不擴
- [Background thread panic 沒被捕] → spawn_goal 用 `std::thread::Builder::spawn` + 在 thread 內 catch_unwind 包 run_goal，panic 後 emit goal-stream event `panic` payload + 從 active_runs remove；frontend 收到後切到 cancelled detail + warning

## Migration Plan

純擴展、無 breaking change：

1. WorkspaceStub.tsx 刪除、route store 從 `workspace-stub` 改 `workspace`（一次性 PR-level swap）
2. 新 Workspace.tsx mount 邏輯處理「沒 runs / 沒 wiki pages」空狀態 — 對 fresh vault 直接顯示 hint，無 migration
3. IPC Command Registry 從 5 擴 11 — 純加 commands，既有 5 個 commands 行為不變
4. AppState 加 active_runs field — 用 `Default::default()`，既有 AppState 構造路徑 unchanged
5. 既有 vault 內 runs-*.jsonl + events-*.jsonl 都被 `list_runs` / `get_run_detail` 識別讀回；無資料遷移

無 rollback step — 純加 capability、純擴 IPC。

## Open Questions

- Workspace 從 lobby 進去時，要 transition 動畫嗎？（v1 預設無動畫、直接 swap，UX polish 後續 change 處理）
- New Goal modal 是否需要 goal text autocomplete from history？（v1 沒有，[Retry with same goal] 已 cover 主要 use case）
- Wiki tab file tree 收摺狀態要 persist 到 localStorage 嗎？（v1 預設每次進 vault 重置為收起；persist 是後續 polish）
- 多語系（zh-tw / en）messages.ts 翻譯誰來校對？（沿用既有 i18n 結構、機器翻譯 + 手動校 critical strings；user-facing 文件動手前先討論的 memory note 適用，apply 階段校時對齊）
