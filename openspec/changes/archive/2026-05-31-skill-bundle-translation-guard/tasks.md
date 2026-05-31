## 1. 測試性 seam（行為保留重構）

- [x] 1.1 在 `codebus-core/src/skill_bundle/mod.rs` 把 `claude_to_codex_translate` 內 6 個 inline `(from, to)` 抽成模組級 `const CODEX_BODY_TRANSLATIONS: &[(&str, &str)]`（保留現宣告順序與各段 `// F49/F40/F65/F66/F73/F72` 註解），函式改為依序迭代 `body = body.replace(from, to)`。驗證：`cargo test -p codebus-core` 既有 skill_bundle 測試全綠（materialize 輸出逐字不變、行為保留）。

## 2. Guard 測試 (a) — 每個 from 必須 match 當前 claude body

- [x] 2.1 在 `#[cfg(test)]` 加純函式輔助 `froms_absent_from(froms: &[&str], bodies: &[&str]) -> Vec<String>`（回傳不出現在任何 body 的 froms）。加測試 `every_codex_translation_from_appears_in_a_claude_body`：以 `stub_content(verb, Provider::Claude)` 渲染 goal/query/fix/chat/quiz 五個 claude body，斷言 `froms_absent_from(CODEX_BODY_TRANSLATIONS 的 froms, &claude_bodies)` 為空；失敗訊息指名找不到的 from literal。驗證：`cargo test -p codebus-core every_codex_translation_from` 綠。

## 3. Guard 測試 (b) — codex body 不得殘留 claude-only token

- [x] 3.1 在 `#[cfg(test)]` 加純函式輔助 `claude_only_tokens_in(body: &str, denylist: &[&str]) -> Vec<String>`（回傳 body 含有的 denylist token）與 `const CLAUDE_ONLY_DENYLIST: &[&str] = &["--tools", "PreToolUse", "mcp_", "CLAUDE.md", "<<'CBQZ'"]`。加測試 `codex_bodies_contain_no_claude_only_tokens`：對 goal/query/fix/chat/quiz 以 `stub_content(verb, Provider::Codex)` 渲染 codex body，斷言每個 `claude_only_tokens_in(body, CLAUDE_ONLY_DENYLIST)` 為空；失敗訊息指名 verb + 漏掉的 token。驗證：`cargo test -p codebus-core codex_bodies_contain_no_claude_only_tokens` 綠（guard 在當前程式碼即綠、非 surfacing 既有 bug）。

## 4. Meta-test (c) — 證明 guard 真的會抓 drift

- [x] 4.1 加 meta-test `drift_guard_detects_unmatched_from`：斷言 `froms_absent_from(&["__NONEXISTENT_FROM__"], &claude_bodies)` 非空、且 `froms_absent_from(真實 froms, &claude_bodies)` 為空。加 meta-test `drift_guard_detects_leaked_claude_token`：斷言 `claude_only_tokens_in("... --tools Read,Glob,Grep ...", CLAUDE_ONLY_DENYLIST)` 非空、且 `claude_only_tokens_in("clean codex sandbox text", CLAUDE_ONLY_DENYLIST)` 為空。驗證：`cargo test -p codebus-core drift_guard_detects` 綠。
- [x] 4.2 一次性手動 RED 確認（TDD red-green 驗證、不留改動）：暫時把某個真實 replace 的 from literal 改錯一字元，跑 `cargo test -p codebus-core every_codex_translation_from` 確認該測試 RED 並在訊息指名該 from，然後**還原**。驗證：還原後全綠；於 commit message / chat 記錄已確認 RED→GREEN（原始碼最終無此暫時改動）。

## 5. 收尾與驗證

- [x] 5.1 確認改動符合 `skill-bundles` spec delta「Codex Body Translation Drift Guard」requirement 三個 scenario（from 全 match / codex 無 claude-only token / meta 偵測 drift）。驗證：`spectra validate skill-bundle-translation-guard` 通過、`spectra analyze` 無 Critical。
- [x] 5.2 全 codebus-core 測試與 lint 不退步。驗證：`cargo test -p codebus-core` 全綠、`cargo clippy -p codebus-core` 無新增 warning、materialize 輸出位元不變（既有 skill_bundle 輸出測試仍綠）。
