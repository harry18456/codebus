## 1. Scaffolding

- [x] 1.1 於 `sidecar/src/codebus_agent/providers/openai_chat.py` 建空模組（`OpenAIChatProvider` 類別 stub + `OpenAIContextLengthError` 例外類別 + `__all__`，待 Section 3 GREEN 補實作）
- [x] 1.2 於 `sidecar/src/codebus_agent/providers/__init__.py` re-export `OpenAIChatProvider` / `OpenAIContextLengthError`，並加入 `__all__`

## 2. RED — `OpenAI chat provider` 規約測試

對應 spec `llm-provider / OpenAI chat provider`。

- [x] 2.1 [P] 於 `sidecar/tests/providers/test_openai_chat.py` 加 `test_chat_returns_validated_pydantic_instance`（OpenAI chat provider Scenario "Chat call returns validated Pydantic instance"；用 `respx` mock OpenAI `/v1/chat/completions` 回 JSON，斷言返回 `BaseModel` 實例且欄位正確）
- [x] 2.2 [P] 於 `test_openai_chat.py` 加 `test_missing_env_var_blocks_construction`（OpenAI chat provider Scenario "Missing CODEBUS_OPENAI_API_KEY env var blocks construction"；錯誤訊息提到 `CODEBUS_OPENAI_API_KEY`、不 fallback `OPENAI_API_KEY`）
- [x] 2.3 [P] 於 `test_openai_chat.py` 加 `test_no_fallback_to_openai_api_key`（即使設了 `OPENAI_API_KEY` 但沒設 `CODEBUS_OPENAI_API_KEY` 仍 raise）
- [x] 2.4 [P] 於 `test_openai_chat.py` 加 `test_401_maps_to_openai_auth_failed`（OpenAI chat provider Scenario "Authentication failure maps to OPENAI_AUTH_FAILED"；mock 401，斷言拋 `OpenAIAuthError`、訊息不含 API key）
- [x] 2.5 [P] 於 `test_openai_chat.py` 加 `test_429_after_retries_maps_to_openai_rate_limited`（OpenAI chat provider Scenario "Rate limit after retries maps to OPENAI_RATE_LIMITED"）
- [x] 2.6 [P] 於 `test_openai_chat.py` 加 `test_context_length_exceeded_maps_to_openai_context_exceeded`（OpenAI chat provider Scenario "Context-length error maps to OPENAI_CONTEXT_EXCEEDED"；mock 400 with `error.code == "context_length_exceeded"`，斷言拋新 `OpenAIContextLengthError`，`_classify_exception` map 到 `OPENAI_CONTEXT_EXCEEDED`，error event 不含 prompt 內容）
- [x] 2.7 [P] 於 `test_openai_chat.py` 加 `test_temperature_and_max_tokens_pass_through`（OpenAI chat provider Scenario "Temperature and max_tokens passed to OpenAI"；驗 `respx` 攔到的 request body 有正確 `temperature` / `max_tokens`）
- [x] 2.8 [P] 於 `test_openai_chat.py` 加 `test_registry_rejects_unwrapped_openai_chat_provider`（OpenAI chat provider Scenario "Provider must be registered through TrackedProvider"）

## 3. GREEN — 實作 `OpenAI chat provider`

- [x] 3.1 於 `providers/openai_chat.py` 實作 `OpenAIChatProvider.__init__(model, *, temperature=0.2, max_tokens=None)`：讀 `CODEBUS_OPENAI_API_KEY` env、構 `instructor.from_openai(openai.AsyncOpenAI(api_key=...))` client；無 env 即 raise（對齊決策 5：OpenAI API key 只吃 env var）
- [x] 3.2 於 `providers/openai_chat.py` 實作 `chat(messages, *, response_model)`：把 `Message` 轉 OpenAI 格式 `{role, content}`、呼 `client.chat.completions.create_with_completion(model=..., temperature=..., max_tokens=..., response_model=response_model, messages=...)`、回 validated Pydantic 實例
- [x] 3.3 於 `providers/openai_chat.py` 加 `OpenAIContextLengthError(Exception)` 類別；`chat` 內 catch `openai.AuthenticationError` → raise `OpenAIAuthError`、catch `openai.RateLimitError` → raise `OpenAIRateLimitError`、catch `openai.BadRequestError` 且 body `error.code == "context_length_exceeded"` → raise `OpenAIContextLengthError`
- [x] 3.4 於 `api/tasks.py::_classify_exception` 加 `OpenAIContextLengthError` → `"OPENAI_CONTEXT_EXCEEDED"` mapping；`ERROR_CODES` 加入 `"OPENAI_CONTEXT_EXCEEDED"`；`_safe_error_message` 加對應友善訊息「LLM context window exceeded」（不含 prompt 內容）
- [x] 3.5 於 `providers/tracked.py::TrackedProvider.ALLOWED_INNER_TYPES` 加入 `OpenAIChatProvider`——**落實 `Outbound LLM traffic gated by TrackedProvider whitelist` Requirement 的 Scenario "Allowed inner types are explicitly enumerated" + 退役 `No outbound LLM traffic during M1` Requirement**(M1-era invariant 被替代,allowlist 從 `{MockProvider, OpenAIEmbeddingProvider}` 變成 `{MockProvider, OpenAIEmbeddingProvider, OpenAIChatProvider}`,spec 明文記錄的 allowlist 與 code 一致)
- [x] 3.6 於 `sidecar/tests/providers/test_tracked_provider.py`（或等效測檔）加 `test_tracked_provider_rejects_unknown_inner_types`:驗 `Outbound LLM traffic gated by TrackedProvider whitelist` 的 Scenario "ALLOWED_INNER_TYPES enforces explicit allowlist"——亂傳一個類別進 TrackedProvider 必 raise TypeError（若既有已有此測,確認它覆蓋範圍包含新 allowlist）
- [x] 3.7 執行 `uv run pytest sidecar/tests/providers/test_openai_chat.py sidecar/tests/providers/test_tracked_provider.py` 確認 2.1 ~ 2.8 + 3.6 全綠

## 4. RED — `KB dependency injection hook`（含 chat slots）規約測試

對應 spec `sidecar-runtime / KB dependency injection hook` 新加的 Scenarios。

- [x] 4.1 [P] 於 `sidecar/tests/test_wire_kb_dependencies.py` 加 `test_wires_all_eight_slots_when_env_present`（KB dependency injection hook Scenario "Both env vars present wire all eight slots"；env 有設時 8 個 slot 都非 None）
- [x] 4.2 [P] 於 `test_wire_kb_dependencies.py` 加 `test_missing_openai_key_leaves_chat_slots_none`（KB dependency injection hook Scenario "Missing OpenAI API key leaves provider slot as None"；env 缺時 3 個 chat slot 與所有 OpenAI-dependent slot 都 None）
- [x] 4.3 [P] 於 `test_wire_kb_dependencies.py` 加 `test_chat_slots_are_factories_returning_tracked_providers`（KB dependency injection hook Scenario "Chat-ish provider slots are factories returning TrackedProviders with role-appropriate default_module"；分別 invoke 三個 slot,驗回 TrackedProvider with 對應 `_default_module` `"reasoning"` / `"judge"` / `"chat"` 與 `role` `REASONING` / `JUDGE` / `CHAT`）
- [x] 4.4 [P] 於 `test_wire_kb_dependencies.py` 加 `test_healthz_reports_openai_chat_dependency_states`（KB dependency injection hook Scenario "Healthz reflects OpenAI chat configuration state"；同 embedding 三態 ok / degraded / not-configured pattern）

## 5. GREEN — 實作 `KB dependency injection hook` 擴充

- [x] 5.1 於 `api/__init__.py` 加 `_make_chat_provider_factory(*, model, temperature, default_module, role)` helper：與 `_make_provider_factory` 結構類似，但內構 `OpenAIChatProvider` 而非 `OpenAIEmbeddingProvider`，role 取自參數而非寫死 `EMBED`
- [x] 5.2 於 `api/__init__.py::wire_kb_dependencies` 在 `openai_api_key` 有設時新增 3 個 slot 設定：`llm_reasoning_provider` (model="gpt-4o-mini", temperature=0.1, default_module="reasoning", role=REASONING)、`llm_judge_provider` (model="gpt-4o-mini", temperature=0.0, default_module="judge", role=JUDGE)、`llm_chat_provider` (model="gpt-4o-mini", temperature=0.2, default_module="chat", role=CHAT)；缺 env 時三個都 `None`
- [x] 5.3 於 `api/__init__.py` 加 `_probe_openai_chat_raw()` async helper：構 raw `OpenAIChatProvider("gpt-4o-mini")`、呼 `chat([Message(role="user", content="ping")], response_model=_PingModel)`（內部定義最小 Pydantic model）；catch 例外回 `DependencyStatus(ok=False, status="degraded", detail=...)`，成功回 `DependencyStatus(ok=True, status="ok")`
- [x] 5.4 於 `api/__init__.py::create_app` 啟動時做一次 chat smoke probe（同 embedding pattern，cache 結果於 `app.state.openai_chat_probe`）；`dependency_checks` 加 `openai_chat` key
- [x] 5.5 執行 `uv run pytest sidecar/tests/test_wire_kb_dependencies.py` 確認 4.1 ~ 4.4 全綠 + 既有測無 regression

## 6. 文件更新

- [x] 6.1 於 `docs/llm-provider.md §三-bis` 補 OpenAIChatProvider 段：契約摘要、錯誤碼對照（含新的 `OPENAI_CONTEXT_EXCEEDED`）、與 OpenAIEmbeddingProvider 的對照表
- [x] 6.2 於 `docs/sidecar-api.md §一` healthz 段補 `openai_chat` dependency key 三態（與既有 `openai_embedding` 並列）
- [x] 6.3 於 `docs/module-2-kb-builder.md §七` Production wiring 段補三個 chat-ish slot 並列說明（與既有 `kb_provider` / `kb_query_provider` 對照）—— 雖然 chat 不是 KB 用,但 wiring 都集中在 `wire_kb_dependencies`,放此處方便交叉查
- [x] 6.4 於 `CLAUDE.md` 「最近一筆 in-progress」改指 `chat-provider-wiring`，sidecar 描述補「OpenAIChatProvider + 三個 chat-ish role factory(reasoning / judge / chat,皆預設 `gpt-4o-mini`),解鎖 Module 4 Explorer」

## 7. 驗證與 commit gate

- [x] 7.1 執行 `uv run pytest sidecar/tests/providers/` 確認 provider 層全綠
- [x] 7.2 執行 `uv run pytest sidecar/tests/` 完整 suite 無 regression
- [x] 7.3 執行 `pre-commit run --all-files` 全綠
- [x] 7.4 手動煙霧測（需 `CODEBUS_OPENAI_API_KEY` + `.env` 已備好）：(a) `/healthz` `openai_chat.status == "ok"`；(b) 直接用 Python 在 sidecar 內呼叫 `app.state.llm_reasoning_provider(workspace)` 取 TrackedProvider,跑一個簡單 chat call 測 Instructor 結構化輸出可運作（暫無 chat HTTP endpoint,Module 4 落地時才有外部觸發）；(c) 檢 `<workspace>/token_usage.jsonl` 多一筆 `module="reasoning"` 行（驗 cost 標籤分離）—— 用 `sidecar/scripts/smoke_chat_provider.py` 自動化跑過，全綠
