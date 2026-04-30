## Why

D-020（2026-04-17）把「Module 6 介入控制器 spec」延後到前端實作階段「介面契約自然浮現時」再寫；現在 Phase 6 step 29 是 Module 6 的落地節點（`docs/implementation-plan.md §二第六階段`），R-01 / 28 Explorer console / 30 Q&A drawer 都已上線，三個介入點的 UX 接點都長出來了。README §四 MVP 把三介入點列為「demo 神器」必備（`docs/implementation-plan.md` M6 milestone 標明「三介入點可用」是 demo ready 條件之一）；不做這步，使用者卡在某站不滿意 → 唯一出路是關掉 app，落差很大。

## What Changes

- **介入點 1：路線調整（per-station skip）** — `web/app/pages/tutorial/[workspace_id]/[station_id].vue` 站牌頁加「↷ 跳過此站」按鈕；按下後 progress.json 多一個 `skipped_station_ids: string[]` 欄位（schema additive，不破舊 progress 檔案）；MOC 首頁與 station nav 的解鎖 / 完成判定仍以 `completed_station_ids ∪ skipped_station_ids` 為準（已會的、跳過的同樣解鎖後續站）。**Reorder / 加自訂站留 P1**（per AskUserQuestion 對齊「只 skip 最小推薦」）。
- **介入點 2：per-station 重生** — `POST /generate` 既有 endpoint 加 optional `target_stations: list[str] | None = None`（default `None` 走全 tutorial 重生，與現況等價）；前端 station 頁 header 加「↻ 重生此站」按鈕，按下走 confirm modal「重生會覆蓋此站 markdown 與 frontmatter」→ 觸發 `POST /generate` 帶 `target_stations=[station_id]` → 復用 `useSseTask` 顯示 task progress → 完成後 `useTutorialFiles` 重新讀取此站檔案。**全 tutorial 重生不需要新 UI**（現況：在 `/explorer/[task_id]` 重輸入 task 即可）。
- **介入點 3：換資料夾** — `web/app/components/TopBar.vue` 加 workspace chip + 下拉選單，含「🔁 換資料夾」項；按下走 confirm modal「進度按 workspace 路徑分開保存，回來後會延續」→ `router.push('/')` 回到 entry page 重新走 grant flow。**進度不刪不轉移**（既有 `<ws>/codebus-tutorials/{task_id}/progress.json` 完整保留，下次同 workspace 直接接續）。
- **新增 composable** `web/app/composables/useIntervention.ts`：三個介入點共用的 confirm-modal 觸發 / state 管理 / SSE 接力；不引入 Pinia（implementation-plan 提到的「Pinia + 既有 API」hint 已過時，現網狀 composable + module-level singleton 慣例覆蓋同樣需求 — `useQaSession` / `useTutorialProgress` / `useExplorerStream` 三支已驗證 pattern）。
- **新增元件** `web/app/components/intervention/InterventionConfirmModal.vue` / `SkipStationButton.vue` / `RegenStationButton.vue` / `SwitchWorkspaceMenu.vue`：四個 leaf 元件，依 dumb + emit pattern（與 Phase 6 既有元件約定一致）。
- **defensive 測**：vitest unit 測 `useIntervention` 三條 flow（skip → progress.json 寫入、regen → POST /generate target_stations、switch → router push + modal）+ 元件 render 測 + sidecar pytest 測 `target_stations` arg 的 partial regen 路徑（既有 station files 不被誤刪、unrelated stations 不被改寫）。

## Non-Goals

- **不做 reorder / drag-drop 重排站牌順序** — UX 複雜度 +0.5–1d、超出 D-020 「MVP 介入」範圍；P1 follow-up。
- **不做新增自訂站功能** — 需要手動 station id stable mapping、與 generator station_id discriminator 衝突風險高；P1+ 評估。
- **不做 station-level rollback / undo skip** — 跳過的站可重新點開，但「取消跳過」需要 progress schema 第二輪設計；本 change 一律前向（skip 後仍可重新點開站學）。
- **不做 KB 管理介入點**（README §五「介入點 A：知識庫管理」）— 那是 Phase 2 KB ops UI 範疇（per `docs/qa-agent.md §十` Q&A P1+ defer 清單），本 change 純前端互動 + `/generate` 既有 endpoint 擴充。
- **不引入 Pinia** — 沿用現有 composable + module-level singleton 慣例（D-026 npm + Vue 3 + `<script setup>`）。
- **不改 D-020 ADR 結論** — 仍維持「Module 6 = 前端組合、不獨立成 backend module」；本 change 唯一 backend touch 是擴 `/generate` optional arg。

## Capabilities

### New Capabilities

（無）

### Modified Capabilities

- `interactive-tutorial`：ADD `progress.json` 的 `skipped_station_ids: string[]` 欄位 + skip station 行為 scenarios + station unlock 規則更新（已完成的、跳過的並列）
- `module-5-generator`：ADD `POST /generate` `target_stations: list[str] | None` arg + partial regen 行為 scenarios（unrelated station files 不變、frontmatter / MOC 對應行只改命中站）
- `frontend-shell`：ADD TopBar workspace switcher 入口 + confirm-then-navigate 行為 scenarios

## Impact

- Affected specs:
  - openspec/specs/interactive-tutorial/spec.md（MODIFIED：progress schema + skip behavior）
  - openspec/specs/module-5-generator/spec.md（MODIFIED：target_stations arg + partial regen invariants）
  - openspec/specs/frontend-shell/spec.md（MODIFIED：TopBar workspace switcher）
- Affected code:
  - New:
    - web/app/composables/useIntervention.ts
    - web/app/components/intervention/InterventionConfirmModal.vue
    - web/app/components/intervention/SkipStationButton.vue
    - web/app/components/intervention/RegenStationButton.vue
    - web/app/components/intervention/SwitchWorkspaceMenu.vue
    - web/tests/intervention/useIntervention.spec.ts
    - web/tests/intervention/SkipStationButton.spec.ts
    - web/tests/intervention/RegenStationButton.spec.ts
    - web/tests/intervention/SwitchWorkspaceMenu.spec.ts
    - sidecar/tests/api/test_generate_target_stations.py
  - Modified:
    - web/app/composables/useTutorialProgress.ts（加 skipped_station_ids 欄位 + 解鎖規則）
    - web/app/components/tutorial/StationLayout.vue（站牌頁 header 掛 SkipStationButton + RegenStationButton）
    - web/app/components/tutorial/StationNav.vue（已跳過站的視覺差異化）
    - web/app/components/tutorial/MOCIndex.vue（skip 標記顯示）
    - web/app/components/TopBar.vue（workspace chip + dropdown）
    - sidecar/src/codebus_agent/api/generate.py（GenerateRequest 加 target_stations）
    - sidecar/src/codebus_agent/generator/runner.py（partial regen 分派路徑）
  - Removed:（無）
- Affected docs:
  - Modified:
    - docs/decisions.md（D-020 加「[x] 前端實作階段已決定 — 不另開 Module 6 capability，3 MODIFIED 收敘述」追記）
    - docs/implementation-plan.md（step 29 標 ✅ landed）
    - docs/interactive-tutorial.md（progress schema + skip flow）
    - docs/module-5-generator.md（target_stations arg）
