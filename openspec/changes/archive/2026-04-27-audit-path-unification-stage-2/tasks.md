## 1. Setup canonical leaf module `agent/station_id.py`（Decision 1）

對應 design Decision 1（leaf module pattern）+ Decision 2（identity-check defensive test）。

- [x] 1.1 新建 `sidecar/src/codebus_agent/agent/station_id.py`：module docstring 引用 `audit-path-unification-stage-2` + `RULES_VERSION` pattern；module-level constants `STATION_ID_RE: re.Pattern = re.compile(r"^s\d{2}-[a-z0-9-]{1,40}(-\d+)?$")` + `_STATION_ID_RE = STATION_ID_RE` backward-compat private alias（讓 callsite import `_STATION_ID_RE` 名字也能對齊原本的 module-private convention）；helper functions `validate_station_id(sid: str) -> None`（invalid 即 raise `ValueError(f"invalid station_id: {sid}")`）+ `find_invalid_station_id(ids: list[str]) -> str | None`（回第一個違反 regex 的 id 或 None）；`__all__` 列出三 public symbol
- [x] 1.2 RED `sidecar/tests/agent/test_station_id_constant.py::test_station_id_re_single_source`：5 處 callsite import alias，全部 `is`-identity check；初始 RED 因為 callsite 仍各定義自己的 `re.compile(...)`（identity 不等）
- [x] 1.3 RED `test_station_id_constant.py::test_no_station_id_regex_compile_outside_canonical`：用 `Path.glob("**/*.py")` 走 `sidecar/src/codebus_agent/`，每個檔 source 內 grep `re.compile(r"^s\\d{2}-...")` 字串，assert 命中 0 次（除了 `agent/station_id.py` 本身）
- [x] 1.4 RED `test_station_id_constant.py::test_validate_station_id_helpers`：assert `validate_station_id("s02-storage")` 不 raise；`validate_station_id("bad-id")` raise `ValueError`；`find_invalid_station_id(["s02-x", "bad", "s03-y"])` 回 `"bad"`；`find_invalid_station_id(["s02-x"])` 回 `None`

## 2. Migrate 6 station_id callsites（Cat 2.5-2 / Decision 1）

6 處 callsite 改 `from codebus_agent.agent.station_id import _STATION_ID_RE`（用 backward-compat alias 名字符合既有 module-private convention）。

- [x] 2.1 [P] 改 `sidecar/src/codebus_agent/agent/tools/add_to_kb.py:36`：拆掉 `_STATION_ID_RE = re.compile(r"^s\d{2}-[a-z0-9-]{1,40}(-\d+)?$")`，加 `from codebus_agent.agent.station_id import _STATION_ID_RE`；如果 `import re` 在 file 內無其他用途則一併移除
- [x] 2.2 [P] 改 `sidecar/src/codebus_agent/agent/tools/kb_search.py:24`：同 2.1 處理
- [x] 2.3 [P] 改 `sidecar/src/codebus_agent/kb/growth_logger.py:31`：同 2.1 處理
- [x] 2.4 [P] 改 `sidecar/src/codebus_agent/kb/knowledge_base.py:50`：同 2.1 處理（注意 `kb/knowledge_base.py` 仍會 import `re`，因為 module 還有別的 regex 用途，只刪定義不刪 import）
- [x] 2.5 [P] 改 `sidecar/src/codebus_agent/api/qa.py:51`：同 2.1 處理（注意：原本是 `_STATION_ID_RE = r"..."` 字串字面量 + `re.fullmatch(_STATION_ID_RE, v)`，改 import 後同步把 callsite 改成 `_STATION_ID_RE.fullmatch(v)` method 形式；error message 內的 `{_STATION_ID_RE}` 也要改成 `{_STATION_ID_RE.pattern}`）
- [x] 2.5b [P] 改 `sidecar/src/codebus_agent/kb/payload.py:23`：同 2.1 處理（apply 階段重新 grep 補加，原 proposal 漏列；注意 `kb/payload.py` 仍 import `re` 因 `_TEXT_HASH_RE` 還在）
- [x] 2.6 GREEN — 1.2 / 1.3 / 1.4 + 既有 6 個 callsite 對應 spec scenario 全綠

## 3. QA budget constants single source（Cat 2.5-3 / Decision 4）

對應 design Decision 4（`agent.qa` 是 budget constants owner，反向 import 不循環）。

- [x] 3.1 改 `sidecar/src/codebus_agent/agent/tools/add_to_kb.py:37-39`：拆掉 3 個重複定義 `_QA_MAX_CHUNK_SIZE_CHARS` / `_QA_MAX_ADD_TO_KB_PER_SESSION` / `_QA_MAX_ADD_TO_KB_PER_QUESTION`，加 `from codebus_agent.agent.qa import _QA_MAX_CHUNK_SIZE_CHARS, _QA_MAX_ADD_TO_KB_PER_SESSION, _QA_MAX_ADD_TO_KB_PER_QUESTION`
- [x] 3.2 RED `sidecar/tests/agent/test_qa_constants_single_source.py::test_qa_budget_constants_single_source`：`from codebus_agent.agent.qa import _QA_MAX_*` + `from codebus_agent.agent.tools.add_to_kb import _QA_MAX_*`，3 個 `is`-identity assert
- [x] 3.3 RED `test_qa_constants_single_source.py::test_no_qa_max_definition_outside_canonical`：grep `^_QA_(MAX|DEDUP)_` 在 `sidecar/src/codebus_agent/`（line-anchored），assert 命中檔案 set 只有 `codebus_agent/agent/qa.py`
- [x] 3.4 GREEN — 3.1 + 3.2 + 3.3 串聯通過 + 既有 `tests/agent/test_qa_budget_constants.py` 全綠

## 4. Dedup threshold single source（Cat 2.5-4 / Decision 4）

- [x] 4.1 改 `sidecar/src/codebus_agent/kb/knowledge_base.py:51`：拆掉 `_QA_DEDUP_THRESHOLD: float = 0.95`，加 `from codebus_agent.agent.qa import _QA_DEDUP_THRESHOLD`（若 import 行已存在則合併到既有 import 群組）
- [x] 4.2 RED `test_qa_constants_single_source.py::test_dedup_threshold_single_source`：`from codebus_agent.kb.knowledge_base import _QA_DEDUP_THRESHOLD` + `from codebus_agent.agent.qa import _QA_DEDUP_THRESHOLD as _qa_threshold` + `is`-identity assert
- [x] 4.3 GREEN — 4.1 + 4.2 通過 + 既有 `tests/kb/test_upsert_chunk.py` 全綠（dedup 行為不變）
- [x] 4.4 確認 `__all__` 在 `kb/knowledge_base.py` 不再 export `_QA_DEDUP_THRESHOLD`（因為已從 `agent.qa` import 進來，不該重新 export 給下游）

## 5. JSONL 字面量收回 `_audit_paths.py`（Cat 2.5-1 / Decision 3 + 5）

對應 design Decision 3（grep-based source-level test）+ Decision 5（直接刪重複常數，不改 alias 名）。

- [x] 5.1 [P] 改 `sidecar/src/codebus_agent/api/scan.py:48-49`：拆掉重複定義 `_WORKSPACE_AUDIT_SUBDIR=".codebus"` + `_SANITIZE_AUDIT_FILENAME="sanitize_audit.jsonl"`，加 `from codebus_agent._audit_paths import _WORKSPACE_AUDIT_SUBDIR, _SANITIZE_AUDIT_FILENAME`（變數名不變）
- [x] 5.2 [P] 改 `sidecar/src/codebus_agent/api/qa.py:207`：字面量 `"sanitize_audit.jsonl"` 替換為 `_SANITIZE_AUDIT_FILENAME`，import 補 `from codebus_agent._audit_paths import _SANITIZE_AUDIT_FILENAME`
- [x] 5.3 [P] 改 `sidecar/src/codebus_agent/agent/tools/folder_tools.py:118`：字面量 `"tool_audit.jsonl"` 替換為 `_TOOL_AUDIT_FILENAME`，import 補 `from codebus_agent._audit_paths import _TOOL_AUDIT_FILENAME`
- [x] 5.4 [P] 改 `sidecar/src/codebus_agent/agent/tools/folder_tools.py:473`：字面量 `"sanitize_audit.jsonl"` 替換為 `_SANITIZE_AUDIT_FILENAME`，補 import（若已 import 則合併）
- [x] 5.5 RED `sidecar/tests/test_no_jsonl_literal_drift.py::test_jsonl_literal_only_in_canonical_module`：用 `Path("sidecar/src/codebus_agent").rglob("*.py")` 走 source tree，每個檔讀 source string，正則 `r"['\"][\w_-]+\.jsonl['\"]"` 找命中；assert 命中檔案路徑只能是 `codebus_agent/_audit_paths.py`（whitelist）
- [x] 5.6 RED `test_no_jsonl_literal_drift.py::test_seven_audit_filenames_present_in_canonical`：assert `_audit_paths.py` source 內含 7 個 filename string `"sanitize_audit.jsonl"` / `"tool_audit.jsonl"` / `"token_usage.jsonl"` / `"llm_calls.jsonl"` / `"reasoning_log.jsonl"` / `"generator_log.jsonl"` / `"kb_growth.jsonl"`
- [x] 5.7 GREEN — 5.1-5.6 串聯通過 + 既有 `tests/api/test_audit_paths_kb_growth.py` + `tests/api/test_factory_audit_paths_*` 全綠

## 6. 完整驗證 + commit gate

- [x] 6.1 `uv run pytest sidecar/tests/ -m "not slow" -q` 全綠（baseline 830 → 實際 843 passed / 19 skipped，+13 新測：3 station_id + 3 qa constants + 2 jsonl drift + 5 由 spec scenario 觸發既存 path）
- [x] 6.2 `pre-commit run --all-files` 全綠
- [x] 6.3 `spectra validate audit-path-unification-stage-2 --strict` 全綠
- [x] 6.4 Grep `_STATION_ID_RE = re\.compile` 在 `sidecar/src/codebus_agent/`，命中檔案 set 只有 `codebus_agent/agent/station_id.py`
- [x] 6.5 Grep `^_QA_(MAX|DEDUP)_\w+\s*[:=]` 在 `sidecar/src/codebus_agent/`（line-anchored 定義），命中檔案 set 只有 `codebus_agent/agent/qa.py`
- [x] 6.6 Grep `['\"][\w_-]+\.jsonl['\"]` 在 `sidecar/src/codebus_agent/`，命中檔案 set 只有 `codebus_agent/_audit_paths.py`
- [x] 6.7 冒煙 import：`uv run python -c "from codebus_agent.api.qa import router; from codebus_agent.kb.knowledge_base import KnowledgeBase; from codebus_agent.agent.tools.add_to_kb import add_to_kb; from codebus_agent.agent.station_id import STATION_ID_RE; print('OK')"` 印 OK 不噴 ImportError

## 7. Documentation 連動更新

- [x] 7.1 改 `docs/reviews/2026-04-26-stage-5.md` Cat 2.5 段：4 sub-cat 全 `[x]` + 標 archive 日期；進度狀態表 Cat 2.5 row `[x]`
- [x] 7.2 改 `CLAUDE.md` archive 表加 row（audit-path-unification-stage-2 收尾）
- [x] 7.3 改 `CLAUDE.md` 「Path constants」段補一句：station_id regex 也走 leaf module pattern（`codebus_agent/agent/station_id.py`），與 `_audit_paths.py` 採同樣 single-source 模式

## 8. 規格 / 設計覆蓋錨點（apply 階段純驗證 checkbox）

- [x] 8.1 Spec coverage：sanitizer `SanitizerAuditLogger appends each replacement to JSONL` MODIFIED Scenario `Filename literal is single-sourced in canonical leaf module` 由 5.5 + 5.6 滿足
- [x] 8.2 Spec coverage：kb-growth `Required fields on every kb_growth.jsonl line` MODIFIED Scenario `Station id regex sourced from canonical leaf module` 由 1.2 + 2.3 + 2.6 滿足
- [x] 8.3 Spec coverage：knowledge-base `KnowledgeBase query and find_similar API` MODIFIED Scenario `Station id regex sourced from canonical leaf module` 由 1.2 + 2.4 + 2.6 滿足
- [x] 8.4 Spec coverage：knowledge-base `KnowledgeBase exposes upsert_chunk for Q&A add_to_kb path` MODIFIED Scenario `Dedup threshold sourced from canonical single source` 由 4.1 + 4.2 + 4.3 滿足
- [x] 8.5 Spec coverage：qa-agent `Q&A budget constants are module-level` MODIFIED Scenario `All callsites import constants from agent.qa single source` 由 3.1-3.4 + 4.1-4.3 滿足
- [x] 8.6 Spec coverage：qa-agent `add_to_kb pipeline runs sanitize, validate, upsert, growth-log in order` MODIFIED Scenario `Station id regex sourced from canonical leaf module` 由 1.2 + 2.1 + 2.6 滿足
- [x] 8.7 Design anchor：Decision 1（`agent/station_id.py` leaf module）由 1.1-1.4 落地
- [x] 8.8 Design anchor：Decision 2（identity-check defensive test）由 1.2 + 3.2 + 4.2 落地
- [x] 8.9 Design anchor：Decision 3（grep-based jsonl literal test）由 5.5 + 5.6 落地
- [x] 8.10 Design anchor：Decision 4（`_QA_DEDUP_THRESHOLD` 走 import 不另抽 leaf）由 4.1 落地
- [x] 8.11 Design anchor：Decision 5（`api/scan.py` 直接刪 alias）由 5.1 落地
