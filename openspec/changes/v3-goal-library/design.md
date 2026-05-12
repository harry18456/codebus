## Context

codebus v3 主線 10 條 change 2026-05-10 ship 後進 app v1（`v3-app-foundation` 已 archive 2026-05-11，含 Tauri shell / Lobby / Settings / Workspace stub）。`v3-app-roadmap` §Sequence 規劃下一條為 `v3-app-workspace-goal`（C change），動工前 spectra-discuss（紀錄 `docs/2026-05-12-v3-app-workspace-goal-discussion.md`）發現倚賴的 CLI 側基建有 2 個未做的洞，必須先以兩條獨立 prerequisite change 補完：

- **A `v3-goal-library`**（本 change）— 抽 3 個 spawn verb orchestration 進 codebus-core library + `invoke()` 加 callback
- **B `v3-run-log-events`**（下一條）— RunLog schema 加 outcome / events.jsonl 持久化

CLI 現況（實機 grep `codebus-cli/src/commands/*.rs` 取得）：

| Verb | CLI 端結構 | 抽？ |
|---|---|---|
| init | 已抽（foundation `init::run_init` 接 on_event closure，CLI thin wrapper） | — |
| lint | thin wrapper（40 行 clap + `wiki::lint::lint_wiki()` + format text/json） | 不抽 |
| goal | ~250 行 orchestration（drift / sync / invoke / fix loop / auto-commit / RunLog） | 抽 |
| query | ~100 行 orchestration（vault precondition / config / env / invoke / RunLog） | 抽 |
| fix | ~150 行 orchestration（vault precondition / lint pre-check / invoke / fix loop / final lint / auto-commit / RunLog） | 抽 |

`codebus_core::agent::invoke()`（`codebus-core/src/agent/claude_cli.rs`）目前實作 stream loop 寫死 `parse_claude_stream_line` → `print_event(... println!)`，render 跟 invoke 綁死無 hook，GUI 想 emit StreamEvent 到 Tauri event bus 沒入口。

## Goals / Non-Goals

**Goals:**

- `codebus_core::verb::{goal,query,fix}` 三個新 library function 對外暴露，GUI（C change `v3-app-workspace-goal`）能直接 call、不需重做 orchestration
- `agent::invoke()` 透過 caller-supplied `on_event` callback 收 StreamEvent；CLI 端 closure 包既有 `print_event` 渲染為 terminal stdout；GUI 端 closure 把 StreamEvent emit 到 Tauri event bus
- `agent::invoke()` 接 `cancel: Option<Arc<AtomicBool>>` 旗標，流式讀 loop 每筆 event 後 check，flip true → kill child + 中斷 loop（GUI 的 Cancel 按鈕底層基建）
- CLI 三個 commands 變 thin wrapper，stdout / stderr / exit code byte-equivalent — 既有 cli_routing / verb_flow / goal_flow / query_flow / fix_flow integration tests 全綠作為驗收
- 新增 codebus-core verb library 的 unit test 覆蓋 callback 觸發 / cancel mid-stream / VerbReport 欄位正確性

**Non-Goals:**

- 不引入 provider trait（claude_cli 仍 single impl）— second impl 真要進來再開 change 設計 trait surface（v3-roadmap §3 anti-pattern #1）
- 不改 RunLog schema 或新增 events.jsonl 持久化 — 那是 B change
- 不導入 tokio / async runtime 給 invoke()（保留 std sync 實作；cancel 走 `Arc<AtomicBool>` polling 不引 `tokio_util::sync::CancellationToken`）
- 不抽 lint / init（lint 已是 thin wrapper、init 已抽）
- 不改 sandbox flag / toolset / slash command 行為（純位置搬移）
- 不建 GUI、不寫 Tauri IPC、不動 codebus-app — 全是 C change 範圍
- 不改 banner 文字、不改 exit code policy、不改 auto-commit 訊息 — 對外行為 byte-equivalent

## Decisions

### 新 module 路徑：codebus_core::verb 而非 cmd 或 agent::run

選 `verb::*` 的理由：

- `Verb` enum 已存在 `codebus_core::config`（per-verb model/effort resolution），命名一致
- `agent::*` 模組目前 scope 是「spawn claude 子程序」，加 verb orchestration 進去職責變模糊（orchestration 涉及 vault / git / log / wiki::fix 多個模組）
- `cmd::*` 容易跟 CLI commands/ 混淆

`verb` 模組對齊既有 `init::run_init`（在 `vault::init::run_init`，因為 init 操作 vault layout）；goal / query / fix 都是「跨 capability 的 verb orchestration」沒有單一 owning capability，所以給獨立 top-level module。

Alternatives considered：`codebus_core::cmd::*`（與 CLI 混淆）、`codebus_core::agent::run_*`（agent 模組職責變太雜）、`codebus_core::orchestration::*`（過度泛化）。

### Cancel 機制：Arc AtomicBool polling 而非 tokio CancellationToken

選擇理由：

- `invoke()` 既有實作純 sync `std::process::Command` + std thread；引入 tokio runtime 等於拖整個 dependency
- `Arc<AtomicBool>` 是 std，零依賴；GUI 端拿一個 `Arc<AtomicBool>` 給 invoke()、Cancel 按鈕 flip true
- Polling 點明確：stream loop 每筆 event 後 check 一次（events 間隔通常 < 100ms，cancel latency 可接受）
- 失活時 invoke() 行為：`child.kill()` + drain stdout 後 return `Ok(InvokeReport)`，`exit` 欄位反映被 kill 的狀態；caller（verb function）依此判斷 outcome

Alternatives considered：tokio CancellationToken（引入新 runtime 不值得）、async invoke + tokio::process（重寫整套 sync 邏輯 risk 過高）、channel-based abort signal（多一個 sync primitive 沒必要）。

### on_event callback 簽名：impl FnMut StreamEvent 同步 closure

選擇理由：

- StreamEvent 來自 sync stream loop，callback 也 sync 一致
- `FnMut` 允許 closure mutate 外部 state（CLI 端要 accumulate render state、GUI 端要 emit）
- 沒回傳值（caller 自決定怎麼用，errors 在 closure 內處理）
- `accumulate_token_usage` 留在 invoke() 內部（Usage 是 RunLog 必要欄位，每個 caller 都需要，不該丟給 callback 重複寫）

Alternatives considered：`Box<dyn FnMut>`（多一層 vtable，無實際好處）、async callback（與 sync loop 不合）、Iterator 回傳（child lifetime 難管 + cancel 路徑複雜）。

### VerbReport / VerbError 型別：per-verb 結構回傳

每個 verb 的 success/failure 結構略不同：

- `GoalReport`：accumulated_tokens / wiki_changed / lint_error_count / lint_warn_count / wiki_pages_changed（後續 GUI 列改動清單用）
- `QueryReport`：accumulated_tokens（無 wiki / lint 欄位 — query 是 read-only）
- `FixReport`：accumulated_tokens / wiki_changed / final_lint_error_count / final_lint_warn_count / fix_iterations

VerbError 走 thiserror enum：`VaultMissing { path }` / `ConfigParse { source }` / `Spawn { source }` / `Cancelled` / `Internal { message }`。CLI 端 match enum 對應 exit code（保持現有 0/1/2/3 對應）。

Alternatives considered：單一 `VerbReport` union（欄位 Option 過多，型別失語意）、`Box<dyn VerbReport>` trait（單 impl 抽象 → 違反 anti-pattern #1）。

### CLI 端 closure 結構：保留 RenderOptions 傳遞，closure 內呼 print_event

CLI 端的 callback closure pseudo-code：

```
let render_opts = render_opts.clone();
let on_event = move |event| { print_event(&event, &render_opts); };
verb::goal::run_goal(repo, options, on_event, cancel)
```

`print_event` 函數簽名 / 行為完全保留，純粹從「invoke() 內部呼叫」變成「CLI thin wrapper 透過 closure 呼叫」。terminal output byte-equivalent 透過既有 `goal_flow.rs` / `query_flow.rs` / `fix_flow.rs` integration test 驗收。

Alternatives considered：把 `print_event` 也搬進 library 變成 default callback（GUI 還是要覆寫，反而多一層抽象）、CLI 端不傳 closure 而是 invoke() 暴露 events iterator（child lifetime 難管）。

### codebus-cli/src/run_log.rs 的位置

`run_log.rs` 含 `load_log_config_with_warning` / `resolve_sink_dir` / `write_run_log` / `wiki_changed_since_last_commit` 等 helper。3 個 verb（goal / query / fix）都 import 使用。

抽 verb 進 core 後，這些 helper 也要跟著去 core 端。決策：

- pure 邏輯（`resolve_sink_dir` / `write_run_log` / `wiki_changed_since_last_commit`）搬進 `codebus_core::log::verb_log`
- `load_log_config_with_warning` 拆兩半：`codebus_core::log::load_log_config`（pure Result，無 stderr emit）+ CLI thin wrapper 端保留 stderr warning emit（保持 CLI byte-equivalent stderr）

Alternatives considered：run_log helper 全留在 CLI 但 library function 接受 caller 傳入 RunLog write closure（過度複雜化）、把 stderr emit 也搬進 library（破壞 library 純 functional 原則）。

### Banner milestone 事件統一為 VerbEvent enum

3 個 verb library function 內部需要 emit 多種事件：

- 既有 banner milestone（Start / Goal / SyncStart / SyncDone / FixStart / CommitStart / CommitDone / Done）
- StreamEvent（Thought / ToolUse / ToolResult / Usage）— invoke() 內部產生
- 新增 verb 生命週期事件（SpawnStart / SpawnEnd / FixIterationStart / LintFinal — 給 GUI 顯示 progress 用）

決策：define unified `VerbEvent` enum 包 3 種：

```
pub enum VerbEvent {
    Banner(BannerKind),
    Stream(StreamEvent),
    Lifecycle(VerbLifecycleEvent),
}
```

verb library function 接 `on_event: impl FnMut(VerbEvent)`，invoke() 仍接 `on_event: impl FnMut(StreamEvent)`（invoke 是 lower-level，純 stream）。verb function 內部把 invoke() 的 StreamEvent 包成 `VerbEvent::Stream` 再轉發給 caller。

CLI 端 closure：

```
let on_event = move |event| match event {
    VerbEvent::Banner(b) => print_banner(b, &render_opts),
    VerbEvent::Stream(s) => print_event(&s, &render_opts),
    VerbEvent::Lifecycle(_) => {} // CLI 不渲染 lifecycle，純 GUI 用
};
```

Alternatives considered：兩個 callback（`on_banner` + `on_stream`，呼叫端寫得繁瑣）、callback 接 `&dyn Any`（型別失語意）、library 內部直接呼 print_banner（CLI byte-equivalent 但 GUI 拿不到 banner milestone）。

## Implementation Contract

### Behavior

- **CLI 對外行為 byte-equivalent**：`codebus goal "..." [--force-resync] [--no-fix]` / `codebus query "..."` / `codebus fix [--no-fix]` 三個命令的 stdout banner / `print_event` 渲染輸出 / stderr error 訊息 / exit code 與 refactor 前完全一致
- **codebus-core 公開新 library function**：`verb::goal::run_goal` / `verb::query::run_query` / `verb::fix::run_fix` 三個 pub function，caller 可從 codebus-cli 或 codebus-app 直接呼叫
- **`agent::invoke()` 簽名變更**：增加 2 個參數（`on_event` callback、`cancel` signal）；既有 caller（CLI 透過 verb library 間接呼叫）行為一致
- **Cancel 行為**：caller flip `Arc<AtomicBool>` true → invoke() stream loop next iteration 偵測 → `child.kill()` → drain 剩餘 stdout（best-effort）→ return `Ok(InvokeReport { exit: <kill狀態> })`；verb function 視 `cancel.load(true)` 為 cancel 路徑回 `Err(VerbError::Cancelled)`，並跳過 auto-commit step

### Interface / data shape

**codebus-core agent invoke 新簽名（codebus-core/src/agent/claude_cli.rs）**：

```
pub fn invoke(
    opts: InvokeAgentOptions,
    on_event: impl FnMut(StreamEvent),
    cancel: Option<Arc<AtomicBool>>,
) -> io::Result<InvokeReport>
```

註：移除 `render_opts: &RenderOptions` 參數（render 責任移到 caller closure）。

**codebus-core verb goal（codebus-core/src/verb/goal.rs）**：

```
pub struct GoalOptions {
    pub text: String,
    pub force_resync: bool,
    pub no_fix: bool,
    pub no_obsidian_register: bool,
}
pub struct GoalReport {
    pub accumulated_tokens: TokenUsage,
    pub wiki_changed: bool,
    pub lint_error_count: usize,
    pub lint_warn_count: usize,
    pub started_at: String,
    pub finished_at: String,
}
pub fn run_goal(
    repo: &Path,
    options: GoalOptions,
    on_event: impl FnMut(VerbEvent),
    cancel: Option<Arc<AtomicBool>>,
) -> Result<GoalReport, VerbError>
```

**codebus-core verb query（codebus-core/src/verb/query.rs）**：類似結構，`QueryOptions { text }` / `QueryReport { accumulated_tokens, started_at, finished_at }`，無 wiki / lint 欄位。

**codebus-core verb fix（codebus-core/src/verb/fix.rs）**：類似結構，`FixOptions { no_fix }` / `FixReport { accumulated_tokens, wiki_changed, final_lint_error_count, final_lint_warn_count, fix_iterations, started_at, finished_at }`。

**codebus-core verb error（codebus-core/src/verb/mod.rs）**：

```
pub enum VerbError {
    VaultMissing { path: PathBuf },
    ConfigParse { source: ConfigLoadError },
    Spawn { source: io::Error },
    Cancelled,
    Internal { message: String },
}
```

**codebus-core verb event（codebus-core/src/verb/mod.rs）**：

```
pub enum VerbEvent {
    Banner(BannerKind),
    Stream(StreamEvent),
    Lifecycle(VerbLifecycleEvent),
}
pub enum VerbLifecycleEvent {
    SpawnStart { verb: Verb },
    SpawnEnd { verb: Verb, exit_code: Option<i32> },
    FixIterationStart { iteration: u8 },
    LintFinal { error_count: usize, warn_count: usize },
}
```

### Failure modes

- `VerbError::VaultMissing` → CLI exit code 2（query / fix 路徑），goal 路徑 auto-init 後再 retry（保留現行行為）
- `VerbError::ConfigParse` → CLI exit code 2（per `cli` spec 既有 fail-loud 行為）
- `VerbError::Spawn` → CLI exit code 1，stderr 印 underlying error
- `VerbError::Cancelled` → CLI 不會 hit；GUI 拿到後 detail view 顯示 cancelled，**不 auto-commit**（由 verb function 內部跳過 auto-commit step）
- `VerbError::Internal` → CLI exit code 1，stderr 印 message
- `agent::invoke()` 內部 spawn 失敗（binary not found / fork failure）→ `io::Result::Err` 上傳，verb function 翻成 `VerbError::Spawn`

### Acceptance criteria

- 既有 27+ 個 integration test 全綠：`codebus-cli/tests/cli_routing.rs` / `goal_flow.rs` / `query_flow.rs` / `fix_flow.rs`（透過 mock_claude 驗 byte-equivalent stdout / stderr / exit code）
- 新增 codebus-core verb library unit test：
  - `verb::goal::run_goal` 在 on_event 上看到 ≥ 1 個 ToolUse + ≥ 1 個 Usage 後 callback 觸發次數正確
  - `verb::goal::run_goal` 接到 cancel flip true 後 ≤ 1 個 event 內中斷，回 `Err(VerbError::Cancelled)`，不 auto-commit
  - `verb::query::run_query` 在缺 vault 時回 `Err(VerbError::VaultMissing)`、不 spawn agent
  - `verb::fix::run_fix` 在 lint 0 issues pre-check 時 short-circuit、不 spawn agent
  - `agent::invoke` 用 `Vec<StreamEvent>` 收集 closure，驗證 events 順序與 parse output 一致
- `cargo build --workspace` 通過、`cargo clippy --workspace -- -D warnings` 通過、`cargo fmt --all --check` 通過
- Manual verification（Windows MSVC）：`codebus goal "test"` / `codebus query "test"` / `codebus fix` 對 `<vault>/.codebus/` 跑通並產生與 refactor 前一致的 banner + run-log entry

### Scope boundaries

**In scope:**

- `agent::invoke()` 簽名變更 + on_event + cancel signal 邏輯
- 新增 `codebus_core::verb::{goal,query,fix}` 三個 module + run_* function + GoalOptions/QueryOptions/FixOptions + GoalReport/QueryReport/FixReport + VerbError + VerbEvent + VerbLifecycleEvent
- CLI 三個 commands 變 thin wrapper
- run_log helper 搬移（pure 邏輯入 codebus-core；stderr warning emit 留在 CLI）
- 既有 integration test 通過 + 新增 verb library unit tests

**Out of scope:**

- GUI / Tauri / IPC（C change）
- RunLog schema 變更 / events.jsonl 持久化（B change）
- Provider trait / multi-agentic provider abstraction（v3-multi-agentic-provider follow-up）
- async / tokio runtime 導入
- lint / init refactor（lint 已 thin、init 已抽）
- banner 文字 / exit code policy / auto-commit 訊息變更

## Risks / Trade-offs

- **CLI byte-equivalent regression risk** → refactor 動 3 個 verb 的 orchestration，stdout / stderr 任何 drift 都會被 integration test 抓到 → Mitigation：mock_claude based golden test 已存在（cli_routing 27 test、加 goal_flow / query_flow / fix_flow），先把這些補強為「stdout byte-snapshot」如必要、再開始 refactor
- **on_event closure 跨 move 限制** → `impl FnMut` 簽名在 `invoke()` 內部跨 stream loop 多次呼叫，Rust 借用檢查可能在 CLI 端 closure 捕獲 `RenderOptions` 時報錯 → Mitigation：CLI 端 closure 內 `clone()` render_opts；若 lifetimes 仍卡，局部改 `Box<dyn FnMut>`（不寫進 spec surface）
- **Cancel polling 點密度** → events 間隔可能長（agent thinking 數秒、tool call 數十秒），cancel latency 上限 = 一個 event interval → 接受；GUI Cancel 按鈕 UI 顯示「Cancelling…」即可
- **Banner milestone 事件設計** → unified `VerbEvent` 包 3 種事件 type，CLI 端要 match 全部、GUI 端可選擇處理，型別擴充風險可控 → Mitigation：specs 階段把 `VerbEvent` 變體列為 normative requirement，未來增 lifecycle event 走 minor change
- **VerbError 變體擴張** → `VaultMissing` / `Cancelled` / `ConfigParse` / `Spawn` / `Internal` 對 CLI exit code mapping 必須完整 → Mitigation：CLI thin wrapper 對 enum 做 exhaustive match（編譯時保證 mapping 完整）
- **`load_log_config_with_warning` 的 stderr emit 位置** → library 內部 emit stderr 跟 library 行為純 functional 原則矛盾 → Mitigation：library 純 `Result`，warning 由 CLI thin wrapper emit；保持 CLI stderr byte-equivalent
- **run_log helper 搬家觸發 import cascade** → 多檔 import path 變動，可能漏改 → Mitigation：tasks.md 把 helper 搬家拆成獨立 step，搬完先跑 `cargo check --workspace` 確認再進下個 step
