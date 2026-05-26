<!--
每條任務都需描述：(1) 完成後可觀察的行為 / 契約 (2) 驗證方式。
File paths 只是定位線索，不能單獨構成一條 task。
-->

## 1. 基礎建設與測試骨架（TDD red 階段）

- [x] 1.1 在 `codebus-app/src/i18n/settings.test.ts` 新檔，撰寫 vitest 斷言：i18n Bundle Coverage Policy 要求的 Cat A settings keys 全部在 `messages.ts` 的 `en` 與 `zh` map 中存在，且 zh 對「端點設定不完整：」翻為中文、jargon (`base_url` / `api_version` / `keyring_service` / verb names) zh 值與 en 值字面相同；執行 `pnpm --filter codebus-app vitest run src/i18n/settings.test.ts` 應 fail（key 尚未加入）作為 red 確認。
- [x] 1.2 在 `codebus-app/src/i18n/a11y.test.ts` 新檔，撰寫 vitest 斷言：i18n Bundle Coverage Policy 要求的 Cat C a11y keys 全部在雙 locale 存在，且 `a11y.pageNotFound` 為唯一共用 key（非每組件一份）；執行 `pnpm --filter codebus-app vitest run src/i18n/a11y.test.ts` 應 fail 作為 red 確認。
- [x] 1.3 擴充既有 `codebus-app/src/i18n/workspace.test.ts`（或同層新增 `quiz.test.ts`）覆蓋 Cat B 的 QuizAnswering / QuizReview / QuizTab / NewGoalModal / ChatInput / RunDetailRunning button + placeholder + DialogTitle 新 keys；執行對應 vitest 應 fail 作為 red 確認。

## 2. messages.ts bundle 加 keys（i18n Bundle Coverage Policy · green 第一階段）

- [x] 2.1 [P] 在 `codebus-app/src/i18n/messages.ts` 加入 Cat A settings keys（EndpointSection / CodexEndpointSection / SetKeyDialog 全部 hard-code 字串），en + zh 雙 locale；jargon 詞（`base_url` / `api_version` / verb names）兩 locale 同值；「Endpoint configuration is incomplete:」zh 翻為「端點設定不完整：」；驗證：1.1 settings.test.ts vitest 轉綠 + `pnpm --filter codebus-app exec tsc --noEmit` 通過（key parity 編譯保證）。
- [x] 2.2 [P] 在 `codebus-app/src/i18n/messages.ts` 加入 Cat B workspace keys（QuizAnswering Passed/Failed/Correct/Incorrect/Finish/Next、QuizReview Passed/Failed/Generation log、QuizTab generation log/quiz failed/placeholder、NewGoalModal title/placeholder、ChatInput placeholder、RunDetailRunning Cancel/Cancelling…）；驗證：1.3 vitest 轉綠 + tsc --noEmit 通過。
- [x] 2.3 [P] 在 `codebus-app/src/i18n/messages.ts` 加入 Cat C a11y keys，含 shared `a11y.pageNotFound` 與 dialog close / chat widget open/resize/minimize/drag / wiki tree toggle 各 key；驗證：1.2 a11y.test.ts vitest 轉綠 + 1.2 對「shared key 唯一性」的斷言通過。
- [x] 2.4 在 `codebus-app/src/i18n/messages.ts` 加入 Cat D jargon allow-list keys（workspace tab labels `Goals` / `Wiki` / `Quiz`、verb names、codex effort、PII action）en 與 zh 值皆為英文 jargon；驗證：新增 vitest case 斷言對 jargon keys 兩 locale 字面相等，跑綠後留作 regression。

## 3. Cat A 組件改寫（settings panel 收 hard-code 英文）

- [x] 3.1 把 `codebus-app/src/components/settings/EndpointSection.tsx` 所有渲染英文字串（section heading、label="System" / label="Azure"、card title、label="API key"、status Set/Unset、aria-label="Active endpoint profile"、placeholders）改走 `t("...")`；行為：zh locale 下 Settings → Claude Code 段完全中文（除 jargon yaml key 名）；驗證：vitest snapshot 或 render test 對 zh locale 主要字串斷言為中文 + 手動 smoke zh locale 開 Settings panel 確認無英文漏網。
- [x] 3.2 把 `codebus-app/src/components/settings/CodexEndpointSection.tsx` 所有 hard-code 英文字串（含 section heading、「OpenAI Codex endpoint settings」、「Endpoint configuration is incomplete:」、label="effort"、effort 值 dropdown、placeholders）改走 `t("...")`；effort 值走 Cat D jargon 鍵（兩 locale 同字面）；驗證：vitest 對 zh locale 「端點設定不完整：」字串 + en locale 「Endpoint configuration is incomplete:」字串 + effort dropdown 兩 locale 都顯示 `low`/`medium`/`high`/`xhigh`。
- [x] 3.3 把 `codebus-app/src/components/settings/SetKeyDialog.tsx` 的 DialogTitle、error message「API key cannot be empty」、button「Confirm」/「Saving…」改走 `t("...")`；驗證：vitest render test 對話框 zh locale 顯示中文 title + error + button 文案。

## 4. Cat B 組件改寫（workspace 零星 hard-code 收掉）

- [x] 4.1 [P] `codebus-app/src/components/workspace/QuizAnswering.tsx` 的 Passed/Failed (threshold {n}%) / Correct / Incorrect / Finish / Next 全部走 `t("...")`，threshold 百分比走 placeholder；驗證：擴充 `quiz.test.ts` 對 5 個 quiz outcome 字串雙 locale 斷言通過。
- [x] 4.2 [P] `codebus-app/src/components/workspace/QuizReview.tsx` 的 Passed/Failed + DialogTitle「Generation log」改走 `t("...")`；驗證：對應 vitest case 雙 locale 斷言通過。
- [x] 4.3 [P] `codebus-app/src/components/workspace/QuizTab.tsx` 的 DialogTitle「Generation log」、error「Quiz failed: {errorMsg}」、placeholder「What do you want to be quizzed on?」改走 `t("...")`；驗證：對應 vitest case 雙 locale 斷言通過。
- [x] 4.4 [P] `codebus-app/src/components/workspace/NewGoalModal.tsx` DialogTitle「New goal」 + placeholder「What should codebus document?」改走 `t("...")`；驗證：對應 vitest case 雙 locale 斷言通過。
- [x] 4.5 [P] `codebus-app/src/components/workspace/ChatInput.tsx` placeholder「Type your message...」改走 `t("...")`；驗證：對應 vitest case 雙 locale 斷言通過。
- [x] 4.6 [P] `codebus-app/src/components/workspace/RunDetailRunning.tsx` 的「⏹ Cancel」與「Cancelling…」改走 `t("...")`，emoji 留在 JSX 不進 bundle；驗證：對應 vitest case 雙 locale 斷言通過。

## 5. Cat C 共用 a11y 收洞（shared key 強制）

- [x] 5.1 `codebus-app/src/components/ui/dialog.tsx` 的 `aria-label="Close"` 改走 `t("a11y.dialogClose")`；驗證：vitest render test 對 dialog close button 的 accessible name 在 en/zh 雙 locale 都符合 bundle 值。
- [x] 5.2 `codebus-app/src/components/workspace/ChatWidget.tsx` 4 處 a11y attr（Open chat / Resize chat widget / Minimize chat / Drag to resize）改走 `t("...")` 各自獨立 key；驗證：vitest 對 4 個 accessible name 斷言通過。
- [x] 5.3 `codebus-app/src/components/workspace/ChatTranscript.tsx`、`ExplanationText.tsx`、`WikiPreview.tsx` 3 處 `title="Page not found"` 統一走 `t("a11y.pageNotFound")` 單一 shared key，符合 i18n Bundle Coverage Policy 對 shared accessibility key 的要求；驗證：1.2 a11y.test.ts 對「3 處同 key」的斷言通過 + grep 確認三檔僅引用同一 key。
- [x] 5.4 `codebus-app/src/components/workspace/WikiTab.tsx` 的 `aria-label="Toggle Pages tree"` 改走 `t("...")`；驗證：vitest 對 toggle button accessible name 雙 locale 斷言通過。

## 6. Cat D jargon 走 bundle（policy 強制集中）

- [x] 6.1 `codebus-app/src/components/workspace/Workspace.tsx` 的 `Goals` / `Wiki` / `Quiz` tab label 改走 Cat D jargon keys；驗證：vitest 對 3 個 tab label 在雙 locale 皆顯示英文 jargon 字面，且 keys 解析自 bundle 而非 hard-code。
- [x] 6.2 EndpointSection / CodexEndpointSection 的 verb name rows（`goal` / `query` / `fix` / `verify` / `chat`）改走 Cat D jargon keys；驗證：vitest 對 verb row label 雙 locale 字面相等且字串等於 verb identifier。

## 7. 中段驗收（Cat A/B/C/D 六群完成後）

- [x] 7.1 跑 `pnpm --filter codebus-app vitest run` 全套 i18n / component 測試綠燈 + 處理任何 snapshot regen；驗證：cli 退出碼 0、無 unexpected snapshot diff。
- [x] 7.2 跑 `pnpm --filter codebus-app exec tsc --noEmit` typecheck 全綠；驗證：cli 退出碼 0。

## 8. 殘留 sweep — 29 處 hard-code 收洞（scope expansion 2026-05-26，4-pattern grep audit）

第一輪 §7.3 grep 規則太窄（只抓 `[A-Z]` 開頭），漏掉 emoji / 箭頭 prefix 與多行 JSX text。第二輪 4-pattern grep audit 抓出 29 處違反 i18n Bundle Coverage Policy 的 hard-code，全部收進同 change 避免新 spec 在 main 上立刻有違反。21 處純 wiring + 8 處新 key（涵蓋 6 個新 bundle key + 1 個既有 key value 微調對齊）。

- [x] 8.1 在 `codebus-app/src/i18n/messages.ts` 加入第一波 4 個新 key（雙 locale 對齊）：`settings.cliStatus.installed`、`settings.cliStatus.notInstalled`、`workspace.wiki.quizMeOnThis`、`workspace.quiz.generationLogLoadError`、外加 apply session inline 已加的 `workspace.sidebar.vaultPathHint` 與 `chat.widget.title.minimizeShortcut`；驗證：tsc --noEmit 通過（key parity 編譯保證）+ vitest 全套 643/643 綠燈。
- [x] 8.2 在 `codebus-app/src/i18n/messages.ts` 加入第二波 4 個新 key（雙 locale 對齊）：`settings.endpoint.saveButtonIncompleteTitle` "Endpoint configuration is incomplete — fix highlighted fields" / 中文翻譯、`chat.error.promoteFailed` "Promote failed. Try again." / 中文翻譯、`chat.undoToast.heading` "🆕 New chat started" / "🆕 已開始新對話"、`chat.undoToast.countdown` "({n}s to undo)" / "（{n} 秒可復原）"；同時把既有 `chat.error.anotherGoalRunning` value 對齊「wait for it to finish」phrasing（grep 確認目前無實際 caller、僅 ChatTranscript.tsx:151 將成為第一個 caller）；驗證：tsc --noEmit 通過 + 新 key 雙 locale vitest 斷言通過。
- [x] 8.3 [P] 把 `codebus-app/src/components/workspace/RunDetailCancelled.tsx` 9 處 hard-code 改走 bundle：2 處 `← back` → `workspace.runDetail.backLink`、`⏹ Cancelled` → `cancelledBadge`、`Wiki has uncommitted changes…` → `cancelledWarning`、2 處 `Retry with same goal` → `retryButton`、`⚠ Interrupted` → `interruptedBadge`、`App was closed before this goal finished…` → `interruptedWarning`、`Partial timeline` → `partialTimelineLabel`；驗證：vitest 對 RunDetail cancelled/interrupted 兩視圖渲染雙 locale 對應字串、tsc 通過。
- [x] 8.4 [P] 把 `codebus-app/src/components/workspace/RunDetailDone.tsx` 5 處 hard-code 改走 bundle：`← back` → `workspace.runDetail.backLink`、`✓ Done` → `doneBadge`、`Covered pages` → `coveredPagesLabel`、`No wiki pages changed` → `coveredPagesEmpty`、`Lint` → `lintLabel`；驗證：vitest 對 done 視圖渲染雙 locale 對應字串、tsc 通過。
- [x] 8.5 [P] 把 `codebus-app/src/components/workspace/RunDetailRunning.tsx` 2 處 hard-code 改走 bundle：`← back` → `workspace.runDetail.backLink`、`⏺ Running` → `runningBadge`；驗證：vitest 對 running 視圖渲染雙 locale 對應字串、tsc 通過。
- [x] 8.6 [P] 把 `codebus-app/src/components/workspace/Workspace.tsx` 的 `← Back to Lobby` 改走 `t("workspace.backToLobby")`；驗證：vitest 對 sidebar back link 雙 locale 渲染對應 bundle 值。
- [x] 8.7 [P] 把 `codebus-app/src/components/workspace/WikiTab.tsx` 的 `No wiki pages yet — run a goal to start documenting` 改走 `t("workspace.wiki.empty")`；驗證：vitest 對 WikiTab 空狀態雙 locale 渲染對應 bundle 值。
- [x] 8.8 [P] 把 `codebus-app/src/components/workspace/GoalsTab.tsx` 的 `Click + New goal to ask codebus to ingest something into the wiki` 改走 `t("workspace.goals.emptyHint")`；驗證：vitest 對 Goals empty state 雙 locale 渲染對應 bundle 值。
- [x] 8.9 [P] 把 `codebus-app/src/components/workspace/ChatTranscript.tsx` 改三處 hard-code：(a) onboarding hint「Ask anything about this vault. AI will suggest…」走 `t("chat.onboarding.hintEn")`（清掉 TODO comment），(b) promote 失敗 fallback「Another goal is running…」走 `t("chat.error.anotherGoalRunning")`，(c)「Promote failed. Try again.」走 8.2 新 key `t("chat.error.promoteFailed")`；驗證：vitest 對 onboarding 段 + promote 錯誤兩分支雙 locale 渲染對應 bundle 文案。
- [x] 8.10 [P] 把 `codebus-app/src/components/workspace/ChatUndoToast.tsx` 3 處 hard-code 改走 bundle：`🆕 New chat started` → `t("chat.undoToast.heading")`、`({remaining}s to undo)` → `t("chat.undoToast.countdown", { n: remaining })`、`Undo` button → `t("chat.toast.undo")`；驗證：vitest 對 ChatUndoToast 三組字串雙 locale 渲染對應 bundle 文案。
- [x] 8.11 [P] 把 `codebus-app/src/components/settings/SettingsModal.tsx` 3 處 hard-code 改走 bundle：`Installed · {version}` → `t("settings.cliStatus.installed", { version })`、`Not installed` → `t("settings.cliStatus.notInstalled")`、save button 不完整 tooltip `Endpoint configuration is incomplete — fix highlighted fields` → `t("settings.endpoint.saveButtonIncompleteTitle")`；驗證：vitest 對 cli-status badge + save button tooltip 雙 locale 渲染對應字串，tsc 通過。
- [x] 8.12 [P] 把 `codebus-app/src/components/workspace/QuizGenerationLog.tsx` 的 `Could not load generation log: {error}` 改走 8.1 新 key `t("workspace.quiz.generationLogLoadError", { error })`；驗證：vitest 對 QuizGenerationLog 錯誤分支雙 locale 渲染對應字串。
- [x] 8.13 [P] 把 `codebus-app/src/components/workspace/WikiPreview.tsx` 的 `Quiz me on this` button 改走 8.1 新 key `t("workspace.wiki.quizMeOnThis")`；驗證：vitest 對 WikiPreview button label 雙 locale 渲染對應 bundle 值。

## 9. 全綠驗收

- [x] 9.1 跑 `pnpm --filter codebus-app vitest run` 全套 i18n / component 測試綠燈 + 處理任何 snapshot regen；驗證：cli 退出碼 0、無 unexpected snapshot diff。
- [x] 9.2 跑 `pnpm --filter codebus-app exec tsc --noEmit` typecheck 全綠；驗證：cli 退出碼 0。
- [x] 9.3 在 `codebus-app/` 內跑 spec「Re-running 4-pattern sweep」scenario 的 4 條 grep 指令（Pattern 1 JSX text、Pattern 2 emoji/arrow + Latin、Pattern 3 a11y attrs、Pattern 4 placeholder syntax 字串）；剩餘命中必須能逐條對應到 (a) `t("...")` 呼叫、(b) Cat D jargon allow-list 條目、(c) tool name identifier 例外、或 (d) 文件化的 runtime keyword `NewVaultFlow.tsx:106 "delete"`；驗證：grep 結果逐條 ack 為 expected exemption，紀錄在 apply session output 與 archive PR description。
- [x] 9.4 切到 zh locale 跑 manual smoke：開 codebus-app、進 Settings 各 section、開 Quiz / New Goal / Chat / Wiki / RunDetail 三狀態（Running / Done / Cancelled / Interrupted）、ChatUndoToast、WikiPreview 的 Quiz me on this button，確認無英文漏網（除 Cat D 與 yaml key 名）；驗證：harry 在 chat 回報「zh smoke 通過」。
- [x] 9.5 切到 en locale 跑 manual smoke：同樣路徑，確認沒誤翻 jargon（Goals / verb names / effort 值 / PII action / yaml keys 皆顯示英文）；驗證：harry 在 chat 回報「en smoke 通過」。
