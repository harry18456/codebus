## Why

兩個獨立 backend 小議題綁同 change（scope 相近、ceremony 省一輪）：

1. **Codex hosted web search 未關**：Codex provider 預設啟用 hosted `web_search` tool，允許 agent 對外 fetch 網頁，破壞 codebus「offline / sandbox-bounded」契約。2026-05-28 `docs/2026-05-28-codex-hook-hard-gate-spike.md` E11 已驗加 `-c web_search=disabled` 後 codex 回 `Web search is unavailable.` 並阻斷對外連線。
2. **`active_runs` key 同秒碰撞**：`spawn_goal` 與 `spawn_chat_turn` 用 `chrono::SecondsFormat::Secs` 派生 `RunId`（如 `2026-05-28T07-39-26Z`）。同秒兩次 spawn 會產生相同 key、後者覆蓋前者的 cancel handle、先 spawn 的 run 失去 cancel 通道。2026-05-21 `quiz-double-spawn-guard` 已對 quiz 路徑改用 `SecondsFormat::Millis`（見 `codebus-app/src-tauri/src/ipc/quiz.rs:120`），但 goal / chat 路徑未對齊；2026-05-28 Bug 3 cross-vault 允許後機率升。

## What Changes

- **Codex argv 加 `-c web_search=disabled`**：在 `codebus-core/src/agent/codex_backend.rs` 的 `build_command` isolation flag builder（line 117-126）新增第 6 個 flag pair。
- **`spawn_goal` RunId 改 millis 精度**：`codebus-app/src-tauri/src/ipc/goals.rs:230` 將 `SecondsFormat::Secs` 改 `SecondsFormat::Millis`。
- **`spawn_chat_turn` RunId 改 millis 精度**：`codebus-app/src-tauri/src/ipc/chats.rs:174` 同步改 `Millis`。
- **`codex-backend` spec**:「Codex Backend Argv Composition」requirement 與「Isolation flags always present」scenario 加入 `web_search=disabled` 並補 1 句 rationale。
- **`app-workspace` spec**:「spawn_goal returns run id derived from started_at」與「spawn_chat_turn returns chat run id」兩 scenario 字面例子改用 millis 格式（如 `2026-05-13T14-56-21.123Z`、`chat-2026-05-14T10-20-30.456Z`），並補一句「`RunId` SHALL be derived using `SecondsFormat::Millis` so two spawns within the same wall-clock second receive distinct keys」requirement 補充。
- **regression test**:
  - `codex_backend.rs` `isolation_flags_always_present` test 加 `assert_pair_present(&args, "-c", "web_search=disabled")` 一行。
  - `active_runs` 或 `goals.rs` / `chats.rs` 補 test：同 ms 內生兩個 `RunId`（mock `Utc::now` 或直接 builder 函式）驗證 key 不碰撞。Chat per-vault 限一活躍、testcase 改成「跨 vault 同 ms 兩 chat turn」對齊 `Cross-Vault Concurrent Active Runs` requirement。

## Non-Goals

- **不動 `--disable image_generation`**：user 決議保留 image generation 行為（per `docs/2026-05-28-four-bugs-backlog.md` Bug 4「Scope 限定」段）。
- **不動 RunLog disk persistence timestamps**：`codebus-core/src/agent/claude_cli.rs:100,179` 與 `codebus-core/src/verb/{chat,goal,query,fix,quiz}.rs` 的 `started_at` / `ts` 仍維持 `SecondsFormat::Secs`。這些是 events-log spec line 62 normative 強制（`SHALL ... using SecondsFormat::Secs`）、且不參與 `active_runs` key、collision 不影響 cancel signal。若要全 codebase 統一精度、另開 change。
- **不動 `codebus-app/src-tauri/src/ipc/quiz.rs`**：line 120 已是 `Millis`、是本 change 對齊的 reference impl。
- **不動 i18n / UI / `AUDIT.md`**：純 backend、無 surface 字串變動。
- **不分兩個 change**：user 已決議 bundle、scope 同 batch。

## Capabilities

### New Capabilities

(none)

### Modified Capabilities

- `codex-backend`: Codex Backend Argv Composition requirement 加 `-c web_search=disabled` 到 isolation flag set；Isolation flags always present scenario 更新對應 assertion。
- `app-workspace`: Goal Run Spawn Endpoint 與 Chat Turn Spawn Endpoint 兩 requirement 補「RunId millis 精度」requirement 字面例子更新對齊 millis format。

## Impact

- Affected specs:
  - `openspec/specs/codex-backend/spec.md`（modified — isolation flag + scenario）
  - `openspec/specs/app-workspace/spec.md`（modified — RunId millis 例子 + 補述）
- Affected code:
  - Modified:
    - `codebus-core/src/agent/codex_backend.rs`（argv builder + test）
    - `codebus-app/src-tauri/src/ipc/goals.rs`（RunId 派生）
    - `codebus-app/src-tauri/src/ipc/chats.rs`（RunId 派生）
    - `codebus-app/src-tauri/src/state/active_runs.rs`（補 collision regression test，或就近寫進 goals/chats）
  - New: (none)
  - Removed: (none)
- Affected docs（archive 階段順手更新、不在 apply scope）:
  - `docs/2026-05-28-four-bugs-backlog.md`（Bug 4 標 archived）
  - `docs/2026-05-28-run-id-collision-todo.md`（整篇標 archived）
  - `docs/2026-05-28-codex-hook-hard-gate-spike.md`（E11 段可選標 archived）
