## Why

D-027 archive 把 Qdrant 從 Docker Compose 切成 standalone binary 取代為主路徑，但**啟動權責**沒寫進 sidecar / Tauri lifecycle — user 必須先手動跑 `pwsh sidecar/scripts/start-qdrant.ps1` 才能 `cargo tauri dev`，否則 `POST /kb/build` 起的 SSE task 在 0.5~2s 內因 Qdrant 連線失敗 raise → 前端「internal sidecar error」。違反 desktop app「下載即可用」UX 標準，也讓 Phase 7 demo flow 多一個 user 不會做的前置步驟。本 change 把 Qdrant 子程序 lifecycle 收進 sidecar，補完 D-027 落地缺口。

## What Changes

- **新模組** `sidecar/src/codebus_agent/qdrant_supervisor.py`：暴露 `maybe_spawn_qdrant(parent_pid: int | None) -> Popen | None` 與對應 cleanup hook
- **sidecar 啟動 sequence**（`sidecar/src/codebus_agent/api/main.py` `run()`）：handshake emit 之後、`asyncio.run(_serve(...))` 之前呼 `maybe_spawn_qdrant`；spawn 成功的 Popen handle 經 `atexit.register` 與 watchdog 連動
- **binary 解析順序**：`$CODEBUS_QDRANT_BIN`（絕對路徑） → `~/.codebus/bin/qdrant.exe`（Windows）/ `~/.codebus/bin/qdrant`（POSIX）
- **storage env 沿用**：`QDRANT__STORAGE__STORAGE_PATH=$HOME/.codebus/kb`、`QDRANT__STORAGE__SNAPSHOTS_PATH=$HOME/.codebus/kb/snapshots`、可被 `$CODEBUS_QDRANT_STORAGE` 覆寫；與 `start-qdrant.ps1` 對齊
- **reuse 偵測**：spawn 前先 probe `http://127.0.0.1:6333/healthz`；2xx → log info 不 spawn（並發 sidecar instance 或 user 已手動啟動皆 reuse）
- **degraded fallback**：binary 找不到 OR probe 10s timeout → log warning + 不 raise；sidecar 仍以「Qdrant unreachable」狀態啟動（保留 D-027「degraded-but-alive」不變式）
- **三層 cleanup**：sidecar 正常退出（`atexit`）/ Tauri 死導致 sidecar `--parent-pid` watchdog `os._exit` / OS SIGTERM 三 path 都 terminate Qdrant child（先 `terminate()`、5s 內未死 `kill()`）
- **D-027 追記**：`docs/decisions.md` D-027 加「sidecar-managed auto-spawn」段，記錄此 change 落地時間與 fallback 行為
- **`start-qdrant.ps1` / `start-qdrant.sh`** 保留不刪：作 dev tool（手動跑時繞過 sidecar）+ 文件範例 + degraded 模式下的 user 自救路徑

## Non-Goals

- 不取代 `start-qdrant.ps1` / `start-qdrant.sh` —— 仍是 dev tool 與 fallback 文件範例
- 不從 Tauri Rust 層 spawn Qdrant —— 避免雙處 race；sidecar 是 Qdrant 唯一 consumer，由它管最少 surface
- 不 bundle Qdrant binary 進 PyInstaller —— Qdrant 83MB，bundle 後 PyInstaller artifact 翻倍；user 從 GitHub release 下載仍是預期路徑（未來如需單檔 distribution 開新 ADR 評估）
- 不做 Qdrant version mismatch 偵測 —— P1+ 範疇；本 change 假設 binary 是 v1.17+ 相容
- 不做 Qdrant healthcheck retry / restart loop —— probe 10s timeout 即視為 degraded，不嘗試 restart child（避免複雜度與 race）
- 不 spawn 多個 Qdrant instance —— 只在 6333 偵測得 reuse / 偵測不到才 spawn 一個；不支援 user 配置 alternate port
- 不改 `start-qdrant.ps1` 的環境變數契約 —— 完全沿用，避免 dev tool / sidecar 分歧

## Capabilities

### New Capabilities

(none)

### Modified Capabilities

- `qdrant-client`: 加 sidecar-managed auto-spawn Requirement，記錄 binary 解析順序、reuse 偵測、degraded fallback、storage env 對齊
- `sidecar-runtime`: 加 Qdrant child process supervision Requirement，記錄 spawn 時機（handshake 後、serve 前）、三層 cleanup（atexit / watchdog / SIGTERM）、並發 sidecar 偵測

## Impact

- Affected specs:
  - 改：openspec/specs/qdrant-client/spec.md（新 Requirement: Sidecar-managed Qdrant child process）
  - 改：openspec/specs/sidecar-runtime/spec.md（新 Requirement: Qdrant child process supervision lifecycle）
- Affected code:
  - New:
    - sidecar/src/codebus_agent/qdrant_supervisor.py
    - sidecar/tests/test_qdrant_supervisor.py
  - Modified:
    - sidecar/src/codebus_agent/api/main.py（`run()` spawn 時機 + 註冊 cleanup hook）
    - sidecar/src/codebus_agent/watchdog.py（`watch_parent` 觸發 `os._exit` 前 terminate Qdrant child）
    - docs/decisions.md（D-027 追記 sidecar-managed auto-spawn）
    - CLAUDE.md（Qdrant 章節改：from「user 自起 + script」改為「sidecar 自動，degraded 時 fallback to script」）
  - Removed:（無）
- Affected docs:
  - docs/decisions.md D-027 追記
  - CLAUDE.md Qdrant 章節
