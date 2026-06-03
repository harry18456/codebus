## 1. i18n 範例與標籤文案

- [x] 1.1 在 `codebus-app/src/i18n/messages.ts` 的 en 與 zh 兩份 bundle，把 `workspace.goals.examplePlaceholder1..3` 內容換成定稿通用範例（1 `describe what this project does`／說明這個專案在做什麼、2 `list the key dependencies and frameworks`／列出主要依賴套件與框架、3 `summarize the main features`／整理主要功能），並新增 `workspace.goals.examplePlaceholder4`（`map the project structure`／畫出專案結構）與 `workspace.goals.quickStartLabel`（`Quick start`／快速開始）。完成標準：5 個 key 在 en/zh 皆存在且為非空字串。驗證：`npm run test -- workspace.test`（i18n 完整性測試綠）。
- [x] 1.2 在 `codebus-app/src/i18n/workspace.test.ts` 的 `WORKSPACE_KEYS` 補入 `workspace.goals.examplePlaceholder4` 與 `workspace.goals.quickStartLabel`，使完整性測試實際守住新 key 的 en/zh 對齊。完成標準：兩 key 出現在 `WORKSPACE_KEYS`。驗證：`npm run test -- workspace.test` 對新 key 產生 en/zh 兩條 case 且通過。

## 2. GoalsTab 常駐 quick-start chips 與 4 例

- [x] 2.1 在 `codebus-app/src/components/workspace/GoalsTab.tsx` 把 `GOAL_EXAMPLE_KEYS` 由 3 個擴為 4 個（加入 `workspace.goals.examplePlaceholder4`），使空狀態 pills 變為 4 顆（testid `goals-empty-prefill-0..3`），所有 label 仍來自 `t(key)` 不得 hard-code 字面值。完成標準：空狀態渲染 4 顆 pill 且文案為新 i18n 值。驗證：更新後 `GoalsTab.test.tsx` 空狀態測試綠。
- [x] 2.2 在 `GoalsTab.tsx` 的非空分支（`goalRuns.length > 0`），於 `RECENT` `SectionLabel` 之上插入常駐 quick-start 區塊：容器 testid `goals-quickstart`、一個 `SectionLabel variant="caps"` 顯示 `t("workspace.goals.quickStartLabel")`、後接 4 顆 chip 按鈕（testid `goals-quickstart-chip-0..3`），label 來自同一 `GOAL_EXAMPLE_KEYS`，`onClick` 走既有 `openModalWith(t(key))`。複用 `Button`/`SectionLabel`，不新造抽象。此 2.1 與 2.2 共同落實 spec requirement `Goals Overview List and Filter` 修改後的 empty-state 與 populated-state 行為。完成標準：有 goal 時 RECENT 上方顯示 4 顆 chip，點擊開 NewGoalModal 並帶對應 prefill。驗證：新增的 populated-state 測試綠（見 3.1）。

## 3. 測試更新

- [x] 3.1 更新 `codebus-app/src/components/workspace/GoalsTab.test.tsx`：(a) 把空狀態既有斷言改為 4 顆 pill 且文案為新通用範例（en 與 zh literal）；(b) 新增 populated-state 測試——render 帶至少一筆 goal run，斷言 `goals-quickstart` + 4 顆 `goals-quickstart-chip-*` 存在、`quickStartLabel` 顯示、點第一顆 chip 開 `new-goal-modal` 且 textarea 值等於第一個範例；(c) zh locale 下 chips 與 pills 皆不得出現英文範例字面值（如 `describe what this project does`）。完成標準：上述行為皆有對應斷言。驗證：`npm run test -- GoalsTab` 全綠。

## 4. 驗收

- [x] 4.1 跑 `npm run typecheck` 與 `npm run test`（Vitest）全綠，確認純前端 + i18n 改動無型別錯誤、所有更新/新增測試通過。完成標準：兩指令皆 0 exit。驗證：終端輸出無錯誤。
