## Why

`codebus goal "..."` 在 v3 之前（含 v3-workspace stub）只是個印 `not yet implemented` 的 stub。Path D 的核心 verb 之一終於要實作：binary spawn `claude -p` 帶 slash command 觸發 `codebus-goal` skill bundle，agent 在 vault cwd 內讀 raw / 寫 wiki，binary 用 v2 iter-9 的三旗 sandbox 鎖死 toolset。本 change 把 spawn pattern + auto-init fallback + source-signal detection re-sync + spawn 收尾 auto_commit 一次落地，並把 codebus-goal/SKILL.md 從 stub 升級為含 5-step ingest workflow 的完整內容。Sandbox 三旗模式已於 2026-05-09 透過 `_pii-toolgate-spike` 5-cell 對照 spike 驗證為 hard gate（[roadmap §6](file:///D:/side_project/codebus/docs/v3-roadmap.md)）。

## What Changes

- 新增 Rust 模組 `codebus-core/src/agent/claude_cli.rs`：single impl，pub fn `invoke(opts: InvokeAgentOptions) -> io::Result<ExitStatus>`；不寫 trait（[anti-pattern #1](file:///D:/side_project/codebus/docs/v3-roadmap.md)）。內部 `std::process::Command::new("claude")` + 三旗 + slash command + cwd + stdio inherit。
- 新增 Rust 模組 `codebus-core/src/vault/source_signal_detect.rs`：實作 source-signal drift detection — 比對 `manifest.yaml` 內的 `source_signal` 與當前 source 重算結果（git_head / file_count / total_bytes），任一欄位不一致 → 視為 drifted。Detection 自身失敗（manifest 無法讀 / git 不可呼叫）採 fail-safe，回傳 drifted 強制 re-sync。
- 改寫 `codebus-cli/src/commands/goal.rs`：5 步流程 — clap parse → resolve repo → auto-init if `.codebus/` missing → detection-based re-sync (or `--force-resync` 強制) → spawn agent → 收尾 `auto_commit "wiki: {goal}"`。
- 修改 capability `cli`：新增 Goal Subcommand Behavior requirement 規範新 verb。
- 修改 capability `vault`：新增 Source-Signal Detection on Verb Invocation requirement 規範 detection 行為（含 fail-safe）。
- 修改 capability `skill-bundles`：新增 Goal Bundle Workflow Content requirement，把 `codebus-goal/SKILL.md` 從 placeholder workflow 段升級為 5-step ingest 內容（raw 探索 → page 規劃 → frontmatter+body → wikilinks → 摘要）。Schema 規則仍以 `CLAUDE.md` by-reference 方式持有（[anti-pattern #2「不雙投遞」](file:///D:/side_project/codebus/docs/v3-roadmap.md)），不 inline 進 SKILL.md。
- 修改 `codebus-core/src/skill_bundle/mod.rs::stub_content`：goal verb 分支回傳新 full workflow 字串；query / fix verb 仍是 stub（#6 / #8 各自處理）。
- spawn 收尾：不論 child exit code 是否 0 一律呼叫 `auto_commit`（v2 carry：clean tree no-op、dirty tree 保留 partial 紀錄）；codebus 本身 propagate child exit code。

## Non-Goals

- query / fix verb 共用 `agent::claude_cli::invoke` 的接口設計：本 change 內不過度抽象 InvokeAgentOptions 為了「未來 query / fix」彈性；#6 v3-query / #8 v3-fix 自己 wire 各自 toolset 時若需要 refactor 再做。
- agent / claude 子 process 的 `model` / `effort` flag 透傳：v2 InvokeOptions 帶這兩欄位，v3 path D 還沒 user-visible 必要。等 #9 v3-config 真接 config-driven 時加。
- Token usage tracking：v2 phase 1 carry，path D 不列。
- Source enrichment / stale page detection：v2 carry，本 change scope 排除（會 inflate task 數）。
- Multi-LLM provider：[follow-up `v3-multi-agentic-provider`](file:///D:/side_project/codebus/docs/v3-roadmap.md)。
- 改既有 init.rs 的 raw_sync 邏輯：本 change 的 detection 是 vault module 新 helper，goal.rs 自己呼叫 `sync_with_scanner`（既有 API）；不動 init.rs。
- 把 SKILL.md description 字串依 Claude Code auto-activation 機制最佳化：v3 binary fork claude -p 帶 explicit slash trigger，不靠 description-driven auto-activation。description 維持骨架現值。

## Capabilities

### New Capabilities

(none)

### Modified Capabilities

- `cli`: 新增 Goal Subcommand Behavior requirement（描述 v3-goal verb 5-step 流程 + 三旗 sandbox + auto-init + force-resync flag + propagate exit code）
- `vault`: 新增 Source-Signal Detection on Verb Invocation requirement（讀 manifest source_signal vs 重算 current 比對 / drift 條件 / fail-safe 策略）
- `skill-bundles`: 新增 Goal Bundle Workflow Content requirement（codebus-goal/SKILL.md 須含 5-step ingest workflow 與 schema by-reference 規範）

## Impact

- Affected specs:
  - Modified: `cli`, `vault`, `skill-bundles`
- Affected code:
  - New:
    - codebus-core/src/agent/mod.rs
    - codebus-core/src/agent/claude_cli.rs
    - codebus-core/src/vault/source_signal_detect.rs
  - Modified:
    - codebus-core/src/lib.rs
    - codebus-core/src/vault/mod.rs
    - codebus-core/src/skill_bundle/mod.rs
    - codebus-cli/src/commands/goal.rs
    - codebus-cli/tests/cli_routing.rs
    - codebus-core/tests/vault_init.rs
  - Removed: 無
