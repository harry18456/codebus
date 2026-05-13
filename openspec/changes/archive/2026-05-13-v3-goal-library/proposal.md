## Why

v3-app-roadmap §Sequence 規劃 `v3-app-workspace-goal` 在 Tauri app 跑 goal flow（含 live stream rendering / cancel / run-log）。動工前 spectra-discuss（紀錄於 `docs/2026-05-12-v3-app-workspace-goal-discussion.md`）發現 2 個 CLI 側基建洞：

1. **`codebus_core::agent::invoke()` stream render 與 invoke 綁死、沒 callback hook** — 內部 loop 寫死 `parse → print_event → println!`，GUI 想 reuse pipeline 沒入口
2. **3 個 spawn verb（goal / query / fix）的 top-level orchestration 寫在 CLI 端** — `codebus-cli/src/commands/{goal,query,fix}.rs` 各自 ~100-250 行（vault precondition / config / env / invoke / fix loop / auto-commit / run-log），GUI 無法 reuse；強重做等於 spec-drift 風險來源（CLI 改了 GUI 不知道）

兩個洞合起來解最划算：`invoke()` callback refactor 一動，3 個 verb 的 callsite 都要連動改，所以順手抽 orchestration 進 library function 比讓 3 份「closure 包 print_event」散在 CLI 端乾淨。本 change 是 `v3-app-workspace-goal` 動工前的 prerequisite refactor，CLI 對外行為 byte-equivalent。

## What Changes

- **`agent::invoke()` 簽名擴展**：加 `on_event: impl FnMut(StreamEvent)` callback（caller 收每個 StreamEvent 自決定 render）與 `cancel: Option<Arc<AtomicBool>>` 旗標（流式讀 loop 每筆 event 後 check，flip true → kill child + 中斷 loop）。`invoke()` 內部 sync `std::process::Command` 實作不改
- **抽 3 個 spawn verb 為 library function**：新增 `codebus_core::verb::{goal,query,fix}` 模組，每個內含 `run_*(repo, options, on_event, cancel) -> Result<VerbReport, VerbError>`
  - `verb::goal::run_goal` — drift detection / re-sync / invoke / fix loop / auto-commit / RunLog write
  - `verb::query::run_query` — vault precondition / invoke / RunLog write（無 auto-commit）
  - `verb::fix::run_fix` — vault precondition / lint pre-check / invoke / fix loop / final lint / auto-commit / RunLog write
- **CLI 三個 commands 變 thin wrapper**：`codebus-cli/src/commands/{goal,query,fix}.rs` 只剩 clap arg parse / banner closure / `print_event` closure / call library / 翻 `VerbReport` 成 ExitCode。CLI 對外 stdout / stderr / exit code byte-equivalent（既有 cli_routing / verb_flow / goal_flow / query_flow / fix_flow 測試全綠作為驗收）
- **lint / init 不動**：`commands/lint.rs` 已是 thin wrapper（40 行）+ core lint logic 早在 `codebus_core::wiki::lint::lint_wiki()`；init 由 foundation `v3-app-foundation` 已抽為 `codebus_core::vault::init::run_init`

## Non-Goals

- **不引入 provider trait**：`agent::invoke()` 仍是 single impl（claude_cli），trait surface 等 codex / gemini second impl 真要進來再開 change（v3-roadmap §3 anti-pattern #1：no single-impl abstraction in spec）
- **不改 RunLog schema**：`outcome` 欄位（succeeded / failed / cancelled）與 per-run events.jsonl 是 B `v3-run-log-events` 的事；A change 只動 orchestration 位置不改持久化格式
- **不改 sandbox flag / toolset / slash command**：各 verb 的 toolset 常量保留現位置（CLI const 或搬進 library const 都可），triple-flag sandbox 行為完全保留
- **不建 GUI**：`spawn_goal` / `cancel_goal` IPC、Tauri event emit、Vault Workspace UI 全是後續 `v3-app-workspace-goal`（C change）範圍
- **不導入 async / tokio**：保留 invoke() 既有 sync 實作；cancel 用 `Arc<AtomicBool>` polling 不引入 `tokio::process::Command` 或 `tokio_util::sync::CancellationToken` 依賴
- **不抽 init**：foundation 已抽 `init::run_init`，本 change 不重抽
- **不改 SKILL.md / 不動 skill-bundles 寫入邏輯**：純 Rust orchestration 位置變動

## Capabilities

### New Capabilities

- `verb-library`: codebus-core verb orchestration library — `verb::{goal,query,fix}::run_*` 三個 library function 的簽名、`on_event` callback 語意、cancel signal 行為、`VerbReport` / `VerbError` 型別、與 `agent::invoke()`、`wiki::fix::run_fix_loop`、`git::auto_commit`、`log::RunLog` 等既有 core primitive 的 composition 契約

### Modified Capabilities

- `agent-stream-rendering`: `invoke()` 簽名擴展為接受 caller-supplied `on_event` callback；既有 terminal renderer 行為改為「CLI thin wrapper 端透過 closure 呼叫」，但渲染輸出 byte-equivalent
- `cli`: 三個 spawn verb（goal / query / fix）的 behavior 要求保留（banner / exit code / sandbox / auto-commit 時機），但內部實作要求改為「thin wrapper 委派給 verb-library」；新增 requirement 要求 CLI 不再內嵌 verb orchestration
- `lint-feedback-loop`: fix loop spawn pattern 由 CLI 移到 `verb::fix::run_fix` library；既有 fix loop spec 行為（max iterations / Bash hook / final-only verifier）不變，但 caller 換成 library 而非 CLI

## Impact

- Affected specs: 新建 `openspec/specs/verb-library/spec.md`；修改 `openspec/specs/agent-stream-rendering/spec.md`、`openspec/specs/cli/spec.md`、`openspec/specs/lint-feedback-loop/spec.md`
- Affected code:
  - New:
    - codebus-core/src/verb/mod.rs
    - codebus-core/src/verb/goal.rs
    - codebus-core/src/verb/query.rs
    - codebus-core/src/verb/fix.rs
  - Modified:
    - codebus-core/src/lib.rs（pub mod verb）
    - codebus-core/src/agent/claude_cli.rs（invoke() 簽名加 on_event + cancel；既有 print_event 呼叫移除）
    - codebus-cli/src/commands/goal.rs（thin wrapper）
    - codebus-cli/src/commands/query.rs（thin wrapper）
    - codebus-cli/src/commands/fix.rs（thin wrapper）
    - codebus-cli/src/run_log.rs（依抽離結果決定保留 / 部分搬進 library）
  - Removed: (none — refactor 而非刪除)
- Affected dependencies: 無新增 Cargo crate
- Test coverage 影響：既有 27+ 個 cli_routing / verb_flow / goal_flow / query_flow / fix_flow integration tests 全綠是驗收門檻；新增 codebus-core verb library function unit tests（覆蓋 on_event callback 觸發、cancel signal middle-of-stream 中斷、VerbReport 欄位正確性）
