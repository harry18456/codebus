<!--
Traceability：每 task 標交付行為、驗證目標、對應 design 決策與 spec。TDD：(RED) 先寫失敗測試，(GREEN) 才實作到綠。audit：config/parser 走容錯+clamp。檔案多處重疊（QuizTab.tsx / Workspace.tsx / settings.ts），故序列不平行。
-->

## 1. plan-marker 行內容忍收編（design D5；spec `quiz` / Quiz Verb Two-Shot Flow）

- [x] 1.1 驗證 `codebus-core/src/verb/quiz.rs` `parse_plan_outcome` 已用「行內 `find` marker、取其後 payload」（非 `strip_prefix` 行首限定），且既有單元測試 `parse_scope_marker_glued_after_inline_preamble_is_recovered`、`parse_no_match_marker_glued_after_inline_preamble_is_recovered` 與全 `parse_*` 皆綠（`cargo test -p codebus-core parse_`）。code 於先前已 TDD RED→GREEN 實作（未 commit），本 task 為規格對齊 + regression-verify，無新增實作碼。對應 design D5 與 spec `quiz` / Quiz Verb Two-Shot Flow。驗證：上述測試全綠且 quiz.rs 確為 `find` 實作。

## 2. settings store 在 Workspace 啟動時 load（design D3；spec `app-workspace` / Quiz Tab Plan-Confirm-Generate Flow）

- [x] 2.1 (RED) 在 `codebus-app/src/components/workspace/Workspace.test.tsx` 新增測試：mount `<Workspace>` → 觸發 `load_global_config` IPC 一次（mock 回 `{app:{quiz:{pass_threshold:75}}}`），且 `useSettingsStore.getState().config.app.quiz.pass_threshold` 變 75。對應 design D3 與 spec `app-workspace` / Quiz Tab Plan-Confirm-Generate Flow（threshold 場景）。驗證：測試先 fail（現 Workspace mount 不 load settings）。
- [x] 2.2 (GREEN) `codebus-app/src/components/workspace/Workspace.tsx`：mount effect 呼叫 `useSettingsStore.load()`，guard 僅在 store 仍為初始 `EMPTY_CONFIG`（不覆蓋未存編輯、不重複 in-flight）。對應 design D3。驗證：2.1 全綠；`npx vitest run` 全綠不回歸；`npm run typecheck` 乾淨。依賴 2.1。

## 3. 出題數接 shared `quiz.default_length`（design D4；spec `quiz` / Shared Quiz Config Namespace）

- [x] 3.1 (RED) 在 `codebus-app/src/store/settings.test.ts` 新增 `getDefaultLength()` 測試：shared `config.quiz.default_length` 優先、其次 legacy `config.app.quiz.default_length`、皆無→5；clamp：值 2→3、99→10、10→10、3→3。並在 `QuizTab.test.tsx` 加：generate 流程 `spawnQuizGenerate` 收到的 question count = `getDefaultLength()` 值（非寫死 5）。對應 design D4 與 spec `quiz` / Shared Quiz Config Namespace（App generate uses/clamps 場景）。驗證：測試先 fail（無 `getDefaultLength`、QuizTab 寫死 `DEFAULT_QUESTION_COUNT=5`）。
- [x] 3.2 (GREEN) `codebus-app/src/store/settings.ts`：加 `getDefaultLength()` = `config.quiz?.default_length ?? config.app?.quiz?.default_length ?? 5`，clamp 至 inclusive 3..10；`codebus-app/src/components/workspace/QuizTab.tsx`：移除寫死 `DEFAULT_QUESTION_COUNT`，改以 `useSettingsStore` 的 `getDefaultLength()` 傳給 `spawnQuizGenerate`。對應 design D4。驗證：3.1 全綠；`npx vitest run` 全綠不回歸；`npm run typecheck` 乾淨。依賴 3.1、2.2（實機需 D3 才反映持久化；單元測試以 store setState 驅動）。

## 4. 答題/summary 返回 quiz history（design D1；spec `app-workspace` / Quiz Tab Plan-Confirm-Generate Flow）

- [x] 4.1 (RED) 在 `codebus-app/src/components/workspace/QuizTab.test.tsx` 新增測試：(a) 開啟一個 in-progress attempt 進答題視圖 → 存在 `quiz-back-to-history`，點擊 → 顯示 `quiz-history`、`invokedCommands` 不含 `spawn_quiz_plan`/`spawn_quiz_generate`；(b) 答完到 summary（`quiz-summary`）→ 同樣有 `quiz-back-to-history`，點擊 → 回 `quiz-history`、無 spawn。對應 design D1 與 spec `app-workspace` / Quiz Tab Plan-Confirm-Generate Flow（back-to-history 兩場景）。驗證：測試先 fail（ready phase 無 back 控制）。
- [x] 4.2 (GREEN) `codebus-app/src/components/workspace/QuizTab.tsx`：`ready` phase 外層包一含 `quiz-back-to-history` 的控制（`onClick` → `setPhase("history")`），答題中與 summary 皆渲染；不 spawn；不影響 `+ New quiz` 仍隱藏於 quiz 內。對應 design D1。驗證：4.1 全綠；`npx vitest run` 全綠不回歸；`npm run typecheck` 乾淨。依賴 4.1。

## 5. 已 active 的 Quiz 分頁再點 → 回 quiz history（design D2 / B1；spec `app-workspace`）

- [x] 5.1 (RED) 在 `codebus-app/src/components/workspace/Workspace.test.tsx` 新增測試：進入 quiz 流程後，再次選取 Quiz tab → `quiz-history` 顯示；從別的 tab 切回 Quiz 不重置進行中的 quiz。並在 `QuizTab.test.tsx` 加：`quizHomeSignal` prop 由 0→1 → phase 變 `history`（初始 0 不觸發）。對應 design D2 與 spec `app-workspace` / Quiz Tab Plan-Confirm-Generate Flow（re-select 場景）。驗證：測試先 fail（重點 active tab 無反應、QuizTab 無 `quizHomeSignal`）。
- [x] 5.2 (GREEN) `codebus-app/src/components/workspace/Workspace.tsx`：Quiz tab 的 onSelect 偵測「已是 active quiz tab 又被選」→ 遞增 `quizHomeSignal` 計數並以 prop 傳給 `QuizTab`；`QuizTab.tsx`：新增 `quizHomeSignal?: number` prop，`useEffect` 監看（值 > 0 時）→ `setPhase("history")`。對應 design D2。驗證：5.1 全綠；`npx vitest run` 全綠不回歸；`npm run typecheck` 乾淨。依賴 5.1、4.2（皆動 QuizTab，序列）。

## 6. 全域回歸 sweep

- [x] 6.1 全域回歸：`cargo test -p codebus-core -p codebus-cli`、`cargo test --manifest-path codebus-app/src-tauri/Cargo.toml`、`cd codebus-app && npx vitest run && npm run typecheck` 全綠（彙總 0 failed）；確認 5 項未回歸既有 quiz/v3-app-quiz/quiz-attempt-progress 行為。對應 design「In scope」。驗證：四套件彙總 0 failed。依賴 1.1、2.2、3.2、4.2、5.2。

## Traceability

| Design topic | Tasks |
| --- | --- |
| D1: Back-to-history control in the answering/summary view | 4.1, 4.2 |
| D2: Re-selecting the active Quiz tab returns to quiz history (B1) | 5.1, 5.2 |
| D3: Load global config into the settings store at Workspace mount | 2.1, 2.2 |
| D4: Question count comes from the shared quiz length config | 3.1, 3.2 |
| D5: Plan-marker parser tolerates an inline (same-line) marker | 1.1 |
| Goals | 6.1 |
| Non-Goals | 6.1 |
| In scope | 6.1 |

## Spec requirement coverage

| Spec requirement | Tasks |
| --- | --- |
| Quiz Tab Plan-Confirm-Generate Flow | 4.1, 4.2, 5.1, 5.2 |
| Quiz Verb Two-Shot Flow | 1.1 |
| Shared Quiz Config Namespace | 3.1, 3.2 |
