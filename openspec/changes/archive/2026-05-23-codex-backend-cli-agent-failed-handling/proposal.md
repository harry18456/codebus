## Why

`codex-backend` change（archived 2026-05-23）為了修「exit-code 誤標 succeeded」bug 在 `codebus_core::verb::VerbError` 加了新 variant `AgentFailed { exit_code: Option<i32> }`，並由 `verb/chat.rs:275` 在 agent 非零 exit 時 emit。但：

1. **5 個 CLI command thin wrapper（`commands/{chat,fix,goal,query,quiz}.rs`）的 `translate_error` 沒同步加 match arm** → `cargo build -p codebus-cli` 全失敗（5 個 E0004 non-exhaustive patterns）。
2. **`verb-library` spec 的 `Verb Error Enum` 要求（spec.md:292）明列「exactly these variants」但沒含 `AgentFailed`**，`cli_exit_code` 映射表（line 314）也沒涵蓋 → spec 與 code 失步。

build 破在 main、不可實質開發任何 CLI side 工作（含 `agent-hook-hardening` 已被 park 在等這條解）。`codebus-app` GUI path 不走這 5 個 thin wrapper（透過 tauri commands 直連 core），所以 codex-backend change 走 GUI 實機驗證時沒發現；CLI side 缺 CI matrix 攔截。

## Problem

執行 `cargo build -p codebus-cli` 或 `cargo test -p codebus-cli` 時，rustc 報 5 處 E0004：

- codebus-cli/src/commands/chat.rs：`translate_error` 在 line ~313
- codebus-cli/src/commands/fix.rs：類似位置
- codebus-cli/src/commands/goal.rs：類似位置
- codebus-cli/src/commands/query.rs：類似位置
- codebus-cli/src/commands/quiz.rs：類似位置

每處的 `match err { ... }` 沒涵蓋 `VerbError::AgentFailed { .. }`。

額外地，`verb-library` spec.md:292 的 `Verb Error Enum` 列舉「exactly these variants」沒有 `AgentFailed`，spec.md:314 `cli_exit_code` 映射表也沒有對應條目；spec 顯示為 6 variant，code 實為 7 variant。

## Root Cause

`codex-backend` change 在 codebus-core 端把 `AgentFailed` 加進 `VerbError` 並由 `verb/chat.rs` 使用，**但沒同時更新 5 個 CLI thin wrapper 的 match arm，也沒更新 `verb-library` spec 的 variant 列舉**。codex-backend change 透過 codebus-app GUI 端到端驗證（GUI 走 tauri command 不經 CLI thin wrapper），CLI side build 沒人跑 → 漏到 main。

設計面：`AgentFailed` 目前**只由 chat verb emit**（多輪 REPL 需要把 turn 失敗當 `Err` 中斷）。one-shot verb（query/goal/fix/quiz）採不同機制，於 `Ok(report).agent_exit_code` 透傳 child exit code，不會 emit `AgentFailed`。所以 5 個 CLI match arm 中只有 chat 真會收到，其他 4 個是純編譯 exhaustiveness 需求。

## Proposed Solution

**Code（5 個 CLI command 檔）：**
- `chat.rs::translate_error` 加 `AgentFailed { exit_code }` arm：`eprintln!("error: chat: agent exited with code {:?}", exit_code)` + `ExitCode::from(1)`（exit code 透過 `cli_exit_code()` 既有 mapping = 1，不透傳 child code，與 chat 的 1=error 慣例一致）。
- `fix.rs / goal.rs / query.rs / quiz.rs::translate_error` 各加 `AgentFailed { exit_code }` arm 使用 **generic fallback 而非 `unreachable!()`**：`eprintln!("error: {verb}: agent exited with code {:?}", exit_code)` + `ExitCode::from(1)`。理由：與 `Cancelled` 既有處理方式對齊（spec.md:311 寫 `unreachable` 但 code 實際用 `ExitCode::from(0)` 防護）；萬一未來重構讓這些 verb 也 emit `AgentFailed`，generic fallback 不會 panic。

**Spec（`verb-library`）：**
- `Verb Error Enum` 要求（spec.md:292-313）的 variant 列舉補入 `AgentFailed { exit_code: Option<i32> }` 描述（含 `chat`-only emit 的設計意圖、與 `Spawn` 的差異）。
- `cli_exit_code` 映射表（spec.md:314）補入 `AgentFailed → 1` 條目。
- 補上對 CLI match arm 的對應指引（與 `Cancelled` 段平行）：chat 在 active arm 寫 user-facing 文字、其他 verb 寫 generic fallback。
- 新增 1-2 個 Scenario 涵蓋 chat 端 `AgentFailed` 行為。

**Tests（隨手補強）：**
- `codebus-core/src/verb/error.rs` 既有 `cli_exit_code_mapping_covers_every_variant` test 補入 `AgentFailed { exit_code: Some(2) } → 1` case。
- 新增 1 個 `agent_failed_display_includes_exit_code` test：驗 `AgentFailed { exit_code: Some(42) }.to_string()` 含 `"42"`，`AgentFailed { exit_code: None }.to_string()` 不含 parens（既有 Display 條件展開邏輯）。

## Non-Goals (optional)

- 不改 `cli_exit_code` 的回傳 policy（保持 AgentFailed → 1，不透傳 child exit code）——chat vs query/goal 在 exit code 透傳上的 inconsistency 是另一條議題，本 change 不處理（記錄在 design notes）。
- 不擴 `AgentFailed` emit 到其他 verb（goal/query/fix/quiz）——這是設計差異而非 bug，one-shot verb 已透過 `Ok(report).agent_exit_code` 處理。
- 不改 codex-backend 既有 archive 內容（已 archive 的 change 不回頭翻動，本 change 是 forward-looking 補救）。
- 不為 CLI thin wrapper 寫 integration test 覆蓋 AgentFailed runtime 觸發路徑——chat 端整合測試需要 mock claude 子程序回 non-zero exit，工程量超出 build unblock 範圍。
- 不引入 `#[non_exhaustive]` 標註——這會掩蓋未來相同類型遺漏，與專案「exhaustive match 強制 compile-time 覆蓋」哲學衝突（spec.md:314 明寫此意圖）。

## Success Criteria

- `cargo build -p codebus-cli` 通過（0 errors）。
- `cargo build --workspace` 通過。
- `cargo test -p codebus-core verb::error` 全綠（既有 6 條 + 新增 2 條，共 8 條）。
- `cargo test -p codebus-cli` 全綠（不會新增 hook 端測試，但既有測試 + 編譯都需通過）。
- `spectra validate codex-backend-cli-agent-failed-handling` 通過。
- `verb-library` spec 的 `Verb Error Enum` 條款明列 `AgentFailed` variant 與其 `cli_exit_code` 映射，`grep AgentFailed openspec/specs/verb-library/spec.md` 至少 3 個命中（variant 描述、cli_exit_code 表、Scenario）。
- 手動 sanity：`cargo build` 後直接執行 `codebus chat`，與 codex 對話一輪、收到 codex 非零 exit 時 stderr 出現「error: chat: agent exited with code <N>」。

## Impact

- Affected specs:
  - `verb-library`（`Verb Error Enum` requirement MODIFIED）
- Affected code:
  - Modified:
    - codebus-cli/src/commands/chat.rs
    - codebus-cli/src/commands/fix.rs
    - codebus-cli/src/commands/goal.rs
    - codebus-cli/src/commands/query.rs
    - codebus-cli/src/commands/quiz.rs
    - codebus-core/src/verb/error.rs（補既有 test）
- Tests:
  - codebus-core/src/verb/error.rs 既有 test 模組擴增 2 條（cli_exit_code 涵蓋 AgentFailed、AgentFailed Display 條件展開驗證）
- 不影響：
  - codebus-core/src/verb/chat.rs（`AgentFailed` 既有 emit 點不動）
  - codebus-core/src/verb/{goal,query,fix,quiz}.rs（不擴 AgentFailed emit；維持既有 `Ok(report).agent_exit_code` 透傳路徑）
  - codebus-app（GUI 不走這 5 個 thin wrapper）
  - codex-backend 旗標、claude_cli backend、PII filter、vault 同步邏輯
- 跨平台：純 Rust 編譯期修正 + 純字串級 stderr 訊息，無 OS-specific syscall，Windows / macOS / Linux 行為一致。
- 解鎖效應：本 change merge 後，`agent-hook-hardening` 可以 unpark 並從 task 1.1 重啟（既有 park 在等這條解 build）。
