## 1. Tauri host：dialog plugin wiring

> Implements spec Requirement "Tauri host wires the dialog plugin".

- [x] 1.1 [Tauri host wires the dialog plugin] `tauri/src-tauri/Cargo.toml` 加 `tauri-plugin-dialog = "2"` dependency
- [x] 1.2 `tauri/src-tauri/src/lib.rs` builder 鏈加 `.plugin(tauri_plugin_dialog::init())`
- [x] 1.3 `tauri/src-tauri/capabilities/default.json` permissions array 加 `"dialog:default"`
- [x] 1.4 `tauri/src-tauri/tests/dialog_plugin_smoke.rs` 加 smoke：build app instance 不 panic、plugin handle 可拿；加進 `cargo test`

## 2. workspace_id 前後端 parity（TDD red）

> Implements design Decision 1: workspace_id 在前端 SHA-256 derive（與 sidecar parity）；以及 spec Requirement "Folder picker invocation flow" 的 deterministic-id scenario。

- [x] 2.1 [Folder picker invocation flow] 加 `web/tests/utils/workspace-id.spec.ts` 紅測：呼叫 `deriveWorkspaceId('/abs/path')` 應回 `ws_<12 hex>` 格式；同 path 兩次呼叫結果相同；mixed-case path 落回同一 id；用一組覆蓋 Windows backslash / posix slash / mixed case 的 fixture path（4 條 case）對照 sidecar `auth.service.workspace_id_for_path` 的 expected hash（fixture 寫成 `(input, expected)` table，expected 從 sidecar 端產出）
- [x] 2.2 跑 `cd web && npm run test -- tests/utils/workspace-id` 確認紅
- [x] 2.3 加 `web/app/utils/workspace-id.ts` export `deriveWorkspaceId(absolutePath: string): string`：用 Web Crypto API SHA-256 + 取前 12 hex + prefix `ws_`；canonical 規則 = 全轉小寫 + posix slash
- [x] 2.4 跑 task 2.1 case 確認綠
- [x] 2.5 加 sidecar parity test `sidecar/tests/auth/test_workspace_id_parity.py`：用同一組 fixture path，斷言 sidecar `workspace_id_for_path` 結果與 task 2.1 frontend test 的 expected 完全相同（避免演算法 drift）；同步跑 `cd sidecar && uv run pytest tests/auth/test_workspace_id_parity.py` 全綠

## 3. useWorkspaceOnramp composable（TDD red）

> Implements spec Requirement "Workspace onramp drives scan, kb-build, explore, then generate via SSE" 與 spec Requirement "Onramp state survives navigation away from entry page"；對應 design Decision 3 (state survives navigation) / Decision 5 (4-step pipeline behind 2 clicks) / Decision 6 (`ONRAMP_DEFAULT_TASK` constant)。

- [x] 3.1 [Workspace onramp drives scan, kb-build, explore, then generate via SSE] [Onramp state survives navigation away from entry page] 加 `web/tests/onramp/useWorkspaceOnramp.spec.ts` 紅測：(a) initial state phase=`idle` / workspaceId=null；(b) `start(path)` 呼 `deriveWorkspaceId` + POST `/scan?stream=true` 帶 `{ workspace_root, workspace_type:'folder' }` + 訂 SSE，phase 進 `scanning`；(c) scan SSE `done` → composable 自動 GET `/tasks/<scan_task_id>/result` → POST `/kb/build` 帶 `{ workspace_root, scan_result }` + 訂新 SSE，phase 進 `indexing`；(d) kb-build SSE `done` → phase `scan-complete`，**未** 觸發 explore；(e) `triggerGenerate()` 呼 POST `/explore` 帶 `{ workspace_root, task: ONRAMP_DEFAULT_TASK }` + 訂 SSE，phase 進 `exploring`；(f) explore SSE `done` → composable 自動 GET `/tasks/<explore_task_id>/result` → POST `/generate` 帶 `{ workspace_root, task: ONRAMP_DEFAULT_TASK, stations: <ExplorerState.stations> }` + 訂 SSE，phase 進 `generating`；(g) generate SSE `done` → phase `ready`；(h) 任一 in-flight task SSE `error` event → phase `error` + error message 留在 state；(i) `retry()` re-issue 上一次失敗的 POST（不回退到更早 phase）；(j) module-level singleton — 兩次 `useWorkspaceOnramp()` 拿同一 ref；(k) `ONRAMP_DEFAULT_TASK` 是 exported constant 等於 `"認識整個 codebase"`
- [x] 3.2 跑 task 3.1 確認紅
- [x] 3.3 加 `web/app/composables/useWorkspaceOnramp.ts`：(a) module scope state（phase / workspaceId / pickedPath / progressEvents / errorMsg / activeTaskId / scanResult / explorerState）；(b) export `ONRAMP_DEFAULT_TASK = '認識整個 codebase'`；(c) `start(path)` 算 id → POST `/scan?stream=true` → `useSseTask` subscribe → 監聽 `done` event → 自動 GET `/tasks/<id>/result` → POST `/kb/build` → 切 `useSseTask` 訂新 task → on `done` 進 `scan-complete`；(d) `triggerGenerate()` POST `/explore` → `useSseTask` subscribe → 監聽 `done` → 自動 GET `/tasks/<id>/result` → POST `/generate` → 切 `useSseTask` 訂新 task → on `done` 進 `ready`；(e) SSE error → phase `error` 帶 errorMsg；(f) `retry()` 看哪個 active POST 失敗就 re-issue 哪個；(g) chain 切換 SSE 時必須先 close 舊的 useSseTask 避免 EventSource leak（mirror `useQaSession::disposeSse`）
- [x] 3.4 跑 task 3.1 case 確認全綠

## 4. UI 元件（前端 view-level）

> Implements spec Requirement "Entry page exposes folder-picker workspace onramp"；對應 design Decision 2: SSE progress 用新元件 `<OnrampProgress>`，不重用 `<ProgressStrip>` 與 design Decision 4: generate 完成後顯示「進入 tutorial」按鈕，不自動 navigate。

- [ ] 4.1 [P] [Entry page exposes folder-picker workspace onramp] 加 `web/app/components/workspace-onramp/FolderPickerButton.vue`：button click → `import('@tauri-apps/plugin-dialog').then(m => m.open({ directory: true, multiple: false }))`；non-Tauri 環境（vitest）走 inject mock；emit `picked(path)` event；button 文字「+ 開新 codebase」zh-TW
- [ ] 4.2 [P] 加 `web/tests/onramp/FolderPickerButton.spec.ts`：mock dialog `open` 回 `'/some/path'` → emit `picked` 帶該 path；mock 回 `null`（user cancel）→ NOT emit；button label 包含「開新 codebase」
- [ ] 4.3 [P] 加 `web/app/components/workspace-onramp/OnrampProgress.vue`：props `phase: string` + `events: ProgressEvent[]`；render phase label（zh-TW，覆蓋 `scanning` / `indexing` / `exploring` / `generating` 四種 in-flight phase）+ throughput counter（從 events 拆 `current` / `total` / `current_file` 等 sidecar 共通 progress 欄）+ elapsed timer（轉動秒數）
- [ ] 4.4 [P] 加 `web/tests/onramp/OnrampProgress.spec.ts`：phase=`scanning` + event `{type:'progress', current:42, total:120, phase:'scanning'}` → DOM 含「掃描中」+ `42`；phase=`indexing` + event `{type:'progress', current:30, total:120, phase:'embedding'}` → DOM 含「建立索引中」+ `30`；phase=`exploring` + event `{type:'agent_thought', step:3}` → DOM 含「探索中」+ `step 3`；phase=`generating` + event `{type:'progress', current:2, total:5, phase:'generating'}` → DOM 含「產生教學中」+ `2`
- [ ] 4.5 加 `web/app/components/workspace-onramp/WorkspaceOnrampCard.vue`：props 從 `useWorkspaceOnramp()` 取；按 phase 切 render 分支：(idle) 顯示 picker prompt；(scanning / indexing / exploring / generating) 顯示 `<OnrampProgress>`；(scan-complete) 顯示「+ 產生 tutorial」按鈕（`data-testid="onramp-generate-cta"`）；(ready) 顯示「進入 tutorial」`<NuxtLink>`（`data-testid="onramp-enter-tutorial"`，`to=/tutorial/<workspaceId>`）；(error) 顯示 errorMsg + 「重試」按鈕（`data-testid="onramp-retry"`）；workspaceId + path tail 在所有非 idle phase 都可見
- [ ] 4.6 加 `web/tests/onramp/WorkspaceOnrampCard.spec.ts`：(a) phase=`scan-complete` → generate cta 渲染 + 點擊呼 `triggerGenerate`；(b) phase=`ready` workspaceId='ws_abc123def456' → enter-tutorial anchor href=`/tutorial/ws_abc123def456`；(c) phase=`error` errorMsg='oops' → DOM 含 'oops' 與 retry 按鈕；(d) error 階段 retry 點擊呼 `retry()`

## 5. Entry page 重寫

> Implements spec Requirement "AppShell ping-smoke placeholder is removed"。

- [ ] 5.1 改 `web/app/pages/index.vue`：保留 onMounted 的 `/healthz` 導 `/onboarding/welcome` 邏輯；checked=true 後 mount `<WorkspaceOnrampCard />` 與 `<FolderPickerButton @picked="onramp.start($event)" />` 取代 `<AppShell />`；layout 與既有 onboarding 頁一致（flex 容器、置中、padding）
- [ ] 5.2 改 `web/tests/onboarding/index-page-redirect.spec.ts`（既有 file）：所有 assertion 仍跑（onboarding redirect 邏輯不改）；新增一條 case：兩 lane ready → render `[data-testid="onramp-folder-picker"]` 而非 `<AppShell>`
- [ ] 5.3 [AppShell ping-smoke placeholder is removed] 刪除 `web/app/components/AppShell.vue`；repo-wide grep 確認沒有其他 import / 引用
- [ ] 5.4 跑 `cd web && npm run test --silent --run` 全綠（既有 242 + 本 change 新增 ~25 case）

## 6. End-to-end smoke + 文件 + baseline

- [ ] 6.1 跑 `cd web && npm run typecheck` zero error baseline 守住
- [ ] 6.2 跑 `cd sidecar && uv run pytest -q` baseline 全綠（sidecar 端只新增 task 2.5 一個 parity test）
- [ ] 6.3 重打 PyInstaller binary：`cd sidecar && uv run pyinstaller codebus-sidecar.spec --noconfirm`（雖然 sidecar code 沒改，但保險起見統一 binary 來源 — 加 parity test 的測試 fixture 不影響打包，可選）
- [ ] 6.4 重啟 cargo tauri dev → 走整條 onramp：(a) 完成 onboarding（如果還沒）→ entry page → 點「+ 開新 codebase」→ native picker 選一個小 codebase → scan progress 跑完 → 點「+ 產生 tutorial」→ generate progress 跑完 → 點「進入 tutorial」→ 進到 R-01 station board
- [ ] 6.5 完成後手動驗：D-033 B task 12.4 (b) chat hot-swap — 在 settings 改 chat binding → 回 station 跑一次 Q&A → 開 `<workspace>/.codebus/llm_calls.jsonl` 看新 model 欄已切換；驗 (c) embed hot-swap — 在 settings 改 embed → confirm modal → KB rebuild → 期間 `/qa` 503 → 完成後 Q&A 恢復
