## Context

`agent::invoke`（`codebus-core/src/agent/claude_cli.rs`）是所有 verb 共用、provider-agnostic 的 spawn / 串流 / 收尾迴圈。它目前：

- 用阻塞 `BufReader::lines()` 在主執行緒讀 child stdout；child 不再吐 stdout 時這個讀會無限期阻塞。
- 背景 watcher thread 每 100ms 輪詢 `cancel` 與 `done` 兩個 flag；觀察到 `cancel` 就呼叫 `KillHandle::terminate_tree()`（Windows Job Object KILL_ON_JOB_CLOSE / Unix killpg），child 樹被收掉後 stdout EOF 解除主迴圈阻塞。
- `started_at` 只是回報用的 RFC 3339 字串，沒有任何 wall-clock 計時器。
- 回傳 `InvokeReport { exit, accumulated_tokens, started_at, finished_at, session_id }`。

outcome 由各 verb 自行從 `invoke_report.exit.code()` 派生（`goal.rs` / `query.rs` / `fix.rs` / `chat.rs` / `quiz.rs` 同模式），寫進 `RunLog`。`RunLog` / `InterruptReason` 定義在 `codebus-core/src/log/sink.rs`，schema 由 `run-log` capability 規範。

codex 串流由 `codebus-core/src/stream/codex_parser.rs` 解析：`item.completed` 的 `command_execution` 會把 `aggregated_output`（stderr 已折入）與 `exit_code != 0` 解成 `StreamEvent::ToolResult { output, is_error }`，但這個 `is_error` 只進 render、沒回饋 outcome。

**Part B 的關鍵實證（PoC 0.135.0，`agent-cli-research/poc/codex-sandbox/write-acl-run/`）**：codex `exec` 內層 PowerShell 寫入被 normal-ACL 擋下時，`item.completed.exit_code = 1` / `status = "failed"`，但頂層 codex process exit 0（`summary.json` 兩種結果都 `exit_code: 0`）。更關鍵：在此 zh-TW 機器上，`aggregated_output` 的人類可讀訊息是「拒絕存取路徑 …」而**非**英文「Access is denied」；但同一段輸出同時含 locale-independent 標記 `PermissionDenied`（完整）與 `GetContentWriterUnauthorizedAccessError`（完整），而 `UnauthorizedAccessException` 被換行切成「Unauthorized」+ 空白 +「AccessException」不可靠。

## Goals / Non-Goals

**Goals:**

- 在 `agent::invoke` 加一個可選的 per-run wall-clock 上限作為 hang/無人值守兜底，重用既有 tree-kill，預設關閉時行為與現狀 byte-equivalent。
- 讓「codex 頂層 exit 0 但內層其實被 sandbox 擋」這件事可被觀測（durable 記在 RunLog + 即時 stderr warning），且**絕不誤報**（正常 grep-no-match 不得被當成 denial）。
- timeout 命中時 outcome / interrupt_reason 正確且可區分於 cancel 與一般失敗。

**Non-Goals:**

- **不自動把 denial 訊號翻成 `outcome = "failed"`**。MVP 只做可觀測性；`sandbox_denial_count` 與 `outcome` 正交（如同 `interrupt_reason`）。只有未來在實機驗證偵測精度後，才考慮讓它影響 outcome。明確延後。
- **不處理 claude 路徑的 `permission_denials`**。那是另一個語意（claude 的工具權限拒絕），不在本 change。
- **不保證 macOS / Linux 的 denial marker 涵蓋完整**。curated marker 以 PoC（Windows）實證 + 常見 Unix `strerror`（`Permission denied` / `Operation not permitted`）為主，跨平台為 best-effort，寧可少報。
- **不做 per-tool / per-command 的 timeout**，只做 per-run wall-clock。
- **不改既有 cancel 機制**，timeout 重用其 `KillHandle` 與 watcher。
- **不做投機抽象**：不把 `cancel` + `timeout` 包成新的 control struct（會擴大既有 cancel 程式的 blast radius），只加並列的可選參數。

## Decisions

### D1 — timeout 走 `invoke` 的新參數，與 `cancel` 並列

`agent::invoke` 新增 `timeout: Option<Duration>`，與 `cancel: Option<Arc<AtomicBool>>` 並列。兩者都是 caller 注入的 runtime 控制信號，不屬於 provider/argv 的 `SpawnSpec`。理由：沿用既有 cancel 的注入模式（library 不讀 config），且 additive 不動既有 cancel 簽章內容。

### D2 — watcher 第三分支用 `Instant`，主迴圈阻塞由 EOF 解除

`invoke` 在 spawn 前後 `started_at` 旁邊另捕捉一個 `Instant`（RFC 3339 字串不能拿來算 elapsed）。把 `(start_instant, timeout)` 傳進 watcher；watcher 迴圈在既有 `done` / `cancel` 之後加第三個檢查：`timeout` 為 `Some(limit)` 且 `start_instant.elapsed() > limit` 時呼叫 `kill_handle.terminate_tree()`，設一個共享 `timed_out` 旗標後 return。tree-kill 造成 stdout EOF，主迴圈的 `BufReader::lines()` 解除阻塞（與 cancel 完全同機制）。watcher 仍在 `invoke` 返回前被 `join`，不會對回收的 PID 誤殺。

### D3 — `InvokeReport.timed_out: bool` 作為 verb 的判定依據

timeout-kill 後的 `exit` 與任何 kill 無法分辨，故 `invoke` 必須顯式回報。watcher 設定的 `timed_out` 旗標（`Arc<AtomicBool>`）在 `invoke` 收尾時讀出寫進 `InvokeReport.timed_out`。

### D4 — outcome 派生：cancel 優先於 timeout 優先於 exit code

各 verb 在 `invoke` 返回後依序判定：

1. `cancel` flag 已 true 時 → `outcome = "cancelled"` + `interrupt_reason = Some(UserCancel)`（既有路徑，不變）。
2. 否則 `invoke_report.timed_out` 為 true 時 → `outcome = "failed"` + `interrupt_reason = Some(Timeout)`。
3. 否則沿用既有 exit-code 派生。

cancel 是 user 主動意圖，語意上應蓋過 timeout。

### D5 — `InterruptReason::Timeout` 用具名變體，不用 `Other("timeout")`

timeout 是刻意設計、與 `UserCancel` / `NetworkDrop` 同位階的終止原因，理應具名（`Other` 是「尚未升格」的 fallback）。serde `rename_all = "kebab-case"` 產生 `"timeout"`。需同步 `run-log` capability：列舉、序列化規則、序列化形狀範例表、round-trip scenario。

### D6 — config：新 `lifecycle` namespace，`run_timeout_secs`，預設 None

新增 `codebus-core/src/config/lifecycle.rs`，鏡像 `config/goal.rs` 的 loader / Default / forward-compat 模式。schema：

```yaml
lifecycle:
  run_timeout_secs: 1800   # 整數秒；缺省 = 無上限
```

key 命名：grep 過 config 既無 `lifecycle` 也無任何 duration key；選 `run_timeout_secs`（複數 `secs` 對齊 Rust `Duration::from_secs`，全小寫 snake_case 對齊既有 `content_verify` / `lint_error_count`）。預設（檔案缺 / section 缺 / 欄位缺）解析為 `None`，即不限、與現狀 byte-equivalent。型別錯產生 `YamlParse`，caller warn-and-default 到 `None`。caller（CLI / app）把 `Option<u64>` 秒數轉成 `Option<Duration>` 注入 `run_*`。

### D7 — Part B 偵測器：只掃 is_error 結果、比對 curated locale-independent marker

新增 `codebus-core/src/stream/sandbox_signal.rs`，純函式 `fn is_sandbox_denial(output: &str) -> bool`：對輸出做 case-insensitive 比對一組高特異性、跨語系穩定的權限拒絕標記。初始 marker（apply 時對 PoC fixture 校準確認）：

- `Access is denied`（Windows 英文 locale / 通用）
- `PermissionDenied`（PowerShell `CategoryInfo`，zh-TW PoC 命中）
- `UnauthorizedAccessError`（`FullyQualifiedErrorId` 片段，zh-TW PoC 命中；**刻意不用** `UnauthorizedAccessException`，因會被換行切斷）
- `Permission denied`（Unix EACCES `strerror` 英文）
- `Operation not permitted`（Unix EPERM `strerror` 英文）

只有 `is_error == true` 的 `ToolResult` 才送進偵測；偵測命中才計數。這給高精度（grep-no-match 是 exit 1 但輸出無上述標記，故不計）換取較低召回（純 localized、且不含任何 .NET/errno 標記的訊息會漏；列為 documented limitation，符合「寧可少報」）。

### D8 — 累計在 `invoke`，surface 在 `InvokeReport` + `RunLog` + stderr

`invoke` 主迴圈在既有 token 累計處旁邊，對每個 `StreamEvent::ToolResult { output, is_error }` 呼叫偵測器並累加 `sandbox_denial_count`，寫進 `InvokeReport`。偵測本身 provider-agnostic（claude 也產 `ToolResult`），但語意上幾乎只有 codex 的 raw shell 會吐這些 OS 權限字串；claude 的 `permission_denials` 明確不在此範圍。各 verb 把 `invoke_report.sandbox_denial_count` 寫進 `RunLog.sandbox_denial_count`，並在大於 0 時印一行 `warning: sandbox-denial: N ...` 到 stderr（parent-side `eprintln!`，不受 child stderr sink 影響）。`RunLog.sandbox_denial_count` 用 `#[serde(default)]` 加「為 0 時略過序列化」的 helper，使既有非 codex / 乾淨 codex run 的 jsonl 維持 byte-identical，legacy row 缺欄位乾淨還原為 0。

## Implementation Contract

**Behavior：**

- 設定 `lifecycle.run_timeout_secs: N` 後，任何 verb 的單次 agent run 超過 N 秒會被連同子孫程序一起收掉，該 run 記為 `outcome = "failed"` + `interrupt_reason = "timeout"`。未設定（預設）時行為與本 change 前完全一致。
- 當 codex 頂層 exit 0 但內層有被 sandbox 擋的指令（且輸出含可辨識標記）時，該 run 的 RunLog 會帶 `sandbox_denial_count` 大於 0，且 run 進行中 stderr 會有一行 `warning: sandbox-denial`。outcome 不因此改變（仍照 exit code）。
- 正常 grep-no-match（exit 1、無權限標記）不計入 `sandbox_denial_count`。

**Interface / data shape：**

- `agent::invoke` 簽章新增尾端參數 `timeout: Option<Duration>`。
- `InvokeReport` 新增 `timed_out: bool` 與 `sandbox_denial_count: usize`。
- 每個 `run_*` 簽章新增 `timeout: Option<Duration>` 參數（位置與既有 `cancel` 並列）。
- `InterruptReason` 新增 `Timeout` 變體，序列化為字串 `"timeout"`。
- `RunLog` 新增 `sandbox_denial_count: usize`，serde `default`、為 0 時略過序列化。
- config：top-level `lifecycle` section，欄位 `run_timeout_secs: u64`（選填）。loader `load_lifecycle_config(path) -> Result<LifecycleConfig, ConfigLoadError>`，`LifecycleConfig { run_timeout_secs: Option<u64> }`。
- 新純函式 `stream::sandbox_signal::is_sandbox_denial(output: &str) -> bool`。

**Failure modes：**

- `terminate_tree()` 失敗為 best-effort 忽略（與 cancel 路徑一致）。
- config 型別錯 / 結構壞產生 `ConfigLoadError::YamlParse`，caller warn-and-default 到 `None`（不限），絕不因 config 壞而靜默縮短或拉長 timeout。
- `RunLog` 寫入失敗仍為非致命（既有 `warning: run-log` 契約不變）。
- denial 偵測為 best-effort 觀測：漏報（localized-only 訊息）是可接受的設計取捨；誤報（把正常失敗當 denial）不可接受、由 negative test 守門。

**Acceptance criteria：**

- Part A：以 fake「慢 spawn」backend 的單元測試證明 — timeout 命中會呼叫 `terminate_tree` 並讓 `invoke` 在遠早於 child 自然結束前返回；`InvokeReport.timed_out == true`；對應 verb 派生出 `outcome == "failed"` + `interrupt_reason == Some(Timeout)`；`timeout: None` 時迴圈行為與現狀一致（既有 cancel/finite 測試不退步）。
- Part B（守門）：餵含 PoC denial 輸出的 codex stream 時 → `sandbox_denial_count == 1` 且 stderr 有 `warning: sandbox-denial`；**餵正常 grep-no-match（exit 1、輸出無權限標記）時 → `sandbox_denial_count == 0`、outcome 不變**（此 negative test 是 Part B 的關鍵守門）。
- `InterruptReason::Timeout` round-trip 與 kebab-case 序列化 scenario 通過；legacy jsonl 缺 `sandbox_denial_count` 乾淨還原為 0。
- `cargo test` 全綠；`spectra validate` 通過。

**Scope boundaries：**

- In scope：`agent::invoke` timeout 機制與 denial 累計、`InvokeReport` / `RunLog` / `InterruptReason` schema、五個 `run_*` 的下傳與派生、`lifecycle` config loader、`sandbox_signal` 偵測器、CLI（5 個 command）與 app IPC（3 個）caller 注入、`verb-library` 與 `run-log` spec delta。
- Out of scope：claude `permission_denials`、denial 自動翻 outcome、前端 UI 顯示 `sandbox_denial_count`（RunLog 已落欄位，前端呈現另案）、macOS/Linux marker 完整驗證、per-command timeout。

## Risks / Trade-offs

- **denial 偵測召回不完整**：純 localized 且不含 .NET/errno 標記的拒絕會漏報。取捨明確偏向「寧可少報、不要誤報」，因為誤報會污染 outcome 信任。marker 清單於 apply 時對 PoC fixture 校準，且留有後續擴充空間。
- **provider-agnostic 偵測器跑在 claude 路徑**：claude 的 `ToolResult` 也會過偵測器，理論上若 claude 工具輸出剛好含「Access is denied」會被計入。實務上 claude 的受控工具（Read/Glob/Grep/Write/Edit）幾乎不會吐 raw OS ACL 文字；可接受，且 stderr warning 會讓任何意外可見。
- **timeout 與 cancel 競態**：兩者都可能同時觸發 tree-kill；`terminate_tree` 既已 idempotent，watcher 在 return 前被 join，無安全問題。outcome 由 D4 的優先序決定（cancel 蓋過 timeout）。
- **blast radius**：新增 `RunLog` 欄位會使所有 `RunLog { .. }` 字面量（含跨檔測試）需補欄位；這與先前 `interrupt_reason` 落欄位時的成本同型、codebus 已接受此模式，屬機械式修改。
- **change 偏大**（跨 verb-library + run-log 兩 capability、code 觸及約 17 檔）。但兩部分共用同一條 `invoke 至 outcome` seam，拆開反而割裂該 seam 的一致性；維持單一 cohesive change。
