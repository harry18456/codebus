## Why

兩個會讓 run 的「結果可信度」失真的漏洞，都落在同一條 `agent::invoke` → outcome 判定路徑上：

1. **無 per-run wall-clock 兜底**：`agent::invoke` 的主迴圈用阻塞 `BufReader::lines()` 讀 stdout，背景 watcher 只輪詢 `cancel` / `done` 兩個 flag。當 agent 在網路呼叫或 stalled tool 上 hang 住、且無人值守（無人按取消）時，run 會無限期卡住，沒有任何上限把它收掉。

2. **codex 內層 sandbox-denial 被頂層 exit 0 遮蔽**：codex `exec` 即使內層 shell 指令被 sandbox 擋下（PoC 0.135.0 實證），頂層 process 仍 exit 0；而 codebus 的 outcome 只看頂層 exit（`goal.rs` 等五個 verb 同模式），於是「其實被擋」的 run 被標成 `succeeded`，且這個訊號雖已被 `codex_parser` 解析成 `ToolResult.is_error` 卻只進 render、沒回饋 outcome 也沒被記錄。

兩者都侵蝕 RunLog 作為「這次 run 到底發生什麼」的權威紀錄，適合在同一個 change 一起補。

## What Changes

- **Part A — per-run wall-clock timeout（安全網）**
  - `agent::invoke` 新增可選 `timeout: Option<Duration>` 參數（與既有 `cancel` 並列、同為 caller 注入的 runtime 控制信號）。watcher thread 新增第三分支：當 `elapsed > limit` 時呼叫既有 `KillHandle::terminate_tree()`（**重用** cancel 路徑的 tree-kill，不另造）。
  - `InvokeReport` 新增 `timed_out: bool`，讓 verb 能把「因 timeout 被殺」和「child 自己非零退出」「user cancel」三者區分開。
  - 五個 `run_*`（goal / query / fix / chat / quiz）把 `timeout` 往下傳；timeout 命中時 `outcome = "failed"` 且 `interrupt_reason = Some(Timeout)`。`cancel` 優先於 `timeout`（已 cancel 就是 `cancelled` / `UserCancel`）。
  - 新增 `InterruptReason::Timeout` 具名變體（serde kebab-case `"timeout"`），同步 run-log capability 列舉、serde 規則、scenario。
  - 新增 config namespace `lifecycle.run_timeout_secs`（整數秒；**預設不存在 = `None` = 不限 = 維持現狀 byte-equivalent**），由 caller（CLI / app）載入後注入；verb library 自身不讀 config（沿用既有 convention）。

- **Part B — sandbox-denial 可觀測性（防誤報為第一守則）**
  - 新增 locale-independent 的 sandbox-denial 偵測器：**只**掃描 `is_error == true` 的 `ToolResult` 輸出，比對一組 curated、跨語系穩定的權限拒絕標記（如 `PermissionDenied` / `UnauthorizedAccessError` / `Access is denied` / Unix `Permission denied`）。**不是**任何內層非零都算 denial——正常 grep-no-match（exit 1、無標記）不計數。
  - `agent::invoke` 累計 `sandbox_denial_count`（與 token 累計同模式），透過 `InvokeReport` 回報。
  - `RunLog` 新增 `sandbox_denial_count: usize`（serde `default` + 為 0 時略過序列化，讓既有非 codex / 乾淨 run 的 jsonl 維持 byte-identical）；偵測數 > 0 時 verb 額外印一行 `warning: sandbox-denial` 到 stderr。
  - **MVP 不自動翻 outcome**：denial 數與 `outcome` 正交（如同 `interrupt_reason`）；只有未來於實機驗證精度後才考慮影響 outcome。

## Capabilities

### New Capabilities

(none)

### Modified Capabilities

- `verb-library`: `agent::invoke` 新增 `timeout` 參數與 watcher timeout 分支、`InvokeReport` 新增 `timed_out` / `sandbox_denial_count`；五個 `run_*` 的 timeout 下傳與 outcome / interrupt_reason 派生（cancel 優先）；timeout limit 的 config schema 與 caller 注入契約；sandbox-denial 累計與 stderr warning。
- `run-log`: `InterruptReason` 新增 `Timeout` 具名變體（kebab-case 序列化 + 向後相容 + scenario）；`RunLog` 新增 `sandbox_denial_count` 欄位（serde default、為 0 時略過、legacy row 乾淨還原）。

## Impact

- Affected specs: `verb-library`, `run-log`
- Affected code:
  - New:
    - codebus-core/src/config/lifecycle.rs
    - codebus-core/src/stream/sandbox_signal.rs
  - Modified:
    - codebus-core/src/agent/claude_cli.rs
    - codebus-core/src/log/sink.rs
    - codebus-core/src/config/mod.rs
    - codebus-core/src/stream/mod.rs
    - codebus-core/src/verb/goal.rs
    - codebus-core/src/verb/query.rs
    - codebus-core/src/verb/fix.rs
    - codebus-core/src/verb/chat.rs
    - codebus-core/src/verb/quiz.rs
    - codebus-cli/src/commands/goal.rs
    - codebus-cli/src/commands/query.rs
    - codebus-cli/src/commands/fix.rs
    - codebus-cli/src/commands/chat.rs
    - codebus-cli/src/commands/quiz.rs
    - codebus-app/src-tauri/src/ipc/goals.rs
    - codebus-app/src-tauri/src/ipc/chats.rs
    - codebus-app/src-tauri/src/ipc/quiz.rs
  - Removed: (none)
