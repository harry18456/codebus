## Context

`audit-path-unification`（2026-04-25 archive）建立兩個 single-source pattern：

1. **Path constants leaf module**（`codebus_agent/_audit_paths.py`）—— 7 個 filename 常數 + `_WORKSPACE_AUDIT_SUBDIR=".codebus"`，用 leaf module 避開 `api/__init__.py` ↔ `api/generate.py` ↔ `generator/runner.py` 三角 circular import。
2. **Identity-check defensive test**（`tests/sanitizer/test_rules_version_constant.py`）—— 用 `is` identity 鎖死 4 處 callsite 必同 `RULES_VERSION` Python object，drift 即時測爆。

但 `audit-path-unification` 收尾時 agent spot-check 漏掃了 4 組 callsite —— 這是 `docs/reviews/2026-04-26-stage-5.md` Cat 2.5 點出的 4 條 cross-cutting drift。

**動工時的 baseline**：
- M2 backend 全部 archive（Modules 1, 2, 4, 5, 8 P0）+ `review-2-critical-fix` 收尾 7 條 Critical（2026-04-26）
- 全 suite 830 passed / 19 skipped
- Cat 1 doc-stale 22 條已隨 doc-sync commit 收掉（2026-04-26）
- 18 個 capability spec、`_audit_paths.py` + `RULES_VERSION` single-source pattern 已就位

**為什麼現在做**：Phase 6 前端會 pin 在 audit 路徑 + station id regex 上 —— R-01 panel 直接 glob `<ws>/.codebus/*.jsonl`、frontend 也會校驗 station id。現在 7 處 drift 不收齊，Phase 6 中如果有 bump dedup threshold / 改 budget / rename audit file 會重蹈 `RULES_VERSION` 那次「漏掃 callsite」覆轍。

## Goals / Non-Goals

**Goals:**

- 4 組 cross-cutting code drift 一次性收掉（4 處 jsonl 字面量 + 6 處 station id regex + 2 處 QA budget + 2 處 dedup threshold）
- 建立第二個 leaf module `codebus_agent/agent/station_id.py`（types-only，避免 circular），確立「跨 module 共用的 regex / validator 抽到 leaf」的二級 pattern
- 補三條 grep / identity defensive test 鎖死 single-source 約束，CI 永遠擋住將來新增的 drift
- 4 個 capability spec 加 Scenario 把「callsite MUST import from canonical module」明文寫進 normative behaviour contract

**Non-Goals:**

詳 proposal Non-Goals 段。摘要：
- 不擴增 `_audit_paths.py` 的 path constants（7 個 filename 不動）
- 不重構 `_audit_paths.py` ↔ `api/_audit_paths.py` shim 關係
- 不改任何 production 行為 / wire payload / audit JSONL schema
- 不抽 station id regex 的 token format（regex 字面不變，只移位置）
- 不 refactor `_QACtxAdapter`（屬 Cat 3 backlog）
- 不留 backward-compat alias（直接刪重複 callsite）

## Decisions

### Decision 1：新建 `codebus_agent/agent/station_id.py` leaf module，與 `_audit_paths.py` 採同樣 leaf 模式

**選擇**：建立第二個 package-internal leaf module `sidecar/src/codebus_agent/agent/station_id.py`，導出 `STATION_ID_RE` + `validate_station_id` + `find_invalid_station_id` 三個 symbol。6 處 callsite 改 `from codebus_agent.agent.station_id import ...`（`agent/tools/add_to_kb.py` / `agent/tools/kb_search.py` / `kb/growth_logger.py` / `kb/knowledge_base.py` / `kb/payload.py` / `api/qa.py`）。

**對比方案 A（reject）**：把 regex 放在 `codebus_agent/agent/types.py`（既有的 Pydantic types 集合），不另開新 module。**Reject 理由**：`types.py` 已 import `Step` / `ExplorerState` / `QAState` 等 Pydantic model，會把 regex utility 混進 type module；leaf module 模式（`_audit_paths.py` 已驗證）對 import cycle 更安全。

**對比方案 B（reject）**：放在 `codebus_agent/agent/qa.py`（QA module 是 station_id pre-validation 的最早 consumer）。**Reject 理由**：`agent/qa.py` 也 import 了 `LLMProvider` / `KnowledgeBase` / `KBGrowthLogger`，重 callsite（如 `kb/knowledge_base.py:50` 的 `_validate_station_filter`）反向 import `agent.qa` 會建立 `kb → agent.qa → kb` 循環。

**理由**：
- Leaf module（最少 import：只 `import re` 與 stdlib `typing`）對 6 處跨層 callsite（`api/qa.py` / `agent/tools/*` / `kb/*`）都安全可 import
- 對齊 `_audit_paths.py` 的既有結構，新加入的 contributor 一眼就理解「跨 module 共用的 const / regex 抽到 leaf module」
- module path `codebus_agent/agent/station_id.py` 的命名與 station id 概念綁定（station id 屬 agent layer），避開放在 package root（不該被 `kb/` 直接 import root utilities）

**取捨**：未來若有 station id 相關的更複雜 logic（slug 正規化、ID generator、collision 檢查）也可進這個 module；本 change 只放 P0 必要三 symbol。

### Decision 2：Identity-check defensive test 用 `is` 鎖 `re.Pattern` 物件，不 rely on regex source 字串相等

**選擇**：`tests/agent/test_station_id_constant.py` 用：

```python
from codebus_agent.agent.station_id import STATION_ID_RE
from codebus_agent.agent.tools.add_to_kb import STATION_ID_RE as _add_to_kb_re
# ... 5 處 callsite import alias
def test_station_id_re_single_source() -> None:
    assert _add_to_kb_re is STATION_ID_RE
    # ... 6 條 is identity check
```

**對比方案 A（reject）**：用 `assert _add_to_kb_re.pattern == STATION_ID_RE.pattern` 比較 regex 字串。**Reject 理由**：identity check 才能擋 drift —— 如果有人 copy-paste 了 regex 字串到別的 module，`.pattern` 比較會通過但 single-source 已破。

**理由**：與 `tests/sanitizer/test_rules_version_constant.py` 既有模式一致；`is` identity 對 immutable Python object（`re.Pattern` 是 immutable）就是 single-source 鐵證。

**取捨**：callsite import alias 必須名字對得上（如 `from codebus_agent.agent.tools.add_to_kb import STATION_ID_RE`），所以 6 處 callsite 在重構時 module-level 不能 rename 成別的私名；rename 時要同步更新 defensive test。

### Decision 3：Grep-based jsonl 字面量 defensive test 走 source-level scan，不依賴 import graph

**選擇**：`tests/test_no_jsonl_literal_drift.py` 用 `Path.glob("**/*.py")` 走整個 `sidecar/src/codebus_agent/`，每個 `.py` 檔讀 source string，正則 `r"['\"][\w_-]+\.jsonl['\"]"` 找所有 `*.jsonl` 字面量；assert 命中的檔案路徑只能是 `codebus_agent/_audit_paths.py`。

**對比方案 A（reject）**：用 `ast.parse` 走 AST 找 `Constant(value=str_ending_with_jsonl)`。**Reject 理由**：source-level grep 簡單、零 false positive（`.jsonl` 字面在 codebus 只有 audit filename 用途）；AST 多花 setup time + 對 docstring / comment 中的字面也會誤判。

**對比方案 B（reject）**：用 `git grep -E '"[^"]+\.jsonl"' -- 'sidecar/src/'` 在 pre-commit hook 跑。**Reject 理由**：pre-commit hook 不便在 PR review 時看見 fail 訊息；test suite 內的 grep test 跑 `pytest -k jsonl` 就能驗證，CI 走的也是 pytest。

**理由**：
- pytest 是 codebus single source of CI truth
- source-level grep 對「字面量 leak」這種模式最直接
- whitelist 唯一允許檔案 `_audit_paths.py` 是 canonical source，符合「single writer」設計直覺

**取捨**：未來如果 audit filename 命名 convention 變動（例如改 `.json` 副檔名），test 也要同步調整 regex；目前 `.jsonl` 是 D-021 / D-022 / kb-growth / generator-log 全層共識，預期長期穩定。

### Decision 4：`_QA_DEDUP_THRESHOLD` 走 import from `agent.qa`，不另抽 leaf module

**選擇**：`kb/knowledge_base.py:51` 的 `_QA_DEDUP_THRESHOLD: float = 0.95` 改 `from codebus_agent.agent.qa import _QA_DEDUP_THRESHOLD`，與 5 個 `_QA_MAX_*` budget 常數同 source。

**對比方案 A（reject）**：把 `_QA_DEDUP_THRESHOLD` 抽到 `codebus_agent/agent/station_id.py`（已建立的 leaf）—— 但這個 threshold 屬 KB dedup behaviour，與 station id 無關；放錯 module。

**對比方案 B（reject）**：建第三個 leaf `codebus_agent/agent/budget.py`（QA budget constants 集中）。**Reject 理由**：5 個 `_QA_MAX_*` 常數已合理住在 `agent/qa.py` —— 它們是 QA loop 的 spec-mandated 行為錨點，QA module 是它們的 home；其他 module（`agent/tools/add_to_kb.py` 是 QA loop 的 tool callee、`kb/knowledge_base.py` 也只在 QA dedup path 用）反向 import `agent.qa` 不會造成循環（已驗證 import graph）。

**理由**：
- `agent.qa` 是 QA loop 的 spec-mandated owner，5 個 budget + dedup threshold 屬於它的 contract surface
- import 方向「`tools/add_to_kb` → `agent.qa`」與「`kb/knowledge_base` → `agent.qa`」都是 callee → owner，符合直覺
- 不為了「leaf module 數對齊」勉強拆 budget module；YAGNI

**取捨**：`agent/qa.py` import chain 較深（pulls in `instructor`、`Pydantic` model）；`kb/knowledge_base.py` import `agent.qa` 會增加 KB module 的 startup cost。實測：`agent.qa` 不 import `kb`（只 import `kb.payload` 與 `kb.growth_logger`，後者 import `agent.station_id` 是 leaf）—— 沒有循環。

### Decision 5：`api/scan.py` 的兩個重複常數直接刪、不改 alias 名

**選擇**：`api/scan.py:48-49` 的 `_WORKSPACE_AUDIT_SUBDIR` + `_SANITIZE_AUDIT_FILENAME` 兩個 module-private 常數直接刪掉，改 `from codebus_agent._audit_paths import _WORKSPACE_AUDIT_SUBDIR, _SANITIZE_AUDIT_FILENAME`。callsite 變數名不改（`_WORKSPACE_AUDIT_SUBDIR` 仍叫 `_WORKSPACE_AUDIT_SUBDIR`）。

**對比方案 A（reject）**：保留 module-private alias，改成 `_WORKSPACE_AUDIT_SUBDIR = _audit_paths._WORKSPACE_AUDIT_SUBDIR`。**Reject 理由**：alias 帶來「同名兩處」假象（`api/scan.py._WORKSPACE_AUDIT_SUBDIR` vs `_audit_paths._WORKSPACE_AUDIT_SUBDIR`），未來重構 `api/scan.py` 時可能再次認為「這是 module-private 的，可以改」走回頭路。

**理由**：
- 既有 `api/__init__.py` 的 callsite 用 `from codebus_agent._audit_paths import ...` 直接 import，沒 alias —— 對齊 codebase convention
- 直接刪比加 alias 更明確「這個常數的 home 在 `_audit_paths.py`」

**取捨**：無，純 code style 一致性。

## Risks / Trade-offs

- **Import cycle risk**：新建 `agent/station_id.py` 是 leaf module（只 stdlib import），但 6 處 callsite 跨 `api/` / `agent/tools/` / `kb/` 三層，import order 變動可能觸發 `__init__.py` 副作用。**Mitigation**：每個 callsite 改 import 後跑 `uv run python -c "import codebus_agent.api.qa"` 等冒煙 import；新增的 grep / identity test 走 `pytest tests/agent/` 也會觸發完整 import chain。

- **Test 命名衝突**：`test_qa_constants_single_source.py` 同時 cover `_QA_MAX_*` (Decision C) + `_QA_DEDUP_THRESHOLD` (Decision D)，若未來 D 加新常數會擴張 test 範圍。**Mitigation**：test 用 module-level loop 驗證 `for name in {"_QA_MAX_STEPS", ...}: assert getattr(add_to_kb, name) is getattr(qa, name)`，新加常數只需擴 set 即可。

- **`test_no_jsonl_literal_drift.py` 對 docstring / comment 字面也敏感**：例如將來有人在註解寫「`# 寫到 token_usage.jsonl`」會被 grep 命中。**Mitigation**：regex 限定 quoted string `r"['\"][\w_-]+\.jsonl['\"]"`，不抓 raw `.jsonl` 字面（comment / docstring 內的 `.jsonl` mention 通常不帶引號或在 markdown code block 內，不會誤判）；真有需要可加 `# noqa: jsonl-literal` opt-out 機制（YAGNI，先不做）。

- **Spec scenario 推廣到全 7 層的範圍 scope**：`sanitizer` capability 加 Scenario 鎖 `sanitize_audit.jsonl` filename 字面 single-source 容易，但要不要同時鎖 `tool_audit.jsonl` / `token_usage.jsonl` / 等其他 6 層？**決定**：grep-based defensive test 已涵蓋全 7 層（regex `*.jsonl` 通殺），spec 層只需在 `sanitizer` 加一條代表性 Scenario + cross-reference `_audit_paths.py` 即可，不為了「7 層 spec 各加一條」過度膨脹 spec。

- **`_QA_DEDUP_THRESHOLD` 改 import 後 module load order**：`kb/knowledge_base.py` 載入時會觸發 `agent/qa.py` 載入（pulls in `instructor`），增加 cold-start 時間。**Mitigation**：實測 import 鏈路 `agent.qa` → `agent.types` → `pydantic` 是既有 path，KB build 時 `agent.qa` 早已載入；新增的 `kb → agent.qa` 邊不會再產生新的 module load 成本（cache hit）。

## Migration Plan

無 backward-incompatible 對外影響。檢查清單：

- 既有 token_usage.jsonl / sanitize_audit.jsonl / 等 7 層 audit JSONL 檔不變（純 code 重構，audit schema / writer 行為一致）
- 既有 SSE wire / API contract 不變（沒改 endpoint / 沒改 event schema）
- 既有 KB collections 不需 re-embed（`upsert_chunk` dedup threshold 數值不變，只改 import）
- 既有 4 個 MODIFIED capability spec 加 Scenario 屬增量約束（既有 Scenario 不刪不改）
- 全 suite baseline 830 passed → 預期 ~833-835 passed（本 change 預估 ~3-5 新測 + 0 個既有測 regression）

## Open Questions

無。
