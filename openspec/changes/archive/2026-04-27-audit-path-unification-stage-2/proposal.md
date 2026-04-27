## Why

`docs/reviews/2026-04-26-stage-5.md` Cat 2.5 點出 4 條 cross-cutting code drift —— `audit-path-unification`（2026-04-25 archive）已建立 `_audit_paths.py` single-source pattern + `review-backlog-cleanup` 已建立 `RULES_VERSION` single-constant pattern，但有 4 組 callsite 沒掃乾淨：（a）4 處 `*.jsonl` 字面量散落 `api/scan.py` / `api/qa.py` / `agent/tools/folder_tools.py`；（b）`_STATION_ID_RE` 在 6 處 module 各定義一份（`agent/tools/add_to_kb.py` / `agent/tools/kb_search.py` / `kb/growth_logger.py` / `kb/knowledge_base.py` / `kb/payload.py` / `api/qa.py`）；（c）5 個 `_QA_MAX_*` budget 常數在 2 處重複；（d）`_QA_DEDUP_THRESHOLD=0.95` 在 2 處重複。

Phase 6 前端動工會 pin 在 audit 路徑 + station id regex 上（R-01 panel 直接 glob `<ws>/.codebus/*.jsonl`、frontend 也會校驗 station id）；現在不收齊，將來 bump rules / 改 budget / 改 regex 時會有 drift 漏掃風險（`RULES_VERSION` 收緊那次就吃過虧）。本 change 一次性收掉 4 組 drift，並補 grep-based defensive test 鎖死「single source 不可分歧」契約。

關聯 ADR：D-021（`token_usage.jsonl` path）/ D-022（`llm_calls.jsonl` path）/ 不變式 #9（rules version single source）；對齊 `audit-path-unification` 落地的 `_audit_paths.py` 模式 + `review-backlog-cleanup` 落地的 `RULES_VERSION` identity-check defensive test 模式。

## What Changes

### A. `*.jsonl` 字面量收回 `_audit_paths.py`（Cat 2.5-1）

- `sidecar/src/codebus_agent/api/scan.py:48-49` 拆掉重複定義的 `_WORKSPACE_AUDIT_SUBDIR=".codebus"` + `_SANITIZE_AUDIT_FILENAME="sanitize_audit.jsonl"`，改 `from codebus_agent._audit_paths import ...`
- `sidecar/src/codebus_agent/api/qa.py:207` 字面量 `"sanitize_audit.jsonl"` 改 import
- `sidecar/src/codebus_agent/agent/tools/folder_tools.py:118` 字面量 `"tool_audit.jsonl"` 改 import
- `sidecar/src/codebus_agent/agent/tools/folder_tools.py:473` 字面量 `"sanitize_audit.jsonl"` 改 import
- 補 grep-based defensive test：`sidecar/src/codebus_agent/` 內所有 `*.jsonl` 字面量字串僅允許出現在 `codebus_agent/_audit_paths.py`

### B. `_STATION_ID_RE` single source（Cat 2.5-2）

- 新建 leaf module `sidecar/src/codebus_agent/agent/station_id.py`（types-only，避免 circular import），匯出：
  - `STATION_ID_RE: re.Pattern`
  - `validate_station_id(sid: str) -> None`（invalid 即 raise `ValueError`）
  - `find_invalid_station_id(ids: list[str]) -> str | None`（回第一個違反 regex 的 id 或 None）
- 6 處 callsite 改 import：`agent/tools/add_to_kb.py:36` / `agent/tools/kb_search.py:24` / `kb/growth_logger.py:31` / `kb/knowledge_base.py:50` / `kb/payload.py:23` / `api/qa.py:51`
- 補 `is`-identity defensive test 鎖死 6 處 callsite 必同 `re.Pattern` object（仿 `test_rules_version_constant.py` 模式）

### C. `_QA_MAX_*` budget 常數 single source（Cat 2.5-3）

- `agent/qa.py:65-69` 5 個常數（`_QA_MAX_STEPS` / `_QA_MAX_ADD_TO_KB_PER_SESSION` / `_QA_MAX_CHUNK_SIZE_CHARS` / `_QA_MAX_ADD_TO_KB_PER_QUESTION` / `_QA_DEDUP_THRESHOLD`）保持為 single source
- `agent/tools/add_to_kb.py:37-39` 重複定義的 3 個常數刪掉，改 `from codebus_agent.agent.qa import ...`
- 補 `is`-identity defensive test

### D. `_QA_DEDUP_THRESHOLD` single source（Cat 2.5-4）

- `kb/knowledge_base.py:51` 重複定義的 `_QA_DEDUP_THRESHOLD: float = 0.95` 刪掉，改 `from codebus_agent.agent.qa import _QA_DEDUP_THRESHOLD`
- 補 `is`-identity defensive test（與 C 共用 test 檔）

## Non-Goals

- **不擴增 `_audit_paths.py` 的 path constants**：本 change 只收 callsite drift，不改 7 個 filename 常數本身。
- **不重構 `_audit_paths.py` 與 `api/_audit_paths.py` 的 backward-compat shim 關係**：`audit-path-unification` 已落地 `codebus_agent/_audit_paths.py` 為 canonical + `api/_audit_paths.py` 為 re-export shim 的雙 module 結構，本 change 沿用不動。
- **不改任何 production 行為 / wire payload / audit JSONL schema**：純 import 重構 + grep / identity defensive test，全 suite baseline `830 passed / 19 skipped` 應不變（僅新增測試 +N）。
- **不抽 station id regex 的 invalid token format**：`r"^s\d{2}-[a-z0-9-]{1,40}(-\d+)?$"` 為單一 regex 字面量，本 change 把它從 5 處重複改 1 處 canonical，不改 regex 本身。
- **不 refactor `_QACtxAdapter`**（屬 Cat 2.5 review 拆掉的 Cat 3，本 change 範圍不收）。
- **不留 backward-compat alias**：直接刪重複 callsite 改 import，沒人在 production 依賴 `add_to_kb._STATION_ID_RE` 等私名（grep 確認過）。

## Capabilities

### New Capabilities

（無）

### Modified Capabilities

- `qa-agent`：加 Scenario 鎖死「`_STATION_ID_RE` / `_QA_MAX_*` / `_QA_DEDUP_THRESHOLD` 必走 `agent.qa` 與 `agent.station_id` single source，callsite 不可重複定義」
- `kb-growth`：加 Scenario 鎖死「`KBGrowthLogger.write` 的 station_id pre-validation 必走 `agent.station_id` canonical regex」
- `knowledge-base`：加 Scenario 鎖死「`_validate_station_filter` / `upsert_chunk` dedup threshold 必走 single source」
- `sanitizer`：加 Scenario 鎖死「`sanitize_audit.jsonl` filename 字面量僅允許出現在 `codebus_agent/_audit_paths.py`」（推廣 `kb-growth` 既有的 path-constant 約束到全 7 層 audit）

## Impact

- Affected specs（4 個 MODIFIED capability）：
  - openspec/specs/qa-agent/spec.md
  - openspec/specs/kb-growth/spec.md
  - openspec/specs/knowledge-base/spec.md
  - openspec/specs/sanitizer/spec.md
- Affected code:
  - New:
    - sidecar/src/codebus_agent/agent/station_id.py（leaf module，types-only）
    - sidecar/tests/agent/test_station_id_constant.py（identity defensive test）
    - sidecar/tests/agent/test_qa_constants_single_source.py（QA budget + dedup threshold identity test）
    - sidecar/tests/test_no_jsonl_literal_drift.py（grep-based source-level test）
  - Modified:
    - sidecar/src/codebus_agent/api/scan.py
    - sidecar/src/codebus_agent/api/qa.py
    - sidecar/src/codebus_agent/agent/tools/folder_tools.py
    - sidecar/src/codebus_agent/agent/tools/add_to_kb.py
    - sidecar/src/codebus_agent/agent/tools/kb_search.py
    - sidecar/src/codebus_agent/kb/growth_logger.py
    - sidecar/src/codebus_agent/kb/knowledge_base.py
    - sidecar/src/codebus_agent/kb/payload.py
  - Removed: 無
