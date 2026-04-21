> 關聯決策：D-027（Qdrant standalone binary 主路徑；Docker Compose fallback）、D-local-6（M1 原決策，已被 D-027 翻轉）、D-009（本地優先）。
> 關聯步驟：`docs/implementation-plan.md` Step 7「Qdrant 本地跑 + Python client 連通」Step 14 前置條件。

## Why

M1「power-on」只交付了 Qdrant 的 **外圍設施**：`sidecar/scripts/start-qdrant.{sh,ps1}` 啟動腳本、`docker-compose.qdrant.yml` fallback、`--healthz` CLI 自檢、`tests/qdrant/test_smoke.py`。但 **sidecar runtime 本身** 目前：

1. `create_app()` 預設不注入任何 dependency check，runtime `GET /healthz` 無論 Qdrant 是否起來都回 `status=ok`（D-local-5 與 D-027 承諾的「`dependencies.qdrant` 欄位不因啟動方式改變而刪」尚未兌現於 runtime 路徑，只活在 `--healthz` CLI 分支裡）
2. sidecar source 內沒有任何 `QdrantClient` wrapper — 只有 smoke test 直接 `from qdrant_client import QdrantClient`
3. `CODEBUS_QDRANT_URL` 的讀取邏輯散落在 `healthz.py` 與 `test_smoke.py` 兩處（預設值 `http://127.0.0.1:6333`），沒有單一 source of truth
4. Module 2 KB Builder（`docs/module-2-kb-builder.md §十三` P0 第一項「Qdrant client wrapper」）依賴此基礎才能開工

這支 change 把 Qdrant 從「靠 side-channel 存在」升級成 sidecar 的一等 runtime dependency：帶 lifecycle 的 client、statup-time probe、runtime `/healthz` 可觀測性，並把 `CODEBUS_QDRANT_URL` 收斂到一個 config 入口。

## What Changes

- **新增** `codebus_agent.kb.qdrant_client` module：封裝 `QdrantClient` 建構、`CODEBUS_QDRANT_URL` 解析、connection probe、`close()` 釋放。預設走 HTTP；測試可 inject fake。
- **新增** `ensure_collection(name, vector_size, distance="Cosine")` helper：idempotent，collection 已存在則 no-op，vector 設定不符則 raise（避免隱性 schema drift）。給 Module 2 之後的 build pipeline 當底層。
- **修改** `codebus_agent.api.create_app()`：新增 `qdrant_url` kwarg（optional；`None` 代表不啟用 qdrant check，沿用 M1 語意）；若給定就自動注入 `dependencies.qdrant` check。
- **修改** `codebus_agent.api.main.run()` entry：讀 `CODEBUS_QDRANT_URL`（env 未設走預設 `http://127.0.0.1:6333`），把 URL 餵給 `create_app()`，讓 runtime `GET /healthz` 回 `{"status": "ok"|"degraded", "dependencies": {"qdrant": {...}}}`。
- **修改** `codebus_agent.healthz`：共用 `codebus_agent.kb.qdrant_client` 的 URL / probe 邏輯，移除重複。`--healthz` CLI 行為不變（exit 0、印 JSON、degraded 代表 Qdrant 不通）。
- **行為約束**：Qdrant 不可達 **不得** 阻止 sidecar 起來 — 與 D-009 一致，使用者可能還沒下載 binary。sidecar 照常 bind port、emit handshake，只是 `/healthz` 回 `status=degraded`。
- **行為約束**：sidecar **不自動 spawn Qdrant process**。啟動責任落在使用者 / `start-qdrant.{sh,ps1}`（D-027 關鍵不變式 1、3）。
- **修改** spec `openspec/specs/qdrant-client/spec.md`：Purpose 從 `TBD` 補完；新增 Runtime lifecycle 章節（client wrapper / health integration / degraded-mode startup / `ensure_collection`）。
- **修改** spec `openspec/specs/sidecar-runtime/spec.md`：`/healthz` 現行條款擴增 `dependencies` 欄位契約。

## Non-Goals

- **不** 做 Module 2 的 chunk / embed / upsert / dedup（`module-2-kb-builder.md §十三` P0 後續項）— 這支 change 只鋪地基。
- **不** 做 collection schema migration、versioning、跨 workspace 共用 collection — Phase 3。
- **不** 打包 Qdrant binary 進 PyInstaller（D-027 關鍵不變式 1）。
- **不** 讓 sidecar 監督 Qdrant process（spawn / restart / log capture）— 使用者用 `start-qdrant.{sh,ps1}` 管理，sidecar 只看 HTTP endpoint（D-027 關鍵不變式 3）。
- **不** 支援非 localhost Qdrant 的 auth（API key、TLS）— M1 `security.md §3` 限定 Provider 層才能出網；Qdrant 走 `127.0.0.1`，無 auth 需求。若未來加需求另開 change。
- **不** 實作 payload index、KBPayload schema（`module-2-kb-builder.md §三`）— 屬 Module 2 範疇。

## Capabilities

### New Capabilities

（none — 此 change 不新增 capability）

### Modified Capabilities

- `qdrant-client`: Purpose 從 TBD 補完；新增 runtime lifecycle（client wrapper、`/healthz` 整合、degraded-mode startup、`ensure_collection` helper）條款。既有 M1 launch script / smoke test 條款保留。
- `sidecar-runtime`: `/healthz` 契約補 `dependencies` 欄位（以 `qdrant` 為首個成員）；startup 行為明定 Qdrant 不可達不阻塞 sidecar 起動。

## Impact

- **Affected specs**:
  - `openspec/specs/qdrant-client/spec.md`（Purpose + Runtime lifecycle 條款）
  - `openspec/specs/sidecar-runtime/spec.md`（`/healthz` dependencies 欄位 + degraded startup）
- **Affected code**:
  - 新增 `sidecar/src/codebus_agent/kb/__init__.py`
  - 新增 `sidecar/src/codebus_agent/kb/qdrant_client.py`（wrapper + URL helper + `ensure_collection`）
  - 修改 `sidecar/src/codebus_agent/api/__init__.py`（`create_app` 接 `qdrant_url` kwarg）
  - 修改 `sidecar/src/codebus_agent/api/main.py`（`run()` 讀 env、傳給 `create_app`）
  - 修改 `sidecar/src/codebus_agent/healthz.py`（共用 kb wrapper 的 URL / probe）
  - 新增 `sidecar/tests/kb/__init__.py`
  - 新增 `sidecar/tests/kb/test_qdrant_client.py`（URL 解析、connection probe、degraded fallback、`ensure_collection` idempotency）
  - 修改 `sidecar/tests/test_healthz.py`（涵蓋 runtime `/healthz` 注入 qdrant check 情境）
  - 修改 `sidecar/tests/test_healthz_cli.py`（若共享路徑改動）
- **Affected docs**:
  - `docs/module-2-kb-builder.md §十三`（P0 第一項「Qdrant client wrapper」標為本 change 交付）
  - `docs/decisions.md` D-027「連動更新」清單可勾掉 `docs/module-2-kb-builder.md §三` 行文中立化一條（本 change 不改 spec prose，但 runtime 層補齊後可打勾）
- **Dependencies**:
  - `qdrant-client` Python package 已在 `sidecar/pyproject.toml`（M1 引入）；本 change **不升版、不新增**。
- **Downstream unlock**:
  - `docs/implementation-plan.md` Step 14 Module 2 P0 可開工。
- **Archive follow-up**（`spectra archive` 流程 `MODIFIED Requirements` 不吃 Purpose prose）：archive 後手動把 `openspec/specs/qdrant-client/spec.md` 的 Purpose 從 `TBD - created by archiving change 'm1-power-on'` 改為「定義 Qdrant 的 launch recipe、smoke test 與 sidecar runtime lifecycle（client wrapper、health probe、`ensure_collection`、degraded-mode startup）」。
