## Why

`codebus query "..."` 在 v3 主序列 #6 — read-only 對應於 #5 v3-goal 的 ingest verb。Agent 接 query → 在 vault cwd 內 glob `wiki/`、read 相關 page → 印 answer 到 stdout，**不寫任何檔**。binary 端共用 #5 已 ship 的 `agent::claude_cli::invoke`，差別在 toolset 限縮為 `Read,Glob,Grep`（無 Write/Edit）+ 無 detection re-sync + 無 auto-init fallback + 無 auto_commit。Read-only 在兩層保護：binary `--tools` hard gate（spike-verified 2026-05-09）+ SKILL.md 內顯式 invariant 重申。

## What Changes

- 改寫 `codebus-cli/src/commands/query.rs`：clap-derive `QueryArgs { text: String }` + `pub async fn run(repo, args, debug) -> ExitCode`；4 步流程（resolve repo → vault precondition → spawn agent → propagate exit code）。**vault 不存在 → exit 2 + stderr 提示「先跑 codebus init」**，不 fall-back auto-init（query 是 wiki user 不該觸發 ingest 副作用）。
- 修改 `codebus-cli/src/main.rs`：`Command::Query` 改為 `Query(QueryArgs)`，傳 `&cli.repo` + args + `cli.debug`（不傳 `--no-obsidian-register` 因為 query 不 init）。
- 修改 `codebus-core/src/skill_bundle/mod.rs::workflow_section`：query 分支補 4-step query workflow 內容（全英 / abstract step 4 / 顯式宣告 read-only invariant 「MUST NOT use Write or Edit」），跟 §0 Language Policy + v3-goal ingest fix 的 lessons 一致；fix verb 仍 stub 直到 #8。
- Spec 加 capability `cli` 的 `Query Subcommand Behavior` requirement（spawn / cwd / read-only triple-flag toolset / vault-missing strict refuse / no auto-commit / propagate child exit code）。
- Spec 加 capability `skill-bundles` 的 `Query Bundle Workflow Content` requirement（4-step workflow markers / read-only invariant 顯式宣告 / English body / abstract step 4 / 引用 cwd `CLAUDE.md` 為 schema + language source-of-truth）。
- 修改既有 `Goal Bundle Workflow Content` requirement 內的「codebus-query and codebus-fix bundles retain stub workflow」scenario，改為「codebus-fix bundle retains stub workflow」（query 不再 stub）。
- Integration tests with mock-claude binary（reuse #5 ship 的 `tests/bins/mock_claude.rs` 跟 `CODEBUS_CLAUDE_BIN` env override hook）：spawn args 含 read-only toolset / vault-missing strict refuse / no auto_commit emitted / propagate exit code。

## Non-Goals

- `agent::claude_cli::invoke` 的 trait 抽象：v3-query 是第二個 verb 共用 invoke，但跟 v3-goal 共用就算「兩個 caller」尚不足以證明 trait surface 形狀；等 #8 v3-fix 也 ship 後 3 個 caller 才回頭看是否 refactor。本 change 不動 `claude_cli.rs`。
- query 自動 detect source drift 提示「raw mirror outdated」：query 是純 read-only verb，不該觸發 raw_sync 副作用；user 想要 fresh source 自己跑 init / goal。
- query 命中 `wiki/` 為空時 emit 特殊 message：「empty wiki」case 由 agent 自己處理（agent 看 wiki/ 為空就 emit 「no relevant pages」之類），spec 不規定 binary 端要 detect。
- query 結果 caching / 持久化：query 結果不 commit 進 nested git、不寫 vault — 純印 stdout 一次性，user 想保留自己 redirect。
- query 接 `--no-obsidian-register` flag：obsidian register 是 init 階段的事，query 不 init 自然不會踩 obsidian config。
- token usage tracking / source enrich / multi-LLM provider：跟 #5 一樣排除。

## Capabilities

### New Capabilities

(none)

### Modified Capabilities

- `cli`: 新增 Query Subcommand Behavior requirement（4-step flow / read-only toolset / strict refuse missing vault / no auto_commit / propagate exit code）
- `skill-bundles`: 新增 Query Bundle Workflow Content requirement（4-step workflow markers / read-only invariant 顯式宣告 / English / abstract step 4）；同時修改既有 Goal Bundle Workflow Content 的「query/fix retain stub」scenario 改為僅「fix retain stub」

## Impact

- Affected specs:
  - Modified: `cli`, `skill-bundles`
- Affected code:
  - New: 無
  - Modified:
    - codebus-cli/src/commands/query.rs
    - codebus-cli/src/main.rs
    - codebus-cli/tests/cli_routing.rs
    - codebus-core/src/skill_bundle/mod.rs
  - Removed: 無
- Affected tests (new):
  - codebus-cli/tests/query_flow.rs
