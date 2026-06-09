# T8 品質檢查：codebus-app（前端）

**Date:** 2026-05-22
**Task:** loop T8（只讀分析，產 backlog 候選）
**範圍:** app 11385 LOC（TS/TSX）。聚焦安全 lens（markdown/連結 XSS、IPC 邊界）+ React 常見 bug 源（exhaustive-deps 抑制、stale closure）。

---

## TL;DR — 前端防禦面紮實，無真實 bug

不像 core（F1）/ cli（F4）各挖到 latent 安全 bug，**前端這輪沒找到真實缺陷**。TS 紀律好（無 `any` / `dangerouslySetInnerHTML` / `@ts-ignore`），markdown 連結渲染對 XSS 安全，exhaustive-deps 抑制都有正當理由。**唯一實質產出是一個 T3 實作細節的修正**（見下）。

## 安全核對（✅ 通過）

- **無 raw HTML 注入面**：全 codebase 無 `dangerouslySetInnerHTML`、無 `rehype-raw`。markdown 走 `ReactMarkdown`（預設不渲染 raw HTML + 內建 `urlTransform` 濾危險 scheme）。
- **chat 連結渲染雙重防護**（`ChatTranscript.tsx:407-447`）：wiki href → `<button>` 無 href；external 僅當 `EXTERNAL_HREF_RE=/^https?:/i`（`:59`）通過才開，且走 `openExternalUrl` + `preventDefault`；其餘 → 惰性 `<span>`。`javascript:` / `data:` 等不符 `^https?:` → 落入惰性分支，**無 XSS**。
- **wikilink 轉換安全**（`milkdown-wikilink.tsx:83`）：`encodeURIComponent(slug)` 包 href，link text 經 ReactMarkdown 逸出。`WikilinkLink` 用 `href="#"` + `onClick preventDefault`，resolvable 判定用 `Object.prototype.hasOwnProperty.call`（避開原型污染）。

## React 正確性核對（✅ 可接受）

- `QuizTab.tsx:386,403` 兩個 `exhaustive-deps` 抑制：**有正當理由**——one-shot 生成用 `firedForPageRef` latch 防 StrictMode 雙呼叫 + 同值重觸發，刻意只依賴 `[pendingPage]`。把 `startGenerate`/`onPendingConsumed` 加進 deps 反而會誤觸發。屬標準 latch pattern，非 bug。
- `SettingsModal.tsx:182` `no-new` 抑制：未深究，低風險。

## 🟡 次要觀察（低）

1. `EXTERNAL_HREF_RE=/^https?:/i` 偏鬆——`https:foo`（無 `//`）也通過，會把畸形 URL 丟給 OS opener。非危險 scheme、低風險；可收緊成 `/^https?:\/\//i`。
2. `ChatTranscript.tsx:285` 有未接的 i18n `TODO(task 7.2)`（locale hint），技術債、非 bug。

## 🔵 修正 T3（chat-display-polish）的實作假設 — 有價值

讀實際 renderer 後發現 [T3 spike](2026-05-22-chat-display-polish-spike.md) 的「app 端接 `transformBodyWikilinks`」**比想像多兩步**：

1. `transformBodyWikilinks` 產出的是 `codebus://wiki/<slug>` href，但 **react-markdown 預設 `urlTransform` 會把非 http/https/mailto 的 scheme（含 `codebus://`）洗掉** → href 變空。T3 落地時 **必須給 `<ReactMarkdown urlTransform={...}>` 放行 `codebus://`**（WikiPreview 用 Milkdown 沒這問題，但 chat 用 react-markdown 有）。
2. chat 現有的 link 分流靠 `WIKI_HREF_RE=/^wiki\/.+\.md$/`（`:58`），**匹配不到** `codebus://wiki/<slug>`。T3 要嘛改 `WIKI_HREF_RE` 認 `codebus://wiki/` scheme、要嘛 transform 直接產 `wiki/<slug>.md` 形式對齊現有 regex。

→ 建議更新 T3 spike：app 端「半天」要含這兩步（urlTransform 放行 + href 分流對齊），否則接上去 wikilink 會靜默失效。

## 後續 review 候選（T8 未深讀）
- `lib/ipc.ts`（1032，Tauri invoke 型別邊界 + event payload 契約）。
- `store/chat.ts`（530）/ `store/settings.ts`（351，已於 T1 看過 normalizer）。
- `i18n/messages.ts` 覆蓋完整度（接 ChatTranscript:285 的 TODO）。

## 待 harry
前端體質好，無急需修的安全項。實質行動是：T3 落地時記得處理上面的 `codebus://` urlTransform + href 分流（已回寫進 T3 spike 的待辦理解）。
