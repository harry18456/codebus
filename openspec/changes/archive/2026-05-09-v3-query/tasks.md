## 1. query.rs binary 4-step 流程

- [x] 1.1 改寫 `codebus-cli/src/commands/query.rs`：定義 `pub struct QueryArgs { text: String }` 用 clap-derive `#[derive(Args)]`（positional `value_name = "QUERY"`）；提供 `pub async fn run(repo: &Path, args: QueryArgs, debug: bool) -> ExitCode` 入口；新增 const `QUERY_TOOLSET: &[&str] = &["Read", "Glob", "Grep"]`；落實 design.md「Reuse `agent::claude_cli::invoke` + read-only toolset」決策與 Query Subcommand Behavior requirement 的「passes the read-only triple-flag sandbox」scenario
- [x] 1.2 在 `codebus-cli/src/commands/query.rs::run` 內 wire 4 步：(1) `let paths = vault_paths(repo)`；(2) precondition：`!paths.root.exists()` → `eprintln!("error: query: vault not found at {}; run \`codebus init\` first", paths.root.display())` + return `ExitCode::from(2)`；(3) `agent::invoke(InvokeAgentOptions { slash_command: format!("/codebus-query \"{}\"", args.text), vault_root: paths.root.clone(), toolset: QUERY_TOOLSET })`；(4) 拿 child `ExitStatus`、unwrap exit code（unknown → 1）、return `ExitCode::from(child_exit_code)`；**不**呼叫 `auto_commit`、**不**呼叫 init、**不**呼叫 detection；落實 design.md「Binary 4-step 流程（vs goal 5-step）」決策與「Vault-missing strict refuse: exit 2 vs exit 1」決策，以及 Query Subcommand Behavior requirement 的「Query refuses when vault is missing」+「Query does not auto-commit」+「Query propagates agent exit code」+「Query does not modify any vault file」scenario

## 2. main.rs routing

- [x] 2.1 在 `codebus-cli/src/main.rs` 把 `Command::Query` enum variant 改為 `Query(commands::query::QueryArgs)`；routing match arm 改為 `Some(Command::Query(args)) => commands::query::run(&cli.repo, args, cli.debug).await`（不傳 `--no-obsidian-register` — query 不 init）；確認 既有 stub verbs 跟 init / goal routing 不破

## 3. SKILL.md query workflow content

- [x] 3.1 修改 `codebus-core/src/skill_bundle/mod.rs::workflow_section`：新增 `"query" => QUERY_WORKFLOW.to_string()` arm（goal arm 不動，fix 仍走 `_` stub arm）；新增 const `QUERY_WORKFLOW: &str` 含 4-step query workflow（全英、abstract step 4、顯式 read-only invariant 「MUST NOT use Write or Edit」+「toolset is gated at the binary layer」、引用 cwd `CLAUDE.md` 為 schema + language source-of-truth、step 1-4 涵蓋 parse-intent → glob-frontmatter-filter → wikilink-bounded-traversal → emit-stdout）；落實 design.md「Read-only invariant 雙層保護」決策與「SKILL.md content：4-step 跟 v3-goal 的 abstract / English / no-literal-template lessons 對齊」決策，以及 Query Bundle Workflow Content requirement 的「four-step workflow markers」+「declares read-only invariant」+「workflow body is written in English」+「step 4 is abstract」+「defers schema rules to CLAUDE.md」5 scenario

## 4. SKILL.md content unit tests

- [x] 4.1 在 `codebus-core/src/skill_bundle/mod.rs` 加 unit test `query_workflow_body_is_english`：呼叫 `stub_content("query")` 拿 body，assert `body` 內無 char in `'\u{4E00}'..='\u{9FFF}'`；落實 Query Bundle Workflow Content 的 English scenario
- [x] 4.2 在 `codebus-core/src/skill_bundle/mod.rs` 加 unit test `query_step_4_has_no_literal_template`：assert query body 不含 forbidden literal phrase blocklist（`Found 4 pages` / `Here is the answer` / `查到` / `回答如下` 等 sample phrase），同時含 substring `CLAUDE.md` reference 跟 `verbatim` directive；落實 Query Bundle Workflow Content 的 step-4-abstract scenario
- [x] 4.3 在 `codebus-core/src/skill_bundle/mod.rs` 加 unit test `query_workflow_declares_read_only_invariant`：assert query body 含 substring `MUST NOT use Write` 跟 substring `gated at the binary layer`（或 case-insensitive 等價）；落實 Query Bundle Workflow Content 的 read-only-invariant scenario

## 5. Integration tests with mock-claude

- [x] 5.1 在 `codebus-cli/tests/query_flow.rs`（新檔，跟 `goal_flow.rs` 同 layout）新增 helper `run_query(repo, query_text, behavior)`：set `CODEBUS_CLAUDE_BIN` 指向 mock-claude + `CODEBUS_MOCK_BEHAVIOR=<behavior>` + `CODEBUS_MOCK_LOG`，跑 `codebus query <text>`，回 (Output, log_path)；新增 integration test `query_spawns_agent_with_read_only_toolset`：先跑 init 建 vault、跑 query mock=success-noop、assert mock log 含 `--tools Read,Glob,Grep` + `--allowedTools Read,Glob,Grep` + `--permission-mode acceptEdits` + cwd vault root + slash `/codebus-query "..."`、assert log 內 toolset 不含 `Write` 也不含 `Edit`；落實「Query passes the read-only triple-flag sandbox」+「Query spawns agent with cwd at vault root」scenario
- [x] 5.2 在 `codebus-cli/tests/query_flow.rs` 新增 integration test `query_refuses_when_vault_missing`：對 fresh repo（無 .codebus/）跑 query、assert exit code 2 + stderr 含 `vault not found` 跟 `codebus init` 字眼 + assert mock log 不存在（agent 沒被 spawn）；落實「Query refuses when vault is missing」scenario
- [x] 5.3 在 `codebus-cli/tests/query_flow.rs` 新增 integration test `query_does_not_auto_commit`：先 init 建 vault、記錄 nested git rev-list count 為 N、跑 query mock=success-noop、assert post-query rev-list count 仍 = N（無新 commit）+ assert `.codebus` working tree 仍 clean；落實「Query does not auto-commit」+「Query does not modify any vault file」scenario
- [x] 5.4 在 `codebus-cli/tests/query_flow.rs` 新增 integration test `query_propagates_agent_exit_code`：mock=failure-write-then-exit-1（即使 mock 嘗試 Write 它的 process 仍 exit 1，binary 層 `--tools` 應該擋住 Write 但 mock 自身行為由 mock binary 決定 — 這 test 重點是 exit code 傳播）、跑 query、assert exit code 1；落實「Query propagates agent exit code」scenario
- [x] 5.5 修改既有 `codebus-cli/tests/cli_routing.rs` 內 stub verb list（line 361 / 385 / 395 三處 `["query", "lint", "fix"]`），把 `"query"` 移除、留 `["lint", "fix"]`；確認 既有 init / goal stubless 行為驗證不被影響；對齊 spec MODIFIED Goal Bundle Workflow Content 的「codebus-fix bundle retains stub workflow」scenario（query 不再 stub）

## 6. workspace 全綠

- [x] 6.1 跑 `cargo test --workspace` 全綠，含本 change 新增 3 unit test + 4 integration test、與既有 v3-init / v3-pii / v3-vault-history / v3-goal tests 全部通過
