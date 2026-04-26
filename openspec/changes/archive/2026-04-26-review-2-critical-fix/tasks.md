## 1. CR-1: ERROR_CODES KB_EMBED_FAILED → KB_BUILD_FAILED rename（sidecar-runtime / Decision 1）

對應 design Decision 1（`KB_EMBED_FAILED` 直接 rename 為 `KB_BUILD_FAILED`，不留 alias）。spec 滿足 sidecar-runtime Requirement `Background task error containment`。

- [x] 1.1 改 `sidecar/src/codebus_agent/api/tasks.py`：`ERROR_CODES` frozenset 把 `"KB_EMBED_FAILED"` 改 `"KB_BUILD_FAILED"`；`_classify_exception` 內走 `KB_EMBED_FAILED` 的分支同名 rename；`_safe_error_message` 對應條目從「knowledge-base build failed」(KB_EMBED_FAILED) 改為對應 KB_BUILD_FAILED 同訊息
- [x] 1.2 grep `KB_EMBED_FAILED` 在 `sidecar/src/`，所有 hit 同步 rename（預期 0 hit on done）
- [x] 1.3 grep `KB_EMBED_FAILED` 在 `sidecar/tests/`，所有 hit 同步 rename
- [x] 1.4 RED `sidecar/tests/api/test_task_error_containment.py::test_error_codes_frozenset_exact_ten_elements`：assert `ERROR_CODES == {"SCAN_FAILED", "KB_BUILD_FAILED", "EXPLORE_FAILED", "GENERATE_FAILED", "QA_FAILED", "OPENAI_AUTH_FAILED", "OPENAI_RATE_LIMITED", "OPENAI_CONTEXT_EXCEEDED", "KB_DIM_MISMATCH", "INTERNAL_ERROR"}`（精確等於、不多不少）
- [x] 1.5 RED `test_task_error_containment.py::test_kb_embed_failed_alias_not_present`：assert `"KB_EMBED_FAILED" not in ERROR_CODES`（防 alias 復活）
- [x] 1.6 GREEN — 1.1 + 1.4 + 1.5 串聯通過

## 2. CR-2: add_to_kb rules_version fallback 移除（不變式 #9 + fail-loud / Decision 4 + Decision 5）

對應 design Decision 4（`add_to_kb` rules_version 走 module-level import 直接使用，不留 fallback）+ Decision 5（defensive test 鎖在 `test_rules_version_constant.py`，不另開新檔）。

- [x] 2.1 改 `sidecar/src/codebus_agent/agent/tools/add_to_kb.py`：檔頂加 `from codebus_agent.sanitizer import RULES_VERSION`；line 124-131 整段 fallback chain（`getattr` + `try/except: pass`）刪掉，function body 內 `rules_version = RULES_VERSION` 直接使用
- [x] 2.2 RED `sidecar/tests/sanitizer/test_rules_version_constant.py::test_add_to_kb_uses_rules_version_constant_directly`：`from codebus_agent.agent.tools import add_to_kb` 後 `assert add_to_kb.RULES_VERSION is RULES_VERSION`（identity check）
- [x] 2.3 RED `test_rules_version_constant.py::test_no_rules_unknown_literal_in_add_to_kb`：grep `"rules-unknown"` 在 `sidecar/src/codebus_agent/agent/tools/add_to_kb.py`，assert 0 hit
- [x] 2.4 GREEN — 2.1 + 2.2 + 2.3 串聯通過
- [x] 2.5 跑既有 `tests/agent/tools/test_add_to_kb.py` 全綠（rules_version 移除後既有 8 個 mock 測試應沿用 — 確認 sanitizer mock 不需要再 patch `rules_version` 屬性）

## 3. CR-3: KnowledgeBase.upsert_chunk 簽名改 tuple[str, str]（knowledge-base + kb-growth / Decision 2 + Decision 3）

對應 design Decision 2（`upsert_chunk` 簽名改 `tuple[str, str]`，第一欄是 outcome enum-like 字串、第二欄是真實 point_id）+ Decision 3（hash dedup path 加 `_lookup_existing_point_id_by_hash` helper）。spec 滿足 knowledge-base Requirement `KnowledgeBase exposes upsert_chunk for Q&A add_to_kb path` MODIFIED + kb-growth Requirement `Required fields on every kb_growth.jsonl line` MODIFIED（含新 Scenario `Dedup-skipped write records existing point id`）。

- [x] 3.1 改 `sidecar/src/codebus_agent/kb/knowledge_base.py`：新加 private async helper `_lookup_existing_point_id_by_hash(self, text_hash: str) -> str | None`（走 `backend.search_points(collection, vector=[0.0]*dim, limit=1, query_filter={"text_hash": text_hash})` 拿第一筆 hit 的 `point_id`，找不到回 None；comment 寫明「一致性 fallback：未來 backend 加 `find_point_id_by_hash` Protocol method 後可砍掉」）
- [x] 3.2 改同檔 `upsert_chunk` 簽名 `async def upsert_chunk(self, text: str, *, payload: KBPayload) -> tuple[str, str]`；docstring 更新對齊 spec；body 改：
  - hash dedup hit → `existing = await self._lookup_existing_point_id_by_hash(payload.text_hash)`，回 `("dedup_hash", existing or "<unknown>")`（理論上必命中、`<unknown>` 是極罕見 race fallback）
  - similarity dedup hit → 從 `find_similar` 回的 `KBHit` 取 `hit.point_id`，回 `("dedup_sim", hit.point_id)`
  - new path → 既有的 `point_id = str(uuid.uuid4())` + upsert，回 `("new", point_id)`
- [x] 3.3 改 `sidecar/src/codebus_agent/agent/tools/add_to_kb.py:181-194`：`upsert_result = await kb.upsert_chunk(...)` 解構成 `(outcome, real_point_id) = await kb.upsert_chunk(...)`；`dedup_skipped = outcome.startswith("dedup_")`；`growth_logger.write(point_id=real_point_id, ..., dedup_skipped=dedup_skipped)`；`response_tokens.append(outcome)`（取代舊的 `upsert_result` 字串塞 token list）
- [x] 3.4 改 `sidecar/src/codebus_agent/agent/qa.py` 跟其他可能 import `upsert_chunk` 的 module（grep 確認；目前應只 add_to_kb.py 一處 caller）
- [x] 3.5 改 `sidecar/tests/kb/test_upsert_chunk.py`：4 個 Scenario 測試全部改寫成 tuple-based assertion：
  - `test_hash_dedup_short_circuits` → assert 回 `(outcome, point_id)` where `outcome == "dedup_hash"` AND `point_id` 為非空 UUID 且不以 `"dedup:"` 開頭
  - `test_similarity_dedup_after_embed` → 同上 `outcome == "dedup_sim"` AND `point_id == hit.point_id`
  - `test_new_chunk_returns_point_id` → assert `outcome == "new"` AND `point_id` 為新 UUID
  - `test_dedup_token_format_reserved` 改為 `test_outcome_literal_closed_set` → assert outcome ∈ `{"new", "dedup_hash", "dedup_sim"}`，外加 `point_id 不為 dedup: 開頭` assertion
- [x] 3.6 改 `sidecar/tests/agent/tools/test_add_to_kb.py`：所有 mock `kb.upsert_chunk = AsyncMock(return_value=...)` 從字串改 tuple；`test_dedup_hit_records_growth_log_with_dedup_skipped_true` assertion 改成「kwargs `point_id` 為真實 UUID」
- [x] 3.7 改 `sidecar/tests/integration/test_qa_end_to_end.py`：mock `kb.upsert_chunk = AsyncMock(return_value=...)` 從字串改 tuple
- [x] 3.8 改 `sidecar/tests/api/test_qa_sse_events.py`：同 3.7（如果有）
- [x] 3.9 RED `tests/kb/test_upsert_chunk.py::test_dedup_hash_returns_real_existing_point_id`：seed populated KB 一個 chunk 拿到 `existing_pt_id`，再呼 `upsert_chunk` 同 text → assert 回 `("dedup_hash", existing_pt_id)`（嚴格 equality，不是「不為 dedup:」）
- [x] 3.10 RED `tests/integration/test_qa_end_to_end.py::test_dedup_path_writes_real_point_id_to_kb_growth`：scripted KB 命中 dedup → assert `<ws>/.codebus/kb_growth.jsonl` 第一行 `entry_id` 為 UUID 格式（`re.match(r"^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$", line["entry_id"])`）
- [x] 3.11 GREEN — 3.1-3.10 串聯通過

## 4. CR-4: docs answer_stream → qa_answer（Decision 6）

對應 design Decision 6（docs/sidecar-api.md 一次完整 sync，不局部 patch — 第 1/4 part）。

- [x] 4.1 改 `docs/sidecar-api.md` §四（line ~588）Q&A 專屬事件區塊：`answer_stream` 整段 wire schema 改為 `qa_answer`（`{type, answer: str, citations: [{file_path, line_start, line_end, related_stations}]}`）；備註「P0 一次性 non-streaming，欄位級 streaming P1 reserved」
- [x] 4.2 改 `docs/qa-agent.md` §八訊息流圖（line ~287）`answer_stream` → `qa_answer`，欄位同 4.1
- [x] 4.3 改 `docs/qa-agent.md` §十二 連動更新清單對應 sidecar-api.md / qa_answer 條目改 `[x]`（已 review 完）
- [x] 4.4 grep `answer_stream` 在整個 repo（`docs/` + `openspec/specs/` + `sidecar/`），assert 0 hit on done（`tests/` 內若有測 `answer_stream` 字面量需同步處理；本 change 預期 0 hit）

## 5. CR-5: docs POST /generate Request body 重寫（Decision 6）

對應 design Decision 6（docs/sidecar-api.md 一次完整 sync — 第 2/4 part）。

- [x] 5.1 改 `docs/sidecar-api.md` §三 POST /generate 段（line ~268-280）：Request body 重寫對齊 `GenerateRequest` Pydantic：`{workspace_root: str (absolute), task: str (min_length=1), stations: list[Station], options: GeneratorOptions}` + `extra="forbid"` 註記；補 Station / GeneratorOptions schema link 到對應 spec
- [x] 5.2 同段補 Response：202 Accepted `{task_id: "generate_<8hex>"}`；錯誤碼表 `GENERATE_NOT_CONFIGURED`(503) / `GENERATE_WORKSPACE_INVALID`(400) / `TASK_IN_FLIGHT`(409) / `GENERATE_FAILED`(SSE error event)
- [x] 5.3 grep `workspace_id` / `explore_task_id` 在 `docs/sidecar-api.md` POST /generate 段，assert 0 hit on done

## 6. CR-6: docs usage_summary event 拿掉（Decision 6）

對應 design Decision 6（docs/sidecar-api.md 一次完整 sync — 第 3/4 part）。

- [x] 6.1 改 `docs/sidecar-api.md` §四（line ~594-600）拿掉 `usage_summary` event 整段 wire schema
- [x] 6.2 同 §四 `done` event 描述（line ~620）拿掉「送出前必先 emit 一筆 usage_summary」；改為「送出前不需任何前置 event；client-side 由累積 `usage_delta` 算 session total（每筆 `usage_delta` 都帶 `session_total_cost_usd` + `session_total_tokens`）」
- [x] 6.3 grep `usage_summary` 在 `docs/`，assert 0 hit on done

## 7. CR-7: docs §三-bis task_id regex + ERROR_CODES sync（Decision 6）

對應 design Decision 6（docs/sidecar-api.md 一次完整 sync — 第 4/4 part）。

- [x] 7.1 改 `docs/sidecar-api.md` §三-bis（line ~507）task_id regex `^(scan|kb)_[0-9a-f]{8}$` → `^(scan|kb|explore|generate|qa)_[0-9a-f]{8}$`
- [x] 7.2 改同 §三-bis（line ~526）ERROR_CODES 表補齊 10 個：`SCAN_FAILED` / `KB_BUILD_FAILED` / `EXPLORE_FAILED` / `GENERATE_FAILED` / `QA_FAILED` / `OPENAI_AUTH_FAILED` / `OPENAI_RATE_LIMITED` / `OPENAI_CONTEXT_EXCEEDED` / `KB_DIM_MISMATCH` / `INTERNAL_ERROR`，每條附簡述（從 `_safe_error_message` 取真值）
- [x] 7.3 grep `KB_EMBED_FAILED` 在 `docs/`，assert 0 hit on done

## 8. 完整驗證 + commit gate

- [x] 8.1 `uv run pytest sidecar/tests/ -m "not slow" -q` 全綠（baseline 823 → 預期 ~825-828 含本 change ~3-5 新測 + ~5-10 KB_EMBED_FAILED 字面量 rename）
- [x] 8.2 `pre-commit run --all-files` 全綠
- [x] 8.3 `spectra validate --strict` 對 `review-2-critical-fix` 全綠
- [x] 8.4 Grep `KB_EMBED_FAILED` 在 `sidecar/src/` + `sidecar/tests/` + `openspec/specs/` + `docs/`，全 0 hit
- [x] 8.5 Grep `"rules-unknown"` 在 `sidecar/src/codebus_agent/`，0 hit
- [x] 8.6 Grep `answer_stream` 在 `docs/` + `openspec/specs/` + `sidecar/`，0 hit
- [x] 8.7 跑 Q&A integration test，命中 dedup path，assert `<ws>/.codebus/kb_growth.jsonl` 第一行 `entry_id` 為 UUID 格式
- [x] 8.8 跑 Q&A integration test 命中 Pass 3，assert `<ws>/.codebus/sanitize_audit.jsonl` 第一行 `rules_version == "2026-04-20-1"`（或當前 RULES_VERSION 真值）

## 9. Documentation 連動更新

- [x] 9.1 改 `docs/reviews/2026-04-26-stage-5.md` Critical 段 7 條全 `[x]`（標 archive 日期）
- [x] 9.2 改 `CLAUDE.md` archive 表加 row（review-2-critical-fix 收尾）
- [x] 9.3 改 `docs/decisions.md`：D-016 / D-021 / D-022 後續清單若有對應條目則勾 `[x]`（review-2-critical-fix 連帶解掉的）

## 10. 規格 / 設計覆蓋錨點（apply 階段純驗證 checkbox）

- [x] 10.1 Spec coverage：sidecar-runtime `Background task error containment` 由 1.1 / 1.4 / 1.5 共同滿足
- [x] 10.2 Spec coverage：sidecar-runtime Scenario `Error code table is exhaustively enumerated` 由 1.4 滿足
- [x] 10.3 Spec coverage：knowledge-base `KnowledgeBase exposes upsert_chunk for Q&A add_to_kb path` MODIFIED 由 3.1 / 3.2 / 3.5 / 3.9 共同滿足
- [x] 10.4 Spec coverage：kb-growth Scenario `Dedup-skipped write records existing point id` 由 3.10 滿足
- [x] 10.5 Spec coverage：kb-growth Scenario `New write records new point id` 由既有 `test_growth_logger.py` 滿足（assert entry_id UUID 格式）
- [x] 10.6 Design anchor：Decision 1（KB_BUILD_FAILED rename，不留 alias）由 1.1-1.6 落地
- [x] 10.7 Design anchor：Decision 2（upsert_chunk tuple 簽名）由 3.1-3.11 落地
- [x] 10.8 Design anchor：Decision 3（_lookup_existing_point_id_by_hash helper）由 3.1 + 3.9 落地
- [x] 10.9 Design anchor：Decision 4（rules_version 移除 fallback）由 2.1-2.5 落地
- [x] 10.10 Design anchor：Decision 5（test_rules_version_constant.py home）由 2.2 + 2.3 落地
- [x] 10.11 Design anchor：Decision 6（docs/sidecar-api.md 一次完整 sync）由 4.1-4.4 + 5.1-5.3 + 6.1-6.3 + 7.1-7.3 落地
