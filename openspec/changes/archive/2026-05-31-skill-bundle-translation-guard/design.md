## Context

`codebus-core/src/skill_bundle/mod.rs` 以 claude 為 source-of-truth、codex body 為衍生：`finalize_for_provider(body, Provider::Codex)` 呼叫 `claude_to_codex_translate(body)`，後者用 6 個逐字 `String::replace` 把 claude body 翻成 codex 變體：

1. `"CLAUDE.md"` → `"AGENTS.md"`（全域 token，出現在每個 body 的 Schema rules 段與多處）
2. F49：FIX step 1 的 `PreToolUse` hook 段（來源 `FIX_WORKFLOW`）
3. F40：QUERY read-only invariant 的 `(--tools Read,Glob,Grep …)` 括號段（來源 `QUERY_WORKFLOW`）
4. F65/F66：CHAT read-only 整段含 `--tools` + `mcp_`（來源 `CHAT_SKILL_CONTENT`）
5. F73：QUIZ Mode B 的 heredoc 自驗整段含 `<<'CBQZ'`（來源 `QUIZ_SKILL_CONTENT`）
6. F72：QUIZ read-only invariant 含 `--tools` + `mcp_` + `PreToolUse`（來源 `QUIZ_SKILL_CONTENT`）

`String::replace` 對找不到的 from 是 no-op、不報錯。改 claude 來源讓某 from 不再逐字 match → 該段不翻譯 → codex body 殘留 claude-only 機制描述，CI 全綠不發現。`skill-bundles` spec 已規範 codex body 的輸出形態，但無機制層 guard 也無測試攔。

froms 彼此獨立（不重疊、不互為前後綴），各自只出現在一個來源 body（CLAUDE.md 例外，全域多處）；故 replace 順序對結果無影響。

## Goals / Non-Goals

**Goals：**

- 任何讓某 replace 的 from 不再 match 的 claude body 編輯，被測試 RED 攔下，訊息指名 from。
- 任何 claude-only 機制 token 漏進任一 verb 的 codex body，被測試 RED 攔下，訊息指名 verb + token。
- 有一個 meta-test 證明上述 guard 真的會抓 drift（不需手改原始碼）。

**Non-Goals：**

- 不換 single-source generator（見 proposal Alternatives）。
- 不改 6 個 replace 的 from/to 內容或 codex 文案。
- 不碰 vault `CLAUDE.md`/`AGENTS.md` 雙寫、不碰 `SystemModel` drift。

## Decisions

### D1：抽 `CODEX_BODY_TRANSLATIONS` const slice（測試性 seam）

把 6 個 inline `(from, to)` 抽成模組級 `const CODEX_BODY_TRANSLATIONS: &[(&str, &str)]`，依現宣告順序排列；`claude_to_codex_translate` 改為 `let mut body = body; for (from, to) in CODEX_BODY_TRANSLATIONS { body = body.replace(from, to); } body`。行為與順序完全保留。

**理由**：impl 與測試共用單一 froms 來源，guard (a) 自動涵蓋未來新增的 replace；避免測試自帶 hardcode 清單而漂移。這是行為保留的可測性 seam、非機制重構。各段對應的 `// F49/F40/...` 註解保留（移到 const 各項上方）。

### D2：guard 邏輯抽成純函式以利 meta-test

- `fn froms_absent_from(froms: &[&str], bodies: &[&str]) -> Vec<String>`：回傳「不出現在任何 body」的 froms。
- `fn claude_only_tokens_in(body: &str, denylist: &[&str]) -> Vec<String>`：回傳 body 含有的 denylist token。

兩者為 `#[cfg(test)]` 測試輔助純函式（不進 production 路徑）。guard 測試與 meta-test 都呼叫它們，meta-test 餵壞輸入驗證偵測為真。

### D3：claude-only token denylist

`["--tools", "PreToolUse", "mcp_", "CLAUDE.md", "<<'CBQZ'"]`。

**理由**：涵蓋 prompt 點名的 `--tools`/`PreToolUse`/heredoc 自驗，外加 `mcp_`（claude-only 工具命名族）與 `CLAUDE.md`（codex 應全為 `AGENTS.md`，守 CLAUDE.md→AGENTS.md 全域 replace 沒 fire 的情形）。denylist **不**含 `codebus quiz validate`——codex quiz no-validate 段合法提及該命令；claude-only 的是 heredoc 界定符 `<<'CBQZ'`，故用它而非命令字串。

## Implementation Contract

**Observable behavior**：`cargo test -p codebus-core` 在以下任一情形 RED——(a) `CODEX_BODY_TRANSLATIONS` 任一 from 不出現在 {goal,query,fix,chat,quiz} 的 claude body 聯集；(b) 任一 verb 的 codex body 含 denylist token。production 行為（materialize 出的 SKILL 內容）與本 change 前**逐字不變**（const 抽取行為保留）。

**Interface**：
- `const CODEX_BODY_TRANSLATIONS: &[(&str, &str)]`（module-private；6 項，順序同現況）。
- `claude_to_codex_translate` 簽章不變、輸出對任意輸入逐字不變。
- 測試輔助純函式 `froms_absent_from` / `claude_only_tokens_in`（`#[cfg(test)]`）。

**Acceptance（測試名為驗證標的）**：
- `every_codex_translation_from_appears_in_a_claude_body`：對每個 `CODEX_BODY_TRANSLATIONS` 的 from 斷言出現在某 claude body；`froms_absent_from(froms, &claude_bodies)` 為空。
- `codex_bodies_contain_no_claude_only_tokens`：對 goal/query/fix/chat/quiz 的 codex body，`claude_only_tokens_in(body, DENYLIST)` 皆為空。
- `drift_guard_detects_unmatched_from`（meta）：`froms_absent_from(&["__NONEXISTENT_FROM__"], &claude_bodies)` 非空；且 `froms_absent_from(CODEX_BODY_TRANSLATIONS_froms, &claude_bodies)` 為空。
- `drift_guard_detects_leaked_claude_token`（meta）：`claude_only_tokens_in("... --tools Read ...", DENYLIST)` 非空；`claude_only_tokens_in("clean codex text", DENYLIST)` 為空。
- 既有 skill_bundle 測試全綠不退步；materialize 輸出位元不變。

**Scope boundaries**：
- In scope：`codebus-core/src/skill_bundle/mod.rs`（const 抽取 + 測試）；`skill-bundles` spec delta（drift guard requirement）。
- Out of scope：translate 文案/語意、single-source generator、vault CLAUDE/AGENTS 雙寫、其他 capability。

## Risks / Trade-offs

- [const 抽取意外改變 replace 順序或內容] → froms 互相獨立、順序無影響；抽取為機械搬移。Mitigation：保留宣告順序；既有 materialize 輸出測試 + 新 (a)/(b) 守住逐字不變。
- [denylist 誤報（codex 合法內容含某 token）] → 已逐一核對：codex bodies 無 `--tools`/`PreToolUse`/`mcp_`/`CLAUDE.md`/`<<'CBQZ'`（皆被對應 replace 蓋掉），denylist 用 heredoc 界定符而非 `codebus quiz validate`。Mitigation：測試在當前程式碼即綠（guard 非 surfacing 既有 bug）；若日後 codex 文案需引入某 token，再窄化 denylist。
- [guard 僅護「from 存在」不護「to 正確」] → 範圍取捨：silent-no-op（from 不 match）是真實風險來源；to 內容正確性由既有 spec scenario 與 review 負責。記錄為已知邊界。
