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
| 5. **進子 backlog → 2026-05-25 spike closed (defer)** | F73 後半（codex per-command allowance 架構研究） — codex 沙箱目前無中間態；spike 結論：技術可行但 trade-off 不划算（見尾段「Phase 5 — 2026-05-25 spike 收尾」） | spike done | 高（已驗）|

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

## Phase 5 — 2026-05-25 spike 收尾

**狀態**：spike 完成、結論「技術可行、不採用」、Phase 5 正式 close。

### 為什麼跑 spike

2026-05-25 `codex-skill-trigger-fix` change（commit `a4a931b`）修通 codex provider 5/5 verb 後，Phase 5 spike 從「codex broken 走不到」變成「可以驗了」。F73 後半的核心問題 — 「codex 沙箱有沒有等價 claude PreToolUse hook 那種 per-command allowance 機制，讓 generate-stage agent 可以在 in-session 跑 `codebus quiz validate` self-correct 但**只**那一條 command」— 必須實機驗才能定論。

### Spike 實驗 + 觀察

跑 `codex exec -s workspace-write` 在 `/tmp/exp-vault` 三組指令：

1. **裸 spawn `codebus --version`**：✓ exit 0、stdout `codebus 3.0.0`。codex agent 在 `-s workspace-write` 下能直叫 `codebus` binary。
2. **bash heredoc 跑 `codebus quiz validate - <<EOF...EOF`**（複製 claude path 模式）：✗ codex sandbox **自動擋掉** `bash -lc <heredoc>` 模式，回 `rejected: blocked by policy`、status `declined`、exit -1。codex 有某種**內建** command policy（不是 user-configurable），對 bash subshell 形式不放行。
3. **Tempfile + 純 shell call**：✓ agent 用 `apply_patch` 寫 `.codebus/tmp-quiz-draft.md`、跑 `codebus quiz validate <file>`、刪檔。乾淨 quiz 回 `0 issues — quiz is structurally valid`、exit 0；故意注入 `[[fake-nonexistent-page]]` 的 broken quiz 回 `1 issue(s)... [quiz-broken-wikilink] Q1: explanation cites [[fake-nonexistent-page]] but no page named fake-nonexistent-page.md exists in any wiki/<type>/ folder`、exit 1。validator 真實抓到 broken citation、code path 完全 work。

實驗 logs：`/tmp/codex-phase5-test1.log`、`/tmp/codex-phase5-test2b.log`、`/tmp/codex-phase5-test3.log`、`/tmp/codex-phase5-test4.log`。

### 結論：技術可行、不採用

**結論一**：F73 後半的原假設「codex 沙箱無中間態」**部分錯**。codex 內部其實有 command policy（擋 bash heredoc），但是**不可配置** — 沒有等價 claude PreToolUse 的 user-defined per-command allowance。claude path 的「`Permission::ReadOnly` + `--tools Bash` + PreToolUse hook 鎖死 `codebus quiz validate *`」三層防禦無法在 codex 完整對映。

**結論二**：Mode B in-session self-validate 在 codex 上仍**可行**，但實作形狀跟 claude 不同：codex 要 `Permission::Workspace`（不是 ReadOnly）+ SKILL body 引導 agent 走「tempfile + 純 shell call」（不是 heredoc）。要付的代價：

| 安全層 | claude path | codex path（若實作 Phase 5）|
|---|---|---|
| 工具白名單 | `--tools Read,Glob,Grep,Bash` 限工具 | 沙箱 `-s workspace-write` 全放、無工具白名單 |
| 命令層 gate | PreToolUse hook 限 Bash 命令首字 = `codebus quiz validate` | 無；agent 可跑任何 shell 命令 |
| 防止 Write/Edit | tool gate 沒 `Write/Edit` 工具 | 沙箱允許；只能 SKILL body 紀律 |
| **net 防禦層數** | 3 層（tool gate + hook + SKILL body）| 1 層（SKILL body）|

**結論三**：caller-side（codebus CLI 端）的 post-agent `codebus quiz validate` 仍會跑（per Phase 2 的 SKILL body 設計：`The caller (codebus CLI) will run codebus quiz validate after this agent terminates and use that result as the authoritative success signal`）。所以 in-session self-validate 是 **quality optimization**（agent 自己有機會修），**不是 correctness fix** — 不實作的話 codex quiz 仍然會被 caller 驗、broken citation 仍會被抓、user 仍會看到 `content_review: flagged` 標籤。

**決策**：不實作 Phase 5。理由：

1. caller-side validate 已 cover correctness；in-session 只是 quality 微優
2. 為這個微優把 codex 沙箱從 `read-only` 放寬到 `workspace-write` 不划算 — 防禦從 3 層降到 1 層
3. `[CODEBUS_QUIZ_NO_VALIDATE]` 標籤是**設計上正確的**誠實 surfacing（codex 沙箱無等價機制這件事本身值得讓 user / caller 知道，不是 bug）
4. Per `codex-skill-trigger-fix` design Non-Goal「不嘗試讓 codex provider 在所有 verb 上 100% 等價 claude」— grounded behavior 差異本來就允許

### 後續

- F73 整個 finding（前半 Phase 2 + 後半 Phase 5）正式 close
- 5-phase 計畫全部完成
- 沒有 follow-up task；如果未來 codex CLI 引入 user-defined per-command allowance（unlikely），可重評
