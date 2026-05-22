# Backlog: provider-specific prompt engineering（Codex 整合後輸出品質）

**Date:** 2026-05-22
**Surfaced during:** harry 實際以 Codex 跑 codebus，發現輸出不如原本 Claude 理想
**Severity:** 輸出品質 / multi-provider 完成度
**Owner:** harry
**Status:** 待研究（已排進 loop 佇列 PE1→PE2，純探勘，不動實作）

---

## 觀察

整合 Codex 作為第二 provider 後，同樣的 verb 流程下 Codex 的輸出品質不如 Claude。原本整套 prompt / 指示材料是針對 Claude 調出來的，直接套到 Codex 上不見得最佳。

## 懷疑的成因（待 PE1 釐清是哪一類）

1. **Prompt / 指示不對味** — 現有 system prompt、`.codebus/CLAUDE.md` ↔ 鏡射出的 `.codebus/AGENTS.md`、skill bundle 內容都是針對 Claude 寫的，Codex 可能需要不同寫法 / 結構。
2. **Parser / 渲染保真度** — Codex 的 JSONL event 經 `parse_codex_stream_line` 映成既有 `StreamEvent` 時，可能丟失或壓扁了某些內容（reasoning、格式），使「看起來」變差，未必是模型本身輸出差。
3. **模型行為差異** — 即便前兩者修好，Codex 對指示的遵從度 / 風格本來就和 Claude 不同。

## 可能要動的面向（之後實作 change 的範圍，非本次 spike）

- **Prompt 策略**：單一共用 prompt（現況）vs 每 provider 一套 prompt 模板 / override，甚至 per-provider × per-verb。
- **指示材料**：`CLAUDE.md` 與 `AGENTS.md` 是否該分流（目前 AGENTS.md 只是 CLAUDE.md 的鏡射）。
- **Skill 設定**：dual-write 的 skill bundle 是否需要 provider 專屬變體。
- **Response / stream parser**：codex parser 是否需補映更多 event 類型 / 保留 reasoning 與格式。

## Spike 拆成兩段（在 loop 跑，只讀 + 寫 doc）

- **PE1 — 診斷**：定位「輸出不理想」屬上面三類成因的哪些。盤現有指示材料通道（per-verb system prompt 在 `codebus-core/src/verb/*`、agent 層、`CLAUDE.md`/`AGENTS.md` 生成、skill dual-write），並比對 claude vs codex 兩支 stream parser 的保真度差異。產出診斷 doc。
- **PE2 — 設計**：基於 PE1，列 provider-specific prompt 策略選項（共用 vs 每 provider 模板）、各選項對 skill / CLAUDE.md / AGENTS.md / parser 的影響、file-level 任務拆解 + 工程量 + 風險。產出設計 doc。

## 待 harry 補的料

PE1 若需要具體「不理想」的例子（哪個 verb、輸入、Codex 實際輸出 vs 期望），harry 提供樣本會讓診斷更準——目前 spike 只能做結構性分析。

## 何時動

明天（2026-05-23）起。先跑 PE1/PE2 探勘，再決定要不要起實作 change（那需另解除「只讀」邊界）。
