<!--
Each task description MUST state the behavior delivered and the verification
target. File paths are locator context, not the task itself.
-->

## 1. Interrupted Run Detection 判據修正 — 後端（TDD）

- [x] 1.1 在 codebus-app/src-tauri/src/ipc/goals.rs 新增失敗測試 `list_runs_skips_non_goal_orphan_events`：寫入一個無 `VerbBanner::Goal` 事件的孤兒 `events-*.jsonl`（模擬進行中的 chat / query / fix / quiz），呼叫 `list_runs_impl(.., ModeFilter::Goal)` 與 `ModeFilter::All`，斷言回傳清單**不含**該 slug 對應的任何 entry（既不合成 `interrupted`、也不含空 goal row）。驗證：`cargo test -p codebus-app list_runs_skips_non_goal_orphan_events` 先 RED。
- [x] 1.2 修改 `list_runs_impl` 的孤兒 events 合成迴圈，落實 `Interrupted Run Detection` 需求的 goal-only 判據：合成虛擬 `interrupted` entry 前，先判斷該 events 檔開頭事件是否含 `VerbBanner::Goal`（沿用掃描開頭視窗的 `first_goal_text_in_events`，其回傳 `Some` 即視為 goal run）。僅當為 goal run 時才合成 `mode="goal"`、`goal` 取自該 banner 的 entry；否則略過不合成。行為：非 goal verb 的孤兒 events 不再出現在 `list_runs` 回應。驗證：1.1 測試轉 GREEN。
- [x] 1.3 新增/保留迴歸測試 `list_runs_synthesizes_interrupted_only_for_goal_events`：goal 孤兒 events（含 `VerbBanner::Goal` 且 goal_text 非空）仍合成 `outcome="interrupted"`、`mode="goal"`、`goal` 等於 banner 文字；既有 `list_runs_synthesizes_interrupted_virtual_entry` 與 real-row-supersedes 行為不回歸。對應 `Interrupted Run Detection` 需求的三個 scenario。驗證：`cargo test -p codebus-app --lib ipc::goals` 全綠。

## 2. 進行中 goal 列表顯示修正 — 前端（TDD）

- [x] 2.1 在 codebus-app/src/store/goals.test.ts 新增失敗測試：當 `list_runs` 回傳的清單內含與當前 `activeRun.runId` 同 id 但 `outcome="interrupted"` 的磁碟合成紀錄時，呼叫 `refreshRuns` 後斷言 store 的 `runs` 中該 id 的列為 `outcome="running"`（樂觀進行中狀態），且 `goal` / `started_at` 取自 `activeRun`。驗證：`pnpm --filter codebus-app test goals` 先 RED。
- [x] 2.2 修改 `useGoalsStore.refreshRuns`：以 `list_runs` 結果覆寫 `runs` 時，若 `activeRun` 非 null，將回傳清單中 `run_id === activeRun.runId` 的列替換為由 `activeRun` 衍生的 `running` 摘要（清單中不存在時則 unshift 一筆），使進行中的 goal 維持 `🚌 running` 顯示而非被磁碟合成的 `⚠ interrupted` 覆蓋。行為：使用者自行啟動、仍在執行的 goal 切回 Goals 列表時不再顯示為中斷。驗證：2.1 測試轉 GREEN，且既有 goals store 測試（`pnpm --filter codebus-app test goals`）不回歸。
