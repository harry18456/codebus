## Why

Phase 5 sequencing 5.4 與 Phase 6 v1.1 spec landing 入口要同時收尾：`codebus-app/src/components/workspace/QuizTab.tsx` 目前是 list-based「TabContentHeader + tab content」layout，沒有 4 步 wizard、沒有 LLM scope confirm、沒有 generation 進度尾流、沒有 URL state 持久化、cancel mid-step 會留 staged quiz 殘渣。`codebus-app/design-handoff/AUDIT.md` QNEW-1/2 與 `codebus-app/design-handoff/v1.1-mocks.html` 已凍結 v1.1 視覺與互動 spec，這個 change 把 wizard 從 list-based UX 升格成 4 步 content-area wizard view（sidebar 全程顯示），並把 v1.1 mock 細節此 change 一起 land（不另開 change）。

## What Changes

- `codebus-app/src/components/workspace/QuizTab.tsx` 重寫成 content-area wizard view（Workspace sidebar 全程顯示、Workspace 殼層不動）；wizard 啟動時改餵 `codebus-app/src/components/ui/TabContentHeader.tsx` 對應 props（title 改為「New quiz · Step X/N」、`stepIndicator` 渲染 step dots、`cta` 設為 undefined）；wizard 退出時 `TabContentHeader` 恢復既有「Quiz history」title + `+ New quiz` CTA。Wizard-active 判斷維持在 QuizTab local state（derive 自 `useQuizWizardStore` step + URL `quiz_step`），不引入 Workspace 級別 store / Context。
- 新 wizard state machine 4 主 step（topic / scope_confirm / generating / review）+ Step 4 三 sub-state（review_pending / reviewing / completion），用 zustand store 實作（codebase 既有偏好，見 `codebus-app/src/store/`）。
- URL state 持久化：新加 `codebus-app/src/hooks/useUrlState.ts` hook，handle `?quiz_step=...&staged_id=...`；reload 不丟進度。codebus 目前無 React Router，需用 `window.history.pushState` + `window.location.search` 自製 seam；既有 `route.ts` 不擴。
- Step 1 topic 輸入 + 從 vault 既有 wiki page title 拉的 example pill（fallback hard-coded 3-5 個）；empty submit 顯示 amber border + tooltip（不 disable button）。
- Step 2 scope confirm：渲染 LLM 規劃出來的 Karpathy 5-bucket taxonomy（`concepts` / `entities` / `modules` / `processes` / `synthesis`），UI 顯示順序對齊 walkthrough source（modules → processes → synthesis → concepts → entities）；user 可 accept / deselect 個別 bucket / 回 Step 1。
- Step 3 generating：codebus brand banner（🎓 主題 + 🚌 ambient / amber accent 出題中 + 🤔 思考說明，per v1.1 mock §3.3 三 banner）+ 重用 `codebus-app/src/lib/streamEventSummary.ts` 與 `codebus-app/src/lib/clusterTimeline.ts` 渲染 generation stream tail；user 可 cancel（呼叫既有 `cancelQuiz`）。
- Step 4 review→completion：4b reviewing 重用既有 `codebus-app/src/components/workspace/QuizAnswering.tsx` 邏輯（letter chip + radio + citation blockquote + wikilink）但 host 進 wizard content area；4c completion 顯示 fail（XCircle red）/ pass（CheckCircle2 green）summary + 「重做此份」（primary）+「看錯題」/「看過程」action + wrong list mono 行；「← 回 history」back link 由 wizard `TabContentHeader` 提供。
- Cancel mid-step：清 staged quiz + 回 quiz list；目前無 `staged_quiz` / `quiz_draft` seam（grep confirm），此 change 新加 zustand store reset；backend cancel 呼叫已存在的 `cancelQuiz(runId)`（`codebus-app/src/lib/ipc.ts:918`），不新增 IPC。
- i18n：在 `codebus-app/src/i18n/messages.ts` `messages.en` / `messages.zh` 兩 bundle 新增 `workspace.quiz.wizard.*` key（既有 key 不改名、value-only）。Karpathy 5-bucket identifier 不翻譯（identifier-typed value 在 en + zh 兩 bundle 都是英文字面）。
- `codebus-app/src/components/ui/TabContentHeader.tsx` 加 optional `stepIndicator?: ReactNode` prop（既有 caller 不傳即不渲染、行為不變）。
- 純 frontend 改動，不動 backend Rust schema、不動 Workspace 殼層、不引入 Workspace 級別 store / Context、不引入 React Router。

## Capabilities

### New Capabilities

(none)

### Modified Capabilities

- `quiz`: quiz 介面從 list-based 升格成 4 步 content-area wizard、新 scope confirm step（5-bucket Karpathy taxonomy）、URL state 持久化、cancel 不留 staged 殘渣、stream tail 顯示 generation 進度。
- `app-workspace`: Quiz tab 在 wizard 啟動時重新利用既有 `TabContentHeader` row（hide CTA、改 title、加 step indicator）；Workspace 殼層不變、Workspace 不參與 wizard gating。

## Impact

- Affected specs: `quiz`, `app-workspace`
- Affected code:
  - Modified:
    - codebus-app/src/components/workspace/QuizTab.tsx
    - codebus-app/src/components/workspace/QuizAnswering.tsx
    - codebus-app/src/components/workspace/QuizReview.tsx
    - codebus-app/src/components/ui/TabContentHeader.tsx
    - codebus-app/src/i18n/messages.ts
  - New:
    - codebus-app/src/store/quiz-wizard.ts
    - codebus-app/src/hooks/useUrlState.ts
    - codebus-app/src/components/workspace/QuizWizardTopic.tsx
    - codebus-app/src/components/workspace/QuizWizardScopeConfirm.tsx
    - codebus-app/src/components/workspace/QuizWizardGenerating.tsx
    - codebus-app/src/components/workspace/QuizWizardCompletion.tsx
    - codebus-app/scripts/.quiz-wizard-smoke/.gitkeep
  - Removed: (none)
