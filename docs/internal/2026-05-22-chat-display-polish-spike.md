# T3 Spike：Chat 文字顯示優化（GFM 表格 + `[[wikilink]]`）

**Date:** 2026-05-22
**Task:** loop T3（只讀探勘）
**背景:** [chat-display-polish backlog](2026-05-21-chat-display-polish-backlog.md)（2026-05-21）

---

## TL;DR

2026-05-21 backlog 對現碼核對**全屬實**，app 端可立即實作（半天、依賴與重用元件全現成）。**兩個值得標註的點**：(1) 此修法**provider-agnostic**——對 Claude / codex 一視同仁（純呈現層、operate on 正規化 Thought text），**無 PE2-C2 耦合**，是「乾淨」任務；(2) CLI 端的 `[[slug]]` 可點連結被刻意切給另一條 backlog（cli-wikilink-link-target），本條 CLI 只做 GFM/基本 markdown。

---

## 對現碼核對（✅ 屬實）

**App（`ChatTranscript.tsx`）**
- `AssistantMarkdownBlock`（`:401`）用 `ReactMarkdown`（`:2`）渲染，但 grep 確認**沒掛 `remarkGfm`** → GFM 表格原樣輸出。
- 對照組 `WikiPreview.tsx:4,145` 有 `import remarkGfm` + `remarkPlugins={[remarkGfm]}`，且 `remark-gfm` 已是 dep（**零新依賴**）。
- 連結只路由 markdown link 語法：`WIKI_HREF_RE=/^wiki\/.+\.md$/`（`:58`），`:54-55` 註解明說裸 wiki 路徑「deliberately left as inert prose」→ 確認 `[[slug]]` 不被處理。
- 重用元件現成：`lib/milkdown-wikilink.tsx` 已 export `transformBodyWikilinks()`（`:73`）與 `WikilinkLink`（`:32`，含 resolvable/unresolvable 判斷）——chat 只是沒接上。

**CLI（`commands/chat.rs`）**
- assistant 文字 `assistant_chunks.borrow().join("")`（`:192`）後直接 `println!`，**零 markdown 處理**。
- chat thought 是 REPL 自己 buffer+印（`:143,162,192`），**不經** `core` 的 `format_event` → 確認 backlog「改 render/stream_event.rs 解不掉、要在 chat 命令層或抽共用 helper」。

## 受影響檔案 / 工程量（沿用 backlog、已核實）

| 端 | 檔案 | 工程量 |
|---|---|---|
| App | `ChatTranscript.tsx`（`AssistantMarkdownBlock` 掛 remark-gfm + 接 `transformBodyWikilinks`/`WikilinkLink`/既有 `onWikiLinkClick`）+ table 最小 Tailwind 樣式 + vitest | **輕（半天）** |
| CLI | 移植 legacy `markdown_style` thought styling + OSC 8（`v2-rust/.../render/renderers/terminal.rs` 有前例）；抽共用 thought-render helper。表格渲染範圍**先 brainstorm**（終端表格對齊成本高、可能引 `termimad` crate） | **中（约 1 個半天；表格另計）** |

## 標註點

1. **無 PE2 耦合（與 T2 不同）**：本條 operate on 正規化的 Thought text，Claude 的 `text` 與 codex 的 `agent_message` 都映成 Thought（PE1 確認兩者一致），所以 app/CLI 一改、兩 provider 同等受益。不像 T2 要等 codex parser 補 event。→ **可獨立先行、無前置**。
2. **CLI `[[slug]]` 可點連結不在本條**：已切給 [cli-wikilink-link-target](2026-05-21-cli-wikilink-link-target-backlog.md)（含 `codebus://` deep-link）。本條 CLI 只解 GFM/基本 markdown 呈現；app 端則因 `WikilinkLink` 現成，順手把 `[[slug]]` 一起做。
3. **建議拆兩段落地**：app 半天（低風險、立即見效）先做；CLI 端動工前先定表格範圍，別一次吃下整個終端 markdown 渲染器（backlog out-of-scope 已明確警告）。

## Out of scope（沿用 backlog）
- 改 StreamEvent / parser / 後端（資料已是完整 markdown 文字）。
- chat 以外 surface（Activity Stream 摘要、Run Detail）。
- 終端「完整 markdown 渲染器」。

## 待 harry
app 半天那段幾乎零風險、零依賴，最值得先批。CLI 端要先回答：終端表格要做到什麼程度（純 `[[slug]]` 樣式+OSC8 / 還是含表格對齊）？這決定 CLI 是 1 個半天還是更多。
