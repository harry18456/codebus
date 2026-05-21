## Problem

在 app 對一個 wiki 頁面按一次「Quiz me on this」出題，卻產生**兩份** quiz attempt（實測：`D:\side_project\tools\.codebus\quiz\tool-wrapper-0aceba89/` 下出現 `2026-05-21T05-38-25Z.md` 與 `2026-05-21T05-38-48Z.md` 兩份不同內容的題目）。使用者只觸發一次，卻多花一次 generate 的 token 與時間，且 quiz history 多一筆混淆。

## Root Cause

兩層問題（已由 events log + 程式碼證實）：

1. **前端 double-fire（主因，dev-only 觸發）**：`codebus-app/src/components/workspace/QuizTab.tsx` 的 Page-flow `useEffect`（依賴 `pendingPage`）在 `pendingPage` 為真時呼叫 `startGenerate`，但**沒有 re-entry guard**。`codebus-app/src/main.tsx` 啟用 React `StrictMode`，dev（`cargo tauri dev`）下 effect 會 double-invoke，兩次都讀到同一個 `pendingPage`、都呼叫 `startGenerate` → 同一瞬間發兩次 `spawn_quiz_generate`。events log `events-2026-05-21T05-37-14Z.jsonl` 內有 4 個 quiz `spawn_start`，其中 2 個時間戳完全相同（05:37:14Z），證實併發兩次 generate。

2. **後端不擋併發（次因，prod 仍可觸發）**：`codebus-app/src-tauri/src/ipc/quiz.rs` 的 `spawn_quiz_generate` 只 `active_runs.insert(...)`，**沒有像 goal/chat 那樣「已有 active run 就拒絕」**。且 `quiz_run_id` 用秒級精度，同秒兩次會撞號、`active_runs` map 靜默覆蓋（第一條的 cancel handle 遺失）。兩條背景 thread 都跑到底、各自 `persist_quiz` 寫一份。

## Proposed Solution

兩道鎖（defense-in-depth，比照 goal/chat 既有風格）：

1. **前端 re-entry guard**：在 `QuizTab.tsx` 的 Page-flow effect 加一個 `useRef` 記錄「已對此 `pendingPage` 觸發過」，同一個 `pendingPage` 值只 `startGenerate` 一次，StrictMode double-invoke 下第二次 no-op。
2. **後端併發拒絕**：`ActiveRuns` 加 `has_quiz_run()`（key 以 `quiz-` 前綴判定，涵蓋 plan 與 generate）；`spawn_quiz_plan` / `spawn_quiz_generate` 在已有 quiz run active 時回 `AppError::Invalid { field: "active_runs", .. }`（與 goal/chat 一致），不再 insert、不 spawn。
3. **run_id 去撞號**：`quiz_run_id` 精度由秒提到毫秒（`SecondsFormat::Millis`），同一秒內的兩次取得不同 id，避免 `active_runs` 覆蓋導致 cancel handle 遺失。

## Non-Goals

- 不改 quiz 的 plan→confirm→generate 流程本身（plan 與 generate 非同時併發，本 change 只擋「同類 spawn 重入」）。
- 不改 goal / chat 既有併發行為（它們已有 guard）。
- 不回頭刪除已產生的兩份重複 quiz 檔案（使用者手動清理；修 code 不動既有資料）。
- 不改 quiz 內容生成 / content-verify / 持久化邏輯（`persist_quiz`、`run_quiz_generate` 不動）。
- 不移除 React StrictMode（它在 dev 抓 bug 有價值；正解是讓 effect StrictMode-safe）。

## Success Criteria

- 前端：對同一 `pendingPage` 連續 render / StrictMode double-invoke 下，`spawnQuizGenerate` 只被呼叫一次（vitest 斷言 mock invoke 次數 == 1）。
- 後端：當已有一個 `quiz-` 前綴 run 在 `active_runs` 時，再呼叫 `spawn_quiz_generate` / `spawn_quiz_plan` 回 `AppError::Invalid { field: "active_runs" }` 且不 spawn（單元測試以注入式 runner 斷言 runner 未被呼叫）。
- 後端：`quiz_run_id` 連續兩次呼叫產出不同字串（毫秒精度）。
- 一次「Quiz me on this」只產生一份 attempt 檔。

## Impact

- Affected code:
  - Modified: codebus-app/src/components/workspace/QuizTab.tsx（Page-flow effect 加 useRef re-entry guard）
  - Modified: codebus-app/src-tauri/src/state/active_runs.rs（加 `has_quiz_run()`）
  - Modified: codebus-app/src-tauri/src/ipc/quiz.rs（plan/generate spawn 加併發拒絕；`quiz_run_id` 改毫秒精度）
  - Modified: codebus-app/src/components/workspace/QuizTab.test.tsx（前端 double-fire 回歸測試）
