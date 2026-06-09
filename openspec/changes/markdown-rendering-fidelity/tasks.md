## 1. Grounding 與依賴邊界

- [x] 1.1 apply 前重讀目前 wiki 與 quiz rendering surfaces，確認文件中的進入點仍對應 `WikiPreview`、`ExplanationText`、`QuizAnswering`、`QuizReview` 與 `globals.css`；驗證：在 apply notes 記錄 grep/read 結果，且確認 `spectra validate markdown-rendering-fidelity` 仍以 `app-workspace` 為目標。
- [x] 1.2 只新增 `Wiki Markdown Code Block Highlighting` 與 D2 Highlight 主題與作用域所需的 `rehype-highlight` dependency；驗證：package diff 只顯示 `rehype-highlight`，沒有 `@milkdown/*` 變更，lockfile diff 沒有無關 dependency intent。

## 2. Wikilink 視覺 Variants

- [x] 2.1 實作 D1 Wikilink 底線定義的 `Wikilink Plain and Citation Style Variants` body-link 行為：resolvable wiki body links 以 accent `.plain-wikilink` label 顯示，不顯示 raw brackets，且只有 hover/focus 顯示 underline；驗證：`WikiPreview` tests 斷言 class、label、click navigation、沒有 raw `[[ ]]`。
- [x] 2.2 保留 `Wikilink Plain and Citation Style Variants` citation 行為為可區分的 `.cite-link` variant：monospace、accent、dashed underline，並保留既有 resolvable/unresolvable wiki store 行為；驗證：quiz/chat citation tests 斷言 citation anchors 有 `.cite-link` 且沒有 `.plain-wikilink`。

## 3. Wiki Code Block Highlighting

- [x] 3.1 為 `Wiki Markdown Code Block Highlighting` 將 `rehype-highlight` 接入 wiki markdown renderer，同時保留 `remark-gfm`、custom wikilink routing 與既有 inline code chip 行為；驗證：`WikiPreview` tests 以 Rust code fence 斷言 highlight token descendants，並斷言 inline code 不帶 block highlight classes。
- [x] 3.2 依 `Wiki Markdown Code Block Highlighting` 與 D2 Highlight 主題與作用域，在 wiki preview container 下新增 scoped dark theme rules，pre background/border 仍由既有 app tokens 持有；驗證：style/CSS review 確認 selectors scoped，manual/CDP check 確認 Rust fixture 在既有 sunken pre box 內有高亮。

## 4. Quiz Inline Markdown Rendering

- [x] 4.1 依 D3 Quiz inline markdown 子集，把 `ExplanationText` 擴成 shared inline renderer，只支援 inline `code`、`strong`、`em` 與 `[[wikilink]]`；驗證：unit tests 證明 code/bold/em 以語意 DOM 渲染、wikilink citations 仍可解析，且 heading/fence/table-like input 不產生 block DOM。
- [x] 4.2 將 `Quiz Answering and Summary` inline markdown rendering 套到 `QuizAnswering` 的 stem、choices、explanation；驗證：`QuizAnswering` tests 顯示 `codebus-core` 不再露出 raw backticks、choices 可渲染 inline formatting、explanation wikilinks 仍可導航。
- [x] 4.3 將 `Quiz Answering and Summary` inline markdown rendering 套到 `QuizReview` 的 stem、choices、explanation；驗證：`QuizReview` tests 覆蓋 completed attempt review text 的 inline code/bold/em，以及 resolvable/unresolvable wikilinks。

## 5. Validation

- [x] 5.1 對已變更 surfaces 執行 frontend verification；驗證：`WikiPreview`、`QuizAnswering`、`QuizReview` 與 inline renderer 的 targeted Vitest suites 通過，接著跑 `npm run typecheck` 或 repo-standard TypeScript check。
- [x] 5.2 對完成後的 apply 執行 spectra verification；驗證：`spectra validate markdown-rendering-fidelity` 通過，且 `Wikilink Plain and Citation Style Variants`、`Quiz Answering and Summary`、`Wiki Markdown Code Block Highlighting` deltas 保持完整。
