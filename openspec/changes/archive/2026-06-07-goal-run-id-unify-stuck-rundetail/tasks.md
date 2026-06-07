## 1. core: Goal Verb Library Function

- [x] [P] 1.1 為 design 決策「run_goal 簽章新增 run_started_at 參數（Some 採用 / None 內部派生）」在 `codebus-core/src/verb/goal.rs` 測試模組新增失敗測試（RED）：`Some(s)` 時 events 檔名 slug 與 `RunLog.started_at` 等於 `s`、`None` 時內部以 `Millis` 派生。驗證：新測試以 `cargo test -p codebus-core run_started_at` 先 RED。
- [x] 1.2 實作 design 決策「run_goal 簽章新增 run_started_at 參數（Some 採用 / None 內部派生）」，更新 spec requirement「Goal Verb Library Function」：`run_goal` 結尾新增 `run_started_at: Option<String>`，`Some` 取代內部派生作為 events sink 檔名 slug 來源與 `RunLog.started_at`、`GoalReport.started_at`，`None` 維持現行 `chrono::Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true)`。驗證：1.1 測試轉綠 + `cargo test -p codebus-core` 全綠。
- [x] 1.3 `codebus-cli/src/commands/goal.rs` 呼叫點傳 `None`，CLI goal 行為不回歸（events 檔名/RunLog 仍由 verb 內部派生）。驗證：`cargo test -p codebus-cli`（mock spawn）全綠。

## 2. ipc: Tauri IPC Commands 與 Interrupted Run Detection

- [x] [P] 2.1 為 design 決策「IPC 取樣一次並下傳 run_goal（單一來源根治）」在 `codebus-app/src-tauri/src/ipc/goals.rs` 測試模組新增回歸測試（RED）：以單次取樣得到的 RunId 寫出 events + RunLog 後，`get_run_detail_impl` / `list_runs_impl` 以該 RunId 必須命中同一筆 run（不得回 `AppError::Invalid { field: "run_id" }`）。驗證：新測試先 RED（src-tauri crate 的 `cargo test`）。
- [x] 2.2 實作 design 決策「IPC 取樣一次並下傳 run_goal（單一來源根治）」，更新 spec requirement「Tauri IPC Commands for Goal Lifecycle and Wiki Read」：`spawn_goal_with_runner` 只取樣一次 `Utc::now()`（`Millis`），colon 形式經 `run_started_at` 傳入 `run_goal`，slug 形式（`:`→`-`）同時作為回傳值、`active_runs` key、`goal-stream`/`goal-terminal` payload 的 `run_id`；`goal_run_id()` helper 拆成「取樣得 colon 字串」與「slug 化」兩步。驗證：2.1 測試轉綠。
- [x] 2.3 落實 design 決策「改寫 Precision Alignment Invariant NOTE 並補回歸測試」，守住 spec requirement「Interrupted Run Detection」的 Single-Source Run Id Invariant：移除只驗 byte length 的 `goal_run_id_precision_matches_verb_run_started_at_slug` 測試，改以 2.1 的值相等回歸測試守住單一來源不變式。驗證：src-tauri crate `cargo test` 全綠、無殘留只驗長度的精度測試。

## 3. frontend: Run Detail Load Failure Surfacing

- [x] [P] 3.1 為 design 決策「前端 RunDetail 載入失敗改為可重試錯誤態」在 `codebus-app/src/components/workspace/Workspace.test.tsx` 新增失敗測試（RED）：`get_run_detail` reject 時 Goals 內容區顯示錯誤態（含 retry 與返回清單），不得停留在 `workspace.runDetail.loading`；resolve 時切到對應終態檢視。驗證：`npm run test Workspace` 先 RED。
- [x] 3.2 在 `codebus-app/src/i18n/messages.ts` 新增 RunDetail 載入失敗的錯誤標題、說明、retry 文案 message key（zh-tw 與 en 對齊）。驗證：`npm run test` 的 i18n 對齊測試綠、無缺漏 key。
- [x] 3.3 實作 design 決策「前端 RunDetail 載入失敗改為可重試錯誤態」，落實 spec requirement「Run Detail Load Failure Surfacing」：`Workspace.tsx` 的 GoalsArea RunDetail 載入 effect 移除 `.catch(() => {})`，改為設定錯誤狀態並 render 可重試錯誤分支（retry 重新呼叫 `getRunDetail`、back 返回清單），取代落回 loading 文案。驗證：3.1 測試轉綠 + `npm run typecheck` 無誤。

## 4. 整合驗證與範圍確認

- [x] 4.1 全工作區回歸綠：`cargo test -p codebus-core`、`cargo test -p codebus-cli`、src-tauri crate `cargo test`、`npm run test`、`npm run typecheck` 全綠，且 `cargo clippy --workspace` 無新增警告。驗證：上述指令逐一通過。
- [x] 4.2 手動 / CDP smoke：GUI spawn 一個 goal、停留在 RunDetail 等其完成，畫面自動切到 Done/終態而非卡「載入中…」；並驗一個 failed goal 同樣自動進終態。驗證：CDP 截圖或實機觀察兩條路徑皆不卡。
- [x] 4.3 落實 design 決策「quiz / chat 同類 drift 排除於本 change」：不更動 quiz / chat 的 run id 派生，僅在 design 記錄；驗證 `cargo test -p codebus-cli` 與 src-tauri quiz 相關測試未受本 change 影響、quiz/chat 行為不回歸。
