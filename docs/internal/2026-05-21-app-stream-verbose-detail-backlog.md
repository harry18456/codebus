# Backlog: App Activity Stream 顯示完整 AI 回覆細節

**Date:** 2026-05-21
**Surfaced during:** discuss 2026-05-21（「debug 模式看 AI 詳細回覆」討論，CLI 部分另起 change 處理）
**Severity:** UX 補強（觀察 agent 行為時有感）
**Owner:** harry
**Status:** open

---

## 觀察

App 的 Activity Stream（`codebus-app/src/components/workspace/ActivityStreamItem.tsx`）跟 CLI 一樣，把 agent 的 tool 活動**摘要/截斷**後才顯示——`summarizeToolInput()` 把複雜 input 收成摘要、Write/Edit 只顯示 `writeEditPath()`（basename）、Bash command `obj.command.slice(0, 79)…`。結果是：使用者看得到「agent 想了什麼、用了哪個 tool」，但看不到「tool 的完整 input」「tool 拿到的完整 result」。

關鍵事實（決定本條範圍）：

- App 的 renderer 跟 CLI 的 Rust `format_event` 是**兩條獨立程式路徑**，無共用程式碼。CLI 的詳細模式由 `--debug` 解（已另起獨立 change，動 `codebus-core` render + `cli` flag）。
- App 收到的 `StreamEvent` 資料**本來就完整**（`event.data.input` 全量 `Value`、`event.data.output` 全量字串；events.jsonl 也存完整）。截斷只發生在前端 TSX。
- 因此本條是**純前端 follow-up，後端零改動**。

## Proposed fix（discuss 2026-05-21 收斂，觸發 UX 已定案）

採 **per-line 展開按鈕**——把現有 `ThoughtItem` 的「`▼` 展開 / `▲ collapse`」模式延伸到 timeline 的每一行 tool。不採全域 verbose 開關（B）/ settings 開關（C）：使用者要的是「需要時點開那一行看細項」，granular 不洗版，且 thought 已有同款先例。

具體行為（改在共用的 `ActivityStreamItem` + `foldTimeline`）：

- **tool_use 行**（`🛠️ Read · CLAUDE.md`）：摺疊=現有摘要；展開=完整 input + **配對的 tool_result**（該 tool 拿回什麼）。例：
  ```
  🛠️ Read · CLAUDE.md   ▲ collapse
     input:  D:/side_project/uv/.codebus/CLAUDE.md
     result: <CLAUDE.md 完整內容>
  ```
- **Write/Edit 行**（`✍️ <path>`）：展開=完整寫入內容（解開 `writeEditPath` 只給 basename 的摘要）。
- **thought / prose 不動**：chat 的 thought_block 已用 `AssistantMarkdownBlock` 完整 markdown 渲染；goal Run Detail 的 thought 已有 `ThoughtItem` 折疊 + `▼`。兩者各自已處理，本條不碰。

**一處改動、全 surface 生效**：`ActivityStreamItem` + `foldTimeline` 被 5 處共用（RunDetailRunning / RunDetailDone 的 Run details 摺疊區 / ChatTranscript / QuizGenerationLog / QuizTab），所以 tool 行展開做一次，goal running/done + chat + quiz 一起拿到——使用者明確希望 chat 也有，這點自動涵蓋。

**關鍵實作 edge**：現在 `tool_result` 是獨立 event 且被丟棄（`ActivityStreamItem` `return null`）。要讓 tool 行展開看「結果」，需在 `foldTimeline` 把每個 tool_use 後面緊跟的 tool_result **配對併進同一個 item**。live stream 下 result 可能還沒到 → 展開先只顯示 input、標 pending。

## Tasks（粗估）

1. `foldTimeline`：把 tool_use 與其後的 tool_result 配對成單一 item（含「result 未到」的 pending 狀態）
2. `ActivityStreamItem`：tool_use / Write / Edit 加摺疊（預設）↔ 展開（完整 input + 配對 result / 完整寫入內容），沿用 `ThoughtItem` 的展開/摺疊 state 模式 + i18n
3. vitest：摘要 vs 展開兩種渲染、配對邏輯、pending result、跨 surface（至少 RunDetail + Chat）
4. 手動驗收：Windows 跑一個 goal + 一次 chat，timeline / 對話裡的 tool 行能展開看到完整 input + result

工程量：輕-中（約 1 個半天；觸發 UX 已不需再 brainstorm）。

## Out of scope

- 後端 / `StreamEvent` / spawn / parser 改動（不需要，資料已完整）
- CLI 端詳細模式（已另起獨立 change，本條只動 app 前端）
- 把 raw claude stream-json（system/init/hook 等被正規化丟掉的事件）也撈進來——那是另一個更大的「raw protocol 視圖」需求，本條只解「已正規化事件的完整呈現」

## 何時動

CLI `--debug` 詳細模式 change（`cli-debug-stream-detail`）已於 2026-05-21 落地並實機驗證「完整渲染」的形狀與價值，前置條件已滿足。觸發 UX 也已於 discuss 定案（per-line 展開）。無硬依賴、設計已備齊，可隨時 `/spectra-propose` 起 change。
