## 1. Sanitizer rules_version 集中常數（Cat 3 #1）

對應 spec MODIFIED Requirement `Rules version is recorded on every audit line`（新 Scenario `Single source of truth for rules_version constant`）。

- [x] 1.1 改 `sidecar/src/codebus_agent/sanitizer/config.py`：把 `_BUILTIN_RULES_VERSION` 改名 `RULES_VERSION`（公開常數），保留 `_BUILTIN_RULES_VERSION = RULES_VERSION` backward-compat alias 一行（避免 `SanitizerConfig.from_defaults()` 的既有引用變動）
- [x] 1.2 改 `sidecar/src/codebus_agent/sanitizer/__init__.py`：在 `__all__` 加 `"RULES_VERSION"` + 新 `from .config import RULES_VERSION` re-export
- [x] 1.3 [P] 改 `sidecar/src/codebus_agent/api/__init__.py:81`：刪 `_RULES_VERSION = "2026-04-20-1"` 字面量，改 `from codebus_agent.sanitizer import RULES_VERSION as _RULES_VERSION`（保留 `_RULES_VERSION` 私名讓 `_make_provider_factory` / `_make_chat_provider_factory` 既有 callsite 不變）；同步移除 L78-80 「Kept in sync ...」comment（單一來源已成立）
- [x] 1.4 [P] 改 `sidecar/src/codebus_agent/api/scan.py:55`：同上手法（`_RULES_VERSION` import alias），移除 docstring / comment 中提及多處同步的句子
- [x] 1.5 RED `sidecar/tests/sanitizer/test_rules_version_constant.py::test_rules_version_constant_has_single_source`：assert `codebus_agent.sanitizer.RULES_VERSION is codebus_agent.api._RULES_VERSION`（identity check, 用 `is`）+ `is codebus_agent.api.scan._RULES_VERSION`；reset path：sanitizer 的 `_BUILTIN_RULES_VERSION` 也 `is RULES_VERSION`
- [x] 1.6 GREEN — `Rules version is recorded on every audit line` Requirement 主文 + 新 Scenario `Single source of truth for rules_version constant` 由 1.1-1.5 共同滿足；確認 1.5 identity check passes
- [x] 1.7 跑 `uv run pytest sidecar/tests/sanitizer/ -q` 既有測 + 新測全綠

## 2. Pricing module — chat cost lookup table（Cat 3 #4 prep）

對應 spec MODIFIED Requirement `UsageTracker writes token_usage.jsonl`（新 Scenario `Known chat model writes non-zero cost_usd` / `Unknown chat model logs warning and writes zero cost_usd`）。

- [x] 2.1 [P] RED `sidecar/tests/providers/test_pricing.py::test_known_model_returns_non_zero_cost`：呼叫 `estimate_chat_cost_usd("gpt-4o-mini-chat-v1", prompt_tokens=1000, completion_tokens=500)` → 預期非零、等於 `1000 * 0.15 / 1e6 + 500 * 0.60 / 1e6`（手算 0.00045）
- [x] 2.2 [P] RED `test_pricing.py::test_unknown_model_returns_zero_and_warns`：未知 model id（如 `"fake-model"`）→ 回 `0.0` + `caplog` 抓到 WARNING-level 含 model id 的訊息
- [x] 2.3 [P] RED `test_pricing.py::test_zero_tokens_returns_zero`：`prompt_tokens=0, completion_tokens=0` → 回 `0.0`、無 warning（即便 model unknown 也不該為了 zero token 噴 warning — log only when actually missing pricing）
- [x] 2.4 GREEN — 實作 `sidecar/src/codebus_agent/providers/pricing.py`：常數 `_CHAT_PRICING: dict[str, tuple[float, float]]`（key 為 inner-provider 報出的 model id 如 `"gpt-4o-mini-chat-v1"`、value 為 `(input_per_1m_usd, output_per_1m_usd)`）+ `estimate_chat_cost_usd(model, prompt_tokens, completion_tokens) -> float` 純函式 + module-level `logger = logging.getLogger(__name__)`；未知 model 在 `prompt_tokens + completion_tokens > 0` 時 emit WARNING（避免 zero-token 的 healthz probe 被刷 warning）
- [x] 2.5 GREEN：在 `sidecar/src/codebus_agent/providers/__init__.py` 補 `from .pricing import estimate_chat_cost_usd` re-export + `__all__` 加 `"estimate_chat_cost_usd"`
- [x] 2.6 跑 3 測全綠

## 3. TrackedProvider chat cost wired through pricing（Cat 3 #4）

對應 spec MODIFIED Requirement `UsageTracker writes token_usage.jsonl`（前述兩個新 Scenario）。

- [x] 3.1 RED 擴 `sidecar/tests/providers/test_tracked.py`（或新 `test_tracked_chat_cost.py`）：`test_chat_call_writes_non_zero_cost_usd_for_known_model` — 用 `MockProvider(role=CHAT)` + `model_id` mock 成 known 值（讓 `_chat_model_id` 回 `"mock-chat-v1"` 不在 table，所以需要先把 `mock-chat-v1` 加進 `_CHAT_PRICING` 為某個非零 placeholder 或在測試裡用 monkeypatch 注入）→ 跑 `provider.chat(...)` 後讀 `token_usage.jsonl` 該行 `cost_usd > 0` + 對應 `usage_delta` event `cost_usd` 與其相等
- [x] 3.2 RED `test_chat_call_writes_zero_cost_usd_for_unknown_model_and_warns`：MockProvider 走預設 `mock-chat-v1` model id（已加進 table）反向操作 — monkeypatch 把 `_CHAT_PRICING` 清空，再跑 chat，斷言 `cost_usd=0.0` + caplog 抓 WARNING
- [x] 3.3 GREEN — `UsageTracker writes token_usage.jsonl` Requirement 新 Scenario `Known chat model writes non-zero cost_usd` 落地：改 `sidecar/src/codebus_agent/providers/tracked.py`，頂層 import `from .pricing import estimate_chat_cost_usd`；`chat()` 內 `cost_usd_for_chat = estimate_chat_cost_usd(model_id, prompt_tokens, completion_tokens)` 計一次（不重複算），把 `_tracker.record(cost_usd=...)` 與 `_emit_usage_delta(cost_usd=...)` 兩處 `0.0` 改成同一個變數
- [x] 3.4 GREEN：把 `mock-chat-v1` 加進 `_CHAT_PRICING` 用占位價（如 `(0.0, 0.0)` 表 mock 不收錢，但 key 存在所以不噴 warning）— 確保既有不期望 warning 的測試（28 個 providers + agent + api 既有測）不被新 code path 噴 log 污染
- [x] 3.5 跑 `uv run pytest sidecar/tests/providers/ sidecar/tests/agent/ sidecar/tests/api/ -q` 全綠（確認 cost 改變沒打破既有 budget probe / golden replay 等斷言）

## 4. Golden replay 期望值對齊（防回歸）

- [x] 4.1 跑 `uv run pytest sidecar/tests/golden/ -q` — 觀察 `usage_delta.cost_usd` 斷言是否 hardcode `== 0.0`（若有，調整為 `>= 0.0` 或忽略 cost 值；若已是寬鬆斷言則不動）
- [x] 4.2 若有需調整：改 `sidecar/tests/golden/test_timeline_synthetic_replay.py` 的 cost_usd 期望（具體改法視 4.1 結果而定）— N/A，golden 無 `cost_usd` 斷言
- [x] 4.3 若無調整需求：保留 4.2 為 N/A，於 commit message 記「golden cost_usd assertions already lenient — no change needed」

## 5. Documentation 連動更新

- [x] 5.1 改 `docs/reviews/2026-04-25-stage-4.md` Cat 3 #1 與 #4 兩條從 `[ ]` → `[x]`，加註 `(review-backlog-cleanup, 2026-04-25)` 等 archive 後再補日期
- [x] 5.2 改 `docs/decisions.md` D-021 後續清單補一條 `[x] chat cost_usd 走 pricing table（review-backlog-cleanup）`、D-022 同步補一條對應 wire payload 裡 `cost_usd` 反映真值
- [x] 5.3 `CLAUDE.md` archive 表加 row（待 archive 後 work commit 一起做，archive date placeholder）

## 6. 完整驗證 + commit gate

- [x] 6.1 `uv run pytest sidecar/tests/ -q` 完整 suite 全綠（baseline 751 passed → 預期 ~756+ passed 含本 change 新測）— 實際 756 passed / 19 skipped；唯一 fail `tests/test_main_run.py::test_startup_remains_available_when_qdrant_unreachable` 是 Windows handshake timing flake（baseline 3.92s vs 3.0s budget，本 change 改動為 0；已於 baseline stash 重現），非本 change 回歸
- [x] 6.2 `pre-commit run --all-files` 全綠
- [x] 6.3 `spectra validate --strict` 整個 change 合法
- [x] 6.4 Grep `"2026-04-20-1"` 在 `sidecar/src/` 下確認只剩 `sanitizer/config.py` 一處字面量（其他兩處應已 import）— 實際 grep 揪出 5 處：除了 task 1.3/1.4 列的 `api/__init__.py:81` + `api/scan.py:55`，還有 `scanner/service.py:75` / `generator/runner.py:53` / `agent/tools/folder_tools.py:56` 三處同樣 hardcode；全改 `from codebus_agent.sanitizer import RULES_VERSION as <alias>`，identity test 擴展到鎖死全 5 callsite + sanitizer config，重跑後僅 `sanitizer/config.py:26 RULES_VERSION = "2026-04-20-1"` 留唯一字面量
- [x] 6.5 確認 `sanitizer` 與 `usage-tracking` 兩個 capability 的 MODIFIED Requirement 都有對應 production code + test，0 spec drift — `sanitizer` MODIFIED `Rules version is recorded on every audit line` 主文加單一常數約束 + Scenario `Single source of truth for rules_version constant` 由 `sanitizer/__init__.py::RULES_VERSION` re-export + 5 callsite import + `tests/sanitizer/test_rules_version_constant.py` identity check 共同滿足；`usage-tracking` MODIFIED `UsageTracker writes token_usage.jsonl` 主文加 pricing-table 行為 + 兩個新 Scenario（known / unknown chat model）由 `providers/pricing.py::estimate_chat_cost_usd` + `tracked.py::chat()` 接通 + `tests/providers/test_pricing.py` 3 測 + `tests/providers/test_tracked_chat_cost.py` 2 測共同滿足
