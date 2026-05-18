<!--
Traceability：每個 task 標明交付行為、驗證目標、對應 spec requirement 與 design 決策（D1–D4 / Implementation Contract）。TDD：標 (RED) 的 task 先寫失敗測試，標 (GREEN) 的 task 才實作到綠。
-->

## 1. pass_threshold 接 settings store（design D1 / Quiz Answering and Summary）

- [x] 1.1 (RED) 在 `codebus-app/src/store/settings.test.ts` 新增測試：settings store 暴露 `pass_threshold`，來源為 `app.quiz.pass_threshold`，缺鍵時 default 80、有值時原樣讀回。對應 design D1 與 spec `app-workspace` / Quiz Answering and Summary。驗證：該測試先 fail（store 尚無欄位）。
- [x] 1.2 (GREEN) 在 `codebus-app/src/store/settings.ts` 新增 `pass_threshold`（綁 `app.quiz.pass_threshold`，缺鍵→80，沿用既有 `quiz.default_length` 的 default-when-absent 模式）。驗證：1.1 測試轉綠；既有 settings store 測試不回歸。依賴 1.1。
- [x] 1.3 (RED) 在 `codebus-app/src/components/workspace/QuizTab.test.tsx`（QuizTab 是 settings store 的消費點；`QuizAnswering` 依 design D1 維持 prop-driven 不讀 config，故 store→outcome 的失敗測試必須放在 QuizTab 這層）改/增測試，以 `useSettingsStore` 的 `app.quiz.pass_threshold` 值驅動跑完一份 5 題 4 對的 quiz：store=80 → summary pass；store=90 → summary fail。對應 spec `app-workspace` / Quiz Answering and Summary 的「Summary applies pass threshold」與「Changing the threshold setting changes the outcome」scenario，及 design D1 contract「drives the settings store value (not a component prop)」。驗證：store=90 案先 fail（QuizTab 目前傳 hardcode 80）。依賴 1.2。
- [x] 1.4 (GREEN) `QuizTab.tsx` 改從 settings store 讀 `pass_threshold` 傳入 `QuizAnswering` 的 `passThreshold` prop，移除 module-level `DEFAULT_PASS_THRESHOLD` 常數與其 follow-up 註解。對應 design D1 / Implementation Contract「pass_threshold」。驗證：1.3 兩案皆綠；`grep -n DEFAULT_PASS_THRESHOLD codebus-app/src` 無殘留。依賴 1.3。

## 2. view-generation-log 渲染 timeline（design D2 / Quiz History List）

- [x] 2.1 (RED) 在 `codebus-app/src-tauri` 新增測試（含 `tests/keyring_ipc.rs` registry count 22→23、`ipc/mod.rs` 的 `exactly_twenty_two_commands`/`command_names_match_spec` 期望含 `read_quiz_events` 且總數 23）與一個 `read_quiz_events(vault_path, path)` 行為測試：給定 vault `.codebus/` 樹下一個 events.jsonl，回傳依序解析的 `EventEnvelope` 清單（壞行跳過不致命）；樹外 path 以 `AppError::Invalid { field: "path" }` 拒絕。對應 design D2 / Implementation Contract「view-generation-log」與 spec `app-workspace` / Tauri IPC Commands for Quiz Plan and Generate Lifecycle。驗證：上述測試先 fail（command 未存在、count 仍 22）。
- [x] 2.2 (GREEN) 在 `codebus-app/src-tauri/src/ipc/quiz.rs` 實作 `read_quiz_events`（逐行解析 `EventEnvelope`、壞行跳過、path 必須 resolve 在 vault `.codebus/` 樹下否則 `AppError::Invalid { field: "path" }`，containment guard 對齊既有 `read_quiz_attempt` 不得更弱 — audit Scoundrel：禁無界路徑讀）；於 `ipc/mod.rs` 的 `generate_ipc_handler!` 與 `REGISTERED_COMMANDS` 註冊；count 測試與 `tests/keyring_ipc.rs` 更新為 23；於 `codebus-app/src/lib/ipc.ts` 加 typed `readQuizEvents` wrapper。對應 design D2 與 spec `app-workspace` / Tauri IPC Commands for Quiz Plan and Generate Lifecycle。驗證：2.1 全綠；既有 tauri 測試不回歸。依賴 2.1。
- [x] 2.3 (RED) 新增 `codebus-app/src/components/workspace/QuizGenerationLog.test.tsx`：給定一個 attempt 的 `events_log` 路徑（mock `readQuizEvents` 回 envelope），元件透過既有 agent stream rendering 渲染出 thought / tool-use / result 串流項目（非僅 path 字串）。對應 spec `app-workspace` / Quiz History List 的「View-generation-log opens the events timeline」與「View-generation-log is not a bare path」scenario。驗證：測試先 fail（元件未存在）。依賴 2.2。
- [x] 2.4 (GREEN) 新增 `codebus-app/src/components/workspace/QuizGenerationLog.tsx`：吃 attempt `events_log` 路徑，呼叫 `readQuizEvents`，reuse `./ActivityStreamItem` 的 `foldTimeline` + `ThoughtItem` + `ActivityStreamItem`（與 `RunDetailDone.tsx` 同一 fold；不得改變 run-detail 行為）。對應 design D2 / Implementation Contract「view-generation-log」。驗證：2.3 轉綠；既有 RunDetailDone 測試不回歸。依賴 2.3。
- [x] 2.5 (GREEN) `QuizTab.tsx` 的 history attempt row：view-generation-log affordance 改開 `QuizGenerationLog`（取代 `quiz-view-log-path` 純路徑顯示）。對應 spec `app-workspace` / Quiz History List。驗證：QuizTab 既有測試調整為斷言 stream-rendered 項目出現、非 path-only，並通過。依賴 2.4、1.4（同檔 `QuizTab.tsx`，序列避免衝突）。

## 3. events_log 端到端驗證（design D3 / Quiz Storage Layout）

- [x] 3.1 (RED) 在 `codebus-cli/tests/quiz_flow.rs` 新增 mock-claude 端到端斷言：一次真實 generate spawn 後，落檔 quiz md 的 `events_log` frontmatter 指向**磁碟上實際存在**的檔，且其內容為該次 generate spawn 的 events。對應 design D3 / Implementation Contract「events_log」與 spec `quiz` / Quiz Storage Layout。驗證：測試先 fail 或揭露現況（grounded：先觀察真實 wiring）。
- [x] 3.2 (GREEN) 若 3.1 顯示 wiring 錯誤，修 `codebus-core/src/verb/quiz.rs` / `codebus-cli/src/commands/quiz.rs` 的 events sink / frontmatter 路徑，使 `events_log` 指向真實 generate events 檔；若 3.1 顯示已正確，將斷言固化為回歸測試。對應 design D3。驗證：3.1 斷言綠；既有 quiz_flow / core quiz 測試不回歸。依賴 3.1。

## 4. 前端型別/lint 衛生（design Goals / Implementation Contract「hygiene」）

- [x] 4.1 在 `codebus-app` 跑 `npx tsc --noEmit` 與 eslint，修掉所有可歸因於 quiz 變更（task 1–2 觸及檔）的 type/lint error。對應 design Implementation Contract「hygiene」。驗證：`npx tsc --noEmit` 與 eslint 對 codebus-app 退出 0 且無 quiz-attributable error；`npx vitest run` 全綠不回歸。依賴 1.4、2.3。

## 5. Windows 人工驗收與收尾（design D4）

- [x] 5.1 執行 Windows 人工端到端驗收：`cd codebus-app && cargo tauri dev`，逐條跑五區塊 checklist——(1) CLI `codebus quiz "<topic>"` 端到端（explicit/`--count` 缺省走共用 `quiz.default_length`、no-match exit 0 不落檔、retry 非破壞兩檔、frontmatter 真值、不 auto-commit）；(2) GUI plan→confirm gate→generate→一題一畫面 client-side 評分→summary（pass_threshold 走 settings）；(3) wiki preview `[Quiz me on this]`（nav 頁不顯示、內容頁 Page scope 跳 plan 直接 generate）；(4) history（同 slug retry 兩列、view-generation-log 渲染 timeline）；(5) 共用 `quiz.default_length` 與 `app.*` namespace isolation（CLI 不讀 app.*）。對應 design D4。驗證：每一區塊每一項記錄 pass 或 defect；全部 pass 或所有 defect 已於 5.2 ingest。
- [x] 5.2 對 5.1 發現的每個 defect：以 plan mode → `/spectra-ingest fix-app-quiz` 併為新 task（compliance defect 收進本 change；若屬新功能則拆出另開 change，不混入），再回 `/spectra-apply` 修到綠。對應 design D4 / Risks。驗證：5.1 所有 defect 皆已 ingest 並修復且對應驗證綠，無遺留未處理項。依賴 5.1。
- [x] 5.3 更新 `docs/v3-app-roadmap.md` 的 Deferred acceptance registry：把 `v3-app-quiz (E)` 條目中「Windows MSVC 上述皆已必跑必過」修正為精確陳述（Windows 人工端到端於本 `fix-app-quiz` change 補跑，自動測試先前已綠；macOS/Linux 仍 deferred 至 polish-ship）。對應 design D4。驗證：roadmap registry 文字與實況一致，review 確認。依賴 5.2。

## 6. Manual-e2e defects ingested（design D4 / task 5.2）

- [x] 6.1 (RED) 在 `codebus-app/src/components/workspace/QuizTab.test.tsx` 新增測試：承載 `+ New quiz` 的 Quiz header 列必須比照 `GoalsTab.tsx` 的 header —— 為固定 WindowControls（3×46px）保留右側 inset 且為拖曳區，具體斷言該 header 容器 className 含 `pr-[160px]` 且帶 `data-tauri-drag-region` 屬性。對應 design D4（defect #1：`+ New quiz` 被視窗 min/max/close 鍵蓋住）與 spec `app-workspace` / Workspace Layout and Tab Navigation。驗證：測試先 fail（現 header 為 `mb-4 flex items-center justify-between`，無 inset / 無 drag-region）。
- [x] 6.2 (GREEN) `QuizTab.tsx` 的 Quiz header 列比照 `GoalsTab.tsx`：加 `pr-[160px]`（WindowControls 空間）與 `data-tauri-drag-region`，使 `+ New quiz` 不再與右上角視窗控制鍵碰撞，位置/樣式與 Goals 的 `+ New goal` 一致。對應 design D4。驗證：6.1 轉綠；QuizTab 既有測試與 `npx vitest run` 全綠不回歸。依賴 6.1。

## 7. Manual-e2e defect #2：+ New quiz 無反應（design D5 / task 5.2）

- [x] 7.1 (RED) 在 `codebus-app/src/components/workspace/QuizTab.test.tsx` 新增測試（defect #2）：(a) 掛載預設只顯示 `quiz-history`、**不**顯示 `quiz-topic-input`；(b) 按 `new-quiz` → 出現 `quiz-topic-input` 且 `quiz-history` 不在；(c) 輸入視圖有 `← History` affordance（testid `quiz-back-to-history`）按下回到只剩 `quiz-history`；(d) 帶 `pendingPage` 掛載仍直接 `spawn_quiz_generate`、不經 plan。對應 design D5 與 spec `app-workspace` / Quiz Tab Plan-Confirm-Generate Flow。驗證：測試先 fail（現 idle 同時含 input+history、`new-quiz` 為 no-op）。
- [x] 7.2 (GREEN) `QuizTab.tsx`：新增 mount-default `history` phase（只渲染歷史清單，含既有「no quizzes yet」空態）；`idle` 改為只渲染 topic-input + Start + `← History`（testid `quiz-back-to-history` → setPhase `history`）；`+ New quiz` 由 `history`/`ready` → `idle`；`pendingPage` effect guard 改為掛載/prop 變更時觸發（不再綁 `phase === "idle"`），維持 `[Quiz me on this]` 直接 generate。同步調整既有 QuizTab 測試（流程先按 `new-quiz` 再操作輸入框）。對應 design D5。驗證：7.1 轉綠；`npx vitest run` 全綠不回歸；`npm run typecheck` 乾淨。依賴 7.1。

## 8. Manual-e2e defect #3：plan marker 解析過脆 + 錯誤不可診斷（design D6 / task 5.2）

- [x] 8.1 (RED) 在 `codebus-core/src/verb/quiz.rs` 的測試模組新增/調整測試：(a) `parse_plan_outcome("Sure, here is the scope.\n[CODEBUS_QUIZ_SCOPE] wiki/a.md")` → `Some(Scope(["wiki/a.md"]))`（容忍前置 preamble，原嚴格測試若斷言 None 一併改為新行為）；(b) `parse_plan_outcome` 對 leading code fence 包住的 `[CODEBUS_QUIZ_NO_MATCH] reason` → `Some(NoMatch("reason"))`；(c) 一個新 helper（如 `plan_parse_error_message(plan_text)`）在無 marker 時回傳含 plan_text 前 ≤200 字 head 的訊息。對應 design D6 與 spec `quiz` / Quiz Verb Two-Shot Flow 新增 scenario。驗證：上述測試先 fail（現 parser 嚴格、錯誤不含 head）。
- [x] 8.2 (GREEN) 修 `codebus-core/src/verb/quiz.rs`：`parse_plan_outcome` 先 `strip_code_fence` 再逐行掃描第一個以 `[CODEBUS_QUIZ_SCOPE]`/`[CODEBUS_QUIZ_NO_MATCH]` 開頭的行（不再僅限 offset 0，鏡像 D4 的容忍 strip）；`None` 路徑改為建構含 `plan_text` 截斷 head（≤200 字）的 `VerbError::Internal`。不動 SKILL agent 契約 / spawn / generate / persist。對應 design D6 與 spec `quiz` / Quiz Verb Two-Shot Flow。驗證：8.1 全綠；`cargo test -p codebus-core` 與 `cargo test -p codebus-cli` 全綠不回歸。依賴 8.1。

## 9. Manual-e2e defect #4：generate body preamble 漏進落檔 + 破壞首題解析（design D7 / task 5.2）

- [x] 9.1 (RED) 在 `codebus-core/src/verb/quiz.rs` 測試模組新增測試：一個新 helper `strip_preamble_before_first_question(body)` 對 `"讀取三個指定的 wiki 頁面以產生測驗題目。## Q1. stem\n- A) a\n## Answer: A\n## Explanation: e"` 回傳以 `## Q1.` 為行首開頭、且不含該 preamble 句的字串；對「本來就以 `## Q1.` 開頭」的 body 原樣回傳；對「無任何 `## Q` 標題」的 body 原樣回傳（不遮蔽下游 no-question 處理）。對應 design D7 與 spec `quiz` / Quiz Markdown Schema and Caller Frontmatter Injection 新增 scenario。驗證：測試先 fail（helper 未存在）。
- [x] 9.2 (GREEN) 在 `codebus-core/src/verb/quiz.rs` 實作 `strip_preamble_before_first_question`（找第一個 `## Q<n>.` 出現處——即使被 glue 在同行——回傳自該處起 trim 後字串；無則原樣）；於 `run_quiz_generate` 在 `strip_code_fence` 之後套用，使 `quiz_md` 單一來源同時餵 GUI `QuizReport` 與 CLI `persist_quiz`。不動 SKILL 契約 / spawn / frontmatter / IPC。對應 design D7 與 spec `quiz` / Quiz Markdown Schema and Caller Frontmatter Injection。驗證：9.1 全綠；`cargo test -p codebus-core` 與 `cargo test -p codebus-cli` 全綠不回歸。依賴 9.1。

## 10. Manual-e2e defect #5：Quiz tab 未即時顯示 agent 過程（design D8 / task 5.2）

- [x] 10.1 (RED) 在 `codebus-app/src/components/workspace/QuizTab.test.tsx` 新增測試：mock `quiz-stream` 在 `quiz-plan-terminal` 之前推送一筆 `{ run_id, event: { kind:"stream", data:{ kind:"thought", text:"planning…" } } }`，斷言 `planning` 階段出現既有 stream rendering 的 `thought-item`（非僅靜態 `quiz-planning` 文字）；同樣對 `generating` 階段。對應 design D8 與 spec `app-workspace` / Quiz Tab Plan-Confirm-Generate Flow 新 scenario「Plan/generate agent activity is rendered live」。驗證：測試先 fail（QuizTab 未接 `quiz-stream`、僅靜態字）。
- [x] 10.2 (GREEN) `QuizTab.tsx`：onStart/onConfirm 與 pendingPage 直接 generate 路徑皆 `listen<QuizStreamPayload>("quiz-stream")`，`event` 累積進 `liveEvents` state（新 run 開始/確認時重置），`planning` 與 `generating` 階段以既有 `./ActivityStreamItem` 的 `foldTimeline` + `ThoughtItem`/`ActivityStreamItem` 渲染（靜態標題可留為標頭但不得單獨存在）；listener 生命週期比照既有 terminal-channel（同 unlisten ref、terminal/unmount 清理）。對應 design D8。驗證：10.1 轉綠；`npx vitest run` 全綠不回歸；`npm run typecheck` 乾淨。依賴 10.1。

## 11. UX feedback #6：view-log 移進 attempt 詳情頁 + 中央 Modal（design D9 / task 5.2）

- [x] 11.1 (RED) 在 `codebus-app/src/components/workspace/QuizTab.test.tsx` 改/增測試：(a) history 列**不再**有 `quiz-view-log` 按鈕、也無 `quiz-view-log-panel` inline 展開；(b) 開啟一個 `events_log` 非 null 的 attempt（`quiz-attempt-view`）後出現 view-log 按鈕（testid `quiz-view-log`），點下出現中央 modal（testid `quiz-view-log-modal`）內含既有 stream rendering 的 `thought-item`（mock `readQuizEvents` 回 thought envelope），關閉後回到 `quiz-attempt-view`；(c) 開啟 `events_log` 為 null 的 attempt 時**無** view-log 按鈕。對應 design D9 與 spec `app-workspace` / Quiz History List 改寫後 scenario。驗證：測試先 fail（現按鈕在 history 列、inline panel、attempt 詳情頁無此鈕）。
- [x] 11.2 (GREEN) `QuizTab.tsx`：移除 history 列的 `quiz-view-log` 按鈕與 `quiz-view-log-panel` inline 區塊；`openAttempt` 保留所選 attempt meta（含 `events_log`）；`quiz-attempt-view` 在 `events_log` 非 null 時渲染 `quiz-view-log` 按鈕，點擊以既有 `Dialog`（`components/ui/dialog.tsx`，與 SettingsModal 同元件）開中央 modal（`quiz-view-log-modal`），body 為既有 `QuizGenerationLog`（不改）；關閉回 attempt view（不改 phase）。對應 design D9。驗證：11.1 轉綠；`npx vitest run` 全綠不回歸；`npm run typecheck` 乾淨。依賴 11.1。

## 12. Manual-e2e defect #7：+ New quiz 進到 quiz 內不該出現（design D10 / task 5.2）

- [x] 12.1 (RED) 在 `codebus-app/src/components/workspace/QuizTab.test.tsx` 新增測試：(a) `history` 與按 `new-quiz` 後的 `idle` 仍有 `new-quiz` 按鈕；(b) 進入 quiz 流程/詳情各 phase（planning、confirm、generating、ready、no_match、error、attempt）時 `new-quiz` 按鈕**不存在**（`queryByTestId("new-quiz")` 為 null）。對應 design D10 與 spec `app-workspace` / Quiz Tab Plan-Confirm-Generate Flow 新 scenario「+ New quiz is not shown while inside a quiz」。驗證：測試先 fail（現 `new-quiz` 在所有 phase 皆 render，只是部分 disabled）。
- [x] 12.2 (GREEN) `QuizTab.tsx`：header 的 `+ New quiz` 改為**僅當 `phase === "history" || phase === "idle"` 才 render**（取代原本「永遠 render、部分 disabled」）；header bar 本身與 `pr-[160px]`/`data-tauri-drag-region` 不變。對應 design D10。驗證：12.1 轉綠；`npx vitest run` 全綠不回歸；`npm run typecheck` 乾淨。依賴 12.1。

## Traceability

| Design topic | Tasks |
| --- | --- |
| Non-Goals | 5.1, 5.3 |
| D1: pass_threshold flows settings store → QuizTab → QuizAnswering | 1.1, 1.2, 1.3, 1.4 |
| D2: view-generation-log reuses RunDetailDone's stream rendering | 2.1, 2.2, 2.3, 2.4, 2.5 |
| D3: events_log verified at the CLI layer with mock-claude | 3.1, 3.2 |
| D4: manual acceptance is a task with a written checklist; defects ingested | 5.1, 5.2, 5.3, 6.1, 6.2 |
| D5: Quiz tab default is history; `+ New quiz` opens a distinct topic-input view (manual-e2e defect #2) | 7.1, 7.2 |
| D6: Plan-marker parser tolerant recovery + diagnostic (manual-e2e defect #3) | 8.1, 8.2 |
| D7: Strip generate-body preamble before the first question (manual-e2e defect #4) | 9.1, 9.2 |
| D8: Quiz tab renders the live agent stream during plan/generate (manual-e2e defect #5) | 10.1, 10.2 |
| D9: View-generation-log moves into the attempt detail view as a centered modal (manual-e2e UX feedback #6) | 11.1, 11.2 |
| D10: `+ New quiz` is hidden once inside a quiz (manual-e2e defect #7) | 12.1, 12.2 |
