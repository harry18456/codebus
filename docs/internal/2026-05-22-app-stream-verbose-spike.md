# T2 Spike：App Activity Stream 顯示完整 AI 回覆細節

**Date:** 2026-05-22
**Task:** loop T2（只讀探勘）
**背景:** [app-stream-verbose-detail backlog](2026-05-21-app-stream-verbose-detail-backlog.md)（2026-05-21，設計已收斂）

---

## TL;DR

2026-05-21 backlog **設計已收斂、可直接實作**（per-line 展開、frontend-only），本 spike 對現碼逐條核對**全數屬實**，無新增缺口。**唯一值得補的新發現**：T2 與 PE2-C2（codex parser）有**順序耦合**——T2 的「展開看 tool result」對 codex 的檔案編輯**目前會是空的**，需 PE2-C2 先把 codex 編輯映成 event 才有東西可展開。

---

## 對現碼核對（backlog 宣稱 ✅ 屬實）

1. **截斷只在前端**：`ActivityStreamItem.tsx`——`tool_result` 直接 `return null`（`:59`，非 Write/Edit 的 tool_use 之後）；`summarizeToolInput` 把 command 截 `slice(0,79)…`（`:188`）、有 file_path 只回 basename（`:182-185`）；Write/Edit 只顯示 `writeEditPath` 正規化路徑（`:34,171-176`）。
2. **後端資料本來就完整**：`ipc.ts:557-560` 的 stream data —— `tool_use{name, input:unknown}`、`tool_result{output:string, is_error}`、`thought{text}`。input 是全量 `unknown`、output 是全量字串。→ 確認 backlog「截斷只發生在 TSX、後端零改」。
3. **pairing 是 net-new**：`foldTimeline`（`:149-168`）目前**只**折 `thought`（`:159`），其餘 event 一律包成 `{kind:"event"}`，所以 tool_use 與其後的 tool_result **沒有配對**，且 tool_result 被 `ActivityStreamItem` 丟掉。要「展開看結果」必須在 `foldTimeline` 新增 tool_use↔後續 tool_result 的配對（含 live stream 下 result 未到的 pending 態）——與 backlog task 1 一致。
4. **6 個 surface 共用**：`ActivityStreamItem`/`foldTimeline` 被 `RunDetailRunning` / `RunDetailDone` / `ChatTranscript` / `QuizGenerationLog` / `QuizTab` 引用（grep 確認）。一處改、全 surface 生效——含 chat（使用者明確要的）。
5. **展開模式有現成範本**：`ThoughtItem`（`:84-129`）已實作 `▼ 展開 / ▲ collapse` + `useState(open)`，tool 行沿用同款即可。

## 受影響檔案 / 工程量（沿用 backlog，已核實）
- `components/workspace/ActivityStreamItem.tsx`（含 `foldTimeline`）+ 同名 test。
- 工程量：**輕-中（約 1 個半天）**，純前端、後端零改、設計已定案（無需再 brainstorm）。

## ⚠️ 新發現：與 PE2-C2 的順序耦合

T2 的核心價值是「點開 tool 行看完整 input + 配對 result」。但 [PE2 設計](2026-05-22-provider-prompt-design.md) C2 指出：**codex 的檔案編輯（apply_patch）目前不產生任何 tool_use/tool_result event**（parser 未映）。所以：

- 對 **Claude**：T2 立即有完整價值（Read/Write/Edit/Bash 都有 event 可展開）。
- 對 **Codex**：goal/fix 的實際寫檔在 timeline 裡**根本沒有行**，T2 做了也沒東西展開；且 codex 的 command 行工具名是 `"Shell"`（PE1）。

**建議順序**：若近期要兼顧 codex 體驗，**PE2-C2（擴 codex parser）應先於或併同 T2**——否則 codex 使用者裝了「展開」卻發現 goal/fix 的關鍵動作仍不可見，體感落差更明顯。若只先服務 Claude，T2 可獨立先行。

## Out of scope（沿用 backlog）
- 後端 / StreamEvent / parser（C2 另計）、CLI 詳細模式（已落地）、raw protocol 視圖。

## 待 harry
此項設計已備齊可隨時起 change。決策點只剩一個：**先做 T2（先爽 Claude）還是先做 PE2-C2 再 T2（讓 codex 也有料可展開）**？取決於 PE1 未決問題 1 的答案（若 codex 體驗是痛點，PE2-C2 優先）。
