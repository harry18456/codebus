## Why

Stage 5 Review #2 Cat 2 共 28 條 spec drift，3 條已 covered（review-2-critical-fix / audit-path-unification-stage-2 / module-8-qa-p0），8 條由 `spec-cleanup-stage-5-batch-a`（archive 2026-04-27）收尾，剩 17 條：13 條純 spec wording 走 `spec-cleanup-stage-5-batch-b`，4 條改 code + test + spec 走本 change。

這 4 條的共同 risk profile：production code 與 spec 預期不符、且**繞過了 sanitizer 防禦深度**或**違反 endpoint status code 一致性紅線**。Phase 6 前端對接時若沿用既有錯版會踩到三條：(a) `POST /kb/build` 走 200 而其他 task endpoint 走 202，前端 SSE 輪詢邏輯需特殊化；(b) `find_callers` / `read_file` 把 `pass_num=1` 寫進 `MessageSource` 行，違反「Pass 1=file-source / Pass 2=message-source」設計，Trust Layer R-01 的 Pass 1/2 過濾按 source type 分組會誤判；(c) `_search_via_grep` snippet 不過 Pass 1 sanitize（KB path 已 sanitized 因 Scanner Pass 1，grep path 漏網），grep 命中含密鑰時 Trust Layer 顯示原文。

關聯 ADR：D-015（Sanitizer 三段式設計）/ D-017（ToolSandbox 紅線）/ D-022（llm_calls.jsonl post-sanitize 紀錄）；不變式 #3「LLM 看到的一定是 Sanitize 過的」要在 production code 端落實，不能只寫在 spec。

## What Changes

### D2.12 — `POST /kb/build` 統一 202 status code

- **改 code**: `sidecar/src/codebus_agent/api/kb.py` 的 `POST /kb/build` 加 `status_code=status.HTTP_202_ACCEPTED`（既有 `/explore` `/generate` `/qa` 都已 202）
- **改 spec**: `knowledge-base` capability `Background KB build endpoint emits SSE events` Scenario `Successful spawn returns task_id` 從 200 改 202
- **新測**: `test_kb_build_returns_202_accepted` 鎖死 status code（response status assertion，現有測試只 assert `task_id` 存在沒鎖 status）

### D2.14 — `find_callers` / `read_file` Pass 1 用 `FileSource` 不是 `MessageSource`

- **改 code**: `sidecar/src/codebus_agent/agent/tools/folder_tools.py` 的 `read_file`（line 450 區）+ `find_callers`（line 630 區）兩處 `MessageSource(message_id="...")` + `pass_num=1` 改為 `FileSource(path=<path>, pass_="explorer_read_file")` / `FileSource(path=<path>, pass_="find_callers")`
- **改 spec**: `explorer-tools` capability 對應 Requirement 主文加「Pass 1 audit lines from file-reading tools MUST carry `FileSource`, not `MessageSource`」+ 新 Scenario `Pass 1 audit source type matches pass_num invariant` 鎖死 `pass_num=1` 對應 `source.startswith("file:")`
- **改 spec**: `sanitizer` capability `SanitizerAuditLogger appends each replacement to JSONL` 加 Scenario `pass_num to source-type invariant`（cross-cutting 不變式）
- **新測**: `test_read_file_pass1_uses_file_source` + `test_find_callers_pass1_uses_file_source` 兩測 grep `<ws>/.codebus/sanitize_audit.jsonl` 確認 Pass 1 行 `source` 欄以 `file:` 開頭

### D2.15 — `_search_via_grep` snippet 補 Pass 1 sanitize

- **改 code**: `sidecar/src/codebus_agent/agent/tools/folder_tools.py` 的 `_search_via_grep`（line 324-374）對每個命中的 snippet 跑 `ctx.sanitizer.sanitize(snippet, source=FileSource(path=hit_path, pass_="grep_search"))`，命中 sanitize 寫 `sanitize_audit.jsonl` `pass_num=1`；保持 KB path 的 sanitized snippet 行為一致
- **改 spec**: `explorer-tools` capability `FolderTools search tool` 主文加「`_search_via_grep` fallback path MUST sanitize each hit's snippet through Pass 1 before returning」+ 新 Scenario `Grep fallback hit snippet sanitized through Pass 1`
- **新測**: `test_search_via_grep_sanitizes_hit_snippets`（fixture：故意在某檔放假密鑰，assert grep 命中該檔時 SearchHit.snippet 含 `<REDACTED:` placeholder + `sanitize_audit.jsonl` 多一行 `pass_num=1`）

### D2.19 — Explorer tool error path 過 Pass 2 sanitize

- **改 code**: `sidecar/src/codebus_agent/agent/explorer.py` 兩處 error path（line 187 `output=f"ERROR: {msg}"` + line 197 `output=f"ERROR: {exc}"`）改為跑 Pass 2 sanitize：error string 若含使用者 input（例如 `read_file(path="/etc/passwd")` 拋的 path 字串）必須 redact 後才放進 `ToolResult.output`，避免後續循環送回 LLM 時繞過 Pass 2
- **改 spec**: `agent-core` capability `Tool errors do not crash the loop` 主文加「ToolResult.output for error path MUST be Pass 2 sanitized before return」+ 新 Scenario `Tool error string sanitized through Pass 2`
- **改 spec**: `sanitizer` capability 補 Scenario `Explorer tool error path runs Pass 2 sanitize`（cross-cutting）
- **新測**: `test_explorer_tool_error_path_sanitized`（fixture：mock 一個 raise `ValueError("api_key=sk-xxx")` 的 tool，assert `state.steps[-1].observation` 不含 `sk-xxx` literal、含 `<REDACTED:` placeholder）

## Non-Goals

- **不改任何 spec 純 wording 條目**：D2.7 / D2.8 / D2.10 / D2.11 / D2.13 / D2.16-D2.18 / D2.23-D2.26 / D2.28 共 13 條走 `spec-cleanup-stage-5-batch-b`（後續 change），本 change 只收 4 條 code-touching 條目
- **不重命名 module / endpoint / capability**：D2.12 只動 status code 數字，URL path / function name / spec name 不動
- **不擴 sanitizer rules**：D2.15 / D2.19 只是把既有 `SanitizerEngine` 套用到漏掉的 path，不加新 detection rule（rules version 不 bump）
- **不改 KB path 的 sanitize 行為**：D2.15 KB path（Scanner Pass 1 入 KB 前）已 sanitized，本 change 只補 grep fallback 漏網，不重做 KB 端
- **不引入新的 SSE event type**：error path Pass 2 sanitize 寫進既有 `agent_action_result` event 的 `output` 欄位，不新增 event
- **不擴 Trust Layer R-01 panel UI**：本 change 只動 sidecar，前端配合在 Phase 6 步驟 28 改

## Capabilities

### New Capabilities

(none — 純修 production drift)

### Modified Capabilities

- `knowledge-base`: D2.12 `POST /kb/build async endpoint` Requirement MODIFIED — 200 改 202 + 加 Scenario `Status code aligned with sibling task endpoints`
- `explorer-tools`: D2.14 + D2.15 — `read_file sanitizes output via Pass 1 before returning to Agent` / `find_callers returns sanitized call-site FileMatches` / `search consults KB first then falls back to grep` 三個 Requirement MODIFIED
- `agent-core`: D2.19 `ReAct loop executes think-act-observe-judge-log-update each iteration` Requirement MODIFIED — Scenario `Tool errors do not crash the loop` 加 Pass 2 sanitize 約束 + 新 Scenario `Tool error string sanitized through Pass 2`
- `sanitizer`: D2.14 / D2.19 cross-cutting — `SanitizerAuditLogger appends each replacement to JSONL` Requirement MODIFIED 加 Scenario `pass_num to source-type invariant` 與 `Explorer tool error path runs Pass 2 sanitize`

## Impact

- Affected code:
  - Modified:
    - sidecar/src/codebus_agent/api/kb.py（D2.12 status code 202）
    - sidecar/src/codebus_agent/agent/tools/folder_tools.py（D2.14 read_file / find_callers source type、D2.15 grep snippet sanitize）
    - sidecar/src/codebus_agent/agent/explorer.py（D2.19 error path Pass 2）
  - New:
    - sidecar/tests/api/test_kb_build_status_code.py（D2.12 新測）
    - sidecar/tests/agent/tools/test_pass1_source_type.py（D2.14 新測：read_file + find_callers）
    - sidecar/tests/agent/tools/test_grep_fallback_sanitize.py（D2.15 新測）
    - sidecar/tests/agent/test_explorer_error_sanitize.py（D2.19 新測）
- Affected specs:
  - openspec/specs/knowledge-base/spec.md（D2.12 MODIFIED Requirement + Scenario）
  - openspec/specs/explorer-tools/spec.md（D2.14 + D2.15 MODIFIED Requirements）
  - openspec/specs/agent-core/spec.md（D2.19 MODIFIED Requirement）
  - openspec/specs/sanitizer/spec.md（D2.14 + D2.19 cross-cutting MODIFIED Requirements）
- Affected docs:
  - docs/reviews/2026-04-26-stage-5.md（4 條 D2.x checkbox 改 [x] + verdict 加尾註）
  - CLAUDE.md（archive 表加 row）
  - docs/sidecar-api.md（D2.12 POST /kb/build response 段改 202）
- Test suite delta：baseline 843 passed / 19 skipped → 預期 +4 ~ +6 新測（每條 D2.x 至少 1 測，加 1-2 條 cross-cutting integration）
