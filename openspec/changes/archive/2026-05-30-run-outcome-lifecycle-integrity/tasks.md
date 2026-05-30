# Tasks — run-outcome-lifecycle-integrity

行號一律於實作時自行 grep 重驗；以下任務以「行為 + 驗證目標」描述，檔案路徑僅為定位。

## 1. Schema 基礎（`codebus-core/src/log/sink.rs` + 全 repo RunLog 字面量）

> 對應 spec：run-log `Requirement: RunLog Schema and Per-Invocation Capture`（`InterruptReason::Timeout` + `sandbox_denial_count` 欄位）。設計：D5、D8。

- [x] [P] 先寫測試（sink.rs `#[cfg(test)]`）：`InterruptReason::Timeout` 序列化為 `"interrupt_reason":"timeout"` 且 round-trip 相等；`RunLog.sandbox_denial_count == 0` 時序列化略過該欄、為 `2` 時含 `"sandbox_denial_count":2`；缺 `sandbox_denial_count` 的 legacy jsonl row 反序列化為 `0`。確認測試 RED。
- [x] [P] 在 `InterruptReason` enum 新增 `Timeout` 具名變體（沿用既有 `#[serde(rename_all = "kebab-case")]`，產生 `"timeout"`）。
- [x] [P] 在 `RunLog` struct 新增 `sandbox_denial_count: usize`，加 `#[serde(default, skip_serializing_if = "<zero-check helper>")]`；新增 `fn is_zero(n: &usize) -> bool` helper（或等價）使值為 0 時略過序列化。
- [x] [P] 全 repo grep 補齊每一處 `RunLog { .. }` 字面量（goal/query/fix/chat/quiz.rs + 各 `#[cfg(test)]` + sink.rs 測試 fixture）新增 `sandbox_denial_count: 0`，使 crate 重新編譯通過。
- [x] [P] 跑 sink.rs 測試轉 GREEN。

## 2. config `lifecycle` namespace（`codebus-core/src/config/lifecycle.rs` + `config/mod.rs`）

> 對應 spec：verb-library `Requirement: Run Wall-Clock Timeout Safety Net`（config schema 與 caller 注入段）。設計：D6。

- [x] [P] 先寫測試（lifecycle.rs `#[cfg(test)]`，鏡像 `config/goal.rs` 測試結構）：檔案缺 / `lifecycle` section 缺 / `run_timeout_secs` 欄缺 → `None`；`lifecycle:\n  run_timeout_secs: 1800` → `Some(1800)`；`run_timeout_secs: 0` → `None`（正規化為不限、非 zero-duration 瞬殺）；型別錯（`run_timeout_secs: not-a-number`）→ `Err(ConfigLoadError::YamlParse)`；未知子鍵被靜默忽略。確認 RED。
- [x] [P] 新增 `config/lifecycle.rs`：`LifecycleConfig { run_timeout_secs: Option<u64> }` + `Default`（`None`）+ `load_lifecycle_config(path) -> Result<LifecycleConfig, ConfigLoadError>`，forward-compat tolerance 與 `goal.rs` 一致。
- [x] [P] 在 `config/mod.rs` 新增 `pub mod lifecycle;` 與 `pub use lifecycle::{LifecycleConfig, load_lifecycle_config};`。
- [x] [P] 跑測試轉 GREEN。

## 3. sandbox-denial 偵測器（`codebus-core/src/stream/sandbox_signal.rs` + `stream/mod.rs`）

> 對應 spec：verb-library `Requirement: Sandbox Denial Signal Observability`（偵測器 + curated marker 段）。設計：D7。

- [x] [P] 先寫測試（sandbox_signal.rs `#[cfg(test)]`）：以 PoC `write_normal_acl.jsonl` 的 `aggregated_output` 文字（含 zh-TW「拒絕存取」+ `PermissionDenied` + `UnauthorizedAccessError`）→ `is_sandbox_denial` 回 `true`；正常 grep-no-match 樣本（如 `""` 或一般非權限錯誤文字）→ 回 `false`；確認 marker 比對 case-insensitive；**刻意驗證** `UnauthorizedAccessException` 被換行切斷時不靠它命中（改靠 `PermissionDenied`）。確認 RED。
- [x] [P] 實作 `pub fn is_sandbox_denial(output: &str) -> bool`：對 curated locale-independent marker set（`Access is denied` / `PermissionDenied` / `UnauthorizedAccessError` / `Permission denied` / `Operation not permitted`）做 case-insensitive substring 比對；marker set 以常數陣列定義並加註各 marker 來源與「高精度低召回、寧可少報」的設計理由。
- [x] [P] 在 `stream/mod.rs` 匯出 `sandbox_signal` 模組與 `is_sandbox_denial`。
- [x] [P] 跑測試轉 GREEN。

## 4. invoke timeout 機制（`codebus-core/src/agent/claude_cli.rs`）

> 對應 spec：verb-library `Requirement: Run Wall-Clock Timeout Safety Net`（invoke 機制段）。設計：D1、D2、D3。

- [x] 先寫測試（claude_cli.rs `#[cfg(test)]`，重用既有 `TestBackend` 的 `Silent` 慢 spawn）：`timeout: Some(短 limit)` → `invoke` 遠早於 child 自然結束返回、`InvokeReport.exit.success() == false`、`InvokeReport.timed_out == true`；`timeout: None` 對 `Finite` child → 正常返回、`timed_out == false`（既有 cancel/finite 測試不得退步）。確認 RED。
- [x] 在 `InvokeReport` 新增 `timed_out: bool`；更新 claude_cli.rs 內所有 `InvokeReport { .. }` 建構與測試 fixture。
- [x] `invoke` 簽章新增尾端 `timeout: Option<Duration>`；spawn 前捕捉 `Instant`；把 `(start_instant, timeout)` 與一個共享 `Arc<AtomicBool>` timed_out 旗標傳入 watcher。
- [x] `spawn_cancel_watcher` 新增第三檢查分支：`timeout` 為 `Some(limit)` 且 `start_instant.elapsed() > limit` → `kill_handle.terminate_tree()` + 設 timed_out 旗標 + return；維持既有 `done`/`cancel` 優先序與 join-before-return 不變。
- [x] 收尾時把 timed_out 旗標讀進 `InvokeReport.timed_out`。
- [x] 跑測試轉 GREEN；確認既有 cancel 測試全綠。

## 5. invoke denial 累計（`codebus-core/src/agent/claude_cli.rs`，依賴 task 3）

> 對應 spec：verb-library `Requirement: Sandbox Denial Signal Observability`（invoke 累計段）。設計：D8。

- [x] 先寫測試（claude_cli.rs `#[cfg(test)]`，用 `TestBackend` 餵 `ToolResult` 事件）：餵 `is_error == true` 且含 denial marker 的結果 → `InvokeReport.sandbox_denial_count == 1`；餵 `is_error == true` 無 marker（grep-no-match）→ `0`；餵 `is_error == false` 含 marker 文字 → `0`。確認 RED。
- [x] 在 `InvokeReport` 新增 `sandbox_denial_count: usize`；更新 claude_cli.rs 內 `InvokeReport { .. }` 建構與 fixture。
- [x] 在 invoke 主迴圈 token 累計處旁，對每個 `StreamEvent::ToolResult { output, is_error }` 且 `is_error == true` 呼叫 `stream::is_sandbox_denial(output)`，命中則累加 count；收尾寫進 `InvokeReport.sandbox_denial_count`。
- [x] 跑測試轉 GREEN。

## 6. verb 層派生與 surface（goal/query/fix/chat/quiz.rs，依賴 1/4/5）

> 對應 spec：verb-library `Requirement: Run Wall-Clock Timeout Safety Net` + `Requirement: Sandbox Denial Signal Observability`（verb 下傳/派生/surface 段）。設計：D4、D8。

- [x] 先寫/擴充測試：以 mock 使 `invoke` 回 `timed_out == true` 且 cancel 未觸發 → verb 寫 `RunLog.outcome == "failed"` + `interrupt_reason == Some(Timeout)` 且不 auto-commit；cancel 與 timeout 同時 → outcome `"cancelled"` + `UserCancel`（cancel 優先）；`invoke` 回 `sandbox_denial_count > 0` → `RunLog.sandbox_denial_count` 帶值且 stderr 有 `warning: sandbox-denial`、outcome 不變。確認 RED。
- [x] 五個 `run_*` 簽章新增 `timeout: Option<Duration>`（位置與既有 `cancel` 並列）並下傳給對應 `agent::invoke`（goal.rs 兩處 invoke 都要傳）。
- [x] 在每個 verb 的 `invoke` 返回後依 cancel → timeout → exit 優先序派生 outcome / interrupt_reason，timeout 命中時設 `outcome="failed"` + `interrupt_reason=Some(Timeout)` 並維持既有 auto-commit-skip 契約。
- [x] 每個 verb 把 `invoke_report.sandbox_denial_count` 寫進 `RunLog.sandbox_denial_count`；count > 0 時以 parent-side `eprintln!` 印一行 `warning: sandbox-denial: N ...`。
- [x] 跑測試轉 GREEN。

## 7. CLI caller 注入（`codebus-cli/src/commands/{goal,query,fix,chat,quiz}.rs`，依賴 2/6）

> 對應 spec：verb-library `Requirement: Run Wall-Clock Timeout Safety Net`（caller 注入段）。設計：D6。

- [x] [P] 先寫/擴充 CLI 測試：確認 timeout 自 `lifecycle.run_timeout_secs` 載入並注入（至少一個 command 的 flow 測試覆蓋 config→Duration 注入；config 壞 → warn + `None`）。確認 RED。
- [x] [P] 五個 CLI command 在既有 config 載入處加 `load_lifecycle_config`，把 `Option<u64>` 秒轉 `Option<Duration>`，注入對應 `run_*` 呼叫；載入失敗沿用標準 warn-and-default 到 `None`。
- [x] [P] 跑 CLI 測試轉 GREEN。

## 8. App IPC caller 注入（`codebus-app/src-tauri/src/ipc/{goals,chats,quiz}.rs`，依賴 2/6）

> 對應 spec：verb-library `Requirement: Run Wall-Clock Timeout Safety Net`（caller 注入段）。設計：D6。

- [x] [P] 三個 IPC handler 在既有 config 載入處加 `load_lifecycle_config`，轉 `Option<Duration>` 注入對應 `run_*`；載入失敗 warn-and-default 到 `None`。
- [x] [P] 補/更新對應 handler 測試（若有）；確認 app crate 編譯通過。

## 9. 收尾驗證

- [x] 跑 `cargo test`（core + cli）+ `cargo clippy` 全綠。
- [x] 跑 `spectra validate run-outcome-lifecycle-integrity`。
- [x] 自我核對 Part B：grep-no-match negative test 存在且綠、denial 不翻 outcome；Part A：`timeout: None` 路徑 byte-equivalent（既有 invoke 測試未改語意）。
