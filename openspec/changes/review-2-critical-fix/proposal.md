## Problem

`docs/reviews/2026-04-26-stage-5.md`（Review #2，2026-04-26）由 6 個 read-only agent 平行掃出 84 條 issue，其中 **7 條 Critical** 是 Phase 6 前端動工前必須清掉的 production / demo-breaking 問題：

- **CR-1**：`sidecar/src/codebus_agent/api/tasks.py:52` ERROR_CODES 字面量 `KB_EMBED_FAILED`，但 `openspec/specs/sidecar-runtime/spec.md:441` 規定 `KB_BUILD_FAILED`。前端按 spec 寫 exhaustive `code` match 在每個 `/kb/build` 失敗時收到不認識的 code。
- **CR-2**：`sidecar/src/codebus_agent/agent/tools/add_to_kb.py:124-131` 三層 fallback `getattr(sanitizer, "rules_version", None) or "rules-unknown"` → import in `try/except: pass`，會把字串 `"rules-unknown"` 寫進 `sanitize_audit.jsonl.rules_version` 欄，違反不變式 9（rules_version bump）+ `review-backlog-cleanup` 的 single-constant 收緊 + fail-loud 紅線。
- **CR-3**：`KnowledgeBase.upsert_chunk` dedup 路徑只回字面 sentinel `"dedup:hash"` / `"dedup:sim"`；`add_to_kb.py:181-194` 把 sentinel 直接塞 `kb_growth_logger.write(point_id=...)`，違反 `kb-growth/spec.md:83`「dedup-skipped writes MUST record the existing point id reported by the dedup match」。Trust Layer R-01 audit panel join `kb_growth.jsonl.entry_id` 回 Qdrant point 會斷鏈。
- **CR-4**：`docs/sidecar-api.md:588`（§四）+ `docs/qa-agent.md:287`（§八訊息流圖）仍寫 `{"type": "answer_stream", "delta": "..."}` 但 spec.qa-agent.spec.md L518 + production code 都是 `{"type": "qa_answer", "answer": str, "citations": list}`。前端用 EventSource 等 `answer_stream` 永遠等不到答案。
- **CR-5**：`docs/sidecar-api.md:269-274` POST /generate Request body 列 `{workspace_id, explore_task_id, mode}` 完全是虛構欄位；真實 `GenerateRequest` 是 `{workspace_root, task, stations, options}` + `extra="forbid"`。前端 POST 會收 422 + extra-fields-forbidden。
- **CR-6**：`docs/sidecar-api.md:594-600,620` 宣告 `usage_summary` event 必在每個 `done` 之前 emit，但 grep code 完全沒這個 event；前端等 `usage_summary` 渲染 cost panel 會 hang。
- **CR-7**：`docs/sidecar-api.md:507,526` `task_id` regex 寫 `^(scan|kb)_[0-9a-f]{8}$`、ERROR_CODES 列三個。實際 regex 是 `^(scan|kb|explore|generate|qa)_[0-9a-f]{8}$`、ERROR_CODES 共 10 個。前端 exhaustive type-narrow 會炸。

每一條都被多個 agent 重複命中或同份 review 標為 Critical / production-shipping bug。

## Root Cause

這 7 條 Critical 來自三個失誤模式：

1. **Spec ↔ Code 字面量沒 sync**（CR-1 / CR-4 / CR-5 / CR-6 / CR-7）：`module-5-generator-p0`（步驟 24，2026-04-25）+ `module-8-qa-p0`（步驟 25，2026-04-26）兩次 archive 帶來 5 個新 endpoint 與 13 個 SSE event type，`docs/sidecar-api.md` 在 archive 階段只做局部 patch，沒做完整 contract sync。
2. **Defensive code 違反 fail-loud**（CR-2）：`add_to_kb.py` 引入時為了應付「sanitizer 沒有 rules_version 屬性」的測試環境，加了三層 fallback；`review-backlog-cleanup` 落地的 single-constant 收緊沒掃到這個新 callsite。
3. **Sentinel 訊號跟 wire payload 共用同個 channel**（CR-3）：`KnowledgeBase.upsert_chunk` 用字串 prefix `dedup:` 當 discriminator，回傳值同時當「狀態」與「資料」，caller 沒解構直接餵下游 logger。

## Proposed Solution

走一個 Spectra change 把 7 條一次性收掉，分三類處理：

### A. Code fix（CR-1 / CR-2 / CR-3）

- **CR-1**：sidecar/src/codebus_agent/api/tasks.py 把 ERROR_CODES frozenset 的 `KB_EMBED_FAILED` 改 `KB_BUILD_FAILED`；`_classify_exception` 內走 `KB_EMBED_FAILED` 的分支改 `KB_BUILD_FAILED`；`_safe_error_message` 對應分支改名；既有 test 內所有 `KB_EMBED_FAILED` 字面量同步 rename。
- **CR-2**：sidecar/src/codebus_agent/agent/tools/add_to_kb.py:124-131 整段 fallback chain 拆掉，改檔頂 module-level 直接 import RULES_VERSION 使用（對齊 folder_tools.py / tracked.py 既有模式）；補 defensive test 加進 sidecar/tests/sanitizer/test_rules_version_constant.py 鎖死「add_to_kb.py 內 rules_version 變數的取值來源 `is RULES_VERSION`」。
- **CR-3**：sidecar/src/codebus_agent/kb/knowledge_base.py upsert_chunk 簽名改回 `tuple[str, str]`：`(outcome, point_id)`，其中 outcome 屬於 `{"new", "dedup_hash", "dedup_sim"}`；hash dedup path 透過新 helper（從 backend 查 hash 對應的真實 point_id）回真實 point_id；similarity dedup path 從 find_similar 拿到 KBHit 後取 `hit.point_id`。sidecar/src/codebus_agent/agent/tools/add_to_kb.py 解構後餵 kb_growth_logger.write(point_id=real_point_id, dedup_skipped=outcome.startswith("dedup_"))。

### B. Spec sync（CR-1 / CR-3）

- openspec/specs/sidecar-runtime/spec.md：`Background task error containment` Requirement 主文 + Scenario 內所有 KB_EMBED_FAILED 字面量改 KB_BUILD_FAILED（已是 spec 真值，本 change 同步 code 對齊）。
- openspec/specs/knowledge-base/spec.md：`KnowledgeBase exposes upsert_chunk for Q&A add_to_kb path` Requirement 主文簽名改 `(outcome, point_id) tuple` 形式；4 個 Scenario（hash dedup / similarity dedup / new chunk / dedup token format）改寫成 tuple 解構斷言。
- openspec/specs/kb-growth/spec.md：`Required fields on every kb_growth.jsonl line` Requirement 加一條 Scenario `Dedup-skipped write records existing point id`：assert entry_id 必為符合 Qdrant point UUID 格式的字串、不可為 `dedup:` 開頭的 sentinel。

### C. Docs sync（CR-4 / CR-5 / CR-6 / CR-7）

純 docs 修改，不動 spec / code：

- **CR-4**：docs/sidecar-api.md §四 Q&A 專屬事件 answer_stream 整段 wire schema 改 qa_answer（type / answer / citations 含 file_path / line_start / line_end / related_stations）+ 註明「P0 一次性 non-streaming，欄位級 streaming P1 reserved」。docs/qa-agent.md §八訊息流圖同改。docs/qa-agent.md §十二 連動更新清單對應條目改 [x]（已 review 完）。
- **CR-5**：docs/sidecar-api.md §三 POST /generate Request body 整段重寫對齊 GenerateRequest Pydantic（workspace_root / task / stations / options + extra=forbid）；補 Response 202 task_id + 錯誤碼表（GENERATE_NOT_CONFIGURED / GENERATE_WORKSPACE_INVALID / GENERATE_FAILED / TASK_IN_FLIGHT）。
- **CR-6**：docs/sidecar-api.md §四 整段 usage_summary event schema 拿掉；done 事件描述「送出前必先 emit 一筆 usage_summary」改為「送出前不需任何前置 event；client-side 由累積 usage_delta 算 session total」。
- **CR-7**：docs/sidecar-api.md §三-bis task_id regex 從 `^(scan|kb)_[0-9a-f]{8}$` 改 `^(scan|kb|explore|generate|qa)_[0-9a-f]{8}$`；ERROR_CODES 表補齊 10 個（SCAN_FAILED / KB_BUILD_FAILED / INTERNAL_ERROR / OPENAI_AUTH_FAILED / OPENAI_RATE_LIMITED / OPENAI_CONTEXT_EXCEEDED / KB_DIM_MISMATCH / EXPLORE_FAILED / GENERATE_FAILED / QA_FAILED），各帶簡述。

## Non-Goals

- **不處理 Cat 1 doc-stale 22 條**：另走一個獨立 doc-sync commit（不需 Spectra 流程）。
- **不處理 Cat 2 spec-wrong 28 條**：另走 `spec-cleanup-stage-5-batch-1`，本 change 僅收 7 條 Critical。
- **不處理 Cat 2.5 cross-cutting code drift 4 條**：另走 `audit-path-unification-stage-2`。
- **不處理 Cat 3 latent risk 23 條**：backlog，Phase 6 / Phase 2 動工觸到再處理。
- **不處理 Phase 6 預警 4 條**：留給 Phase 6 開工 brief 帶入設計。
- **不重做 Q&A integration test**：本 change 不擴 test coverage，只把 Critical bug 收掉；test 紀律走 `spec-cleanup-stage-5-batch-1` 或獨立 test-coverage change。
- **不 refactor `_QACtxAdapter`** / **不抽 `_STATION_ID_RE` single source**：屬 Cat 2.5 / Cat 3，不在本 change 範圍。
- **不留 backward-compat alias**：`KB_EMBED_FAILED` 直接 rename 為 `KB_BUILD_FAILED`，不保留 alias（沒 production deployment 需要兼容、且 alias 會延長 drift 風險）。

## Success Criteria

- `uv run pytest tests/ -m "not slow"` 全綠（baseline 823 passed / 19 skipped），新增 1 條 defensive test 不破既有測試。
- `spectra validate --strict` 對本 change 三個 MODIFIED capability spec（sidecar-runtime / knowledge-base / kb-growth）全綠。
- Grep `KB_EMBED_FAILED` 在 sidecar/src/ + sidecar/tests/ + openspec/specs/ 全 0 hit。
- Grep `"rules-unknown"` 在 sidecar/src/codebus_agent/ 0 hit。
- Grep `answer_stream` 在 docs/ + openspec/specs/ 0 hit。
- docs/sidecar-api.md POST /generate request body schema 跟 sidecar/src/codebus_agent/api/generate.py 的 GenerateRequest Pydantic 欄位 1:1 對齊。
- docs/sidecar-api.md §三-bis task_id regex + ERROR_CODES 跟 sidecar/src/codebus_agent/api/tasks.py 的 `_VALID_KINDS` + ERROR_CODES 1:1 對齊。
- 跑一次 Q&A integration test，命中 dedup path（upsert_chunk 回 `("dedup_hash", real_point_id)`），`<ws>/.codebus/kb_growth.jsonl` 第一行 entry_id 為符合 UUID 格式的真實字串、不為 `"dedup:hash"` / `"dedup:sim"`。
- `<ws>/.codebus/sanitize_audit.jsonl` 內 add_to_kb Pass 3 命中 line 的 rules_version 欄為 RULES_VERSION 常數真值（`"2026-04-20-1"`）、不為 `"rules-unknown"`。

## Impact

- Affected specs:
  - openspec/specs/sidecar-runtime/spec.md（MODIFIED — KB_BUILD_FAILED rename）
  - openspec/specs/knowledge-base/spec.md（MODIFIED — upsert_chunk 回 tuple）
  - openspec/specs/kb-growth/spec.md（MODIFIED — 補 dedup-skipped entry_id Scenario）
- Affected code:
  - Modified:
    - sidecar/src/codebus_agent/api/tasks.py
    - sidecar/src/codebus_agent/agent/tools/add_to_kb.py
    - sidecar/src/codebus_agent/kb/knowledge_base.py
    - sidecar/tests/api/test_qa_endpoint.py
    - sidecar/tests/api/test_task_error_containment.py
    - sidecar/tests/agent/tools/test_add_to_kb.py
    - sidecar/tests/kb/test_upsert_chunk.py
    - sidecar/tests/integration/test_qa_end_to_end.py
    - sidecar/tests/api/test_qa_sse_events.py
    - sidecar/tests/sanitizer/test_rules_version_constant.py
    - docs/sidecar-api.md
    - docs/qa-agent.md
  - New: 無
  - Removed: 無
