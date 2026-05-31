## Summary

為「codex SKILL body 由 claude body 經逐字 `str.replace` 衍生」這條加 drift 守門測試：改了 claude SKILL body 的某段、對應 replace 的 from literal 不再 match 時會**靜默 no-op**，codex body 殘留過時且對 codex 為假的 claude-only 機制描述（`--tools` / `PreToolUse` / heredoc 自驗），目前無任何測試攔。

## Motivation

`codebus-core/src/skill_bundle/mod.rs` 的 `claude_to_codex_translate` 用 6 個逐字 `String::replace` 把 claude body 翻成 codex 變體（`CLAUDE.md`→`AGENTS.md` 全域 token，外加 5 段整段替換：F49 PreToolUse fix step、F40 query `--tools` read-only、F65/F66 chat read-only 段、F73 quiz Mode B heredoc 自驗、F72 quiz read-only 段）。dispatch 在 `finalize_for_provider`（`Provider::Codex` 走 translate）。

`String::replace` 對找不到的 from 是 **no-op、不報錯**。任何人編輯 claude 來源 body（shared head / `GOAL_WORKFLOW` / `QUERY_WORKFLOW` / `FIX_WORKFLOW` / `CHAT_SKILL_CONTENT` / `QUIZ_SKILL_CONTENT`）讓某個 from literal 不再逐字 match，該段就**不會被翻譯**，codex SKILL body 會留下對 codex 不成立的 claude-only 機制描述，誤導 codex agent，且 CI 全綠不會發現。

`skill-bundles` capability 的「Codex Instruction Materialization」已用 normative 規則描述 codex body **不得**含 claude 機制 token（scenario「Claude SKILL body references Claude-specific mechanisms; codex body does not」等），但**缺少守護「每個 replace 的 from 必須對得上當前 claude body」的機制層 guard**，也沒有對應測試。本 change 補這個缺口。

## Proposed Solution

預設**只加測試 + 最小測試性 seam**，不重構 translate 機制：

1. **測試性 seam**：把 `claude_to_codex_translate` 內 6 個 inline `(from, to)` 抽成一個模組級 const slice（如 `CODEX_BODY_TRANSLATIONS: &[(&str, &str)]`），函式改成依宣告順序迭代 `body = body.replace(from, to)`。行為與順序完全保留（froms 互相獨立、不重疊），只是讓 impl 與測試**共用同一份 from 清單**——避免測試自帶一份 hardcode 清單而與 impl 漂移（新增第 7 個 replace 時測試自動涵蓋）。

2. **Guard 測試 (a)「每個 from 必須真的 fire」**：對 `CODEX_BODY_TRANSLATIONS` 的每個 `from`，斷言它出現在至少一個實際被翻譯的 claude body（`stub_content(verb, Provider::Claude)` for verb ∈ {goal, query, fix, chat, quiz} 的聯集）。任何人改 claude body 讓某 from 不再 match → 該 from 不在任何 claude body → 測試 RED，失敗訊息指名「哪個 from literal 找不到」。

3. **Guard 測試 (b)「codex body 不得殘留 claude-only token」**：對每個 verb 的最終 codex body（`stub_content(verb, Provider::Codex)`），斷言**不含** claude-only token denylist：`--tools`、`PreToolUse`、`mcp_`、`CLAUDE.md`、以及 claude 式 heredoc 自驗界定符 `<<'CBQZ'`。失敗訊息指名「哪個 verb 的 codex body 漏了哪個 token」。

4. **Meta-test (c)「故意改錯會 RED」**：把 (a)/(b) 的偵測邏輯抽成純函式（如 `froms_absent_from(froms, bodies) -> Vec<&str>` 與 `claude_only_tokens_in(body, denylist) -> Vec<&str>`），meta-test 餵入刻意壞掉的輸入（不存在的 from literal / 含 `--tools` 的假 body）斷言偵測函式回非空、餵真實輸入回空 —— 證明 guard 真的會抓到 drift，無需手動改原始碼。

失敗訊息設計成讓人一眼知道「是改了 claude body 忘了同步 codex translate」。

## Non-Goals

- **不換 single-source generator**：不把 translate 改成從單一中性來源生成 claude/codex 兩版。現有 `str.replace` 衍生法對 5 段 divergence 夠用，guard 測試是相稱的輕量解；single-source generator 是更重的重構、本 change 不做（見 Alternatives）。
- **不改 6 個 replace 的語意或 codex 文案**：只加 const seam + 測試，不動 translate 的 from/to 內容。
- **不碰 vault `CLAUDE.md` vs `AGENTS.md` 雙寫**：那是 vault-init NEUTRAL_RULES materialization（codex-only sensitive-read 段）的範疇，與 SKILL bundle body translate 是不同 surface，不在本 change。
- **不處理既有 `SystemModel` spec↔code drift**（另一條 backlog）。

## Alternatives Considered

- **single-source generator（從中性模板生成兩版 body）**：能根本消除 silent-no-op 風險，但要重寫整個 body 組裝、改 5 段 divergence 的表述模型，工作量與回歸面遠大於問題本身。本 change 的風險是「drift 無人攔」，加 guard 測試即可關閉，不需動架構。若日後 divergence 段數成長到難維護再評估。
- **測試自帶一份 hardcode 的 from 清單**（不抽 const）：可行但測試清單會與 impl 的 inline froms 漂移（新增 replace 時測試不會自動涵蓋），守門不完整。抽 const 讓兩者共用單一事實來源。

## Impact

- Affected specs:
  - `skill-bundles`（修改，有 delta）— 新增一條 requirement「Codex Body Translation Drift Guard」+ scenarios，把「每個翻譯 source-literal 必須對得上當前 claude body」與「codex body 不得含 claude-only 機制 token」明訂為衍生機制的 normative 不變式。
- Affected code:
  - Modified:
    - codebus-core/src/skill_bundle/mod.rs （抽 `CODEX_BODY_TRANSLATIONS` const + `claude_to_codex_translate` 改迭代；新增 guard 測試與 meta-test 於 `#[cfg(test)]`）
  - New: (none)
  - Removed: (none)
