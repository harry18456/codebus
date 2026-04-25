## 1. 前置驗證

- [x] 1.1 baseline test 確認全綠（`uv run pytest sidecar/tests/` ≥ 698 passed / 19 skipped）
- [x] 1.2 grep 確認影響範圍無變動（`Grep "ws.*token_usage.jsonl|ws.*llm_calls.jsonl|workspace_root.*reasoning_log.jsonl" sidecar/src/` 仍是 6 處 production hit + 1 處 reasoning_log hit）

## 2. Spec MODIFIED — usage-tracking 兩條 Requirement

- [x] 2.1 寫 `specs/usage-tracking/spec.md` delta，`## MODIFIED Requirements` 包 `UsageTracker writes token_usage.jsonl` 全文（4 scenario + main text），主文 `<workspace>/token_usage.jsonl` → `<workspace>/.codebus/token_usage.jsonl`
- [x] 2.2 同檔 `---` 分隔加第二條 MODIFIED：`LLMCallLogger writes llm_calls.jsonl` 全文（3 scenario），主文補一句明寫 path convention `<workspace>/.codebus/llm_calls.jsonl`
- [x] 2.3 `spectra validate --strict` 確認 delta 合法

## 3. Spec MODIFIED — agent-core ReasoningLogger Requirement

- [x] 3.1 寫 `specs/agent-core/spec.md` delta，`## MODIFIED Requirements` 包 `ReasoningLogger appends one JSONL line per Step to workspace path` 全文（4 scenario：含 Cat 2 加的 `Logger is the single source of truth for prompt version stamping`）
- [x] 3.2 主文 `{workspace_root}/reasoning_log.jsonl` → `{workspace_root}/.codebus/reasoning_log.jsonl`，補一句「The caller MUST ensure the `.codebus/` parent directory exists before constructing the logger (consistent with `<workspace>/.codebus/sanitize_audit.jsonl` / `<workspace>/.codebus/tool_audit.jsonl` and the workspace-level audit chain convention)」
- [x] 3.3 `Path stays under workspace` Scenario 微調：保留 ensure_in_workspace 約束，加註明 caller 也要負責 `.codebus/` 子目錄存在
- [x] 3.4 `spectra validate --strict` 確認 delta 合法

## 4. Production code — api/__init__.py factory paths

- [x] 4.1 加三 filename 常數（與既有 `_SANITIZE_AUDIT_FILENAME` 對齊）：
  ```python
  _TOKEN_USAGE_FILENAME = "token_usage.jsonl"
  _LLM_CALLS_FILENAME = "llm_calls.jsonl"
  _REASONING_LOG_FILENAME = "reasoning_log.jsonl"
  ```
- [x] 4.2 `_make_tracker_factory:170` —— `Path(workspace_root) / "token_usage.jsonl"` → `Path(workspace_root) / _WORKSPACE_AUDIT_SUBDIR / _TOKEN_USAGE_FILENAME`
- [x] 4.3 `_make_provider_factory:198-199` —— UsageTracker 用 `ws / _WORKSPACE_AUDIT_SUBDIR / _TOKEN_USAGE_FILENAME` / LLMCallLogger 用 `ws / _WORKSPACE_AUDIT_SUBDIR / _LLM_CALLS_FILENAME`
- [x] 4.4 `_make_chat_provider_factory:243-244` —— 同模式
- [x] 4.5 更新 `_make_provider_factory` docstring `<ws>/token_usage.jsonl` → `<ws>/.codebus/token_usage.jsonl` 等
- [x] 4.6 更新 module-level docstring（L24 `<workspace>/token_usage.jsonl etc.`）對齊新 path

## 5. Production code — api/explore.py reasoning_log path + mkdir

- [x] 5.1 `api/explore.py` import `_WORKSPACE_AUDIT_SUBDIR` + `_REASONING_LOG_FILENAME` from `.` (api/__init__.py)
- [x] 5.2 `api/explore.py:178` 前面加 `(workspace_root / _WORKSPACE_AUDIT_SUBDIR).mkdir(parents=True, exist_ok=True)`（spec 規定 ReasoningLogger 不 auto-mkdir，caller 負責）
- [x] 5.3 `api/explore.py:178` `ReasoningLogger(workspace_root / "reasoning_log.jsonl")` → `ReasoningLogger(workspace_root / _WORKSPACE_AUDIT_SUBDIR / _REASONING_LOG_FILENAME)`

## 6. Test fixture — factory-output assertion 更新

只動 factory-wiring test 中 assert path 的部分；直接 construct 的 test **不動**。

- [x] 6.1 grep `Grep "(ws|ws_path|tmp_path).*\"(token_usage|llm_calls|reasoning_log)\.jsonl\"" sidecar/tests/` 列出所有 candidate
- [x] 6.2 `test_kb_build_production.py:218,243` 改吃 `.codebus/` path
- [x] 6.3 `test_kb_query.py` 同模式 assertion 改
- [x] 6.4 `test_explore_endpoint.py` 同模式 assertion 改（注意 L189 `tmp_path / "reasoning_log.jsonl"` 是 fixture 自定的 ExplorerResult.log_path，**不動** —— 那是 result 物件 attribute，不是 factory 寫入路徑）
- [x] 6.5 `test_explore_sse_integration.py` 同模式 assertion 改
- [x] 6.6 `test_wire_kb_dependencies.py` 同模式 assertion 改
- [x] 6.7 cross-check：直接 construct `UsageTracker(tmp_path / "token_usage.jsonl")` 的 test 一律保留（path 是 test 自選 arbitrary 值）
- [x] 6.8 跑 `uv run pytest sidecar/tests/` 確認 0 regression

## 7. Docs — CLAUDE.md 七層 Audit JSONL 段更新

- [x] 7.1 `CLAUDE.md` 七層段六個 ✅ 層的位址改 `<ws>/.codebus/`：
  - `sanitize_audit.jsonl` ✓ 已是
  - `tool_audit.jsonl` ✓ 已是
  - `reasoning_log.jsonl` 改為 `<ws>/.codebus/`
  - `token_usage.jsonl` 改為 `<ws>/.codebus/`
  - `llm_calls.jsonl` 改為 `<ws>/.codebus/`
  - `kb_growth.jsonl` ⏳ 待 Module 8 落地時直接遵循 `.codebus/` convention（在七層描述順手 note 一下）
- [x] 7.2 `CLAUDE.md` 拿掉「Audit 路徑不一致是已知 latent risk」尾段（commit `06744bb` 加的）
- [x] 7.3 `CLAUDE.md` 順手提一句使用者可在 `.gitignore` 加 `.codebus/` 一行排除全部 workspace audit

## 8. 文件 / metadata 更新

- [x] 8.1 更新 `docs/reviews/2026-04-25-stage-4.md`：Cat 2.5-B 從 `🟨 決策完成、執行延後` 改 `✅ 完成`；Cat 2.5-B 全文補上 execution commit 連結
- [x] 8.2 更新 `CLAUDE.md` archive 表加入本 change（一行；archive date apply 階段先 placeholder，archive 後改實際）

## 9. 完整驗證 + commit gate

- [x] 9.1 `uv run pytest sidecar/tests/` 完整 suite 全綠（698 passed / 19 skipped baseline）
- [x] 9.2 `pre-commit run --all-files` 全綠
- [x] 9.3 `spectra validate --strict` 整個 change 合法
- [x] 9.4 `Grep "ws.*\"token_usage.jsonl\"|ws.*\"llm_calls.jsonl\"|workspace_root.*\"reasoning_log.jsonl\"" sidecar/src/` 確認 production code 0 hits（全用常數 + `.codebus/` subdir）
- [x] 9.5 `Grep "<workspace>/token_usage.jsonl|<workspace>/llm_calls.jsonl|{workspace_root}/reasoning_log.jsonl" openspec/specs/` 確認 spec 0 hits（全 `.codebus/`）
