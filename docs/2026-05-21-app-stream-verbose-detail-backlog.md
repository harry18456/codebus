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

## Proposed fix

在 Activity Stream / Run Detail timeline 上提供「展開看完整」的能力：完整 tool input（含 Write/Edit 寫入內容、複雜物件參數）、完整 tool result（不截斷）。thinking 視情況確認是否也需要完整展開（CLI 端 thinking 已完整）。

**需先 brainstorm 的觸發 UX（本條動工前先決定）：**

- **A**：每個 tool item 可點擊展開/摺疊（預設摺疊摘要，點開看完整）——區域性、不影響整體版面
- **B**：Run Detail 上一個全域「詳細模式」切換，一鍵全展開
- **C**：Settings 全域開關（對齊 CLI `--debug` 的心智模型）

A 最貼近「需要時才看」、不洗版；B/C 較粗粒度。動工前先 brainstorm 定案，別直接抄 CLI 的「一個旗標全開」。

## Tasks（粗估）

1. 決定觸發 UX（A/B/C，先 brainstorm）
2. `ActivityStreamItem` 加完整渲染分支（解開 `summarizeToolInput` / `command.slice` / `writeEditPath` 的摘要，提供完整版）
3. 對應的展開/摺疊或切換 state + i18n
4. vitest：摘要 vs 完整兩種渲染、切換行為
5. 手動驗收：Windows 上 run 一個 goal，timeline 能展開看到完整 tool input/result

工程量：輕-中（觸發 UX 定案後約 1 個半天）。

## Out of scope

- 後端 / `StreamEvent` / spawn / parser 改動（不需要，資料已完整）
- CLI 端詳細模式（已另起獨立 change，本條只動 app 前端）
- 把 raw claude stream-json（system/init/hook 等被正規化丟掉的事件）也撈進來——那是另一個更大的「raw protocol 視圖」需求，本條只解「已正規化事件的完整呈現」

## 何時動

CLI `--debug` 詳細模式 change 落地後再動，讓 CLI 先驗證「完整渲染」的形狀與價值，app 再對齊（含自己的觸發 UX）。無硬依賴，可隨時插入。
