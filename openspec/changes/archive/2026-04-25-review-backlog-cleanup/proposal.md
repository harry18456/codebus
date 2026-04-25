## Why

`docs/reviews/2026-04-25-stage-4.md` Cat 3 latent risk 5 條中還剩兩條沒解：**#1 `rules_version` 字串獨立寫死三處**（違反不變式 9 精神，漏改一處 → Sanitizer rule bump 時稽核鏈斷裂）+ **#4 chat `cost_usd` 永遠是 0**（`tracked.py:226,238` hardcode → `session_total_cost_usd` 只反映 embed、Trust Layer R-01 / O-04 評審看到「reasoning + judge 0 元」會起疑，是 Demo 賣點漏洞）。兩條都跟 Module 8 Q&A P0 動工無強耦合，但都該在 Stage 6 前端 Trust Layer 接上之前清掉 — Q&A 大量 reasoning call 會把 cost_usd=0 問題直接放大到 demo 介面。

不變式 9（`CLAUDE.md:131`）：「Sanitizer rules 改動必 bump version」依賴單一 source-of-truth 常數；目前三處字串各寫各的，bump 時必須三地手改才能維持稽核完整。Trust Layer Demo 賣點（`docs/decisions.md` D-022）：`token_usage.jsonl` cost_usd 是 R-01 / O-04 panel 顯示的核心指標，硬寫 0 違反「離開 sidecar 的內容都已附真實成本帳」承諾。

關聯決策：**D-021**（token_usage / cost ledger）、**D-022**（llm_calls / wire payload）、**不變式 9**（rules_version sync）、`docs/reviews/2026-04-25-stage-4.md` Cat 3 #1 + #4。

## What Changes

**A. `rules_version` 集中為單一常數**（`sanitizer` capability MODIFIED）

- 認 `sanitizer/config.py::_BUILTIN_RULES_VERSION` 為唯一 source of truth；改名 `RULES_VERSION` 並從 `codebus_agent.sanitizer` 公開 import
- `api/__init__.py:81 _RULES_VERSION` 與 `api/scan.py:55 _RULES_VERSION` 兩處字串移除，改 `from codebus_agent.sanitizer import RULES_VERSION as _RULES_VERSION`（保留私名避免 callsite 大規模 rename）
- 新 defensive test：assert 兩個 callsite 引用的常數 ID 與 sanitizer 內常數同物件（不是值相等，是 identity check）— drift guard 鎖死 convention
- spec MODIFIED：`sanitizer` capability `Sanitizer rules version stamping` Requirement 主文加「`rules_version` SHALL be a single module-level constant exported from `codebus_agent.sanitizer`」+ Scenario `Single source of truth for rules_version constant`

**B. chat `cost_usd` 算真值**（`usage-tracking` capability MODIFIED）

- 新 module `codebus_agent/providers/pricing.py`：cost lookup table（`gpt-4o-mini` `$0.15 / $0.60` per 1M token、`text-embedding-3-small` `$0.02` per 1M token，model→（input_per_1m_usd, output_per_1m_usd）映射）+ 純函式 `estimate_chat_cost_usd(model, prompt_tokens, completion_tokens) -> float`
- `tracked.py::chat()` `cost_usd=0.0` 兩處（`record(...)` 與 `_emit_usage_delta(...)`）改成 `estimate_chat_cost_usd(model_id, prompt_tokens, completion_tokens)`
- 未知 model 不在 table 時回 `0.0`（保留既有行為，避免 unknown model 硬 raise；log warning by `logger.warning("unknown chat pricing for model %s", model_id)`）
- spec MODIFIED：`usage-tracking` capability `TrackedProvider records chat cost` Requirement 加「chat cost SHALL be derived from a model→pricing table when model is known; unknown model SHALL log warning and record 0.0」+ Scenario `Known chat model writes non-zero cost_usd` + Scenario `Unknown chat model logs warning and writes 0.0`
- `usage_delta` SSE event `cost_usd` 與 `session_total_cost_usd` 自然反映非零（既有 SSE schema 不變，純值層更新）
- 既有 `chat-provider-wiring` archive 的 cost field semantic 保持，只是值不再 hardcode 0

## Non-Goals

- **Embed cost 改寫**：embed 的 cost 已經走 `result.usage.cost_usd or 0.0`（`tracked.py:278`），現況 OpenAI SDK 不回傳 cost，但邏輯上已預留 hook；本 change 不動
- **動態 pricing 同步 OpenAI API**：pricing table 是手動維護常數，不從 OpenAI API 抓最新價（Phase 2 / 真有 model 變動再說）
- **Per-call cost override**：caller 不能傳 `cost_usd=...` override；統一走 pricing table
- **Cost 警告 / budget enforcement**：本 change 只算真值寫進 audit log + emit；不擋呼叫、不觸發警告（屬未來 Trust Layer feature）
- **Cat 3 #2 FolderTools 不接 SSE emitter**：`docs/reviews/2026-04-25-stage-4.md` 標明「Stage 6 前端開鋸時若實際缺軸再決定」— 本 change 不解
- **Cat 3 #3 Judge / Coverage prompt fork**：屬 Module 8 Q&A 動工時的 prompt mode-aware 重整，併進 `module-8-qa-p0` 後續 change

## Capabilities

### New Capabilities

（none — 純 invariant 強化 + cost 算真值，不新增 capability）

### Modified Capabilities

- `sanitizer`：`Sanitizer rules version stamping` Requirement 加單一常數約束 + Scenario
- `usage-tracking`：`TrackedProvider records chat cost` Requirement 加 pricing table 行為 + 兩個 Scenario（known model / unknown model）

## Impact

**受影響 spec**：

- `openspec/specs/sanitizer/spec.md`（MODIFIED — 一條 Requirement）
- `openspec/specs/usage-tracking/spec.md`（MODIFIED — 一條 Requirement + 兩個 Scenario）

**受影響 production code（新檔）**：

- `sidecar/src/codebus_agent/providers/pricing.py`（cost lookup table + `estimate_chat_cost_usd`）

**受影響 production code（修改）**：

- `sidecar/src/codebus_agent/sanitizer/__init__.py`（公開 `RULES_VERSION`）
- `sidecar/src/codebus_agent/sanitizer/config.py`（`_BUILTIN_RULES_VERSION` → `RULES_VERSION`，保留 backward-compat alias）
- `sidecar/src/codebus_agent/api/__init__.py:81`（`_RULES_VERSION = "..."` → `from sanitizer import RULES_VERSION as _RULES_VERSION`）
- `sidecar/src/codebus_agent/api/scan.py:55`（同上）
- `sidecar/src/codebus_agent/providers/tracked.py:226,238`（chat cost hardcode 改 `estimate_chat_cost_usd(...)` 呼叫）
- `sidecar/src/codebus_agent/providers/__init__.py`（公開新 `pricing` 子模組 entry）

**受影響 docs**：

- `docs/reviews/2026-04-25-stage-4.md`（Cat 3 #1 + #4 兩條打 `[x]` + 註明本 change archive 日期）
- `docs/decisions.md` D-021 / D-022 連動清單補一條（cost_usd 真值 + rules_version 集中）
- `CLAUDE.md`（archive 表加 row）
- `docs/implementation-plan.md`（不需動 — 此 change 不在 plan 步驟編號內，是 review backlog 收尾）

**受影響 fixture / golden**：

- 既有 golden replay test（`tests/golden/test_timeline_synthetic_replay.py`）的 `usage_delta.cost_usd` 期望值從 0.0 改 `>= 0.0`（避免 pin 到具體浮點數，但反映 chat call 後該值非零）— 視 golden 是否實際斷言再決定

**受影響 tests**：

- 新 `sidecar/tests/sanitizer/test_rules_version_constant.py`：assert sanitizer / api `_RULES_VERSION` 同 identity
- 新 `sidecar/tests/providers/test_pricing.py`：`estimate_chat_cost_usd` 三 case（known model / unknown model / zero tokens）
- 既有 `sidecar/tests/providers/test_tracked.py`：擴 case 驗 chat call 後 `token_usage.jsonl` `cost_usd` 為 non-zero（具體值不 pin，僅 `> 0`）+ unknown model 案例（mock model_id 為 "fake-model" → cost 0.0 + warning）

**無新依賴**（純 in-process 計算 + 常數重用）。

**無 schema breaking change**（spec MODIFIED 只擴語意、不變欄位形狀；既有 `token_usage.jsonl` / `usage_delta` schema 不變）。

**Migration**：無 — 純內部常數集中 + cost 計算層升級，既有 audit log 既有 reader 全相容。

**估計工期**：~1d。
