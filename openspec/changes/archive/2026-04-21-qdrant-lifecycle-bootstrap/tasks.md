## 1. 前置 scaffolding（奠基 spec「Qdrant client wrapper module」）

- [x] 1.1 建立 package 目錄與空 `__init__.py`：`sidecar/src/codebus_agent/kb/__init__.py` 與對應 `sidecar/tests/kb/__init__.py`；`pyproject.toml` 的 package discovery 已涵蓋 `src/codebus_agent/**`，確認不需改設定即可 import。
- [x] 1.2 為 spec「Qdrant client wrapper module」建 `sidecar/src/codebus_agent/kb/qdrant_client.py` 骨架：模組 docstring 引本 change + D-027，宣告 `resolve_url` / `probe` / `build_client` / `ensure_collection` / `QdrantCollectionSchemaError` 的 signature + `raise NotImplementedError`，不含邏輯。這支骨架存在是為了讓後續 TDD task 的 import 生效。
- [x] 1.3 為 spec「Qdrant client wrapper module」寫守門測試 `tests/kb/test_no_direct_sdk_import.py`：掃 `sidecar/src/codebus_agent/` 下非 `kb/` 檔案，assert 無 `import qdrant_client` / `from qdrant_client`。對應 design「Client 生命週期：single async client，app state 常駐」的邊界。

## 2. URL 解析：spec「CODEBUS_QDRANT_URL resolution has a single source of truth」＋ design「URL config：單一 helper，env + default」

- [x] 2.1 [P] 在 `sidecar/tests/kb/test_qdrant_client.py` 寫 `resolve_url` 三組失敗測試（explicit 覆蓋 env、env 覆蓋 default、default fallback）— 所有案例依賴 spec「CODEBUS_QDRANT_URL resolution has a single source of truth」的 scenario。先跑 `uv run pytest tests/kb/test_qdrant_client.py -k resolve_url`，確認三題皆 red。
- [x] 2.2 `codebus_agent.kb.qdrant_client.resolve_url(override)` 實作最小邏輯把 2.1 轉綠：`override or os.environ.get("CODEBUS_QDRANT_URL") or DEFAULT`；`DEFAULT = "http://127.0.0.1:6333"`。
- [x] 2.3 `codebus_agent/healthz.py`：刪除私有 `_qdrant_url()`、改 delegate 到 `kb.qdrant_client.resolve_url()`，兌現 scenario「healthz CLI uses the shared resolver」。`sidecar/tests/test_healthz_cli.py` 新增斷言：`CODEBUS_QDRANT_URL=<sentinel>` 時 CLI 輸出 `dependencies.qdrant.detail` 包含 sentinel。

## 3. 實作 spec「Qdrant connection probe」＋ design「Probe 時 timeout：1 秒」「Probe 失敗不 log 原始 exception」

- [x] 3.1 [P] 在 `tests/kb/test_qdrant_client.py` 為 spec「Qdrant connection probe」新增 TDD 群組：
  - `probe` 對 `readyz` 200 回 `DependencyStatus(ok=True, detail=url)`
  - `probe` 對 unbound loopback port 回 `ok=False`、detail 含 URL 與 exception type 名
  - `probe` 對非 200 response 回 `ok=False`
  - `probe` 不 raise
  - `probe` detail 不含 exception `str()` 內文（寫一題把 sentinel 放到 fake exception 的 message，驗證不出現在 detail）—兌現 design「Probe 失敗不 log 原始 exception」

  測試用 stub HTTP server（`http.server.HTTPServer` thread 或 `respx` 也可）涵蓋 200 / 500 情境；不可達 case 用已 bind 但立即 close 的 ephemeral port。timeout 以 design「Probe 時 timeout：1 秒」為準。
- [x] 3.2 `kb.qdrant_client.probe(url, timeout_seconds=1.0)` 實作：`urllib.request.urlopen(f"{url}/readyz", timeout=…)`；catch `URLError / TimeoutError / ConnectionError / OSError`；組 `DependencyStatus`。與 `healthz.py` 原有邏輯比對、語義對齊。
- [x] 3.3 `codebus_agent/healthz.py` 的 `_check_qdrant` 改 delegate 到 `kb.qdrant_client.probe`，刪 duplicated 邏輯。`tests/test_healthz.py` 既有 degraded case 仍須過。

## 4. 實作 spec「Async Qdrant client lifecycle bound to FastAPI app」＋ design「Client 生命週期：single async client，app state 常駐」

- [x] 4.1 [P] 在 `tests/kb/test_qdrant_client.py` 為 spec「Async Qdrant client lifecycle bound to FastAPI app」寫 `build_client(url)` TDD：
  - 回傳物件型別為 `AsyncQdrantClient`
  - 建構期間不做網路 I/O（用 `socket.socket` monkeypatch 或在 Qdrant 不可達的 port 執行、計時 < 1s）
  - `await client.close()` 不 raise
- [x] 4.2 實作 `build_client(url: str) -> AsyncQdrantClient`：單行包裝 + docstring 引 D-027。
- [x] 4.3 [P] 在 `tests/test_create_app.py`（新檔或併入既有 api test）驗證 spec「Async Qdrant client lifecycle bound to FastAPI app」全部四個 scenario：
  - `create_app(bearer_token=..., qdrant_url=None)` 後 `app.state` 無 `qdrant_client` 或為 `None`
  - `create_app(bearer_token=..., qdrant_url="http://127.0.0.1:6333")` 後 `app.state.qdrant_client` 為 `AsyncQdrantClient`
  - 即使 URL 指向不可達 endpoint，`create_app` 也在 1 秒內回（construction 非 blocking）
  - 用 `TestClient` 進 shutdown lifespan 後，`AsyncQdrantClient.close()` 被呼叫恰好一次（用 `AsyncMock` 替換 client）
- [x] 4.4 修改 `codebus_agent/api/__init__.py`：
  - `create_app` 加 `qdrant_url: str | None = None` kwarg
  - URL 給定 → `app.state.qdrant_client = build_client(qdrant_url)`
  - `app.on_event("shutdown")` 呼 `await app.state.qdrant_client.close()`
  - URL 給定時自動把 `kb.qdrant_client.probe` bind 成 dependency check 塞進 `dependency_checks["qdrant"]`

## 5. 實作 spec「Runtime health endpoint reflects Qdrant connectivity」＋ design「`/healthz` response schema：沿用現有 `HealthReport` 結構」

- [x] 5.1 [P] 在 `tests/test_healthz.py` 為 spec「Runtime health endpoint reflects Qdrant connectivity」加三組 runtime case：
  - qdrant_url 指到 mock reachable URL → `/healthz` 200 + `status=ok` + `dependencies.qdrant.ok=true`
  - qdrant_url 指到不可達 URL → `/healthz` 200 + `status=degraded` + `dependencies.qdrant.ok=false`
  - `create_app` 未給 qdrant_url → response body 的 `dependencies` 不含 `qdrant` 鍵
- [x] 5.2 讓 5.1 轉綠：4.4 的 dependency_checks 注入已足夠，只需確認 `collect()` 組出的 shape 符合 scenario 斷言；若 shape 有缺補 `health.py` 的 `to_dict()`（預期不需改）。

## 6. 實作 spec「Sidecar entry point wires Qdrant URL into app factory」＋「Sidecar startup remains available when Qdrant is unreachable」＋ design「Startup policy：degraded-but-alive，不 fail-fast」

- [x] 6.1 [P] 在 `tests/test_main_run.py`（新檔或併入既有 entry point test）為 spec「Sidecar startup remains available when Qdrant is unreachable」與「Sidecar entry point wires Qdrant URL into app factory」加測試：
  - monkeypatch `os.environ["CODEBUS_QDRANT_URL"]="http://custom.invalid:7000"`，spawn sidecar subprocess 直到 handshake 印出，接著 `GET /healthz` 驗 `dependencies.qdrant.detail` 包含 `custom.invalid:7000`
  - 不設 env → `detail` 包含 `127.0.0.1:6333`
  - 不可達 URL 下，handshake 在 startup 正常 budget 內印出（衡量 t(spawn)→t(handshake) < 3 秒）、`/healthz` 回 degraded —這題正面兌現 design「Startup policy：degraded-but-alive，不 fail-fast」
- [x] 6.2 `codebus_agent/api/main.py` `run()` 把 `qdrant_url = kb.qdrant_client.resolve_url()` 傳給 `create_app`。確認 degraded case 不讓 `create_app` 拋錯（4.3 已保證 construction non-blocking）。
- [x] 6.3 回歸既有 `tests/test_healthz_cli.py` / `tests/test_handshake*.py` — 確認 `--healthz` exit code 仍為 0 even when degraded；兌現 spec「Sidecar entry point wires Qdrant URL into app factory」的「--healthz CLI shares the same resolver」scenario。

## 7. 實作 spec「Idempotent collection provisioning」＋ design「`ensure_collection` 不符合時的行為：raise，不 auto-migrate」「`ensure_collection` 不做 payload index」

- [x] 7.1 [P] 在 `tests/kb/test_qdrant_client.py` 為 spec「Idempotent collection provisioning」寫 `ensure_collection` TDD：
  - collection 不存在 → 建立（用 `AsyncMock` client stub `get_collection` raise `ValueError` / `UnexpectedResponse`、`create_collection` 被呼叫一次）
  - 既存同 schema → no-op（`get_collection` 回符合的 config、`create_collection` 不被呼叫）
  - 既存 schema 不符（vector size 或 distance）→ raise `QdrantCollectionSchemaError`、`create_collection` 不被呼叫、`delete_collection` 亦不被呼叫—兌現 design「`ensure_collection` 不符合時的行為：raise，不 auto-migrate」
  - 不呼叫任何 payload index API（驗證 mock client 的 `create_payload_index` 未被動過）—兌現 design「`ensure_collection` 不做 payload index」
- [x] 7.2 實作 `ensure_collection(client, name, vector_size, distance="Cosine")`：try `get_collection` → 若 raise collection-missing 就 `create_collection`；拿到既存 config → compare vector size + distance → 不符 raise。`QdrantCollectionSchemaError` 作為 module-level exception class（繼承 `RuntimeError`）。

## 8. Spec / docs 連動

- [x] 8.1 更新 `openspec/specs/qdrant-client/spec.md` Purpose（目前是 `TBD - created by archiving change 'm1-power-on'`）→ 一句話說明「定義 Qdrant 的 launch recipe、smoke test 與 sidecar runtime lifecycle」。注意：本 change 的 delta 歸 Purpose 補完一併在 `spectra archive` 時吃掉，但 archive 流程 `MODIFIED Requirements` 不吃 Purpose prose，`openspec/specs/qdrant-client/spec.md` 需要在 archive 後手動補 Purpose — 本 task 先在 change 目錄的 proposal `Impact` 段落記錄此 follow-up，並在 `tasks.md` 打勾。
- [x] 8.2 `docs/module-2-kb-builder.md §十三` 的 P0 第一項「Qdrant client wrapper（connect / ensure_collection / upsert / query）0.5d」：在該行備註「connect / ensure_collection 由 change `qdrant-lifecycle-bootstrap` 交付；upsert / query 屬 Module 2 range」。不刪該行（那是 Module 2 spec 的本份工作分解）。
- [x] 8.3 `docs/decisions.md` D-027「連動更新」checklist：在 `docs/module-2-kb-builder.md §三` 行中立化那一項附註「runtime lifecycle 已在 change `qdrant-lifecycle-bootstrap` 實作」；本 change 不新增 D-XXX。

## 9. 驗收

- [x] 9.1 `uv run pytest tests/kb/ tests/test_healthz.py tests/test_healthz_cli.py` 全綠；`tests/qdrant/test_smoke.py` 在 Qdrant 不通時仍 skip 而非 error。
- [x] 9.2 `pre-commit run --all-files` 全綠（ruff / pyright）。
- [x] 9.3 `spectra validate qdrant-lifecycle-bootstrap` + `spectra analyze qdrant-lifecycle-bootstrap --json` 無 Critical / Warning finding。
- [x] 9.4 人工 smoke：`bash sidecar/scripts/start-qdrant.sh` + `uv run python -m codebus_agent.api.main --healthz` 印 `status=ok`；`pkill qdrant`（或 `taskkill /F`）後重跑印 `status=degraded`、exit 0；`uv run python -m codebus_agent.api.main` 不因 Qdrant 不在而 fail，`/healthz` 回 degraded。
