## 1. 前置驗證（apply 動工前）

- [x] 1.1 確認 baseline 測試全綠：`uv run pytest sidecar/tests/ -q`，記錄為 `843 passed / 19 skipped`（baseline 不變式；本 change 改 production code，預期跑完後 +4 ~ +6 新測）
- [x] 1.2 `spectra validate agent-defense-depth --strict` 確認 propose 階段建好的 5 個 delta spec 全綠
- [x] 1.3 確認 production 真值與 design.md 對齊：grep `MessageSource` 在 `sidecar/src/codebus_agent/agent/tools/folder_tools.py:450,630` 確實存在；grep `output=f"ERROR:` 在 `agent/explorer.py:187,197` 確實存在；grep `POST /kb/build` 對應 endpoint 確實沒設 `status_code`

## 2. D2.12 — `POST /kb/build` 統一 202 status code（TDD；對齊 Decision 4: D2.12 status code 改點選 endpoint 端、不動 task registry；落地 Requirement `POST /kb/build async endpoint` MODIFIED）

- [x] 2.1 寫 failing test `sidecar/tests/api/test_kb_build_status_code.py::test_kb_build_returns_202_accepted`：assert `client.post("/kb/build", ...)` response status_code == 202（既有 fixture：bearer + 合法 workspace + scan_result）
- [x] 2.2 寫 failing test `test_all_task_endpoints_return_202_on_success`：parameterized 測 5 個 endpoint（`POST /scan?stream=true` / `POST /kb/build` / `POST /explore` / `POST /generate` / `POST /qa`）成功路徑全部 status_code == 202
- [x] 2.3 改 `sidecar/src/codebus_agent/api/kb.py` `POST /kb/build` decorator 加 `status_code=status.HTTP_202_ACCEPTED`（FastAPI router 一行）
- [x] 2.4 跑 2.1 + 2.2 兩測通過；既有 `/kb/build` 測（4 條：429 / 409 / done event / kb_query）保持綠

## 3. D2.14 — `read_file` / `find_callers` Pass 1 用 `FileSource`（TDD；對齊 Decision 2: D2.14 source type invariant 寫進 `sanitizer` capability cross-cutting Scenario；落地 Requirements `read_file sanitizes output via Pass 1 before returning to Agent` 與 `find_callers returns sanitized call-site FileMatches` MODIFIED）

- [x] 3.1 [P] 寫 failing test `sidecar/tests/agent/tools/test_pass1_source_type.py::test_read_file_pass1_uses_file_source`：對含密鑰的檔跑 `read_file`，assert `<ws>/.codebus/sanitize_audit.jsonl` 第一行 `pass==1` AND `source` 反序列化後型別為 `FileSource`（path + pass_ 欄位）AND `pass_=="explorer_read_file"`
- [x] 3.2 [P] 寫 failing test 同檔 `test_find_callers_pass1_uses_file_source`：對 `find_callers("authorize")` 命中含密鑰的行，assert 對應 audit line `source` 為 `FileSource(path=<call_site>, pass_="find_callers")`
- [x] 3.3 [P] 寫 cross-cutting failing test `sidecar/tests/sanitizer/test_pass_source_invariant.py::test_pass1_lines_carry_file_source_only`：跑一個含 read_file + find_callers 命中的 fixture session，全部 sanitize_audit.jsonl 行掃過，assert `pass==1` 行 `source.startswith("file:")` 100%（無 `message:` 混入）
- [x] 3.4 改 `sidecar/src/codebus_agent/agent/tools/folder_tools.py` 的 `read_file` 內 sanitize call：`MessageSource(message_id="...")` → `FileSource(path=<resolved_relative_path>, pass_="explorer_read_file")`（line 450 區）— 落實 Requirement `read_file sanitizes output via Pass 1 before returning to Agent` 新 Scenario `Pass 1 audit line carries FileSource`
- [x] 3.5 改 `find_callers` 內 sanitize call：`MessageSource(message_id="...")` → `FileSource(path=<call_site_path>, pass_="find_callers")`（line 630 區）
- [x] 3.6 跑 3.1 + 3.2 + 3.3 三測通過；既有 `read_file` / `find_callers` Pass 1 sanitize 測（5 條）保持綠

## 4. D2.15 — `_search_via_grep` snippet 補 Pass 1 sanitize（TDD；對齊 Decision 3: D2.15 grep snippet sanitize 不 cache；落地 Requirement `search consults KB first then falls back to grep` MODIFIED）

- [x] 4.1 [P] 寫 failing test `sidecar/tests/agent/tools/test_grep_fallback_sanitize.py::test_search_via_grep_sanitizes_hit_snippets`：fixture workspace 故意在 `src/secrets.py` line 4 放 `authorize("AKIAIOSFODNN7EXAMPLE")`、`ctx.kb=None` 強制走 grep fallback、`search("authorize")`，assert SearchHit.snippet 含 `<REDACTED:`、不含 `AKIAIOSFODNN7EXAMPLE`、`<ws>/.codebus/sanitize_audit.jsonl` 多一行 `pass==1` `source` 反序列化後是 `FileSource(path="src/secrets.py", pass_="grep_search")`
- [x] 4.2 [P] 寫 failing test 同檔 `test_search_via_grep_fails_loud_when_sanitizer_missing`：`ctx.kb=None` AND `ctx.sanitizer=None` 時 `search("anything")` 必 raise ValueError 提到 missing sanitizer
- [x] 4.3 改 `sidecar/src/codebus_agent/agent/tools/folder_tools.py` 的 `_search_via_grep`（line 324-374 區）：每個命中前先過 `ctx.sanitizer.sanitize(snippet, source=FileSource(path=<hit_path>, pass_="grep_search"))`、用 sanitized snippet 構造 `SearchHit`；KB path 不動（已 sanitized）
- [x] 4.4 改 `_search_via_grep` 進入點加 `ctx.sanitizer is None` fail-loud guard（KB path 不會走到、grep fallback 才會踩到）
- [x] 4.5 跑 4.1 + 4.2 兩測通過；既有 `search` 測（KB-path / grep fallback / empty result / cap 100）保持綠

## 5. D2.19 — Explorer error path Pass 2 sanitize（TDD；對齊 Decision 1: D2.19 error path 走 Pass 2 sanitize（vs 其他三選項）；落地 Requirement `ReAct loop executes think-act-observe-judge-log-update each iteration` MODIFIED — Scenario `Tool errors do not crash the loop` 補 Pass 2 + 新 Scenario `Tool error string sanitized through Pass 2`；以及 `SanitizerAuditLogger appends each replacement to JSONL` 加 Scenario `Explorer tool error path runs Pass 2 sanitize`；本節同時實現 Goals / Non-Goals 範圍內的 production code 改動）

- [x] 5.1 [P] 寫 failing test `sidecar/tests/agent/test_explorer_error_sanitize.py::test_explorer_tool_error_path_sanitized`：mock 一個 raise `ValueError("api_key=sk-AKIAIOSFODNN7EXAMPLE invalid")` 的 tool、`run_explorer` 跑一輪、assert `state.steps[-1].tool_results[0].output` 含 `<REDACTED:`、不含 `sk-AKIAIOSFODNN7EXAMPLE`
- [x] 5.2 [P] 寫 failing test 同檔 `test_explorer_error_writes_pass2_audit_with_message_source`：同 fixture，assert `<ws>/.codebus/sanitize_audit.jsonl` 多一行 `pass==2` AND `source` 反序列化後型別為 `MessageSource(message_id=<contains "explorer_step_">)`
- [x] 5.3 [P] 寫 failing test `test_explorer_error_with_clean_message_no_audit`：mock 一個 raise `ValueError("file not found")`（無密鑰）的 tool、assert `ToolResult.output` 含原訊息文字 AND audit log 無新 `pass==2` 行（sanitize hit 為 0 不寫 audit）
- [x] 5.4 改 `sidecar/src/codebus_agent/agent/explorer.py` 兩處 error path（line 187 + line 197 區）：`output=f"ERROR: {msg}"` 與 `output=f"ERROR: {exc}"` 改為先過 `ctx.sanitizer.sanitize(error_text, source=MessageSource(message_id=f"explorer_step_{state.step_count}_tool_error"))`、用 `result.text` 餵 `ToolResult.output`；audit 由 SanitizerAuditLogger 自動寫 — 落實 Requirement `ReAct loop executes think-act-observe-judge-log-update each iteration` 內 Scenario `Tool errors do not crash the loop` 補 Pass 2 與新 Scenario `Tool error string sanitized through Pass 2`
- [x] 5.5 跑 5.1 + 5.2 + 5.3 三測通過；既有 explorer error containment 測（4 條）保持綠

## 6. 完整驗證 + commit gate

- [x] 6.1 `spectra analyze agent-defense-depth --json` 全綠（Coverage / Consistency / Ambiguity / Gaps 四維度均無 Critical / Warning）
- [x] 6.2 `spectra validate agent-defense-depth --strict` 全綠
- [x] 6.3 `uv run pytest sidecar/tests/ -q` 確認 baseline 843 → 預期 ~849（+6 新測：1 D2.12 status + 1 D2.12 parameterized + 2 D2.14 file-source + 1 D2.14 invariant cross-cutting + 2 D2.15 grep + 3 D2.19）；零 regression
- [x] 6.4 `pre-commit run --all-files` 全綠
- [x] 6.5 手動 grep `MessageSource` 在 `sidecar/src/codebus_agent/agent/tools/folder_tools.py` 確認已不存在（搬到 `find_callers` / `read_file` 既有 callsite 全改為 `FileSource`）
- [x] 6.6 手動 grep `output=f"ERROR:` 在 `agent/explorer.py` 確認已不存在直接賦值（兩處都改為 `ctx.sanitizer.sanitize(...)` 包裝）

## 7. Documentation 連動更新

- [x] 7.1 改 `docs/reviews/2026-04-26-stage-5.md` Cat 2 段：4 條（D2.12 / D2.14 / D2.15 / D2.19）checkbox 改 `[x]`，每條 verdict 行加「by `agent-defense-depth` archive 2026-MM-DD」尾註
- [x] 7.2 改 `docs/reviews/2026-04-26-stage-5.md` 進度狀態表 Cat 2 row 數字從「28 → 17（11 條 covered）」進一步減為「28 → 13（15 條 covered）」（先前 11 條 + 本 change 4 條）
- [x] 7.3 改 `docs/sidecar-api.md` `POST /kb/build` 段：response status code 從 200 改 202、增補「對齊其他 task endpoint」說明
- [x] 7.4 改 `CLAUDE.md` archive 表加 row（agent-defense-depth 收尾），記錄 4 條 D2.x 處理 + production code 變動 3 個檔 + 新測 +6
- [x] 7.5 確認 `docs/decisions.md` 不需新增 ADR — 4 條都是 production drift 修正、屬實作細節非架構決策

## 8. 規格 / 設計覆蓋錨點（apply 階段純驗證 checkbox）

- [x] 8.1 Spec coverage：D2.12 由 task 2.x 滿足（knowledge-base Requirement `POST /kb/build async endpoint` MODIFIED + Scenario `Successful request returns 202 with task_id immediately` + Scenario `Status code aligned with sibling task endpoints`）
- [x] 8.2 Spec coverage：D2.14 由 task 3.x 滿足（explorer-tools Requirements `read_file sanitizes output via Pass 1...` MODIFIED + `find_callers returns sanitized call-site FileMatches` MODIFIED 各加 Scenario `Pass 1 audit line carries FileSource`；sanitizer Requirement `SanitizerAuditLogger appends each replacement to JSONL` MODIFIED + 新 Scenario `pass_num to source-type invariant`）
- [x] 8.3 Spec coverage：D2.15 由 task 4.x 滿足（explorer-tools Requirement `search consults KB first then falls back to grep` MODIFIED 主文補 grep fallback Pass 1 sanitize + 2 條 Scenario `Grep fallback hit snippet sanitized through Pass 1` / `Grep fallback fails loud when sanitizer missing`）
- [x] 8.4 Spec coverage：D2.19 由 task 5.x 滿足（agent-core Requirement `ReAct loop executes...` MODIFIED Scenario `Tool errors do not crash the loop` 補 Pass 2 sanitize 約束 + 新 Scenario `Tool error string sanitized through Pass 2`；sanitizer Requirement `SanitizerAuditLogger...` MODIFIED + 新 Scenario `Explorer tool error path runs Pass 2 sanitize`）
- [x] 8.5 Design coverage：Decision 1（D2.19 Pass 2 vs alternatives）/ Decision 2（D2.14 cross-cutting Scenario placement）/ Decision 3（D2.15 不 cache snippet sanitize）/ Decision 4（D2.12 改點 endpoint 端）四條 Decision 對應 task 5.4 / 8.2 / 4.3 / 2.3 落地
