## Context

`spectra list --json` 回傳 `changes: []`，目前沒有 active change 與 `markdown-rendering-fidelity` 衝突。

已讀取並對齊現況：

- `WikiPreview.tsx` 的 markdown renderer 只掛 `remarkGfm`；`code` / `pre` renderer 只保留 `language-*` class，沒有 rehype highlighter，也沒有 token span。
- `WikiPreview.tsx` 的 body wikilink anchor 使用 `plain-wikilink text-fg underline decoration-border-strong ... hover:text-accent`，實際預設狀態仍接近內文。
- `globals.css` 的 `.plain-wikilink` 是 foreground + strong-border underline；`.cite-link` 是 monospace + accent + dashed underline。WP11 spec lock 指向 `Wikilink Plain and Citation Style Variants`。
- `QuizAnswering.tsx` 的 stem 與 choices 直接輸出 raw string；explanation 經 `ExplanationText`。`QuizReview.tsx` 同樣 raw 輸出 stem/choices，explanation 經 `ExplanationText`。
- `ExplanationText.tsx` 只用 regex 拆 `[[slug]]`，其餘文字原樣輸出，因此 quiz 路徑中的 inline code / bold / emphasis delimiter 會漏出。wiki preview 路徑不受此問題影響，因為它經 `react-markdown`。
- `codebus-app/package.json` 已有 `react-markdown ^10.1.0` 與 `remark-gfm ^4.0.1`，尚未有 `rehype-highlight`；`@milkdown/*` 仍存在但目前未用。

## Goals / Non-Goals

**Goals:**

- 正式化 wiki body wikilink 的 Obsidian 風格 accent 顯示，維持 `plain-wikilink` 與 `cite-link` 兩個視覺 variant 可辨識。
- 正式化 wiki code block 語法高亮，採用 `rehype-highlight` 與 scoped 深色主題。
- 正式化 quiz inline markdown 支援範圍，涵蓋 `QuizAnswering` 與 `QuizReview` 的 stem、choices、explanation。
- 讓 apply 階段有明確 dependency、CSS scope、renderer 抽象與驗證邊界。

**Non-Goals:**

- 不在 propose 階段寫任何 app 實作程式碼。
- 不 archive、不 commit。
- 不重新啟用 Milkdown，不做 ProseMirror 遷移。
- 不新增 quiz block markdown 支援；code fence、table、heading、list、blockquote 均不進入 quiz renderer scope。
- 不改 Rust IPC、quiz markdown parser、wiki store page resolution model。
- 不加入 speculative abstraction；inline renderer 只因 answering/review 至少六個真實文字呈現點共用同一 tokenize 行為而抽出。

## Decisions

### D1 Wikilink 底線

已拍板：wiki body `.plain-wikilink` 預設文字改為 accent，顯示頁面標題，不顯示 `[[ ]]`。

建議採用：accent 文字 + hover/focus-only accent 底線。預設不顯示 persistent underline，讓 body navigation 與 `.cite-link` 的 monospace + dashed underline 在視覺上保持閉集差異。

備選方案：

- 保留 persistent underline：辨識度高，但與 citation dashed underline 的輪廓過近。
- 改為細實線 persistent underline：比現況清楚，但仍可能與 citation link 混淆。
- 完全移除 underline：區分度最高，但 keyboard focus 與 hover affordance 較弱。

規格建議：`plain-wikilink` = accent + proportional font + hover/focus underline；`cite-link` = accent + monospace + dashed underline。兩者都不引入 visited state；unresolvable wikilink 維持 dimmed non-clickable。

### D2 Highlight 主題與作用域

建議採用：`rehype-highlight` + `github-dark` 風格的 scoped theme rules，作用域限定在 wiki preview 容器，例如 `.wiki-preview .hljs` 與 `.wiki-preview .hljs-*`。主題背景不要覆蓋 pre box；`pre` 繼續使用既有 `bg-bg-sunken`、border、padding，highlight theme 只負責 token 顏色。

備選方案：

- 直接全域 import `highlight.js/styles/github-dark.css`：成本最低，但 `.hljs` 會污染非 wiki markdown surface。
- 使用 `atom-one-dark`：token 對比清楚，但背景色需額外覆寫才能貼合 `bg-bg-sunken`。
- 自訂少量 token 色：scope 最乾淨，但不符合「現成深色主題」方向。

依賴邊界：只新增 `rehype-highlight`。主題 CSS 可用現成 theme 色票作 scoped copy，不新增 `highlight.js` 直接 dependency，不動 `@milkdown/*`。

### D3 Quiz inline markdown 子集

建議採用：把 `ExplanationText.tsx` 擴成共用 inline markdown renderer，保留 `ExplanationText` export 或提供相容 wrapper，並套到 `QuizAnswering` 與 `QuizReview` 的 stem、choices、explanation。

支援子集固定為 inline only：

- `code`
- `**bold**`
- `*em*`
- `[[wikilink]]`

不支援 block markdown：code fence、table、heading、list、blockquote 在 quiz text fragment 中不得產生 block DOM。這些內容若出現，應以 literal 或 flattened inline text 顯示，避免 quiz 單行片段變成多區塊 layout。

抽象理由：同一 tokenizer/resolution 行為需覆蓋 `QuizAnswering` stem、四個 choices、explanation，以及 `QuizReview` stem、四個 choices、explanation。這是跨至少三類欄位與兩個 view 的真實共用，不是預先抽象。

## Implementation Contract

行為：

- Wiki body 中可解析的 wikilink 以 accent 色 `.plain-wikilink` label 顯示；頁面 title 已知時用 title，否則 fallback 到 slug。點擊仍走既有 wiki store 路徑，不顯示原始括號。
- Quiz/chat citation wikilink 以 `.cite-link` 顯示，維持 monospace + accent + dashed underline。citation 樣式必須與 body navigation link 保持視覺可辨識。
- Wiki fenced code block 在語言可辨識時產生 highlight token span/class。Inline code 維持既有 chip 處理，不當成 block 做語法高亮。
- Quiz answering 與 review 的 stem、choices、explanation 會把 inline markdown delimiter 渲染為格式；`codebus-core` 外層 raw backtick 不再出現在畫面。
- Quiz explanation 既有 `[[slug]]` 解析保持不變：resolvable citation 透過 `onOpenWikiPage` 導航；unresolvable citation dimmed 且 inactive。

介面 / 資料形狀：

- `WikiPreview` 繼續使用 `react-markdown` 與 `remark-gfm`；apply 透過 `rehypePlugins` 加入 `rehype-highlight`。
- Wiki preview root 需要穩定 CSS scope class，例如 `wiki-preview`，避免 highlight theme selector 全域外洩。
- `.plain-wikilink` 與 `.cite-link` 維持 literal class name，因為 app-workspace spec 把它們視為視覺 variant contract。
- 共用 quiz inline renderer 接收 `{ text, pages, onOpenWikiPage }` 或等價 shape，並維持 `WikiPageMeta` 以 slug 為 key 的 lookup。

失敗模式：

- 未知或缺失語言的 code block 仍在既有 pre box 內保持可讀，不要求 token span。
- 不支援的 quiz block markdown 不產生 block-level quiz layout；以 literal 或 flattened inline content 顯示。
- Unresolvable wikilink 維持目前 `Page not found` tooltip 與 inactive dimmed presentation。
- Reduced-motion 使用者的 link color/underline 變化立即套用，不做 transition。

驗收條件：

- Unit tests 覆蓋 `.plain-wikilink` 預設 accent styling contract 與 `.cite-link` 區分。
- Unit tests 以 Rust fixture 覆蓋 wiki code fence highlighting，並確認 inline code 仍是 chip。
- Unit tests 覆蓋 `QuizAnswering` 與 `QuizReview` 的 stem、choices、explanation inline markdown。
- Tests 確認 `[[wikilink]]` 對 resolvable pages 仍可點擊，對 unresolvable pages 仍 inactive。
- Manual/CDP 驗證可使用 `.codebus` vault：`modules/desktop-app-workspace.md` 看 Rust code block、`synthesis/main-features.md` 看 body wikilinks、既有 quiz attempt 看 inline code。
- apply 開始前與實作完成後都要通過 `spectra validate markdown-rendering-fidelity`。

Scope boundaries：

- Apply 不得修改 Rust IPC 或 quiz generation output contracts。
- Apply 不得新增 `rehype-highlight` 以外的 dependency。
- Apply 不得移除或重接 `@milkdown/*`。
- Apply 不得引入 quiz block markdown 支援。
- Apply 必須涵蓋 `QuizAnswering` 與 `QuizReview`；只覆蓋 explanation text 不算完成。

## Risks / Trade-offs

- CSS theme leakage -> 將 highlight selectors 限制在 wiki preview container 下，並驗證全域 `.hljs` styling 不影響 chat 或 quiz。
- Theme background conflict -> 讓 `pre` 持有 background/border/padding，highlight theme 只處理 code 內 token 顏色。
- Inline renderer 誤渲染 block markdown -> 限制允許元素，並加入 heading/fence/table-like input 測試。
- 既有 tests 斷言 plain text output -> 改為斷言渲染後 DOM 語意，而不是 raw markdown delimiter。
- Rehype language coverage variance -> 驗證常見 Rust fixture；未知語言仍可讀且不讓 render 失敗。
