## 1. Defensive test infra（RED first）

- [x] 1.1 對應 spec ADDED Requirement「Frontend typecheck baseline stays at zero errors」scenario「Defensive vitest test asserts zero typecheck errors」+「Defensive vitest test surfaces the offending diagnostic on regression」：寫 RED 測 `web/tests/types/typecheck-baseline.spec.ts`，用 `child_process.execSync('npx vue-tsc --build --noEmit 2>&1', { cwd: <web root>, encoding: 'utf-8' })` 跑（catch 非 0 exit），assert 1) exit code 0、2) stdout/stderr 不含 `error TS\d+:` regex 任何匹配；fail message 直接 echo 整份 vue-tsc 輸出供 debug。**`--build` 旗標必要**：`tsconfig.json` 是 project-references-only（`files: []`），無 `--build` 時 `vue-tsc -p .` 不會 traverse references、silently 檢查 0 檔案、~2 秒退場，等於沒測。**現況會 fail**（3 個 pre-existing：`QAOverlay.vue:29` × 2 + `sanitizer.vue:113`）

## 2. Fix the two pre-existing typecheck errors（GREEN for 1.1）

- [x] 2.1 [P] 改 `web/app/components/qa/QAOverlay.vue::sendDisabled` computed：把 `if (lastTurn.value === null) return false` 改成 `if (lastTurn.value == null) return false`（同時 narrow `null | undefined`）；不動 `lastTurn` computed 本身、不動其他 computed（`originStationId` / `addToKbCount` 等已 narrow OK）
- [x] 2.2 [P] 改 `web/app/pages/audit/sanitizer.vue`：把本地 `const showError = computed(...)` 改名為 `const hasError = computed(...)`，`v-else-if="showError"` 同步改 `v-else-if="hasError"`，避開與 Nuxt auto-import `showError` 全域函式（`.nuxt/types/imports.d.ts:96`，來自 `nuxt/dist/app/composables/error`）撞名 — 才是 TS2774 的根因。**runtime 行為不變**：仍是同一個 `audit?.error.value !== null && audit?.error.value !== undefined` 判斷。注意：原本 proposal/task 提的「v-else-if="showError.value"」修法經 apply 期間驗證**會觸發 TS2551**（Property 'value' does not exist on type 'boolean & ...'），不可採用

## 3. Regression（GREEN gate + scope check）

- [x] 3.1 確認 1.1 RED 測轉綠（`cd web && npm run test -- tests/types/typecheck-baseline.spec.ts`）
- [x] 3.2 `cd web && npm run typecheck` 直接跑全綠（mirror 1.1 內 spawn 的判斷，作為人類可讀的二次確認）
- [x] 3.3 `cd web && npm run test` 全綠（既有 27+ test files / 139+ tests + 新增 typecheck-baseline 共 0 regression；尤其 useQaSession / QAOverlay / QaTurnCard / QaCitations / sanitizer-page 既有測必須仍通）
- [x] 3.4 `cd web && npm run dev` 起 dev server，本地 curl `http://localhost:3000/` 與 `http://localhost:3000/audit/sanitizer`、`http://localhost:3000/tutorial/test-ws` 都拿 200，stdout 無 vue / vite warning（驗證兩支 .vue 改動沒在 SSR / hydration 路徑炸開）

## 4. 文件同步 + commit

- [x] 4.1 `docs/notes-2026-04-29-phase7-e2e-findings.md` A12 段標 `[x] 已修（2026-04-30，fix-phase7-typecheck-baseline）`，補一句指向 spec frontend-shell ADDED Requirement 鎖死 baseline + defensive vitest 測落點
- [x] 4.2 `pre-commit run --all-files` 全綠後 commit
