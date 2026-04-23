## Why

`kb-build-production-wiring`(2026-04-23 archived)接通了 OpenAI **embedding** provider,讓 KB build / query 跑真實向量。但 **chat / reasoning / judge** 三個 role 仍然只有 `MockProvider`——這直接擋住 Module 4 Explorer Agent(D-012 ReAct loop)落地:Explorer 的核心是 `provider.chat(messages, response_model=Action)` 呼叫,沒有真實 LLM 對話端就只能跑死腳本。

本 change 把 chat-ish 三個 role(REASONING / JUDGE / CHAT)接上 OpenAI `gpt-4o-mini`,套用與 embedding 完全相同的 wiring pattern(`wire_kb_dependencies` factory + `default_module` cost 標籤 + healthz probe + graceful degrade),解鎖 Module 4 P0。同時 **修正 spec 對齊 M2 現況**——既有 `No outbound LLM traffic during M1` Requirement 是 M1-era 的暫時性 invariant,M2 已合法允許特定 role 走 outbound。

對齊 `docs/decisions.md` D-003(LLM provider 抽象)、D-012(自寫 ReAct + Instructor)、D-021(usage tracking)、D-022(LLM call inspector)。

## What Changes

- **新增 `OpenAIChatProvider`**(`providers/openai_chat.py`):
  - `chat(messages, *, response_model) -> BaseModel` 走 `instructor` 包 `openai.AsyncOpenAI` 的 `chat.completions.create`,Instructor 處理 Pydantic 結構化輸出
  - 建構參數:`model: str`、`temperature: float = 0.2`、`max_tokens: int | None = None`
  - 預設 model:`gpt-4o-mini`(per-role 可在 wiring 階段覆蓋)
  - API key:讀 `CODEBUS_OPENAI_API_KEY`(與 embedding 共用,不另開 env var)
  - 錯誤類別:沿用既有 `OpenAIAuthError` / `OpenAIRateLimitError`,新增 `OpenAIContextLengthError`(map 到新 wire code `OPENAI_CONTEXT_EXCEEDED`)
- **`TrackedProvider.ALLOWED_INNER_TYPES` 加入 `OpenAIChatProvider`**(現為 `{MockProvider, OpenAIEmbeddingProvider}`,改成 `+ OpenAIChatProvider`)
- **`api/__init__.py::wire_kb_dependencies` 新增 3 個 factory slot**:
  - `app.state.llm_reasoning_provider`(default_module=`"reasoning"`,model=`gpt-4o-mini`,temperature=0.1)
  - `app.state.llm_judge_provider`(default_module=`"judge"`,model=`gpt-4o-mini`,temperature=0.0)
  - `app.state.llm_chat_provider`(default_module=`"chat"`,model=`gpt-4o-mini`,temperature=0.2)
  - 都是 `Callable[[Path], TrackedProvider]`,workspace-scoped,沿用 D-032 A 方案 factory pattern
- **`/healthz` 新增 `openai_chat` dependency key**:啟動時 raw `OpenAIChatProvider("gpt-4o-mini").chat([Message(role="user", content="ping")], response_model=_PingModel)` smoke probe;三態 ok / degraded / not-configured(沿用 embedding healthz pattern)
- **MODIFIED spec**:`No outbound LLM traffic during M1` Requirement 改寫為 `Outbound LLM traffic gated by TrackedProvider whitelist`,反映 M2 現況——sidecar 不再禁止 outbound,但**任何**外部 LLM call 都必須經 `TrackedProvider`(`ALLOWED_INNER_TYPES` 顯式 allowlist 控管)
- **新增 `_classify_exception` 分類**:`OpenAIContextLengthError` → `OPENAI_CONTEXT_EXCEEDED`,加入 `ERROR_CODES` 集合
- **無新 dependency**(`openai>=1.0` 已在 pyproject;`instructor>=1.15.1` 已在 pyproject)
- **無新 env var**(共用 `CODEBUS_OPENAI_API_KEY`)

## Non-Goals

- **本 change 不實作 Module 4 Explorer Agent 本身**:只接 provider,Explorer 是另一條 change 的事(`module-4-explorer-p0` 之類)
- **不做 streaming chat**(`chat_stream`):D-012 自寫 ReAct 用 Instructor 結構化輸出,不需要 token 流;streaming 留給 M5+ 使用者對話介面
- **不做 native function calling**(OpenAI tool_choice):D-012 明文「自寫 ReAct」,工具 dispatch 由 Agent 層處理,provider 只回 `response_model`;native tool_calls 不在 M2 scope
- **不做 per-role 不同 model 的 env var override**(例 `CODEBUS_OPENAI_REASONING_MODEL`):MVP 三個 role 都預設 `gpt-4o-mini`,避免 demo 跑出不一致行為;如果使用者真要換,改 wire code 即可,1 行
- **不做 vision / multimodal capability**(D-028 明文延後 Phase 2)
- **不做 token budget 上限強制中止**(D-021 明文 MVP 只顯示、不強制;Budget 仍走 token count)
- **不重構 `kb_provider` / `kb_query_provider` slot 名稱對齊新 `llm_*` 前綴**:現有名稱反映「KB 用的 provider」語意,改名是 breaking change,留給未來統一 LLM registry 的 change
- **不改 Explorer 預期的 chat-side Sanitizer Pass 2 行為**:既有 TrackedProvider 對 `chat` 路徑的 Pass 2 邏輯不動,本 change 只串接 inner provider
- **不做完整 LLM cost dashboard**:`token_usage.jsonl` 已能 group by module,UI 屬 Module 7 範疇

## Capabilities

### New Capabilities

(無)

### Modified Capabilities

- `llm-provider`(modify + add):
  - **MODIFY** `No outbound LLM traffic during M1`(改寫 + 重命名為 `Outbound LLM traffic gated by TrackedProvider whitelist`,反映 M2 已允許特定 role 走 outbound,但仍受 `ALLOWED_INNER_TYPES` allowlist 嚴格控管)
  - **ADD** `OpenAI chat provider`(`OpenAIChatProvider.chat` 契約 + 必經 `TrackedProvider` 包裝 + 三個錯誤碼對應)
- `sidecar-runtime`(modify):
  - **MODIFY** `KB dependency injection hook`(擴大為 `LLM dependency injection hook`,新增三個 chat-ish slot 的 wiring + healthz `openai_chat` 三態 probe)

## Impact

- **受影響 spec**:`openspec/specs/llm-provider/spec.md`(modify + add)、`openspec/specs/sidecar-runtime/spec.md`(modify)
- **受影響 code**:
  - `sidecar/src/codebus_agent/providers/openai_chat.py`(新檔)
  - `sidecar/src/codebus_agent/providers/__init__.py`(re-export `OpenAIChatProvider` + `OpenAIContextLengthError`)
  - `sidecar/src/codebus_agent/providers/tracked.py`(`ALLOWED_INNER_TYPES` 加入 `OpenAIChatProvider`)
  - `sidecar/src/codebus_agent/api/__init__.py`(`wire_kb_dependencies` 加 3 個 factory slot + healthz `openai_chat` probe)
  - `sidecar/src/codebus_agent/api/tasks.py`(`_classify_exception` + `ERROR_CODES` 加 `OPENAI_CONTEXT_EXCEEDED`)
- **受影響測試**:
  - `sidecar/tests/providers/test_openai_chat.py`(新檔,respx mock OpenAI chat completions)
  - `sidecar/tests/test_wire_kb_dependencies.py`(加 3 個 chat slot 與 healthz 三態測)
- **受影響文件**:
  - `docs/llm-provider.md §三-bis`(補 OpenAIChatProvider 段)
  - `docs/sidecar-api.md §一` healthz 段(`dependencies` map 加 `openai_chat` 三態)
  - `docs/module-2-kb-builder.md §七` Production wiring 段(補 chat-ish 三 slot 並列說明)
  - `CLAUDE.md` Repo 現況 sidecar 描述 + in-progress pointer
- **無新依賴 / 無新 env var**
