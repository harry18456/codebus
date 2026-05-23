# Backlog: Chat assistant 文字顯示優化（GFM 表格 + `[[wikilink]]`，app + CLI 兩邊）

**Date:** 2026-05-21
**Surfaced during:** 實際 chat 對話（問「這個專案的目的是什麼？」AI 回覆含 `[[project-purpose]]` 連結與 markdown 表格，兩者都沒被渲染）
**Severity:** UX 補強（chat 可讀性 / 一致性）
**Owner:** harry
**Status:** open

---

## 觀察

chat 的 assistant 回覆常含兩種 markdown 元素，但**兩個前端都把它們當原始文字丟出來**，沒有渲染：

1. **GFM 表格** —— 三欄對照表（例：`| uv 取代的工具 | 對應 uv 命令 |`）顯示成一堆 `|` 和 `---` 的原始 markdown，而不是排版好的表格。
2. **`[[slug]]` 雙括號 wikilink** —— 例 `[[project-purpose]]` 顯示成字面文字，點不開、也沒視覺區隔，不像 wiki 引用。

兩條獨立程式路徑、症狀相同：

### App（`codebus-app/src/components/workspace/ChatTranscript.tsx`）

- `AssistantMarkdownBlock` 用 `<ReactMarkdown>` 渲染，但**沒有掛 `remark-gfm`** → GFM 表格不被解析，原樣輸出。對照組：`WikiPreview.tsx:145` 有 `remarkPlugins={[remarkGfm]}`，且 `remark-gfm` 已在 `package.json` deps（無需新增依賴）。
- 連結只路由 markdown link 語法（`[label](wiki/foo.md)` → `WIKI_HREF_RE`），**不處理 `[[slug]]`**。`ChatTranscript.tsx:54-55` 註解明說「Plain-text wiki paths NOT wrapped in markdown link syntax are deliberately left as inert prose」。對照組：`milkdown-wikilink.tsx` 的 `transformBodyWikilinks()` 已能把 `[[slug]]` 轉成 anchor、`WikilinkLink` 已能判斷 resolvable/unresolvable——可重用，chat 只是沒接上。

### CLI（`codebus-cli/src/commands/chat.rs`）

- 回覆走 `assistant_chunks.borrow().join("")` 後直接 `println!("{full_text}")`（`chat.rs:192-194`），**完全沒有 markdown 處理**——表格、`[[slug]]` 全是純文字。
- 注意：CLI chat 的 thought text **沒有**經過 `codebus-core` 的 `format_event`，是 chat REPL 自己 buffer + 印；所以這不是改 `render/stream_event.rs` 就能順帶解掉的，要在 chat 命令層或抽一個共用 thought-render helper。
- **`[[slug]]` 的可點連結另立專條**：CLI 把 `[[slug]]` 渲染成可點連結（補回 v2、連結目標 app/obsidian 可設定、含 `codebus://` deep-link 協定）已獨立為 [cli-wikilink-link-target](2026-05-21-cli-wikilink-link-target-backlog.md)。本條只管 chat 文字的 GFM 表格與基本 markdown 呈現，`[[slug]]` 連結化交給那條。
- 前例存在：legacy `v2-rust/codebus-core/src/render/renderers/terminal.rs` 對 `Thought` 文字做過「lightweight markdown styling（`markdown_style::style_thought_text`）+ OSC 8 把 resolvable `[[slug]]` 包成 `obsidian://open?vault=...&file=...` 超連結，不支援的終端自動降級為純樣式」。v3 沒沿用，可當設計參考。

## Proposed fix

**App（輕）**

- `AssistantMarkdownBlock` 加 `remarkPlugins={[remarkGfm]}`（依賴已在）。
- 在 markdown render 前先跑 `transformBodyWikilinks()` 把 `[[slug]]` 轉成可點 anchor，沿用既有 `WikilinkLink` resolvable/unresolvable 判斷與既有 `onWikiLinkClick` 路由（點擊切到 wiki tab + `loadPage`，並摺疊 chat widget）。
- **（T8 補充 2026-05-22）** `transformBodyWikilinks` 產出的是 `codebus://wiki/<slug>` href，要落地需多兩步，否則接上去 wikilink 會靜默失效：
  1. react-markdown 預設 `urlTransform` 會把非 http/https/mailto 的 scheme（含 `codebus://`）洗成空 href。`AssistantMarkdownBlock` 必須給 `<ReactMarkdown urlTransform={...}>` 放行 `codebus://`（WikiPreview 用 Milkdown 無此問題）。
  2. chat 現有 link 分流靠 `ChatTranscript.tsx:58 WIKI_HREF_RE=/^wiki\/.+\.md$/`，**匹配不到** `codebus://wiki/<slug>`。要嘛改 `WIKI_HREF_RE` 認 `codebus://wiki/` scheme，要嘛 transform 直接產 `wiki/<slug>.md` 形式對齊現有 regex（後者較省）。
- table 需要的話加最小 Tailwind 樣式（border/padding）避免裸 `<table>` 難讀。

**CLI（中）**

- 先決定範圍：是要「終端 markdown 渲染」到什麼程度（表格對齊、粗體/標題）還是只解 `[[slug]]` 視覺化 + OSC 8 超連結。建議**先抄 legacy 的 thought styling + OSC 8 路徑**，表格渲染另議（終端表格對齊成本高，可能引第三方 crate 如 `termimad`，需先評估）。
- 把 thought-render 抽成 core 內可共用的 helper，避免 chat 命令層各寫一份。

## Tasks（粗估）

1. **App**：`AssistantMarkdownBlock` 掛 `remark-gfm` + 接 `transformBodyWikilinks`/`WikilinkLink`；vitest 補「表格被渲染成 `<table>`」「`[[slug]]` 變可點 link、點擊觸發 `onWikiLinkClick`」「unresolvable slug 為惰性」。
2. **CLI**：移植 legacy `markdown_style` thought styling + OSC 8 `[[slug]]` → `obsidian://open`（依 `RenderOptions` 能力降級）；表格渲染範圍先 brainstorm 再定。
3. 手動驗收：app 與 CLI 各跑一次含表格 + `[[slug]]` 的 chat 回覆，確認渲染正確、降級安全。

工程量：app 輕（半天）；CLI 中（thought styling + OSC 8 約 1 個半天，表格渲染若做另計）。

## Out of scope

- 改 `StreamEvent` / parser / 後端資料（資料已是完整 markdown 文字，純呈現層問題）。
- chat 以外的 surface（Activity Stream tool 一行摘要、Run Detail）——它們本就刻意摘要，不在本條。
- 終端「完整 markdown 渲染器」（語法高亮、巢狀清單等）這種大範圍需求；本條只解表格 + wikilink 兩個高頻痛點。

## 何時動

無硬依賴。app 那半天隨時可插（依賴與重用元件都現成）；CLI 端動工前先 brainstorm 表格範圍，別一次吃下整個終端 markdown 渲染器。
