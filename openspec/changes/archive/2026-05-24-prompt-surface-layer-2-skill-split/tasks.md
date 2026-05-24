<!--
Each task description states:
- the behavior or contract being delivered, and
- the verification target that proves completion.

Tasks 3-7 all touch codebus-core/src/skill_bundle/mod.rs sequentially
(same file → no [P] markers, even though parallel_tasks: true).
-->

## 1. Foundation: Provider enum + signature change（spec MODIFIED Requirement "Codex Instruction Materialization" — provider-aware body divergence rules 落地）

- [x] 1.1 在 `codebus-core/src/skill_bundle/mod.rs` 新增 `pub(crate) enum Provider { Claude, Codex }`，把 `fn stub_content(verb: &str) -> String` 改 `fn stub_content(verb: &str, provider: Provider) -> String`，`fn workflow_section(verb)` 同步改 `fn workflow_section(verb, provider)`。call site：`write_bundle_if_missing` 加 `provider` 參數（claude path 帶 `Provider::Claude`、codex path 帶 `Provider::Codex`）。本任務階段 body 內容暫不分流（兩 provider 仍走同一 match arm 產出原 body），讓既有 11 個 `stub_content_*` test 在 parameterize 前 still pass。**驗證**：(1) `cargo check -p codebus-core` 綠（簽名擴展編譯通過）；(2) `cargo test -p codebus-core skill_bundle` 既有 11 個 test 改成各自顯式傳 `Provider::Claude` 後 still pass。

## 2. Test parameterization 既有 11 → 22 test cases

- [x] 2.1 把 5 個 `stub_content_*` 命名的 test（chat 3 個 + quiz 2 個，於 `stub_content_chat_*` / `stub_content_quiz_*` 區段）改成每個跑兩 provider：抽 helper fn 持 assertion body 接 `provider` 參數、兩個 `#[test] fn` wrapper 分別 `_claude` / `_codex` suffix。本階段 body 內容仍兩 provider identical，所以兩 provider 各 5 test 應同時 pass。其他 ~13 個非 `stub_content_*` 命名的 verb-internal test（如 `goal_workflow_body_is_english` / `fix_workflow_*`）維持 Provider::Claude 單版本不動 — 那些斷言內容多為 provider-agnostic（CJK 字檢查、anti-template 等），無需雙 provider；divergence assertion 由 task 3-7 補。**驗證**：cargo test 顯示 10 個 `stub_content_*_<provider>` test 全綠（5 claude + 5 codex）；無 test name collision；既有非 stub_content_* test 仍綠。

## 3. Shared head provider split — F19/F67/F21/F68/F80

- [x] 3.1 在 `stub_content` template（shared head，給 goal/query/fix 用）內：(a) 把 Schema rules 行內的 `CLAUDE.md` 改 `match provider { Claude => "CLAUDE.md", Codex => "AGENTS.md" }`（F19/F67）；(b) Trigger 行改 semantic 句「Activate when the user requests <verb action>」、兩 provider 共用，移除 `/codebus-<verb>` 字面（F21/F68/F80）。**驗證**：新加 test `stub_content_<verb>_trigger_is_semantic_<provider>` 斷言 claude body 不含 `/codebus-<verb>` 字面、codex body 不含 `$codebus-<verb>` 字面；新加 test `stub_content_<verb>_schema_doc_filename_<provider>` 斷言 claude body 含 `CLAUDE.md`、codex body 含 `AGENTS.md` 且不含 `CLAUDE.md`。實作 spec scenario「Trigger language is semantic and provider-agnostic on both paths」+ 部分 scenario「Claude SKILL body references Claude-specific mechanisms; codex body does not」（schema doc 部分）。

## 4. FIX_WORKFLOW provider split — F40/F49

- [x] 4.1 `FIX_WORKFLOW` const 內：(a) Step 1 PreToolUse hook 描述（F49）→ claude 保留 `PreToolUse` 字面 + `codebus lint` 放行說明；codex 改寫成 sandbox `-s read-only` posture 描述、無 PreToolUse 字面。(b) Read-Only Invariant 段（F40）→ claude 寫 `--tools Read,Glob,Grep` 機制；codex 寫 codex sandbox + AGENTS.md scope 段保護機制、無 `--tools` 字面。`workflow_section("fix", provider)` 內 match。**驗證**：新 test `stub_content_fix_claude_contains_pretooluse` 斷言 claude fix body 含 `PreToolUse`；`stub_content_fix_codex_no_pretooluse` 斷言 codex fix body 不含 `PreToolUse` 字面；`stub_content_fix_claude_contains_tools_flag` 斷言 claude 含 `--tools`；`stub_content_fix_codex_no_tools_flag` 斷言 codex 不含。實作 spec scenario「Claude SKILL body references Claude-specific mechanisms; codex body does not」對 fix verb 部分；對應 Example「fix verb body divergence」。

## 5. CHAT_SKILL_CONTENT provider split — F65/F66/F67

- [x] 5.1 `CHAT_SKILL_CONTENT` const 改成 `fn chat_skill_content(provider) -> String`：(a) Read-Only Invariant 段（F65）— 同 task 4.1 拆 claude/codex 機制描述；(b) `mcp_*` family 段（F66）— claude 保留排除說明、codex 整段移除（codex 無 mcp tool naming）；(c) Schema rules 段（F67）— 同 task 3.1 拆 filename。**驗證**：`stub_content_chat_claude_contains_mcp` 斷言 claude chat body 含 `mcp_` 字面；`stub_content_chat_codex_no_mcp` 斷言 codex chat body 不含 `mcp_` 字面；既有 `stub_content_chat_contains_promote_marker_format_<provider>` 兩 provider 仍 pass（promote marker 是 provider-agnostic）。

## 6. QUIZ_SKILL_CONTENT provider split — F72/F79/F73

- [x] 6.1 `QUIZ_SKILL_CONTENT` const 改成 `fn quiz_skill_content(provider) -> String`：(a) Read-Only Invariant（F72）+ Schema rules（F79）拆同 task 4.1/3.1 模式。**驗證**：`stub_content_quiz_claude_contains_tools_flag` / `stub_content_quiz_codex_no_tools_flag` 等斷言成立；既有 `stub_content_quiz_does_not_instruct_agent_frontmatter` 兩 provider 仍 pass。
- [x] 6.2 Mode B self-validate 段（F73 / Pattern 9）：claude 維持 bash heredoc 呼叫 `codebus quiz validate`；codex 改寫成「emit one line `[CODEBUS_QUIZ_NO_VALIDATE] <reason>` AND skip Mode B」instruction、無 heredoc。**驗證**：`stub_content_quiz_claude_mode_b_has_heredoc` 斷言 claude quiz body 含 `<<EOF` 或 `<<'EOF'` heredoc marker + `codebus quiz validate` 字面；`stub_content_quiz_codex_mode_b_no_validate_marker` 斷言 codex quiz body 含 `[CODEBUS_QUIZ_NO_VALIDATE]` literal 且不含 heredoc marker。實作 spec scenario「Codex quiz Mode B emits no-validate marker instead of running validate」。

## 7. Taxonomy enum dedup（Pattern 1 Layer 2 — F32 + F45）

- [x] 7.1 `GOAL_WORKFLOW` Step 2 + `QUERY_WORKFLOW` Step 1 內的 taxonomy enum 列舉（`concept / entity / module / process / synthesis` 5 個 type bucket）移除，改成「see §2 in cwd `CLAUDE.md`」（claude path）/「see §2 in cwd `AGENTS.md`」（codex path）reference。filename 部分透過 task 3.1 已建立的 provider match。**驗證**：新 test `stub_content_goal_no_taxonomy_enum_<provider>` / `stub_content_query_no_taxonomy_enum_<provider>` 斷言 goal/query body 不含完整 `concepts / entities / modules / processes / synthesis` 序列；新 test `stub_content_goal_references_schema_doc_<provider>` 斷言 body 含 `§2` 或 `Wiki Structure` 字面 reference。實作 spec scenario「Taxonomy enumeration not duplicated in either provider's SKILL body」。

## 8. Regression + real materialization

- [x] 8.1 跑 `cargo test --workspace` 全套 regression 確認 spec MODIFIED Requirement "Codex Instruction Materialization" 對應的所有 scenario 透過 unit / integration test 落地：既有測試 + 新增的 provider-specific 斷言全綠（包含 `schema_neutrality` 4 個 + `vault_init` AGENTS.md + 所有新 `stub_content_*_<provider>` test）。**驗證**：`cargo test --workspace` exit 0；輸出含 task 3-7 新增的 test name 全部 `... ok`。
- [x] 8.2 設 `CODEBUS_HOME=<tmp>` + 寫 codex active config 觸發 codex materialization；對乾淨 vault 跑 `codebus init --repo <vault>`，inspect (a) `<vault>/.codebus/.claude/skills/codebus-fix/SKILL.md` 與 `<vault>/.codebus/.codex/skills/codebus-fix/SKILL.md` 兩份 body 用 diff 確認**不再 byte-identical**、且 claude 含 `PreToolUse` / `--tools` 字眼 codex 不含、codex 含 `AGENTS.md` claude 含 `CLAUDE.md`；(b) `<vault>/.codebus/.codex/skills/codebus-quiz/SKILL.md` 含 `[CODEBUS_QUIZ_NO_VALIDATE]` literal、不含 heredoc；(c) 兩份 body 都不含 `/codebus-fix` / `$codebus-fix` 字面、Trigger 句為 semantic。**驗證**：手動 diff 兩 SKILL.md + grep 確認上述條件；對應 spec scenario「Claude SKILL body references Claude-specific mechanisms; codex body does not」+ Example「fix verb body divergence」+「Codex quiz Mode B emits no-validate marker」三者實機驗證。
