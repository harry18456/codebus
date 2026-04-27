## 1. 前置驗證（apply 動工前）

- [x] 1.1 確認 baseline 測試全綠：`uv run pytest sidecar/tests/ -q`，記錄為 `853 passed / 19 skipped`（baseline 不變式；本 change 純 spec 編輯，預期 +0 新測，跑完仍是 853 / 19）
- [x] 1.2 `spectra validate spec-cleanup-stage-5-batch-b --strict` 確認 propose 階段建好的 8 個 delta spec 全綠
- [x] 1.3 `spectra analyze spec-cleanup-stage-5-batch-b --json` 確認無 Critical / Warning（Suggestion 級的 RFC 2119 用詞不阻塞）

## 2. 驗證 production 真值對齊 delta spec（並行；apply 階段 sanity check）

- [x] 2.1 [P] **sidecar-runtime D2.7** — grep `app.state.` 在 `sidecar/src/codebus_agent/api/__init__.py` 確認共 12 個 KB-related slot（`kb_backend` / `kb_provider` / `kb_query_provider` / `kb_usage_tracker` / `kb_embedding_dim` / `llm_reasoning_provider` / `llm_judge_provider` / `llm_chat_provider` / `llm_coverage_provider` / `llm_generate_provider` / `llm_qa_provider` / `kb_growth_logger_factory`）。若數字不符，apply 當場修正 delta spec 真值
- [x] 2.2 [P] **sidecar-runtime D2.8** — grep `HTTP_202_ACCEPTED` 在 `sidecar/src/codebus_agent/api/qa.py` 確認 `POST /qa` decorator 已是 202
- [x] 2.3 [P] **sidecar-runtime D2.28** — 讀 `sidecar/src/codebus_agent/api/tasks.py::_enrich_error_event` 確認 `KB_DIM_MISMATCH` extras 真值（`expected_dim` / `actual_dim` / `suggestion="delete collection and rebuild"`），其他 9 個 error code 真值為空 dict `{}`
- [x] 2.4 [P] **knowledge-base D2.10** — grep `tracker.record\|usage_tracker.record` 在 `sidecar/src/codebus_agent/kb/knowledge_base.py` 確認 `KnowledgeBase.build` 確實沒有手動 `record(...)` call（只有 TrackedProvider 自動寫的 path）
- [x] 2.5 [P] **knowledge-base D2.11** — 讀 `sidecar/src/codebus_agent/agent/qa.py` 內 KB upsert path 確認用的是 `kb_query_provider`（即 `default_module="kb_query"` lane），非 `qa_agent` lane
- [x] 2.6 [P] **explorer-sse D2.13** — grep `ReasoningLogger\(` 在 `sidecar/src/codebus_agent/api/explore.py` 確認 path 為 `<workspace>/.codebus/reasoning_log.jsonl` AND 確認 `mkdir(parents=True, exist_ok=True)` 在 ReasoningLogger 構造前 caller-side 執行
- [x] 2.7 [P] **explorer-golden D2.16** — grep `_COVERAGE_MAX_DEPTH` 在 `sidecar/src/codebus_agent/agent/explorer.py` 確認常數值仍為 `3`（若改變，drift condition 容差仍由常數來，spec wording 不受影響）
- [x] 2.8 [P] **sanitizer D2.17** — 讀 `sidecar/src/codebus_agent/sanitizer/audit.py::SanitizerAuditLogger.append` 確認 line dict key 名是 `"pass"`（非 `"pass_num"`）AND Python 參數名是 `pass_num`
- [x] 2.9 [P] **agent-core D2.18** — grep `_AUDIT_FIELDS` 在 `sidecar/src/codebus_agent/agent/tools/folder_tools.py` 確認包含 6 個 tool（`search` / `list_dir` / `read_file` / `mark_station` / `trace_import` / `find_callers`）
- [x] 2.10 [P] **usage-tracking D2.23** — grep `default_module=` 在 `sidecar/src/codebus_agent/api/__init__.py` 確認 8 個 distinct 值（`kb_build` / `kb_query` / `reasoning` / `judge` / `chat` / `coverage` / `generate` / `qa_agent`），與 spec 列表逐字對齊
- [x] 2.11 [P] **qa-agent D2.24** — 讀 `sidecar/src/codebus_agent/agent/qa.py` 確認 SSE emitter 在 ReAct 路徑下確實會 emit `agent_action_result` 帶 `tokens_used: 0` placeholder（現實作對等 explorer-sse）
- [x] 2.12 [P] **qa-agent D2.25** — grep `_QA_MAX_ADD_TO_KB_PER_QUESTION\|add_to_kb_question_count` 在 `sidecar/src/codebus_agent/agent/qa.py` + `sidecar/src/codebus_agent/agent/tools/add_to_kb.py` 確認常數 `=5` 且 budget check 在 step (a) 前執行
- [x] 2.13 [P] **qa-agent D2.26** — grep `_SNIPPET_TRUNCATE_LIMIT` 在 `sidecar/src/codebus_agent/agent/tools/kb_search.py` 確認常數值 `200` AND 截斷 marker 是 `…`（U+2026 Unicode ellipsis）非 `...`

## 3. 完整驗證 + commit gate

- [x] 3.1 `spectra analyze spec-cleanup-stage-5-batch-b --json` 全綠（無 Critical / Warning；Suggestion 級的 RFC 2119 `MAY` / `should` 用詞不阻塞）
- [x] 3.2 `spectra validate spec-cleanup-stage-5-batch-b --strict` 全綠
- [x] 3.3 `uv run pytest sidecar/tests/ -q` 確認 baseline 853 → 預期 +0 新測（純 spec 不引入測試）；零 regression（已知 Windows timing flake `test_startup_remains_available_when_qdrant_unreachable` 與本 change 無因果關係，可忽略）
- [x] 3.4 `pre-commit run --all-files` 全綠

## 4. Documentation 連動更新

- [x] 4.1 改 `docs/reviews/2026-04-26-stage-5.md` Cat 2 段：13 條 D2.x（D2.7 / D2.8 / D2.10 / D2.11 / D2.13 / D2.16 / D2.17 / D2.18 / D2.23 / D2.24 / D2.25 / D2.26 / D2.28）checkbox 改 `[x]`，每條 verdict 行加「by `spec-cleanup-stage-5-batch-b` archive 2026-MM-DD」尾註
- [x] 4.2 改 `docs/reviews/2026-04-26-stage-5.md` 進度狀態表 Cat 2 row 數字從「28 → 13（15 條 covered：3 條由先前 archive + 8 條由 `spec-cleanup-stage-5-batch-a` + 4 條由 `agent-defense-depth`）」改「28 → 0（28 條全 covered：3 條由先前 archive + 8 條由 `spec-cleanup-stage-5-batch-a` + 4 條由 `agent-defense-depth` + 13 條由本 change）」+ row 末 `[ ]` 改 `[x]`
- [x] 4.3 改 `CLAUDE.md` archive 表加 row（spec-cleanup-stage-5-batch-b 收尾），記錄 13 條 wording cleanup + 8 個受影響 capability + 零 production code / test 變動
- [x] 4.4 確認 `docs/decisions.md` 不需新增 ADR — 13 條都是 spec wording / 補 Scenario，非架構決策

## 5. 規格覆蓋錨點（apply 階段純驗證 checkbox）

- [x] 5.1 Spec coverage：D2.7 由 task 2.1 + sidecar-runtime delta MODIFIED Requirement `KB dependency injection hook` + 新 Scenario `Both env vars present wire all twelve slots` / `KB growth logger factory targets the workspace .codebus subdirectory` 滿足
- [x] 5.2 Spec coverage：D2.8 由 task 2.2 + sidecar-runtime delta MODIFIED Requirement `Q&A task spawn endpoint` Scenario `Successful spawn returns task_id` 200 → 202 滿足
- [x] 5.3 Spec coverage：D2.28 由 task 2.3 + sidecar-runtime delta MODIFIED Requirement `Background task error containment` 主文加 extras whitelist + 新 Scenario `KB_DIM_MISMATCH error event carries expected_dim, actual_dim, and suggestion extras` / `Other error codes carry no extras beyond code and message` 滿足
- [x] 5.4 Spec coverage：D2.10 由 task 2.4 + knowledge-base delta MODIFIED Requirement `Embedding batch pipeline with UsageTracker wiring` 主文移除手動 `tracker.record(...)` wording + Scenario `UsageTracker records exactly one entry per batch via TrackedProvider only` 滿足
- [x] 5.5 Spec coverage：D2.11 由 task 2.5 + knowledge-base delta MODIFIED Requirement `KnowledgeBase exposes upsert_chunk for Q&A add_to_kb path` step 2 wording + 新 Scenario `Embed lane is kb_query when called from the Q&A pipeline` 滿足
- [x] 5.6 Spec coverage：D2.13 由 task 2.6 + explorer-sse delta MODIFIED Requirement `POST /explore endpoint spawns Explorer under task registry` path 改 `.codebus/` + 新 Scenario `ReasoningLogger lands under .codebus subdirectory` 滿足
- [x] 5.7 Spec coverage：D2.16 由 task 2.7 + explorer-golden delta MODIFIED Requirement `Golden replay harness runs under pytest and fails on drift` drift condition 加 `_COVERAGE_MAX_DEPTH` 容差 + 新 Scenario `Reasoning log line count within main-loop and coverage-recurse range passes` / `Reasoning log line count outside the tolerance range fails the harness` 滿足
- [x] 5.8 Spec coverage：D2.17 由 task 2.8 + sanitizer delta MODIFIED Requirement `Pass 3 add_to_kb sanitize emits structured audit entry` 主文加 Python `pass_num` vs JSONL `pass` doc-string 釐清 + 新 Scenario `JSONL key is bare pass, never pass_num` 滿足
- [x] 5.9 Spec coverage：D2.18 由 task 2.9 + agent-core delta MODIFIED Requirement `ExplorerTools, Judge, and CoverageChecker are structural Protocols` Scenario `FolderTools advertises its tool surface via tool_specs` 補 `trace_import` / `find_callers` 兩 P1 tool 滿足
- [x] 5.10 Spec coverage：D2.23 由 task 2.10 + usage-tracking delta MODIFIED Requirement `UsageTracker writes token_usage.jsonl` 主文加 8 lane 列舉 + 新 Scenario `Module field uses one of eight known lane labels` 滿足
- [x] 5.11 Spec coverage：D2.24 由 task 2.11 + qa-agent delta MODIFIED Requirement `Q&A run emits SSE events on the task channel` 主文加 `tokens_used` placeholder 約束 + 新 Scenario `tokens_used field accepts P0 placeholder zero` 滿足
- [x] 5.12 Spec coverage：D2.25 由 task 2.12 + qa-agent delta MODIFIED Requirement `add_to_kb pipeline runs sanitize, validate, upsert, growth-log in order` 主文加 per-question 邊界段 + 新 Scenario `Per-question budget caps add_to_kb at five chunks` / `Per-question and per-session ceilings sourced from canonical single source` 滿足
- [x] 5.13 Spec coverage：D2.26 由 task 2.13 + qa-agent delta MODIFIED Requirement `kb_search invokes KnowledgeBase query with optional station filter` 主文加 200-char ceiling 從 `_SNIPPET_TRUNCATE_LIMIT` 來源 + 新 Scenario `Snippet truncates at 200 characters with ellipsis` / `Snippet shorter than 200 characters left intact` 滿足
