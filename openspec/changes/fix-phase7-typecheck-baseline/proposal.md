## Why

`fix-action-entry-import-collision` task 5.1（2026-04-30）跑 `cd web && npm run typecheck` 時發現 baseline 已有 3 個 pre-existing TS 錯誤，與 ActionEntry collision 無關，已被記錄為 phase 7 e2e finding A12（`docs/notes-2026-04-29-phase7-e2e-findings.md`）。為保持 fix-action-entry-import-collision scoped、單一 change 單一焦點，留到本 change 處理。對齊 D-026（npm + Nuxt 4 + TypeScript 前端慣例）— `vue-tsc` 全綠是 frontend gate 的一部分。

3 個錯誤：

1. `web/app/components/qa/QAOverlay.vue:29`（兩處同行）— `lastTurn.value` possibly `undefined`（TS18048）。
   - 來源 archive：`qa-overlay-p0`（commit `0cbacac`，2026-04-29）。
2. `web/app/pages/audit/sanitizer.vue:113` — `v-else-if="showError"` 觸發 TS2774（`This condition will always return true since this function is always defined. Did you mean to call it instead?`）。
   - 來源 archive：`sanitizer-audit-inspector-p0`（commit `a26024c`，2026-04-29）。
   - 真因：本地 `const showError = computed(...)` 與 Nuxt auto-import 全域函式 `showError`（`.nuxt/types/imports.d.ts:96`，來自 `nuxt/dist/app/composables/error`）撞名；template 型別推論看到 always-defined 的 auto-import 函式才報 TS2774。

這些錯誤兩支 archive 各自 apply 期間沒被 typecheck 攔下，留到 phase 7 e2e + 後續 change 才暴露 — 對齊 phase 7 collateral findings 的處理節奏（A8 已修，A12 接續）。

額外 finding（2026-04-30 apply 期間發現）：原本 spec/proposal 提的指令 `npx vue-tsc --noEmit -p .` 因 `web/tsconfig.json` 是 project-references-only（`files: []` + 4 個 references）、無 `--build` 旗標時不會 traverse references，跑 ~2 秒 0 錯但**等於沒檢查**。`nuxt typecheck` 內部會走 build mode 才抓得到。本 change 統一改用 `vue-tsc --build`（13 秒實檢，與 `nuxt typecheck` 等價），確保 defensive 測有效。

## What Changes

- 修 `web/app/components/qa/QAOverlay.vue` `sendDisabled` computed：把 `if (lastTurn.value === null) return false` 改成 `if (lastTurn.value == null) return false`（同時 narrow `null | undefined`），讓後續存取 `.status` 通過 TS strict narrowing。**行為不變**：`lastTurn.value` 為 `null` 或 `undefined` 時都應該讓 send 按鈕可按（陣列空就可發第一條）。
- 修 `web/app/pages/audit/sanitizer.vue`：把本地 `const showError = computed(...)` 改名為 `const hasError = computed(...)`，`v-else-if="showError"` 同步改 `v-else-if="hasError"`，避開與 Nuxt auto-import `showError` 全域函式的撞名。**行為不變**：仍是同一個 `audit?.error.value !== null && audit?.error.value !== undefined` 判斷。
- 新增 defensive 測 `web/tests/types/typecheck-baseline.spec.ts`：用 `child_process.execSync` 跑 `npx vue-tsc --build --noEmit` 並 assert 0 typecheck error，鎖死 frontend baseline 全綠 — 未來任何 archive land 過程引入新 typecheck error 都會被測抓到。`--build` 旗標讓 vue-tsc 走 project-references build mode，與 `nuxt typecheck` 等價；無 `--build` 時 `vue-tsc -p .` 會因 `tsconfig.json` 的 `files: []` 直接退場、不檢查任何檔案。

## Non-Goals

- **不擴大重構 `QAOverlay.vue` `lastTurn` computed 的回傳型別**（保留 `T | null` 介面，只在消費端 narrow）— 變更介面會牽動 `originStationId` / `addToKbCount` 等其他 computed，超出 typecheck cleanup 範圍。
- **不改 vue-tsc 設定 / tsconfig.json**（如關掉 `noUncheckedIndexedAccess` / 換 strictness）— 維持目前嚴格度，只修觸發位點。
- **不重寫 `sanitizer.vue` 的 audit-loading state machine**（loading/error/empty/list 分支）— 只 rename 一個 computed 變數本身、不動 reactive 流。
- **不解 phase 7 其他 findings**（A9 Tauri stderr、A11 demo-synthetic fixture 等各自開 change）。
- **不 backport defensive typecheck 測到 sidecar Python pyright** — 純前端 scope。

## Capabilities

### New Capabilities

（無）

### Modified Capabilities

- `frontend-shell`：ADD Requirement「Frontend typecheck baseline stays at zero errors」— 鎖死 `vue-tsc --noEmit -p .` 必須 exit 0，並透過新 defensive 測 `web/tests/types/typecheck-baseline.spec.ts` 把這個 invariant 帶進 vitest 標準 run（既有 `qa-overlay` 與 `sanitizer-audit-inspector` 的 Requirement / Scenario 措辭與既有 invariant 完全保留 — 兩支 .vue 檔的修正屬實作層）。

## Impact

- Affected specs：openspec/specs/frontend-shell/spec.md（ADDED 1 個 Requirement + 2 個 Scenarios）
- Affected code：
  - Modified：
    - web/app/components/qa/QAOverlay.vue
    - web/app/pages/audit/sanitizer.vue
  - New：
    - web/tests/types/typecheck-baseline.spec.ts
  - Removed：（無）
- Affected docs：
  - Modified：docs/notes-2026-04-29-phase7-e2e-findings.md（A12 段補 `[x] 已修（2026-04-30，fix-phase7-typecheck-baseline）`）
