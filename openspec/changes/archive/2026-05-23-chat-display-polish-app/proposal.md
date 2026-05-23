## Summary

App 端 chat assistant 訊息渲染補上 GFM 表格 + `[[slug]]` wikilink 可點化（含 resolvable / unresolvable 視覺區分），順手修一個既有 callback contract type-lie。

## Motivation

實際 chat 使用時 assistant 回覆常含兩種 markdown 結構但前端把它們當原始文字丟出：

- **GFM 表格**：`| col1 | col2 |\n|---|---|\n` 顯示成一堆 `|` 與 `---` 的原始 markdown
- **`[[slug]]` wikilink**：顯示成字面 `[[some-page]]` 文字，點不開、無視覺區隔

對照組 `WikiPreview`（vault 內 wiki 頁面渲染）已正確掛 `remark-gfm` + `transformBodyWikilinks` + `urlTransform={(url) => url}` 三件事，並 inline render `codebus://wiki/<slug>` 的 anchor 成 resolvable / unresolvable 視覺。Chat 的 `AssistantMarkdownBlock` 缺這整套——本 change 把 WikiPreview 的渲染 pattern 平移過來，讓 chat 與 wiki preview 兩個 surface 的 markdown 行為一致。

順帶發現一個既有 latent bug：`ChatTranscript.tsx::AssistantMarkdownBlock` 的 `a` override 在 wiki link click 時呼叫 `onWikiLinkClick(href)`（傳整段 href 例如 `wiki/modules/auth.md`），但 `Workspace.tsx::onSelectPage(slug)` 預期 **slug**（例如 `auth`），會把 href 當 slug 餵 `useWikiStore.loadPage(vault, "wiki/modules/auth.md")` → `readWikiPage` 抓 `wiki/wiki/modules/auth.md.md` 路徑 → 抓不到頁。沒人爆是因為 chat 從未真正有 clickable wiki link（這次才補）。`app-workspace` spec line 1760 也已含「`(or the equivalent slug modules/auth)`」字句，承認意圖是 slug。本 change 順手把 callback contract 校正為 slug-based、同時更新對應 spec scenario 與既有測試 assertion。

## Proposed Solution

**Code（`codebus-app/src/components/workspace/ChatTranscript.tsx`）**

把 `AssistantMarkdownBlock` 改造成沿用 `WikiPreview` 的 markdown render pattern：

1. **預處理**：呼叫 `transformBodyWikilinks(text)` 把 `[[slug]]` 轉成 `[<slug>](codebus://wiki/<encoded>)`，再餵給 `<ReactMarkdown>`
2. **掛 remark-gfm**：`remarkPlugins={[remarkGfm]}`（依賴 `remark-gfm` 已在 `package.json`）
3. **放行 codebus:// scheme**：`urlTransform={(url) => url}`（react-markdown 預設 urlTransform 會把 non-http(s)/mailto scheme 洗成空 href）
4. **重構 `a` component override** 為三條 branch（**按優先序**）：
   - href 是 `codebus://wiki/<encoded>` → 解析 slug → 從 `useWikiStore.pages` 查 resolvable → resolvable 顯示為 button + 顯示 `meta.title ?? slug`、unresolvable 顯示為灰字 span + tooltip「Page not found」
   - href 匹配 `^wiki\/(.+)\.md$` → 抽 slug（capture group 1）→ 同上 resolvable / unresolvable 流程（保留 legacy markdown link 形式相容）
   - href 匹配 `^https?:` → 既有外部連結 behavior 不變（Tauri opener）
   - 其他 → 既有 inert `<span>`
5. **callback contract 修正**：`ChatTranscriptProps.onWikiLinkClick` 從 `(href: string) => void` 改成 `(slug: string) => void`。內部 a-handler 在點擊時傳已抽出的 **slug**（不再傳 raw href）。Workspace 的 `onSelectPage(slug)` 不動。
6. **加 `<table>` 最小 Tailwind 樣式**：透過 `components.table` 套上 `border-border` / padding，避免裸 `<table>` 難讀
7. **`WikilinkLink` component 不使用**：grep 確認零 production caller、且 visual 帶 `[[brackets]]` 與 chat prose 不合。Inline render 仿 WikiPreview pattern。

**Spec（`app-workspace` 的 `Chat Assistant Message Markdown Rendering and Wiki Citation Links` MODIFIED）**

- 加入 GFM 表格渲染要求（`remark-gfm` plugin）
- 加入 `[[slug]]` wikilink 預處理 + resolvable / unresolvable 渲染要求（沿用 Wikilink Resolution and Click Behavior 的 client-side pages map 查找，但 surface 是 chat）
- callback contract：明示傳遞 **slug**、不是 href；legacy `wiki/<path>.md` 形式 callback 同樣抽 slug 後傳
- 既有 scenarios（wiki markdown link、external、inert）assertion 對齊 slug
- 新增 scenarios：GFM 表格渲染、`[[slug]]` resolvable click、`[[slug]]` unresolvable 顯示

**Tests（`codebus-app/src/components/workspace/ChatTranscript.test.tsx`）**

- 更新既有 3 條 assertion：`onWikiLinkClick` 被呼叫帶 slug（如 `"modules/auth"` 或 `"auth"`——依抽法決定）而非 href
- 新增測試：GFM 表格 input → DOM 內有 `<table>` 與至少一個 `<th>`
- 新增測試：`[[some-page]]`（純文字） + `pages` map 有 `some-page` → 渲染 resolvable button、click 觸發 `onWikiLinkClick("some-page")`
- 新增測試：`[[nonexistent]]` + `pages` map 沒此 slug → 渲染 unresolvable span（無 click handler、有 tooltip 文字）

## Non-Goals (optional)

- **不動 CLI 端 chat 渲染**（terminal markdown + OSC 8 wikilink 是另一條 backlog 項目「CLI `[[slug]]` 可點連結」，重量級且 codebus:// 協定相依）
- **不移除 dead code `WikilinkLink`**（零 caller，可獨立移除；但本 change 只專注 chat polish，移除留 refactor cleanup 另開）
- **不改 `transformBodyWikilinks` 自身輸出格式**（共用給 WikiPreview，動就要兩邊同步驗，scope 暴增）
- **不擴 markdown plugin 集**到 `remark-gfm` 之外（如 `remark-math`、`rehype-raw`）——表格是當下實際痛點，其他 plugin 沒 use case
- **不改 chat 與 wiki tab 之間的 widget 摺疊行為**（既有 `useChatStore.toggleExpanded()` 邏輯保留）
- **不引入 page title fetch / lazy load**——pages map 已是 client-side state（Workspace mount 時載入），沿用即可

## Alternatives Considered (optional)

- **Alt 1：直接 import `WikilinkLink`**——`milkdown-wikilink.tsx:32-63` 已存在，但 visual 帶 `[[brackets]]` 不適合 chat 自然 prose 流。且 WikiPreview 自己也不用 WikilinkLink、是 inline render。我跟 WikiPreview pattern 保持一致。
- **Alt 2：改 `transformBodyWikilinks` 直接產 `wiki/<slug>.md` 形式對齊既有 chat regex**——會破壞 WikiPreview（共用此 helper），兩邊都要改，scope 翻倍。否決。
- **Alt 3：保留現 callback contract 不修 latent bug**——bug 不爆是因為 chat 從未真正有 clickable wiki link，補完本 change 後就會爆。順手修成本 5 行，否決「不修」。

## Impact

- Affected specs:
  - `app-workspace`（`Chat Assistant Message Markdown Rendering and Wiki Citation Links` requirement MODIFIED）
- Affected code:
  - Modified:
    - codebus-app/src/components/workspace/ChatTranscript.tsx（`AssistantMarkdownBlock` 改造 + props type、`WIKI_HREF_RE` 加 capture group）
- Tests:
  - codebus-app/src/components/workspace/ChatTranscript.test.tsx（3 條既有 assertion 對齊 slug + 3 條新增覆蓋 GFM table / resolvable / unresolvable）
- 不影響：
  - `codebus-app/src/components/workspace/WikiPreview.tsx`（既有 wiki tab 渲染保持）
  - `codebus-app/src/lib/milkdown-wikilink.tsx`（共用 helper `transformBodyWikilinks` 簽名與輸出不變；`WikilinkLink` dead-code 不動）
  - `codebus-app/src/store/wiki.ts`（pages map 既有結構不變）
  - `Workspace.tsx::onSelectPage(slug)`（已預期收 slug、不必動）
  - 後端 Rust（codebus-core / codebus-cli）完全不碰
- 跨平台：純 TSX / React 改動，無 OS-specific 行為。
