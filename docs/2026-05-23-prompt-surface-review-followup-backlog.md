# Backlog: prompt surface deep review 後續行動（5-phase 計畫）

**Date:** 2026-05-23
**Surfaced during:** prompt surface inventory deep review，見 [`docs/2026-05-23-prompt-surface-inventory.md`](2026-05-23-prompt-surface-inventory.md)
**Parent backlog:** [provider-prompt-engineering](2026-05-22-provider-prompt-engineering-backlog.md)（PE1/PE2 已 archive 但這條延續其精神）
**Severity:** quality / multi-provider 完成度（PE1/PE2 落地）
**Owner:** harry
**Status:** open — 等明天繼續

---

## 一句話狀態

prompt surface 三層（Layer 1 vault root files / Layer 2 SKILL bundles × 6 verb / Layer 3 spawn templates × 10）已 review 完，**共產出 ~95 個 finding（含 F1-F18a, F11a, F19-F25, F27-F38, F39-F48, F49-F62, F63-F71, F72-F85, F26, F86-F96，扣除撤回 F28/F56/F74/F95）**。今天最後一輪 meta-review 已過，**決策待 user trigger**：

- **核心決策**：claude/codex SKILL **拆兩份** vs **機制無關化** — 我的建議是 **SPLIT (PE2 B)**，理由見 inventory doc §15 meta-review + 下方「核心決策待定」
- **執行順序**：5-phase 分階段執行（見下方）

## 為什麼建議拆 SKILL

byte-identical 假設**已經爆掉**——F73 codex quiz Mode B self-validate **literal broken**（heredoc syntax + sandbox 雙重失敗），F19/F67 codex 找不到 CLAUDE.md（實機證實），F66/F72 codex 看到對它無效的機制描述。實際選擇是：

| 方案 | 結果 |
|---|---|
| 現況 byte-identical | claude OK、**codex 多處 broken** |
| PE2 A 機制無關化 | 兩 provider 都拿到 lowest-common-denominator，**F73 仍破** |
| **PE2 B 拆兩份**（建議） | 兩 provider 都拿到準確描述、F73 SKILL 層可解一半 |

成本：10 個 SKILL（不是 5），bounded；收益：agent output quality 直接提升（PE1/PE2 原始目標）。

## 5-phase 執行計畫（建議順序）

| Phase | 內容 | 工程量 | 風險 |
|---|---|---|---|
| **0. Doc consolidation** | 寫 inventory doc §17 跨 verb pattern 彙整 + 重寫 §5/§6/§7（stale 已被深 review 推翻）+ split 決策明示 | 半天（純 doc） | 無 |
| 1. Layer 1 batch | F1-F18a 共 19 個 finding 一起改 `codebus-core/src/schema/neutral.md` | 1 個半天 | 低（純文字） |
| 2. **SKILL split** | `stub_content(verb)` → `stub_content(verb, Provider)`、`.claude/skills/` 與 `.codex/skills/` 各自準確；新 `schema_neutrality` test 涵蓋 provider-specific 部分 | 2-3 半天 | 中 |
| 3. SpawnSpec 重構 | `SpawnSpec.prompt: String` → `verb + sub_mode + input`、claude_backend 組 `/`、codex_backend 組 `$` | 1-2 半天 | 中（核心 type 改動，blast radius 清楚） |
| 4. Verb-specific design fixes | F49a (rule_id→rule) / F63 chat scope guard / F85 lint vs quiz validate JSON 欄名統一 / F93 quiz verify 補 planned_pages / F27 / F29 / F39 / F44 / F75 等 | 2-3 半天 | 低（逐項獨立） |
| 5. **進子 backlog** | F73 後半（codex per-command allowance 架構研究） — codex 沙箱目前無中間態 | 待 spike | 高（可能無解） |

### 為什麼這個順序

- **Phase 0 先**：把決策鎖在 doc，避免日後重議；半天就好
- **Phase 1 先實作**：純文字改、19 個 finding 高 ROI、低風險、暖機
- **Phase 2-3 緊鄰**：split 後 SpawnSpec 自然要重構（backend 知道用哪份 SKILL + `/` 或 `$`），但分兩個 change propose 避免過大
- **Phase 4 最後**：細粒度、可挑、user 隨時可取消
- **Phase 5 進 backlog**：F73 後半牽涉 codex 架構研究，**不阻塞前 4 phase**（Phase 2 後 codex 可 emit「無法 validate，best-effort」）

## 對應的 finding 編號（供明天接手用）

關鍵 finding：

**🔴 CRITICAL（4 個）**：
- F1 — §0 Language Policy 不存在但被 SKILL 引用 5 次（Phase 1）
- F49a — SKILL Step 3 寫 `rule_id`、lint JSON 實際是 `rule`（Phase 4）
- F63 — chat 沒 scope guard、實機證實洩漏 GPT-5 model 身分（Phase 4，可能要先做）
- F73 — codex quiz Mode B self-validate 雙重失敗（Phase 2 解一半、Phase 5 解另一半）
- F86 — 10 個 spawn 模板全用 `/`、codex 應 `$`（Phase 3 cross-cutting）

**🟠 HIGH 不重複列舉**：F2-F5, F40, F49-F51, F65-F66, F72, F75, F85, F88, F93 等（散在 5 phase）

**翻盤 / 撤回**（避免明天重議）：
- F28（goal verify pipe escape） — 撤回，parser 用 `splitn(3, '|')` 已 mitigate
- F56（fix 無 iteration cap） — 撤回，spec `lint-feedback-loop:368` 明文 by-design
- F60（broken-wikilink-related 沒 remove） — 升級 LOW → MEDIUM
- F64（chat prompt injection） — 翻盤降級 HIGH → MEDIUM，codex baseline 已擋
- F74（quiz Mode C pipe escape） — 撤回，同 F28 parser mitigation
- F87（user-side injection） — 降級 HIGH → MEDIUM，被 F63 scope guard 覆蓋
- F89（boundary collision） — 降級 HIGH → MEDIUM，parser line-by-line + LLM baseline
- F95（雙引號 escape） — 翻盤撤回，實機驗 modern LLM 不嚴格 parse

**待補的 pattern（§17）**：10 條跨 verb 共通 pattern，已在 inventory doc 末尾列基準，Phase 0 要寫成完整 §17。

## 相關記憶（明天 session 自動載入）

- [[project_codex_skill_invocation_mechanism]] — codex `$` 觸發機制 + body 要 explicit Read（與 claude 不同的根因）
- [[feedback_claude_code_write_tmp_sandbox]] — Write tool 寫 /tmp 可能寫到 bash 看不到的位置（這次踩過）
- [[feedback_review_discuss_before_record]] — 每節 review 先 chat 等同意才寫 doc
- [[feedback_finish_review_before_action]] — 全 review 跑完才提行動

## Out of scope

- 不在這條範圍：CLI chat polish / app stream verbose / settings chat model 等獨立 backlog 項目
- 不動：goal-subagent-delegation / RAG / MCP Server 等 after-F 項目

## 何時動

明天（2026-05-24）起。**先跑 Phase 0**（純 doc work、半天），完成後 chat 確認再決定要不要直接接 Phase 1。

如果 Phase 0 後對 split 決策有 push back，回頭調整再進 Phase 1+。
