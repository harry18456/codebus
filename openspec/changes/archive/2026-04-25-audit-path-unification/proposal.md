## Why

`docs/reviews/2026-04-25-stage-4.md` Cat 2.5-B 決策（commit `06744bb`）已選定統一所有 workspace-level audit JSONL 到 `<ws>/.codebus/` 子目錄，但執行延後。本 change 是執行階段。

**Ordering 考量**：Module 5 Generator P0 動工會透過 `app.state.llm_chat_provider(ws)` factory 寫 audit JSONL（`default_module="chat"`）。如果先 Module 5、後本 change，Generator 會繼承舊 `<ws>/<file>.jsonl` path，再次回頭動 Module 5 wiring；先本 change、後 Module 5，Generator 自然繼承新 `<ws>/.codebus/<file>.jsonl` path，零回頭。

對齊 Cat 2.5-B 決策：勝出方案 (a) 統一到 `<ws>/.codebus/`（vs 維持兩 group 含糊狀態 / vs 統一到 `<ws>/` root 污染 workspace）。

## What Changes

**A. Production code — 6 個 factory path strings + ReasoningLogger 父目錄 mkdir**

`sidecar/src/codebus_agent/api/__init__.py` 三個 factory：
- `_make_tracker_factory:170` — `ws / "token_usage.jsonl"` → `ws / _WORKSPACE_AUDIT_SUBDIR / "token_usage.jsonl"`
- `_make_provider_factory:198-199` — UsageTracker + LLMCallLogger 路徑同模式
- `_make_chat_provider_factory:243-244` — UsageTracker + LLMCallLogger 路徑同模式

`sidecar/src/codebus_agent/api/explore.py:178` — `workspace_root / "reasoning_log.jsonl"` → `workspace_root / _WORKSPACE_AUDIT_SUBDIR / "reasoning_log.jsonl"`，並在 `ReasoningLogger(...)` 構造前加 `(workspace_root / _WORKSPACE_AUDIT_SUBDIR).mkdir(parents=True, exist_ok=True)`（因為 ReasoningLogger 不 auto-mkdir，spec 規定 caller-side path safety）。

`UsageTracker` 與 `LLMCallLogger` 已 auto-mkdir parent（`usage_tracker.py:32` / `llm_call_logger.py:49`），無須改 constructor。

**B. 引入命名常數消除 magic string**

api/__init__.py 已有 `_WORKSPACE_AUDIT_SUBDIR = ".codebus"` 與 `_SANITIZE_AUDIT_FILENAME = "sanitize_audit.jsonl"` 兩常數（M1 落地時建立）。本 change 補三個 sibling 常數對齊：

```python
_TOKEN_USAGE_FILENAME = "token_usage.jsonl"
_LLM_CALLS_FILENAME = "llm_calls.jsonl"
_REASONING_LOG_FILENAME = "reasoning_log.jsonl"
```

也讓 api/explore.py import 並複用 `_WORKSPACE_AUDIT_SUBDIR` + `_REASONING_LOG_FILENAME`，杜絕 magic string 散播。

**C. Spec MODIFIED Requirements — 2 條**

C-1. `usage-tracking` `UsageTracker writes token_usage.jsonl` Requirement：路徑從 `<workspace>/token_usage.jsonl` 改為 `<workspace>/.codebus/token_usage.jsonl`。
C-2. `usage-tracking` `LLMCallLogger writes llm_calls.jsonl` Requirement：路徑從預設 `<workspace>/llm_calls.jsonl` 改為 `<workspace>/.codebus/llm_calls.jsonl`（spec 主文若無明確路徑則新增句子明寫 path convention）。
C-3. `agent-core` `ReasoningLogger appends one JSONL line per Step to workspace path` Requirement：路徑從 `{workspace_root}/reasoning_log.jsonl` 改為 `{workspace_root}/.codebus/reasoning_log.jsonl`，主文補一句說明 caller 必先 ensure `.codebus/` 子目錄存在（與 `sanitize_audit` / `tool_audit` 既有 path convention 對齊）。

**D. Tests — factory-output assertion 更新**

只動「真的 assert factory 輸出 path」的 test，不動「直接 construct logger 給任意 path」的 test：

- `test_kb_build_production.py:218` `assert (ws_path / "token_usage.jsonl").exists()` → 改吃 `.codebus/` 路徑
- `test_kb_build_production.py:243` 對應 line count assertion 同步
- `test_kb_query.py` / `test_explore_endpoint.py` / `test_explore_sse_integration.py` / `test_wire_kb_dependencies.py` 的相同模式 assertion 同步
- 直接 construct（如 `UsageTracker(tmp_path / "token_usage.jsonl")`）的 test **不動** —— path 是 test 自選的，與 factory 路徑慣例無關

**E. CLAUDE.md 七層 Audit JSONL 段更新**

把 `position: <ws>/` 三條（reasoning_log / token_usage / llm_calls）改成 `position: <ws>/.codebus/`，與既有 sanitize_audit / tool_audit 描述對齊。Cat 2.5-B 「latent risk」段 paragraph 拿掉（已解決）。

**F. Review tracker 收尾**

`docs/reviews/2026-04-25-stage-4.md` Cat 2.5-B 從 `🟨 決策完成、執行延後` 改 `✅ 完成`。

## Non-Goals

- **不動 `sanitize_audit.jsonl` / `tool_audit.jsonl` 的 path** — 兩者已在 `<ws>/.codebus/`，零變動
- **不動 `kb_growth.jsonl`** — 待 Module 8 P0 落地時建立，本 change 之後它會自然遵循 `.codebus/` convention
- **不動 `~/.codebus/authorization_audit.jsonl`** — 那是 App-level（跨 workspace），與 workspace-level 路徑統一無關
- **不引入 path migration 工具** — 既有使用者 workspace 沒有任何（Module 4 P0 落地至今沒外發），不需 migration script
- **不重 architect ReasoningLogger / UsageTracker / LLMCallLogger constructor 行為** — UsageTracker / LLMCallLogger 已 auto-mkdir，ReasoningLogger 維持 caller-side mkdir convention（spec 已規定）
- **不動 production code 行為** — audit 內容、欄位、JSON shape 完全不變，只改寫入位置
- **不動 fixture baseline** — `tests/golden/*/expected.json` 與 `ideal-route.json` 與本 change 無關（fixture workspace 是隔離的）
- **不動 archive folder** — 永遠 frozen
- **不動 Cat 2.5-A 已落地的部分** — `auth-flow` change 排程仍在 step 26.5

**拒絕的設計**

- **「保留 backward-compat 雙寫舊 path + 新 path」**：違反 audit chain 「single source of truth」精神，多寫一份等於多一個 drift 風險源
- **「在 ReasoningLogger 加 auto-mkdir」**：`agent-core` spec L156 明寫「MUST NOT silently create parent directories outside the workspace」—— 改 logger 行為要動 spec scenario，且改後 caller 也少了一次 path-safety 檢查機會
- **「拆兩 change（先 token_usage / llm_calls，後 reasoning_log）」**：三個 path 是同一件事「workspace audit 統一目錄」，atomic 改最乾淨。檔案數本來就少（3 個 production code 檔 + 2 spec + N 個 test）
- **「直接動 UsageTracker / LLMCallLogger / ReasoningLogger 預設 path」**：違反 dependency injection 設計，constructor 不該知道 workspace convention，path 是 caller / factory 的責任

## Capabilities

### New Capabilities

（無）

### Modified Capabilities

- `usage-tracking`：MODIFIED 兩條 Requirement
  - `UsageTracker writes token_usage.jsonl` —— path 從 `<workspace>/` 移到 `<workspace>/.codebus/`
  - `LLMCallLogger writes llm_calls.jsonl` —— path 從 `<workspace>/` 移到 `<workspace>/.codebus/`
- `agent-core`：MODIFIED 一條 Requirement
  - `ReasoningLogger appends one JSONL line per Step to workspace path` —— path 從 `{workspace_root}/reasoning_log.jsonl` 移到 `{workspace_root}/.codebus/reasoning_log.jsonl`，caller-side `.codebus/` mkdir 約束 explicit

## Impact

**受影響 spec**：

- `openspec/specs/usage-tracking/spec.md`（兩條 MODIFIED Requirement）
- `openspec/specs/agent-core/spec.md`（一條 MODIFIED Requirement）

**受影響 production code**：

- `sidecar/src/codebus_agent/api/__init__.py`（3 個 factory，5 個 path strings + 3 個 filename 常數新增）
- `sidecar/src/codebus_agent/api/explore.py`（1 個 path string + ReasoningLogger 前加 mkdir + 共用常數 import）

**受影響 test**：

只改 factory-output assertion 的 test 檔（預估 5-7 檔，與 `wire_kb_dependencies` / `app.state.llm_*_provider` 互動的）：

- `test_wire_kb_dependencies.py`
- `test_kb_build_production.py`
- `test_kb_query.py`
- `test_explore_endpoint.py`
- `test_explore_sse_integration.py`
- 其他若 grep 揭露的同模式 assertion

**直接 construct 的 logger test 不動**（~29 檔），因為 path 是 test 自選的 arbitrary 值，與 factory 路徑慣例無關。

**受影響 docs**：

- `CLAUDE.md` 七層 Audit JSONL 段位址更新 + 拿掉 Cat 2.5-B latent risk paragraph
- `docs/reviews/2026-04-25-stage-4.md` Cat 2.5-B 改 ✅ 完成

**無新依賴**。**無 schema 改動**（audit JSONL 內容欄位完全不變）。**無 LLM 行為改變**。

**Migration**：無 — 既有使用者 workspace 沒有，不需 migration script。內部 dev / golden replay 用 tmp_path，每次 fresh，零碰撞。
