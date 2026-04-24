## 1. Scaffolding

- [x] 1.1 建立 test scaffolding：`sidecar/tests/agent/tools/test_trace_import.py` 與 `sidecar/tests/agent/tools/test_find_callers.py` 各一個占位檔（from __future__ import annotations + module docstring），並在 `sidecar/tests/agent/tools/conftest.py` 補 `find_callers` / `trace_import` 共用 fixture（如有需要）
- [x] 1.2 建立 `FileMatch schema — 輕量、只放 Agent 需要的` 所需的測試 fixture：在 `sidecar/tests/agent/tools/test_schemas.py` 加一個 placeholder test 確認 `FileMatch` 匯出路徑，待 3.1 green 填入真測

## 2. RED — `trace_import resolves symbols to definition paths via regex`

對應 spec requirement `trace_import resolves symbols to definition paths via regex`；覆蓋 design 決策「Regex-based definition & call-site 比對 — 不引 AST / tree-sitter」與「掃描實作 — async path iteration + early-exit for trace_import」。

- [x] 2.1 [P] `test_trace_import.py::test_python_def_resolves_to_source_path`（落實 spec requirement `trace_import resolves symbols to definition paths via regex`）—— 建 fixture repo 含 `src/kb/base.py`（`class KnowledgeBase:`），assert `await tools.trace_import("KnowledgeBase") == "src/kb/base.py"`
- [x] 2.2 [P] `test_trace_import.py::test_typescript_export_function_resolves` —— fixture 含 `web/src/providers.ts`（`export function makeProvider(`），assert 回 `"web/src/providers.ts"`
- [x] 2.3 [P] `test_trace_import.py::test_rust_pub_async_fn_resolves` —— fixture 含 `crates/server/src/lib.rs`（`pub async fn handle_request(`），assert 回 `"crates/server/src/lib.rs"`
- [x] 2.4 [P] `test_trace_import.py::test_symbol_not_defined_returns_none` —— 空 workspace + `trace_import("Zzz_NotDefined")`，assert `None` 且不 raise
- [x] 2.5 [P] `test_trace_import.py::test_multiple_definitions_pick_shortest_path_depth` —— fixture 含 `src/util.py` 與 `tests/helpers/util.py` 都 `class Util:`，assert 回 `"src/util.py"`（path_depth 較淺）
- [x] 2.6 [P] `test_trace_import.py::test_symbol_with_regex_metacharacters_safe` —— 呼叫 `trace_import("foo.bar")`，assert 不 raise 且不會把 `foo.bar` 當 wildcard 匹配到 `foo_bar`
- [x] 2.7 [P] `test_trace_import.py::test_tool_audit_line_written_on_allowed_path` —— 成功命中後，assert `tool_audit.jsonl` 新增一行含 `tool="trace_import"` 與 `allowed=true`

## 3. GREEN — 實作 trace_import 與 FileMatch schema

對應 design 決策「Regex-based definition & call-site 比對 — 不引 AST / tree-sitter」、「FileMatch schema — 輕量、只放 Agent 需要的」、「回傳格式 — trace_import 回 single path，find_callers 回 list ≤ 100」、「掃描實作 — async path iteration + early-exit for trace_import」。

- [x] 3.1 `sidecar/src/codebus_agent/agent/tools/schemas.py` 新增 `FileMatch(path: str, line: int, snippet: str)` Pydantic BaseModel；`sidecar/src/codebus_agent/agent/tools/__init__.py` re-export；同步補 `test_schemas.py::test_file_match_shape`
- [x] 3.2 `sidecar/src/codebus_agent/agent/tools/folder_tools.py::FolderTools.trace_import` async 實作（對應 design 決策「回傳格式 — `trace_import` 回 single path，`find_callers` 回 list ≤ 100」）：組 language-neutral pattern set、對允許副檔名 iterate、`re.escape(symbol)` 防注入、第一個命中即 early-exit 並 return deterministic-sorted 候選
- [x] 3.3 在 `trace_import` 的 path 組裝前用 `ensure_in_workspace(candidate_path, ctx)` 把關；被拒路徑略過並透過 `sandbox.append_tool_audit_line` 寫 `allowed=false`
- [x] 3.4 執行 `uv run pytest sidecar/tests/agent/tools/test_trace_import.py` 直到 2.x 全綠

## 4. RED — `find_callers returns sanitized call-site FileMatches`

對應 spec requirement `find_callers returns sanitized call-site FileMatches`；覆蓋 design 決策「回傳格式 — trace_import 回 single path，find_callers 回 list ≤ 100」與「sanitize 策略 — 只有 find_callers 要，trace_import 不要」。

- [x] 4.1 [P] `test_find_callers.py::test_multiple_callsites_return_sanitized_file_matches`（落實 spec requirement `find_callers returns sanitized call-site FileMatches`）—— fixture 含 `src/app.py` line14 與 `src/api/routes.py` line30 兩處呼叫，assert 返回兩個 FileMatch
- [x] 4.2 [P] `test_find_callers.py::test_whole_word_boundary_rejects_substring` —— fixture 只含 `foobar(x)`，呼 `find_callers("foo")` 回 `[]`
- [x] 4.3 [P] `test_find_callers.py::test_definition_site_excluded_from_results` —— fixture `src/bar.py` line5 = `class Bar:`、line20 = `Bar()`，assert 返回只含 line20
- [x] 4.4 [P] `test_find_callers.py::test_per_file_cap_limits_snippet_storm` —— fixture `src/constants.py` 含 50 個 `MAX`，assert 同檔命中 ≤ 5
- [x] 4.5 [P] `test_find_callers.py::test_global_cap_enforces_100_ceiling` —— 合成超過 100 筆命中，assert `len(result) <= 100`
- [x] 4.6 [P] `test_find_callers.py::test_snippet_sanitize_redacts_secrets` —— 命中 `authorize("AKIAIOSFODNN7EXAMPLE")`，assert snippet 無 raw AWS key、含 `<REDACTED:`、`sanitize_audit.jsonl` 新增 `pass_num=1` 行
- [x] 4.7 [P] `test_find_callers.py::test_missing_sanitizer_fails_loud` —— `ctx.sanitizer=None`，assert raise `ValueError`
- [x] 4.8 [P] `test_find_callers.py::test_symbol_with_zero_matches_returns_empty_list` —— `find_callers("ZzzNoSuchName")` 回 `[]` 且不 raise

## 5. GREEN — 實作 find_callers 與 sanitize / ensure_in_workspace 整合

對應 design 決策「sanitize 策略 — 只有 find_callers 要，trace_import 不要」。

- [x] 5.1 `FolderTools.find_callers` async 實作：掃同副檔名集合、`\b<escaped_symbol>\b` regex、per-file ≤ 5、global ≤ 100、排序鍵 `(path_depth, path, line)`
- [x] 5.2 find_callers 呼叫 `ctx.sanitizer.sanitize(...)` Pass 1 把 snippet 過濾 + 截到 200 字（對應 design 決策「sanitize 策略 — 只有 `find_callers` 要，`trace_import` 不要」）；sanitize hit 寫 `sanitize_audit.jsonl`（`pass_num=1`）；`ctx.sanitizer is None` → raise `ValueError`
- [x] 5.3 find_callers 排除 definition site：呼叫 `self.trace_import(symbol)` 取定義 path（非 None 時），再比對每個 hit 是否為定義行並剔除
- [x] 5.4 每次呼叫透過 `sandbox.append_tool_audit_line` 寫一行 `tool_audit.jsonl`（含 symbol 參數 + allowed 結果）
- [x] 5.5 執行 `uv run pytest sidecar/tests/agent/tools/test_find_callers.py` 直到 4.x 全綠

## 6. RED — 紅隊覆蓋與 MODIFIED requirement

對應 spec requirement `Folder-mode Explorer exposes four P0 tools`（MODIFIED：scenario "Unknown tool name" 改 placeholder）；覆蓋 design 決策「紅隊覆蓋 — path escape / symlink 必測」。

- [x] 6.1 [P] `test_trace_import.py::test_symlink_escape_discarded` —— 建 symlink 指向 workspace 外的 `class ExternalSymbol`，assert trace_import 回 `None` 且 `tool_audit.jsonl` 有 `allowed=false` 行（Windows 跳過 skip marker，對齊 P0 紅隊紀律）
- [x] 6.2 [P] `test_folder_tools_structural.py` 加 test 確認 `tools.trace_import` / `tools.find_callers` 都是 callable 且 `_execute_one` dispatch 得到（配合 MODIFIED requirement `Folder-mode Explorer exposes four P0 tools` 裡的 "Tool dispatch by ExplorerAction.tool_calls name" scenario 擴充）
- [x] 6.3 [P] `test_tool_specs.py` 把枚舉斷言從 4 個工具改 6 個（含 `trace_import` / `find_callers` 的 `name` / `description` / `parameters` 合規）
- [x] 6.4 [P] "Unknown tool name" scenario 的 placeholder 實際落在 `test_folder_tools_structural.py::test_unknown_tool_name_yields_tool_result_error`（非 `test_explorer_loop.py`）—— placeholder 已從 `trace_import` 換成 `find_nonexistent`

## 7. GREEN — 補紅隊與 tool_specs + Explorer loop 更新

- [x] 7.1 `FolderTools.tool_specs()` 加入 `trace_import` / `find_callers` 兩筆 spec（name / description / parameters schema）
- [x] 7.2 落實 6.1 紅隊 green：symlink 命中路徑被 `ensure_in_workspace` 拒絕後，`trace_import` 把候選剔除；對應 audit log 有 `allowed=false` 行
- [x] 7.3 執行 `uv run pytest sidecar/tests/agent/tools/test_trace_import.py sidecar/tests/agent/tools/test_find_callers.py sidecar/tests/agent/tools/test_folder_tools_structural.py sidecar/tests/agent/tools/test_tool_specs.py sidecar/tests/agent/test_explorer_loop.py` 全綠

## 8. 文件與 repo metadata 更新

- [x] 8.1 `CLAUDE.md` archive 時間軸加入本 change；「下一步」改指向步驟 20 `coverage-gap-recurse` 或步驟 21 `context 壓縮 + token-aware budget`
- [x] 8.2 `docs/agent-explorer-spec.md §九` 狀態表：`trace_import` / `find_callers` 從 `⏳ P1（步驟 19）` 改 `✅ 步驟 19 landed（本 change）`（§三 狀態欄與 P0 落地細節段亦同步更新）
- [x] 8.3 `docs/tool-sandbox.md` 的工具稽核對照表補兩行（`trace_import` / `find_callers` 的 audit 欄位範例）；若現行文件已以通用 pattern 覆蓋就留 note 說明（§七 補了 P1 symbol-navigation 工具的 audit 細節段）

## 9. 驗證與 commit gate

- [x] 9.1 執行 `uv run pytest sidecar/tests/agent/tools/` 整個 tools 測試目錄全綠（P0 6 檔 + P1 2 檔新加）—— 43 passed, 2 skipped（Windows symlink）
- [x] 9.2 執行 `uv run pytest sidecar/tests/` 完整 suite 無 regression —— 629 passed / 19 skipped（排除 `test_main_run.py` 整組跑；加入 `test_main_run.py` 後 631 passed 1 failed 19 skipped，但該 failure 是 `test_startup_remains_available_when_qdrant_unreachable` 在 full-suite timing 壓力下 3s handshake budget 失守的 pre-existing flakiness，獨立跑穩定 pass 且非本 change 的 logic regression）
- [x] 9.3 執行 `pre-commit run --all-files` 全綠（trailing-ws / eof / check-yaml / check-json / merge-conflict / line-ending 皆 pass）
