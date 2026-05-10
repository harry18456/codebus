## Context

v3.0.0 ship 後三大可觀測性 gap：

1. **agent in-session 動作不可見** — `Stdio::inherit()` 讓 claude 自己印什麼 user 看什麼，但 claude `--print` default text mode 只印 final assistant response，中間 5-30 分鐘 user 對著黑盒
2. **token / cost 不可量** — claude 不在 text mode 寫 usage；user 只能事後 `claude /cost` 拿 per-session 數字（不是 per-codebus-invocation）
3. **歷史不可查** — 沒有 per-run jsonl，user 無法知道「這個 vault 跑過幾次 goal、累積花了多少 token、哪幾條 wiki page 是哪次 goal 寫的」

v2 完整解這三 gap：spawn `claude -p --output-format stream-json --verbose --input-format stream-json` → BufReader 一行一行 parse 成 `StreamEvent` → render terminal-friendly form 給 user 看 → 收 `result` event 的 `usage` 寫入 `RunLog::tokens` → flush 到 `runs-YYYY-MM-DD.jsonl`。

v3 path D 為了「skill mode」設計把這層拆掉，但實作 600+ LoC（`stream::parser` 200 + `log::sink + sinks` 360 + `render` event 部分 ~120）已經 spike-verified、test-covered，可整批 carry。

discuss session 收斂結論：claude `--print` 三種 output format 中 **只 stream-json 含 usage event**，但 stream-json 是 JSON-per-line 不可讀；要兩者兼得必須 codebus 自己 parse + render — Option A 是唯一路徑。

## Goals / Non-Goals

**Goals:**

- 補回 token tracking — `<vault>/.codebus/log/runs-YYYY-MM-DD.jsonl` 每個 verb invocation 寫一條 `RunLog`
- 補回 agent in-session 可視化 — terminal 看到 `🤔 [Agent 思考]` / `🛠 [呼叫工具]` / `👀 [觀察結果]` 等事件
- 與 v3-render-polish banner 系統共存 — banner（codebus 印）跟 stream events（claude 透 codebus 渲染）在 codebus stdout 同條串流、順序可預期
- 維持 v3-render-polish 的 `RenderOptions` 機制 — emoji ↔ ASCII fallback、`NO_COLOR` 守 ANSI、非 TTY 自動降級
- v2 的 `LogSink` trait 接口 carry — 為將來 OTel sink 留接口（不在本 change 實作）

**Non-Goals:**

- **不引入 tokio**：v3 沒這 dep；spawn + BufReader 同步行為足夠
- **不 port markdown_style** — thought 文字不加 ANSI bold/italic 樣式
- **不 port slug_index + OSC 8 wikilink wrap in thought** — 需要 per-run 重 build slug index，工程大、收益遠不及現有 lint OSC 8
- **不 port LlmProvider trait** — single-impl 不寫抽象（v3 反覆強調的 anti-pattern）
- **不引入 OTel sink** — feature flag + dep 太重，0 user
- **不破壞 hook subcommand** — `codebus hook check-bash` 走獨立 stdin/stdout JSON 契約
- **不影響 lint subcommand** — lint 不 spawn agent
- **不加 user-visible CLI flag** — 統一走 `~/.codebus/config.yaml log:` section

## Decisions

### Stream parser port verbatim from v2，pure sync function

`legacy/v2-rust/codebus-core/src/stream/parser.rs::parse_claude_stream_line(&str) -> Vec<StreamEvent>` 已是 pure sync function（無 async / 無 IO，純 serde_json::from_str + match）— 直接 copy 過來、only change 是 import path。`StreamEvent` enum drop `Done` variant（v2 沒實際用、是 placeholder）。

對 4 種 outer `type`：
- `assistant` → 抽 content[] 為 0..N `Thought` / `ToolUse`
- `user` → 抽 content[] 為 0..N `ToolResult`
- `result` → 抽 `usage` 物件為 1 `Usage`（含 `input_tokens` / `output_tokens` / `cache_read_input_tokens` / `cache_creation_input_tokens` + verbatim `extras`）
- `system` / `rate_limit_event` / unknown → 回 empty vec（forward-compat）

malformed JSON → return empty vec（不報錯，stream 末尾常有不完整行）。

**Alternative considered**：寫 v3 自己的 parser。Drop — v2 parser 經過 spike-verified 對 claude CLI 2.1.x，重寫沒收益、徒增 bug 機會。

### Sync stream consumption 用 `BufReader::lines()` + 兩條 thread

`std::process::Command::spawn()` + `Stdio::piped()` 後拿 `child.stdout.take().unwrap()` 包 `BufReader::new(...)`，呼叫 `.lines()` 得 line iterator。每行：
1. `parse_claude_stream_line(&line)` → `Vec<StreamEvent>`
2. 對 `Usage` event accumulate 進本地 `TokenUsage`
3. 對其他 event 呼叫 `render::stream_event::print_event(event, render_opts)` 印到 stdout
4. 持續直到 EOF

**stderr 處理**：spawn 時也 `Stdio::piped()` stderr，spawn 一條 background thread 跑 `io::copy(child.stderr.take().unwrap(), io::stderr().lock())` — agent 自己的錯誤訊息（network fail、auth fail 等）原樣 passthrough 給 user 看；codebus 不解讀。Thread join 在 `child.wait()` 後做（best-effort，5s timeout 後 detach）。

**Alternative considered**：tokio async `Stream`。Drop — sync 行為一致、依賴更輕（v3 已經很 lean）。

**Alternative considered**：用 `read_to_string` 一次讀完再 parse。Drop — 5-30 分鐘的 stream 全 buffer 進記憶體、user 也得等到 EOF 才看到任何 thought event，違背即時可視化目的。

### `claude_cli::invoke` 簽章破壞性改動

```rust
// 現行（v3-config 版）
pub fn invoke(opts: InvokeAgentOptions) -> io::Result<ExitStatus>;

// v3-run-log 版
pub struct InvokeReport {
    pub exit: ExitStatus,
    pub accumulated_tokens: TokenUsage,
    pub started_at: String,    // RFC 3339 UTC
    pub finished_at: String,   // RFC 3339 UTC
}
pub fn invoke(opts: InvokeAgentOptions, render_opts: &RenderOptions) -> io::Result<InvokeReport>;
```

新增 `render_opts` 參數因為 stream rendering 需要 emoji / color / TTY 偵測（從 caller pass 進來，不在 invoke 內 detect — `Detection runs once per process` 原則）。

caller（goal / query / fix）拿 `InvokeReport` 後組 `RunLog`：
```rust
RunLog {
    goal: opts.text.clone(),  // or "" for fix
    mode: "goal" | "query" | "fix",
    model: cc_cfg.<verb>.model,
    effort: cc_cfg.<verb>.effort,
    started_at: report.started_at,
    finished_at: report.finished_at,
    tokens: report.accumulated_tokens,
    wiki_changed: ...,         // git diff check
    lint_error_count: ...,     // from final lint
    lint_warn_count: ...,
}
```

### `LogSink` trait + `JsonlSink` / `NullSink` carry from v2

trait 形狀同 v2：
```rust
pub trait LogSink: Send + Sync {
    fn name(&self) -> &str;
    fn write_run(&mut self, entry: &RunLog) -> Result<(), LogError>;
    fn flush(&mut self) -> Result<(), LogError> { Ok(()) }
}
```

兩 impl：
- `NullSink` — `write_run` 直接 Ok(())、user opt-out
- `JsonlSink::new(dir)` — `<dir>/runs-YYYY-MM-DD.jsonl` 每行一個 `RunLog` JSON、append-only、create_dir_all 在第一次 write 時做

date 用 `started_at[..10]` 切（RFC 3339 開頭 10 chars 永遠是 `YYYY-MM-DD`），保證跨午夜的 long-running 不會 split file。

`fs::OpenOptions::new().append(true).create(true).open(...)` POSIX 上 line-wise atomic、Windows 上 best-effort（v2 已驗證 acceptable）。

### `log` config section 直接走 `pii.scanner` 同 pattern

```yaml
# ~/.codebus/config.yaml
log:
  sink: jsonl    # default; 或 "null"
  dir: ~/path    # optional override
```

scalar tag 風格、與 v3-config 既存 `pii.scanner` / `claude_code.<verb>` 模式一致。default `sink: jsonl, dir: None` → caller resolve None 為 `<vault>/.codebus/log/`。`sink: "null"` 顯式 opt-out（YAML quote 必要 — v3-config 已踩過 `null` literal 的雷）。

### Render module extension：`render/stream_event.rs`

新檔，與既有 `banner.rs` / `lint_text.rs` 同層。
```rust
pub fn format_event(event: &StreamEvent, opts: &RenderOptions) -> String;
pub fn print_event(event: &StreamEvent, opts: &RenderOptions);
```

對 4 種 event：
- `Thought { text }` → `🤔 [Agent 思考]\n  <indented text>`（emoji-on）/ `◆ [Agent 思考]\n  <text>`（emoji-off）
- `ToolUse { name: "Write" | "Edit", input.file_path }` → `✍️ [正在生成]\n  <file_path>` / `+ [正在生成]\n  <path>`
- `ToolUse { name: other, input }` → `🛠 [呼叫工具]\n  <name>(<input summarized>)` / `→ [呼叫工具]\n  ...`
- `ToolResult { output, is_error }` → `👀 [觀察結果]\n  <truncated body>`，`output > 200 chars` 截斷加 `…`，`output` matches "<file>: <N>L" 形式時改顯示 `(N lines)`，Write 成功 echo（`File created successfully...`）抑制返回空字串
- `Usage` → 不渲染（caller 累計、寫 RunLog 用）

**Alternative considered**：把 stream event render 直接寫進 `claude_cli::invoke` 內。Drop — 違反「render 模組統一管」原則；invoke 應該只做 spawn + stream + collect。

### Output 順序：banner 與 stream event 共存

```
[codebus banner: 🚌 Start]
[codebus banner: 🎯 Goal]            (goal only)
[codebus banner: ~ SyncStart]        (re-sync 時)
[codebus banner: ok SyncDone]
[codebus banner: ! PiiSummary]
─── invoke() 內部 stream loop ───
[stream event: 🤔 Thought]
[stream event: 🛠 ToolUse(Read)]
[stream event: 👀 ToolResult]
[stream event: 🤔 Thought]
[stream event: ✍️ Write file]
[stream event: 👀 ToolResult]
... N cycles ...
─── invoke() 結束、回 InvokeReport ───
[codebus banner: ~ LintStart]
[codebus banner: ok LintDone]
[codebus banner: . CommitDone]
[codebus banner: 🎉 Done]
─── caller 寫 RunLog ───
[silent — JsonlSink append]
```

stream event 由 `invoke()` 自行 `print_event` 印（不然 caller 要 callback）。banner 由 cli command function 印（既有行為）。兩者都用同一 `RenderOptions`，emoji / color / 非 TTY 邏輯一致。

### Init 不需新增 mkdir step

`vault::layout::create_vault_layout` 已建 `<vault>/.codebus/log/`（v3 既有，命名是 singular 不是 plural）。但 `init.rs` 的 `INTERNAL_GITIGNORE_LINES` 寫的是 `logs/`（plural）— 這條 line 失效（git 比對 literal）。本 change 順手把 line 改 `log/` 對齊磁碟實際命名。

### Mock claude 需要 stream-json behavior

既有 `tests/bins/mock_claude.rs` 用 plain text。本 change 加新 behavior：
- `success-stream-json` — 寫 4 條 stream-json line: `{type:"system",...}`（被 parser skip）+ `{type:"assistant", message:{content:[{type:"text",text:"思考中..."}]}}` + `{type:"assistant", ..."tool_use", name:"Read", input:{file_path:"/x"}}` + `{type:"user", ..."tool_result", content:"file contents"}` + `{type:"result", usage:{input_tokens:100, output_tokens:50, cache_read_input_tokens:10}}`
- `failure-stream-json` — 同上但少 result line + exit 1，模擬 mid-stream 中斷
- 既有 behaviors（`success-noop` / `success-write-page` / `failure-write-then-exit-1`）保留供 v3-render-polish / v3-config / v3-fix-trust-agent 等舊 test 用 — 那些 test spawn 仍要 work（mock 同時支援兩種 output format，依 behavior 切）

## Implementation Contract

#### Behavior

##### `claude_cli::invoke` 新行為

- spawn argv 多 `--output-format stream-json --verbose --input-format stream-json` 三個 flag（無條件加；舊 text mode 路徑廢棄）
- stdout / stderr 改 `Stdio::piped()`
- 主 thread loop on `BufReader::new(stdout).lines()`：
  - 解 line → events → stream events 印到 stdout、Usage events accumulate
  - non-UTF-8 line（從 `read_until(b'\n') + from_utf8_lossy`）→ 當作 malformed JSON 處理（empty events vec）
- background thread `io::copy(stderr, std::io::stderr())` passthrough agent 錯誤訊息
- `child.wait()` 後 join stderr thread（5s timeout，超時 detach）
- 回 `InvokeReport { exit, accumulated_tokens, started_at, finished_at }`

##### `RunLog` 寫入時機

每個 verb（goal / query / fix）的 successful path 結尾、Done banner **之前**：
- 從 `InvokeReport` 拿 `accumulated_tokens` / `started_at` / `finished_at`
- 收集 `wiki_changed`（`git -C <vault> diff --quiet HEAD~1` exit code）/ lint counts（從 fix phase report 或 standalone post-spawn lint）
- 組 `RunLog` → `log_sink.write_run(&entry)`
- 寫失敗 → 印 `eprintln!("warning: run-log write failed (non-fatal): {e}")`、不 propagate

verb 失敗路徑（spawn fail / agent panic）→ 仍寫 RunLog（記載 partial state）— 失敗也是 history 一部分。

##### `~/.codebus/config.yaml log:` section schema

```yaml
log:
  sink: jsonl    # values: "jsonl" (default) | "null" (opt-out)
  dir: <path>    # optional; default <vault>/.codebus/log/; tilde expansion supported
```

missing file / missing section / missing field → default `{ sink: jsonl, dir: None }`。`dir: None` resolve 為 `<vault>/.codebus/log/`（caller 處理）。`sink: "null"` 顯式 string（YAML reserved word foot-gun — 同 `pii.scanner: none`，但 `null` 在 sink 場景對齊 v2 命名習慣，採 string-quoted）。

#### Interface / data shape

```rust
// codebus-core/src/stream/parser.rs
pub enum StreamEvent {
    Thought { text: String },
    ToolUse { name: String, input: serde_json::Value },
    ToolResult { output: String, is_error: bool },
    Usage(TokenUsage),
}
pub fn parse_claude_stream_line(raw: &str) -> Vec<StreamEvent>;

// codebus-core/src/log/sink.rs
pub struct TokenUsage {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_read_tokens: Option<u64>,
    pub cache_write_tokens: Option<u64>,
    pub reasoning_tokens: Option<u64>,
    pub extras: serde_json::Value,
}
pub struct RunLog {
    pub goal: String,
    pub mode: String,             // "goal" | "query" | "fix"
    pub model: Option<String>,
    pub effort: Option<String>,
    pub started_at: String,       // RFC 3339 UTC
    pub finished_at: String,
    pub tokens: TokenUsage,
    pub wiki_changed: bool,
    pub lint_error_count: usize,
    pub lint_warn_count: usize,
}
pub trait LogSink: Send + Sync {
    fn name(&self) -> &str;
    fn write_run(&mut self, entry: &RunLog) -> Result<(), LogError>;
    fn flush(&mut self) -> Result<(), LogError> { Ok(()) }
}
pub fn accumulate_token_usage(acc: &mut TokenUsage, addend: &TokenUsage);

// codebus-core/src/log/factory.rs
pub enum SinkConfig { Null {}, Jsonl { dir: Option<PathBuf> } }
pub fn build_sink(cfg: SinkConfig) -> Result<Box<dyn LogSink>, SinkError>;

// codebus-core/src/config/log.rs
pub struct LogConfig { pub sink: SinkConfig }
pub fn load_log_config(path: &Path) -> Result<LogConfig, ConfigLoadError>;

// codebus-core/src/render/stream_event.rs
pub fn format_event(event: &StreamEvent, opts: &RenderOptions) -> String;
pub fn print_event(event: &StreamEvent, opts: &RenderOptions);

// codebus-core/src/agent/claude_cli.rs (modified)
pub struct InvokeReport {
    pub exit: ExitStatus,
    pub accumulated_tokens: TokenUsage,
    pub started_at: String,
    pub finished_at: String,
}
pub fn invoke(opts: InvokeAgentOptions, render_opts: &RenderOptions) -> io::Result<InvokeReport>;
```

#### Failure modes

- **Stream parse error**（malformed JSON line）→ parser 回 empty vec、loop 繼續、不 fail spawn
- **Non-UTF-8 stdout byte**（claude 不該產生但防禦性）→ `from_utf8_lossy` 替換、parser 收到不正常字元時 JSON parse 仍會 fail（→ empty vec）
- **stderr passthrough thread panic** → 不影響主 stream loop；child 仍跑完；exit code 仍正確收集
- **agent crash mid-stream**（pipe close）→ stdout BufReader 讀到 EOF 退 loop、`child.wait()` 取得非零 exit code；已累計的 partial token 仍寫進 RunLog
- **JsonlSink 寫入失敗**（disk full / permission denied）→ `eprintln!("warning: run-log write failed: {e}")`、不 abort verb 的 exit code（log 是 best-effort）
- **`<vault>/.codebus/log/` dir 不存在**（init 沒跑或 user 砍掉）→ JsonlSink lazy `create_dir_all` 在第一次 write 時做；create_dir_all 失敗則同 disk-full path
- **config `log.sink` 未知值** → serde 報錯 → `load_log_config` 回 `Err` → caller `eprintln!("warning: log config: ...")` + 用 default

#### Acceptance criteria

- `cargo test -p codebus-core --lib stream::parser` — port 自 v2 的 12 條 unit test 全綠
- `cargo test -p codebus-core --lib log::` — `JsonlSink` (date rotation, append, parent mkdir) + `NullSink` (no-op) + `accumulate_token_usage` (Some/None combine, saturating_add) — 共 ~10 test
- `cargo test -p codebus-core --lib config::log::` — load default / load `sink: null` / load explicit `dir` / unknown sink rejected — 共 4 test
- `cargo test -p codebus-core --lib render::stream_event::` — Thought emoji on/off / ToolUse Write 特化 / ToolResult truncation 200 chars / Write success echo 抑制 / Usage no-render — 共 ~6 test
- `cargo test -p codebus-cli --test goal_flow` — 用 mock_claude `success-stream-json` behavior，跑 goal 後驗：(a) stdout 含 `🤔 [Agent 思考]` 跟 `🛠 [呼叫工具]` line（subprocess pipe 是非 TTY 所以實際是 ASCII fallback `◆` `→`）；(b) `<vault>/.codebus/log/runs-YYYY-MM-DD.jsonl` 存在且含 1 條 JSON 帶 `tokens.input_tokens=100` 等
- `cargo test -p codebus-cli --test query_flow` — 同上、mode 是 `"query"`
- `cargo test -p codebus-cli --test fix_flow` — 同上、mode 是 `"fix"`，加 `lint_error_count` / `lint_warn_count` 反映 final lint state
- `cargo test --workspace` 全綠（318 既有 + 新增 ~30 = ~350）
- 手動 CLI 驗證 — 對 D:/side_project/uv：
  1. 跑 `codebus goal "describe uv-cli entrypoint"` — terminal 看到 emoji thought / tool / result events 即時 stream（不再黑盒 5 分鐘）
  2. `cat D:/side_project/uv/.codebus/log/runs-2026-MM-DD.jsonl | jq .tokens` — 看到 input/output/cache token counts
  3. 設 `~/.codebus/config.yaml log: { sink: "null" }` 重跑 → jsonl 不再 append
  4. 故意 plant 壞 .gitignore lock 在 .codebus/log/ → JsonlSink 寫失敗 → stderr warn + verb exit 0
- spec 驗證 — `spectra validate v3-run-log` clean、`spectra analyze v3-run-log --json` 無 Critical / Warning

#### Scope boundaries

**In scope**:

- 全部新建檔案（stream / log / log/sinks / log/factory / config/log / render/stream_event）的完整實作 + tests
- `claude_cli::invoke` 破壞性 signature 改 + stdio 架構切換
- goal / query / fix 三 verb 的 stream + RunLog 整合
- mock_claude stream-json behavior 新增（既有 behavior 不變）
- 既有 internal `.gitignore` `logs/` → `log/` 修正（單字 typo 等級）

**Out of scope**:

- markdown_style for thought text（v2 有；v3 skip）
- OSC 8 wikilink wrap in thought events（需要 slug_index）
- OTel sink（feature flag + dep 太重）
- LlmProvider trait 抽象（single-impl）
- log retention / rotation policy（除 date-based file split 外無刪除機制）— v2 也沒、留給 user 自己 cron
- per-event log（runs.jsonl 是 per-run；events.jsonl 之類更細粒度未來再說）
- async runtime / tokio
- multi-process atomic append guarantee（best-effort）

## Risks / Trade-offs

- **stdio piped 壓力測試** → claude `--verbose` 在大型 repo 跑 goal 可能單秒幾十 MB stream-json；BufReader sync 讀沒 backpressure 問題（read 自然 throttle child write），但要確認 stderr thread 不會跟主 thread 競爭 stdout lock 印爛畫面 → mitigation：stdout 印走 `println!` (Mutex-protected)，stderr passthrough 用 `io::copy(stderr, stderr)`，兩條 stream 不交叉
- **stream rendering 改變 user 對 default mode 的觀感** → v3.0.0 user 看到的是 claude 默認文字輸出（中段乾淨）；本 change 後變成 codebus 渲染的 thought / tool / result 流（更熱鬧、更工程感）— **BREAKING UX 變化**。Mitigation：proposal 標 BREAKING；user 可改用 `claude` 直接跑（不過 codebus 工作流就 bypass 了）
- **mock_claude 行為分裂** → 一個 mock binary 同時要支援既有 plain-text behaviors AND 新 stream-json behaviors（既有 v3-render-polish / v3-config 等 test 不能 break）。Mitigation：依 `CODEBUS_MOCK_BEHAVIOR` env 切；新 behavior 走 stream-json、舊 behavior 走原樣
- **RunLog 寫失敗 silent fallback** → user 設 `log.sink: jsonl, dir: /var/log/...` 但 disk full → stderr warn 但 verb 不 fail。可能讓 user 以為有 log 但其實沒寫。Mitigation：warn message 明確提示「fallback to no-op for this run」；docs 寫 log 是 best-effort
- **既有 mock-based test 要全 retrofit** → 4 個 flow_test files 都要讀 mock log + 加 stream behavior 路徑。Mitigation：分 task 漸進；retrofit 期間部分 test 失敗 OK，到 task 整批做完再驗 workspace 綠

## Migration Plan

無 schema migration（純 additive 配 stdio architecture change）。BREAKING 範圍限：

1. **stdio 架構**：`Stdio::inherit()` → `Stdio::piped()` + 自渲染。User 看到的中段 output 形式改變（從 claude default text → codebus stream events）
2. **claude_cli::invoke signature**：caller 改用 `InvokeReport` 取代 `ExitStatus`。所有 in-tree caller（goal / query / fix / fix mod）同步更新；無 external API consumer

無回滾步驟 — config additive、新模組可被忽略（不啟用 jsonl sink 仍可跑、stream rendering 是必經路徑沒辦法跳過 — 但 user 可 `--debug` 看更多細節，default mode 仍 emoji-rich）。

## Open Questions

無；discuss 階段 + design 階段已收斂所有 design choice。
