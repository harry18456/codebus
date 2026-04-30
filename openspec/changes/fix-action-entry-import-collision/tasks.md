## 1. Defensive test infra（RED first）

- [ ] 1.1 [P] 對應 spec Requirement「`useQaSession` is a module-level singleton with one SSE dispatch entry」MODIFIED scenario「ActionEntry is imported from canonical type module — no duplicate export warning」：寫 RED 測 `web/tests/types/agent-action.spec.ts::test_action_entry_single_source`，用 fs.readFileSync source-grep 鎖死 `export interface ActionEntry` 只在 `web/app/types/agent-action.ts` 出現、且 `web/app/composables/useQaSession.ts` 與 `web/app/composables/useExplorerStream.ts` 兩支都不出現該字串。**現況會 fail**（兩 composable 都還 export）
- [ ] 1.2 [P] 寫 RED 測 `web/tests/types/agent-action.spec.ts::test_action_entry_shape_invariant`：`import type { ActionEntry } from '~/types/agent-action'`，用 `expectTypeOf<ActionEntry>().toEqualTypeOf<{tool:string; observation:string; tokens_used:number; isError:boolean}>()`（vitest type-test API）守 Non-Goals「不變更 schema」。**現況會 fail**（檔案不存在）

## 2. Canonical type module landing（GREEN for 1.2）

- [ ] 2.1 新增 `web/app/types/agent-action.ts`：唯一 `export interface ActionEntry { tool: string; observation: string; tokens_used: number; isError: boolean }` + JSDoc cite `openspec/specs/qa-overlay/spec.md` reactSteps[].actions[] 引用、`openspec/specs/agent-console/spec.md` stepBuckets[].actions[] 引用、本 change 名
- [ ] 2.2 確認 1.2 RED 測轉綠（`vitest run web/tests/types/agent-action.spec.ts -t shape_invariant`）

## 3. Composable rewire（GREEN for 1.1 + 解 Nuxt duplicate-export warning）

- [ ] 3.1 改 `web/app/composables/useQaSession.ts`：刪 line 26 起 6 行 `export interface ActionEntry { ... }` block + 在 import block 加 `import type { ActionEntry } from '~/types/agent-action'`
- [ ] 3.2 改 `web/app/composables/useExplorerStream.ts`：刪 line 16 起 6 行 `export interface ActionEntry { ... }` block + 同加 `import type { ActionEntry } from '~/types/agent-action'`
- [ ] 3.3 確認 1.1 RED 測轉綠（`vitest run web/tests/types/agent-action.spec.ts -t single_source`）

## 4. Downstream import audit（多數走 Nuxt auto-import，預期 no-op）

- [ ] 4.1 [P] grep `web/app/components/qa/` 三支 vue 是否含顯式 `import { ActionEntry }` 或 `import type { ActionEntry }` from composable 路徑；有則改指 `~/types/agent-action`，無則 no-op
- [ ] 4.2 [P] 同上 grep `web/app/components/console/` 四支 vue（`ConsoleTimeline` / `StepCard` / `ProgressStrip` / `CoverageBanner`）
- [ ] 4.3 [P] 同上 grep `web/tests/qa/` 與 `web/tests/composables/` 內所有 `.spec.ts`；有顯式 import `ActionEntry` 的測檔改指 `~/types/agent-action`

## 5. Regression + dev server smoke

- [ ] 5.1 `cd web && npm run typecheck` 全綠（既有 4 + 7 個 vue + 多支 composable / test 全部 type-check 通過 single source）
- [ ] 5.2 `cd web && npm run vitest` 全綠（既有 useQaSession / QAOverlay / QaTurnCard / QaCitations / useExplorerStream / console 元件 / 新增 agent-action 測共 0 regression）
- [ ] 5.3 起 `cargo tauri dev`（從 repo root），觀察 Nuxt 啟動 stdout 不再印 `Duplicated imports "ActionEntry"` warning；R-01 station page 開啟、Q&A drawer 召喚（Cmd+K）、Explorer console 進站均無 type error，符合 spec scenario「no duplicate-export warning」

## 6. 文件同步 + commit

- [ ] 6.1 `docs/notes-2026-04-29-phase7-e2e-findings.md` A8 段標 `[x] 已修（2026-04-30，fix-action-entry-import-collision）`，補一句「兩 type 結構同（grep 確認），real fix 是 DRY 化抽 `web/app/types/agent-action.ts`，spec qa-overlay §55 cross-reference 同步」
- [ ] 6.2 `pre-commit run --all-files` 全綠後 commit
