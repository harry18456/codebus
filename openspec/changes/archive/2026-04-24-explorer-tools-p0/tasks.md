## 1. Scaffolding

- [x] 1.1 建立 `sidecar/src/codebus_agent/agent/tools/` 套件目錄與 `__init__.py`；建 `schemas.py` / `folder_tools.py` 空 stub（各含 `__all__` + `NotImplementedError` 讓 Section 3+ 的 RED 有訊號）
- [x] 1.2 建 test 目錄 `sidecar/tests/agent/tools/__init__.py`；建 `tests/agent/tools/conftest.py` 提供 `temp_workspace`（放 seed files + sanitize-sensitive fixtures）/ `mock_kb`（in-memory KB 回固定 SearchHit）/ `sanitizer_for_tools`（SanitizerEngine + SanitizerAuditLogger 寫 temp path）/ `explorer_state_with_stations` 等 fixture

## 2. RED — `ToolContext carries workspace type discriminator`(modified：kb / usage_tracker)

對應 spec `tool-sandbox / ToolContext carries workspace type discriminator`。

- [x] 2.1 [P] `tests/sandbox/test_tool_context_optional_deps.py` 加 `test_kb_and_usage_tracker_default_to_none`(落實 spec Requirement `ToolContext carries workspace type discriminator` 的 modified 部分：kb / usage_tracker 兩個 optional 欄位)—— 既有 constructor 呼叫（含 M1 紅隊 fixture）不傳新欄位時，`ctx.kb is None` 與 `ctx.usage_tracker is None`
- [x] 2.2 [P] `test_tool_context_optional_deps.py` 加 `test_kb_and_usage_tracker_accept_typed_instances` —— 顯式傳入 `KnowledgeBase` / `UsageTracker` instance，`ctx.kb` / `ctx.usage_tracker` 曝露該實例，且 `ConfigDict(frozen=True)` 仍守住（試改拋 `ValidationError`）

## 3. GREEN — 擴 `codebus_agent.sandbox.ToolContext`

- [x] 3.1 `sandbox.py` 在 ToolContext 加 `kb: KnowledgeBase | None = None` 與 `usage_tracker: UsageTracker | None = None`（`arbitrary_types_allowed=True` 已開）
- [x] 3.2 type hint 走 `from __future__ import annotations` + `TYPE_CHECKING` import 避免 `KnowledgeBase` / `UsageTracker` 循環引用
- [x] 3.3 執行 `uv run pytest sidecar/tests/sandbox/` 確認既有紅隊 + 新 optional deps 測試全綠

## 4. RED — tool 層 schema（`SearchHit` / `DirEntry`）

- [x] 4.1 [P] `tests/agent/tools/test_schemas.py` 加 `test_search_hit_round_trips`(落實 spec Requirement `Folder-mode Explorer exposes four P0 tools`)—— `SearchHit(path, snippet, score)` 經 `model_dump_json` / `model_validate_json` round-trip 不失資料；`score: Field(ge=0, le=1)` 擋超界
- [x] 4.2 [P] `test_schemas.py` 加 `test_dir_entry_kind_enum` —— `DirEntry.kind` 只接 `Literal["file", "dir"]`；`size` 為 non-negative int

## 5. GREEN — `agent/tools/schemas.py`

- [x] 5.1 `schemas.py` 定義 `SearchHit(path: str, snippet: str, score: float = Field(ge=0, le=1))`；re-export 既有 `agent.protocols.Content`（不自建）
- [x] 5.2 `schemas.py` 定義 `DirEntry(name: str, kind: Literal["file", "dir"], size: int = Field(ge=0))`
- [x] 5.3 執行 `uv run pytest sidecar/tests/agent/tools/test_schemas.py` 確認 4.1 ~ 4.2 全綠

## 6. RED — `Folder-mode Explorer exposes four P0 tools`(structural)

- [x] 6.1 [P] `tests/agent/tools/test_folder_tools_structural.py` 加 `test_folder_tools_satisfies_explorer_tools_protocol`(落實 spec Requirement `Folder-mode Explorer exposes four P0 tools`)—— 實例化 `FolderTools` 後 `isinstance(tools, ExplorerTools)` 必 True
- [x] 6.2 [P] `test_folder_tools_structural.py` 加 `test_tool_dispatch_by_explorer_action_name` —— 用 `_execute_one(ToolCall(name="search"|"list_dir"|"read_file"|"mark_station", ...), tools)` 走 `getattr` dispatch 必須打到對應 method，不走 `primary_search` / `fetch` / `follow_reference`
- [x] 6.3 [P] `test_folder_tools_structural.py` 加 `test_unknown_tool_name_yields_tool_result_error` —— `ToolCall(name="trace_import", ...)` 經 `_execute_one` 回 `ToolResult.error` 含 `trace_import` 字樣，run_explorer 不 raise

## 7. GREEN — `FolderTools` 骨架

- [x] 7.1 `folder_tools.py` 實作 `FolderTools(ctx: ToolContext, state: ExplorerState)`；constructor 把 `ctx` 與 `state` 存起來（不 freeze，因為 state 要 mutate）
- [x] 7.2 `folder_tools.py` 實作 `primary_search` / `fetch` / `follow_reference` 三個 Protocol coroutine 先 delegate 回 `search` / `read_file` / `mark_station`（或 raise `NotImplementedError` 給 Topic-mode tool 用，P0 fold 即可）
- [x] 7.3 `folder_tools.py` 加 `search` / `list_dir` / `read_file` / `mark_station` method signature + `raise NotImplementedError` stub（讓 Section 8+ RED 有訊號）
- [x] 7.4 執行 `uv run pytest sidecar/tests/agent/tools/test_folder_tools_structural.py` 確認 6.1 ~ 6.3 全綠

## 8. RED — `list_dir and read_file enforce ensure_in_workspace`(list_dir 部分)

對應 spec `explorer-tools / list_dir and read_file enforce ensure_in_workspace`。

- [x] 8.1 [P] `tests/agent/tools/test_list_dir.py` 加 `test_list_dir_happy_path`(落實 spec Requirement `list_dir and read_file enforce ensure_in_workspace` 的 list_dir 部分)—— mini workspace 放 2 file + 1 subdir，`await tools.list_dir(".")` 回 `list[DirEntry]` 三筆、kind 正確、size 合理
- [x] 8.2 [P] `test_list_dir.py` 加 `test_list_dir_nested_path_accepted` —— `list_dir("subdir")` 回 subdir 下 entry（不遞迴，只列一層）
- [x] 8.3 [P] `test_list_dir.py` 加 `test_list_dir_parent_escape_rejected`(落實 `Parent-directory escape in read_file rejected` 的對稱紅線)—— `list_dir("../..")` 必 raise `PathEscapeError`；`tool_audit.jsonl` 應有 deny 一行(task 18 補稽核寫入)
- [x] 8.4 [P] `test_list_dir.py` 加 `test_list_dir_symlink_escape_rejected` —— `list_dir("link_to_outside")` 必 raise（Windows 環境 symlink 需 admin；可 `@pytest.mark.skipif` 無法建 symlink 時 skip，對齊既有紅隊）

## 9. GREEN — `FolderTools.list_dir`

- [x] 9.1 `folder_tools.py` 實作 `list_dir(path)`：`resolved = ensure_in_workspace(path, ctx)` → `resolved.iterdir()` → 每 entry 轉 `DirEntry(name, kind="file"|"dir", size=stat().st_size or 0)`；排除 `.codebus` 子目錄(audit 自家)
- [x] 9.2 執行 `uv run pytest sidecar/tests/agent/tools/test_list_dir.py` 確認 8.1 ~ 8.4 全綠(symlink 紅隊在無 admin 環境 skip)

## 10. RED — `read_file sanitizes output via Pass 1 before returning to Agent`

對應 spec `explorer-tools / read_file sanitizes output via Pass 1 before returning to Agent`。

- [x] 10.1 [P] `tests/agent/tools/test_read_file.py` 加 `test_pass1_runs_on_every_read_file_call`(落實 spec Requirement `read_file sanitizes output via Pass 1 before returning to Agent`)—— fixture 放含 fake AWS key(`AKIA...` 格式) 的檔；`await tools.read_file("secret.txt")` 回字串必含 `<REDACTED:` 不含原 key；`sanitize_audit.jsonl` 新增一行以上 `pass_num=1`
- [x] 10.2 [P] `test_read_file.py` 加 `test_missing_sanitizer_fails_loud` —— `ctx.sanitizer = None` 時 `read_file(...)` 必 raise `ValueError` 含 `sanitizer` 字樣；檔內容不得出現在 exception message
- [x] 10.3 [P] `test_read_file.py` 加 `test_line_range_slices_before_sanitize` —— 10 行檔、line 5-7 含 email；`read_file(path, line_range=(5, 7))` 回 3 行 sanitize 結果；行 1-4 / 8-10 的 email 不被 sanitize(audit 不含那些 line)
- [x] 10.4 [P] `test_read_file.py` 加 `test_large_file_truncation` —— > 12000 char 檔，`read_file(path)` 回長度 ≤ 12000，中間出現 `[... truncated ...]`；頭尾各自 sanitize 過
- [x] 10.5 [P] `test_read_file.py` 加 `test_read_file_parent_escape_rejected`(落實 `Parent-directory escape in read_file rejected`)—— `read_file("../../etc/passwd")` 必 raise `PathEscapeError`；return value 不含 `root:` 字樣；`tool_audit.jsonl` 有 deny 一行(task 18 補寫入)

## 11. GREEN — `FolderTools.read_file`

- [x] 11.1 `folder_tools.py` 實作 `read_file(path, line_range=None)`：`resolved = ensure_in_workspace(path, ctx)` → 檢 `ctx.sanitizer is None` raise ValueError → `open(resolved, encoding=...)` → slice line_range → truncate >12k → `ctx.sanitizer.sanitize(text, source=MessageSource(message_id=f"read_file:{path}"))` → 把 entries append `sanitize_audit.jsonl`(用 workspace 內既有 `.codebus/sanitize_audit.jsonl` logger)
- [x] 11.2 `folder_tools.py` 內建 `_truncate_if_large(text, limit=12000, marker="[... truncated ...]")` helper；encoding 用 `charset-normalizer` 或 `utf-8` fallback（對齊 Scanner 做法）
- [x] 11.3 執行 `uv run pytest sidecar/tests/agent/tools/test_read_file.py` 確認 10.1 ~ 10.5 全綠

## 12. RED — `search consults KB first then falls back to grep`

對應 spec `explorer-tools / search consults KB first then falls back to grep`。

- [x] 12.1 [P] `tests/agent/tools/test_search.py` 加 `test_kb_path_used_when_kb_is_configured`(落實 spec Requirement `search consults KB first then falls back to grep`)—— `ctx.kb` 綁 `MockKB.query(keyword)` 回固定 matches；`await tools.search("entry")` 必呼 `mock_kb.query` 恰一次（spy counter）；回傳 `list[SearchHit]` 的 `path` 是相對 `ctx.workspace_root`
- [x] 12.2 [P] `test_search.py` 加 `test_grep_fallback_when_kb_absent` —— `ctx.kb = None`；mini workspace 放 3 個 `.py` 檔含 keyword；`search("target")` 回 list 長度 ≤ 100、每 hit 的 path 副檔名落在 allowed set
- [x] 12.3 [P] `test_search.py` 加 `test_empty_result_when_no_match_found` —— `search("zzzzz_nonexistent")` 回 `[]`、不 raise
- [x] 12.4 [P] `test_search.py` 加 `test_grep_fallback_skips_binary_and_oversize` —— workspace 放 PNG binary + 5MB 文字檔；都不出現在結果內（延用 Scanner text-file filter）

## 13. GREEN — `FolderTools.search`

- [x] 13.1 `folder_tools.py` 實作 `search(keyword)`：`if self._ctx.kb is not None: return _search_via_kb(keyword)` else `return _search_via_grep(keyword)`
- [x] 13.2 `_search_via_kb(keyword)`：`await ctx.kb.query(...)` → map `KBMatch` to `SearchHit`；path 用 `Path(abs).relative_to(ctx.workspace_root)`；score clamp `[0, 1]`；snippet ≤ 400 char（超過截斷加 `...`）
- [x] 13.3 `_search_via_grep(keyword)`：走 `Path.rglob("*")` + 延用 Scanner 的 text-file 判斷（`scanner.is_text_file` 或 `.ALLOWED_EXTS` 常數），命中 keyword 收 `SearchHit`；score 用 `occurrences / file_size` 正規化後 clamp；上限 100
- [x] 13.4 執行 `uv run pytest sidecar/tests/agent/tools/test_search.py` 確認 12.1 ~ 12.4 全綠

## 14. RED — `mark_station mutates state without calling LLM`

對應 spec `explorer-tools / mark_station mutates state without calling LLM`。

- [x] 14.1 [P] `tests/agent/tools/test_mark_station.py` 加 `test_mark_station_appends_to_state_without_llm`(落實 spec Requirement `mark_station mutates state without calling LLM`)—— spy provider 所有 method raise（以證 LLM 未被呼）；`await tools.mark_station("src/app.py", "entry", "main handler")`；`state.stations` 長度 +1、相應 field 填對、relevance == 0.8
- [x] 14.2 [P] `test_mark_station.py` 加 `test_mark_station_is_idempotent_for_identical_inputs` —— 同一組 (path, role, why) 連呼兩次；`state.stations` 長度 +1（不是 +2）；不 raise
- [x] 14.3 [P] `test_mark_station.py` 加 `test_mark_station_out_of_workspace_path_rejected`(落實 `mark_station with out-of-workspace path rejected`)—— 傳絕對外部路徑必 raise `PathEscapeError`；`state.stations` 不動

## 15. GREEN — `FolderTools.mark_station`

- [x] 15.1 `folder_tools.py` 實作 `mark_station(path, role, why)`：`ensure_in_workspace(path, ctx)` → 檢 `state.stations` 中同 `(path, role, why)` 是否已存在 → append `Station(path=path, role=role, relevance=0.8, why=why, depends_on=[])`；return `None`
- [x] 15.2 執行 `uv run pytest sidecar/tests/agent/tools/test_mark_station.py` 確認 14.1 ~ 14.3 全綠

## 16. RED — `ExplorerTools, Judge, and CoverageChecker are structural Protocols`(modified：optional `tool_specs`)

對應 spec `agent-core / ExplorerTools, Judge, and CoverageChecker are structural Protocols`。

- [x] 16.1 [P] `tests/agent/tools/test_tool_specs.py` 加 `test_folder_tools_advertises_tool_surface_via_tool_specs`(落實 spec Requirement `ExplorerTools, Judge, and CoverageChecker are structural Protocols` 的 modified 部分：optional tool_specs method)—— `tools.tool_specs()` 回 list 恰含 4 entry（`search` / `list_dir` / `read_file` / `mark_station`）；每 entry 有 `name` / `description` / `parameters` 三 key
- [x] 16.2 [P] `tests/agent/test_protocols.py` 既有檔加 `test_tool_specs_method_is_optional_on_explorer_tools` —— 原 `_MockTools` 不實作 `tool_specs`；`isinstance(mock, ExplorerTools)` 仍 True
- [x] 16.3 [P] `tests/agent/test_explorer_loop.py` 既有檔加 `test_run_explorer_falls_back_to_empty_tool_specs_when_absent` —— 用不帶 `tool_specs` 的 `_DummyTools`，`run_explorer(...)` 不 raise `AttributeError`；`_think` 收到的 `tool_specs` 是 `[]`

## 17. GREEN — Protocol 補 optional method + `tool_specs()` 實作

- [x] 17.1 `agent/protocols.py` `ExplorerTools` Protocol 加 `def tool_specs(self) -> list[dict]: ...` 作為 optional method（不走 `async`；純 return metadata）；加 docstring 說明「optional；runtime_checkable 不擋未實作」
- [x] 17.2 `agent/explorer.py` `run_explorer` 內取 `tool_specs`：`specs = tools.tool_specs() if hasattr(tools, "tool_specs") else []`；移除目前 `tool_specs: list[dict] | None = None` kwarg（或保留為 override）
- [x] 17.3 `agent/tools/folder_tools.py` 實作 `tool_specs()` 回 4 個 dict；description / parameters 對齊 `docs/agent-explorer-spec.md §三` 表格
- [x] 17.4 執行 `uv run pytest sidecar/tests/agent/tools/test_tool_specs.py sidecar/tests/agent/test_protocols.py sidecar/tests/agent/test_explorer_loop.py` 確認全綠

## 18. RED — tool dispatch 寫 `tool_audit.jsonl`

- [x] 18.1 [P] `tests/agent/tools/test_folder_tools_audit.py` 加 `test_every_tool_invocation_writes_one_line` —— 連呼 `list_dir` / `read_file` / `mark_station`；`tool_audit.jsonl` 必恰 3 行、每行 `schema_version=1`、`allowed=true`、`resolved_path` 存在
- [x] 18.2 [P] `test_folder_tools_audit.py` 加 `test_denied_invocation_writes_denial_line` —— `list_dir("../..")` 觸發；audit 新增一行 `allowed=false, denial_reason="path_escape"`、`tool_name="list_dir"`；tool body 未執行（沒產生其他檔案）
- [x] 18.3 [P] `test_folder_tools_audit.py` 加 `test_args_summary_uses_whitelist` —— `search("secret_KEYWORD_7777")` 的 audit line 的 `args_summary` 不得含原 keyword（因 `search.audit_fields = []` 或只含 `keyword_hash`）；`read_file("src/app.py")` 的 audit line 的 `args_summary` 含 `path="src/app.py"`（`audit_fields=["path"]`）

## 19. GREEN — wire `ToolSandbox.invoke` 走 FolderTools

- [x] 19.1 `folder_tools.py` 每個 tool method 前宣告 `audit_fields: ClassVar[list[str]]`（`search = []` / `list_dir = ["path"]` / `read_file = ["path", "line_range"]` / `mark_station = ["path", "role"]`——`why` 有語意可能洩漏，故不加 whitelist）
- [x] 19.2 `folder_tools.py` 把每 method 包在 `ctx.audit_log.append(...)` 或透過既有 `sandbox.ToolSandbox.invoke(tool_fn, args, ctx)` 統一走 audit；優先走既有 `ToolSandbox` 不另建 writer
- [x] 19.3 執行 `uv run pytest sidecar/tests/agent/tools/test_folder_tools_audit.py` 確認 18.1 ~ 18.3 全綠

## 20. RED — Explorer loop with real tools(integration)

- [x] 20.1 `tests/agent/test_explorer_loop_with_real_tools.py` 加 `test_mini_workspace_search_read_mark_closes_loop` —— mini workspace 3 個 py 檔；MockProvider script 餵 3 個 `ExplorerAction`：(1) search "entry" → (2) read_file hit(0).path → (3) mark_station 同 path；跑完 `state.stations` 至少 1 筆、`reasoning_log.jsonl` 3 行、`tool_audit.jsonl` 3 行 allow
- [x] 20.2 `test_explorer_loop_with_real_tools.py` 加 `test_sanitizer_end_to_end` —— workspace 放含 fake secret 的 py；Explorer 跑 `read_file` 後 `state.messages` 內 role="tool" content 必 placeholder；`llm_calls.jsonl` 下一輪 request 內同樣只含 placeholder

## 21. GREEN — 補最後 wiring 讓 Section 20 綠

- [x] 21.1 任何 integration 缺角（FolderTools 接 ExplorerState 的 reference、`run_explorer` 從 tools 取 specs 的 fallback）在此修
- [x] 21.2 執行 `uv run pytest sidecar/tests/agent/test_explorer_loop_with_real_tools.py` 確認 20.1 ~ 20.2 全綠

## 22. 文件 + Repo metadata 更新

- [x] 22.1 `CLAUDE.md` §Repo 現況 archive 時間軸加入本 change；「目前 in-progress change」/「下一步」指向步驟 18 `explorer-judge-golden`(Judge prompt 調 + golden sample 首跑)；若決定步驟 19 trace_import 優先則改指向 `explorer-tools-p1`
- [x] 22.2 `docs/agent-explorer-spec.md §三` 工具表把 P0 四個 tool 標為 implemented；`docs/tool-sandbox.md §五` ToolContext 表加 `kb` / `usage_tracker` 兩 optional 欄位；`docs/agent-core.md §六` 若 ToolContext 規格有漂移同步

## 23. 驗證與 commit gate

- [x] 23.1 執行 `uv run pytest sidecar/tests/agent/tools/` 確認 agent tools 層全綠
- [x] 23.2 執行 `uv run pytest sidecar/tests/` 完整 suite 無 regression
- [x] 23.3 執行 `pre-commit run --all-files` 全綠
