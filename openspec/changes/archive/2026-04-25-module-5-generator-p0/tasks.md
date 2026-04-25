## 1. Scaffolding

- [x] 1.1 建立 `sidecar/src/codebus_agent/generator/` 套件骨架（`__init__.py` 公開 `run_generator` / `GeneratorResult`；`types.py` / `runner.py` / `station.py` / `validator.py` / `stable_id.py` / `frontmatter.py` / `moc.py` / `route.py` / `log.py` / `prompts/__init__.py` 全建空 module 含 `from __future__ import annotations` + module docstring 引用對應 spec Requirement）
- [x] 1.2 建立 `sidecar/tests/generator/` 目錄含 `__init__.py` + `conftest.py`（spy provider + workspace fixture + scripted MockProvider helper）
- [x] 1.3 `sidecar/src/codebus_agent/api/_audit_paths.py` 補 `_GENERATOR_LOG_FILENAME = "generator_log.jsonl"` 常數（新增第六個 workspace-level filename 常數）

## 2. Types — Generator Pydantic schema

- [x] 2.1 [P] 在 `generator/types.py` 寫 `StationMarkdown(BaseModel)`：`thought: str`, `body: str`, `notes: str | None`（Instructor `response_model` 用；`body` 是純 markdown，frontmatter 後處理階段加）
- [x] 2.2 [P] 在 `generator/types.py` 寫 `Frontmatter(BaseModel)`：13 欄（11 required + 3 optional）對齊 spec `Frontmatter renderer produces D-029 schema_version 1 YAML` Requirement 列舉
- [x] 2.3 [P] 在 `generator/types.py` 寫 `ValidationResult(BaseModel)`：`issues: list[str]` + `parsed: dict[str, Any]`
- [x] 2.4 [P] 在 `generator/types.py` 寫 `RouteStation(BaseModel)` + `StationSummary(BaseModel)`：對齊 spec `route.json output carries D-029 §八 schema with station_id and file_path` Requirement 欄位
- [x] 2.5 [P] 在 `generator/types.py` 寫 `GeneratorResult(BaseModel)`：`tutorial_path: Path`, `station_paths: list[Path]`, `route_path: Path`, `log_path: Path`, `degraded_count: int`
- [x] 2.6 [P] 寫 `tests/generator/test_types.py`：5 個 round-trip test（每個 BaseModel `model_dump_json` → `model_validate_json` 等價）

## 3. Stable station id generation — RED 後 GREEN

對應 spec Requirement `Stable station id generation produces s{NN}-{slug} with collision handling`（5 scenarios）。

- [x] 3.1 [P] RED `tests/generator/test_stable_id.py::test_ascii_title_produces_clean_slug`：`generate_station_id(2, "Storage Interface Contract", set())` → `"s02-storage-interface-contract"`
- [x] 3.2 [P] RED `test_stable_id.py::test_cjk_only_title_falls_back_to_station`：`generate_station_id(1, "儲存介面契約", set())` → `"s01-station"`
- [x] 3.3 [P] RED `test_stable_id.py::test_slug_truncates_at_dash_boundary_under_40_chars`：產 60-char title 含 dash boundary，slug 在 boundary 截斷
- [x] 3.4 [P] RED `test_stable_id.py::test_collision_appends_dash_two_suffix`：`existing_ids={"s03-storage-interface-contract"}` → `"s03-storage-interface-contract-2"`
- [x] 3.5 [P] RED `test_stable_id.py::test_index_zero_padded_to_two_digits`：index=7 → prefix `"s07-"`
- [x] 3.6 GREEN — Stable station id generation produces s{NN}-{slug} with collision handling：實作 `generator/stable_id.py::generate_station_id(...)`（lowercase → regex sub → collapse dashes → strip → truncate at boundary → fallback "station" → collision -2/-3/...）
- [x] 3.7 跑 `uv run pytest sidecar/tests/generator/test_stable_id.py` 5 測全綠

## 4. Frontmatter renderer — RED 後 GREEN

對應 spec Requirement `Frontmatter renderer produces D-029 schema_version 1 YAML`（3 scenarios）。

- [x] 4.1 [P] RED `tests/generator/test_frontmatter.py::test_required_fields_rendered_in_order`：assert YAML 第一個 key `schema_version`，11 個 required field 在固定順序
- [x] 4.2 [P] RED `test_frontmatter.py::test_optional_empty_lists_are_omitted`：`tags=[]` / `related_stations=[]` / `related_files=[]` 輸出 YAML 不含這三 key
- [x] 4.3 [P] RED `test_frontmatter.py::test_optional_populated_lists_are_rendered`：`tags=["architecture", "interfaces"]` 輸出含 `tags:` key
- [x] 4.4 GREEN — Frontmatter renderer produces D-029 schema_version 1 YAML：實作 `generator/frontmatter.py::render_frontmatter(meta) -> str`（YAML serialize 用 PyYAML safe_dump、固定欄位順序、optional 空值省略、`generated_at` ISO-8601 含 timezone、輸出 wrap `---\n...\n---\n`）
- [x] 4.5 跑測 3 測全綠

## 5. Markdown validator — RED 後 GREEN

對應 spec Requirement `Markdown validator enforces D-029 component rules`（6 scenarios）。

- [x] 5.1 [P] RED `tests/generator/test_validator.py::test_interactive_mode_rejects_markdown_without_checkpoint`：interactive mode + 無 `<Checkpoint>` → issues 含 `"missing_checkpoint"`
- [x] 5.2 [P] RED `test_validator.py::test_interactive_mode_rejects_two_quizzes`：兩個 `<Quiz>` → issues 含 `"too_many_quizzes"`
- [x] 5.3 [P] RED `test_validator.py::test_quiz_with_invalid_correct_attribute_is_rejected`：`<Quiz correct="e">` → issues 含 `"quiz_bad_correct: e"`
- [x] 5.4 [P] RED `test_validator.py::test_length_over_800_chars_fails_validation`：1500-char body → issues 含 `"too_long"`
- [x] 5.5 [P] RED `test_validator.py::test_coderef_pointing_outside_workspace_fails_validation`：`<CodeRef file="../../etc/passwd">` against `workspace_root=/tmp/ws` → issues 含 `"coderef_escape: ../../etc/passwd"`
- [x] 5.6 [P] RED `test_validator.py::test_plain_mode_tolerates_absence_of_components`：plain mode + 無 component → issues 不含 `missing_checkpoint` / `too_many_quizzes` / `quiz_*`
- [x] 5.7 GREEN — Markdown validator enforces D-029 component rules：實作 `generator/validator.py::validate_station_markdown(md, station_idx, mode, workspace_root)`（mode-aware 規則分支、regex parse `<Checkpoint>` / `<Quiz>` / `<CodeRef>`、長度計算用 `len(md_without_frontmatter)`、code block 行數計算）
- [x] 5.8 跑測 6 測全綠

## 6. Prompts — interactive + plain templates

對應 spec Requirement `Plain mode prompt template emits markdown without custom components` 與 `Generator entrypoint orchestrates per-station markdown pipeline` 中 prompt 行為。

- [x] 6.1 [P] Plain mode prompt template emits markdown without custom components — 寫 `generator/prompts/__init__.py` 含 `STATION_PROMPT_VERSION = "2026-04-25-1"` (date-version 對齊 `JUDGE_PROMPT_VERSION` 慣例) + `STATION_SYSTEM_INTERACTIVE: str` + `STATION_SYSTEM_PLAIN: str` + `render_station_prompt(context, mode) -> str`
- [x] 6.2 [P] interactive prompt 內容對齊 spec §三 約束（每站 ≥ 1 `<Checkpoint>`、`<Quiz>` ≤ 1、長度 ≤ 800 字、`###` 分頁 > 300 字插入、code block ≤ 30 行、語言依 `target_persona`）
- [x] 6.3 [P] plain prompt 內容對齊 spec §六（無 component tag、`<Checkpoint>` → task list、`<Quiz>` → 思考題 blockquote）
- [x] 6.4 [P] 寫 `tests/generator/prompts/test_prompts.py`：assert two system 字串都含關鍵約束句、`STATION_PROMPT_VERSION` 符合 `^\d{4}-\d{2}-\d{2}-\d+$` regex、`render_station_prompt` mode 分支正確
- [x] 6.5 跑測全綠

## 7. Per-station LLM call + retry pipeline — RED 後 GREEN

對應 spec Requirement `Generator entrypoint orchestrates per-station markdown pipeline` 與 `Degraded fallback writes per-station stub after retry exhaustion`。覆蓋 design Decision 4。

- [x] 7.1 RED `tests/generator/test_station.py::test_three_retries_with_persistent_issues_produces_degraded_stub`：scripted MockProvider 第 3 次 attempt 後 frontmatter `degraded: true`、第 4 次 attempt 不發生
- [x] 7.2 RED `test_station.py::test_per_station_degradation_does_not_affect_subsequent_stations`：3 站，第 2 站 degraded、1+3 正常
- [x] 7.3 RED `test_station.py::test_disk_write_failure_does_not_retry_indefinitely`：mock `Path.write_text` raise `OSError`、verify 只 call 一次 + log entry `event="write_failed"`
- [x] 7.4 RED `test_station.py::test_validator_issues_feed_into_next_attempt_prompt_as_correction_hint`：第一次 attempt 的 issues 出現在第二次 attempt 的 prompt 中
- [x] 7.5 GREEN — Generator entrypoint orchestrates per-station markdown pipeline (per-station path)：實作 `generator/station.py::_generate_station(station, idx, context, provider, validator, sanitizer, ...)` retry loop max 3 + degraded fallback + write file + log
- [x] 7.6 GREEN — Decision 4: degraded fallback per-station stub + retry quota 3（對齊既有 spec §十）：實作 `generator/station.py::_make_degraded_stub(station_id, station_index, station_title)` 產 minimal markdown — 對應 spec Requirement `Degraded fallback writes per-station stub after retry exhaustion`
- [x] 7.7 跑測 4 測全綠

## 8. Sanitizer Pass 1 over Generator output — RED 後 GREEN

對應 spec Requirement `Generator output passes Sanitizer Pass 1 before disk write`（2 scenarios）。覆蓋 design Decision 1。

- [x] 8.1 [P] RED `tests/generator/test_sanitize_output.py::test_station_file_content_with_pii_pattern_triggers_pass_1_hit`：LLM output 含 `alice@example.com`、disk-written 檔案含 `<REDACTED:email#0>`、`<ws>/.codebus/sanitize_audit.jsonl` 含 `pass_num=1` `source.path` 對應 station path
- [x] 8.2 [P] RED `test_sanitize_output.py::test_clean_llm_output_writes_verbatim_with_no_audit_entries`：純文字 output、verbatim 寫檔、audit 0 hit
- [x] 8.3 GREEN — Decision 1: Generator output 過 Pass 1 Sanitizer（YES）— 對應 spec Requirement `Generator output passes Sanitizer Pass 1 before disk write`：在 `_generate_station` 或 `runner.py` 寫檔前 call `SanitizerEngine.sanitize(content, source=FileSource(path=output_path))`
- [x] 8.4 GREEN：sanitize hit append 到 `<ws>/.codebus/sanitize_audit.jsonl`（既有 `SanitizerAuditLogger` 注入 — 與 Scanner 同模式）
- [x] 8.5 跑測 2 測全綠

## 9. MOC assembler — RED 後 GREEN

對應 spec Requirement `MOC assembler writes pure-index tutorial.md with standard markdown links`（3 scenarios）。

- [x] 9.1 [P] RED `tests/generator/test_moc.py::test_interactive_moc_contains_numbered_station_list_with_standard_markdown_links`：3 stations，輸出含 numbered list 用 `[](./stations/sXX-slug.md)`，無 wikilinks
- [x] 9.2 [P] RED `test_moc.py::test_interactive_moc_ends_with_qaentry_element`：interactive mode → 含 `<QAEntry`、出在 `🎯 下車（完成）` heading 之後
- [x] 9.3 [P] RED `test_moc.py::test_plain_moc_replaces_qaentry_with_plain_sentence`：plain mode → 無 `<QAEntry`、含 plain sentence
- [x] 9.4 GREEN — MOC assembler writes pure-index tutorial.md with standard markdown links：實作 `generator/moc.py::assemble_moc(*, task, total_minutes, generated_at, workspace_name, station_summaries, mode, output_path) -> None`
- [x] 9.5 跑測 3 測全綠

## 10. route.json writer — RED 後 GREEN

對應 spec Requirement `route.json output carries D-029 §八 schema with station_id and file_path`（3 scenarios）。

- [x] 10.1 [P] RED `tests/generator/test_route.py::test_clean_run_emits_route_json_with_all_stations_and_no_top_level_degraded`
- [x] 10.2 [P] RED `test_route.py::test_all_degraded_run_sets_top_level_degraded_flag`
- [x] 10.3 [P] RED `test_route.py::test_file_path_uses_stations_relative_path_with_stable_id`
- [x] 10.4 GREEN — route.json output carries D-029 §八 schema with station_id and file_path：實作 `generator/route.py::write_route_json(...)` 對齊 spec §八 schema + `prerequisites=[]` P0 hardcode（Decision 2: Station.depends_on backfill 留 P1 / follow-up（NO for P0）— 此 P0 `prerequisites` 維持 empty list，待 follow-up `depends-on-backfill` change 補回真值）
- [x] 10.5 跑測 3 測全綠

## 11. generator_log.jsonl writer

對應 spec Requirement `Degraded fallback writes per-station stub after retry exhaustion` 末段（log 寫入點）。

- [x] 11.1 [P] RED `tests/generator/test_log.py::test_degraded_event_appended_with_required_keys`：assert 至少 `timestamp`, `station_id`, `station_index`, `attempts`, `last_issues` 五 key
- [x] 11.2 [P] RED `test_log.py::test_write_failed_event_appended`
- [x] 11.3 [P] RED `test_log.py::test_log_path_under_codebus_subdir`：assert `<ws>/.codebus/generator_log.jsonl`
- [x] 11.4 GREEN：實作 `generator/log.py::GeneratorLogger` (auto-mkdir parent like UsageTracker pattern + `append(event_type, **kwargs)` interface)
- [x] 11.5 跑測 3 測全綠

## 12. Output directory + path constants

對應 spec Requirement `Output root directory is workspace/codebus-tutorials per task`（2 scenarios）。覆蓋 design Decision 3。

- [x] 12.1 [P] RED `tests/generator/test_output_dir.py::test_first_write_creates_codebus_tutorials_directory_tree`：assert `<ws>/codebus-tutorials/{task_id}/stations/` exists
- [x] 12.2 [P] RED `test_output_dir.py::test_generator_does_not_write_to_codebus_subdir_except_generator_log_jsonl`：assert no station file / tutorial.md / route.json under `<ws>/.codebus/`、但 `<ws>/.codebus/generator_log.jsonl` exists
- [x] 12.3 GREEN — Decision 3: 輸出根目錄 `<ws>/codebus-tutorials/{task_id}/`（改 spec）— 對應 spec Requirement `Output root directory is workspace/codebus-tutorials per task`：在 `generator/runner.py` 加 module-level constant `_TUTORIALS_DIRNAME = "codebus-tutorials"`，所有 path 構造用此常數
- [x] 12.4 GREEN：第一個 station write 前 mkdir `<ws>/codebus-tutorials/{task_id}/stations/` (parents=True, exist_ok=True)
- [x] 12.5 跑測 2 測全綠

## 13. Generator entrypoint `run_generator` — RED 後 GREEN

對應 spec Requirement `Generator entrypoint orchestrates per-station markdown pipeline`（3 scenarios）。

- [x] 13.1 RED `tests/generator/test_runner.py::test_run_generator_over_three_scripted_stations_writes_three_station_files_plus_moc_plus_route`：scripted 3 stations → 3 station files + tutorial.md + route.json
- [x] 13.2 RED `test_runner.py::test_per_station_failure_does_not_abort_the_run`：第 2 站 degraded、1+3 正常、route.json 含 3 站、`degraded_count=1`
- [x] 13.3 RED `test_runner.py::test_generator_uses_tracked_provider_through_llm_chat_provider_factory`：`<ws>/.codebus/token_usage.jsonl` 帶 `module="generate"`、`<ws>/.codebus/llm_calls.jsonl` 收 wire payload、generator 不直接 call `tracker.record(...)`
- [x] 13.4 GREEN — Generator entrypoint orchestrates per-station markdown pipeline (orchestrator path)：實作 `generator/runner.py::run_generator(*, state, llm_chat_provider, kb, options)` 整合 station loop + sanitize + MOC + route.json + log
- [x] 13.5 跑測 3 測全綠

## 14. SSE generating events — RED 後 GREEN

對應 spec Requirement `SSE generating events stream per-station progress`（3 scenarios）。

- [x] 14.1 [P] RED `tests/generator/test_sse_events.py::test_three_station_run_emits_all_phases_for_each_station_plus_assembling_moc_twice`：assert 3 generating + 3 validating + 3 writing_file + 2 assembling_moc events
- [x] 14.2 [P] RED `test_sse_events.py::test_retry_attempt_emits_retry_status`：second station 第一次 fail validator → emit `status="retry"` `attempt=2`
- [x] 14.3 [P] RED `test_sse_events.py::test_missing_emitter_suppresses_all_generating_progress_events`：no emitter → 0 events，行為 identical
- [x] 14.4 GREEN — SSE generating events stream per-station progress：在 `runner.py` / `station.py` / `moc.py` 加 `emitter.emit({...})` 發 progress events（與 Explorer `agent-sse-wiring` 同模式）
- [x] 14.5 跑測 3 測全綠

## 15. POST /generate endpoint — RED 後 GREEN

對應 spec Requirement `task_id format` (sidecar-runtime MODIFIED 新 Generate kind scenario) + `Background task error containment` (sidecar-runtime MODIFIED 新 Generate task scenario)。

- [x] 15.1 RED `tests/api/test_generate_endpoint.py::test_generate_endpoint_requires_bearer_token`：no auth → 401
- [x] 15.2 RED `test_generate_endpoint.py::test_generate_kind_follows_same_shape`：generated `task_id` matches `^generate_[0-9a-f]{8}$`
- [x] 15.3 RED `test_generate_endpoint.py::test_in_flight_generate_blocks_other_task_creation`：generate task 在跑時 POST /scan / /kb/build / /explore / /generate 都 409 TASK_IN_FLIGHT
- [x] 15.4 RED `test_generate_endpoint.py::test_generate_task_exception_surfaces_as_safe_error_event`：unrecoverable exception → `error` event with `code="GENERATE_FAILED"` 不含 `repr(exc)`
- [x] 15.5 GREEN：實作 `api/generate.py::router` POST /generate handler，wire `_run_background_task` + `TaskHandleEmitter`
- [x] 15.6 GREEN — task_id format (sidecar-runtime MODIFIED Requirement `task_id format` 擴 generate kind)：在 `api/tasks.py::TaskKind` 加 `generate` member、regex 擴 `^(scan|kb|explore|generate)_[0-9a-f]{8}$`
- [x] 15.7 GREEN：在 `api/__init__.py` include `generate.router`
- [x] 15.8 GREEN — Background task error containment (sidecar-runtime MODIFIED Requirement `Background task error containment` 擴 GENERATE_FAILED)：error code table 加 `GENERATE_FAILED`
- [x] 15.9 跑測 4 測全綠

## 16. Documentation updates — Decision 3 改 spec + 連動

- [x] 16.1 改 `docs/module-5-generator.md` L27 `<workspace-root>/tutorials/{task_id}/` → `<workspace-root>/codebus-tutorials/{task_id}/`
- [x] 16.2 改同檔 §七 layout 樹圖、§八 route.json `file_path` 樣本、§九 sandbox 描述路徑樣本對齊
- [x] 16.3 `docs/decisions.md` D-029 連動更新清單補 `[x] 多檔輸出落地（module-5-generator-p0）`、`[x] 採 codebus-tutorials/ 而非 tutorials/ root`
- [x] 16.4 `docs/implementation-plan.md` 步驟 24 狀態 `🚧 in-progress`（archive 後改 `✅ landed P0（module-5-generator-p0）`）
- [x] 16.5 `docs/reviews/2026-04-25-stage-4.md` Module 5 預警 4 項全打 `[x]`（Pass 1 ✓、depends_on 留 P1 ✓、output dir codebus-tutorials/ ✓、degraded fallback ✓）
- [x] 16.6 `CLAUDE.md` archive 表加 row（待 archive 後 work commit 一起做，archive date placeholder）
- [x] 16.7 `CLAUDE.md` 「八大 Module」段或對應段更新 Module 5 狀態（從 "待開工" 改 "P0 landed"）

## 17. 完整驗證 + commit gate

- [x] 17.1 `uv run pytest sidecar/tests/` 完整 suite 全綠（baseline 699 passed → 實際 751 passed / 19 skipped，多 52 新測符合預期）
- [x] 17.2 `pre-commit run --all-files` 全綠
- [x] 17.3 `spectra validate --strict` 整個 change 合法
- [x] 17.4 `Grep "tutorials/" docs/module-5-generator.md` 確認無 generic `tutorials/` mention（剩下兩處 `tutorials/` 是 Decision 3 對比文字 + path comment，刻意保留）
- [x] 17.5 手動驗證 `<ws>/codebus-tutorials/{task_id}/` directory tree 在第一個 station write 後存在（integration test `test_first_write_creates_codebus_tutorials_directory_tree` 已涵蓋）
- [x] 17.6 確認 `module-5-generator` capability spec 9 個 Requirement 全有對應 production code + test，0 spec drift（spec 實際有 9 條 ADDED Requirement，task 描述「7 個」是當時估算，全數覆蓋）
