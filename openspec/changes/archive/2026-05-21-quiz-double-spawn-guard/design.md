## Context

`spawn_quiz_generate`（`codebus-app/src-tauri/src/ipc/quiz.rs`）是 quiz 出題的 IPC 入口；它把 `run_quiz_generate`（codebus-core）丟到背景 thread、`active_runs.insert(run_id, cancel)` 後回傳 run_id，結束時 `persist_quiz` 寫一份 `<vault>/.codebus/quiz/<slug>/<quiz_id>.md`。前端 `QuizTab.tsx` 的 Page-flow（wiki preview「Quiz me on this」）以 `useEffect([pendingPage])` 在 `pendingPage` 為真時呼叫 `startGenerate` → `spawnQuizGenerate`。

`ActiveRuns`（`codebus-app/src-tauri/src/state/active_runs.rs`）以 run_id 字串為 key 的 map，已有 prefix-based 查詢：`has_chat_turn()`（`chat-` 前綴）、`has_goal_run()`（非 `chat-`）。quiz run_id 形如 `quiz-generate-<ts>` / `quiz-plan-<ts>`（`quiz_run_id` 產生，秒級精度）。goal / chat 的 spawn 在已有同類 run active 時會回 `AppError::Invalid { field: "active_runs" }` 拒絕；**quiz 沒有這道檢查**。

實測 bug：一次「Quiz me on this」產生兩份 attempt。根因見 proposal——前端 StrictMode double-invoke effect（dev）+ 後端無併發拒絕（prod 潛在）。

## Goals / Non-Goals

**Goals:**

- 一次使用者觸發只產生一份 quiz attempt。
- 前端 effect 對 StrictMode double-invoke 免疫（同 `pendingPage` 只 fire 一次）。
- 後端對「已有 quiz run 在跑」的第二次 quiz spawn 明確拒絕，與 goal/chat 一致。

**Non-Goals:**

- 不改 plan→confirm→generate 流程、不改 `run_quiz_generate` / `persist_quiz` / content-verify。
- 不改 goal / chat 既有併發行為。
- 不移除 StrictMode。
- 不自動清理既有重複檔。

## Decisions

### 前端：useRef re-entry guard（不改 effect 依賴）

`QuizTab.tsx` Page-flow effect 加一個 `useRef`（如 `firedForPageRef`）記錄「上次已對哪個 `pendingPage` 觸發過」。effect 進入時，若 `pendingPage` 等於 ref 已記錄值則 no-op；否則更新 ref 並 `startGenerate`。StrictMode 第二次同步 invoke 時 ref 已更新 → 第二次 no-op。沿用既有 `onPendingConsumed?.()` 清空 parent 狀態的 one-shot 行為。

Alternatives：把觸發從 effect 改成顯式 handler——較大改動、偏離既有 Page-flow 設計，rejected。

### 後端：`ActiveRuns::has_quiz_run()` + spawn 併發拒絕

`ActiveRuns` 加 `has_quiz_run()`：key 以 `quiz-` 前綴判定（涵蓋 plan 與 generate）。`spawn_quiz_plan` 與 `spawn_quiz_generate` 在 spawn 前檢查 `has_quiz_run()`，為真則回 `AppError::Invalid { field: "active_runs", message: <quiz already running> }`，不 insert、不 spawn——比照 goal `has_goal_run()` / chat `has_chat_turn()` 既有 pattern。

Alternatives：複用 `has_goal_run()`（quiz- 已被它算進去）——但語意混（goal 與 quiz 互擋），且訊息不清，rejected；用 quiz 專屬鎖語意清楚。

### 後端：`quiz_run_id` 改毫秒精度

`quiz_run_id` 由 `SecondsFormat::Secs` 改 `SecondsFormat::Millis`，避免同秒兩次取得相同 id 導致 `active_runs` 覆蓋、cancel handle 遺失。

### TOCTOU 取捨（明示）

`has_quiz_run()` 檢查與 `active_runs.insert` 是兩次獨立鎖取得，理論上存在極小 race window（兩次 IPC 在同一瞬間都通過檢查才 insert）。本 change **不**為此引入 atomic check-and-insert：前端 guard（Decision 1）已消除唯一已知的同步雙觸發來源（StrictMode），人為連點間隔（數十 ms）下第一次必已 insert。race 視為可接受殘餘風險，若未來觀察到再以 atomic API 收口。

## Implementation Contract

**Behavior:**

對同一 wiki 頁面按一次「Quiz me on this」只產生一份 quiz attempt 檔。前端 effect 在 StrictMode double-invoke 下只呼叫一次 `spawnQuizGenerate`。後端在已有 `quiz-` 前綴 run active 時，第二次 `spawn_quiz_generate` / `spawn_quiz_plan` 被拒絕、不 spawn。

**Interface / data shape:**

- `ActiveRuns`（`codebus-app/src-tauri/src/state/active_runs.rs`）新增 `pub fn has_quiz_run(&self) -> bool`，回傳是否有任一 key 以 `quiz-` 前綴。
- `spawn_quiz_plan_with_runner` / `spawn_quiz_generate_with_runner`（`ipc/quiz.rs`）在 spawn 前：`if active_runs.has_quiz_run() { return Err(AppError::Invalid { field: "active_runs".into(), message: "a quiz run is already active".into() }) }`。
- `quiz_run_id`（`ipc/quiz.rs`）使用 `SecondsFormat::Millis`。
- `QuizTab.tsx` Page-flow effect 用 `useRef` 守住「每個 `pendingPage` 值只 `startGenerate` 一次」。

**Failure modes:**

- 第二次併發 quiz spawn → `AppError::Invalid { field: "active_runs" }`；前端既有錯誤處理（`startGenerate` catch → setPhase("error") / errorMsg）顯示。正常單次觸發下使用者不會看到此錯誤（前端 guard 先擋）。

**Acceptance criteria:**

- 前端 vitest：模擬 `pendingPage` 設定 + effect double-invoke（或重複 render 同值），斷言 `spawnQuizGenerate`（mock invoke）只被呼叫一次。
- 後端單元測試：`active_runs` 預先插入一個 `quiz-...` key 後，呼叫 `spawn_quiz_generate_with_runner`（注入式 runner）回 `AppError::Invalid { field: "active_runs" }` 且 runner 未被呼叫；`has_quiz_run()` 對 `quiz-` 前綴回 true、對 `chat-`/無前綴回 false。
- 後端單元測試：`quiz_run_id` 連兩次呼叫產出不同字串。
- `cargo test --package codebus-app-tauri` 與 `npx vitest run --no-coverage QuizTab` 全綠。

**Scope boundaries:**

In scope：`QuizTab.tsx` effect guard + 測試、`active_runs.rs` 加 `has_quiz_run`、`ipc/quiz.rs` plan/generate 併發拒絕 + run_id 毫秒精度 + 測試、app-workspace spec delta。

Out of scope：plan/generate 流程、`run_quiz_generate`/`persist_quiz`/content-verify、goal/chat 併發行為、StrictMode、既有重複檔清理、atomic check-and-insert。

## Risks / Trade-offs

- [TOCTOU race window] → Mitigation: 前端 guard 消除已知同步雙觸發；殘餘風險可接受，未來再 atomic 收口（見上 Decision）。
- [後端拒絕可能在罕見競態下讓使用者看到 active_runs 錯誤 toast] → Mitigation: 前端 guard 先擋；錯誤訊息沿用既有 pattern，非 crash。
- [毫秒精度仍可能極罕見撞號] → Mitigation: 撞號機率極低；併發拒絕為主防線，run_id 精度為輔。
