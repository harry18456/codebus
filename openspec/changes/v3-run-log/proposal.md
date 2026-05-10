## Why

v3.0.0 ship 後最大的觀測 gap：**user 看不到每個 verb 跑了多少 token、花了多少時間、agent 在 session 內做了什麼**。`codebus goal "..."` 跑 5-30 分鐘、燒未知量 token、UV repo 驗收期間光等待沒有任何中間訊號 — agent stdout 直接 passthrough（`Stdio::inherit()`），claude 自己就當 user 對著黑盒。

v2 解這問題用「parse claude `--output-format stream-json` 的每個 line + render thought / tool / observation event + 收 result event 的 usage 欄位寫 RunLog」整套。v3 path D 為了簡化把這層拆掉了，但 `legacy/v2-rust/codebus-core/src/{stream,log,render}/` 的 600+ LoC 實作仍可 carry — 這次完整 port，補回 token tracking + per-run jsonl history + 即時 agent 進度可視化。

關鍵 trade-off 已在 discuss 階段確認：claude `--print` 三種 output format 中只有 `stream-json` 含 usage event，但是 JSON-per-line 不是人類可讀；要拿 token 又要保留 user 中段可視化，必須**自己 parse + render** — 沒有 passthrough + extract 兩全的捷徑。

## What Changes

- **新增 `agent-stream-rendering` capability**：定義 claude stream-json wire format → `StreamEvent` enum（Thought / ToolUse / ToolResult / Usage）的 mapping；定義 codebus 端的 render 行為（emoji ↔ ASCII fallback、Write/Edit 特化「正在生成」、tool result 截斷至 200 chars、tool result echo 抑制）。Port 自 v2 `stream::parser` + `render::renderers::terminal::format_event`。
- **新增 `run-log` capability**：定義 `RunLog` struct（goal / mode / model / effort / started_at / finished_at / TokenUsage / wiki_changed / lint_error_count / lint_warn_count）+ `LogSink` trait + `JsonlSink`（date-rotated `<dir>/runs-YYYY-MM-DD.jsonl`） + `NullSink`（opt-out）。`pii.on_hit` 那種 scalar tag 風格的 `log.sink: jsonl | null` config。Port 自 v2 `log::{sink,sinks::*}`。
- **`codebus_core::agent::claude_cli::invoke` 行為翻新**：spawn 時加 `--output-format stream-json --verbose --input-format stream-json`；stdout / stderr 從 `Stdio::inherit()` 改 `Stdio::piped()`；同步用 `BufReader::lines()` 讀 stdout 一行一行 parse + render + 累計 usage；stderr 在 background thread passthrough（用 `std::thread::spawn` + `io::copy`）。函式 signature 改：除既有 `ExitStatus` 外，**回傳一個 `InvokeReport { exit: ExitStatus, accumulated_tokens: TokenUsage, started_at: String, finished_at: String }`** 給 caller 寫 RunLog 用。
- **goal / query / fix 三 verb 全部接 stream rendering + RunLog**：
  - 從 `claude_cli::invoke()` 拿 `InvokeReport` → 收尾組 `RunLog` → 透過 `LogSink::write_run` 寫入
  - render banner 順序：codebus banner（Start / Goal / SyncStart / SyncDone / PiiSummary）→ stream event banners（Thought / ToolUse / ToolResult，由 invoke 內部代為 emit）→ codebus banner（LintStart / LintDone / CommitDone / Done）
- **新增 `log` config section**：
  ```yaml
  log:
    sink: jsonl     # default; 或 "null" 完全關掉
    dir: ~/path     # optional override; 預設 <vault>/.codebus/log/
  ```
  `<vault>/.codebus/log/` 目錄 init 已建立（v3 既有），無新增 mkdir 步驟。
- **mock-claude 測試 fixture 升級**：`tests/bins/mock_claude.rs` 加新 behavior `success-stream-json` / `failure-stream-json` — 寫 4-6 條合法 stream-json line（`assistant text` + `assistant tool_use` + `user tool_result` + `result with usage`）讓 integration test 能驗 parser → renderer → log sink 整條 pipeline。
- **修正既有 internal `.gitignore` 與 vault 目錄 naming 不一致**：`init.rs` `INTERNAL_GITIGNORE_LINES` 含 `logs/`（plural），但 `vault::layout` 實際建 `log/`（singular）。本 change 統一為 `log/`（單數，對齊既有 disk state；改 gitignore line）。

## Non-Goals

- **不引入 async runtime / tokio**：v3 沒有 tokio dependency，加進來會大幅膨脹依賴與編譯時間；v2 用 async 是為了 `Stream` trait 串流 model，sync `BufReader::lines()` 在我們 use case（一個 process 一個 reader）行為相同
- **不 port v2 thought 的 markdown 風格化**（`render::markdown_style`）：v2 用 ANSI escape 對 `**bold**` `_italic_` `[[wikilink]]` 加樣式；v3 path D 下 thought 是 agent 自由文字，markdown 樣式收益小、增加 200+ LoC 維護面，留給 follow-up 評估
- **不 port OSC 8 wikilink wrap in thought events**：要 slug_index per-run rebuild，跟 v3-render-polish 已有的 lint OSC 8 是不同 use case；確實有需要時再開 `v3-thought-wikilinks`
- **不引入 OTel sink**：v2 留了 `Otel {}` variant 跟 `log-otel` cargo feature flag — 完全沒人用過。本 change 只 port `Null` + `Jsonl` 兩個變體
- **不 port `LlmProvider` trait**：v3 path D 明確不寫 single-impl trait（「single-impl 不寫 spec」原則）；`claude_cli::invoke` 直接擴 signature，不抽象
- **不改 hook subcommand**：`codebus hook check-bash` 走 stdin JSON / stdout JSON contract，跟 spawn 的 stdio 架構無關
- **不影響 lint subcommand**：lint 不 spawn claude，stdio 變動不影響
- **不增加 user-visible CLI flag**：所有行為由 config 控；不加 `--no-log` / `--stream-json` / `--render-stream` 之類旗子（v3-config 已決定 config 不 user-flag overlap）

## Capabilities

### New Capabilities

- `agent-stream-rendering`: claude stream-json wire format → `StreamEvent` enum 的 mapping、event 種類列表、each-event 的 codebus 端 render 規則（emoji + ASCII fallback、Write/Edit 特化、tool_result 截斷規則、Done event 是 marker 不渲染）。
- `run-log`: `RunLog` struct schema、`LogSink` trait + `JsonlSink`（date-rotated 檔案命名 + 行格式 + 並行 append 行為） + `NullSink`、`log.sink` config、`<vault>/.codebus/log/` default dir。

### Modified Capabilities

- `cli`: goal / query / fix subcommand 行為新增「stream rendering 階段」與「收尾 RunLog 寫入」步驟；`claude_cli::invoke` 簽章與 stdio 行為破壞性改動（`Stdio::inherit()` → `Stdio::piped()`，回傳 `InvokeReport` 取代純 `ExitStatus`）

## Impact

- Affected specs: `agent-stream-rendering` (new), `run-log` (new), `cli` (modified)
- Affected code:
  - New:
    - codebus-core/src/stream/mod.rs
    - codebus-core/src/stream/parser.rs
    - codebus-core/src/log/mod.rs
    - codebus-core/src/log/sink.rs
    - codebus-core/src/log/sinks/mod.rs
    - codebus-core/src/log/sinks/null_sink.rs
    - codebus-core/src/log/sinks/jsonl_sink.rs
    - codebus-core/src/log/factory.rs
    - codebus-core/src/config/log.rs
    - codebus-core/src/render/stream_event.rs
  - Modified:
    - codebus-core/src/lib.rs
    - codebus-core/src/agent/claude_cli.rs
    - codebus-core/src/render/mod.rs
    - codebus-core/src/render/banner.rs
    - codebus-core/src/config/mod.rs
    - codebus-core/src/config/global_starter.rs
    - codebus-cli/src/commands/init.rs
    - codebus-cli/src/commands/goal.rs
    - codebus-cli/src/commands/query.rs
    - codebus-cli/src/commands/fix.rs
    - codebus-cli/src/main.rs
    - codebus-cli/tests/bins/mock_claude.rs
    - codebus-cli/tests/cli_routing.rs
    - codebus-cli/tests/goal_flow.rs
    - codebus-cli/tests/query_flow.rs
    - codebus-cli/tests/fix_flow.rs
    - codebus-core/src/wiki/fix/mod.rs
