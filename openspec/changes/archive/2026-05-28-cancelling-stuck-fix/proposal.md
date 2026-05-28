## Why

User 點 Cancel 後 UI 永遠卡在「Cancelling…」、events file 只 3 行沒 spawn_end、runner thread 永久不返回——背景在 `backend-cleanup-codex-websearch-and-runid-millis` archive 階段 CDP smoke 抓到證據（events file 3 行缺 spawn_end、runner 沒返回、emit_terminal 沒發）。grep `codebus-core/src/agent/claude_cli.rs:142-174` 確認 root cause：主迴圈 `for line in reader.lines()` 是 blocking read child stdout、cancel check 寫在主迴圈內反應 stdout activity；當 child 停止輸出（LLM hang、等 tool result、network wait），`BufReader::lines()` 永久 block、cancel flag 永遠看不到、child 永遠不 kill。這直接違反 `openspec/specs/app-workspace/spec.md:922-925` 的 `Spawn allowed after cancel completes` scenario 隱含的 invariant（cancel 必須完成）。

## What Changes

- `codebus-core/src/agent/claude_cli.rs` 的 `invoke()` 在 spawn child 後新增 background **cancel watcher thread**，每 100ms poll `cancel: Arc<AtomicBool>`；flag = true 時透過平台 PID kill 殺 child，避開 Rust `Child` ownership 障礙，繞過 blocking `reader.lines()`。
- 抽 `kill_child_by_id(pid: u32)` cross-platform helper：Unix 用 `libc::kill(pid, SIGTERM)`；Windows 用 `windows-sys::Win32::System::Threading::OpenProcess + TerminateProcess`。POSIX `kill` idempotent；Windows `TerminateProcess` 對已退出 process 回 error code、忽略即可。
- 主迴圈內既有 cancel check（`codebus-core/src/agent/claude_cli.rs:165-173`）保留為 **fast path**：child 還在輸出時 user 按 cancel、主迴圈 ms 級反應比 watcher 100ms latency 快。兩條 cancel path 並存。
- Watcher cleanup：新增 `done: Arc<AtomicBool>` 主迴圈完成時設 true、watcher break；function 返回前 `join` watcher、不 detach、防 thread leak。
- `openspec/specs/app-workspace/spec.md` 的 `Spawn allowed after cancel completes` scenario 加 NOTE：cancel 必須在 bounded latency（≤ 200ms 典型）完成、即使 child 已不輸出 stdout（LLM hung on network call、child waiting on stalled tool result），cancel signal 不可只 reactive to stdout activity。
- `openspec/specs/agent-backend/spec.md` 的 `Invocation Loop Drives Backend Trait` requirement 加 scenario：invoke 的 cancellation polling 不可耦合 stdout activity；child 不輸出時 cancel 仍須在 bounded latency 觸發 child kill。
- 新增 `codebus-core` unit test：mock child 不輸出、set cancel flag、verify watcher 在 ≤ 200ms 內 kill child + `invoke()` return。
- 真實 CDP smoke（per `project_cdp_smoke_webview2_pitfalls`）：開 goal 跑幾秒按 Cancel → ≤ 1 秒切 terminal；故意製造 child stuck（prompt「等 30 秒不要輸出」）→ 仍 ≤ 1 秒切 terminal；驗 events file 有 `spawn_end` / `interrupted`。截圖存 `codebus-app/scripts/.cancelling-stuck-fix-smoke/`。
- 跨 provider 驗：Claude + Codex 兩 provider 都跑同樣 cancel test、確認 PID kill 機制兩 backend 都動。

## Non-Goals (optional)

- 不 redesign 整個 cancel 機制：保留現有 `cancel: Arc<AtomicBool>` flag pattern、只在 invoke 內加 watcher thread 補洞。
- 不拿掉主迴圈 cancel check：fast path 仍 valuable。
- 不 detach watcher thread：leak risk。
- 不把 `reader.lines()` 改 async / tokio runtime：scope creep、現在是 sync code。
- 不動 frontend cancel UI（`codebus-app/src/store/goals.ts:151-162` 的 `cancelGoal` action、IPC `cancel_goal`）：backend 修完、UI 自然 unblock。
- 不加 force-timeout：user 設 30 分鐘 goal 不該被 force-kill。
- 不修改 IPC 層 `codebus-app/src-tauri/src/ipc/goals.rs:284-298` 的 `cancel_goal` / `cancel_goal_impl`：flag set 正確、bug 不在這。

## Capabilities

### New Capabilities

(none)

### Modified Capabilities

- `agent-backend`: invoke loop 的 cancellation polling 行為加 bounded-latency 保證、明確要求 cancel 偵測不可耦合 stdout activity。
- `app-workspace`: `Spawn allowed after cancel completes` scenario 加 bounded latency NOTE、覆蓋 child 不輸出時的 cancel 行為。

## Impact

- Affected specs:
  - openspec/specs/agent-backend/spec.md
  - openspec/specs/app-workspace/spec.md
- Affected code:
  - Modified:
    - codebus-core/src/agent/claude_cli.rs
    - codebus-core/Cargo.toml
  - New:
    - codebus-core/src/agent/process_kill.rs
  - Removed: (none)
- Affected dependencies:
  - Unix target 已含 `libc`，無新增。
  - Windows target 新增 `windows-sys` features `Win32_System_Threading` + `Win32_Foundation`（若 Cargo.toml 未啟用對應 feature）。
- Affected docs:
  - docs/2026-05-28-cancelling-stuck-todo.md（archive 階段標 archived，per `feedback_archive_commit_immediately_after_apply`）。
- 跨 provider：claude_cli / codex_cli 兩 backend 都走同一個 `agent::invoke` loop，watcher thread 修在 `invoke()` 對兩 backend 同時生效。
- 跨平台：Unix + Windows 都需驗；macOS / Linux 走 `libc::kill`、Windows 走 `TerminateProcess`。
