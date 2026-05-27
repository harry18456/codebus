## Context

Phase 5 sequencing 5.4 與 Phase 6 v1.1 spec landing 入口共構，AUDIT QNEW-1/2 系列 + v1.1 mock + walkthrough-decisions（Karpathy 5-bucket taxonomy）三份設計 source 由此 change 同時 land。

**現況校準（propose 階段 grep confirm；apply 階段 Task 1.1 補充見「Pre-apply 校準」段）：**

- `codebus-app/src/components/workspace/QuizTab.tsx`（803 行）已有 local useState phase 機器（history / idle / planning / confirm / generating / ready / no_match / error / attempt / review），但走 list-based layout、沒有 wizard chrome、沒有 URL state 持久化、沒有 staged quiz cancel cleanup seam。
- `codebus-app/src/components/workspace/Workspace.tsx`（586 行）渲染 sidebar（vault 名 + 3 個 nav row）+ main content area；**沒有 Workspace 殼層 topbar 這個 element**（grep `topbar` / `TabContentHeader` / `content-header` 在 `Workspace.tsx` 全 0 hit）。
- `codebus-app/src/components/ui/TabContentHeader.tsx` 是 tab 內 content header row（title + subtitle + optional CTA + shortcut chip），由 GoalsTab / QuizTab 等各 tab 各自渲染。`QuizTab.tsx:519` 目前用 `phase === "history"` gate 此 header 與 `+ New quiz` CTA。
- `codebus-app/src/store/route.ts` 只有 lobby/workspace 二分 RouteState，沒有 sub-route 概念。
- codebus-app **沒有 React Router**（`useSearchParams` / `useNavigate` / `RouterProvider` grep 全 0 hit）→ URL state 持久化必須走 `window.history.pushState` + `window.location.search`，自製 hook。
- 既有 stream tail helper `codebus-app/src/lib/streamEventSummary.ts` 與 cluster timeline helper `codebus-app/src/lib/clusterTimeline.ts` confirm 存在，由 ActivityCluster / RunDetailRunning 等多處 reuse。
- i18n `workspace.quiz.*` 在 `codebus-app/src/i18n/messages.ts` 已 wire 74 處 key，沿用 prefix。
- 無 `staged_quiz` / `quiz_draft` / `quiz_session` seam（grep 全 0 hit），cancel cleanup 走 wizard zustand store reset + 既有 `cancelQuiz(runId)` IPC（見「Pre-apply 校準」段（b））。

## Pre-apply 校準

由 Task 1.1 在 apply 階段填入。對齊 source：`codebus-app/design-handoff/v1.1-mocks.html` §03（Quiz wizard 4 步）、`codebus-app/design-handoff/walkthrough-decisions.html` §03（Quiz scope-confirm Design A）、`codebus-app/design-handoff/AUDIT.md` QNEW-1/2 + QC2 系列，以及 `codebus-app/src/lib/ipc.ts` cancel IPC 與 `codebus-app/src/components/ui/TabContentHeader.tsx` 既有結構。對齊結論以本段為準、與 propose 階段 design / spec / tasks 衝突時以本段優先。

**(a) Sidebar 全程顯示，wizard 不是視窗級 fullscreen。**

v1.1-mocks.html §03 內每個 step mock（Step 1 / Step 3 / Step 4 answering / Step 4c completion）皆仍渲染 `ws-side`（vault 名 + 返回 + Goals/Wiki/Quiz 三 nav row + 各自 count）。Wizard 接管的是 main content area，不是 Workspace 殼層。Spec 與本 design 內凡寫「fullscreen wizard」處，皆指 **「content-area wizard with sidebar visible」**。

**(b) Cancel IPC：reuse 既有 `cancelQuiz(runId)`。**

`codebus-app/src/lib/ipc.ts:918` 已暴露 `cancelQuiz(runId: string): Promise<void>`，呼叫 `cancel_quiz` Tauri command、idempotent、由既有 codebase 涵蓋 plan + generate 兩階段 cancel。Wizard cancel 直接呼叫此 fn，不新增 IPC。

**(c) Karpathy bucket = 5 個，不是 6 個。**

walkthrough-decisions.html line 893 確認 「Wiki tree · MODULES / PROCESSES / SYNTHESIS / CONCEPTS / ENTITIES」恰好 5 個 identifier。propose 階段 user prompt 寫「6-bucket」是筆誤（AUDIT 與 walkthrough 一直是 5-bucket）。本 change spec / design / tasks 凡寫「6-bucket」處應讀作「5-bucket」；identifier 列表為 `concepts` / `entities` / `modules` / `processes` / `synthesis`，UI 顯示順序對齊 walkthrough source：MODULES → PROCESSES → SYNTHESIS → CONCEPTS → ENTITIES。

**Identifier 不譯 policy：** 5 個 bucket name 為 identifier（Cat D），en 與 zh bundle value 都直接寫英文字面、不翻譯。包含 identifier 的周圍 UI text（如「選擇要包含的 buckets」/ "Select buckets to include"）正常 translatable、不受此規則限制。

**(d) AUDIT vs v1.1 mock vs propose user prompt 「topbar」三方語義不一致——以 mock + AUDIT + 實機為準。**

| Source | 「topbar」字眼指什麼 |
| --- | --- |
| propose user prompt（誤解） | Workspace 殼層 topbar（要 hide） |
| AUDIT QNEW-1/2 | tab content header 內的 `+ New quiz` button + header title 文字 |
| v1.1-mocks §03 | 「Wizard 全程 topbar `+ New quiz` 隱藏」— 同 AUDIT 語義 |
| 實機 `Workspace.tsx` | 無「殼層 topbar」element（grep 0 hit） |

propose 階段我誤讀 AUDIT「topbar」字眼為 Workspace 殼層 topbar，並引入了不必要的 `Workspace Chrome Hide Signal` requirement 與 `workspace-chrome.ts` store。實機 + AUDIT + mock 三方一致：**只需在 `TabContentHeader.tsx` 與 `QuizTab.tsx` 內依 wizard active 狀態切換 header content**（CTA 隱藏 + title 改為「New quiz · Step X/N」、依 AUDIT QC2 dot 規格）。**Workspace Chrome Hide Signal 概念整段 drop**，下節（仲裁規則）詳列影響範圍。

**仲裁規則影響範圍**（spec / design / tasks 同 change ingest）：

- spec：移除 `Workspace Chrome Hide Signal` requirement；`Quiz Tab Fullscreen Wizard Layout` 改為 `Quiz Tab Wizard Content Header And Layout`、移除「workspace topbar hide」字眼。
- design：本「Pre-apply 校準」段為最終依據；下方 D3 改寫為「wizard-active 信號維持在 QuizTab local state、不擴 Workspace store」；Interface / data shape 段移除 `useWorkspaceChromeStore`；Acceptance criteria 段移除 Workspace.test.tsx topbarHidden 訂閱測試。
- tasks：Task 4（Workspace chrome hide signal 兩個 sub-task）改寫為「TabContentHeader wizard-active prop 支援 + QuizTab.tsx 在 wizard 期間渲染對應 header」。

## Goals / Non-Goals

**Goals:**

- QuizTab 重寫成 wizard view（content-area wizard with sidebar visible），狀態機 4 主 step + Step 4 三 sub-state、URL 持久化、cancel 不留 staged 殘渣。
- Wizard 期間 `TabContentHeader.tsx` 內 `+ New quiz` CTA 隱藏、header title 改為「New quiz · Step X/N」（per AUDIT QNEW-1/2 + QC2）；wizard-active 信號維持在 QuizTab local state，不擴 Workspace store。
- Generation stream tail 由既有 streamEventSummary + clusterTimeline 渲染，不另造輪子。
- v1.1 mock 視覺細節（Step 1 wiki page title example pill / Step 3 brand banner / Step 4 answering letter chip + radio / Step 4c completion summary fail vs pass）與 AUDIT QNEW-1/2 行為一起 land。
- i18n 新增 8-12 條 `workspace.quiz.wizard.*` key，既有 key 不改名、value-only；Karpathy 5-bucket identifier 不翻譯（identifier 性質、Cat D，per Pre-apply 校準（c））。

**Non-Goals:**

- 不動 backend Rust schema（純 frontend；若意外要動，須 grep `#[serde(tag` 校準避免命名 collision，由 apply 階段觸發重新 propose 而非靜默擴 scope）。
- 不動 Workspace 殼層（per Pre-apply 校準（d）；sidebar 全程顯示，無殼層 topbar 可動）。
- 不引入 `workspace-chrome.ts` 或同類 Workspace 級別 store / Context（per Pre-apply 校準（d））。
- 不動 Wiki page reader（屬 Phase 6 wiki-page-reader-v1.1 範圍）。
- 不動 ChatWidget 3 modes（屬 Phase 6 chatwidget-three-modes 範圍）。
- 不動 02c Interrupted layout（屬 Phase 6 interrupted-state-formalize 範圍）。
- 不動 LoadingOverlay live progress（屬 Phase 6 loading-overlay-live-progress 範圍）。
- 不重造 stream event 渲染邏輯。
- 不翻譯 Karpathy 5-bucket name（concepts / entities / modules / processes / synthesis 為 identifier，Cat D）。
- 不引入大幅動畫（step 切換 subtle、prefers-reduced-motion 給 instant）。
- 不開 feature branch（solo dev 直接 main）。

## Decisions

### D1 · Wizard state machine 用獨立 zustand store

**選擇：** 新建 `codebus-app/src/store/quiz-wizard.ts`，state 用 discriminated union：

- topic：尚未提交 topic
- scope_confirm：持有 stagedId 與 LLM 規劃的 buckets
- generating：持有 stagedId
- review_pending：持有 stagedId（quiz 已 ready 但 user 未開始答）
- reviewing：持有 stagedId（user 進入 QuizAnswering）
- completion：持有 stagedId 與 result

**為什麼不選 useReducer 內嵌 QuizTab.tsx：** QuizTab 已 803 行、再塞 wizard state 會越界；wizard 內每個 sub-component（Topic / ScopeConfirm / Generating / Completion）要直接讀寫狀態，store 比 prop drilling 乾淨。codebus 既有偏好 zustand（src/store/ 9 個 store），沿用同模式。

**為什麼不選擴 route.ts：** route.ts 是 lobby/workspace 頂層路由，wizard 是 quiz tab 內部 sub-state，責任不同；混在一起會讓 URL 持久化邏輯難維護。

### D2 · URL state 用自製 useUrlState hook

**選擇：** 新建 `codebus-app/src/hooks/useUrlState.ts`，內部用 `window.history.pushState` + popstate listener 同步 `?quiz_step=...&staged_id=...`。

**為什麼不引入 React Router：** codebus 全 codebase 0 hit React Router；引入一個只為了 sub-step 的 router framework 是過度設計，且會跟既有 route.ts 重複。

**邊界：**

- hook 只 own `quiz_step` 與 `staged_id` 兩個 query param，不接管整條 URL；其他 param 不刪不動。
- Wizard mount/unmount 不觸發 popstate（避免 reload 時不必要 push）；只在 user 互動觸發的 step 切換才 `pushState`。
- Reload：mount 時讀 `window.location.search` 還原 step；若 `staged_id` 在 store 不存在（killed by app restart），fallback 回 quiz list（topic step）。

### D3 · wizard-active 信號維持在 QuizTab local state、不擴 Workspace store

**選擇：** 由 `useQuizWizardStore.step.kind !== "topic" || hasUrlQuizStepParam` 推導 wizard-active boolean（QuizTab.tsx 本地 derived state）。`QuizTab.tsx` 依此切換 `TabContentHeader.tsx` 的 props：wizard 期間傳 `title="New quiz"` + `stepIndicator={<StepDots .../>}` + `cta={undefined}`；非 wizard 期間維持既有 `title=t("workspace.quiz.headerTitle")` + `+ New quiz` CTA。`Workspace.tsx` 不參與此判斷、不訂閱任何新 store。

**為什麼不新增 Workspace 級別 store / Context：** Pre-apply 校準（d）confirm 實機 `Workspace.tsx` 無殼層 topbar element、AUDIT + mock 「topbar」字眼指 `TabContentHeader` 與 `+ New quiz` button 本身；任何 Workspace 級別 chrome signal 都是 over-engineering、scope 跨 design-system。Wizard active 狀態純粹是 QuizTab 內部 state、用 derived value 即可。

**為什麼不寫死 `if (activeTab === "quiz" && wizardActive)` in Workspace.tsx：** `Workspace.tsx` 完全不需要參與 wizard chrome 切換（per 上一條）；該檔不新增任何條件分支。

**TabContentHeader 改動：** 新增 optional `stepIndicator?: ReactNode` prop（dots + label），位置在 title 右側 / cta 左側；既有 GoalsTab / 其他 caller 不傳 prop、行為不變（per Phase 4A G-copy-2 value-only 規則的 prop-only 推廣）。

### D4 · Step 3 generation stream tail reuse 既有 helper

**選擇：** 新 `QuizWizardGenerating` 組件 import `streamEventSummary` 與 `clusterTimeline` 兩 helper，渲染同 ActivityCluster / RunDetailRunning 一致的 stream tail。

**為什麼：** helper 已被多處 consumer reuse（grep confirm 8 個檔案），wizard 是新 consumer；另造輪子會散落渲染邏輯、未來 stream schema 改動要多改一處。

### D5 · Cancel cleanup IPC seam

**選擇：** Wizard 在 scope_confirm / generating / review_pending 三 step cancel 時：

1. zustand wizard store reset 回 topic step。
2. 呼叫既有 `cancelQuiz(runId)`（`codebus-app/src/lib/ipc.ts:918`，per Pre-apply 校準（b））；plan + generate 兩階段同一 IPC 處理、idempotent。
3. Wizard staged 狀態（Step 2 LLM 回的 buckets payload）只存 zustand 而非 disk；cancel 直接 reset store 即可，無 disk IO。
4. `staged_id` 在 wizard store 用 nanoid 隨機生成，cancel 後 ID 失效；URL 上殘留的 `staged_id` 在 reload 時被偵測為 not-found → fallback 回 quiz list。

**為什麼不持久化 staged 到 disk / SQLite：** Wizard staged 狀態屬「進行中操作」，user cancel 即捨棄；persist 到 disk 反而需要 garbage collection 政策；既有 quiz ready 完成後 markdown 已落地（disk），那才是「完成品」。

### D6 · Step 4b reviewing 重用既有 QuizAnswering

**選擇：** Wizard reviewing sub-state 不重寫 QuizAnswering，而是把它 host 進 wizard content container；既有 letter chip + radio + citation blockquote + wikilink 行為不動。

**為什麼：** QuizAnswering 已 292 行、含 wiki 跳轉、letter chip 視覺、進度持久化等行為；重寫風險遠大於 host 進新 layout。

**邊界：**

- 若 wizard layout 要求 QuizAnswering 改某個 prop（如 `embedded?: boolean` 控制是否自帶 Back button），透過 prop 注入而非分支內部 logic。
- QuizReview 同理 host 進 wizard 而不重寫。

### D7 · i18n key 命名與不翻譯規則

**選擇：** 新 key prefix 一律 `workspace.quiz.wizard.*`，遵循既有 `workspace.quiz.*` convention（74 處 hit confirm）。en + zh 兩 bundle 都加；既有 key 不改名（per Phase 4A G-copy-2 教訓 value-only）。

**Karpathy 5-bucket：** identifier 不翻，en 與 zh 兩 bundle value 都直接寫英文字面（`concepts` / `entities` / `modules` / `processes` / `synthesis`）。包含 identifier 的周圍 UI text 正常 translatable。

## Implementation Contract

### Observable behavior

- 進入 Quiz tab 時若 wizard store step 為 topic、且無 URL `?quiz_step=` param → 顯示既有 quiz list + `TabContentHeader` 維持既有 title「Quiz history」+ `+ New quiz` CTA（相容、不改 list 視覺）。
- 在 quiz list 點「+ New quiz」→ wizard store 進 topic step、URL push `?quiz_step=topic`、`TabContentHeader` title 改顯示「New quiz」+ step dots（1/4 current）+ 無 CTA、content area 渲染 wizard topic step、sidebar 不變。
- Step 1 提交 topic → wizard store 進 scope_confirm step（含 stagedId + buckets）、URL push `?quiz_step=scope_confirm&staged_id=<id>`、`TabContentHeader` step dots 更新（2/4）、content area 顯示 Karpathy 5-bucket checklist。
- Step 2 accept → wizard store 進 generating step、URL push、`TabContentHeader` step dots 更新（3/4）、content area 顯示 codebus brand banner + stream tail。
- Step 3 generation 完成 → wizard store 進 review_pending step、URL push、`TabContentHeader` step dots 更新（4/4）、content area 顯示 quiz 概要 + Start control。
- Step 4a 點 Start → wizard store 進 reviewing step、URL push、`TabContentHeader` 改顯示「Quiz: <topic>」+ 「Q<n> / 5」counter（per v1.1 mock § 3.5 answering header）、content area host 既有 QuizAnswering。
- Step 4b 完成 → wizard store 進 completion step、URL push、`TabContentHeader` 改顯示「Quiz <topic> · result」+「← 回 history」back link（per v1.1 mock § 3.6）、content area 顯示 fail / pass summary + 「重做此份」（primary）+「看錯題」（fail）/「看過程」（pass）。
- 任一 step cancel → wizard store reset、URL 移除 `quiz_step` 與 `staged_id`、`TabContentHeader` 恢復既有 quiz history view 形態、回 quiz list、呼叫 `cancelQuiz(runId)`（plan 或 generate 在飛者）。
- Reload 在任一 step：mount 時讀 URL param 還原 step；若 `staged_id` 在 store 不存在 → fallback 回 quiz list（topic step）。
- 切 locale en / zh：所有 `workspace.quiz.wizard.*` key value 切對應語系；reload 仍維持；Karpathy 5 bucket identifier 永遠英文。
- Wizard active 時 ChatWidget 圓鈕仍在右下、不擋 wizard action bar（apply 階段 CDP smoke 驗；若 mock 標 wizard 期間 hide ChatWidget，加 hide signal，否則維持）。

### Interface / data shape

- `useQuizWizardStore`：state shape 見 D1；actions `goToTopic` / `goToScopeConfirm(stagedId, buckets)` / `goToGenerating(stagedId)` / `goToReviewPending(stagedId)` / `goToReviewing(stagedId)` / `goToCompletion(stagedId, result)` / `cancel` / `hydrateFromUrl(searchParams)`。
- `useUrlState({ quiz_step, staged_id })`：hook 對外輸出 `read()` 與 `write({ quiz_step, staged_id })` 兩 fn；確切型別簽章由 apply 階段在 TDD 內 finalize。
- `TabContentHeader` 新 optional prop `stepIndicator?: ReactNode`；既有 caller（GoalsTab、quiz history view 自己）不傳、行為不變。
- `cancelQuiz(runId: string): Promise<void>`：reuse 既有（`codebus-app/src/lib/ipc.ts:918`），wizard cancel 時呼叫；無 new IPC。
- i18n key（en + zh 兩 bundle 都加）：
  - `workspace.quiz.wizard.step1.title` / `subtitle` / `placeholder`（per v1.1 mock §3.1 input placeholder）/ `examplePillHint`（「點擊範例直接填入。⏎送出」）
  - `workspace.quiz.wizard.step2.title` / `bucketLabel.concepts` / `bucketLabel.entities` / `bucketLabel.modules` / `bucketLabel.processes` / `bucketLabel.synthesis`
  - `workspace.quiz.wizard.step3.title` / `generatingHint`
  - `workspace.quiz.wizard.step4.pendingTitle` / `reviewingTitle` / `completionTitle.pass` / `completionTitle.fail`
  - `workspace.quiz.wizard.action.cancel` / `back` / `next` / `start` / `submit` / `retry` / `redo` / `viewWrong` / `viewProcess`
  - `workspace.quiz.wizard.header.stepLabel`（「Step <n> / <N>」格式 token；具體 i18n 文法由 implementation 處理）
  - 既有 `workspace.quiz.headerTitle` / `workspace.quiz.tab.newButton` 等 key **不改名**（per D7）。

### Failure modes

- URL `staged_id` 在 wizard store 找不到 → silently fallback 回 quiz list（topic step、不彈錯誤）；console.warn 記 `staged_id` missing 以便調試。
- Step 3 generation 失敗（既有 `quiz-generate-terminal` error payload）→ wizard step 切到 error sub-view（reuse 既有 error 顯示），保持 wizard 不退出；user 可 retry 或 cancel。
- Cancel 時 `cancelQuiz(runId)` 拒絕（罕見）→ wizard store 仍 reset、URL 仍清，但 console.error 記 backend cancel 失敗；不阻塞 UX。
- prefers-reduced-motion：step 切換動畫降為 instant；不阻塞功能。

### Acceptance criteria

- `pnpm tsc`（在 `codebus-app/`）綠。
- `pnpm test`（在 `codebus-app/`）綠，含：
  - `quiz-wizard.test.ts`：state machine 6 step transition 全覆蓋（含 cancel 從任意 step 回 topic）。
  - `useUrlState.test.ts`：reload 還原 step、staged_id missing fallback、其他 query param 不被刪。
  - `QuizWizardTopic.test.tsx` / `QuizWizardScopeConfirm.test.tsx` / `QuizWizardGenerating.test.tsx` / `QuizWizardCompletion.test.tsx` 各自渲染 + 主互動 test。
  - `TabContentHeader.test.tsx` 增加 `stepIndicator` prop 渲染 test（既有 caller 不傳 = 不渲染、傳了 = 顯示在 cta 左側、cta undefined 時 chip 不渲染）。
  - `QuizTab.test.tsx` 增加 wizard-active 時 `TabContentHeader` 收到 `title="New quiz"` + `stepIndicator` + `cta=undefined` test；非 wizard 時恢復既有 props test。
  - i18n `workspace.test.ts` / `quiz.test.ts` 對新 key 增加 en + zh 覆蓋。
- 真實 CDP smoke（zh + en 兩 locale）：依 `project_cdp_smoke_webview2_pitfalls` 5 雷防範；每 step 截圖、reload 還原、cancel cleanup 驗證、stream tail 渲染、locale 切換；截圖存 `codebus-app/scripts/.quiz-wizard-smoke/`。

### Scope boundaries

- **In scope：** QuizTab wizard 重寫（content-area wizard with sidebar visible）、`TabContentHeader` 加 `stepIndicator` prop、wizard zustand store、`useUrlState` hook、5 個 wizard sub-component（Topic / ScopeConfirm / Generating / ReviewPending / Completion，外加 reviewing 透過 host QuizAnswering）、Karpathy 5-bucket scope confirm、i18n 新 key、QuizAnswering / QuizReview host 進 wizard（不改內部邏輯、加 optional `embedded` prop）、cancel cleanup（reuse `cancelQuiz`）。
- **Out of scope：** Workspace 殼層改動（含任何 Workspace-level store / Context / topbar element 新增）、Wiki page reader、ChatWidget 3 modes、Interrupted layout、LoadingOverlay live progress、backend Rust schema、引入 React Router、改名既有 i18n key、翻譯 Karpathy 5-bucket name、wizard 大幅動畫。

## Risks / Trade-offs

- [QuizTab 803 行重寫風險] → 不一次性丟掉舊 phase 機器，apply 階段 Task 8.2 先確認舊 phase 哪些 view component 在 wizard 內仍需 host（QuizAnswering / QuizReview / QuizGenerationLog 都會 host），確認重用 surface 後再拆 wizard 入口；舊 quiz list view 保留為 wizard 未啟動時的 fallback。
- [URL state 自製可能跟既有 IPC 觸發 navigation 邏輯衝突] → `useUrlState` 只 own `quiz_step` 與 `staged_id` 兩個 param，明確 scope 防越界。
- [wizard + ChatWidget 視覺衝突] → 驗收 acceptance smoke 明確 CDP 驗；若衝突則加 hide signal（不在此 change scope 預先決定）。
- [v1.1 mock 與 AUDIT 衝突 / propose user prompt vs 實機脫節] → 仲裁規則：mock + AUDIT + 實機優先，propose 階段假設讓步；本 change 已在「Pre-apply 校準」段處理 topbar、bucket count、sidebar、cancel IPC 四項。
- [Karpathy 5-bucket UI 細節（如某 bucket label 過長被截）] → apply 階段 CDP smoke 截圖驗；過長則由 wizard ScopeConfirm 組件做 truncate + tooltip，不改 identifier。
- [staged_id 在 reload 後 missing 的 fallback UX] → silently 回 quiz list 而非彈錯誤，避免 user 看到陌生 error；console.warn 留調試線索。
- [TabContentHeader 加新 prop 影響其他 caller] → optional prop，既有 caller 不傳即不渲染；test 同時涵蓋兩條路徑。
- [solo dev 直接 main、無 feature branch] → 大檔 wizard 重寫過程要保持 commit 可分段、tests 與 implementation 同 commit；apply 階段嚴守 TDD。
