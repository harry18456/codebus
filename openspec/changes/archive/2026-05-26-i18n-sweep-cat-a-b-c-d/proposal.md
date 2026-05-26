## Why

AUDIT.md cross-cutting i18n 章節盤點：i18n 架構（`i18n/messages.ts` 雙語 bundle、`useT` hook、`errors.ts` LocalizedError、`Toast.tsx` locale-aware）本身乾淨，但組件層大量 JSX hard-code 英文字串。zh-tw 環境下 Settings / Workspace 多處永遠英文，跟周圍中文 UI 混雜成「亂」感。同時 aria-label / title attr 也是 i18n 漏洞、a11y 也跟著漏。Phase 3A 把 AUDIT 列出的 Cat A/B/C/D 全部收掉，並把「user-facing string 一律走 bundle」的政策正式寫進 spec。

## What Changes

- 在 `codebus-app/src/i18n/messages.ts` 新增 Cat A / B / C / D 對應的 zh + en key（雙 locale 對齊；Cat D 兩 locale 都填英文 jargon）。
- 改寫 Cat A 三個 settings 檔（`settings/EndpointSection.tsx`、`settings/CodexEndpointSection.tsx`、`settings/SetKeyDialog.tsx`）所有 hard-code 英文字串改走 `t("...")`。
- 改寫 Cat B 6 個 workspace 檔（`workspace/QuizAnswering.tsx`、`workspace/QuizReview.tsx`、`workspace/QuizTab.tsx`、`workspace/NewGoalModal.tsx`、`workspace/ChatInput.tsx`、`workspace/RunDetailRunning.tsx`）的 button label / DialogTitle / placeholder / error message 改走 `t("...")`。
- 改寫 Cat C 共用 UI 元件的 aria-label / title attr（`ui/dialog.tsx`、`workspace/ChatWidget.tsx`、`workspace/WikiTab.tsx`、`workspace/ChatTranscript.tsx`、`workspace/ExplanationText.tsx`、`workspace/WikiPreview.tsx`、`lib/milkdown-wikilink.tsx`）改走 bundle；4 處 `title="Page not found"`（ChatTranscript / ExplanationText / WikiPreview / milkdown-wikilink）統一走 shared key `workspace.wiki.pageNotFound`。
- 翻譯 wording 細節：「Endpoint configuration is incomplete:」→「端點設定不完整：」（句子整翻、jargon 只限 `base_url` / `api_version` 等 yaml key 名本身）。
- 在 `app-shell` capability 新增 `Requirement: i18n Bundle Coverage Policy`，把「所有 user-facing string（含 aria-label / title / DialogTitle / placeholder / error / status / button label）必須走 i18n bundle」與「Cat D jargon allow-list」「shared aria key per concept」三條規則寫成 normative spec，避免未來組件再漏；同時把可重跑的 4-pattern sweep grep procedure 寫進 spec 的「Re-running 4-pattern sweep」scenario，做成 PR review / 新 component land 的 verification gate。
- **Scope expansion · 第二輪（2026-05-26，4-pattern sweep grep audit）**：第一輪 grep 規則太窄（只抓 `[A-Z]` 開頭），漏掉 emoji / 箭頭 prefix（`← back` / `⏹ Cancelled` / `⏺ Running` / `✓ Done` / `⚠ Interrupted`）+ 多行 JSX text。第二輪 4-pattern grep 抓出 29 處違反 i18n Bundle Coverage Policy 的 hard-code，全部收進同 change：
  - **21 處純 wiring**（bundle key 已有）：`RunDetailCancelled.tsx` 9 處（backLink / cancelledBadge / cancelledWarning / 2× retryButton / interruptedBadge / interruptedWarning / partialTimelineLabel）、`RunDetailDone.tsx` 5 處（backLink / doneBadge / coveredPagesLabel / coveredPagesEmpty / lintLabel）、`RunDetailRunning.tsx` 2 處（backLink / runningBadge）、`Workspace.tsx` 1 處 backToLobby、`WikiTab.tsx` 1 處 empty hint、`GoalsTab.tsx` 1 處 emptyHint、`ChatTranscript.tsx` 1 處 onboarding hint、`ChatUndoToast.tsx` 1 處 Undo button。
  - **8 處需新 key**（涵蓋 6 個新 bundle key）：`SettingsModal.tsx` CLI 安裝狀態 2 處（`settings.cliStatus.installed/notInstalled`）+ 1 處 save button 不完整 tooltip（`settings.endpoint.saveButtonIncompleteTitle`）、`QuizGenerationLog.tsx` 1 處（`workspace.quiz.generationLogLoadError`）、`WikiPreview.tsx` 1 處（`workspace.wiki.quizMeOnThis`）、`ChatTranscript.tsx` 2 處 promote 錯誤（`chat.error.anotherGoalRunning` 沿用、`chat.error.promoteFailed` 新增）、`ChatUndoToast.tsx` 2 處 heading + countdown（`chat.undoToast.heading/countdown`）。

## Non-Goals

- 不調整 i18n pipeline 本身（`useT` hook、`errors.ts` LocalizedError、`Toast.tsx`、locale 偵測邏輯）——AUDIT 已確認架構乾淨。
- 不新增 Language switcher UI（locale auto-detect 維持現狀，AUDIT 已明列 forbidden）。
- 不動 Cat D 的 tool name identifier（`Read` / `Write` / `Glob` / `Grep` / `Edit` / `Bash`），這些是 `case` match 用的 Claude API tool name，是 identifier、不是 UI label，可不走 bundle。
- **不收 `NewVaultFlow.tsx:106` 的 `<span>delete</span>`**：`delete` 是 user 要打進輸入框才能 re-init 的字面 keyword（runtime 比對），翻譯反而破壞行為；屬 Cat D identifier 例外 spirit。
- **不收 `ActivityStreamItem.tsx` bannerLabel 8 處中文 hard-code**（`🚌 來囉來囉…` / `🎯 任務目標…` / `🔄 同步…` / `✓ lint 完成…` / `🛡 PII…` / `🔍 lint 中…` / `🎉 完成` / `💡 提示`）：是 **en bundle 缺中文翻譯**的反向問題（en locale 渲染會中英混雜），跟 Phase 3A「zh 環境英文 leak」是不同 root cause；另開 change `i18n-banner-label-en-bundle` 處理。
- 不動 design-handoff/ 內容、不動 Phase 2 剛 land 的 SectionLabel component 結構。
- 不做 Phase 3B（status three-state token + StatusPill），另開 change。
- 不改 design-system token 或 Tailwind v4 emit 邏輯——Phase 3A 純 string 工作，無 emit gotcha 風險。

## Capabilities

### New Capabilities

(none)

### Modified Capabilities

- `app-shell`: 新增 `i18n Bundle Coverage Policy` requirement——規範所有 user-facing 字串（含 aria-label / title / DialogTitle / placeholder）必須走 `i18n/messages.ts` bundle，列出 Cat D jargon allow-list 與 shared aria key 原則，並把可重跑的 4-pattern sweep grep procedure 寫進 scenario 做成 verification gate。

## Impact

- Affected specs: `openspec/specs/app-shell/spec.md`（新增一條 Requirement + 4-pattern sweep verification scenario）
- Affected code:
  - Modified:
    - codebus-app/src/i18n/messages.ts
    - codebus-app/src/components/settings/EndpointSection.tsx
    - codebus-app/src/components/settings/CodexEndpointSection.tsx
    - codebus-app/src/components/settings/SetKeyDialog.tsx
    - codebus-app/src/components/settings/SettingsModal.tsx
    - codebus-app/src/components/workspace/QuizAnswering.tsx
    - codebus-app/src/components/workspace/QuizReview.tsx
    - codebus-app/src/components/workspace/QuizTab.tsx
    - codebus-app/src/components/workspace/QuizGenerationLog.tsx
    - codebus-app/src/components/workspace/NewGoalModal.tsx
    - codebus-app/src/components/workspace/ChatInput.tsx
    - codebus-app/src/components/workspace/ChatTranscript.tsx
    - codebus-app/src/components/workspace/ChatUndoToast.tsx
    - codebus-app/src/components/workspace/ChatWidget.tsx
    - codebus-app/src/components/workspace/RunDetailRunning.tsx
    - codebus-app/src/components/workspace/RunDetailCancelled.tsx
    - codebus-app/src/components/workspace/RunDetailDone.tsx
    - codebus-app/src/components/workspace/Workspace.tsx
    - codebus-app/src/components/workspace/WikiPreview.tsx
    - codebus-app/src/components/workspace/WikiTab.tsx
    - codebus-app/src/components/workspace/GoalsTab.tsx
    - codebus-app/src/components/workspace/ExplanationText.tsx
    - codebus-app/src/components/ui/dialog.tsx
    - codebus-app/src/lib/milkdown-wikilink.tsx
  - New:
    - codebus-app/src/i18n/settings.test.ts（Cat A 字串雙 locale 覆蓋測試）
    - codebus-app/src/i18n/a11y.test.ts（Cat C aria-label / title shared key 測試）
    - codebus-app/src/i18n/quiz.test.ts（Cat B quiz 字串雙 locale 覆蓋測試）
