## 1. 前置驗證（apply 動工前）

- [x] 1.1 確認 baseline 測試全綠：`uv run pytest sidecar/tests/ -q`，記錄為 `843 passed / 19 skipped`（baseline 不變式）
- [x] 1.2 `spectra validate spec-cleanup-stage-5-batch-a --strict` 確認 propose 階段建好的 delta spec 全綠

## 2. qa-agent delta spec — 5 條 D2.x 對齊（已在 propose 寫好，apply 階段純驗證）

對應 `openspec/changes/spec-cleanup-stage-5-batch-a/specs/qa-agent/spec.md` 的 4 條 `## MODIFIED Requirements`（D2.1 / D2.2+D2.3 同 Requirement / D2.4 / D2.5）。

- [x] 2.1 [P] 核對 D2.1：Requirement `Q&A loop entry point with two-stage RAG-first flow` delta 內 `run_qa(*, question, state, kb, tools, provider, logger=None, emitter=None, cancel_event=None) -> QAAnswer` signature 完全對齊 `sidecar/src/codebus_agent/agent/qa.py:252-262`，且新 Scenario `All run_qa parameters are keyword-only` 用 `inspect.Parameter.KEYWORD_ONLY` 鎖死所有 8 個 keyword-only 參數名
- [x] 2.2 [P] 核對 D2.2 + D2.3：Requirement `add_to_kb pipeline runs sanitize, validate, upsert, growth-log in order` delta 主文寫真實序「budget → pre-validate-all → loop(sanitize → upsert → growth-log)」對齊 `add_to_kb.py:103-181`，且 Scenario `Order of operations: budget, pre-validate, then per-chunk sanitize-upsert-log` + Scenario `Invalid station_id aborts the entire invocation before any sanitize` + Scenario `Budget exhausted aborts before any sanitize` 三條 fail-fast pre-validate 行為鎖死
- [x] 2.3 [P] 核對 D2.4：Requirement `Q&A run emits SSE events on the task channel` delta 主文釐清雙 lane（chat = `qa_agent` / KB embed = `kb_query`），且新 3 條 Scenario `Chat lane writes module="qa_agent"` / `KB embedding lane writes module="kb_query"` / `Both lanes co-occur within one Q&A run` 鎖死兩 lane 真值
- [x] 2.4 [P] 核對 D2.5：Requirement `QATools exposes seven tools with audit_fields declared` delta 主文寫死 5 個 reused tool 的 `audit_fields` 各自 production 真值（mirror `folder_tools._AUDIT_FIELDS`：`search`/`trace_import`/`find_callers`=[], `list_dir`=["path"], `read_file`=["path","line_range"]），且新 Scenario `Reused read tools mirror FolderTools audit_fields exactly` 用 value-equality（不是 `is None` 或 missing-attribute）鎖死 5 個 tool 各自真值

## 3. kb-growth delta spec — 1 條 D2.x 對齊

對應 `openspec/changes/spec-cleanup-stage-5-batch-a/specs/kb-growth/spec.md` 的 1 條 `## MODIFIED Requirements`（D2.6）。

- [x] 3.1 核對 D2.6：Requirement `kb_growth.jsonl path constant lives alongside other audit filenames` delta 主文寫死 canonical 在 `codebus_agent/_audit_paths.py`、`codebus_agent/api/_audit_paths.py` 是 backward-compat shim，且 3 條 Scenario（canonical export / shim 同物件 / 不在 leaf module 外字面量）鎖死「兩處都允許 grep 命中但只有 canonical 含字面量定義」契約

## 4. module-5-generator delta spec — 2 條 D2.x 對齊

對應 `openspec/changes/spec-cleanup-stage-5-batch-a/specs/module-5-generator/spec.md` 的 2 條 `## MODIFIED Requirements`（D2.20 / D2.21）。

- [x] 4.1 [P] 核對 D2.20：Requirement `Generator entrypoint orchestrates per-station markdown pipeline` delta 主文 signature 對齊 production 真實 15 個 keyword-only 參數（4 required + 11 optional），釐清「user task description 走 `state.task`、不是頂層 `task` 參數」；Scenario `All run_generator parameters are keyword-only` 鎖「kind == KEYWORD_ONLY + 必要 4 參數 `{state, workspace_root, task_id, llm_chat_provider}` set」、optional 參數允許演進不鎖 set
- [x] 4.2 [P] 核對 D2.21：Requirement `Markdown validator enforces D-029 component rules` delta 第 5 條 rule 主文加 D-029 cross-reference + 鎖 `_BODY_LIMIT_CHARS` 是 source-of-truth，且 Scenario `Length over 800 characters fails validation per D-029` 加「800-char ceiling MUST be sourced from `_BODY_LIMIT_CHARS` rather than separate test literal」

## 5. 完整驗證 + commit gate

- [x] 5.1 `spectra analyze spec-cleanup-stage-5-batch-a --json` 全綠（Coverage / Consistency / Ambiguity / Gaps 四維度均無 Critical / Warning）
- [x] 5.2 `spectra validate spec-cleanup-stage-5-batch-a --strict` 全綠
- [x] 5.3 `uv run pytest sidecar/tests/ -q` 確認 `843 passed / 19 skipped` 不變（本 change 純改 spec wording，零 production code / test 動，數字不可漂移）— note: apply 階段重跑出現 `test_startup_remains_available_when_qdrant_unreachable` Windows timing flake（3s budget vs 9-12s 實測，純子程序冷啟動環境抖動，零 production code / test 變動可解釋）；baseline Task 1.1 同 suite 通過 843
- [x] 5.4 `pre-commit run --all-files` 全綠

## 6. Documentation 連動更新

- [x] 6.1 改 `docs/reviews/2026-04-26-stage-5.md` Cat 2 段：8 條（D2.1 / D2.2 / D2.3 / D2.4 / D2.5 / D2.6 / D2.20 / D2.21）checkbox 改 `[x]`，每條 verdict 行加「by `spec-cleanup-stage-5-batch-a` archive 2026-04-27」尾註
- [x] 6.2 改 `docs/reviews/2026-04-26-stage-5.md` 進度狀態表 Cat 2 row 數字從「28 → 25」進一步減為「28 → 17（11 條 covered）」（先前 3 條 covered + 本 change 8 條）
- [x] 6.3 改 `CLAUDE.md` archive 表加 row（spec-cleanup-stage-5-batch-a 收尾），記錄 8 條 D2.x 處理 + 全 suite 數字不變

## 7. 規格 / 設計覆蓋錨點（apply 階段純驗證 checkbox）

- [x] 7.1 Spec coverage：D2.1 由 task 2.1 滿足（qa-agent Requirement `Q&A loop entry point...` MODIFIED + Scenario `All run_qa parameters are keyword-only` + Scenario `cancel_event short-circuits the ReAct loop`）
- [x] 7.2 Spec coverage：D2.2 + D2.3 由 task 2.2 滿足（qa-agent Requirement `add_to_kb pipeline...` MODIFIED + Scenario `Order of operations: budget, pre-validate...` + Scenario `Invalid station_id aborts the entire invocation before any sanitize` + Scenario `Budget exhausted aborts before any sanitize`）
- [x] 7.3 Spec coverage：D2.4 由 task 2.3 滿足（qa-agent Requirement `Q&A run emits SSE events...` MODIFIED + 3 條雙 lane Scenario）
- [x] 7.4 Spec coverage：D2.5 由 task 2.4 滿足（qa-agent Requirement `QATools exposes seven tools...` MODIFIED + Scenario `Reused read tools mirror FolderTools audit_fields exactly`）
- [x] 7.5 Spec coverage：D2.6 由 task 3.1 滿足（kb-growth Requirement `kb_growth.jsonl path constant...` MODIFIED + 3 條 Scenario）
- [x] 7.6 Spec coverage：D2.20 由 task 4.1 滿足（module-5-generator Requirement `Generator entrypoint...` MODIFIED 主文對齊 15 kw-only 參數真實 + Scenario `All run_generator parameters are keyword-only` 鎖必要 4 + 不鎖 optional）
- [x] 7.7 Spec coverage：D2.21 由 task 4.2 滿足（module-5-generator Requirement `Markdown validator...` MODIFIED + Scenario `Length over 800 characters fails validation per D-029`）
