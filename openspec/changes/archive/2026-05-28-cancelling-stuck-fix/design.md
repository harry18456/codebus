## Context

`codebus-core/src/agent/claude_cli.rs` 的 `invoke()` 是 codebus 所有 provider（Claude、Codex）共用的 invocation loop。當前實作主迴圈 `for line in reader.lines()` 是 std blocking IO read child stdout、cancel check 寫在主迴圈內。

當 child process 停止輸出 stdout（LLM hung on network call、child waiting on stalled tool result、network wait），`BufReader::lines()` 在 OS pipe 上 blocking、迴圈不轉、`flag.load()` 永遠不執行 → child 永遠不 kill → user 點 Cancel 後 UI 永遠卡「Cancelling…」、events file 缺 `spawn_end`、runner thread leak。

`backend-cleanup-codex-websearch-and-runid-millis` archive 階段 CDP smoke 留下 evidence：events file 只 3 行沒 `spawn_end`、`emit_terminal` 沒發、runner 沒返回。grep `claude_cli.rs:142-174` 確認 root cause（cancel check reactive to stdout activity）。Hypothesis b CONFIRMED；polling latency / thread deadlock / run_id collision 均排除。

Cancel 名詞跨三層、設計時須明確 disambiguate：

- **Frontend `cancelling` flag**：`codebus-app/src/store/goals.ts:151-162` 的 UI state，反映「user 已點 Cancel、等 backend 回 terminal event」。
- **Backend `cancel: Arc<AtomicBool>`**：`codebus-core/src/agent/claude_cli.rs:95` 的 invoke 參數，由 `codebus-app/src-tauri/src/ipc/goals.rs:284-298` 的 `cancel_goal_impl` set true。
- **平台 process kill signal**：Unix `SIGTERM` via `libc::kill`、Windows `TerminateProcess` via `windows-sys`。

本 design 修補的是「backend `cancel` flag 從 set 到實際殺 child」中間斷掉的環節。

### Pre-apply 校準（2026-05-28，apply 階段 CDP smoke 後補）

**Grandchild process tree finding**：apply Task 7 真實 CDP smoke 揭露單純 PID kill child 不夠。

實測：點 Cancel → events 卡 9 行不寫 `spawn_end`、UI 卡 `Cancelling…` 4+ 分鐘。手動 `taskkill /F /PID <孫程序 node.exe>` 後立刻 `spawn_end` 寫入、UI 切 `interrupted`。

根因：codex (`codex.cmd`) 與 claude 同樣是 `cmd.exe → node.exe` 兩層。invoke 持有的 `child.id()` 是 cmd.exe wrapper、watcher 殺它 OK，但 node.exe 孫程序繼承 stdout pipe 繼續活著、`BufReader::lines()` 等不到 EOF、invoke 永久 block。

原 design Risks 段假設「cmd.exe / codex.cmd 拉起的孫 process 在 parent 終止後由 Windows job object 自動收割」是錯的——Windows 沒有預設 job object、孫程序變孤兒。同理 Unix `SIGTERM` 對單一 PID 不會跨 process group。

**Fix 方向更新**：

- **Windows**：每次 spawn 建一個 `JobObject` with `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`、`AssignProcessToJobObject` 把 child handle 綁進去。Cancel 時呼叫 `TerminateJobObject(job, 1)`：kernel 一次殺整棵樹。Invoke 返回前 `CloseHandle(job)` 也會觸發 kill-on-close、保險孫程序自然收割。
- **Unix**：spawn 前 `cmd.process_group(0)` 讓 child 成為新 process group leader、`libc::killpg(-pgid, SIGTERM)` 一次殺整 group。

**Scope 影響**：

- `Cargo.toml` windows-sys 多 feature `Win32_System_JobObjects` + `Win32_System_IO`。
- `process_kill.rs` 重構成 `KillHandle` 抽象：建構時持平台特定資源（job handle / pgid）、`terminate_tree()` 殺整棵、Drop 收尾。`kill_child_by_id(pid)` 移除（不再夠用）。
- `invoke()` spawn 後立即建 `KillHandle`、watcher 持 `Arc<KillHandle>`、cancel 時呼叫 `terminate_tree()`。
- 既有 unit test 用 powershell / sleep（無 grandchild）→ 仍綠。新增 grandchild scenario test：spawn 一個 wrapper 拉起 grandchild、cancel 必須殺到 grandchild。

## Goals / Non-Goals

**Goals:**

- `invoke()` 必須在 child 停止輸出 stdout 時、仍能在 bounded latency（≤ 200ms 典型）內偵測 cancel flag 並 kill child。
- Cancel 機制對 Claude / Codex 兩 backend 同時生效（修在 provider-agnostic 的 `invoke` 內）。
- 跨平台正確：Unix + Windows 都能 kill by PID、idempotent、雙 kill 無害、已死 child kill error 忽略。
- 維持現有 fast path：child 還在輸出時、主迴圈內 cancel check 仍提供 ms 級反應。
- 不 leak watcher thread：主迴圈正常完成（無 cancel）時 watcher 必須 graceful 結束、function 返回前 join。

**Non-Goals:**

- 不 redesign `invoke()` 為 async / tokio runtime（scope creep、現在是 sync code）。
- 不拿掉主迴圈現有 cancel check（fast path 仍 valuable）。
- 不動 frontend cancel UI、不動 IPC 層 `cancel_goal` / `cancel_goal_impl`（flag 流向正確，bug 只在 invoke）。
- 不加 force-timeout（user 30 分鐘 goal 不該被 force-kill）。
- 不改 stdin / stderr 處理路徑。

## Decisions

### Decision A: 背景 cancel watcher thread + PID kill

在 `invoke()` spawn child 後、開額外 background thread。Watcher：

1. 每 100ms `thread::sleep`、然後檢查兩個 flag。
2. `done.load()` true → break（主迴圈已正常完成、watcher 退出）。
3. `cancel.load()` true → 呼叫 `process_kill::kill_child_by_id(pid)`、break。

PID 在 spawn 之後可從 `child.id()` 取得（Rust `std::process::Child::id()` 回 `u32`），watcher 持有 `pid: u32` 而非 `&Child`，避開 `Child` ownership 障礙（主執行緒需 `child.kill()` 與 `child.wait()`）。

**Why over alternatives:**

- vs. 把 `reader.lines()` 換 non-blocking poll loop：要寫自己的 buffer 處理 partial line、複雜度高、scope creep。
- vs. 改 async tokio：rewrite invoke 全部 IO、跨整個 codebus-core API、scope 爆。
- vs. 在 child stdout 端塞 deadline read：std `BufReader` 沒 timeout、要降到 `RawFd` / Windows handle 加 `poll` / `WaitForSingleObject`、平台抽象量同 PID kill、但 PID kill 更直觀。
- vs. `child.kill()` from watcher：`Child` 不 `Clone`，watcher 取得 `&mut Child` 後主執行緒就不能用 `child.wait()` reap、ownership 死結。改用 PID kill 繞過。

### Decision B: 跨平台 process_kill helper

新檔 `codebus-core/src/agent/process_kill.rs` 暴露：

```rust
pub fn kill_child_by_id(pid: u32) -> io::Result<()>;
```

- **Unix（target_family = "unix"）**：`libc::kill(pid as libc::pid_t, libc::SIGTERM)`；errno = `ESRCH`（process not found）視為 Ok。
- **Windows（target_family = "windows"）**：`OpenProcess(PROCESS_TERMINATE, FALSE, pid)` → `TerminateProcess(handle, 1)` → `CloseHandle(handle)`；`OpenProcess` 回 `NULL` 或 `TerminateProcess` 對已退出 process 回 error 視為 Ok。

Helper 內 wrap `unsafe` 區塊；對外回 `io::Result<()>`、test 可 stub（透過建一個短生命 child process）。

**Why over alternatives:**

- vs. 用 `nix` crate：新增 dep、Unix-only；codebus-core 已有的依賴未必含；若無、直接加 `libc` Unix-only dep。
- vs. `windows` crate（更高階）：保持與 codebus-core 既有 windows interop 風格一致；若尚未引入、新加 `windows-sys` 只取 `Win32_System_Threading` + `Win32_Foundation` 兩 feature。

### Decision C: done flag 通知 watcher 退出

新 `done: Arc<AtomicBool>`，主迴圈在所有正常退出 path（loop break、`child.wait()` 之前）設 `done.store(true)`、watcher 下次 wake 看到後 break。Watcher handle `join` on function 返回前、不 detach。

**Why over alternatives:**

- vs. abort watcher thread：Rust `JoinHandle` 不支援 abort。
- vs. mpsc channel signal：兩個 flag 更輕量、無 allocation、語意對稱於現有 `cancel` flag。
- vs. detach watcher：違反「不 leak thread」goal。

### Decision D: 主迴圈 fast path 保留

主迴圈內既有 `if cancel.load() { child.kill(); cancelled = true; }` 保留。child 還在輸出時、cancel 在 ms 級觸發、比 watcher 100ms 快。

雙 kill race 安全：
- POSIX `SIGTERM` idempotent；雙發無害。
- Windows `TerminateProcess` 對已退出 process 回 error code、`process_kill` 忽略。

### Decision E: Spec 雙更新

- `openspec/specs/app-workspace/spec.md` 的 `Spawn allowed after cancel completes` scenario 新增 NOTE 與一個 child-stuck scenario。
- `openspec/specs/agent-backend/spec.md` 的 `Invocation Loop Drives Backend Trait` requirement 新增 scenario：cancellation polling 不可耦合 stdout activity。

這兩條 spec 同 change 一起改、避免分裂；agent-backend 是 invoke 行為的 owner、app-workspace 是 UI-observable 結果的 owner。

## Implementation Contract

**Behavior:** 當 `agent::invoke` 進行中、`cancel: Arc<AtomicBool>` 被 set true，`invoke` SHALL 在 bounded latency（≤ 200ms 典型）內：
1. Kill child process（即使 child 已不輸出 stdout）。
2. 從 `reader.lines()` 自然 EOF 退出主迴圈。
3. `child.wait()` reap。
4. Watcher thread join。
5. 函式正常返回 `Ok(InvokeReport)`，`exit` 反映 killed state。

**Interface:**

- `invoke(backend, spec, vault_root, on_event, cancel: Option<Arc<AtomicBool>>) -> io::Result<InvokeReport>` 簽章不變。
- 新增 module-private `kill_child_by_id(pid: u32) -> io::Result<()>` 在 `codebus-core/src/agent/process_kill.rs`、`agent` mod 內以 `pub(crate)` 暴露。

**Failure modes:**

- `kill_child_by_id` 對已退出 process 回 `Ok(())`（idempotent）。
- `kill_child_by_id` 其他 errno / Windows GLE 回 `io::Error`；watcher 內 log 但不 propagate（best-effort）。
- 主迴圈 `child.kill()` 維持現有「ignore failure」行為。
- Watcher join 在 function 返回前；若 watcher 卡死（不應發生，但保險），最壞 case 由 `done` flag 觸發、≤ 100ms 結束。

**Acceptance criteria:**

1. `cargo test -p codebus-core agent::claude_cli` 綠，含新測試 `cancel_returns_within_bounded_latency_when_child_silent`：fake binary spawn 後 `sleep 30`、不輸出 stdout、set cancel flag、verify `invoke` 在 ≤ 200ms 內返回、`exit.success() == false`。
2. `cargo test --workspace` 全綠。
3. `pnpm tsc && pnpm test`（frontend）綠（frontend 不該動）。
4. 真實 CDP smoke（per `project_cdp_smoke_webview2_pitfalls` 五個踩雷）：
   - 案例 1：開 goal 跑幾秒、click Cancel → ≤ 1 秒 UI 切 terminal（Interrupted 非 Cancelling）。
   - 案例 2：故意 prompt「等 30 秒不要輸出」造 child stuck、click Cancel → 仍 ≤ 1 秒切 terminal。
   - 案例 3：events file 含 `spawn_end` 與 `interrupted`、非 3 行卡死。
   - 案例 4：Codex 同 prompt 同流程重跑、驗 Windows TerminateProcess path 對 codex 也動。
   - 截圖存 `codebus-app/scripts/.cancelling-stuck-fix-smoke/`。
5. Manual on macOS 或 Linux 至少一次 cancel cycle（驗 Unix `SIGTERM` path）。若無 macOS / Linux 機台，apply 階段 stop 對齊。

**Scope boundaries:**

- **In scope:** `claude_cli.rs invoke()` 函式內部、新 `process_kill.rs` helper、`codebus-core/Cargo.toml` deps、`agent-backend` + `app-workspace` 兩 spec、`docs/2026-05-28-cancelling-stuck-todo.md` archive 標記。
- **Out of scope:** frontend cancel store、IPC `cancel_goal` 層、其他 verb（chat、quiz）的 cancel path（雖然共用 `invoke`、會順帶受益但不額外驗）、SpawnSpec / AgentBackend trait、Codex parser、CDP smoke 工具本身。

## Risks / Trade-offs

- ~~**[Windows TerminateProcess 子 process 樹未被收割]** → Mitigation: cmd.exe / codex.cmd 拉起的孫 process 在 parent 終止後由 Windows job object 自動收割，本 change 不處理 process tree termination；若 apply 階段發現 zombie 進程殘留、stop 對齊。~~ **[REVISION 2026-05-28 apply 階段]** 假設錯誤、被 CDP smoke 抓出：Windows 沒有預設 job object，孫 process 變孤兒、繼承 stdout pipe 讓 invoke 永久 block。Scope 已擴張到 Pre-apply 校準描述的 KillHandle + JobObject + Unix pgroup 路線、process tree termination 改為 in-scope。
- **[Unix SIGTERM child trap 不退出]** → Mitigation: Claude CLI 與 codex 都 honor `SIGTERM`；若特定 binary trap、需要 escalate `SIGKILL`，本 change 不實作 escalation，apply 發現 stop 對齊。
- **[Unix process group 跨 Tauri runtime 行為]** → REVISION 2026-05-28：Unix 端用 `Command::process_group(0)` + `libc::killpg(pgid, SIGTERM)`、shell job control 應該無感（child 是 leader、其輸出仍 pipe 回 parent）。Tauri 主程序不受影響（spawned child 在自己的 group）。實機 macOS / Linux 驗證仍 pending、apply 階段若 CI Unix runner 有任何 process group 異常即 stop 對齊。
- **[Watcher thread 100ms latency 不夠快]** → Mitigation: 主迴圈 fast path 仍處理「child 還在輸出」的常見 case；100ms 對「child stuck」的少見 case 足夠（user 容忍 1 秒級反應）。若實測 200ms 仍不夠、調降 poll interval（cheap）。
- **[done flag store 與 watcher load race]** → Mitigation: 用 `Ordering::SeqCst` 或最小 `Ordering::Release` / `Ordering::Acquire`；watcher 漏一次 wake 最壞延遲 100ms，仍在 bounded latency 內。
- **[新 windows-sys feature 增加 Cargo.toml diff]** → Mitigation: 只加 `Win32_System_Threading` + `Win32_Foundation` 兩個 feature；若 codebus-core 已有 `windows-sys`、append features；若無、新加 dep 標 `target_family = "windows"`。
- **[unit test 對 PID kill 平台依賴]** → Mitigation: test 用 `std::process::Command::new("sleep")` (Unix) / `cmd /c timeout` (Windows) 拉子 process、verify kill works；CI 跑兩平台覆蓋。

## Migration Plan

直接 main、無 feature branch、無 rollback gate（solo dev codebus、per memory）。apply 完跑驗收 5 條、CDP smoke 兩 provider 通過、commit、archive、標 `docs/2026-05-28-cancelling-stuck-todo.md` archived。

若 apply 階段 Windows TerminateProcess 行為意外（如 process tree 殘留）、stop 對齊不擅自降級為 force-timeout。

## Open Questions

- `codebus-core/Cargo.toml` 是否已有 `windows-sys` 與目標 features？apply Task 1 先 grep 確認、決定是 append feature 或新加 dep。
- Unix unit test 用 `sleep` 命令、Windows 用什麼 idle binary？候選：`cmd /c timeout /t 30 /nobreak`；apply Task 階段確認 CI Windows runner 有 `timeout.exe`。
