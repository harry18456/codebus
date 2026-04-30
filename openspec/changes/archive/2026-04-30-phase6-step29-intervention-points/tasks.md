## 1. Foundation：`useIntervention` composable + confirm modal shell（Decision 4: useIntervention composable 的 state ownership）

- [x] 1.1 RED test：`web/tests/intervention/useIntervention.spec.ts` — 寫 vitest 測 module-level singleton 的 `pendingAction` 狀態機（initial null / `requestSkip(stationId)` 設為 `{ kind: 'skip', payload, onConfirm }` / `requestRegen(stationId)` / `requestSwitchWorkspace()` / `confirm()` 呼叫 onConfirm 後清空 / `cancel()` 直接清空）。3 種 kind 各 1 testcase + confirm/cancel 各 1，共 ≥ 5 case
- [x] 1.2 GREEN：實作 `web/app/composables/useIntervention.ts` — module-level singleton，輸出 `pendingAction: Ref<…|null>` + `requestSkip(payload)` / `requestRegen(payload)` / `requestSwitchWorkspace()` / `confirm()` / `cancel()` 五個方法。1.1 全綠（落地 design Decision 4: `useIntervention` composable 的 state ownership — singleton 同 `useQaSession` / `useExplorerStream` 慣例）
- [x] 1.3 RED test：`web/tests/intervention/InterventionConfirmModal.spec.ts` — render 測，斷言 `pendingAction === null` 時不渲染、3 種 kind 各 render 對應 copy（skip 含「跳過此站」、regen 含「重生會覆蓋」、switch 含「進度按 workspace 路徑分開保存」），confirm 按鈕 click 呼到 `useIntervention().confirm()`、cancel / dim layer 呼到 `cancel()`
- [x] 1.4 GREEN：實作 `web/app/components/intervention/InterventionConfirmModal.vue` — 訂閱 `useIntervention().pendingAction`，三種 kind 對應 v-if 分支渲染 3 套 copy + Cancel/Confirm 按鈕；dim layer click → cancel；aside click stopPropagation。1.3 全綠
- [x] 1.5 在 `web/app/layouts/default.vue` 樹底 mount `<InterventionConfirmModal />`（與既有 `<QAOverlay />` 同層；singleton modal 一份就夠）

## 2. 介入點 1 — Per-station skip（Modified Requirement: progress.json schema and single-writer path; Modified Requirement: Unlock logic gates next-station access on completion; Added Requirement: Skip station from station page marks station as skipped without completion; Decision 2: progress.json skipped_station_ids schema migration 與解鎖規則）

- [x] 2.1 RED test：`web/tests/composables/useTutorialProgress-skip.spec.ts` — 加 `skipped_station_ids` schema 測、初始 `[]`、`markStationSkipped(id)` append 進去、舊 progress.json 無此欄位讀為 `[]`、completed ∪ skipped 互斥（從 skipped 完成 → 移出 skipped 加進 completed 同一次寫入）。覆蓋 spec scenarios 的「missing skipped_station_ids field reads as empty list」「Mutual exclusion between completed and skipped enforced on transition」
- [x] 2.2 GREEN：改 `web/app/composables/useTutorialProgress.ts` — 加 `skipped_station_ids: string[]` 到 `TutorialProgress` interface + `emptyProgress()` 預設 `[]`；新方法 `markStationSkipped(stationId)` 進入 watch 觸發路徑；watch 守 mutual exclusion（從 skipped 完成 → atomic 寫入移出 + 加入 completed）。2.1 全綠（兌現 spec MODIFIED Requirement「progress.json schema and single-writer path」+ design Decision 2: `progress.json` `skipped_station_ids` schema migration 與解鎖規則）
- [x] 2.3 RED test：`web/tests/intervention/SkipStationButton.spec.ts` — render 測：`completed_station_ids` 含本站時 component 不渲染（DOM 為空）、`skipped_station_ids` 含本站時渲染但 click no-op + 顯示「本站已跳過」tooltip、never-visited click → 呼 `useIntervention().requestSkip(...)` 帶 stationId / stationTitle
- [x] 2.4 GREEN：實作 `web/app/components/intervention/SkipStationButton.vue` — props `{ stationId, stationTitle }`，computed `progress` snapshot 判定 render / no-op / interactive 三狀態。2.3 全綠
- [x] 2.5 RED test：`web/tests/intervention/skip-flow-integration.spec.ts` — flow 測：never-visited 站 click skip → modal open → confirm → progress.skipped_station_ids 增加 + `current_station_id` 清空 + navigate 到下一站；最後一站 skip → navigate 回 MOC（`/tutorial/{ws}`）
- [x] 2.6 GREEN：在 `useIntervention.requestSkip(...)` 的 `onConfirm` 串 `useTutorialProgress().markStationSkipped(...)` + `router.push(...)`；StationLayout 掛 `<SkipStationButton>` 到 header chrome。2.5 全綠（兌現 spec ADDED Requirement「Skip station from station page marks station as skipped without completion」end-to-end flow）
- [x] 2.7 改 `web/app/components/tutorial/StationNav.vue` — 已 skip 站視覺差異化（例：dimmed + ↷ icon）覆蓋 spec scenario「Skipped station revisitable via URL paste in normal mode」的 nav-side 標示
- [x] 2.8 改 `web/app/components/tutorial/MOCIndex.vue` — 已 skip 站徽章顯示「skipped」（與 completed / unlocked / locked 並列），對應 spec scenario「MOC visualizes unlock state per station」更新版
- [x] 2.9 RED test：解鎖規則更新 — `web/tests/composables/useStationRoute-unlock.spec.ts` 補測「Station skip unlocks the next station」+「Skipped station revisitable via URL paste in normal mode」+「Locked station URL paste shows lock screen」更新版（locked 條件變「不在 completed 也不在 skipped」）
- [x] 2.10 GREEN：改 unlock computed — `is_done(S, progress) = S.station_id ∈ completed ∪ skipped`；revisitability 預判同步加 skipped 那條。2.9 全綠（兌現 spec MODIFIED Requirement「Unlock logic gates next-station access on completion」新 algorithm 與 revisitability 規則）

## 3. 介入點 2 — Per-station regen（Modified Requirement: Generator entrypoint orchestrates per-station markdown pipeline; Added Requirement: Partial regen via target_stations preserves unrelated stations; Decision 1: target_stations partial regen 的覆寫範圍）

- [x] 3.1 RED test：`sidecar/tests/api/test_generate_target_stations.py` — pytest 測 `POST /generate` 帶 `target_stations: ["s99-not-real"]` 拒回 400 `GENERATE_TARGET_STATION_INVALID`、`target_stations: None`（default）走全 tutorial 路徑與既有測等價、`target_stations: ["s02-..."]` 走 partial 路徑
- [x] 3.2 RED test：`sidecar/tests/generator/test_runner_partial_regen.py` — pytest 測 `run_generator(..., target_stations=["s02-mqtt-client"])` over 既有 3 站 fixture 工作區，斷言：(a) 命中站 markdown 被覆寫、(b) `s01` / `s03` byte-identical 前後、(c) `tutorial.md` byte-identical、(d) `route.json` byte-identical、(e) `GeneratorResult.station_paths` length 1、(f) `generator_log.jsonl` 多一行 `mode="partial"`
- [x] 3.3 RED test：`test_runner_partial_regen.py` 加 station_id drift case — LLM 對命中站回傳 stable id 與 request 不符（slug 飄掉）→ runner 拒 `GENERATE_STATION_ID_DRIFT`、檔案 byte-identical、log 雙錄 requested + observed id、其他 target station 繼續處理（no short-circuit）
- [x] 3.4 GREEN sidecar：改 `sidecar/src/codebus_agent/api/generate.py` `GenerateRequest` 加 `target_stations: list[str] | None = None`、endpoint 收到 `target_stations` 時預檢 ids match `state.stations[*]` 才轉派、否則 raise 400 `GENERATE_TARGET_STATION_INVALID`
- [x] 3.5 GREEN sidecar：改 `sidecar/src/codebus_agent/generator/runner.py` `run_generator` 加 `target_stations` keyword-only 參數；`target_stations is None` 走原 full path（行為等價，既有測必須仍綠）；非空時走新 partial path：iterate 命中 stations、跑 `_generate_station` 得新 markdown → 守 stable_id == requested_id 否則 `GENERATE_STATION_ID_DRIFT`、過 Sanitizer Pass 1、覆寫對應檔案；不呼 MOC assembler 也不寫 route.json；log 用 `mode="partial"`。3.1 / 3.2 / 3.3 全綠（兌現 spec MODIFIED Requirement「Generator entrypoint orchestrates per-station markdown pipeline」加 `target_stations` keyword-only 參數 + ADDED Requirement「Partial regen via target_stations preserves unrelated stations」+ design Decision 1: `target_stations` partial regen 的覆寫範圍）
- [x] 3.6 RED test：`web/tests/intervention/RegenStationButton.spec.ts` — render 測（degraded / 已生成 兩種狀態都該渲染按鈕）、click → 呼 `useIntervention().requestRegen({ stationId, taskId, workspaceRoot })`
- [x] 3.7 GREEN：實作 `web/app/components/intervention/RegenStationButton.vue` — props `{ stationId, stationTitle, taskId, workspaceRoot }`。3.6 全綠
- [x] 3.8 RED test：`web/tests/intervention/regen-flow-integration.spec.ts` — flow 測：click regen → modal open → confirm → POST /generate 帶 `target_stations=[stationId]` + 解 task_id → useSseTask 接 SSE → 完成後 `useTutorialFiles` 重讀此站 markdown
- [x] 3.9 GREEN：在 `useIntervention.requestRegen(...)` 的 `onConfirm` 串 sidecar 呼叫（`useSidecar().fetch('/generate', { method: 'POST', body: { workspace_root, task, stations, target_stations: [stationId] } })`）+ `useSseTask` + 完成後 `useTutorialFiles().readTutorialFile()` 重讀。3.8 全綠
- [x] 3.10 在 `StationLayout.vue` header chrome 掛 `<RegenStationButton>`（與 `<SkipStationButton>` 並列）；degraded 狀態下視覺強調（與既有「本站產出失敗，請重跑」warning 並列）

## 4. 介入點 3 — Switch workspace（Added Requirement: TopBar workspace switcher offers safe relocation through confirm modal; Decision 3: Switch workspace 與 grant flow 的互動）

- [x] 4.1 RED test：`web/tests/intervention/SwitchWorkspaceMenu.spec.ts` — render 測：tutorial-level page mount 時 chip 渲染 workspace basename（不是 full path）、entry page `/` 與 grant page `/workspace/grant` mount 時 chip 不渲染（DOM 無相關節點）、chip click 開 dropdown、「🔁 換資料夾」select 呼 `useIntervention().requestSwitchWorkspace()`
- [x] 4.2 GREEN：實作 `web/app/components/intervention/SwitchWorkspaceMenu.vue` — 用 `useRoute()` 判斷當前 route name、僅 tutorial-level 路由 render；輸入 prop `workspaceRoot: string`、computed basename。4.1 全綠
- [x] 4.3 RED test：`web/tests/intervention/switch-flow-integration.spec.ts` — flow 測：confirm modal 文案含 (a) 進度保留 / (b) 需重新 grant / (c) 回頭認過會跳過 grant 三項；confirm → `router.push('/')`；on-disk 檔案無變動（mock fs assertions）；no `grant_revoked` event 寫入 audit log（mock IPC assertion）
- [x] 4.4 GREEN：在 `useIntervention.requestSwitchWorkspace()` 的 `onConfirm` 呼 `router.push('/')`；不執行任何 fs / IPC 副作用。4.3 全綠
- [x] 4.5 改 `web/app/components/TopBar.vue` 或新增 layout 對應位置 — mount `<SwitchWorkspaceMenu :workspace-root="..." />` 在 chip 區；workspaceRoot 從現有 layout context 拉（兌現 spec ADDED Requirement「TopBar workspace switcher offers safe relocation through confirm modal」chip render + dropdown 落點）

## 5. Spec scenarios coverage check（Modified Requirement: progress.json schema and single-writer path; Modified Requirement: Unlock logic gates next-station access on completion）

- [x] 5.1 跑 `cd web && npm run test` 全綠（既有 28+ test files / 140+ tests + 新增 intervention 系列 ≥ 12 測共 0 regression）
- [x] 5.2 跑 `cd sidecar && uv run pytest` 全綠（既有 ~885 passed + 新增 generator partial regen 測 ≥ 6 case 共 0 regression；尤其 module-5-generator 既有 `test_runner_*` 必須仍通）
- [x] 5.3 `cd web && npm run typecheck` 跑 `vue-tsc --build --noEmit` 全綠（守 `fix-phase7-typecheck-baseline` archive 立的 zero-error baseline）
- [x] 5.4 補 source-grep defensive 測：`web/tests/composables/useTutorialProgress-single-writer.spec.ts` 的既有「Single-writer invariant enforced by source grep」case 必須涵蓋新方法 `markStationSkipped` — 確認 `useIntervention.ts` 不直接 invoke `writeProgressFile`，要透過 `useTutorialProgress`

## 6. Manual e2e + 文件同步 + commit

- [x] 6.1 `cd web && npm run dev` 起 dev server，手動驗 3 條 flow：(a) 任一 station 點 skip → 跳到下一站、progress.json 多 `skipped_station_ids` 欄、再點開該站還能完成、完成後 skipped → completed 轉換正確；(b) 任一 station 點 regen → 觀察 SSE task 跑完、檔案內容更新、其他站檔案無動；(c) TopBar workspace chip → 換資料夾 → 回 entry page、新 workspace 走 grant flow / 舊 workspace skip grant
- [x] 6.2 `docs/decisions.md` D-020 加追記「[x] 2026-04-30 fix-phase6-step29-intervention-points archive — 已決定不另開 Module 6 capability，介入點以 3 個 MODIFIED capability + 1 個 useIntervention composable 落地」
- [x] 6.3 `docs/implementation-plan.md` step 29 標 ✅ landed（含 archive 名與日期）
- [x] 6.4 `docs/interactive-tutorial.md` 加 `progress.skipped_station_ids` 欄位說明 + skip flow 段；`docs/module-5-generator.md` 加 `target_stations` arg 與 partial regen 行為段
- [x] 6.5 `pre-commit run --all-files` 全綠後 commit
