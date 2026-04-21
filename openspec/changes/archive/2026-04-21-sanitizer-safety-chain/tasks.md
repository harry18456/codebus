> 對齊 `docs/implementation-plan.md §二` 第二階段步驟 9-12 與 `docs/sanitizer.md §九` P0；所有實作 task 走 TDD（先寫 failing test，再實作使其通過）。`[P]` 標記表示可並行執行（目標不同檔、無 in-flight 依賴）。

## 1. Setup 與相依

- [x] 1.1 在 `sidecar/pyproject.toml` `[project]` 加入 `detect-secrets` 相依；`uv sync` 確認 lock 更新且無衝突
- [x] 1.2 建立模組骨架：`sidecar/src/codebus_agent/sanitizer/__init__.py` / `engine.py` / `rules.py` / `config.py` / `audit.py`（空檔或最小 stub，讓後續 task 有目標）
- [x] 1.3 建立測試骨架：`sidecar/tests/sanitizer/__init__.py` + `tests/sanitizer/fixtures/`（含合成 secret / PII / IP / 內部 TLD 樣本文字檔，檔內容為 sanitize 目標）

## 2. SanitizerConfig loads from workspace-then-global YAML

- [x] 2.1 [P] 寫失敗測試 `test_config_load_workspace_replaces_global`：覆蓋 Decision「Sanitizer config — 兩層覆蓋 + Pydantic strict 驗證」的「workspace 覆蓋整份、非 merge」語意
- [x] 2.2 [P] 寫失敗測試 `test_config_fallback_global_when_workspace_absent` 與 `test_config_builtin_defaults_when_neither_file`
- [x] 2.3 [P] 寫失敗測試 `test_config_unknown_field_rejected`（`extra="forbid"` 行為）
- [x] 2.4 [P] 寫失敗測試 `test_config_missing_rules_version_raises`，對應 Requirement「Rules version is recorded on every audit line」的 config 載入 scenario
- [x] 2.5 在 `config.py` 實作 `SanitizerConfig` Pydantic model（`rules_version: str` 必填、`ConfigDict(extra="forbid")`）與 `SanitizerConfig.load(workspace_root)` 載入器，滿足 Requirement「SanitizerConfig loads from workspace-then-global YAML」；所有 2.1-2.4 測試轉綠
- [x] 2.6 實作 `PatternAllowlistEntry`（`pattern: str` / `reason: str` 皆必填）；補測 `test_pattern_allowlist_entry_requires_reason` 覆蓋 Requirement「Config declares allowlist structure without requiring non-empty contents」

## 3. Built-in rule set covers Secret, PII, internal-identifier kinds

- [x] 3.1 [P] 寫失敗測試 `test_rule_taiwan_mobile`（`0912-345-678` / `0912345678` / `+886-912-345-678` 三種格式）
- [x] 3.2 [P] 寫失敗測試 `test_rule_taiwan_national_id`（`A123456789` 合法碼 + 非法開頭字元負測）
- [x] 3.3 [P] 寫失敗測試 `test_rule_email_basic` 與 `test_rule_rfc1918_ip`（`10.x.x.x` / `172.16-31.x.x` / `192.168.x.x` / RFC4193 / link-local）
- [x] 3.4 [P] 寫失敗測試 `test_rule_internal_tld`（`.local` / `.internal` / `.corp` / `.lan`）
- [x] 3.5 [P] 寫失敗測試 `test_rule_detect_secrets_integration`：整合 `detect-secrets` plugin，合成 AWS key / JWT / PEM 片段，要求 kind 分別為 `secret` / `jwt` / `private-key`
- [x] 3.6 在 `rules.py` 實作 built-in rule table（`rule_id` / `kind` / `pattern` / detector callable），滿足 Requirement「Built-in rule set covers Secret, PII, internal-identifier kinds」；3.1-3.5 全綠

## 4. SanitizerEngine exposes pure `sanitize` interface

- [x] 4.1 [P] 寫失敗測試 `test_engine_pass1_replaces_email_and_returns_audit_entries`（Pass 1 FileSource；覆蓋 Requirement「SanitizerEngine exposes pure `sanitize` interface」的主要 scenario）
- [x] 4.2 [P] 寫失敗測試 `test_placeholder_format_matches_redacted_kind_index`，覆蓋 Requirement「Placeholder format is `<REDACTED:kind#index>`」
- [x] 4.3 [P] 寫失敗測試 `test_same_value_same_placeholder_within_call` 與 `test_placeholder_index_resets_across_calls`，覆蓋 Requirement「Placeholder index scope is single sanitize call」及 Decision「Placeholder index — 單檔 scope、session-less、in-memory」
- [x] 4.4 [P] 寫失敗測試 `test_engine_no_reverse_mapping_exposed`：靜態反射 + 呼叫 API，確認無任何方法接受 placeholder 回傳原值
- [x] 4.5 實作 `engine.py` 的 `SanitizerEngine.sanitize(text, source)`；內部 `_index_scope: dict[(kind, value), int]` per-call reset；4.1-4.4 全綠
- [x] 4.6 寫失敗測試 `test_engine_fail_closed_raises_sanitizer_error`（模擬 rule plugin 內部 raise），實作 `SanitizerError` 與 `__cause__` 鏈，覆蓋 Decision「Fail-closed 失敗處理」與 Requirement 的 fail-closed scenario

## 5. SanitizerAuditLogger appends each replacement to JSONL

- [x] 5.1 [P] 寫失敗測試 `test_audit_line_contains_required_fields`：Pass=1 寫入後讀回 JSONL，assert 10 個固定欄位與 `extra: {}`，覆蓋 Requirement「SanitizerAuditLogger appends each replacement to JSONL」
- [x] 5.2 [P] 寫失敗測試 `test_audit_rules_version_propagates_from_config`
- [x] 5.3 [P] 寫失敗測試 `test_audit_schema_version_equals_1`
- [x] 5.4 [P] 寫失敗測試 `test_audit_concurrent_writes_atomic`（兩 thread 併寫 100 筆，每行仍為完整 JSON + `\n`）
- [x] 5.5 實作 `audit.py` `SanitizerAuditLogger.append(entry, pass_num, rules_version, session_id)`；使用 `fcntl`（POSIX）/ `msvcrt.locking`（Windows）或單 thread lock 確保每行原子寫入；對齊 Decision「sanitize_audit.jsonl schema — 固定 10 欄位 + `extra` 擴充欄位」；5.1-5.4 全綠

## 6. Allowlist hits still audited but not redacted

- [x] 6.1 [P] 寫失敗測試 `test_pattern_allowlist_hit_leaves_text_and_flags_extra`（pattern `^noreply@` 命中不替換，`extra.allowlisted == true`），覆蓋 Requirement「Allowlist hits still audited but not redacted」
- [x] 6.2 [P] 寫失敗測試 `test_path_allowlist_glob_matches`（`tests/fixtures/**`）與 `test_filename_allowlist`（`.env.example`）
- [x] 6.3 在 engine 載入 config 後的 pipeline 加入 allowlist 決策層；6.1-6.2 全綠

## 7. TrackedProvider applies Sanitizer Pass 2 before dispatch

- [x] 7.1 [P] 寫失敗測試 `test_tracked_provider_chat_sanitizes_before_wrapped_provider`：assert 內層 `MockProvider.chat` 收到的 messages 已替換；覆蓋 Requirement「TrackedProvider applies Sanitizer Pass 2 before dispatch」與 Decision「Pass 2 hook point — TrackedProvider 裝飾器層，而非 base LLMProvider」
- [x] 7.2 [P] 寫失敗測試 `test_tracked_provider_embed_sanitizes_texts`
- [x] 7.3 [P] 寫失敗測試 `test_sanitizer_pass2_applied_field_true_after_call`：讀 `llm_calls.jsonl` 最末行，欄位為 `true` 且仍為 boolean 型別
- [x] 7.4 [P] 寫失敗測試 `test_tracked_provider_sanitizer_failure_aborts_dispatch`：engine raise 後，inner `MockProvider.chat` 不被呼叫、`llm_calls.jsonl` 無新行、`SanitizerError` 向外傳
- [x] 7.5 [P] 寫失敗測試 `test_pass2_audit_entry_written_with_message_prefix`，覆蓋 Requirement「TrackedProvider writes audit entries to sanitize_audit.jsonl」與 `source` 以 `message:` 前綴
- [x] 7.6 修改 `providers/tracked.py`：於 `chat` / `embed` 開頭注入 Pass 2 sanitize（inject `SanitizerEngine` 與 `SanitizerAuditLogger`），dispatch 前完成替換與稽核寫入；原 `UsageTracker` / `LLMCallLogger` 流程不動；7.1-7.5 全綠
- [x] 7.7 修改 registry 實例化路徑：建構 `TrackedProvider` 時注入共享 `SanitizerEngine` + `SanitizerAuditLogger`；補測 registry 在缺 sanitizer 注入時 raise `ValueError`

## 8. ToolSandbox appends every invocation to tool_audit.jsonl

- [x] 8.1 [P] 寫失敗測試 `test_tool_audit_successful_invocation_line`，覆蓋 Requirement「ToolSandbox appends every invocation to tool_audit.jsonl」的成功 scenario 與 Decision「tool_audit.jsonl schema — 呼應 ToolSandbox `ensure_in_workspace` 結果」
- [x] 8.2 [P] 寫失敗測試 `test_tool_audit_denied_invocation_writes_denial_reason`：路徑逃逸被擋，`allowed=false` 與 closed-enum `denial_reason`
- [x] 8.3 [P] 寫失敗測試 `test_tool_audit_args_summary_excludes_non_whitelisted`，覆蓋 Requirement「Tools declare their auditable field whitelist」的 args_summary scenario
- [x] 8.4 [P] 寫失敗測試 `test_tool_without_audit_fields_rejected_at_registration`
- [x] 8.5 [P] 寫失敗測試 `test_tool_audit_schema_version_equals_1`，覆蓋 Requirement「Schema version on every tool audit line」
- [x] 8.6 在 `sandbox.py` 的 tool 註冊與 dispatch 路徑插入 `tool_audit.jsonl` 寫入（成功與拒絕皆寫）；denied path 先寫 audit 再 raise 原 sandbox 違規；8.1-8.5 全綠

## 9. 整合驗證與不變式守護

- [x] 9.1 寫整合測試 `test_end_to_end_chat_call_writes_both_audit_files`：以 `TrackedProvider` 跑一次合成 prompt，assert `sanitize_audit.jsonl` 有 `pass=2` 行、`llm_calls.jsonl` 最末行 `sanitizer_pass2_applied: true` 且 payload 已去識別化
- [x] 9.2 寫 regression 測試 `test_zero_outbound_invariant_still_holds`：沿用 M1 既有 `respx` / socket patch fixture，確認本 change 不產生任何對外 HTTP
- [x] 9.3 跑完整 `uv run pytest`（預期約 94 + 本 change 新增 ~30 個 test 全綠，Qdrant / symlink 相關自動 skip 維持原狀）
- [x] 9.4 `pre-commit run --all-files`：確認 ruff / pyright / 其他 hook 全過；`bash tests/precommit_gate_test.sh` 仍綠
- [x] 9.5 於 `docs/decisions.md` 檢視是否需新增 D-XXX 記錄 Pass 2 hook point / audit schema v1；若是，先改 `decisions.md` 再回頭在 spec 首行引用（M1 已 archive 的 capability spec 不在本 change 直改，由 spectra archive 流程處理）
