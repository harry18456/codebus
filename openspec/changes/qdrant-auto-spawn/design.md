## Context

D-027 archive（2026-04-19）拍板「Qdrant local binary 取代 Docker Compose 為主路徑」，但只更新了 `docs/decisions.md` + 加了 `sidecar/scripts/start-qdrant.{ps1,sh}` dev tool —— **啟動時機**留給 user 手動。`sidecar/api/__init__.py` `create_app(qdrant_url=...)` 只是被動接 URL；無 reachable 時 `_qdrant_probe` 回 `unreachable`、`/healthz` 報 degraded、`/kb/build` 後續走 `KnowledgeBase.build` → 連線 fail → SSE error event。

當前 user 經驗：完成 onboarding wizard → 進 entry → 點「+ 開新 codebase」 → scan 通 → 點「+ 產生 tutorial」（觸 `/kb/build`） → 1~2s 內前端「internal sidecar error」。除非他們先讀 README / CLAUDE.md 找到「Qdrant 本地 binary（D-027）」段、跑 `pwsh sidecar/scripts/start-qdrant.ps1`，否則整條 demo flow 就斷在第一個會打 KB 的步驟。

Phase 7 demo 目標是「桌面 user 下載 app → cargo tauri dev → 跑通 onboard → entry → scan → tutorial」。手動 Qdrant 是違反 desktop UX 直覺的步驟（user 不會去學 vector DB 是什麼），也是這條 flow 唯一沒被自動化的依賴。本 change 把 Qdrant 子程序生命週期收進 sidecar，符合「sidecar 是 Qdrant 唯一 consumer，由它負責 lifecycle」的最小職責邊界。

## Goals / Non-Goals

**Goals:**

- sidecar 啟動後自動確保 Qdrant on `127.0.0.1:6333` 可用（spawn 或 reuse）
- 不增加 user 手動步驟；保留 `start-qdrant.ps1` 作 dev tool / 文件範例 / degraded fallback
- Qdrant child 隨 sidecar 退出而終止（三層 cleanup：atexit / watchdog / SIGTERM）
- 並發 sidecar 不互相 spawn 競爭：第二個 sidecar 看到 6333 已通就 reuse 不 spawn
- binary 找不到 → graceful degraded（沿用 D-027 「degraded-but-alive」不變式），sidecar 仍能啟動讓 healthz 回 not-configured / unreachable，user 不會被擋在 wizard 外
- 三平台支援（Windows `qdrant.exe` / macOS / Linux `qdrant`）

**Non-Goals:**

- 不取代 `start-qdrant.{ps1,sh}` —— 仍是 dev tool / 文件範例 / degraded 自救路徑
- 不從 Tauri Rust 層 spawn —— 雙處 race + sidecar 已是 Qdrant 唯一 consumer，職責劃分最簡
- 不 bundle Qdrant binary 進 PyInstaller —— Qdrant ~83MB，binary 翻倍；user 從 GitHub release 取仍是預期路徑
- 不偵測 Qdrant version mismatch —— P1+；本 change 假設 v1.17+ 相容
- 不做 Qdrant healthcheck retry / restart —— probe 10s timeout 即視為 degraded，不 restart child（避免 race + restart 風暴）
- 不支援 Qdrant alternate port —— 只 6333；多 instance 場景留 P1+
- 不改 `start-qdrant.ps1` 的 env var 契約 —— 完全沿用 `QDRANT__STORAGE__STORAGE_PATH` 等，避免 dev tool / sidecar 分歧

## Decisions

### Decision 1: Sidecar 擁有 Qdrant lifecycle，不從 Tauri Rust spawn

**選擇**：把 Qdrant 子程序 spawn / poll / cleanup 邏輯放進 sidecar Python（新模組 `qdrant_supervisor.py`），由 `sidecar/api/main.py` 的 `run()` 在 handshake emit 之後、`asyncio.run(_serve(...))` 之前呼叫。

**為何不選**：

- (X) Tauri Rust 層 spawn：要在 `tauri/src-tauri/src/sidecar.rs` 之外多開一條 spawn 路徑，需重複實作 binary 解析 + watchdog + cleanup；且 Rust 層 spawn 後要把 Qdrant URL 傳給 sidecar handshake 之後 → 雙處 race 加複雜
- (X) 兩處同時管：spawn from Tauri + cleanup from sidecar，子程序歸屬不清楚，crash recovery 邏輯難寫

**Rationale**：sidecar 是 Qdrant 唯一 consumer（`/kb/build` / `/qa` / `/scan` 皆透過 `app.state.qdrant_client`）；當 sidecar 不 ready，Qdrant 也不該活著。把 lifecycle 收在同一 process 邊界 = 最少 surface + 自然 cleanup（sidecar 死 → child 死 → resources 釋放）。

**Invariant**：

- Qdrant child 的 parent 永遠是 sidecar process（PID tree 一階）
- 不從 Rust 層直接 reach child PID；只透過 sidecar HTTP 端介接 reach Qdrant

### Decision 2: Reuse-first probe，永不重複 spawn

**選擇**：spawn 前先打 `GET http://127.0.0.1:6333/healthz`（500ms timeout）；任何 2xx 回應就 log info 並 reuse。spawn 後 poll 同一 endpoint 直到 200（10s budget）。

**為何不選**：

- (X) 用 PID file lock 偵測：增加 OS 相依、cleanup 需處理 stale lock；HTTP probe 能複用既有 `_kb_qdrant.probe()`
- (X) 強制每個 sidecar 自己 spawn：N 個 sidecar 起 N 個 Qdrant，port conflict + storage 衝突
- (X) 完全信任 user 預先起：違反本 change 主目的

**Rationale**：HTTP probe 對 Qdrant runtime 唯一狀態（reachability）夠了；不需要持久化 PID。並發 sidecar 場景常見：dev / test 同時跑、PyInstaller 打包 sidecar 與 dev sidecar 並存。reuse-first 讓多 sidecar 共用同一 Qdrant 是預設行為。

**Invariant**：

- 同一 host 任何時刻至多一個 Qdrant on 6333
- sidecar A spawn 的 child 不會被 sidecar B 終止（B reuse 而非接管）

### Decision 3: 三層 cleanup —— atexit / parent_pid watchdog / OS SIGTERM

**選擇**：sidecar 對每個 spawn 出來的 Qdrant Popen handle 註冊三條 cleanup path：

1. **`atexit.register`**：sidecar 正常 `sys.exit(0)` / 例外被 uvicorn 捕獲後正常結束 → atexit hook fire → `_terminate_qdrant_child(handle)`
2. **`watchdog.watch_parent`**：sidecar 偵測 Tauri parent 消失 → 在 `os._exit(0)` **之前**呼 cleanup hook（須改 `watch_parent` 簽章接 `on_exit: Callable | None`）
3. **OS SIGTERM**（POSIX）/ `CTRL_BREAK_EVENT`（Windows）：透過 `signal.signal(SIGTERM, ...)` handler 觸發 cleanup

`_terminate_qdrant_child` 內：先 `Popen.terminate()`（→ Qdrant 收 SIGTERM 自己 graceful flush WAL）→ `wait(timeout=5)`；timeout → `Popen.kill()`（SIGKILL）。

**為何不選**：

- (X) 只用 atexit：parent_pid watchdog 走 `os._exit(0)` 跳過 atexit chain（spec by design），Qdrant 會 orphan
- (X) 只用 watchdog：sidecar 自己 crash（uvicorn raise → main exception → exit）時 atexit fire；watchdog 沒被觸發 → 須兩條 path
- (X) `kill()` 直接：Qdrant 沒機會 flush，下次起來 WAL replay 會慢

**Rationale**：三層覆蓋三種退出 path 各自不可靠時的 fallback；先 graceful（5s 等）再 force kill 兼顧 data safety + 不卡死 sidecar shutdown。

**Invariant**：

- Tauri 死 → sidecar `--parent-pid` watchdog 5s 內偵測 → terminate Qdrant child → `os._exit(0)`
- 任何 cleanup path 必須 idempotent（多次呼叫不爆）—— 用 `Popen.poll()` 檢查 child 是否還活著

### Decision 4: storage env 完全沿用 `start-qdrant.ps1`

**選擇**：sidecar spawn child 時設置與 `start-qdrant.ps1` 完全相同的 env：

```python
env = {
    **os.environ,
    "QDRANT__STORAGE__STORAGE_PATH": storage_path,        # default ~/.codebus/kb
    "QDRANT__STORAGE__SNAPSHOTS_PATH": snapshots_path,    # default ~/.codebus/kb/snapshots
}
```

`storage_path` 解析順序：`$CODEBUS_QDRANT_STORAGE` → `~/.codebus/kb`（沿用）。

**為何不選**：

- (X) sidecar 自創 env var 名（如 `CODEBUS_QDRANT_DATA_DIR`）：dev tool / sidecar 兩條 path 各拿一份，user 改其一另一個沒生效
- (X) 改用 CLI flag：Qdrant v1.x 的 storage path 只認 env var（per `start-qdrant.ps1` 註解）

**Rationale**：dev tool 與 sidecar 必須 share 同一份 storage 配置；否則 user 切換兩條啟動方式時資料路徑不同步 → 看起來像「資料消失」。

**Invariant**：

- `start-qdrant.ps1` 與 sidecar auto-spawn 解析的 storage_path 必須**位元等同**（同一 user / 同一 env 下）

### Decision 5: binary 找不到 → log warning + degraded，不 raise

**選擇**：`maybe_spawn_qdrant()` 在 binary 不存在 OR poll 10s timeout 時 log `warning` 級別、回 `None`、sidecar 繼續啟動。`/healthz` 的 `qdrant` lane 自然會報 `unreachable`；前端透過該 lane 顯示 banner。

**為何不選**：

- (X) raise → sidecar 啟動失敗：違反 D-027「degraded-but-alive」與 m1-power-on `Sidecar starts and /healthz reports degraded` Requirement
- (X) silent return：user 不知道為什麼 KB build 503

**Rationale**：D-027 archive 的 graceful degraded 不變式不可破。降級的 user 仍能：(1) 走 onboarding 看 Trust Layer mockup / (2) 跑非 KB 功能 / (3) 看到 healthz banner 提示「需 Qdrant binary」 + 引到 dev tool script 的文件路徑。

**Invariant**：

- `maybe_spawn_qdrant()` 永遠回 `Popen | None`，從不 raise
- log warning level 必含「binary 解析路徑」+「fallback 指引（指向 `sidecar/scripts/start-qdrant.ps1`）」

### Decision 6: probe poll budget 10s，timeout 視為 degraded

**選擇**：spawn child 後每 200ms 打一次 `/healthz`，至多 50 次（10s）；任何一次 2xx 視為 ready 並回 Popen handle；50 次都失敗 → terminate child + log warning + 回 None（同 Decision 5 degraded path）。

**為何不選**：

- (X) 無限 retry：sidecar 永遠卡在啟動
- (X) 30s budget：sidecar 啟動延遲太久；Qdrant cold start 在 SSD ~1-3s 已綽綽有餘

**Rationale**：10s 對 Qdrant cold start 給夠 buffer（含 NVMe slow case）；又不至於拖累 sidecar handshake。Timeout 後 terminate 已 spawn 的 child（避免 orphan）+ degraded fallback。

**Invariant**：

- timeout 路徑必須 `terminate()` child，不允許 orphan Popen handle
- poll interval 200ms（可參數化但不暴露 CLI flag —— 屬實作細節）

## Risks / Trade-offs

- **Risk: Qdrant binary 在企業 Windows 環境被 SmartScreen / AV 阻擋** → Mitigation: log warning 內含 binary 路徑與 SmartScreen 解鎖步驟（Properties → Unblock）；degraded mode 仍可用，user 可手動跑 `start-qdrant.ps1` 走 fallback 路徑
- **Risk: 並發 sidecar 起跑時 race（兩個都看到 6333 不通、同時 spawn）** → Mitigation: 接受機率（dev / test 才會發生），第二個 child spawn 後 Qdrant 自己會 port conflict exit，第二 sidecar poll 失敗 → degraded；不增加 file lock 等複雜度
- **Risk: Tauri force-kill sidecar（不走 watchdog）** → Mitigation: Windows `taskkill /F` 不送 SIGTERM 給 child；Qdrant 變孤兒。接受此 edge case：next sidecar 啟動時 reuse 偵測會 reuse 孤兒 Qdrant（仍正確）；孤兒 Qdrant 沒 parent_pid 監控 → 下次 user 重啟 OS 才清。Phase 7 範圍可接受
- **Risk: Qdrant SIGTERM → graceful flush 5s 內未完成** → Mitigation: 5s 後 SIGKILL；有極小機率 WAL truncation，但 Qdrant 自己有 startup recovery；資料層風險可接受
- **Trade-off: sidecar 啟動時間增加 0.5~3s（cold spawn case）** → 接受；對比「demo 跑不通」的 cost 微不足道；reuse case 只增 ~50ms probe time
- **Trade-off: sidecar 進程樹從 1 層變 2 層** → ps / Activity Monitor 看起來多一個 qdrant.exe；屬預期行為，文件記錄即可

## Migration Plan

1. **Schema migration**：無 schema 變動；env var 沿用、HTTP API 不變
2. **既有 fixture / test**：sidecar 既有 ~1041 個 pytest 多用 mock，本 change 只在 `test_qdrant_supervisor.py` 觸 subprocess.Popen（mocked）；不影響其他測試
3. **既有 `start-qdrant.ps1` 行為**：完全不變；仍可手動跑作 dev tool / fallback；user 跑了 → reuse path 自然觸發
4. **rollback strategy**：本 change 純加（新模組 + sidecar `run()` 兩行）；rollback = revert commit；degraded fallback 保證 rollback 後 user 仍能用 `start-qdrant.ps1` 補
5. **Feature flag**：不需要；degraded fallback 已是 implicit kill switch（找不到 binary 就走原行為）

## Open Questions

- (apply 期解) `watch_parent` 簽章從 `() -> None` 擴成 `(on_exit: Callable | None) -> None` 或保留原簽章 + 用全域 `_cleanup_hooks: list[Callable]`？前者顯式、後者擴充性高 —— PoC 期決定
- (apply 期解) Qdrant `--config-path` 是否需要在 spawn 時帶？目前只設 env var，sidecar 是否需要 ship 一份 Qdrant config 模板（限制 RAM 用量、log level）—— 預設不帶，跑 default config，apply 期看 dev 體驗再評估
- (P1+) Qdrant version compatibility 偵測：spawn 前 `qdrant --version` 比對 sidecar 預期版本範圍；不符警告 —— 目前假設 v1.17+ 範圍內 forward compat
- (P1+) cross-platform PoC：macOS Activity Monitor / Linux `ps` 看 child 階層、SIGTERM 行為
