## Context

`sse-progress-skeleton`（2026-04-22 archived）落地了 `POST /kb/build` 的 async task lifecycle，但刻意只接到 `app.state.kb_*` 的 hook 層——backend / provider / usage_tracker / embedding_dim 四個 slot 留給本 change 接 production 實作。現況：

- `create_app` 在 `sidecar/src/codebus_agent/api/__init__.py` 接了 Qdrant client lifecycle（2026-04-19 m1-power-on），但沒接 KB 依賴。
- `providers/registry.py` 已有 role-based dispatch（2026-04-20 llm-role-routing archived），`ProviderRole.embedding` 角色存在但沒 provider 註冊。
- `kb/qdrant_client.py` 已有 `QdrantHttpBackend` adapter（2026-04-21 module-2-kb-builder-p0），可直接用。
- D-021 規範 `token_usage.jsonl` **workspace-level** 路徑（`{workspace}/.codebus/token_usage.jsonl`），session_id 生命週期綁 workspace 開關——現有 `sse-progress-skeleton` 測試把 UsageTracker 當 app-level singleton 注入是**測試 shortcut**，不是 production 契約。

本 change 的實際目標：讓 `POST /kb/build` 在 sidecar 正常啟動、Qdrant 通、`CODEBUS_OPENAI_API_KEY` 有設的三件齊備時回 200 + `KBStats`；三件缺一時 graceful degrade（503 或 409，可復原、不崩潰啟動）。

## Goals / Non-Goals

**Goals:**

- `POST /kb/build` 在 production 下能端到端跑通，SSE 吐 progress + done、`/tasks/{id}/result` 拿到真實 `KBStats`。
- OpenAI embedding provider 走 `TrackedProvider` 包裝，`token_usage.jsonl` 在 workspace-level 路徑可驗證落盤。
- 任何單一依賴缺失（Qdrant down / OpenAI API key 未設 / collection dim 不符）→ sidecar 啟動成功、端點回有意義的錯誤碼，不崩潰。
- 既有 `sse-progress-skeleton` 的 HTTP 契約（task_id 格式、409 / 503 / 200 response 形狀）完全不變。

**Non-Goals:**

（完整 Non-Goals 列表見 proposal；此處 design-specific 列幾條對決策有影響的）

- 不做本地 embedding provider（sentence-transformers / Ollama embed）——屬 `offline-mode` change。
- 不做 Qdrant collection 自動 re-embed migration——`KB_DIM_MISMATCH` 只擋住誤寫。
- 不改 M1 `llm_calls.jsonl` 的 `sanitizer_pass2_applied: false` 契約——Pass 2 是獨立 change。

## Decisions

### 決策 1：embedding provider 選 OpenAI `text-embedding-3-small`（dim 1536）

**考慮的選項：**

| 選項 | 優點 | 缺點 |
|---|---|---|
| **OpenAI `text-embedding-3-small`** | dim 1536 穩定、latency 可預期、API 成熟、demo 友善、cost 低（$0.02/1M tokens） | 需 API key、非本地、有 rate limit |
| OpenAI `text-embedding-3-large` | dim 3072 更準 | cost 6 倍（$0.13/1M）、demo 沒差 |
| sentence-transformers 本地 | 完全本地、零 API cost | onefile binary 肥 ~500MB、首次 model 下載 UX 差、準確度較低 |
| Voyage AI / Cohere | 有些 benchmark 勝 OpenAI | 第三方 lock-in、demo 認知成本高 |

**決策**：選 `text-embedding-3-small`。MVP demo 優先「跑得順 + 看得懂」，cost 不是主要 blocker（每個 workspace embed 一次通常 < $0.10）。寫入 `docs/decisions.md` D-032。

**為什麼不把 provider 做成可選（env var 切換）**：M2 scope 下一條路徑就夠；多路徑代表多套 provider 設定 / retry / error code，testing matrix 爆炸。本地 provider 屬 offline-mode change 的 scope，那時一起做。

### 決策 2：missing API key → graceful degrade（維持 503）而非啟動失敗

**考慮的選項：**

| 選項 | 優點 | 缺點 |
|---|---|---|
| **Sidecar 啟動成功，`/kb/build` 回 503** | 前端可查 `/healthz` 看哪條依賴缺；使用者可在不重啟 sidecar 下設好 env 重試；對齊 Qdrant 不可達時 `/healthz` degraded 同策略 | 稍微鬆的契約——使用者需要自己懂 503 代表什麼 |
| Sidecar 啟動即 fail-fast | 錯誤早曝光、前端不用處理 503 | Dev 環境每次沒設 key 就啟動失敗，UX 差；mixed with Qdrant degraded 策略不一致 |

**決策**：選 graceful degrade。對齊既有 `/healthz` degraded 契約，前端可透過 `/healthz` 的 dependency 欄位判斷哪條缺，引導使用者補齊。503 body 的 `code: "KB_NOT_CONFIGURED"` 已是 `sse-progress-skeleton` 落地的 contract，本 change 不改。

**健康檢查擴充**：`/healthz` 的 dependency map 新增 `openai_embedding` key——`env 未設` → `status: "not-configured"`；`env 設但 API call 失敗` → `status: "degraded"`；`env 設且 smoke call 通` → `status: "ok"`。smoke call 只在 sidecar 啟動時做一次（輕量 `embed(["ping"])`），避免 `/healthz` 每次打 OpenAI。

### 決策 3：UsageTracker 用 factory 注入，而非預建實例

**考慮的選項：**

| 選項 | 優點 | 缺點 |
|---|---|---|
| **`kb_usage_tracker` 與 `kb_provider` 皆為 `Callable[[Path], ...]` factory** | 路徑延後到請求時知道；對齊 D-021 workspace-level 規範；TrackedProvider 內綁的 audit logger（usage / llm_calls / sanitize_audit）都落在正確的 workspace path | 兩個 slot 都是 callable，測試注入略繁瑣 |
| `kb_usage_tracker` 是 factory、`kb_provider` 是預建 TrackedProvider 實例 | Provider 單例、memory cleaner | TrackedProvider 建構需要 UsageTracker / LLMCallLogger / SanitizerAuditLogger 全是 workspace path ——啟動時根本不知道 workspace，會產生**兩套 tracker**（一條走 TrackedProvider 內的 app-level log、一條走 factory 的 workspace log），稽核 trail 分裂，違反 D-021 |
| 端點內直接 `UsageTracker(workspace_root / ...)` 跳過 `app.state` | 完全繞過 DI | 沒測試注入 hook |
| 跳過 TrackedProvider 包裝 | 最簡單 | 違反 spec「Registered embedding provider is wrapped in TrackedProvider」與 M1 registry guard invariant |

**決策（A 方案）**：`kb_usage_tracker` 與 `kb_provider` 都做成 factory（`Callable[[Path], ...]`）。`_require_kb_deps(request)` 回傳 `(backend, provider_factory, tracker_factory, embedding_dim)`；端點以 `request.workspace_root` 呼叫兩個 factory 取對應 workspace 的 TrackedProvider + UsageTracker。

**不對稱性說明**：`kb_backend`（Qdrant client）與 `kb_embedding_dim`（`text-embedding-3-small` 常數 1536）仍為 app-level 單例——前者是連線、後者是常數，沒有 workspace 層級差異。只有 audit 相關元件走 factory。

**測試 backward compat**：既有 `sse-progress-skeleton` 測試 fixture 的 `kb_provider` / `kb_usage_tracker` 實例注入，需改為 `lambda _ws: instance`（~3 行）。

**Healthz smoke probe 例外**：sidecar 啟動時的健康檢查 smoke embed（`/healthz` 的 `openai_embedding.status` 判定）使用**未包裝的 raw `OpenAIEmbeddingProvider`**，不經 TrackedProvider——因為此時沒有 workspace_root、健康檢查不是 production traffic、不該污染 audit trail。這個旁路在 hook docstring 明文記錄。

**Pass 2 對 KB chunks 重複 sanitize 的餘問**：KB 已經 Pass 1 過，TrackedProvider 內的 Pass 2 會再掃一次——但 placeholder `<REDACTED:kind#N>` 不會再匹配 sanitize 規則，所以 Pass 2 是 no-op（沒有新 audit entry 污染），只多花 O(chunks) 的 regex 掃描時間，對 1000 chunks 約多幾十 ms，可忽略。

### 決策 4：dim-mismatch guard 放在 KB 端不是 Backend 端

**考慮的選項：**

| 選項 | 優點 | 缺點 |
|---|---|---|
| **`KnowledgeBase.build` 開頭檢查** | 失敗得早，還沒 embed 就擋下；錯誤路徑不浪費 API cost | KB 層要知道 provider 宣告的 dim |
| `QdrantHttpBackend.upsert` 檢查 | backend 職責清晰 | 已經 embed 完才擋下，浪費 API call 與時間 |
| 兩層都檢查 | 最穩 | 重複邏輯，後者永遠跑不到 |

**決策**：KB 層檢查。`KnowledgeBase.__init__` 已拿 `embedding_dim: int` 參數，`build()` 在第一次 `chunking` 進 `embedding` 階段前呼叫 `backend.ensure_collection(name, expected_dim)`——既有 collection dim 不符 → raise `KBDimMismatchError`；`_run_background_task` wrapper 把它 map 到 `code: "KB_DIM_MISMATCH"`（新增到 `ERROR_CODES`）。

**後續遷移路徑**：如果 M3+ 換 `text-embedding-3-large`（dim 3072），使用者會碰到 `KB_DIM_MISMATCH`；目前無自動遷移（Non-Goal），使用者手動刪 collection 重 build。

### 決策 5：OpenAI API key 只吃 env var，不寫入任何持久化

**考慮的選項：**

| 選項 | 優點 | 缺點 |
|---|---|---|
| **只讀 `CODEBUS_OPENAI_API_KEY` env var** | 對齊 D-local-2 bearer token 不落盤原則、無 leak 風險 | 使用者每次 sidecar 啟動都要有設好的 env |
| 存 `~/.codebus/config.yaml` 加密 | UX 好 | 加密 key management 是另一個坑；MVP 不值得 |
| 前端傳入走 IPC | 動態化 | 增加 IPC 攻擊面；bearer 才該走記憶體常駐 |

**決策**：env var only。Tauri 啟 sidecar 時從 host 環境繼承 `CODEBUS_OPENAI_API_KEY`（未來前端設定 UI 走寫入 user-scoped env 的路徑，而不是 sidecar IPC）。這條決策從 D-011（資安與合規）與 D-local-2（bearer 只在記憶體）自然延伸，不需要新 D-XXX。

### 決策 6：retry / backoff 策略委派給 Provider 層，不在 KB pipeline 做

**決策**：`OpenAIEmbeddingProvider` 內部走 `openai` SDK 預設 retry（max_retries=3，exponential backoff）；KB pipeline 看到的是「最終成功或最終失敗」。失敗時 `TrackedProvider` 仍記錄 final attempt 的 usage（若有）與 error，符合 D-021 要求。

**為什麼不在 KB 做**：KB pipeline 已經有 `asyncio.Semaphore(3)` 控制並行；如果雙層 retry，`total_attempts = semaphore_slots * provider_retries * build_retries = 爆炸`，很難除錯 rate limit。

## Risks / Trade-offs

- **[OpenAI 服務中斷 → demo 全掛]** → Mitigation：`/healthz` 的 `openai_embedding` 在啟動 smoke call 失敗時標 degraded，前端可引導 fallback（目前 MVP 無 fallback provider，只能等復原；offline-mode change 落地後有）。
- **[API key 洩漏]** → Mitigation：env var only、不寫 log（`TrackedProvider` 已經過濾 API key；`llm_calls.jsonl` 只記 response payload，不記 request headers）。
- **[Rate limit 高峰 → 整個 build 卡超久]** → Mitigation：Provider 層 exponential backoff + `_PROGRESS_EMIT_EVERY` 節流讓前端看得到卡住；timeout 後 `_run_background_task` wrapper 會 emit `OPENAI_RATE_LIMITED` error。
- **[Workspace `token_usage.jsonl` 路徑建立失敗（無寫權）]** → Mitigation：UsageTracker factory 在建立時呼叫 `Path.mkdir(parents=True, exist_ok=True)`；失敗時 raise，`_run_background_task` 收斂成 `INTERNAL_ERROR`。
- **[Collection 存在但 dim 不符——使用者不懂該做什麼]** → Mitigation：`KB_DIM_MISMATCH` body 帶 `expected_dim` / `actual_dim` / `suggestion: "delete collection and rebuild"` 三個欄位，前端可展示可執行步驟。
- **[M1 的 unwrapped-provider registry guard 會不會 false-positive 擋住新 provider]** → Mitigation：新 `OpenAIEmbeddingProvider` 註冊時必經 `TrackedProvider(role=ProviderRole.embedding)` 包裝（對齊 M1 `TrackedProvider` 必包規則）；既有 registry guard 不需改。

## Open Questions

- **Q1：M2 之後是否允許 `CODEBUS_OPENAI_MODEL` env var 切換 embedding model?**
  - 目前傾向**不允許**（D-032 固定 `text-embedding-3-small`），避免 dim-mismatch 成為常態；若使用者真要換，走 offline-mode change 或另開 ADR。
- **Q2：`/healthz` smoke call 是否對 OpenAI cost 有感?**
  - 一次 `embed(["ping"])` 約 1 token < $0.00001，啟動一次 sidecar 打一次——可忽略。
- **Q3：UsageTracker factory 在 sidecar shutdown 時是否要 flush?**
  - `UsageTracker` 已是 append-only JSONL、每行 flush，shutdown 不需要特別 flush；但若未來改 buffered，需要在 `create_app` 的 shutdown hook 加 close。本 change 先不處理，留註記。
