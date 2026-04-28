## 1. 前置驗證

- [x] 1.1 確認 baseline：`cd sidecar && uv run pytest tests/ -q` 約 899 passed / 19 skipped（本 change 不動 sidecar）；`cd web && npm run typecheck` 全綠；`cd web && npm run dev` HTTP 200
- [x] 1.2 重讀 `docs/interactive-tutorial.md §六 + §九 P0`、`design/v1/10-tutorial-notion.html` mockup、Module 5 Generator 真實輸出範例（`tests/golden/timeline-storage-adapter-synthetic/` 或 demo workspace 的 `codebus-tutorials/`）

## 2. 依賴與設定

- [x] 2.1 `cd web && npm install @nuxtjs/mdc gray-matter`（更新 `package.json` + `package-lock.json`）
- [x] 2.2 改 `web/nuxt.config.ts`：`modules` array 加入 `'@nuxtjs/mdc'`；確認 `ssr: false`（Nuxt SPA mode 已設定，避免 SSR 階段 mdc 行為差異）
- [x] 2.3 改 `tauri/src-tauri/Cargo.toml`：`[dependencies]` 加 `dunce = "1.0"`（Windows long path / UNC 標準化）
- [x] 2.4 跑 `cd web && npm run typecheck` 全綠（mdc auto-import 預期沒有型別錯）

## 3. Tauri command（檔案讀寫雙路徑 + 紅隊測）

- [x] 3.1 [P] 新建 `tauri/src-tauri/src/tutorial.rs`：
    - `read_tutorial_file(workspace_root: String, relative_path: String) -> Result<String, String>` 命令
    - `write_progress_file(workspace_root: String, task_id: String, payload: String) -> Result<(), String>` 命令
    - 共用 helper `validate_path(workspace_root, relative_path) -> Result<PathBuf, String>`：
      1. workspace_root 必須絕對 + 存在 + 是 directory
      2. relative_path MUST 以 `"codebus-tutorials/"` 開頭（不接受 `..` 任何層）
      3. `dunce::canonicalize` 後 MUST 仍在 workspace_root 子樹（`starts_with` 檢查）
      4. 副檔名 allowlist `.md` / `.json`（其他全擋）
      5. 拒絕 symlink 指向外部（`fs::symlink_metadata().is_symlink()` + canonical 後再檢查 starts_with）
    - `read_tutorial_file` 用 `tokio::fs::read_to_string`；失敗回 `format!("read failed: {}", e)`
    - `write_progress_file`：路徑強制為 `<workspace_root>/codebus-tutorials/{task_id}/progress.json`（task_id 必須符合 `^[a-z0-9_-]{1,80}$` 覆蓋 generate_<hex> 格式 + 防 path injection）；用 `tokio::sync::Mutex` 序列化寫入；用 `tokio::fs::write`
    - `list_tutorial_tasks(workspace_root: String) -> Result<Vec<TutorialTaskMeta>, String>` 命令（D-T11 / D-T1，新加）：
      - 列 `<workspace_root>/codebus-tutorials/*/` 子目錄；每個子目錄 dir name = task_id
      - 每個子目錄嘗試讀 `tutorial.md` 取 raw frontmatter 字串（不在 Rust 端解 YAML，前端 `gray-matter` 統一處理）
      - 走共用 `validate_path` helper 確認 `codebus-tutorials/` 子樹安全（與 read_tutorial_file 共用規則）
      - 回 `Vec<TutorialTaskMeta { id: String, frontmatter_raw: Option<String>, dir_mtime_unix: i64 }>`
      - `codebus-tutorials/` 不存在或為空 → 回 `Ok(vec![])`（不 raise；由前端決定 empty CTA）
- [x] 3.2 改 `tauri/src-tauri/src/lib.rs`：`pub mod tutorial`；`tauri::Builder` 的 `invoke_handler!` 註冊三個 command（`tutorial::read_tutorial_file` + `tutorial::write_progress_file` + `tutorial::list_tutorial_tasks`）
- [x] 3.3 [P] 新 `tauri/src-tauri/tests/path_safety.rs`：紅隊測 14 case（11 既有 + 3 新 list_tutorial_tasks）
    - `test_relative_must_start_with_codebus_tutorials`：`relative_path="otherdir/file.md"` → Err
    - `test_dotdot_traversal_rejected`：`relative_path="codebus-tutorials/../../etc/passwd"` → Err
    - `test_extension_allowlist_md_json_only`：`.md` / `.json` Ok；`.txt` / `.exe` / `.sh` 全 Err
    - `test_workspace_must_be_directory`：傳檔案路徑作 workspace_root → Err
    - `test_workspace_must_be_absolute`：傳 relative path → Err
    - `test_symlink_pointing_outside_rejected`：（Unix only，Windows skip）建 symlink 指 home → Err
    - `test_unc_path_normalized`（Windows only）：`\\?\C:\foo` → 與 `C:/foo` canonical 等價
    - `test_case_insensitive_on_windows`（Windows only）：`C:/Foo` 與 `c:/foo` 視為同 workspace
    - `test_write_progress_task_id_format`：task_id 含 `..` / `/` / 空白 → Err
    - `test_write_progress_creates_parent_dir`：parent dir 不存在時自動建立
    - `test_concurrent_writes_serialized`：兩個 async write 不交錯（Mutex 序列化）
    - `test_list_tutorial_tasks_missing_dir_returns_empty`：`codebus-tutorials/` 不存在 → `Ok(vec![])`，不 raise
    - `test_list_tutorial_tasks_workspace_safety`：傳「workspace_root 是檔案 / 不存在 / 相對路徑」→ Err（與 read_tutorial_file 同 validate_path helper）
    - `test_list_tutorial_tasks_skips_non_directories`：`codebus-tutorials/` 內混雜 file（非 dir）時，list 只回實際子目錄，不 raise
- [x] 3.4 跑 `cd tauri/src-tauri && cargo test path_safety` 全綠
- [x] 3.5 跑 `cd tauri/src-tauri && cargo build` 確認新增 dep 編得過

## 4. mdc 互動元件（components/content/）

- [x] 4.1 [P] 新 `web/app/components/content/Checkpoint.vue`：
    - `<script setup lang="ts">` `defineProps<{ id: string }>()`
    - id 格式驗證：`computed` 比對 `/^(station-\d+-check|s\d+-check-\d+)$/`，不符時 console.warn 但仍渲染（fallback gracious）
    - 從 slot 解析 `<input type="checkbox">` 元素（mdc 把 markdown checkbox 渲染成 disabled checkbox），用 `v-for` 渲染成 interactive `<input>` 綁定 useTutorialProgress
    - 全部勾選後顯示 ✓ badge（design token 配色）
    - `useTutorialProgress().setCheckpoint(id, item_index, checked)` 每次 toggle 都呼
    - grep enforce：無 `bg-slate-` / `bg-indigo-` / `bg-zinc-` / hex 字面量
- [x] 4.2 [P] 新 `web/app/components/content/Quiz.vue`：
    - `defineProps<{ id: string; correct: 'a' | 'b' | 'c' | 'd' }>()`（TypeScript Literal union 鎖死，spec scenario 「TypeScript rejects invalid Quiz correct value」要求）
    - 解析 slot 的 `<ul><li>` 為選項，渲染 radio buttons
    - submit button 比對 `selected === correct`；對 → 顯示 ✓ badge + disable 後續修改；錯 → 顯示「再試一次」訊息（不揭示 correct，spec scenario 要求）
    - `useTutorialProgress().setQuizAnswer(id, selectedOption)` 每次 submit 都呼，attempts++
    - grep enforce：無 disallowed colors / hex
- [x] 4.3 [P] 新 `web/app/components/content/QAEntry.vue`：
    - `defineProps<{ prompt: string }>()`
    - 渲染為 button（slot 為 label）
    - click 時 `router.push({ path: '/qa', query: { prompt } })`（P0 placeholder route，後續 change 接 Q&A 頁）
    - 不呼任何 sidecar endpoint（spec scenario 要求）
    - grep enforce：無 disallowed colors / hex
- [x] 4.4 跑 `cd web && npm run typecheck` 全綠

## 5. Composables

- [x] 5.1 [P] 新 `web/app/composables/useTutorialFiles.ts`：
    - 封裝 Tauri IPC 三個 command（D-T1 / D-T11）：
      - `invoke('read_tutorial_file', { workspaceRoot, relativePath })`
      - `invoke('write_progress_file', { workspaceRoot, taskId, payload })`
      - `invoke('list_tutorial_tasks', { workspaceRoot })`
    - 暴露三個 method：
      - `readTutorialFile(workspaceRoot, relativePath): Promise<string>`
      - `writeProgressFile(workspaceRoot, taskId, payload): Promise<void>`
      - `listTutorialTasks(workspaceRoot): Promise<TutorialTaskMeta[]>` 帶 `{ id, frontmatter_raw, dir_mtime_unix }` typed return
    - relative_path 客戶端 sanity check：MUST 以 `'codebus-tutorials/'` 開頭（防呆，Rust 端會再檢一次）
    - 失敗回 typed error（`TutorialFileError`），包含 code 與 message
- [x] 5.2 [P] 新 `web/app/composables/useStationRoute.ts`：
    - `loadRoute(workspaceRoot, taskId): Promise<RouteJson>` 走 useTutorialFiles 載 `codebus-tutorials/{task_id}/route.json`
    - `findStation(route, stationId): RouteStation | null`
    - `findStationFile(route, stationId): string | null`（回 `stations/s02-mqtt-client.md` 相對路徑）
    - TypeScript types `RouteJson` / `RouteStation` 對齊 D-029 schema（`stations[].station_id` / `index` / `title` / `duration` / `file_path` / `required_checks` / `related_stations` / `degraded`）
- [x] 5.3 新 `web/app/composables/useTutorialProgress.ts`：
    - module-level singleton state（與 useSidecar 同 pattern）
    - `loadProgress(workspaceRoot, taskId): Promise<void>` 在 page mount 時呼，載入 progress.json（不存在則初始化空 schema）
    - `setCheckpoint(checkId, itemIndex, checked)` / `setQuizAnswer(quizId, answer)` / `setCurrentStation(stationId)` 三個 mutator
    - 內部 `debounce(500ms)` flush 改動到 Tauri write_progress_file
    - `addEventListener('beforeunload', flush)` 確保關 App 前 sync flush
    - `unlockedStationIds(route): ComputedRef<Set<string>>`：依 spec D-T5 演算法計算
    - `isStationComplete(stationId, route): ComputedRef<boolean>`
    - `canVisitStation(stationId, route): ComputedRef<boolean>`（unlocked 或在 completed_station_ids 內）
    - 嚴格不寫 localStorage / sessionStorage / cookies（spec 要求）
- [x] 5.4 跑 `cd web && npm run typecheck` 全綠

## 6. Tutorial component shell（components/tutorial/）

- [x] 6.1 [P] 新 `web/app/components/tutorial/StationLayout.vue`：
    - props：`frontmatter: StationFrontmatter` / `totalStations: number`
    - render 標題（`frontmatter.title`）、進度（`第 ${frontmatter.station_index} / ${totalStations} 站`）、duration badge（`${frontmatter.duration_minutes} 分鐘`）、degraded badge（`v-if="frontmatter.degraded"` 顯示警告）
    - `<slot>` 給 content 區
    - 失敗 fallback：父 component 處理（StationLayout 只負責 render；missing 必填欄位由父 page 在 mount 時擋）
- [x] 6.2 [P] 新 `web/app/components/tutorial/StationNav.vue`：
    - props：`route: RouteJson` / `currentStationId: string | null` / `unlockedStationIds: Set<string>` / `completedStationIds: string[]`
    - 渲染 station list，每條根據 unlock state 套不同 styling（locked / unlocked / current / completed）
    - 點擊 unlocked 或 completed 條目：emit `navigate(stationId)` event；locked 條目：disable click
- [x] 6.3 [P] 新 `web/app/components/tutorial/StationContent.vue`（對應 spec Requirement: Sub-page navigation within station markdown；design D-T12）：
    - props：`markdown: string`（已 strip frontmatter 的 body）+ optional `key`（station_id 變化時重置 chunk index）
    - mount 時用 `markdown.split(/^###\s+/m)` 切成 chunks（chunk 0 = `###` 之前的內容；其後每段以 `###` 起頭）；`chunkIndex: ref(0)`
    - 渲染：`<MDC :value="chunks[chunkIndex]" />` 只 render 當前 chunk
    - keyboard listener（page-level `@keydown`）：`ArrowDown` / `PageDown` → `chunkIndex = Math.min(chunkIndex + 1, chunks.length - 1)`；`ArrowUp` / `PageUp` → `chunkIndex = Math.max(chunkIndex - 1, 0)`；focus 在 `input` / `textarea` / `[contenteditable]` 上時不觸發（不 preventDefault）
    - 底部進度條：「第 ${chunkIndex + 1} / ${chunks.length} 頁」
    - 0 個 `###` 的 markdown：chunks.length === 1，listener 仍 mount 但操作為 no-op
    - watch `props.markdown`（station 切換時）→ chunkIndex 重置 0
- [x] 6.4 [P] 新 `web/app/components/tutorial/MOCIndex.vue`：
    - props：`mocMarkdown: string`（tutorial.md 的 body，已 strip frontmatter）
    - 用 `<MDC :value="mocMarkdown" />` 渲染
    - 並列 station list overlay：站名 link 走 `/tutorial/${workspaceId}/${station_id}` 路由
    - 每條站附 unlock badge（locked / unlocked / completed）
- [x] 6.5 grep enforce：`web/app/components/tutorial/` 內無 disallowed colors / hex 字面量 / localStorage / sessionStorage

## 7. Pages（pages/tutorial/[workspace_id]/）

- [x] 7.1 新 `web/app/pages/tutorial/[workspace_id]/index.vue`（MOC 首頁，對應 spec Requirement 1 兩條 task_id fallback scenario + Requirement 6 empty CTA scenario；design D-T11 + D-T13）：
    - mount 時：parse `route.params.workspace_id`，呼共用 helper `resolveTaskId(workspaceRoot, route.query.task)`（落 `web/app/composables/useStationRoute.ts` 或新 helper）
    - `resolveTaskId` 演算法（D-T11）：
      1. `?task=<id>` 有帶且符合 `^generate_[0-9a-f]{8}$` 且目錄存在 → 用該 task；source = "query"
      2. query 缺或目錄不存在 → 走 `useTutorialFiles().listTutorialTasks(workspaceRoot)` 掃 `<ws>/codebus-tutorials/*/`
      3. 回 0 個 → `task_id = null, source = "empty"` → render empty CTA panel（D-T13 placeholder copy，含 `POST /generate` curl 範例；MUST NOT 自呼 `/generate`、MUST NOT 顯示 error 框架）
      4. 回 1 個 → 用該 task；source = "single"
      5. 回多個 → 取 `tutorial.md` frontmatter `generated_at` 最新（fallback：dir mtime）；source = "latest"
    - `useTutorialFiles` 加 `listTutorialTasks(workspaceRoot)` method：透過新 Tauri command `list_tutorial_tasks(workspace_root)` 列 `codebus-tutorials/*/` 子目錄與 `tutorial.md` frontmatter 的 `generated_at`（task 3.1 須同步加這個 command + path 安全限 `codebus-tutorials/` 子樹只讀目錄列表，無檔案讀取）
    - 拿到 task_id 後：用 `useTutorialFiles().readTutorialFile(...)` 載 `route.json` + `tutorial.md`
    - `gray-matter` 解析 tutorial.md frontmatter（如有）+ body
    - 用 useTutorialProgress.loadProgress(workspaceRoot, task_id) 把 progress 載入 module state
    - render `<StationLayout>` 包 `<MOCIndex>` + 左側 `<StationNav>`
    - 後置失敗 fallback：task_id 已選但 route.json / tutorial.md 讀取失敗（檔損毀 / 非預期錯誤）→ 顯示「教材檔案讀取失敗」error view 連結回首頁；與 empty CTA 視覺截然不同（empty 是 first-run 引導、error 是異常）
- [x] 7.2 新 `web/app/pages/tutorial/[workspace_id]/[station_id].vue`（單站頁）：
    - parse `station_id` URL param + 用 regex `^s\d{2}-[a-z0-9-]{1,40}(-\d+)?$` 驗證；不符回 friendly error 連結 MOC
    - 載 `route.json` + 對應 `stations/{station_id}.md`（用 useStationRoute.findStationFile）
    - `gray-matter` 解析 frontmatter + body
    - `useTutorialProgress().canVisitStation(station_id, route)` 為 false 時 render「locked」view 連結回當前站或 MOC（spec scenario 要求）
    - 為 true 時 render `<StationLayout :frontmatter="parsed.data" :totalStations="route.stations.length"><StationContent :markdown="parsed.content" /></StationLayout>`
    - 左側永遠 render `<StationNav>` 給切站
    - mount 時 `useTutorialProgress().setCurrentStation(station_id)`
    - 已完成站額外 render review-mode badge
- [x] 7.3 改 `web/app/pages/workspace/grant.vue`：
    - `onGranted(payload: GrantResponse)` 改 `router.push({ path: \`/tutorial/${payload.workspace_id}\` })`，**不帶 `?task=` query**（first-run 設計：grant 完還沒 generate，task_id 未定；URL **不**帶 `/index`，Nuxt 4 file routing 把 `pages/tutorial/[workspace_id]/index.vue` map 成 `/tutorial/:workspace_id`）
    - first-run 流程：grant 成功 → 直跳 R-01 index → `resolveTaskId` 走 empty CTA 分支顯示「請先 generate」placeholder（D-T11 / D-T13）
    - second-run 流程（已跑過 generate）：grant 後跳 R-01 index → `resolveTaskId` 走 latest fallback 自動選最新 task → render MOC
    - 不在本 task 處理 task_id；R-01 index page 內 `resolveTaskId` 是唯一推導路徑（task 7.1）

## 8. 整合驗收

- [x] 8.1 `cd web && npm run typecheck` exit 0
- [x] 8.2 `cd web && npm run dev` HTTP 200，四條路由可達：`/`、`/workspace/grant?path=...`、`/tutorial/ws_x`、`/tutorial/ws_x/s01-overview`（後兩條 P0 沒真檔即 fallback view，但路由本身應 200；MOC 路由 `/tutorial/{ws_id}` **不**帶 `/index` segment — 那會被解成 `[station_id].vue` 並失敗）
- [x] 8.3 `cd tauri/src-tauri && cargo test` 全綠（含 path_safety 13 case + 既有 7 case fs_scope/handshake_parse test）
- [x] 8.4 `cd sidecar && uv run pytest tests/ -q` 數字維持 baseline（本 change 不動 sidecar，期望 899 passed / 19 skipped 持平；實測 899 passed + 1 已知 Windows timing flake `test_startup_remains_available_when_qdrant_unreachable`，與 spec-cleanup-stage-5-batch-a archive 紀錄一致）
- [x] 8.5 grep enforce 七件：
    - `rg "localStorage\|sessionStorage\|document\.cookie" web/app/composables/useTutorialProgress.ts` → 0 命中
    - `rg "\.writeProgressFile\(" web/app/` → 命中只在 `composables/useTutorialProgress.ts`（單一 writer 不變式；`invoke('write_progress_file', ...)` 字面允許在 `useTutorialFiles.ts` IPC wrapper 內）
    - `rg "fetch\(['\"]file://" web/app/` → 0 命中
    - `rg "bg-slate-\|bg-indigo-\|bg-zinc-\|text-slate-\|text-indigo-\|text-zinc-" web/app/components/content/ web/app/components/tutorial/` → 0 命中
    - `rg "/tutorial/[^/]+/[0-9]+" web/app/components/ web/app/pages/` → 0 命中（禁用 numeric index URL）
    - `rg "invoke\(['\"]list_tutorial_tasks" web/app/` → 命中只在 `composables/useTutorialFiles.ts`（單一封裝路徑，禁直呼）
    - `rg "/tutorial/[^/]+/[a-f0-9]{8,}/" web/app/components/ web/app/pages/` → 0 命中（D-T11：URL 不該含 task_id segment）
- [ ] 8.6 manual smoke（**待 user 跑**：本 agent 環境無 GUI 無法執行）：`cargo tauri dev` → 開瀏覽器走 grant → R-01 MOC → 點站牌 → 勾 Checkpoint → 答 Quiz → 走完一站 → 解鎖下一站 → 關 App 重開驗證 progress.json 落地

## 9. 文件連動

- [x] 9.1 改 `CLAUDE.md` Phase 6 動工順序：步驟 26 + 27 row 從待動工改完成（透過 `docs/implementation-plan.md` row 編輯間接達成；CLAUDE.md slim 後不再維護 Phase 6 步驟詳列）
- [x] 9.2 改 `CLAUDE.md` 子系統段 web：補 `app/components/content/` + `app/components/tutorial/` + `useTutorialFiles.ts` / `useStationRoute.ts` / `useTutorialProgress.ts` + 兩 page；補 Tauri 端 `tutorial.rs` + 三 command 描述
- [ ] 9.3 ~~改 `CLAUDE.md` archive 時間軸：新增 row~~ **NoOp**：CLAUDE.md 在 commit `6ade8f8`（2026-04-28）已將 archive 時間軸整段刪除，由 `ls openspec/changes/archive/` 自身充當索引；本 change archive 後直接出現在該目錄即可，CLAUDE.md 無需改動 archive 段
- [x] 9.4 改 `docs/implementation-plan.md §六` 步驟 26 + 27：標 ✅ 已完成（同 26.5 pattern）
- [x] 9.5 改 `docs/interactive-tutorial.md §九 P0`：七條全標 `[x]`（對應 P0 條目 1-7）

## 10. 規格覆蓋錨點（apply 階段純驗證 checkbox）

- [x] 10.1 Spec coverage：`Station routing schema uses stable station id as URL key` 由 task 7.1 + 7.2 + 8.5 grep enforce 滿足
- [x] 10.2 Spec coverage：`Three mdc interactive components with strict prop contracts` 由 task 4.1-4.3 滿足
- [x] 10.3 Spec coverage：`progress.json schema and single-writer path` 由 task 5.3 + 8.5 grep enforce 滿足
- [x] 10.4 Spec coverage：`Unlock logic gates next-station access on completion` 由 task 5.3 + 7.2 滿足
- [x] 10.5 Spec coverage：`frontmatter parser drives StationLayout shell` 由 task 6.1 + 7.2 滿足
- [x] 10.6 Spec coverage：`MOC renders tutorial.md as the index page` 由 task 6.4 + 7.1 滿足（含新加 `Empty workspace shows generate CTA instead of error` scenario）
- [x] 10.7 Spec coverage：`Sub-page navigation within station markdown` 由 task 6.3 滿足（含 ### split / 鍵盤事件 / 不在 input 觸發 / 跨站 reset / 0 個 ### 視為 1 chunk 五條 scenario）
- [x] 10.8 Spec coverage：`Station routing schema` 新加兩條 scenario（task_id query/latest fallback）由 task 3.1 + 5.1 + 7.1 滿足

## 11. Design / Risks 交叉索引（apply 期不執行；對齊 design.md）

| design.md 條目 | 對應 task |
|---|---|
| D-T1：檔案讀取走新 Tauri command，不走 sidecar HTTP | 3.1, 3.2, 3.3, 5.1 |
| D-T2：URL routing 用 Nuxt dynamic params，不用 hash routing | 7.1, 7.2 |
| D-T3：route.json + station 內容分階段 lazy load | 7.1, 7.2, 5.2 |
| D-T4：progress.json 寫入路徑 | 3.1, 5.3 |
| D-T5：解鎖邏輯純 client side，computed 算 | 5.3, 7.2 |
| D-T6：mdc 元件 prop 簽名嚴格 typed | 4.1, 4.2, 4.3 |
| D-T7：StationLayout 三段 grid，Top/Audit 沿用 default layout | 6.1, 6.2, 6.3, 7.1, 7.2 |
| D-T8：station markdown frontmatter 解析用 `gray-matter` | 7.1, 7.2 |
| D-T9：Quiz 答錯重試 + 提示 | 4.2 |
| D-T10：MOC 與 station 頁的 page 結構分離 | 7.1, 7.2 |
| D-T11：task_id implicit latest with query override；selector P1 | 3.1, 3.3, 5.1, 7.1, 7.3, 8.5 |
| D-T12：StationContent `###` 次級切頁屬 P0 | 6.3 |
| D-T13：Empty CTA placeholder copy + future dashboard 預留 | 7.1 |
| R-1：station markdown LLM 產出與 mdc 元件契約不符 | 4.1（fallback gracious render）|
| R-2：progress.json 競爭寫入 | 3.1（Mutex 序列化），R-2 mitigation P0 接受 |
| R-3：tauri command path validation 寫錯造成跨樹遍歷 | 3.1, 3.3（11-case 紅隊測）|
| R-4：mdc Vue runtime SSR / SPA 行為差異 | 2.2（確認 `ssr: false`）|
| R-5：useTutorialProgress 的 debounce 在 close window 時遺失 | 5.3（beforeunload flush）|
