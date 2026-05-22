# Loop Worklog

Append-only。每輪一筆，最新在最上面。格式：

```
## YYYY-MM-DD HH:MM — T# <任務名>
- 狀態: DONE | BLOCKED
- 做了: <一兩句>
- 產出: docs/...
- 下一步 / BLOCKED 原因: ...
```

---

## 2026-05-22 HH:MM — PE1 診斷 Codex 輸出成因
- 狀態: DONE
- 做了: 讀 agent/stream/skill_bundle 層，比對 claude vs codex 指示材料 + parser 保真度。發現：(1) skill bundle 與 AGENTS.md 對 codex 是 byte-identical 沿用 Claude 內容，寫死了 `--tools`/PreToolUse hook/`mcp_*` 等 codex 沒有的機制（quiz 自我驗證契約最受影響）；(2) codex parser 只映 3 種 event，檔案編輯(apply_patch)不可見、turn.failed 靜默吞掉、工具全塌成 "Shell"、無增量串流。修正了 backlog 初步猜測：「答案被當 thought」兩 provider 一致，非 codex 獨有。
- 產出: docs/2026-05-22-provider-prompt-diagnosis.md
- 下一步: PE2 設計（per-provider 指示差異化縫 + codex parser event 覆蓋擴充）。等 harry 補具體樣本以判「模型行為差異」類別。

## 2026-05-22 — 加入 PE1/PE2（Codex prompt engineering）
- 狀態: DONE
- 做了: 依 harry 需求把「Codex 整合後輸出不理想」的 prompt engineering 研究排進佇列最前面（PE1 診斷 → PE2 設計），並建 backlog 文件。
- 產出: docs/2026-05-22-provider-prompt-engineering-backlog.md, 更新 PLAN.md + BACKLOG.md
- 下一步: 首輪從 PE1（診斷成因）開始。

## 2026-05-22 — 初始化
- 狀態: DONE
- 做了: 建 loop PLAN + WORKLOG，定下「只讀 + 寫 doc」自主邊界。
- 產出: docs/loop/PLAN.md, docs/loop/WORKLOG.md
- 下一步: 首輪從 T1（settings-chat-model spike）開始。
