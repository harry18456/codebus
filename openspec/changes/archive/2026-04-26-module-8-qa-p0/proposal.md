## Why

`docs/implementation-plan.md` 步驟 25 是 **Module 8 Q&A P0**（D-016：Q&A Agent + 持續成長 KB），是 M2 Backend 收尾的最後一塊：教材產出後，使用者可以繼續問問題，Agent 走 RAG → 必要時 workspace 補查 → 自主決定是否 `add_to_kb` 沉澱新知識；KB 不再是一次性建好之後凍結，**問答本身就是 KB 成長機制**，撐起 Trust Layer 的「持久化知識庫」與「Agentic 持續可感」敘事。也同步補上 `docs/reviews/2026-04-25-stage-4.md` Module 8 預警 4 條全部缺口（`kb_growth.jsonl` writer 缺、Pass 3 sanitizer hook 缺、新工具 `audit_fields` 必宣告、Cat 3 #3 Judge / Coverage prompt fork）。

關聯決策：**D-016**（Q&A 互動 + KB 自動成長）、**D-015**（Sanitizer 三段式，本 change 落地 Pass 3）、**D-002**（Topic / Folder 雙模 day-1，本 change 不破雙模 discriminator）、**D-021**（token_usage 拆 `module="qa_agent"`）、**D-022**（llm_calls wire payload）、**D-029**（stable station id `s{NN}-slug` 引用，Q&A `add_to_kb` 帶 `related_stations` 並透過 `kb_search(station_filter=...)` 反查）；`docs/qa-agent.md` 為母 spec、`docs/agent-explorer-spec.md §十二` Protocol seam 已就位（agent-core 不需動）。

## What Changes

**A. 新 capability `qa-agent`**（Q&A Agent ReAct 主迴圈 + 工具 + prompts + 防呆）

- `codebus_agent.agent.qa.run_qa(question, state, ...)` 兩階段 entrypoint：階段 1 RAG-first（`_hits_confident` 三條件：top-1 > 0.75 / top-3 平均 > 0.65 / top-5 entity coverage 全過 → 直接 RAG 回答）；不過則進階段 2 ReAct loop（reuse agent-core `_think` / `_execute_tools` / `_should_stop` / `ReasoningLogger`，不 reuse `Judge` / `CoverageChecker` — Q&A 不需要站點驗證 / 補查閉環）；階段 3 `_synthesize_answer` 收尾
- 新 `agent.types` 補 `QAState` / `QAAnswer` / `QAAction` Pydantic schema（不複用 `ExplorerState` — Q&A 沒有 stations / coverage 概念，但共用 `Message` / `ToolCall` / `ToolResult` / `Step`）
- 新 `agent.prompts.qa` 模組：`QA_SYSTEM` system prompt（包含「值得沉澱」三條件 — 可復用 / stable fact / 非同義重複；`STATION_ID_REGEX` 格式約束；`originating_station_id` 脈絡注入規則）+ `render_qa_prompt(state, question, initial_hits)` + `QA_PROMPT_VERSION="2026-04-26-1"` date-version
- 新 `agent.tools.qa_tools.QATools`（不繼承 `FolderTools`，是 sibling impl）：reuse 5 個 read tools（`search` / `read_file` / `list_dir` / `trace_import` / `find_callers` 重新 expose）+ 新 2 個 Q&A 專用 tools `kb_search` / `add_to_kb`；六個 read tool 都帶 `audit_fields` 宣告（reused from FolderTools 一致），新兩個 Q&A 工具同樣宣告 `audit_fields`（kb_search: `["query", "top_k", "station_filter"]` / add_to_kb: `["source", "reason", "related_stations"]` — 不 audit `chunks.text` 內容）
- 新 `kb_search(query, top_k=5, station_filter=None)` tool：吃 `KBSearchArgs` Pydantic（`station_filter` 任一 id 必符合 `^s\d{2}-[a-z0-9-]{1,40}(-\d+)?$` 否則 422）；走 `KnowledgeBase.query(...)` 新增的 `filter_stations` 參數（見 D 段）；回 N 筆 hit 文字格式（`file:line | score=X | stations=[...]\n  <snippet>`）
- 新 `add_to_kb(chunks, source, reason)` tool：每 chunk 過 Pass 3 Sanitizer（見 C 段）→ 驗 `related_stations` 格式 → 走 `KnowledgeBase.upsert_chunk(text, payload)` 新公開 API（見 D 段）→ 寫 `kb_growth.jsonl`（見 B 段）→ 回 added point ids；空文字（sanitize 全替）回 `skipped_empty`，不寫 KB / 不寫 growth log
- 防呆常數 module-level：`_QA_MAX_STEPS = 10` / `_QA_MAX_ADD_TO_KB_PER_SESSION = 20` / `_QA_MAX_CHUNK_SIZE_CHARS = 2000` / `_QA_MAX_ADD_TO_KB_PER_QUESTION = 5` / `_QA_DEDUP_THRESHOLD = 0.95`（per `docs/qa-agent.md §七`）；超 budget tool 回 error 字串給 Agent，prompt 約束「請完成回答」
- `usage_delta` SSE 已通用（`module` 欄反映 `default_module="qa_agent"` 由 TrackedProvider 寫入）；新 `qa_answer` SSE event（`{type, delta, citations}`）為 P0 stub（P1 接 streaming）；`kb_growth` SSE event 在 add_to_kb 寫檔後 emit

**B. 新 capability `kb-growth`**（第七層 workspace audit JSONL `<ws>/.codebus/kb_growth.jsonl`）

- 新 leaf `_KB_GROWTH_FILENAME = "kb_growth.jsonl"` 加進 api/_audit_paths.py（與既有六層 audit filename 並排）
- 新 `codebus_agent.kb.growth_logger.KBGrowthLogger` 唯一 writer，constructor 簽 `(path: Path)` 並 auto-mkdir parent `.codebus/`（對齊 UsageTracker / LLMCallLogger 慣例）；`write(*, point_id: str, source: str, reason: str, related_stations: list[str], originating_station_id: str | None, sanitize_stats: dict[str, int], chunk_size_chars: int, dedup_skipped: bool, session_id: str, question: str | None) -> None`
- JSONL line schema 必含 `ts` ISO 8601 / `session_id` / `question` / `originating_station_id` / `entry_id`（point_id）/ `source` / `related_stations` / `reason` / `sanitize_stats` / `chunk_size_chars` / `dedup_skipped`；append-only、單行 JSON `\n` 結尾；UI rollback 待 P1（spec MUST 預留 `rollback` event 寫入 hook，本 change 只寫 schema 不實作 rollback）
- `app.state.kb_growth_logger_factory` 工廠 in `wire_kb_dependencies(...)`：`Callable[[Path], KBGrowthLogger]` workspace-scoped（同 `_make_tracker_factory` 模式）；P0 endpoint 持 factory 而非 logger 實例
- 七層 audit 表內升級 `kb_growth.jsonl` 從 ⏳ 到 ✅；`CLAUDE.md` 七層段更新

**C. MODIFIED `sanitizer`** — Pass 3 audit hook

- 既有 `SanitizeSource = FileSource | MessageSource` 與 `SanitizerEngine.sanitize(...)` **不變**（spec 已宣稱「reusable by Pass 3 without signature change」）；本 change 落地：`add_to_kb` MUST 呼叫 `sanitizer.sanitize(text, source=FileSource(path=chunk.source, pass_="qa_add_to_kb"))`
- `sanitize_audit.jsonl` 命中時 `pass_num=3`、`source` 為結構化 `{"pass": "qa_add_to_kb", "path": "<chunk.source>"}`（與 Pass 1 scanner 的 `{"pass": "scanner", ...}` 同 schema 路徑、與 Pass 2 message-id 形式並列）
- `sanitize_audit.jsonl` schema 不變（rules_version / placeholder_index / kind 既有欄位全沿用）；schema_version 仍為 1
- spec 新增 Requirement / Scenario：`Pass 3 add_to_kb sanitize emits structured audit entry`（描述 Q&A `add_to_kb` MUST 走 sanitizer + 寫 `pass_num=3` audit）；既有 `Rules version is recorded on every audit line` 不動

**D. MODIFIED `knowledge-base`** — kb_search 過濾 + add_to_kb 寫入 API

- 既有 `KnowledgeBase.query(text, *, top_k, filter_path, filter_source_kind)` 簽名擴一個 keyword-only 參數 `filter_stations: list[str] | None = None`；給定時 Qdrant filter MUST 含 `should: [{"key": "related_stations", "match": {"value": <id>}}, ...]`（per qa-agent.md §三 station_filter 用途；hit 不足由 caller 決定要不要再不過濾打一次）
- 新 `KnowledgeBase.upsert_chunk(text: str, *, payload: KBPayload) -> str` 公開 API：embed 一次 → Layer 1 hash dedup（既有 `exists_by_hash`）→ Layer 2 similarity dedup（向量相似度 ≥ `_QA_DEDUP_THRESHOLD = 0.95`，重用 `find_similar` API）→ Qdrant upsert → 回 `point_id`；命中 dedup 時不 upsert / 不消耗 embed token，回字串 `"dedup:<hash|sim>"`；caller（add_to_kb）拿到非 hash/sim 開頭即視為新 point
- spec 新增兩條 Requirement：`Query filter_stations restricts hits by stable station id` + `KnowledgeBase exposes upsert_chunk for Q&A add_to_kb path`

**E. MODIFIED `sidecar-runtime`** — POST /qa endpoint 通電

- 新 `POST /qa` endpoint：body `QARequest{ workspace_root: str, question: str, originating_station_id: str | None }`；`workspace_root` 過 `ensure_in_workspace` / 要求存在；`question` 非空 / ≤ 4000 字；`originating_station_id` 給定時必符合 `^s\d{2}-[a-z0-9-]{1,40}(-\d+)?$` 否則 422
- `task_id` regex 從 `^(scan|kb|explore|generate)_[0-9a-f]{8}$` 擴成 `^(scan|kb|explore|generate|qa)_[0-9a-f]{8}$`（per `module-5-generator-p0` 的同 pattern）；`TaskKind` Literal 加 `"qa"`
- `_run_background_task` 既有錯誤收斂沿用，新錯誤碼 `QA_FAILED`（QA 例外 → wire `error` event `{code: "QA_FAILED", message: "<safe summary>"}` — 不 echo question / chunk 內容）
- QA 任務啟動前要求所有依賴 slot 齊備（`kb_provider` / `kb_query_provider` / `kb_growth_logger_factory` / `llm_chat_provider` / `llm_judge_provider` 任一缺即 503 `QA_NOT_CONFIGURED`，列出缺哪些 slot）
- spec MODIFIED 三條 Requirement：`task_id format` 擴 regex / `Background task error containment` 加 `QA_FAILED` / 新 `Q&A task spawn endpoint` Requirement

## Non-Goals

- **前端聊天 UI**：`docs/implementation-plan.md` 步驟 30 / Phase 6 範圍，跟 step 25 backend 切開；本 change 只到 SSE event schema + `/qa` endpoint；UI 元件 / 引用 panel / KB Growth 稽核 tab 都留下個 change
- **跨 session 問題關聯**（記憶使用者歷史問題）：Phase 2，qa-agent.md §十 MVP 不做
- **批次 rollback / KB 清理 UI**：P1 → 前端 change；本 change 只在 `kb_growth.jsonl` schema 預留 `rollback` event 形狀，writer / UI / Qdrant delete API 都不做
- **主動 KB 補強**（Agent 閒時自己梳理 KB）：Phase 3
- **多使用者共用 KB / 雲端同步**：Phase 3
- **外部 web 補查**（Topic mode 融合）：Phase 2，待 D-002 Topic mode 動工
- **多輪 planning**（Q 拆 sub-Q）：Phase 2
- **P1 KB Growth UI rollback 機制**：本 change 只暴露 `kb_growth.jsonl` 寫檔；rollback 透過 Qdrant delete + log append 由 P1 frontend change 接
- **`agent-core` capability 動土**：Protocol seam 已在 `explorer-react-loop-p0` 落地，本 change 不需動 `agent-core` spec；Q&A 自帶 prompts / state types，避開 Cat 3 #3 prompt fork 風險（不 reuse Judge / Coverage instance）
- **`tool-sandbox` capability 動土**：`audit_fields` 必填 rule 已存在；新 tool 只是合規，不需擴 spec
- **`usage-tracking` capability 動土**：`module` 欄已通用；`module="qa_agent"` 是值用法不是 schema 改變
- **變更 ReAct core 行為**：`_think` / `_execute_tools` / `_should_stop` / `ReasoningLogger` 全 reuse 既有實作；Q&A 只用 budget step / token / wall 條件停（不接 Judge verdict / coverage gap）

## Capabilities

### New Capabilities

- `qa-agent`：Q&A Agent ReAct 主迴圈 `run_qa` + RAG-first 兩階段流程 + `QATools`（5 個 reused read tool + 2 個新 `kb_search` / `add_to_kb`）+ QA system prompt + 防呆 budgets + `qa_answer` / `kb_growth` SSE event；對應 `docs/qa-agent.md §一-§九`
- `kb-growth`：第七層 workspace audit `<ws>/.codebus/kb_growth.jsonl` 寫入 capability — `KBGrowthLogger` 唯一 writer + JSONL schema + workspace-scoped factory；對應 D-016 + `docs/qa-agent.md §六`

### Modified Capabilities

- `sanitizer`：Pass 3 add_to_kb hook 落地（既有 `SanitizeSource` 不擴 union，沿用 `FileSource(pass_="qa_add_to_kb")`），新增 Requirement / Scenario 描述 `pass_num=3` 結構化 audit
- `knowledge-base`：`KnowledgeBase.query` 加 `filter_stations` 參數 + 新 `upsert_chunk` 公開 API（embed + 雙層 dedup + upsert）
- `sidecar-runtime`：`POST /qa` endpoint + `task_id` regex 擴 `qa_<8hex>` + `QA_FAILED` 錯誤碼 + `QA_NOT_CONFIGURED` 503 dependency-missing 路徑

## Impact

**受影響 spec**：

- 新 `openspec/specs/qa-agent/spec.md`（NEW，~10 條 Requirement）
- 新 `openspec/specs/kb-growth/spec.md`（NEW，~5 條 Requirement）
- `openspec/specs/sanitizer/spec.md`（MODIFIED — 1 條新 Requirement / 2 個 Scenario）
- `openspec/specs/knowledge-base/spec.md`（MODIFIED — 2 條新 Requirement）
- `openspec/specs/sidecar-runtime/spec.md`（MODIFIED — 3 條 Requirement：task_id regex / 錯誤碼表 / Q&A 端點 spawn）

**受影響 production code（新檔）**：

- sidecar/src/codebus_agent/agent/qa.py（`run_qa` + `_hits_confident` + `_synthesize_answer` + budget guards）
- sidecar/src/codebus_agent/agent/prompts/qa.py（`QA_SYSTEM` + `render_qa_prompt` + `QA_PROMPT_VERSION`）
- sidecar/src/codebus_agent/agent/tools/qa_tools.py（`QATools` class 含 7 個 tool method + 7 個 `audit_fields` 宣告）
- sidecar/src/codebus_agent/agent/tools/kb_search.py（單獨 module — KB 依賴注入較重）
- sidecar/src/codebus_agent/agent/tools/add_to_kb.py（同上）
- sidecar/src/codebus_agent/kb/growth_logger.py（`KBGrowthLogger` 唯一 writer）
- sidecar/src/codebus_agent/api/qa.py（`POST /qa` router + `_require_qa_deps` + `QARequest` Pydantic + handler）
- sidecar/tests/agent/test_run_qa.py / test_qa_tools.py / test_kb_search.py / test_add_to_kb.py（新 unit / integration test 群）
- sidecar/tests/kb/test_growth_logger.py（新 unit test）
- sidecar/tests/api/test_qa_endpoint.py（新 endpoint test）
- sidecar/tests/sanitizer/test_pass3_add_to_kb_audit.py（新 — 鎖 Pass 3 audit `pass_num=3` + `source.pass="qa_add_to_kb"` 結構）

**受影響 production code（修改）**：

- sidecar/src/codebus_agent/api/_audit_paths.py（加 `_KB_GROWTH_FILENAME`）
- sidecar/src/codebus_agent/api/__init__.py（`wire_kb_dependencies` 加 `kb_growth_logger_factory`、include_router 加 qa_router、healthz dependency 不動）
- sidecar/src/codebus_agent/api/tasks.py（`TaskKind` 加 `"qa"`、`task_id` regex 擴）
- sidecar/src/codebus_agent/kb/knowledge_base.py（`query.filter_stations` 參數 + 新 `upsert_chunk` 公開 method + Layer 2 similarity dedup hook）
- sidecar/src/codebus_agent/kb/__init__.py（re-export `KBGrowthLogger`）
- sidecar/src/codebus_agent/agent/types.py（新 `QAState` / `QAAnswer` / `QAAction` Pydantic schema）

**受影響 docs**：

- `docs/implementation-plan.md` 步驟 25 改 🚧（apply 階段）/ ✅（archive 階段）
- `docs/qa-agent.md` §十二 連動更新清單 P0 全 `[x]`
- `docs/decisions.md` D-016 後續清單補一條 `[x]`、D-021 / D-022 補 `qa_agent` module 拆帳一條
- `docs/reviews/2026-04-25-stage-4.md` Module 8 預警 4 條全 `[x]`、Cat 3 #3 prompt fork 因「Q&A 不 reuse Judge/Coverage instance」自然解決
- `CLAUDE.md` archive 表加 row、七層 audit 段 `kb_growth.jsonl` 從 ⏳ 翻 ✅、Module 列表 Module 8 標 P0 通電
- `docs/sidecar-api.md §三` 新 `POST /qa` 條目 + §四新 `qa_answer` / `kb_growth` SSE event schema

**無新外部依賴**（純 in-process Q&A loop + 既有 Qdrant + 既有 Sanitizer + 既有 TrackedProvider）。

**Schema breaking change** 無（既有 `KBPayload` 全沿用、`sanitize_audit.jsonl` schema 不變、`token_usage.jsonl` `module` 欄是值層升級不是 schema 變動、`tool_audit.jsonl` 沿用既有 `audit_fields` rule）。

**Migration**：無 — 既有 KB / token_usage / sanitize_audit 全相容；`kb_growth.jsonl` 是新檔，舊 workspace 無此檔不影響舊功能。

**估計工期**：~2.5d（per `docs/implementation-plan.md` 步驟 25 估計 — Q&A prompt 0.5d / main loop 0.5d / add_to_kb 0.5d / kb_growth + safety 0.5d / endpoint + integration test 0.5d）。
