<!--
Each task delivers a behavior or contract and names a verification target.
File paths are locator context, never the task itself.
TDD: write or extend the failing test BEFORE the implementation task in the
same group, then make it pass.
-->

## 1. Bundle keys（i18n Bundle Coverage Policy 前置）

- [x] 1.1 為 i18n Bundle Coverage Policy 的本次擴充準備 key catalogue：在 `codebus-app/src/i18n/messages.ts` 的 `en` 與 `zh` map 同時新增 4 條 unit/verdict key（`quiz.badge.pass`、`quiz.badge.fail`、`workspace.run.lintSummary`、`workspace.run.headerSummary`），其中 lint / header summary 用單條 value 帶 `{errors}`/`{warnings}`/`{durationSec}`/`{totalTokens}` 模板涵蓋 0/1/N 三種數量；驗證：`pnpm tsc` 綠（key parity check 不報錯）且 `pnpm test src/i18n` 既有 parity 測試通過
- [x] 1.2 在 `codebus-app/src/i18n/messages.ts` 的 `en` 與 `zh` map 同時新增 10 條 bannerLabel key（`workspace.activity.banner.{start|goal|syncStart|syncDone|piiSummary|lintStart|lintDone|commitDone|done|hint}`），每條 value 把 emoji 與 label 文字放在同一字串內、placeholder 用 `{path}` `{goalText}` `{files}` `{mib}` `{elapsedMs}` `{scanner}` `{scanned}` `{hits}` `{action}` `{errors}` `{warns}` `{sha7}`；驗證：`pnpm tsc` 綠且新增 vitest case 斷言 10 條 key 的 en 與 zh value 皆以對應 emoji 起頭

## 2. Residual hard-code wiring（i18n Bundle Coverage Policy 條目 A）

- [x] 2.1 [P] `GoalsTab` 的「+ New goal」按鈕改走 `t("workspace.goals.newGoalButton")`，使 en locale 顯示 `+ New goal`、zh locale 顯示 `+ 新增 Goal`；驗證：新增或擴充 `GoalsTab.test.tsx` 斷言按鈕文字在 mock en/zh provider 下對應正確值
- [x] 2.2 [P] `RunListItem` 三條 time-ago template literal（`${diffMin}m ago`、`${diffHr}h ago`、`${diffDay}d ago`）改走 `t("common.minutesAgo")` / `t("common.hoursAgo")` / `t("common.daysAgo")` 並帶 `{n}` placeholder；驗證：新增或擴充 `RunListItem.test.tsx` 用 fixed `Date.now()` 斷言三種時間區間下 en 顯示 `5m ago`、zh 顯示 `5 分鐘前`
- [x] 2.3 [P] `RunDetailDone` 的 header 單位列（`{durationSec}s · {totalTokens} tokens`）與 lint count（`{n} errors · {n} warnings`）改走 `t("workspace.run.headerSummary")` / `t("workspace.run.lintSummary")`，並確認 0 / 1 / N 三種數量都顯示文法正確的句子；驗證：新增或擴充 `RunDetailDone.test.tsx` 至少 3 個 case 覆蓋 errors=0/1/2 在 en/zh locale 下的渲染
- [x] 2.4 [P] `ChatNewChatButton` 的「+ New chat」按鈕改走 `t("chat.button.newChat")`，使 en locale 顯示 `+ New chat`、zh locale 顯示 `+ 新對話`；驗證：擴充 `ChatNewChatButton.test.tsx` 斷言 en/zh provider 下按鈕文字
- [x] 2.5 [P] `src/lib/quiz-parse.ts` 第 151-152 行 quizBadge pass / fail verdict 字串改走 `t("quiz.badge.pass")` / `t("quiz.badge.fail")`，函式簽名 SHALL 接受 `t` 函式 (或改於呼叫端翻譯) 以維持 lib 純度；驗證：擴充 `quiz-parse.test.ts` 覆蓋 pass / fail 兩個 verdict 在 en / zh locale 的輸出

## 3. ActivityStreamItem bannerLabel rewrite（i18n Bundle Coverage Policy 條目 B + emoji-prefixed scenario）

- [x] 3.1 `ActivityStreamItem` 的 `bannerLabel` 函式 10 個 case (`start` / `goal` / `sync_start` / `sync_done` / `pii_summary` / `lint_start` / `lint_done` / `commit_done` / `done` / `hint`) 全部改走對應的 `workspace.activity.banner.*` key，emoji 留在 i18n value 內、placeholder 用 `t()` 第二參數帶入；en locale 渲染 banner 必須全英文（含 emoji）、zh locale 維持中文；驗證：新增或擴充 `ActivityStreamItem.test.tsx` 為每個 case 至少 1 個 en + 1 個 zh 斷言（共 20 個 expectation）

## 4. Pattern 5/6 sweep procedure 落地（i18n Bundle Coverage Policy 6-pattern sweep scenario）

- [x] 4.1 真實執行 Pattern 5 grep（`grep -rPn` 對 backtick + `${}` + Latin 鄰接）對 `codebus-app/src/`，產出結果列表並逐行分類：(a) 已 `t(...)`、(b) Cat D jargon、(c) 已記錄 keep（如 `NewVaultFlow.tsx:106` `<span>delete</span>`）、(d) 違規；驗證：除 known keep 與已 wire 的 5 處 template literal 外，列表無未分類項目；若有未分類項目 STOP 對齊
- [x] 4.2 真實執行 Pattern 6 grep（Patterns 1a / 1b / 2 / 3 / 4 對 `src/` + `--include='*.ts' --include='*.tsx'`，排除 `src/components/` 與 `.test.`）並逐行分類；驗證：除 known keep 與已 wire 的 quiz-parse verdict 外，列表無未分類項目；任何未分類項目 STOP 對齊

## 5. Test suite

- [x] 5.1 整個 `pnpm tsc` 與 `pnpm test` 跑完全綠（i18n parity check + 新增的 2.x / 3.1 測試）；驗證：兩個指令 exit 0 且 vitest summary 顯示新增 expectation 全部通過

## 6. en locale 真實 CDP smoke

- [x] 6.1 啟動真實 en locale（`$env:LANG = "en"` + `pnpm tauri dev` + `--remote-debugging-port=9222`），用 `codebus-app/scripts/cdp.mjs` 截圖跑：Lobby（empty / quickstart）、Workspace Goals tab（含 RunListItem time-ago）、RunDetail Done（header 單位列 + lint count）、RunDetail Running（ActivityStream 10 個 banner case 至少各觸發 1 次）、Quiz tab（含 quizBadge pass/fail）、Chat（含 + New chat button）；驗證：所有截圖無中文殘留、placeholder 全替換為實際值、wording 無彆扭；截圖存進 `codebus-app/scripts/.i18n-followup-smoke/` 供 review

## 7. AUDIT.md 收尾

- [x] 7.1 在 `codebus-app/design-handoff/AUDIT.md` 把「Followup change · `i18n-sweep-phase-3a-followup`（待開）」段落（第 177-234 行）標記 `archived` 並引向本 spectra change 名稱與 archive 路徑；驗證：在 PR 內手動 review 該段落顯示 `archived` tag + 連結到 `openspec/changes/archive/<date>-i18n-sweep-phase-3a-followup/`
