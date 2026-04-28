## Context

Phase 6 步驟 26 + 27 合併實作 R-01 站牌學習頁。教材已由 Module 5 Generator 寫到 `<ws>/codebus-tutorials/{task_id}/`（多檔結構：`tutorial.md` MOC + `stations/s{NN}-slug.md` + `route.json`，D-029）；本 change 在前端把它們解析、渲染、加上互動元件、推 progress 解鎖邏輯。

`docs/interactive-tutorial.md` 410 行 spec 已寫好（2026-04-19），P0 七條清單在 §九；本 change 落地 `openspec/specs/interactive-tutorial/` capability 把 P0 部分規範化（與 `docs/authorization.md` → `auth-flow` 同樣模式）。

P0 mockup 是 `design/v1/10-tutorial-notion.html`（橫向三段：StationNav 站牌列表 / StationContent 中央內容 / 內容區頂置 Checkpoint progress）+ `design/v1/11-tutorial-slideshow.html`（投影片模式：純 mdc 渲染 + 鍵盤翻頁）。本 change 只實作 Notion-style 主畫面（10）；slideshow 模式（11）視 P0 完成度延伸。

`@nuxtjs/mdc` 是 Nuxt 4 官方支援的 markdown 渲染 module，自動掃描 `web/app/components/content/` 註冊 mdc 元件。Timeline PWA 既有同 module 的成功 reference，前端管線無未知數。

## Goals / Non-Goals

**Goals:**

- `<Checkpoint>` / `<Quiz>` / `<QAEntry>` 三個 mdc 元件落地，契約對齊 `docs/interactive-tutorial.md §四`
- R-01 路由 schema：`/tutorial/{workspace_id}/index`（MOC）+ `/tutorial/{workspace_id}/{station_id}`（單站）；URL key 一律用 D-029 stable station id
- `progress.json` 唯一寫入路徑（`useTutorialProgress` composable）+ 解鎖邏輯（Checkpoint 全勾 + Quiz 全對 → 解鎖下一站）
- Tauri command 讀 `<ws>/codebus-tutorials/` 子樹下檔案（路徑限白名單避免跨樹遍歷；讀 read-only）
- frontmatter parser → `<StationLayout>` 外殼（標題 / 時長 / degraded badge / station_index 進度）
- MOC 渲染 + 站名 link 走 stable id 路由
- grant.vue granted 後跳 `/tutorial/{workspace_id}/index`

**Non-Goals:**

- slideshow 模式（mockup 11）— P1
- `<CodeRef>` / `<Reveal>`、Agent console（屬步驟 28 範圍）
- 跨站搜尋 / 教材匯出 / 多語系
- progress.json 雲端同步（local-first）
- LLM 判題 / 多選題

## Decisions

### D-T1：檔案讀取走新 Tauri command，不走 sidecar HTTP

**選項：**
- (a) 新 Tauri command `read_tutorial_file(workspace_root, relative_path)` ✓
- (b) 新 sidecar HTTP endpoint `GET /tutorial/file?workspace=...&path=...`
- (c) Nuxt server route（Nitro）
- (d) `fetch('file://...')` 直接讀

**選 (a)。** 教材檔案讀取是 process 內 IPC，不應該繞 sidecar HTTP（sidecar 的角色是 LLM / KB / Agent，不是檔案 server）。Tauri command 走 OS process boundary 即 trust boundary，無 bearer 開銷、無 serialization；Rust 端 path validation 收斂在一個 source of truth 比前端 / sidecar 雙方都要做來得穩。

(b) 增加 sidecar 不必要的 surface area，違反 sidecar 職責邊界（capability spec `sidecar-runtime` 沒有 file-read endpoint，引入會破壞「sidecar = network/agent/audit」抽象）。
(c) Nitro 在 packaged Tauri app 不會跑（Nuxt 用 SPA mode），死路。
(d) Tauri 預設 disable `file://` 協定，且打開等於放棄 path scope 控制。

**Rust 端實作要求**（落在 `tauri/src-tauri/src/tutorial.rs` 新檔，3 個 command + 共用 `validate_path` helper）：

```rust
#[tauri::command]
async fn read_tutorial_file(
    workspace_root: String,
    relative_path: String,
) -> Result<String, String> {
    // 1. workspace_root 必須是絕對路徑、存在、是 directory
    // 2. relative_path 必須以 "codebus-tutorials/" 開頭
    // 3. canonical_resolve(workspace_root + relative_path) 必須仍在 workspace_root 子樹
    //    (擋 ../ 逃逸 + symlink 指外)
    // 4. 副檔名限 .md / .json
    // 5. fs::read_to_string；失敗回 String error
}

#[tauri::command]
async fn write_progress_file(
    workspace_root: String,
    task_id: String,
    payload: String,
) -> Result<(), String> {
    // 路徑強制為 <workspace_root>/codebus-tutorials/{task_id}/progress.json
    // task_id 必須符合 ^[a-z0-9_-]{1,80}$（覆蓋 generate_<hex> 格式 + 防 path injection）
    // tokio::sync::Mutex 序列化寫入；tokio::fs::write
}

#[tauri::command]
async fn list_tutorial_tasks(
    workspace_root: String,
) -> Result<Vec<TutorialTaskMeta>, String> {
    // 列 <workspace_root>/codebus-tutorials/*/ 子目錄
    // 每個子目錄：dir name = task_id；嘗試讀 tutorial.md 取 frontmatter.generated_at（gray-matter 在前端解析，這層只回 raw frontmatter 字串）
    // 走共用 validate_path helper 確認 codebus-tutorials/ 子樹安全
    // 回 [{ id: "generate_a3f2b1c8", frontmatter_raw: "...", dir_mtime_unix: 1234567890 }]
    // codebus-tutorials/ 不存在或為空 → 回 Ok(vec![])（不 raise，由前端決定 empty CTA）
}
```

`ensure_in_workspace` 概念與 sidecar `ToolSandbox` 對等（`docs/tool-sandbox.md §五`），但實作完全在 Rust 側不共用 Python 程式碼（process 邊界）。三個 command 共用同一個 `validate_path(workspace_root, relative_path) -> Result<PathBuf, String>` helper，避免規則 drift。

### D-T2：URL routing 用 Nuxt dynamic params，不用 hash routing

**選項：**
- (a) `pages/tutorial/[workspace_id]/[station_id].vue` ✓
- (b) `pages/tutorial.vue` + hash routing（`#/ws-abc/s02-mqtt-client`）
- (c) Single page + state-driven view switching

**選 (a)。** Nuxt 4 SPA mode + dynamic params 在 Tauri WebView 完全可用（route 變化純 client-side），browser 重整 / 直接 paste URL 都能 land 正確 view。Hash routing 是過時的 SPA workaround，現代 Nuxt SPA 直接走 history API。Single page state 違反「URL 是 source of truth」原則，使用者按上一頁會失序。

`workspace_id` 從 grant 流程帶入（`GrantResponse.workspace_id`，前綴 `ws_` + 12 hex），station_id 是 D-029 stable id（`s{NN}-slug` 格式）。

### D-T3：route.json + station 內容分階段 lazy load

**選項：**
- (a) MOC 頁載入 route.json，station 頁切換時 lazy load 該站 markdown ✓
- (b) MOC 頁一次載 route.json + 全部 station markdown
- (c) Module-level cache + invalidation

**選 (a)。** 一個 workspace 最多 ~10 站，route.json 通常 < 50KB，per page mount 重 load 無 perf 問題。station markdown 切換時才載一個檔（最大 ~10KB），用 `await readTutorialFile` await 在 page setup；component lifecycle 切換時 Vue reactive 自動 unmount 舊內容、mount 新內容，無需手動 cache。

**禁止 module-level cache：** 教材可能被使用者重新 generate（換任務 / 換 workspace），cache invalidation 是另一個雷區。每次切站讀檔的成本 ~5ms，不 cache 的成本可接受。

### D-T4：progress.json 寫入路徑

**選項：**
- (a) 新 Tauri command `write_progress_file(workspace_root, task_id, payload)` ✓
- (b) 寫 localStorage
- (c) 走 sidecar endpoint write

**選 (a)。** 與 D-T1 對稱（讀寫對等），確保檔案是 source of truth（重啟 App / 換 device 透過 dotfile 同步機制都能無痕續讀）。`progress.json` 寫在 `<ws>/codebus-tutorials/{task_id}/progress.json` 與 generator 輸出同目錄。

(b) localStorage 違反「local-first 但檔案是 SoT」（`auth-flow` 同樣禁 localStorage 存敏感資料；progress 雖非敏感但 SoT 一致性比較重要）。
(c) 不擴 sidecar 職責邊界（同 D-T1 理由）。

`useTutorialProgress` composable 是**唯一寫入路徑**：
- 任何地方要更新 progress（Checkpoint 勾選 / Quiz 答對） MUST 走 `useTutorialProgress().setCheckpoint(...)` / `.setQuizAnswer(...)`
- Composable 內部 debounce ~500ms 後呼 Tauri command 寫檔（避免每次勾選都打 IPC）
- defensive test：grep enforce `web/app/` 內無直接 invoke `write_progress_file` 的呼叫（只能在 useTutorialProgress 內）

### D-T5：解鎖邏輯純 client side，computed 算

**選項：**
- (a) `useTutorialProgress.unlockedStationIds: ComputedRef<string[]>` ✓
- (b) Server-side（不可，純 client 資料）
- (c) Cache + invalidation

**選 (a)。** 解鎖邏輯數據完全 client-side（progress.json + route.json），用 Vue computed reactive 自動重算，cache 是無謂複雜化。

**演算法**（從 `route.json.stations` 第一站開始往後掃）：

```typescript
function computeUnlocked(progress: Progress, route: Route): Set<string> {
  const unlocked = new Set<string>([route.stations[0].station_id])
  for (const station of route.stations) {
    if (!unlocked.has(station.station_id)) break
    if (isStationComplete(station.station_id, progress)) {
      const next = nextStation(station, route)
      if (next) unlocked.add(next.station_id)
    }
  }
  return unlocked
}

function isStationComplete(id: string, progress: Progress): boolean {
  const required = route.stations[id].required_checks  // ['station-N-check', 's{N}-q1', ...]
  return required.every((checkId) => {
    if (progress.checkpoints[checkId]?.done === true) return true
    const quiz = progress.quizzes[checkId]
    return quiz?.correct === true
  })
}
```

**已完成站可回看**：URL 可直接 paste 進已完成站；解鎖邏輯只擋「導航器讀新站」，不擋「URL 重訪已通過站」。實作上，page 載入時 check `unlockedStationIds.has(station_id) || progress.completed_station_ids.includes(station_id)` 為真就允許。

### D-T6：mdc 元件 prop 簽名嚴格 typed

**選項：**
- (a) 三個元件全用 `defineProps<...>()` + TypeScript Literal union 鎖死 ✓
- (b) Optional / loose typing 讓 LLM 產的 markdown 不嚴格時也能渲染

**選 (a)。** `<Quiz id="..." correct="b">` 的 `correct` 必為 `'a' | 'b' | 'c' | 'd'`、`<Checkpoint id="...">` 的 `id` 必為 `^(station-\d+-check|s\d+-q\d+)$` 格式（與 `route.json.required_checks` 對齊）。LLM 產出歪掉時，typecheck 階段就抓出來，比 runtime 才發現安全。

**生成端責任**：Module 5 Generator 已在 prompt 約束（`docs/prompts.md` Generator 章），且 `module-5-generator` capability `Markdown validator enforces D-029 component rules` 鎖定 800-char ceiling 等規則；本 change 不重複 validate（信任 Generator 端 contract）。

### D-T7：StationLayout 三段 grid，Top/Audit 沿用 default layout

**選項：**
- (a) `default` layout（topbar + audit panel）+ R-01 內部三段 grid（`StationNav` 左 / `StationContent` 中 / 進度條頂置） ✓
- (b) 自定 `tutorial.vue` layout 覆蓋 default

**選 (a)。** R-01 是 page 級新功能，不該動 layout（layout 是跨頁共用骨架）。MOC 與 station 兩個 page 共用 `<StationLayout>` component（不是 layout file）做內部三段 grid。`StationNav` 永遠顯示 `route.json.stations` 列表（locked / unlocked / current 三狀態 styling）；`StationContent` 切換時只 swap mdc 渲染內容。

### D-T8：station markdown frontmatter 解析用 `gray-matter`

**選項：**
- (a) `gray-matter` npm package（mature，~30k weekly）✓
- (b) 自寫 YAML parser
- (c) 由 mdc 內建 frontmatter 解析

**選 (a)。** mdc module 雖有 frontmatter parsing 但只給 component 用（`useNuxtApp().$mdc`），對「讀檔之後拆 frontmatter / body 兩半」的明確 API 不直白；`gray-matter` 一行 `matter(rawMd)` 拆乾淨，輸出 `{ data: object, content: string }` 直接餵 mdc 與 StationLayout。Bundle size ~4KB minified，不在意。

### D-T9：Quiz 答錯重試 + 提示

**選項：**
- (a) 答錯顯示「再試」+ `attempts++`，不揭示正解 ✓
- (b) 答錯直接揭示正解
- (c) 答錯 N 次後揭示

**選 (a)。** 教學情境下「再試一次」是正向引導；揭示正解只給「想跳過」的使用者，與 onboarding 學習目標衝突。`progress.quizzes[id].attempts` 紀錄純 audit 用，不影響解鎖（解鎖只看 `correct === true`）。

P1 follow-up：考慮加「3 次後可選擇看解析」的折衷模式，但 P0 stage 拒絕額外 UX 分支。

### D-T10：MOC 與 station 頁的 page 結構分離

**選項：**
- (a) 兩個 page 檔（`index.vue` + `[station_id].vue`）✓
- (b) 一個 page 檔加 conditional rendering

**選 (a)。** 兩個 view 的職責截然不同（MOC 是站列表 + 入口、station 是學習主畫面 + 互動），合一個檔會讓 setup 邏輯肥大。Nuxt dynamic route file structure 自然把它分開。

`<MOCIndex>` 與 `<StationLayout>` 兩個 component 也各自獨立 — 不需要強行抽 base component（職責不同）。

### D-T11：task_id implicit latest with query override；selector P1

**選項：**
- (a) URL 加 `[task_id]/` 層 → `/tutorial/{ws_id}/{task_id}/{station_id}`
- (b) 隱藏 task_id；page mount 時走 query → 掃目錄取最新 fallback ✓
- (c) Sidecar 紀錄 workspace 的 "current task" state

**選 (b)。** `task_id` 是 generator hex（`generate_a3f2b1c8`），URL 醜且使用者不該記。一個 workspace P0 預設情境是「跑一次 generate 看一份教材」，把 task_id 拉進 URL hierarchy 是為 multi-task power user 場景過早優化。spec Requirement 1 的 2 層路由是 conscious design，不是漏層。

(a) 優點是 multi-task 顯式 + URL 可分享精確版本，但代價是 URL 醜化 + first-run 流程多一層 / 隱藏「使用者不需要知道 task_id 的存在」這個 UX 優勢。
(c) 增加 sidecar state 與 endpoint surface（違反 D-T1 sidecar 邊界），且狀態與檔案系統雙來源容易 drift。

**Page mount 推導演算法**（落在 `pages/tutorial/[workspace_id]/index.vue` + `[station_id].vue` 共用 helper）：

```typescript
async function resolveTaskId(workspaceRoot: string, queryTask: string | null): Promise<TaskResolution> {
  if (queryTask && /^generate_[0-9a-f]{8}$/.test(queryTask)) {
    return { task_id: queryTask, source: 'query' }
  }
  // 掃 <ws>/codebus-tutorials/*/ 目錄
  const tasks = await listTutorialTasks(workspaceRoot)  // 走 useTutorialFiles
  if (tasks.length === 0) return { task_id: null, source: 'empty' }
  if (tasks.length === 1) return { task_id: tasks[0].id, source: 'single' }
  // 多 task：取 frontmatter.generated_at 最新（或 dir mtime fallback）
  const latest = tasks.sort((a, b) => b.generated_at - a.generated_at)[0]
  return { task_id: latest.id, source: 'latest' }
}
```

**P1 follow-up**：Topbar 多 task 時加「切換任務」icon，下拉列出所有 task 帶 generated_at；本 change 不做 selector UI（empty / single / multi 三態都走 implicit）。

**Empty 處理**：`source === 'empty'` 時 page render「教材尚未產出」CTA（D-T13），不跳 error；first-run grant→R-01 直跳的預設情境（grant.vue 不帶 query）會自然落在 empty CTA 而非 error，這是 conscious UX 選擇。

### D-T12：StationContent `###` 次級切頁屬 P0

**選項：**
- (a) P0 全部 scroll 一頁 render，up/down 翻頁 P1
- (b) P0 切 `###` chunks + up/down 翻頁 ✓

**選 (b)。** `docs/interactive-tutorial.md §三` + §九 P0 第 4 條明確規定「`###` 當檔內次級分頁符」+ 「up/down 或 PageUp/PageDown 在同檔內滑動」屬 P0；一份 station 檔通常 2-3 個 `###` 區塊（核心概念 / 專案怎麼用 / 檢核站），全部 scroll 渲染等於投影片體驗破功（demo 殺傷力大）。+0.5d 工期可吸收。

**實作要點**：`StationContent.vue` 在 mount 時把 markdown body 用 `/^###\s+/m` split 成 chunk array；保留 chunk 0 預設顯示；keyboard event listener (`up` / `down` / `PageUp` / `PageDown`) 切換 chunk index；URL 不反映 chunk index（chunk 是檔內次級結構，URL 仍是 station 級 stable id）；底部進度條顯示 `chunk_index / total_chunks`。

**Trade-off**：keyboard handler 全域 listen 可能與 R-01 內其他 hotkey（未來）衝突；P0 用 `@keydown` 在 page-level 捕捉、`event.target` 檢查不在 input 上才觸發。

### D-T13：Empty CTA placeholder copy + future dashboard 預留

**選項：**
- (a) Empty 時 R-01 自帶「new run」按鈕直接呼 `POST /generate`
- (b) Empty 時 R-01 顯示「請先 generate」placeholder + debug-friendly curl 指令 ✓
- (c) Empty 時 redirect 回 `/workspace/{ws_id}/run` 進度頁（不存在）

**選 (b)。** R-01 是「教材閱讀器」(proposal 範圍判決：拒絕與步驟 28 Agent console 合一個 change，**同邏輯適用拒絕學習頁吃執行責任**)。Generate 是長 flow + 需要 scan/kb/explore 前置，應由「workspace 執行頁」觸發，那個頁面屬於後續 change（步驟 28+ 之後）。R-01 的 empty CTA 暫顯示 placeholder copy + 直接給 `POST /generate` curl 範例（debug-friendly），future change 落地執行頁後改 CTA 跳路由。

**Placeholder copy（zh-tw）**：

```
此 workspace 尚無已產出的教材。

下一步：呼叫 POST /generate 觸發產生（執行頁面落在後續 change）。
範例：curl -X POST -H "Authorization: Bearer <token>" \
       http://127.0.0.1:<port>/generate \
       -d '{ "workspace_root": "<path>", "task": "<question>", "stations": [...] }'
```

(a) 把 R-01 scope 拉到「執行控制台 + 學習頁」雙模式，工期翻倍且職責不清（同 proposal 範圍判決）。
(c) 跳到不存在的路由是死路，等於沒解。

## Risks / Trade-offs

### R-1：station markdown LLM 產出與 mdc 元件契約不符

**風險：** Module 5 Generator 雖在 prompt 約束 `<Quiz id="..." correct="...">` 格式，但 LLM 偶爾會產 `<Quiz id="" correct="">` 或漏標 id。前端硬擋會讓整站「打不開」（degraded UX）。

**緩解：** 1) 元件層做寬容 fallback（id 缺失時 generate `<crypto.randomUUID()>` 補上、`correct` 缺失時 disable Quiz interaction 但仍渲染題目給使用者讀）；2) 元件渲染失敗 emit `Sentry`-style 訊息到 console（P0 直接 console.warn，P1 接 Audit Panel 第八 tab）；3) Generator side 已有 `Markdown validator enforces D-029 component rules`（`module-5-generator` capability）擋多數歪掉的 case。

### R-2：progress.json 競爭寫入

**風險：** 使用者開兩個視窗（雖然 Tauri 多視窗 P0 沒做，但開發測試時可能）同時操作 → progress.json 兩個寫入 race。

**緩解：** P0 接受這個 limitation；Tauri command Rust 端 use `tokio::sync::Mutex` 序列化寫入（避免檔內容半寫）。多視窗並行修改最壞結果是「後寫的 wins」，跟 file system natural behavior 一致。

### R-3：tauri command path validation 寫錯造成跨樹遍歷

**風險：** Rust 端 path validation 沒寫好（如忽略 Windows UNC、symlink、`\\?\` long path、case sensitivity 等）→ 使用者開的 workspace 可能讀到 home 其他地方檔案。

**緩解：** 1) 對齊 sidecar `ensure_in_workspace` 的紅隊測（`sidecar/tests/sandbox/`）覆蓋 Rust 同名測 `tauri/src-tauri/tests/path_safety.rs`（P0 必落，與 sidecar 紅隊覆蓋同類 case）；2) 用 `dunce::canonicalize` 處理 Windows long path / UNC；3) 副檔名 allowlist `.md` / `.json` 雙重關卡（即使 path 漏網，副檔名擋 `.ssh/id_rsa` 之類）。

### R-4：mdc Vue runtime SSR / SPA 行為差異

**風險：** Nuxt 4 預設 SSR，mdc 在 SSR 階段會 prerender markdown。Tauri 是 SPA mode（`nuxt.config.ts` 已設 `ssr: false`？）— 兩個模式下 mdc 行為可能不同。

**緩解：** 確認 `nuxt.config.ts.ssr === false`（既有設定應該已是 SPA）；mdc module 在 SPA mode 純 client 渲染，與 Tauri WebView 行為一致。dev server 跑 SPA mode 加上 `npm run dev` smoke test 即足驗。

### R-5：useTutorialProgress 的 debounce 在 close window 時遺失

**風險：** 使用者勾完 Checkpoint 立刻關 App，500ms debounce window 內沒寫到檔 → 進度遺失。

**緩解：** 1) `beforeunload` event 觸發 immediate flush（不等 debounce）；2) 「重要事件」（如 station 完成、Quiz 答對）跳過 debounce 直接寫；3) debounce window 縮 200ms（perf 影響微小，UX 無感）。

## Migration Plan

P0 階段 R-01 是新 page，沒既有 user data 要 migrate。`progress.json` 是新檔，不存在則由 useTutorialProgress 在第一次 update 時建立空 `{ current_station_id: null, completed_station_ids: [], checkpoints: {}, quizzes: {} }`。

部署：
1. 安裝 `@nuxtjs/mdc` npm dep + lockfile sync
2. 加 Tauri command `read_tutorial_file` / `write_progress_file` + Rust path validation 紅隊測
3. 7 個新 component + 2 個 composable + 2 個 page 落地
4. grant.vue 修改 redirect target（從 `/workspace/scan` 改 `/tutorial/{workspace_id}/index`）
5. 跑 typecheck + dev HTTP 200 + grep enforce 三件驗收
6. manual smoke：開 demo workspace → grant → 切到 R-01 → 勾 Checkpoint → 答 Quiz → 走完一站 → 解鎖下一站 → progress.json 落地驗證

無 rollback 顧慮（純前端新功能，回滾即還原前一 commit）。

## Open Questions

以下問題在 propose 階段已決，列出避免後人重新質疑：

1. **Q：是否該為 R-01 新建 layout？** A：否，沿用 default layout，三段 grid 在 `<StationLayout>` component 內。
2. **Q：是否該把 station markdown 全部 prefetch 到 module-level cache？** A：否，per page mount lazy load；route.json 每次重讀。
3. **Q：是否該走 sidecar HTTP 讀檔？** A：否，新 Tauri command（process 邊界即 trust 邊界，無 bearer 開銷）。
4. **Q：是否該允許使用者直接編輯 markdown？** A：否，read-only；要更新教材走 Module 5 regenerate flow。
5. **Q：progress.json 是否該加版本號？** A：P0 不需（schema 還在演化階段，破壞性變更直接清檔即可，非生產級資料）。P1 引入 `schema_version: 1` 欄位。
6. **Q：Quiz 答錯 N 次後是否揭示正解？** A：P0 否；考慮 P1 加「可選看解析」按鈕。
7. **Q：是否需要 R-01 內 search 跨站搜尋？** A：否，P1 才做（本 change 拒絕加入避免 scope 膨脹）。
