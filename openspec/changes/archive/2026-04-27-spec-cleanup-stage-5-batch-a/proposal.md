## Summary

把 `docs/reviews/2026-04-26-stage-5.md` Cat 2 的 8 條「spec 跟 production code 不一致」（D2.1-D2.6 / D2.20-D2.21）一次性收回 spec 端，純改 spec wording 對齊既有實作，零 production code / test 變動。

## Motivation

Stage 5 review 點出 28 條 Cat 2「Spec wrong」drift；2026-04-27 audit pass 後 3 條已隨先前 archive 涵蓋（D2.9 / D2.22 / D2.27），剩 25 條按性質拆 3 個 change（詳 review Cat 2 段首）。本 change 收的 8 條都是「純改 spec wording / 補 Scenario」性質、聚焦 qa-agent / kb-growth / module-5-generator 三個 capability，risk profile 最乾淨——零 code 動，零 test 動，propose / apply / archive 流程跑得最快。

Phase 6 前端動工會吃 qa-agent + module-5-generator 兩個 spec 對齊 code 的真實 signature / order / wording 才能寫對 R-01 panel；現在不對齊，前端會踩著錯版 spec 對接，回頭改更貴。

關聯 ADR：D-016（Q&A add_to_kb 流程）/ D-029（Module 5 多檔輸出）；對齊 review Cat 2 段首拆分建議。

## Proposed Solution

### A. qa-agent capability（5 條 drift）

- **D2.1**：Requirement `Q&A loop entry point with two-stage RAG-first flow` L11 真實 signature 改為 `run_qa(*, question, state, kb, tools, provider, logger=None, emitter=None, cancel_event=None) -> QAAnswer`（全 keyword-only、無 sanitizer / sanitizer_audit / kb_growth_logger / provider_factory / workspace_root，多 logger / cancel_event）
- **D2.2**：Requirement `add_to_kb pipeline runs sanitize, validate, upsert, growth-log in order` 改寫真實順序「budget check → pre-validate-all-chunks → loop(sanitize → upsert → growth-log)」並重命名 Requirement 為 `add_to_kb pipeline runs budget, pre-validate, then per-chunk sanitize, upsert, growth-log`；Scenario `Order of operations` 同步重寫
- **D2.3**：Scenario `Invalid station_id aborts before upsert` 主文 + Scenario 兩處改寫為「fail-fast pre-validate；任何 chunk 違規 → 整個 invocation 抛 ValueError，0 個 chunk commit」（移除 transactional / committed 措辭）
- **D2.4**：Requirement `Q&A run emits SSE events on the task channel` L486 把「`default_module="qa_agent"` MUST appear on every record」改為「KB embed lane MUST carry `module="kb_query"`，chat lane MUST carry `module="qa_agent"`」；補 Scenario 鎖死雙 lane 真值
- **D2.5**：Requirement `QATools exposes seven tools with audit_fields declared` 補 Scenario `Reused read tools mirror FolderTools audit_fields exactly` 鎖 `search` / `trace_import` / `find_callers` 為 `[]`、`list_dir` 為 `["path"]`、`read_file` 為 `["path","line_range"]` 五個 reused tool 各自的 production 真值（mirror `folder_tools._AUDIT_FIELDS`）

### B. kb-growth capability（1 條 drift）

- **D2.6**：Requirement `kb_growth.jsonl path constant lives alongside other audit filenames` Scenario `No literal "kb_growth.jsonl" outside the leaf module` 主文 + Scenario 改寫為「canonical 在 `codebus_agent/_audit_paths.py`（package root）；`codebus_agent/api/_audit_paths.py` 是 backward-compat re-export shim；兩處都允許 grep 命中」

### C. module-5-generator capability（2 條 drift）

- **D2.20**：Requirement `Generator entrypoint orchestrates per-station markdown pipeline` L11 改寫 signature 主文，對齊 production 真實 15 個 keyword-only 參數（4 required: `state` / `workspace_root` / `task_id` / `llm_chat_provider`；11 optional: `kb` / `options` / `sanitizer` / `sanitizer_audit` / `rules_version` / `log` / `repo_name` / `workspace_type` / `duration_minutes_per_station` / `title` / `emitter`）。釐清「user task description 走 `state.task` 而不是頂層 `task` 參數」（base spec 與 proposal 原始版本誤把 docstring 散文 `plus a task: str` 當頂層參數）。Scenario `All run_generator parameters are keyword-only` 改為「kind == KEYWORD_ONLY + 必要 4 參數 set 鎖死、optional 參數允許演進不鎖 set」
- **D2.21**：Requirement `Markdown validator enforces D-029 component rules` Scenario `Validator rejects oversized prose block` 補 Scenario 註記引用 D-029 800-char 上限並對齊既有 production validator 的 800-char 真值（既有 1500 chars 超限觸發邏輯保留，補 D-029 cross-reference 段）

## Non-Goals

- **不改任何 production code / test**：本 change 是「spec 端對齊 code」，零 sidecar / 前端 / 文件 production 行為改動；既有測試不需重跑（baseline 843 passed / 19 skipped 不變）
- **不收 D2.7-D2.19 / D2.23-D2.28**：那 17 條走 `spec-cleanup-stage-5-batch-b`（純改 spec + 補 Scenario，13 條）+ `agent-defense-depth`（改 code + 補 test + 補 spec，4 條 D2.12 / D2.14 / D2.15 / D2.19），由本 change archive 後另開 propose
- **不收 D2.9 / D2.22 / D2.27**：3 條 covered（`review-2-critical-fix` CR-1 / `audit-path-unification-stage-2` / `module-8-qa-p0` 已涵蓋），詳 review Cat 2 段首
- **不重命名 module / capability**：D2.2 雖建議重命名 Requirement，但本 change 只動 capability spec 內 Requirement / Scenario wording，不改 capability 名（仍叫 `qa-agent` / `kb-growth` / `module-5-generator`）

## Alternatives Considered

- **方案 A（reject）**：把 25 條全塞一個 `spec-cleanup-stage-5-batch-1` change。**Reject 理由**：25 條跨 9 個 capability 過於發散，propose / apply / review 都失焦；且其中 4 條（D2.12 / D2.14 / D2.15 / D2.19）需改 code + 補 test，risk profile 與「改 spec wording」根本不同
- **方案 B（reject）**：拆兩個 batch（A=NEW capability 連動、B=既有 capability + cross-cutting）但保持 4 條 code-touching 條目混在 batch B。**Reject 理由**：審查 batch B 時要同時讀 spec wording 改動 + production code 改動 + 新 test，認知負擔翻倍；且 propose 階段要不要寫 design.md 的判準也分歧
- **方案 C（adopted）**：拆 3 個 change：本 batch-a（8 條 spec wording）+ batch-b（13 條 spec + Scenario）+ agent-defense-depth（4 條 code+test+spec）。每個 change scope 都收得住、可並行 propose / apply

## Impact

- Affected specs（3 個 MODIFIED capability）：
  - openspec/specs/qa-agent/spec.md
  - openspec/specs/kb-growth/spec.md
  - openspec/specs/module-5-generator/spec.md
- Affected code: 無（純 spec 端 wording 變動）
