## Why

v3-app-foundation 已 ship Lobby + Settings + WorkspaceStub（顯示「Workspace coming in v3-app-workspace-goal」），開 vault 後使用者只看到「🚏 coming soon」沒實際內容。同時 CLI 端 A `v3-goal-library` 把 spawn verb orchestration 抽進 `codebus_core::verb::*`、B `v3-run-log-events` 把 stream events 與 outcome 持久化進 events.jsonl + RunLog，這兩條 prerequisite 都是為了讓 GUI 能以 reuse / read 的方式接上來。本 change 把 Workspace 從 stub 升級成真內容：使用者進 vault 後能新增 goal、看 goal stream 即時進度、瀏覽既有 wiki，並在 run 結束 / 取消 / 中斷時看到對應 detail view。

完整脈絡見 `docs/2026-05-12-v3-app-workspace-goal-discussion.md`（pre-discuss 結論）以及 `docs/v3-app-roadmap.md` §Sequence row C。

## What Changes

新加 GUI Workspace 真內容（取代 WorkspaceStub）：

- **左 sidebar 三個 tab**：Goals（預設選擇）/ Wiki / Quiz。Quiz tab v1 顯示 "Coming soon" placeholder（user override 不隱藏；E `v3-app-quiz` 接續才 ship 真內容）
- **Goals tab**：
  - Goals overview run 列表，只顯示 RunLog rows 中 `mode == "goal"` 的 entry（`chat` / `query` / `fix` 不顯示在這、留給 D `v3-app-chat-cmdk` 與 CLI surface）
  - 每 row 顯示 outcome icon（⚪ running / ✓ done / ⏹ cancelled / ⚠ interrupted）+ goal text + relative timestamp
  - 列表空狀態顯示 centered hint「Click + New goal to ask codebus to ingest something into the wiki」+ 3 個 pre-fill 範例 goal（點下去 pre-fill modal）
  - 右上 [+ New goal] 按鈕 → 開 New Goal modal（goal text input + Cancel / Run 兩個按鈕）
  - 點 row 進對應 detail view 四種：
    - **Running detail**：elapsed time + token count + 即時 activity stream（tool_use 一行 summary、assistant thinking 段落 buffered）+ [⏹ Cancel] 按鈕
    - **Done detail**：duration + tokens + commit sha + covered pages（wikilinks 可點進 Wiki tab）+ lint stats
    - **Cancelled detail**：warning「Wiki has uncommitted changes — not auto-committed. Review in terminal if needed.」+ partial timeline + [Retry with same goal] 按鈕
    - **Interrupted detail**：同 Cancelled 樣式但 header 換 ⚠ Interrupted + 文案「App was closed before this goal finished」+ [Retry with same goal]
- **Wiki tab**：
  - 左側 file tree（collapsible panel，icon button 切顯隱），列出 vault `wiki/**/*.md` 的 slug / title
  - 右側 Milkdown 唯讀預覽，渲染目前選擇 page 的 markdown body
  - `[[wikilink]]` 自訂 Milkdown plugin：點下去前端 lookup page index → 切換 currentPath；找不到 target 顯示 disabled style
  - 進場時 workspace mount 一次性拉全 vault page index 進 `useWikiStore`，後續 wikilink navigate 不打 IPC
- **Quiz tab**：v1 顯示 placeholder 「Coming soon — quiz flow ships in v3-app-quiz」
- **Stream history collapse 砍掉**：detail view 底部不再放 `─── stream history (collapse ▼) ────`；想看 raw stream-json 用 terminal 跑 tail on events-*.jsonl

新加 Tauri IPC commands：

- `spawn_goal(vault_path, goal_text) -> Result<RunId, AppError>` — 起 background thread 跑 `codebus_core::verb::goal::run_goal`，持有 cancel `Arc<AtomicBool>` 於 `AppState`，Tauri event emit `goal-stream` 帶 `{ run_id, event: VerbEvent }` payload
- `cancel_goal(run_id) -> Result<(), AppError>` — 從 `AppState` 查 cancel flag、flip true
- `list_runs(vault_path, mode_filter: "goal" | "all") -> Result<Vec<RunLogSummary>, AppError>` — scan vault 內 runs-*.jsonl，反向時間排序，含 interrupted 偵測（events 有但 RunLog 無）
- `get_run_detail(vault_path, run_id) -> Result<RunDetail, AppError>` — 對應 RunLog row + events-*.jsonl tail-replay
- `list_wiki_pages(vault_path) -> Result<Vec<WikiPageMeta>, AppError>` — 一次性拉全 wiki page index 給 store cache
- `read_wiki_page(vault_path, page_slug) -> Result<String, AppError>` — 拉 page raw markdown body 給 Milkdown render

新加前端 state stores：

- `useGoalsStore`（Zustand）：`runs: RunLogSummary[]`、`activeRun: ActiveRunState | null`、`spawnGoal(text)`、`cancelGoal(runId)`、`refreshRuns()`
- `useWikiStore`（Zustand）：`pages: WikiPageIndex`、`currentPath: string | null`、`loadPage(slug)`、`listPages()`

修改 `useRouteStore` 把 `workspace-stub` 改成 `workspace`，刪除 WorkspaceStub.tsx，新 `Workspace.tsx` 接管 Workspace 路由。

Interrupted run 偵測：app workspace mount 時呼叫 `list_runs`，IPC 端對 events-*.jsonl 檢查對應 RunLog row 是否存在於 runs-*.jsonl；events 有 / RunLog 無 → 標 virtual `outcome="interrupted"`（不寫回 disk）。

## Non-Goals

- Chat 整合 / Cmd+K overlay — D `v3-app-chat-cmdk` 的 scope，本 change 不碰
- Quiz 實作 — E `v3-app-quiz` 的 scope，本 change 留 placeholder
- Wiki edit / rename / delete in GUI — 寫 wiki 走 goal flow 是唯一管道
- Reset button（destructive git op）— design doc 已 explicit ruled out
- Continue button — agent context 沒持久化、claude CLI 沒 resume API；continue 技術上等於 retry，不需要兩顆
- Goal text retry 之外的 history edit / re-prompt — v1 只支援 pre-fill modal
- Multi-vault switcher in workspace — back to lobby 才能切
- Commit history surface inside GUI — auto-commit hash 顯示在 Done detail 即可，git log 用 terminal 看
- Inline wiki edit 或 image upload — Milkdown 設 read-only mode
- 同時跑多個 goal — v1 一個時間最多一個 active run（goal-cmdk 系統規範亦同）
- Real-time collaborative editing
- Telemetry / analytics — 維持 app-shell Forbidden Behaviors 約束
- 修改 `agent-stream-rendering` 既有 closed enum — GUI 只是另一個 caller closure，consume 既有 StreamEvent variants
- 修改 `events-log` write-side schema — GUI 純 reader、不改 jsonl 格式
- 修改 `run-log` schema — GUI 純 reader、不加 RunLog 欄位

## Capabilities

### New Capabilities

- `app-workspace`: GUI Workspace 真內容、Goals overview / Goal flow / Wiki preview / Quiz placeholder、Tauri IPC commands for goal lifecycle + wiki read、Zustand stores for goals / wiki state、interrupted run detection at workspace mount

### Modified Capabilities

- `app-shell`: Workspace Stub Transition requirement 從 stub display 改成完整 Workspace mount；IPC Command Registry 從 5 commands 擴成 11（加 spawn_goal / cancel_goal / list_runs / get_run_detail / list_wiki_pages / read_wiki_page）；Forbidden Behaviors 表內「Workspace stub」措辭更新為「Workspace」，並把「Workspace stub coming-soon page」從 forbidden 移除（本 change 取代之）

## Impact

- Affected specs: `app-workspace` (new), `app-shell` (modified)
- Affected code:
  - New:
    - codebus-app/src/components/workspace/Workspace.tsx
    - codebus-app/src/components/workspace/GoalsTab.tsx
    - codebus-app/src/components/workspace/WikiTab.tsx
    - codebus-app/src/components/workspace/QuizTab.tsx
    - codebus-app/src/components/workspace/NewGoalModal.tsx
    - codebus-app/src/components/workspace/RunListItem.tsx
    - codebus-app/src/components/workspace/RunDetailRunning.tsx
    - codebus-app/src/components/workspace/RunDetailDone.tsx
    - codebus-app/src/components/workspace/RunDetailCancelled.tsx
    - codebus-app/src/components/workspace/RunDetailInterrupted.tsx
    - codebus-app/src/components/workspace/WikiTree.tsx
    - codebus-app/src/components/workspace/WikiPreview.tsx
    - codebus-app/src/components/workspace/ActivityStreamItem.tsx
    - codebus-app/src/store/goals.ts
    - codebus-app/src/store/wiki.ts
    - codebus-app/src/lib/milkdown-wikilink.ts
    - codebus-app/src-tauri/src/ipc/goals.rs
    - codebus-app/src-tauri/src/ipc/wiki.rs
    - codebus-app/src-tauri/src/state/active_runs.rs
  - Modified:
    - codebus-app/src/App.tsx
    - codebus-app/src/store/route.ts
    - codebus-app/src-tauri/src/ipc/mod.rs
    - codebus-app/src-tauri/src/state/app_state.rs
    - codebus-app/src-tauri/src/state/mod.rs
    - codebus-app/src-tauri/src/lib.rs
    - codebus-app/src/lib/ipc.ts
    - codebus-app/src/i18n/messages.ts
    - codebus-app/package.json
  - Removed:
    - codebus-app/src/components/workspace/WorkspaceStub.tsx
    - codebus-app/src/components/workspace/WorkspaceStub.test.tsx
