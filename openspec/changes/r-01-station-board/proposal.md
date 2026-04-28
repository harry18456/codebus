## Why

Phase 6 步驟 25.5 共用骨架（`phase6-shell`，2026-04-27 archive）與步驟 26.5 授權層（`auth-flow`，2026-04-27 archive）已通電，但 grant 後使用者真正會停下來看的「**站牌學習頁**」（R-01）目前只是 placeholder route。Module 5 Generator P0（`module-5-generator-p0`，2026-04-25 archive）已產出 `<ws>/codebus-tutorials/{task_id}/tutorial.md` MOC + `stations/s{NN}-slug.md` 多檔教材 + `route.json`，但前端**沒有任何 component 在解析它**——教材寫在硬碟上沒人讀，Trust Layer 敘事斷在第二幕第一場。

關聯 ADR：D-029（多檔輸出 + stable station id）/ D-016（Q&A Agent 入口 `<QAEntry>`）/ D-026（npm + Nuxt 4 toolchain）。對應 `docs/implementation-plan.md §六` 步驟 26 + 27（合併 4d）、`docs/interactive-tutorial.md §六 + §九 P0`、`design/v1/10-tutorial-notion.html` + `design/v1/11-tutorial-slideshow.html` 兩個 mockup。

## What Changes

P0 範圍（~4d，對齊 `docs/interactive-tutorial.md §九 P0` 七條）：

- 新建 `openspec/specs/interactive-tutorial/` capability，6 個 ADDED Requirements：
  - 站牌路由 schema（`/tutorial/{workspace_id}/{station_id}` 與 `/tutorial/{workspace_id}` MOC，bare path 無 `/index` segment）以 stable station id 為 URL key（禁用 numeric index）
  - `<Checkpoint>` / `<Quiz>` / `<QAEntry>` 三個 mdc 互動元件契約（props 簽名 / 狀態 key / 互動行為）
  - `progress.json` schema 與唯一寫入路徑（`current_station_id` / `completed_station_ids` / `checkpoints` / `quizzes`，key 一律 stable station id；唯一 writer 是 `useTutorialProgress`，禁直寫 file system）
  - 解鎖邏輯（一站完成 = Checkpoint 全勾 + 所有 `<Quiz>` 答對 → 解鎖 `route.json` 順序的下一站；已完成站可回看不被解鎖邏輯擋）
  - frontmatter parser → `<StationLayout>` 外殼（標題 / 時長 / degraded badge / station_index 進度條）
  - MOC 渲染（`tutorial.md` 作為「目錄首頁」，每條站名 link 走 station_id 路由）
- 新前端依賴：`@nuxtjs/mdc`（npm，Nuxt module）
- 新前端 components 共 7 檔：
  - `web/app/components/content/Checkpoint.vue` / `Quiz.vue` / `QAEntry.vue`（mdc 自動掛載到 markdown 渲染管線）
  - `web/app/components/tutorial/StationLayout.vue`（外殼）/ `StationNav.vue`（左側站牌列表，渲染 `route.json.stations`）/ `StationContent.vue`（中間，mdc 渲染 + `###` 次級切頁）/ `MOCIndex.vue`（MOC 首頁）
- 新前端 composables 共 2 檔：
  - `web/app/composables/useTutorialProgress.ts`（`progress.json` 讀寫單一寫入路徑 + 解鎖判定 + 跨站 checkpoint / quiz 狀態）
  - `web/app/composables/useStationRoute.ts`（`route.json` 載入 + station_id ↔ file_path 解析）
- 新前端 pages 共 2 檔：
  - `web/app/pages/tutorial/[workspace_id]/index.vue`（MOC 首頁）
  - `web/app/pages/tutorial/[workspace_id]/[station_id].vue`（站牌頁）
- 新 Tauri command `read_tutorial_file(workspace_root, relative_path)` 與 `write_progress_file(workspace_root, task_id, payload)`（落 `tauri/src-tauri/src/tutorial.rs`，Rust 端 path validation 限 `codebus-tutorials/` 子樹 + 副檔名 allowlist `.md` / `.json` + 紅隊測對齊 sidecar `ensure_in_workspace`）。**不**走 sidecar HTTP——教材檔案讀寫是 process 邊界 IPC，sidecar 職責是 LLM / KB / Agent，引入新 endpoint 違反 capability 邊界（詳見 design D-T1 / D-T4）
- 新前端 composable `web/app/composables/useTutorialFiles.ts`（封裝兩個 Tauri command IPC，附 path safety 防呆）
- 修改 `web/app/pages/workspace/grant.vue`：grant 成功後 `router.push('/workspace/scan')` 改為 `/tutorial/{workspace_id}`（bare path 無 `/index` segment），用 `GrantResponse.workspace_id` 直接帶入路由

## Non-Goals

P1（留給後續 change）：

- 文件模式（document mode）切換 — P0 只做投影片模式
- `<CodeRef file="..." lines="...">` side panel 檔案瀏覽
- `<Reveal>` 漸進揭露元件
- `<AgentThought step="N">` 內嵌 Agent 決策展示（屬步驟 28 Agent console 範圍）
- 站牌跨檔搜尋（純 client-side scan ≤ 500 chunks）
- 教材匯出 PDF / slide deck

明確不做（避免 scope 膨脹）：

- LLM 判題（使用者打字回答 → Agent 判對錯 + 回饋）— Phase 3，純 frontend 比對 `correct` 屬性已夠 MVP
- 多選題、填空題、拖拉題 — 單選 + Checkbox 已覆蓋 demo 需求
- 筆記功能（教材旁寫註解）
- 教材即時編輯（read-only，重產走 Module 5 regenerate flow）
- 多語系切換（教材語言由 Generator 在 generation time 決定）
- progress.json 雲端同步（local-first；progress.json 寫在 `<ws>/codebus-tutorials/{task_id}/progress.json`）

範圍判決（拒絕）：

- 拒絕 R-01 與步驟 28 Agent console 合一個 change：兩個 page-level 區域分屬兩條敘事線（學習主路 vs 透明度武器），合併會讓本 change 工期翻倍且職責不清
- 拒絕用 Pinia store：`progress.json` 是檔案級 source of truth，composable + reactive 已夠；多餘 store 層只增加同步成本
- 拒絕跳過 audit unlock pattern 直接 `fetch('file://...')`：違反「sidecar 是檔案 read 唯一閘門」不變式

## Capabilities

### New Capabilities

- `interactive-tutorial`: R-01 站牌學習頁不變式集合 — 站牌路由 schema（stable id 為 URL key、MOC 路由獨立）/ 三個 mdc 互動元件契約（`<Checkpoint>` / `<Quiz>` / `<QAEntry>` props + 狀態 key + 互動行為）/ `progress.json` schema 與唯一寫入路徑 / 解鎖邏輯（Checkpoint 全勾 + Quiz 答對 → 下一站，已完成站可回看）/ frontmatter parser → StationLayout 外殼 / MOC 渲染。**只規範不變式與 schema，不規範視覺細節**（mockup 以 `design/v1/10-tutorial-notion.html` + `design/v1/11-tutorial-slideshow.html` 為 source of truth）。

### Modified Capabilities

(none)

> 說明：既有 frontend shell 共用骨架的不變式已涵蓋 topbar / audit panel / useSidecar 等基礎；本 change 走純 page 級擴張，不動既有 capability 的 requirement spec-level 行為。

## Impact

- Affected specs: 1 NEW（`interactive-tutorial`，6 ADDED Requirements）+ 0 MODIFIED
- Affected code:
  - New:
    - web/app/components/content/Checkpoint.vue
    - web/app/components/content/Quiz.vue
    - web/app/components/content/QAEntry.vue
    - web/app/components/tutorial/StationLayout.vue
    - web/app/components/tutorial/StationNav.vue
    - web/app/components/tutorial/StationContent.vue
    - web/app/components/tutorial/MOCIndex.vue
    - web/app/composables/useTutorialProgress.ts
    - web/app/composables/useStationRoute.ts
    - web/app/composables/useTutorialFiles.ts
    - web/app/pages/tutorial/[workspace_id]/index.vue
    - web/app/pages/tutorial/[workspace_id]/[station_id].vue
    - tauri/src-tauri/src/tutorial.rs
    - tauri/src-tauri/tests/path_safety.rs
  - Modified:
    - web/nuxt.config.ts（加 `@nuxtjs/mdc` 進 modules）
    - web/package.json（加 `@nuxtjs/mdc` + `gray-matter` dep + lockfile sync）
    - web/app/pages/workspace/grant.vue（granted 後跳 `/tutorial/{workspace_id}` bare path）
    - tauri/src-tauri/src/lib.rs（註冊兩個新 command + `tutorial` mod）
    - tauri/src-tauri/Cargo.toml（補 `dunce` dep 處理 Windows long path / UNC 標準化）
    - CLAUDE.md（Phase 6 動工順序步驟 26 + 27 改完成；子系統段 web 補站牌頁面描述；archive 時間軸新行）
    - docs/implementation-plan.md（§六 步驟 26 + 27 改完成）
    - docs/interactive-tutorial.md（§九 P0 七條 `[x]` 標完成）
  - Removed: 無
- Affected docs:
  - CLAUDE.md（子系統段 web 補 `app/components/content/` + `app/components/tutorial/` + 兩 composable + 兩 page；Phase 6 動工順序步驟 26 + 27 改完成；archive 時間軸新行）
  - docs/interactive-tutorial.md（§九 P0 七條全標 `[x]`）
  - docs/implementation-plan.md（§六 步驟 26 + 27 改完成）
- Test suite delta：前端 typecheck + dev HTTP 200 + grep enforce 三件驗收（test framework Phase B 才裝 — D-026 後續）；sidecar baseline 899 / 19 維持不變（本 change 不動 sidecar 程式碼）
