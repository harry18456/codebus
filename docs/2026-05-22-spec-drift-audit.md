# T9 Spec Drift 稽核

**Date:** 2026-05-22
**Task:** loop T9（只讀比對，不改 spec）
**範圍:** 18 個 spec ~10k LOC,聚焦三個與 T6/T7 發現相關性最高的 spec（pii-filter, lint-feedback-loop, skill-bundles）。其餘列為後續候選。

---

## 找到 3 個 drift + 2 個值得標註的成熟度缺口

### 🔴 D1：lint-feedback-loop 開頭摘要說「hook 只允許 `codebus lint *`」,但 code 還允許 `codebus quiz validate *`

**Spec 位置:** `openspec/specs/lint-feedback-loop/spec.md:5`（Purpose 段）
**Code 事實:** `codebus-cli/src/commands/hook.rs:128 is_allowed_bash_command` 同時放行 `codebus lint *` 與 `codebus quiz validate *`（quiz Mode B 自我驗證契約需要）。

> spec:5: 「...hard-gates the agent's Bash tool to `codebus lint *` only」

但同 spec 後面其實有「Fix Bash Hook Installation, new allow form」段描述 quiz validate 放行（T7 註解處 `hook.rs:286-303` 引用了 spec 條款）。**summary 句沒同步更新** → 讀者只看開頭會誤以為 quiz validate 不在允許清單。
**修法:** spec.md:5 summary 改成「to `codebus lint *` or `codebus quiz validate *` only」。文檔級,半小時。

### 🟡 D2：skill-bundles 開頭摘要說「three skill bundles (`codebus-goal`, `codebus-query`, `codebus-fix`)」,但實際是五個

**Spec 位置:** `openspec/specs/skill-bundles/spec.md:5`（Purpose 段）
**Code 事實:** `codebus-core/src/skill_bundle/mod.rs:28 VERBS = ["goal", "query", "fix", "chat", "quiz"]`,且**同一份 spec** `:11` 就正確寫了「**five** skill bundles ... `codebus-{goal,query,fix,chat,quiz}`」。

→ 開頭摘要 sentence 沒跟上 chat / quiz 加入後的更新。和 D1 同類:文檔級不一致。
**修法:** spec.md:5 句子改 "three" → "five",列上五個 verb。半小時。

### 🟠 D3（連 F1）：pii-filter spec 未要求「matches non-overlapping」,但 mask 實作依賴此前提

**Spec 位置:** `openspec/specs/pii-filter/spec.md:11,23,26`
**Code 事實:** spec 只規定「sorted in ascending byte-offset order」（`:11`,`:26`）,**完全沒有** non-overlapping / 區間合併 / disjoint 之類的要求。但 `vault/raw_sync.rs:343 mask_matches` 文件假設並依賴「matches are non-overlapping」（[T6/F1](2026-05-22-core-quality-review.md)）。
→ **spec 沒承諾 disjoint,實作卻假設 disjoint**。這正是 F1 的源頭:scanner 契約跟 mask 契約不接縫。
**修法選擇:**
- A. 緊縮 **spec**:加 requirement「matches SHALL be returned as a disjoint set (overlapping/contained matches MUST be merged)」+ scanner 測試。
- B. 緊縮 **實作**:mask_matches 之前自己做 interval-merge,不再依賴 scanner 保證。
推薦 B(F1 本就建議這做法),spec 順手加一條「caller MAY merge」即可,不強加 scanner 義務。

### 🟡 D4（spec 成熟度,連 PE1）：skill-bundles Codex Instruction Materialization 描述「verbatim 重用」是事實,但 spec 不標註其代價

**Spec 位置:** `openspec/specs/skill-bundles/spec.md:451-465`
**Code 事實:** spec `:453` 老實寫「**the bundle content is reused verbatim**」(`stub_content` byte-identical 雙寫)——準確反映實作。但 spec **沒指出** SKILL.md 內容對 codex 含**事實錯誤的機制描述**（`--tools Read,Glob,Grep`、PreToolUse hook、`mcp_*`,皆 codex 沒有的東西,見 [PE1 診斷]）。
→ spec 忠實描述了「壞」實作,但沒設置 invariant 防止此類失準（例:「指示材料 SHALL NOT 描述具體 provider 機制名稱」）。**非傳統 drift**,是 spec 成熟度缺口。
**修法:** 等 PE2 設計（C1 機制無關化）落地時順手在 spec 加一條「skill bundle 內容 SHALL be mechanism-agnostic（不點名 `--tools`/hook/`mcp_*`）」。

### 🟡 D5（次要）：lint-feedback-loop spec 未要求 hook 拒 shell 元字元（連 F4）

**Spec 位置:** `openspec/specs/lint-feedback-loop/spec.md`（Fix Bash Hook Installation 段）
**Code 事實:** hook 只查 argv[0]/argv[1]([T7/F4](2026-05-22-cli-quality-review.md)),spec 也沒要求「整條 command 必須是單一 codebus 調用 / 拒絕含 shell 元字元」。
→ spec 跟實作對齊在「不夠安全」的狀態。實作該補,spec 也該加 requirement「allow 條件 SHALL additionally reject commands containing shell metacharacters (`;`, `&`, `|`, `$`, backtick, `>`, `<`, newline)」。

## 後續 spec 候選
未掃: agent-backend, codex-backend, claude-code-config, app-workspace（最大,2664 LOC）、cli、vault 等 14 個 spec。若哪天要全面 drift 稽核,優先這幾條(連動 T1/T8/PE2)。

## 待 harry
D1/D2 是純文檔修(各半小時),低風險可順手清。D3-D5 連動既有 BACKLOG（F1/F4/PE2）,**修 code 時順帶加 spec requirement** 比現在獨立改 spec 更划算（避免 spec 跟 code 兩次 round-trip）。
