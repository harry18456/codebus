## Summary

Stage 5 Review #2 Cat 2 剩 13 條 spec wording drift 一次性對齊 production 真值，純 spec 編輯無 production code / test 變動。

## Motivation

`docs/reviews/2026-04-26-stage-5.md` Cat 2 共 28 條 drift；3 條由先前 archive、8 條由 `spec-cleanup-stage-5-batch-a`、4 條由 `agent-defense-depth` 收尾，剩 13 條都是純 spec wording / 補 Scenario 的編輯，**沒有 production code / test 改動需求**。把剩餘條目一次清完才能讓 Stage 5 Review #2 Cat 2 完全收齊（28/28），Phase 6 前端動工時不再卡 spec 不對齊。

13 條按性質拆兩組：

- **9 條 STILL DRIFT 修錯**（spec 與 production 真值脫鉤）：D2.7 / D2.8 / D2.10 / D2.11 / D2.13 / D2.16 / D2.17 / D2.18 / D2.28
- **4 條 NEW 補 Scenario**（production 已實作、spec 未鎖）：D2.23 / D2.24 / D2.25 / D2.26

關聯 ADR：D-015（Sanitizer 三段式）/ D-021（usage_tracking 單一寫入路徑）/ D-022（llm_calls.jsonl 紀錄）/ D-016（Q&A add_to_kb 邊界）/ D-029（Module 5 多檔輸出）— 13 條都是把這幾條 ADR 的 production 真值倒回 spec 文字。

## Proposed Solution

### sidecar-runtime（3 條）

- **D2.7** Requirement `KB dependency injection hook` 主文 + Scenario `Both env vars present wire all eight slots` 從 8 slot 改 12 slot：實際是 `kb_backend` / `kb_provider` / `kb_query_provider` / `kb_usage_tracker` / `kb_embedding_dim` / `llm_reasoning_provider` / `llm_judge_provider` / `llm_chat_provider` / `llm_coverage_provider` / `llm_generate_provider` / `llm_qa_provider` / `kb_growth_logger_factory`。Scenario 名一併 rename 為 `Both env vars present wire all twelve slots`
- **D2.8** Requirement `Q&A task spawn endpoint` Scenario `Successful spawn returns task_id` 從 200 改 202（對齊 sidecar/src/codebus_agent/api/qa.py 真值 + 與 explore / generate / kb/build / scan?stream=true 五 endpoint 統一）
- **D2.28** Requirement `Background task error containment` 補 Scenario 列每個 error code 可帶的 extras 欄位：例如 `KB_DIM_MISMATCH` 帶 `expected_dim` / `actual_dim` / `suggestion`（對齊 sidecar/src/codebus_agent/api/tasks.py 內 `_safe_error_message` 真值）

### knowledge-base（2 條）

- **D2.10** Requirement `Embedding batch pipeline with UsageTracker wiring` 主文移除手動 `ctx.usage_tracker.record(usage=..., module="kb_build")` wording，改寫為「`KnowledgeBase.build` MUST NOT call `usage_tracker.record(...)` manually — TrackedProvider 在 embed 成功時自動寫」。對齊 `usage-tracker-dedup` archive 真值 + sidecar/src/codebus_agent/kb/knowledge_base.py 已有的 `KnowledgeBase no longer calls tracker.record(...)` 註解
- **D2.11** Requirement `KnowledgeBase exposes upsert_chunk for Q&A add_to_kb path` example block 從 `default_module="qa_agent"` 改 `default_module="kb_query"`：實際 Q&A endpoint 的 KB embed lane 用 `kb_query_provider` factory（chat lane 才是 `qa_agent`），對齊 sidecar/src/codebus_agent/api/__init__.py + sidecar/src/codebus_agent/agent/qa.py 真值

### explorer-sse（1 條）

- **D2.13** Requirement `POST /explore endpoint spawns Explorer under task registry` 將 `ReasoningLogger(workspace_root / "reasoning_log.jsonl")` 改為 `ReasoningLogger(workspace_root / ".codebus" / "reasoning_log.jsonl")` + 補 caller-mkdir 約束（sidecar/src/codebus_agent/api/explore.py caller-side `mkdir(parents=True, exist_ok=True)`，因為 ReasoningLogger 不 auto-mkdir）。對齊 `audit-path-unification` archive 真值

### explorer-golden（1 條）

- **D2.16** Requirement `Golden replay harness runs under pytest and fails on drift` drift condition `reasoning_log.jsonl line count != pinned step_count` 改為 `not in [step_count, step_count + _COVERAGE_MAX_DEPTH]`（容差來自 `coverage-gap-recurse` archive 後新增的 coverage round Step 寫入；`_COVERAGE_MAX_DEPTH=3` 為 source of truth）

### sanitizer（1 條）

- **D2.17** Requirement `Pass 3 add_to_kb sanitize emits structured audit entry` 主文 + Scenario 加 doc-string 釐清「`pass_num` 是 Python 參數名（`SanitizerAuditLogger.append(pass_num=...)`），jsonl 裡的 key 叫 `pass`（值 1/2/3）」— 把 wording 與 schema key 名脫鉤明寫，避免後續實作者誤以為 jsonl 寫 `pass_num` key

### agent-core（1 條）

- **D2.18** Scenario `FolderTools advertises its tool surface via tool_specs` tool 列表從 4 個（`search` / `list_dir` / `read_file` / `mark_station`）擴成 6 個（補 `trace_import` / `find_callers`），對齊 sidecar/src/codebus_agent/agent/tools/folder_tools.py 內 `_AUDIT_FIELDS` 真值（已在 `explorer-tools-p1` archive 後加上）

### usage-tracking（1 條）

- **D2.23** 補 Requirement `UsageTracker writes token_usage.jsonl` 內新 Scenario `Module field uses one of eight known lane labels` 鎖死 `module` 欄合法值 ∈ `{"kb_build", "kb_query", "reasoning", "judge", "chat", "coverage", "generate", "qa_agent"}`；對齊 sidecar/src/codebus_agent/api/__init__.py 八處 `default_module=` 真值 + CLAUDE.md「七層 Audit JSONL」段已列的 8 lane 名單

### qa-agent（3 條）

- **D2.24** 補 Requirement `Q&A run emits SSE events on the task channel` 內新 Scenario `tokens_used field accepts P0 placeholder zero`，與 explorer-sse capability 既有同名 Scenario 對等（agent-core P0 placeholder 的全域慣例）
- **D2.25** 補 Requirement `add_to_kb pipeline runs sanitize, validate, upsert, growth-log in order` 內新 Scenario `Per-question budget caps add_to_kb at five chunks` 鎖 `_QA_MAX_ADD_TO_KB_PER_QUESTION=5` 邊界（per-session 邊界 spec 已有，per-question 沒有；兩個邊界對等補完）
- **D2.26** 補 Requirement `kb_search returns top-k hits with sanitized snippets` 內新 Scenario `Snippet truncates at 200 characters with ellipsis`，鎖 sidecar/src/codebus_agent/agent/tools/kb_search.py 內 `_SNIPPET_TRUNCATE_LIMIT=200` + `…` ending 真值

## Non-Goals

- **不改任何 production code**：13 條都是 spec 與 code 已脫鉤；本 change 只把 spec 倒回 code 真值，不改 code（code 已對；spec 過時）
- **不改任何測試**：spec wording 變動本身不引入 / 移除測試；既有 853 / 19 baseline 不變
- **不引入新 Requirement**：13 條全部是 MODIFIED Requirements 內加 Scenario 或改主文，沒有新 Requirement
- **不引入新 capability**：8 個受影響 capability 全部已存在
- **不 bump sanitizer rules version**：D2.17 只是 doc clarity 改寫，rules 不動
- **不改 ADR**（`docs/decisions.md`）：13 條都是實作細節 / wording，非架構決策
- **不分次 archive**：13 條一次性收尾；批次 commit 比 13 個 micro-change 重複 propose 省力

## Alternatives Considered

- **拆 13 個 micro-change 各自 propose / archive**（拒絕）— propose / apply / archive overhead × 13，且都是純 spec wording 沒有 cross-impact，集中處理符合 batch-a 已驗證的工作流
- **與 batch-a 合併**（已不可能）— batch-a 已 archive 2026-04-27；當時 batch-a 只挑 3 個 capability 共 8 條，剩餘 13 條跨 8 個 capability 範圍較廣，分批比較可審
- **只改主 spec 不寫 delta**（拒絕）— 違反 Spectra 工作流；spec 變動必須走 change → delta → archive → sync 流程才能留審計軌跡與 unarchive 能力

## Impact

- Affected specs（8 capability MODIFIED Requirements）：
  - openspec/specs/sidecar-runtime/spec.md（D2.7 / D2.8 / D2.28）
  - openspec/specs/knowledge-base/spec.md（D2.10 / D2.11）
  - openspec/specs/explorer-sse/spec.md（D2.13）
  - openspec/specs/explorer-golden/spec.md（D2.16）
  - openspec/specs/sanitizer/spec.md（D2.17）
  - openspec/specs/agent-core/spec.md（D2.18）
  - openspec/specs/usage-tracking/spec.md（D2.23）
  - openspec/specs/qa-agent/spec.md（D2.24 / D2.25 / D2.26）
- Affected code:
  - Modified: （無 production code 變動）
  - New: （無新檔案）
  - Removed: （無刪除）
- Affected docs:
  - docs/reviews/2026-04-26-stage-5.md（13 條 D2.x checkbox `[ ]` → `[x]` + verdict 加「by `spec-cleanup-stage-5-batch-b` archive YYYY-MM-DD」尾註 + 進度狀態表 row「28 → 13（15 條 covered）」改「28 → 0（28 條全 covered）」）
  - CLAUDE.md（archive 表加 row 記錄 13 條 wording cleanup）
- Test suite delta：baseline 853 passed / 19 skipped → 預期 +0（純 spec 不引入測試；apply 階段重跑全 suite 確認 0 regression）
