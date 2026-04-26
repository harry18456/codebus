## 1. 路徑常數 + KBGrowthLogger 骨架（kb-growth capability：`kb_growth.jsonl path constant lives alongside other audit filenames` / `KBGrowthLogger writes kb_growth.jsonl` / `Required fields on every kb_growth.jsonl line` / `Event type field defaults to "add" with rollback reserved for P1`）

對應 design Decision 7（kb_growth.jsonl 預留 rollback event 形狀但 P0 不寫 rollback 路徑）。

- [x] 1.1 改 `sidecar/src/codebus_agent/api/_audit_paths.py` 加 `_KB_GROWTH_FILENAME = "kb_growth.jsonl"` 常數（leaf module、零 import）；同步同模組 `_audit_paths.py` shim re-export 也帶這個（per audit-path-unification 雙 module 慣例）
- [x] 1.2 [P] RED `sidecar/tests/api/test_audit_paths_kb_growth.py::test_kb_growth_filename_constant_exists`：assert `from codebus_agent.api._audit_paths import _KB_GROWTH_FILENAME` 成功 + 值 `== "kb_growth.jsonl"`
- [x] 1.3 [P] RED `sidecar/tests/api/test_audit_paths_kb_growth.py::test_no_literal_kb_growth_jsonl_outside_leaf`：grep `sidecar/src/codebus_agent/` 找 `"kb_growth.jsonl"` 字面量，唯一命中 path 必為 `_audit_paths.py`
- [x] 1.4 GREEN — 1.1 + 1.2-1.3 串聯通過
- [x] 1.5 RED `sidecar/tests/kb/test_growth_logger.py::test_constructor_auto_mkdirs`：tmp_path 無 `.codebus/` 時 `KBGrowthLogger(tmp_path/.codebus/kb_growth.jsonl)` 構造後 `.codebus/` 存在 + 檔尚未存在
- [x] 1.6 RED `test_growth_logger.py::test_write_appends_one_line`：呼叫 `write(...)` 一次後 file 存在 + 行數 1 + 內容 `json.loads` 後含全 12 個 required keys（`ts` / `session_id` / `question` / `originating_station_id` / `entry_id` / `source` / `related_stations` / `reason` / `sanitize_stats` / `chunk_size_chars` / `dedup_skipped` / `event_type`）
- [x] 1.7 RED `test_growth_logger.py::test_event_type_always_add_in_p0`：所有 `write(...)` invocation 寫出的 line `event_type == "add"` 字面量；inspect signature 確認沒有 `event_type` kwarg
- [x] 1.8 RED `test_growth_logger.py::test_invalid_station_id_raises_pre_write`：`write(... related_stations=["s9-bad"], ...)` 拋 `ValueError` 訊息含 `"s9-bad"`、`.codebus/kb_growth.jsonl` 仍不存在 / 行數 0
- [x] 1.9 RED `test_growth_logger.py::test_ts_iso_8601_with_utc`：line `ts` 欄位過 regex `^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(\.\d+)?(Z|[+-]\d{2}:\d{2})$`
- [x] 1.10 GREEN 實作 `sidecar/src/codebus_agent/kb/growth_logger.py::KBGrowthLogger`：constructor `(path: Path)` + auto-mkdir parent；`write(*, point_id, source, reason, related_stations, originating_station_id, sanitize_stats, chunk_size_chars, dedup_skipped, session_id, question)`；station_id pre-validation；hardcode `event_type="add"`；ISO 8601 UTC `datetime.now(timezone.utc).isoformat()`；append-only `\n` 結尾
- [x] 1.11 GREEN — `sidecar/src/codebus_agent/kb/__init__.py` re-export `KBGrowthLogger`；跑 1.5-1.9 全綠

## 2. KnowledgeBase query.filter_stations + upsert_chunk（knowledge-base capability：`KnowledgeBase query and find_similar API` MODIFIED + `KnowledgeBase exposes upsert_chunk for Q&A add_to_kb path` ADDED）

對應 design Decision 4（雙層 dedup，Layer 2 走 find_similar 重用）。

- [x] 2.1 [P] RED `sidecar/tests/kb/test_query_filter_stations.py::test_filter_stations_or_semantics`：populated KB 含 chunk A `related_stations=["s02-storage"]` / chunk B `related_stations=["s03-payment"]` / chunk C empty；`query("x", filter_stations=["s02-storage", "s03-payment"])` 回 A + B 不含 C
- [x] 2.2 [P] RED `test_query_filter_stations.py::test_empty_filter_stations_equivalent_to_none`：同 KB，`query("x", filter_stations=[])` 與 `query("x")` 結果集相同（id + 順序）
- [x] 2.3 [P] RED `test_query_filter_stations.py::test_invalid_station_id_raises_pre_call`：`query("x", filter_stations=["bad-id"])` 拋 `ValueError`、provider.embed 與 Qdrant search 都未呼叫（用 monkeypatch spy 抓）
- [x] 2.4 GREEN — 改 `sidecar/src/codebus_agent/kb/knowledge_base.py::KnowledgeBase.query` 簽名加 keyword-only `filter_stations: list[str] | None = None`；pre-validate regex；組 Qdrant `should` filter；空 list 視為 None
- [x] 2.5 [P] RED `sidecar/tests/kb/test_upsert_chunk.py::test_hash_dedup_short_circuits`：populated KB 含 chunk `text="hello"` 後再 `upsert_chunk("hello", payload)` 回 `"dedup:hash"`、provider.embed 未呼叫（monkeypatch spy）、Qdrant upsert 未呼叫
- [x] 2.6 [P] RED `test_upsert_chunk.py::test_similarity_dedup_after_embed`：populated KB 含相關但不同 hash 的既有 chunk；`upsert_chunk("hello rephrased")` 設定 mock provider 讓 find_similar 回 score=0.97；assertion：回 `"dedup:sim"`、embed 呼叫 1 次、Qdrant upsert 未呼叫
- [x] 2.7 [P] RED `test_upsert_chunk.py::test_new_chunk_returns_point_id`：novel hash + similar 不超 0.95；upsert_chunk 回非空 string + 不以 `"dedup:"` 開頭、Qdrant upsert 呼叫 1 次、回值即為 point_id
- [x] 2.8 [P] RED `test_upsert_chunk.py::test_dedup_token_format_reserved`：所有 `"dedup:"` 開頭 return value 必屬 `{"dedup:hash", "dedup:sim"}`，無第三變體
- [x] 2.9 GREEN — 實作 `KnowledgeBase.upsert_chunk(text: str, *, payload: KBPayload) -> str`：Layer 1 `exists_by_hash` → embed → Layer 2 `find_similar(threshold=0.95)` → Qdrant upsert；用 `payload` 構造 Qdrant point；回 string per dedup token spec

## 3. Sanitizer Pass 3 hook（sanitizer capability：`Pass 3 add_to_kb sanitize emits structured audit entry`）

對應 design Decision 3（Pass 3 source label `qa_add_to_kb`，沿用 FileSource 不擴 union）。

- [x] 3.1 [P] RED `sidecar/tests/sanitizer/test_pass3_add_to_kb_audit.py::test_pass_num_3_on_audit_line`：構 sanitizer + audit logger；`sanitizer.sanitize(text="email is foo@example.com", source=FileSource(path="src/x.py:10-20", pass_="qa_add_to_kb"))` → `audit.append(entry, pass_num=3, ...)`；讀 sanitize_audit.jsonl 第一行 `pass_num == 3`
- [x] 3.2 [P] RED `test_pass3_add_to_kb_audit.py::test_source_field_structured_form`：同上，audit line `source` 欄為 `{"pass": "qa_add_to_kb", "path": "src/x.py:10-20"}` JSON object（非 legacy `"file:..."` string）
- [x] 3.3 [P] RED `test_pass3_add_to_kb_audit.py::test_sanitize_source_union_not_extended`：grep `sidecar/src/codebus_agent/sanitizer/engine.py` 抓 `SanitizeSource =` 行；右值字面 `FileSource | MessageSource`，不含 `Pass3Source` / `QASource`
- [x] 3.4 [P] RED `test_pass3_add_to_kb_audit.py::test_empty_post_sanitize_still_records_hits`：構造文字使每個字都被 redact；assertion：sanitize_audit.jsonl 仍有 `pass_num=3` line（即便 caller 之後決定 skip KB write）
- [x] 3.5 GREEN — 確認 3.1-3.4 全綠（sanitizer engine 不需動 — 既有簽名已支援，本 task 只驗證 + 之後 add_to_kb 會用此 path）

## 4. QA prompts + state types（qa-agent capability：`Q&A system prompt module is isolated from Explorer prompts` + `QAState, QAAnswer, and QAAction are Pydantic models`）

對應 design Decision 1（Q&A 不 reuse Judge / Coverage instance、自帶 prompts module）。

- [x] 4.1 [P] RED `sidecar/tests/agent/test_qa_prompts.py::test_qa_prompt_module_exposes_required_symbols`：`from codebus_agent.agent.prompts import qa as qa_prompts`；hasattr `QA_SYSTEM` / `render_qa_prompt` / `QA_PROMPT_VERSION`；`re.match(r"^\d{4}-\d{2}-\d+$", QA_PROMPT_VERSION)`（注：版本格式 `YYYY-MM-DD-N`，整 regex `^\d{4}-\d{2}-\d{2}-\d+$`）
- [x] 4.2 [P] RED `test_qa_prompts.py::test_qa_module_does_not_import_explorer_or_judge_or_coverage`：用 `ast.parse(open(qa.py).read())` 找 imports；assert no import from `codebus_agent.agent.prompts.explorer` / `prompts.judge` / `prompts.coverage`；同樣對 `agent/qa.py`
- [x] 4.3 [P] RED `test_qa_prompts.py::test_system_prompt_contains_three_worth_persisting_rules`：`QA_SYSTEM` 字串含「可復用」/「stable fact」/「非同義重複」三段（中文 substring 各一）+ 站台 id 格式 regex `^s\d{2}-` substring
- [x] 4.4 GREEN — 新 `sidecar/src/codebus_agent/agent/prompts/qa.py`：`QA_SYSTEM` 三段 prompt + `render_qa_prompt(state, question, initial_hits)` + `QA_PROMPT_VERSION="2026-04-26-1"`；prompt 文字按 `docs/qa-agent.md §五`「值得沉澱」三條件
- [x] 4.5 [P] RED `sidecar/tests/agent/test_qa_types.py::test_qastate_round_trip`：populated `QAState(question="x", originating_station_id="s02-x", session_id="qa_sess", messages=[...], step_count=3, add_to_kb_session_count=2, add_to_kb_question_count=1)` → `model_dump_json` → `model_validate_json` byte-equal
- [x] 4.6 [P] RED `test_qa_types.py::test_qaaction_compatible_with_explorer_action_shape`：`QAAction` 有 `thought: str` + `tool_calls: list[ToolCall]` 欄位；`isinstance` 不要求但欄位 schema 一致
- [x] 4.7 [P] RED `test_qa_types.py::test_qaanswer_citations_schema`：`QAAnswer(answer="text", citations=[KBCitation(file_path="x.py", line_start=1, line_end=10, related_stations=["s01-x"])])`；citation 4 欄齊
- [x] 4.8 GREEN — 改 `sidecar/src/codebus_agent/agent/types.py` 加 `QAState` / `QAAction` / `QAAnswer` / `KBCitation` 四個 Pydantic BaseModel；過 4.5-4.7

## 5. QATools — kb_search tool（qa-agent capability：`QATools exposes seven tools with audit_fields declared` + `kb_search invokes KnowledgeBase query with optional station filter`）

對應 design Decision 8（audit_fields 不收錄 free-text 欄位）。

- [x] 5.1 [P] RED `sidecar/tests/agent/tools/test_kb_search.py::test_kbsearchargs_validates_station_id_format`：`KBSearchArgs(query="x", station_filter=["s02-storage"])` 成功；`KBSearchArgs(query="x", station_filter=["bad"])` 拋 `pydantic.ValidationError`
- [x] 5.2 [P] RED `test_kb_search.py::test_forwards_station_filter_to_kb_query`：mock `ctx.kb.query`；`kb_search(KBSearchArgs(query="x", station_filter=["s02-x"]), ctx)`；spy 看 `ctx.kb.query` call 帶 `filter_stations=["s02-x"]`
- [x] 5.3 [P] RED `test_kb_search.py::test_hit_rendering_omits_empty_station_list`：mock kb hit `payload.related_stations=[]`；`kb_search` 回字串不含 `"stations="` substring
- [x] 5.4 [P] RED `test_kb_search.py::test_hit_rendering_includes_station_list_when_nonempty`：mock kb hit `related_stations=["s02-storage"]`；回字串含 `"stations=[s02-storage]"`
- [x] 5.5 [P] RED `test_kb_search.py::test_audit_fields_declaration`：`QATools.kb_search.audit_fields == ["query", "top_k", "station_filter"]`
- [x] 5.6 GREEN — 新 `sidecar/src/codebus_agent/agent/tools/kb_search.py`：`KBSearchArgs` Pydantic + `kb_search(args, ctx)` impl + `audit_fields` 屬性

## 6. QATools — add_to_kb tool（qa-agent capability：`add_to_kb pipeline runs sanitize, validate, upsert, growth-log in order`）

- [x] 6.1 [P] RED `sidecar/tests/agent/tools/test_add_to_kb.py::test_pipeline_order`：mock sanitizer / sanitizer_audit / kb / kb_growth_logger；`add_to_kb(...)` 一個 chunk；用 `unittest.mock.MagicMock.mock_calls` 檢查呼叫順序：sanitize → sanitizer_audit.append(`pass_num=3`) → kb.upsert_chunk → kb_growth_logger.write
- [x] 6.2 [P] RED `test_add_to_kb.py::test_empty_post_sanitize_chunk_skipped`：sanitize 回空字串 chunk；assert `kb.upsert_chunk` 未呼叫 + `kb_growth_logger.write` 未呼叫 + 回字串含 `"skipped_empty"`
- [x] 6.3 [P] RED `test_add_to_kb.py::test_dedup_hit_records_growth_log_with_dedup_skipped_true`：`kb.upsert_chunk` 回 `"dedup:hash"`；`kb_growth_logger.write` 仍呼叫且 kwargs `dedup_skipped=True`；回字串含 `"dedup:hash"`
- [x] 6.4 [P] RED `test_add_to_kb.py::test_invalid_station_id_aborts_before_upsert`：chunk `related_stations=["s2-bad"]`；回字串以 `"invalid station_id:"` 開頭含 `"s2-bad"`；`kb.upsert_chunk` 未呼叫
- [x] 6.5 [P] RED `test_add_to_kb.py::test_audit_fields_excludes_chunks`：`QATools.add_to_kb.audit_fields` 不含 `"chunks"`、含 `["source", "reason", "related_stations"]`
- [x] 6.6 [P] RED `test_add_to_kb.py::test_per_session_budget_returns_string_error`：state 累積 20 次成功 add_to_kb；第 21 次 chunk 走 `add_to_kb` 回字串以 `"budget exhausted:"` 開頭、sanitize / kb / growth-log 都未呼叫
- [x] 6.7 [P] RED `test_add_to_kb.py::test_oversize_chunk_rejected_without_kb_or_growth_log`：chunk text post-sanitize 2001 chars；該 chunk skip + 不寫 KB / 不寫 growth-log；其他 OK 的 chunk 仍處理
- [x] 6.8 GREEN — 新 `sidecar/src/codebus_agent/agent/tools/add_to_kb.py`：`AddToKBArgs` / `AddToKBChunk` Pydantic + `add_to_kb(args, ctx)` 五階段 pipeline + budget guard 順序：budget check 先 → sanitize → validate → upsert → growth-log；audit_fields 屬性

## 7. QATools 集合（qa-agent capability：剩餘 `QATools exposes seven tools` 未滿足部分）

對應 design Decision 1（不 reuse Judge / Coverage instance）。

- [x] 7.1 RED `sidecar/tests/agent/tools/test_qa_tools.py::test_seven_tools_with_audit_fields`：構 QATools instance；assert hasattr 七個 method（search / list_dir / read_file / trace_import / find_callers / kb_search / add_to_kb），每個都有 `audit_fields: list[str]`
- [x] 7.2 RED `test_qa_tools.py::test_register_with_tool_sandbox_does_not_raise`：構 ToolSandbox + 註冊 QATools 全 7 個 tool；註冊不 raise（per tool-sandbox spec audit_fields rule）
- [x] 7.3 RED `test_qa_tools.py::test_reused_read_tools_delegate_to_folder_tools_semantics`：QATools.search 與 FolderTools.search 對同 ctx 回相同結果（用 spy 或 result equality 檢查）
- [x] 7.4 GREEN — 新 `sidecar/src/codebus_agent/agent/tools/qa_tools.py::QATools`：reuse 5 read tools（直 delegate 或 inline reuse）+ kb_search / add_to_kb；統一 audit_fields

## 8. run_qa 主迴圈 + budget 常數（qa-agent capability：`Q&A loop entry point with two-stage RAG-first flow` + `_hits_confident declares three threshold conditions` + `Q&A budget constants are module-level`）

對應 design Decision 2（RAG-first 兩階段、不直接進 ReAct）+ Decision 1。

- [x] 8.1 [P] RED `sidecar/tests/agent/test_qa_budget_constants.py::test_constants_present_with_correct_values`：`from codebus_agent.agent.qa import _QA_MAX_STEPS, _QA_MAX_ADD_TO_KB_PER_SESSION, _QA_MAX_CHUNK_SIZE_CHARS, _QA_MAX_ADD_TO_KB_PER_QUESTION, _QA_DEDUP_THRESHOLD`；值分別 10 / 20 / 2000 / 5 / 0.95
- [x] 8.2 [P] RED `sidecar/tests/agent/test_hits_confident.py::test_all_three_conditions_met`：populated hits with top-1=0.82 / top-3 mean=0.75 / top-5 含 question entity；`_hits_confident` 回 `True`
- [x] 8.3 [P] RED `test_hits_confident.py::test_insufficient_hits_returns_false`：`hits=[KBHit(score=0.99)]`（length 1）；回 `False`
- [x] 8.4 [P] RED `test_hits_confident.py::test_high_top1_no_entity_coverage_returns_false`：`hits[0].score=0.90` 但 top-5 無 question 任一 significant token；回 `False`
- [x] 8.5 [P] RED `test_hits_confident.py::test_low_top1_returns_false`：`hits[0].score=0.74`；回 `False`（即便其他條件過）
- [x] 8.6 [P] RED `sidecar/tests/agent/test_run_qa.py::test_confident_hits_skip_react_loop`：mock kb 回 confident hits、mock provider；`run_qa(...)` 回 `QAAnswer`、reasoning_log.jsonl 行數 0（cheap path 不寫 Step）
- [x] 8.7 [P] RED `test_run_qa.py::test_non_confident_hits_enter_react_loop`：mock kb 回 weak hits；`run_qa(...)` 進 ReAct loop、reasoning_log.jsonl 至少有 1 個 Step；`_should_stop` 收斂後回 QAAnswer
- [x] 8.8 [P] RED `test_run_qa.py::test_step_limit_via_should_stop`：scripted provider 永遠回 tool_call 製造無限 loop；`_QA_MAX_STEPS=10` 前 ≤10 step 收斂、step_count 不超 10
- [x] 8.9 [P] RED `test_run_qa.py::test_qa_does_not_instantiate_judge_or_coverage`：import-graph：`ast.parse(open("agent/qa.py").read())` 找到的 imports 不含 `LLMJudge` / `LLMCoverageChecker` / `Judge` / `CoverageChecker`
- [x] 8.10 [P] RED `test_run_qa.py::test_reasoning_log_has_qa_prompt_version_not_explorer`：跑 `run_qa` 進 ReAct loop；reasoning_log.jsonl 每 Step 含 `qa_prompt_version` 等於 `QA_PROMPT_VERSION` 常數、不含 `explorer_prompt_version` / `judge_prompt_version`
- [x] 8.11 GREEN — 新 `sidecar/src/codebus_agent/agent/qa.py`：5 個 budget 常數 + `_significant_tokens(text)` helper + `_hits_confident(question, hits)` 三條件檢查 + `_answer_from_hits(question, hits, provider)` cheap path + `_build_qa_prompt` + `_synthesize_answer` + `run_qa(...)` 三階段 entry point；reuse `_think` / `_execute_tools` / `_should_stop` / `ReasoningLogger`；`ReasoningLogger.write` 注入 `qa_prompt_version` 欄

## 9. POST /qa endpoint + dependency check（sidecar-runtime capability：`Q&A task spawn endpoint` + `task_id format` MODIFIED + `Background task error containment` MODIFIED）+ kb_growth_logger_factory 注入

對應 design Decision 5（dependency 全齊備才 spawn）+ Decision 6（task_id regex 擴 qa）。

- [x] 9.1 [P] RED `sidecar/tests/api/test_qa_endpoint.py::test_empty_question_returns_422`：構 app + bearer；POST /qa with `question=""` → 422、body 含 `question` field name；`TaskRegistry` 未消耗
- [x] 9.2 [P] RED `test_qa_endpoint.py::test_oversize_question_returns_422`：4001 char question → 422 含 maxLength 約束
- [x] 9.3 [P] RED `test_qa_endpoint.py::test_invalid_originating_station_id_returns_422`：`originating_station_id="bad"` → 422 含 regex 約束
- [x] 9.4 [P] RED `test_qa_endpoint.py::test_missing_dependency_returns_503_with_detail_listing_slots`：構 app 但 `kb_growth_logger_factory=None`（其他 slot 齊）；POST /qa → 503 / body `code="QA_NOT_CONFIGURED"` / `detail` 含 `"kb_growth_logger_factory"`；多個 slot 缺時 detail 列全
- [x] 9.5 [P] RED `test_qa_endpoint.py::test_successful_spawn_returns_qa_task_id`：所有 slot 齊；POST /qa 合法 body → 200 / `task_id` 過 `^qa_[0-9a-f]{8}$`；TaskRegistry 占 1 slot
- [x] 9.6 [P] RED `test_qa_endpoint.py::test_in_flight_qa_task_blocks_concurrent_qa`：先 POST /qa 進行中；第二 POST /qa → 409 `TASK_IN_FLIGHT`、第二 coroutine 未 spawn
- [x] 9.7 [P] RED `test_qa_endpoint.py::test_question_text_never_echoed_in_error_message`：mock run_qa 拋 `RuntimeError("question='secret payload'")`；SSE stream 收到 `error` event `code="QA_FAILED"`、`message` 不含 `"secret payload"` substring
- [x] 9.8 [P] RED `sidecar/tests/api/test_audit_paths_kb_growth.py::test_factory_kb_growth_logger_lands_under_codebus`：`wire_kb_dependencies` 後 `app.state.kb_growth_logger_factory(<ws>)` 內部 path 為 `<ws>/.codebus/kb_growth.jsonl`；無 openai key 時 factory `is None`
- [x] 9.9 [P] RED `sidecar/tests/api/test_task_id_qa_kind.py::test_qa_kind_matches_regex`：手動建 qa task_id；過 `^qa_[0-9a-f]{8}$`；`TaskKind` enum 含 `"qa"`
- [x] 9.10 GREEN — 改 `sidecar/src/codebus_agent/api/tasks.py`：`TaskKind` Literal 加 `"qa"`、`task_id` regex 擴 `qa` 分支、`_run_background_task` 錯誤碼表加 `QA_FAILED`
- [x] 9.11 GREEN — 新 `sidecar/src/codebus_agent/api/qa.py`：`QARequest` Pydantic + `_require_qa_deps(app_state) -> list[str]` + `POST /qa` handler；handler 順序：validate → dep check 503 → registry register → spawn coroutine → 200 task_id
- [x] 9.12 GREEN — 改 `sidecar/src/codebus_agent/api/__init__.py`：`wire_kb_dependencies(...)` 加 `kb_growth_logger_factory` 工廠（在有 openai key 條件分支）；`include_router(qa_router)`；無 openai 時 `kb_growth_logger_factory = None`

## 10. SSE 事件 + 整合（qa-agent capability：`Q&A run emits SSE events on the task channel`）

- [x] 10.1 [P] RED `sidecar/tests/api/test_qa_sse_events.py::test_rag_hits_emitted_once_after_initial_query`：scripted run_qa；SSE event sequence 含恰 1 個 `rag_hits`、payload `hits` ≤8 + 每 hit 含 `score` / `file_path` / `line_start` / `line_end` / `snippet` / `related_stations`
- [x] 10.2 [P] RED `test_qa_sse_events.py::test_rag_hits_precedes_any_agent_thought`：confident=False scripted；event sequence 中 `rag_hits` index < 任一 `agent_thought` index
- [x] 10.3 [P] RED `test_qa_sse_events.py::test_kb_growth_event_emitted_on_new_chunk`：scripted run_qa 觸發 add_to_kb 寫 1 新 chunk；SSE 含恰 1 個 `kb_growth` event 帶 `entry_id` / `source` / `related_stations` / `originating_station_id`
- [x] 10.4 [P] RED `test_qa_sse_events.py::test_kb_growth_event_omitted_on_dedup_skip`：scripted upsert_chunk 回 `"dedup:hash"`；SSE 無 `kb_growth` event；但 kb_growth.jsonl 仍寫 line `dedup_skipped=True`
- [x] 10.5 [P] RED `test_qa_sse_events.py::test_qa_answer_payload_schema_p0`：成功 run；SSE 最後 `qa_answer` event 1 個、payload `answer: str` + `citations: list[{file_path, line_start, line_end, related_stations}]`
- [x] 10.6 GREEN — 改 `agent/qa.py` 串 emitter；`run_qa` 三階段對應 emit `rag_hits` / ReAct（既有）/ `qa_answer`；`add_to_kb` tool 在 growth-log 寫成功後 emit `kb_growth`；usage_delta / llm_call 由 TrackedProvider 既有路徑（`default_module="qa_agent"` 由工廠注入）
- [x] 10.7 GREEN — 改 `api/qa.py` 把 emitter 注入 `run_qa` + 把 `default_module="qa_agent"` 傳給 chat_provider 工廠（沿用 reasoning / coverage 模式）

## 11. End-to-end smoke + golden（防回歸）

- [x] 11.1 RED `sidecar/tests/integration/test_qa_end_to_end.py::test_confident_path_full_stack`：scripted KB + provider；POST /qa → SSE 全收 `rag_hits` + `qa_answer` + `usage_delta`（>=1 筆 module="qa_agent"）；無 `agent_thought` / `kb_growth` / `error`；reasoning_log.jsonl 0 行
- [x] 11.2 RED `test_qa_end_to_end.py::test_react_path_with_add_to_kb_full_stack`：scripted KB（hits 不 confident）+ scripted ReAct 序列含 1 次 `add_to_kb`；SSE 全包含 `rag_hits` / 多 `agent_thought` / 1 個 `kb_growth` / 1 個 `qa_answer` / `usage_delta`；7 層 audit JSONL 全填（sanitize Pass 3 hit / tool_audit add_to_kb / kb_growth.jsonl 1 行 / reasoning_log 多行 / token_usage `module="qa_agent"` / llm_calls 多筆）
- [x] 11.3 GREEN — 確認 11.1 / 11.2 全綠，golden chain integrity 鎖死

## 12. Documentation 連動更新

- [x] 12.1 改 `docs/implementation-plan.md` 步驟 25 改 🚧（apply 階段）→ 待 archive 改 ✅
- [x] 12.2 改 `docs/qa-agent.md §十二` 連動更新清單 P0 全 `[x]`（5 條：sanitizer.md §三 / agent-core.md §十四 / agent-explorer-spec.md §十二 / sidecar-api.md / interactive-tutorial.md）
- [x] 12.3 改 `docs/decisions.md` D-016 後續清單補一條 `[x] Module 8 Q&A P0 落地（module-8-qa-p0）`、D-021 / D-022 各補一條 `[x] qa_agent module 拆帳`
- [x] 12.4 改 `docs/reviews/2026-04-25-stage-4.md` Module 8 預警 4 條全 `[x]`、Cat 3 #3 prompt fork 加註「Q&A 不 reuse Judge/Coverage instance、自帶 prompts module，自然解決 — module-8-qa-p0, 2026-04-XX」
- [x] 12.5 改 `CLAUDE.md`：archive 表加 row、七層 audit 段 `kb_growth.jsonl` 從 ⏳ 翻 ✅、`目前沒有 in-progress change` 段下一步指向步驟 26（Phase 6 前端 / `auth-flow`）
- [x] 12.6 改 `docs/sidecar-api.md §三` 加 `POST /qa` 條目 + §四加 `rag_hits` / `kb_growth` / `qa_answer` SSE event schema

## 13. 完整驗證 + commit gate

- [x] 13.1 `uv run pytest sidecar/tests/ -q` 完整 suite 全綠（baseline 756 passed → 預期 ~800+ passed 含本 change ~45 新測；既有 1 deselected Windows handshake flake 維持）
- [x] 13.2 `pre-commit run --all-files` 全綠
- [x] 13.3 `spectra validate --strict` 整個 change 合法
- [x] 13.4 Grep `"kb_growth.jsonl"` 在 `sidecar/src/` 下確認只剩 `api/_audit_paths.py` 一處字面量
- [x] 13.5 確認 5 個 capability spec 的所有 ADDED / MODIFIED Requirement 都有對應 production code + test，0 spec drift
- [x] 13.6 Manual smoke：`uv run python scripts/smoke_qa_endpoint.py`（新腳本，呼一次真 OpenAI gpt-4o-mini，驗證 7 層 audit 全寫；可選跑，無 OpenAI key 時 skip）

## 14. 規格 / 設計覆蓋錨點（analyzer cross-ref，每條對應 spec Requirement 或 design Decision，apply 階段視為純驗證 checkbox — 對應任務在前述 1–13 節）

- [x] 14.1 Spec coverage：`Q&A loop entry point with two-stage RAG-first flow` 由 8.6 / 8.7 / 8.11 共同滿足
- [x] 14.2 Spec coverage：`_hits_confident` declares three threshold conditions — 由 8.2 / 8.3 / 8.4 / 8.5 / 8.11 共同滿足
- [x] 14.3 Spec coverage：`Q&A budget constants are module-level` 由 8.1 / 8.8 / 6.6 / 6.7 / 8.11 共同滿足
- [x] 14.4 Spec coverage：`Q&A system prompt module is isolated from Explorer prompts` 由 4.1 / 4.2 / 4.3 / 4.4 / 8.10 共同滿足
- [x] 14.5 Spec coverage：`QATools exposes seven tools with audit_fields declared` 由 7.1 / 7.2 / 7.3 / 7.4 / 5.5 / 6.5 共同滿足
- [x] 14.6 Spec coverage：`kb_search invokes KnowledgeBase query with optional station filter` 由 5.1 / 5.2 / 5.3 / 5.4 / 5.6 共同滿足
- [x] 14.7 Spec coverage：`add_to_kb pipeline runs sanitize, validate, upsert, growth-log in order` 由 6.1 / 6.2 / 6.3 / 6.4 / 6.8 共同滿足
- [x] 14.8 Spec coverage：`Q&A run emits SSE events on the task channel` 由 10.1 / 10.2 / 10.3 / 10.4 / 10.5 / 10.6 / 10.7 共同滿足
- [x] 14.9 Spec coverage：`QAState, QAAnswer, and QAAction are Pydantic models` 由 4.5 / 4.6 / 4.7 / 4.8 共同滿足
- [x] 14.10 Spec coverage：`KBGrowthLogger writes kb_growth.jsonl` 由 1.5 / 1.6 / 1.10 / 1.11 共同滿足
- [x] 14.11 Spec coverage：`Required fields on every kb_growth.jsonl line` 由 1.6 / 1.9 / 1.10 共同滿足
- [x] 14.12 Spec coverage：`Event type field defaults to "add" with rollback reserved for P1` 由 1.7 / 1.10 共同滿足
- [x] 14.13 Spec coverage：`kb_growth_logger_factory wired into app.state` 由 9.8 / 9.12 共同滿足
- [x] 14.14 Spec coverage：`kb_growth.jsonl path constant lives alongside other audit filenames` 由 1.1 / 1.2 / 1.3 / 1.4 / 13.4 共同滿足
- [x] 14.15 Spec coverage：`Pass 3 add_to_kb sanitize emits structured audit entry` 由 3.1 / 3.2 / 3.3 / 3.4 / 3.5 共同滿足
- [x] 14.16 Spec coverage：`KnowledgeBase query and find_similar API`（MODIFIED — filter_stations 加入）由 2.1 / 2.2 / 2.3 / 2.4 共同滿足
- [x] 14.17 Spec coverage：`KnowledgeBase exposes upsert_chunk for Q&A add_to_kb path` 由 2.5 / 2.6 / 2.7 / 2.8 / 2.9 共同滿足
- [x] 14.18 Spec coverage：`task_id format`（MODIFIED — qa kind 加入）由 9.9 / 9.10 共同滿足
- [x] 14.19 Spec coverage：`Background task error containment`（MODIFIED — QA_FAILED 加入）由 9.7 / 9.10 共同滿足
- [x] 14.20 Spec coverage：`Q&A task spawn endpoint` 由 9.1 / 9.2 / 9.3 / 9.4 / 9.5 / 9.6 / 9.11 共同滿足
- [x] 14.21 Design anchor：Decision 1: Q&A 不 reuse Judge / Coverage instance，自帶 prompts module — 由 4.1-4.4 / 8.9 / 8.10 落地
- [x] 14.22 Design anchor：Decision 2: RAG-first 兩階段（cheap path 先），不直接進 ReAct — 由 8.2-8.7 落地
- [x] 14.23 Design anchor：Decision 3: Pass 3 Sanitizer source label `qa_add_to_kb`，沿用 FileSource 不擴 union — 由 3.1-3.5 落地
- [x] 14.24 Design anchor：Decision 4: `KnowledgeBase.upsert_chunk` 雙層 dedup，Layer 2 走 find_similar 重用 — 由 2.5-2.9 落地
- [x] 14.25 Design anchor：Decision 5: Q&A 任務啟動前所有 dependency slot 必須齊備 — 由 9.4 / 9.11 落地
- [x] 14.26 Design anchor：Decision 6: `task_id` regex 直接擴一個 `qa` 分支 — 由 9.9 / 9.10 落地
- [x] 14.27 Design anchor：Decision 7: `kb_growth.jsonl` 預留 `rollback` event 形狀但 P0 不寫 rollback 路徑 — 由 1.7 / 1.10 落地
- [x] 14.28 Design anchor：Decision 8: tool `audit_fields` 不收錄 free-text 欄位 — 由 5.5 / 6.5 落地
