## Why

對齊 D-003（LLM Provider 抽象）、D-021（UsageTracker 強制走所有 Provider）、D-027（Qdrant 走 local binary）。`sse-progress-skeleton`（2026-04-22 archived）把 `POST /kb/build` 的 SSE 骨架接好，但端點在 production 會回 `503 KB_NOT_CONFIGURED`——因為 `app.state.kb_backend` / `kb_provider` / `kb_usage_tracker` / `kb_embedding_dim` 四個 slot 還沒人填。本 change 負責把這條線從 503 接到 200，讓使用者第一次能在 Trust Layer demo 看到真實的 chunk → embed → upsert → KBStats 全鏈路。

## What Changes

- **新增 OpenAI embedding provider**（`OpenAIEmbeddingProvider`）：`embed()` API，走 `text-embedding-3-small`（dim 1536），註冊至 `ProviderRegistry` 的 `ProviderRole.embedding` role；必經 `TrackedProvider` 裝飾（M1 registry guard 已強制）。
- **在 `create_app` / `main.py` 加 `wire_kb_dependencies(app)` hook**：讀 env var（`CODEBUS_OPENAI_API_KEY`、`CODEBUS_QDRANT_URL` fallback 至既有 resolver）→ 組 `OpenAIEmbeddingProvider` + `QdrantHttpBackend` + `UsageTracker` factory → 塞進 `app.state.kb_*`。
- **UsageTracker per-workspace 實例化**：依 D-021 `{workspace}/.codebus/token_usage.jsonl` 路徑規範，`app.state.kb_usage_tracker` 改為 factory（`Callable[[Path], UsageTracker]`）而非預建實例；`POST /kb/build` 端點於收到請求時依 `workspace_root` 構造。既有測試注入 pattern 保留 backward compat（測試可注入固定 factory 返回固定實例）。
- **Sidecar 啟動時 graceful degradation**：無 `CODEBUS_OPENAI_API_KEY` → `kb_provider` 留 `None`，`POST /kb/build` 維持 503 `KB_NOT_CONFIGURED`（不崩潰啟動，對齊「degraded-but-alive」慣例，與 Qdrant 不可達時 `/healthz` 回 degraded 同策略）。
- **新增 D-032 ADR**：M2 預設 embedding provider 為 OpenAI；本地 provider（sentence-transformers）延至 offline-mode change。
- **Qdrant collection 既存時的 dim-mismatch guard**：KB build 前比對現有 collection 的 vector dim 與 provider 宣告的 dim，不符時回 409 `KB_DIM_MISMATCH`（防止誤把新 model 的向量灌進舊 collection）。
- **spec deltas**：
  - `knowledge-base`：新增 `KB build production dependency wiring` Requirement（cover 503 graceful mode + dim-mismatch guard + happy path 200 with KBStats）
  - `sidecar-runtime`：新增 `KB dependency injection hook` Requirement（cover `wire_kb_dependencies` contract + env var resolution + factory slot semantics）
  - `llm-provider`：新增 `OpenAI embedding provider` Requirement（cover role registration + API key resolution + 錯誤路徑 `OPENAI_AUTH_FAILED` / `OPENAI_RATE_LIMITED`）
  - `usage-tracking`：**不動 spec**。既有 `UsageTracker writes token_usage.jsonl` Requirement 已定義 `<workspace>/token_usage.jsonl` path，本 change 只是依規使用，不改變 tracker 行為。

## Non-Goals

- **Sanitizer Pass 2（LLM call pre-flight）**：是橫切關注、獨立 change `sanitizer-pass2-implementation`；本 change `llm_calls.jsonl` 的 `sanitizer_pass2_applied` 欄位維持 false，與 M1 契約一致。
- **本地 embedding provider（sentence-transformers / Ollama embed）**：D-032 決定 M2 先走 OpenAI；offline-mode 屬另一條 change。
- **Multi-workspace 同時 KB build**：sidecar 已用單槽 task registry 擋住（`sse-progress-skeleton` 的 `TASK_IN_FLIGHT` 409），本 change 不動 registry 語意。
- **OpenAI rate-limit retry / exponential backoff 細調**：initial 實作走 Provider 層預設（最多 3 次 retry），正式調參等跑出真實 rate limit 數據再說。
- **前端 KB build progress UI / 錯誤 toast**：屬 Module 7 前端範疇，本 change 只到 sidecar 端。
- **跨 session 成本累計 dashboard**：D-021 明文 Phase 2+ 才做，本 change 延續 per-session summary。
- **Qdrant collection 自動遷移**（dim 變動時自動 re-embed）：`KB_DIM_MISMATCH` 只擋住誤寫，自動遷移屬 migration change 範疇。

## Capabilities

### New Capabilities

（無）

### Modified Capabilities

- `knowledge-base`：新增 `KB build production dependency wiring` Requirement——覆蓋 production 下 `POST /kb/build` 的 happy path（200 + `KBStats`）、graceful degraded（503 `KB_NOT_CONFIGURED`）、dim-mismatch（409 `KB_DIM_MISMATCH`）三條路徑；與 `sse-progress-skeleton` 落地的 `POST /kb/build async endpoint` 互補。
- `sidecar-runtime`：新增 `KB dependency injection hook` Requirement——定義 `wire_kb_dependencies(app, *, openai_api_key, qdrant_url)` 契約與 env var 解析規則（`CODEBUS_OPENAI_API_KEY` / `CODEBUS_QDRANT_URL`），以及 factory vs instance slot 語意。
- `llm-provider`：新增 `OpenAI embedding provider` Requirement——`OpenAIEmbeddingProvider.embed(texts) -> EmbedResponse` 契約、必經 `TrackedProvider` 包裝、registry `ProviderRole.embedding` 註冊、錯誤路徑 `OPENAI_AUTH_FAILED` / `OPENAI_RATE_LIMITED`。

## Impact

- **受影響 spec**：`openspec/specs/{knowledge-base,sidecar-runtime,llm-provider,usage-tracking}/spec.md`（皆 delta）
- **受影響 code**：
  - `sidecar/src/codebus_agent/providers/openai_embedding.py`（新檔）
  - `sidecar/src/codebus_agent/providers/registry.py`（註冊 embedding role）
  - `sidecar/src/codebus_agent/api/__init__.py`（`create_app` 接 `wire_kb_dependencies`）
  - `sidecar/src/codebus_agent/api/main.py`（啟動時呼叫 wire，讀 env）
  - `sidecar/src/codebus_agent/api/kb.py`（`_require_kb_deps` 支援 factory，`_coro_factory` 加 dim-mismatch guard）
  - `sidecar/src/codebus_agent/kb/knowledge_base.py`（build 前呼叫 backend dim check）
- **受影響文件**：
  - `docs/decisions.md`（新增 D-032）
  - `docs/module-2-kb-builder.md §七`（補 production wiring 段）
  - `docs/llm-provider.md`（補 OpenAI embedding 段）
  - `docs/implementation-plan.md`（步驟 15 後新增 15.5 或 16.1 標記本 change）
  - `CLAUDE.md`（Repo 現況 sidecar 描述 + in-progress pointer）
- **依賴套件**：`openai>=1.0`（已在 pyproject 但未實際使用，本 change 會真的拉起來）
- **env var 新增**：`CODEBUS_OPENAI_API_KEY`（無值時走 degraded mode）
