## Why

Wiki 與 quiz 的 markdown 顯示目前有三個已定位的 fidelity gap：wiki body wikilink 在內文中不明顯、wiki code block 沒有語法高亮、quiz 文字片段會把 inline markdown delimiter 原樣顯示。這些問題跨 `WikiPreview`、quiz answering/review 與全域樣式契約，需先正式化為同一個 app-workspace change。

## What Changes

- 將 wiki body 的 `.plain-wikilink` 從內文同色改為 Obsidian 風格的 accent 文字，維持顯示頁面標題、不還原 `[[ ]]` 括號，並在 spec 中保留它與 `.cite-link` 的閉集差異。
- 讓 wiki markdown preview 的 fenced code block 透過 `rehype-highlight` 產生語法高亮 token，並使用 scoped 深色主題樣式與既有 `bg-bg-sunken` pre box 對齊。
- 將 quiz 的 stem、choices、explanation 與 completed review 中相同欄位改用共用 inline markdown renderer，支援 inline only：`code`、`**bold**`、`*em*`、既有 `[[wikilink]]` citation。
- 保留 quiz citation 的現有 wiki store 解析與 `.cite-link` 視覺語意，不引入 block markdown 支援。

## Non-Goals (optional)

詳細邊界記錄於 `design.md`。本 change 不包含 app 實作、archive、commit、Milkdown 重新啟用、ProseMirror 遷移、Rust IPC 變更、quiz block markdown 支援。

## Capabilities

### New Capabilities

(none)

### Modified Capabilities

- `app-workspace`: 修改 wiki/quiz markdown rendering 契約，包括 wikilink visual variants、wiki code block highlighting、quiz inline markdown rendering。

## Impact

- Affected specs:
  - `openspec/specs/app-workspace/spec.md`: MODIFY `Wikilink Plain and Citation Style Variants`; MODIFY `Quiz Answering and Summary`; ADD `Wiki Markdown Code Block Highlighting`。
- Affected frontend surfaces for apply:
  - `codebus-app/src/components/workspace/WikiPreview.tsx`
  - `codebus-app/src/components/workspace/ExplanationText.tsx`
  - `codebus-app/src/components/workspace/QuizAnswering.tsx`
  - `codebus-app/src/components/workspace/QuizReview.tsx`
  - `codebus-app/src/styles/globals.css`
  - `codebus-app/package.json` / lockfile
- Dependency impact:
  - Add `rehype-highlight` only.
  - Keep existing `react-markdown` and `remark-gfm`.
  - Keep existing unused `@milkdown/*` dependencies untouched for future migration.
