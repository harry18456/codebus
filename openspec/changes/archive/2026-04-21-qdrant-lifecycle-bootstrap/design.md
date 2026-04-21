## Context

M1「power-on」封存後，sidecar 對 Qdrant 只有「外圍存在」：`sidecar/scripts/start-qdrant.{sh,ps1}`、`docker-compose.qdrant.yml`、`--healthz` CLI 自檢、`tests/qdrant/test_smoke.py`。runtime `create_app()` 的 `dependency_checks` 預設是空 dict，`/healthz` 永遠回 `status=ok` 即便 Qdrant 沒起來。`qdrant_client` 套件已在 `pyproject.toml`，但 sidecar 原始碼沒有任何 import — 只有 smoke test 直接用。

本 change 要把 Qdrant 升格成 sidecar 的一等 runtime dependency：封裝 client、注入 `/healthz`、統一 `CODEBUS_QDRANT_URL` 入口、提供 `ensure_collection` 給 Module 2。範圍刻意限制在 **lifecycle + visibility**，不碰 chunk / embed / payload schema（那是後續 Module 2 change）。

關聯決策：D-027（binary 主路徑）、D-local-5（PyInstaller entry）、D-009（本地優先）、D-012（自寫 ReAct 不依賴 framework 動態能力）。

## Goals / Non-Goals

**Goals:**

- Qdrant 可觀測性：runtime `GET /healthz` 的 `dependencies.qdrant` 欄位真實反映連通性（D-027 關鍵不變式 5）。
- 單一 URL config 入口：`CODEBUS_QDRANT_URL` 的解析邏輯只活在一個 helper，不在 `healthz.py` / kb module / test fixture 重複。
- 提供 `ensure_collection(name, vector_size)` 給 Module 2 P0 作 build pipeline 底層。
- Qdrant 不可達時 sidecar 照常起來（degraded，不是 fail）— 使用者可能還沒下載 binary / Qdrant 還在 warm-up。
- `--healthz` CLI 與 runtime `/healthz` **共用** qdrant probe 邏輯，結果一致。

**Non-Goals:**

- 不自動 spawn Qdrant process（D-027 關鍵不變式 1、3）。
- 不實作 Module 2 的 chunk / embed / upsert / dedup / payload index。
- 不加 Qdrant auth / TLS 支援（localhost only，M1 `security.md §3` 限制）。
- 不做 collection schema migration / versioning — Phase 3。
- 不升版 `qdrant-client`、不改 docker compose / 啟動腳本。

## Decisions

### Startup policy：degraded-but-alive，不 fail-fast

**選擇**：Qdrant 不可達時 sidecar 正常 bind port、emit handshake、起 uvicorn；`/healthz` 回 `status=degraded, dependencies.qdrant.ok=false`。

**替代**：
- (A) fail-fast — sidecar 見 Qdrant 不通就 exit 非零。Tauri 會在 stdout handshake 拿不到 JSON 後超時、彈錯誤 modal。
- (B) lazy probe — 不在 startup 檢查，延到第一個 KB 操作。`/healthz` 永遠回 ok。

**理由**：
- D-009「本地優先」+ D-027 使用者獨立下載 binary —「第一次跑 sidecar 時 Qdrant 可能還沒裝」是 expected flow，不該 fail。Trust Layer 希望讓使用者看到「sidecar 已連上、Qdrant 尚未就緒」這個 intermediate state，而非整包 crash。
- `--healthz` CLI 已經走此語義（`exit 0 even when degraded`）— runtime 對齊才能讓「UI 顯示的狀態」跟「CI 自檢」不分歧。
- (B) 會讓 Tauri shell 沒有 pre-flight 能偵測到 Qdrant 問題，Trust Layer 的「即時反饋」體驗打折。

### Client 生命週期：single async client，app state 常駐

**選擇**：`create_app()` 若拿到 `qdrant_url`，在 app factory 就實例化一個 `AsyncQdrantClient`，塞進 `app.state.qdrant_client`；FastAPI shutdown event 呼叫 `client.close()`。`ensure_collection` / 之後的 KB API 從 `app.state` 拿。

**替代**：
- (A) per-request client — 每次 call 開新 `QdrantClient`。
- (B) module-global singleton — `codebus_agent.kb.qdrant_client._client`，用 lazy init + lock。

**理由**：
- (A) 建 client 便宜但 HTTP 連線池不重用，Module 2 批量 embed/upsert 時 overhead 變 hot path。
- (B) module-level singleton 與 FastAPI app factory 的測試隔離衝突 — `create_app()` 同 process 起多次（測試常見）會共用同一 client，bearer token 也一樣。`app.state` 是 FastAPI 原生、per-app scope，天然隔離。
- `AsyncQdrantClient` 支援 `async with` / `.close()`，與 uvicorn shutdown event 對齊。

### URL config：單一 helper，env + default

**選擇**：新增 `codebus_agent.kb.qdrant_client.resolve_url()`，順序為 (1) 呼叫者顯式傳入 → (2) `CODEBUS_QDRANT_URL` env → (3) hardcoded `http://127.0.0.1:6333`。`healthz.py` 和 `test_smoke.py` 的重複解析邏輯移除、改呼叫此 helper。

**替代**：用 pydantic `BaseSettings` 建 config class。

**理由**：目前只有一個 URL 變數，引入 settings class 是過度設計。單一 helper 足夠，之後若出現第二第三個 env（Qdrant API key、gRPC port）再升級成 settings class。

### `ensure_collection` 不符合時的行為：raise，不 auto-migrate

**選擇**：collection 已存在 → 檢查 vector size + distance → 不符 raise `QdrantCollectionSchemaError`；相符 → no-op；不存在 → create。

**替代**：
- (A) drop + recreate — 靜默銷毀資料。
- (B) warn + return — 呼叫者自己決定要不要炸。

**理由**：
- Module 2 `docs/module-2-kb-builder.md §十 "Workspace 已有同名 collection"` 已定義「預設 drop 再 build」的語意，但那是 **KB rebuild** 的明確路徑、由 Module 2 層決定，不該由 `ensure_collection` 這種 low-level helper 偷偷做。
- Schema drift 通常代表 bug（embedding provider 換了但 collection 沒 rebuild）— raise 讓呼叫端決定。(A) 會讓 drift 變成靜默資料流失。

### `ensure_collection` 不做 payload index

本 change 只做 vector 設定的 idempotent ensure。Payload index（`text_hash`、`related_stations` 等）由 Module 2 build pipeline 在 ensure collection 後自己呼叫 Qdrant API 建。理由：payload schema 屬 Module 2 range，提早放進 ensure helper 會把介面綁死。

### `/healthz` response schema：沿用現有 `HealthReport` 結構

**選擇**：不改 `health.py` 的 `HealthReport` / `DependencyStatus` dataclass；只是從「dependency_checks 預設空 dict」變成「runtime 依 `qdrant_url` 是否給定、自動塞 qdrant probe」。response shape：

```json
{
  "status": "ok" | "degraded",
  "dependencies": {
    "qdrant": { "ok": true, "detail": "http://127.0.0.1:6333" }
  }
}
```

與 `--healthz` CLI 輸出 schema 一致（本來就共享 `HealthReport.to_dict()`）。

**對 Tauri 端的影響**：`tauri/src-tauri/src/sidecar.rs` 現行 `/healthz` 只檢 200 — 新欄位是 additive，不打破既有 contract。Tauri 後續要展示 Qdrant 狀態時再 parse `dependencies.qdrant.ok`。

### Probe 時 timeout：1 秒

沿用 `healthz.py` 現行的 `timeout=1.0` 秒。不同值會造成 CLI 與 runtime behaviour 分歧，除非有實測理由，保持一致。`/healthz` 呼叫預期頻率不高（Tauri startup polling + 前端偶爾刷新），1 秒阻塞可接受。

### Probe 失敗不 log 原始 exception

`DependencyStatus.detail` 只帶「URL + exception type name」（沿用 `healthz.py` 目前格式），**不** 印完整 stack 或 exception message。理由：`detail` 會回給前端 / stdout，URL 已經是 localhost 不敏感，但未來若 URL 變設定檔帶 token，stack 可能洩露。保守設計。

## Risks / Trade-offs

- **[風險] degraded-mode startup 讓「Qdrant 忘了開」變成 silent foot-gun** → **緩解**：(1) Tauri shell 之後可在 `sidecar_ping` 後 parse `dependencies.qdrant.ok` 秀醒目提示（本 change 不做，但 schema 已預備）。(2) `docs/dev-setup.md` 後續需補「首次跑必先跑 `start-qdrant`」指引（D-027 連動清單既有項目，本 change 不推進）。
- **[風險] `AsyncQdrantClient` 在測試 teardown 若沒 close 會留 socket** → **緩解**：FastAPI `@app.on_event("shutdown")` 呼叫 `client.close()`；測試 fixture 用 `TestClient` 的 context manager 自動觸發 shutdown。
- **[風險] `qdrant-client` 套件版本跳 minor 時 API 改名**（smoke test 已撞過 `.search → .query_points`） → **緩解**：wrapper 把 Qdrant SDK 呼叫收斂在一個模組，版本升級 blast radius 縮小到單檔；`pyproject.toml` 鎖 minor。
- **[Trade-off] degraded ≠ fail，CI 不會因 Qdrant 沒起來 red** → 可接受。`--healthz` 在 CI 的語義一直就是「binary 跑得起來」而非「整條 stack 通」；smoke test（`test_smoke.py`）才是整條 stack 的驗證，它會自動 skip 在 Qdrant 不通時 — CI 若要把 skip 視為失敗，應改 CI 設定（`pytest --strict-markers` + Qdrant service），不是改 sidecar startup policy。
- **[Trade-off] URL helper 不用 pydantic settings** → 可接受。現行只有一個變數，YAGNI。第二個 Qdrant 相關 env 出現時重構。
