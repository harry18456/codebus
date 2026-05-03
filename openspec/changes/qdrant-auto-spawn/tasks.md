## 1. Subprocess spawn PoC（Decision 1: Sidecar 擁有 Qdrant lifecycle，不從 Tauri Rust spawn）

- [x] 1.1 PoC: 在 Windows host 跑 `python -c "import subprocess, os; os.environ['QDRANT__STORAGE__STORAGE_PATH']=r'C:\Users\<u>\.codebus\kb'; p=subprocess.Popen([r'C:\Users\<u>\.codebus\bin\qdrant.exe']); ..."` 觀察 child 起得來、6333 listen、`Ctrl+C` 在 parent 後 child 終止 —— 把 PoC 結論記到 `docs/decisions.md` D-027 追記段（資料證明 sidecar Python subprocess.Popen 路徑可行）

## 2. `qdrant_supervisor` happy / degraded path（TDD red → green，Decision 2: Reuse-first probe，永不重複 spawn / Decision 5: binary 找不到 → log warning + degraded，不 raise / Decision 6: probe poll budget 10s，timeout 視為 degraded）

- [x] 2.1 [P] RED test：`sidecar/tests/test_qdrant_supervisor.py` — pytest 測四 scenario：(a) probe 6333 回 2xx → `maybe_spawn_qdrant()` 回 `None` 不 spawn（兌現 Sidecar-managed Qdrant child process Requirement「Spawn skipped when Qdrant already reachable」scenario）；(b) probe 失敗 + binary 存在 → `subprocess.Popen` mock 被叫一次、env 帶 `QDRANT__STORAGE__STORAGE_PATH`、return Popen handle（兌現 spec scenario「Spawn happens when port 6333 is unreachable」）；(c) binary 路徑都不存在 → log warning + 回 None 不 raise（兌現 spec scenario「Binary not found degrades to fallback」）；(d) probe poll 10s timeout → child `terminate()` 被叫 + 回 None（兌現 spec scenario「Spawn timeout terminates orphaned child」）
- [x] 2.2 [P] RED test：`sidecar/tests/test_qdrant_supervisor.py` 補 binary 解析測 — `CODEBUS_QDRANT_BIN` 設絕對路徑優先；fallback `~/.codebus/bin/qdrant{.exe}`；Windows / POSIX 副檔名差異（兌現 spec Requirement 文字「resolved in the following order」）
- [x] 2.3 GREEN：[P] `sidecar/src/codebus_agent/qdrant_supervisor.py` 實作 `maybe_spawn_qdrant(parent_pid: int | None) -> Popen | None`、`_resolve_binary_path()`、`_resolve_storage_paths()`、`_probe_reachable(timeout=0.5)`、`_poll_until_ready(timeout=10, interval=0.2)`。2.1 + 2.2 全綠
- [x] 2.4 RED test：補 storage env 對齊測 — 跑 `start-qdrant.ps1` 解析的 `QDRANT__STORAGE__STORAGE_PATH` 與 `_resolve_storage_paths()` byte-equivalent；同 `CODEBUS_QDRANT_STORAGE` env 下兩者結果相等（兌現 spec scenario「Storage env vars match dev tool resolution」+ Decision 4: storage env 完全沿用 `start-qdrant.ps1`）
- [x] 2.5 GREEN：補完 `_resolve_storage_paths()` 與 `start-qdrant.ps1` 對齊。2.4 全綠

## 3. Three-path cleanup（TDD red → green，Decision 3: 三層 cleanup —— atexit / parent_pid watchdog / OS SIGTERM）

- [x] 3.1 RED test：`sidecar/tests/test_qdrant_supervisor.py` 補 cleanup 測 — (a) `cleanup_qdrant_child(handle)` 對活著的 child 呼 `terminate()` + `wait(5)`，timeout 後 `kill()`；(b) 對已死 child（`poll()` 回 0）no-op 不爆；(c) 同一 handle 連叫兩次 cleanup 第二次 no-op（兌現 spec Requirement「Qdrant child process supervision lifecycle」+ scenario「Cleanup is idempotent under multiple exit paths」）
- [x] 3.2 GREEN：`qdrant_supervisor.py` 補 `cleanup_qdrant_child(handle)` 函式 + `register_cleanup(handle)` 把 handle 註冊進 `atexit`。3.1 全綠
- [x] 3.3 RED test：`sidecar/tests/test_watchdog.py`（新或補既有）— `watch_parent` 偵測 parent 消失 → 在 `os._exit(0)` 之前呼 cleanup hook（mock `os._exit` 攔截）（兌現 spec scenario「Tauri parent exit triggers child termination via watchdog」）
- [x] 3.4 GREEN：改 `sidecar/src/codebus_agent/watchdog.py` 的 `watch_parent` 加 `on_exit: Callable[[], None] | None = None` 參數；偵測到 parent 消失 → call on_exit (best-effort) → `os._exit(0)`。3.3 全綠
- [x] 3.5 RED test：`test_qdrant_supervisor.py` 補 SIGTERM scenario — `signal.signal(SIGTERM, ...)` handler 觸發 cleanup（POSIX；Windows 可 skip / 用 `CTRL_BREAK_EVENT` 對等驗證）
- [x] 3.6 GREEN：`qdrant_supervisor.py` 補 signal handler 註冊（platform-aware：POSIX `SIGTERM` / Windows `CTRL_BREAK_EVENT`）

## 4. Sidecar `run()` 整合（Decision 1: Sidecar 擁有 Qdrant lifecycle）

- [x] 4.1 RED test：`sidecar/tests/test_main_run.py` 補 spawn-after-handshake 測 — `run()` 跑 mock 路徑（mock subprocess + handshake stdout）→ 確認 `maybe_spawn_qdrant` 在 handshake emit 之後、`asyncio.run(_serve(...))` 之前被呼（兌現 spec scenario「Spawn never blocks sidecar startup」+ Requirement 文字「after the sidecar's handshake JSON line has been emitted ... and BEFORE asyncio.run」）
- [x] 4.2 GREEN：改 `sidecar/src/codebus_agent/api/main.py` `run()`：`auth.generate_token()` 之後 → `handshake.emit()` 在 `_serve` 內部 → 在進 `_serve` 之前 / `_serve` 開頭呼 `maybe_spawn_qdrant(parent_pid)` + `register_cleanup(handle)` + 把 handle 傳給 `_serve` 的 watchdog `on_exit` hook
- [x] 4.3 GREEN：改 `_serve` 把 `parent_pid` 對應的 watchdog `watch_parent` 帶 `on_exit=lambda: cleanup_qdrant_child(handle)`（連動 3.4 簽章變更）

## 5. Sidecar baseline + 文件 + commit

- [x] 5.1 跑 `cd sidecar && uv run pytest` 全綠（既有 ~1041 + 新增 ≥ 12 case 共 0 regression）
- [x] 5.2 `pre-commit run --all-files` 全綠
- [x] 5.3 `docs/decisions.md` D-027 加追記段「sidecar-managed auto-spawn」+ 落地日期（apply session 結束日）+ 三層 cleanup 摘要 + degraded fallback 摘要 + `start-qdrant.ps1` 仍保留作 dev tool / fallback 的角色
- [x] 5.4 改 `CLAUDE.md` Qdrant 章節：from「user 自起 + script 命令」改為「sidecar 自動，degraded 時 fallback to script」描述；`.spectra/worktrees/` 章節若存在順帶確認 worktree 模式關閉狀態（已是當前 main 狀態）
- [x] 5.5 改 `sidecar/scripts/start-qdrant.ps1` / `start-qdrant.sh` 開頭註解：標明此 script 是 dev tool / degraded fallback；不再是 user 預設啟動方式
- [x] 5.6 重打 PyInstaller binary：`cd sidecar && uv run pyinstaller codebus-sidecar.spec --noconfirm` → 確認 mtime 比 main HEAD 新

## 6. End-to-end manual smoke

- [ ] 6.1 手動 e2e（happy path，reuse-first probe 走 spawn 分支）：(a) 殺光所有 Qdrant 進程 + 確認 6333 沒人 listen；(b) `cargo tauri dev`；(c) 觀察 sidecar stdout 出現「Qdrant spawn」/ ready log；(d) 進 onboard / entry / scan / generate 全跑通；(e) 結束 cargo tauri dev → 看 Qdrant 進程也沒了
- [ ] 6.2 手動 e2e（reuse 分支）：(a) 先手動 `pwsh sidecar/scripts/start-qdrant.ps1` 起 Qdrant；(b) `cargo tauri dev`；(c) 觀察 sidecar log 出現「Qdrant already reachable, reusing」；(d) 結束 cargo tauri dev → 手動起的 Qdrant **仍存活**（reuse 路徑不應 cleanup 別人的 child）
- [ ] 6.3 手動 e2e（degraded 分支）：(a) 暫時 rename `~/.codebus/bin/qdrant.exe` → `qdrant.exe.bak`；(b) `cargo tauri dev`；(c) 觀察 sidecar log warning + sidecar 仍啟動；(d) `/healthz` 報 `dependency.qdrant: "unreachable"`；(e) UI 不會卡死（onboard 仍能進，但 KB 操作會 503 + 對應 banner）
- [ ] 6.4 cross-platform PoC（macOS / Linux）：[P] 跑 1.1 + 6.1~6.3 happy path（**留 manual TODO**：本 session 在 Windows 主機；macOS / Linux 待實機環境）
