## Why

D-033 已敲定方向：M2 wire 完 OpenAI 後，Provider 抽象層浮現三個結構性問題，本 change 解前兩個（第三個由後續 `provider-settings-and-onboarding` change 處理）：

1. **單一 Protocol 形同虛設**：現行 `LLMProvider` Protocol 同時宣告 `chat()` 與 `embed()`，但實作上 `OpenAIChatProvider` 沒有 `embed()` 方法、`OpenAIEmbeddingProvider` 沒有 `chat()` 方法 — `@runtime_checkable` isinstance 不可用，type narrowing 在 caller 端要靠 `ProviderRole` 旁路推斷
2. **未來 LLM-based PII 偵測沒有 spec 級的介面**：Sanitizer 目前純 rule-based（`RegexRule` + `DetectSecretsRule`），未來要加「本機 LLM PII 偵測」就會撞上 D-015「LLM 看到的一定是 sanitize 過的」不變式 — 需要正規破口，避免後續臨時開 `skip_sanitizer=True` 旗標被濫用

A 純後端、不動 Registry lifecycle、不動 Sanitizer rules 內容、不引入具體 LLM PII 服務 — 只立**介面骨架 + 預設 RuleBasedPIIProvider**，讓未來加 LLM PII provider 是 additive 改動。

## What Changes

- **拆 Protocol 為兩個窄介面**：`LLMProvider`（只有 `chat`，chat-shaped）/ `EmbeddingProvider`（只有 `embed`，embed-shaped）；新增 `PIIProvider`（detect-shaped，回傳 PII spans）— **BREAKING**：caller 從原 union Protocol 換到對應窄介面，既有 type hint / mock / 測試需對齊
- **TrackedProvider 雙 allowlist**：`ALLOWED_INNER_TYPES`（LLM / Embedding 既有 allowlist 不變）+ 新增 `PII_ALLOWED_INNER_TYPES`（PII 專用）；marker 模式 — TrackedProvider 看到 inner 是 PIIProvider 就走 PII 分派路徑，自動 bypass Pass 2，外部沒有 `skip_sanitizer` flag 可開
- **SanitizerEngine 消費 PIIProvider**：Engine 仍管 placeholder 編號 + audit 寫入；PIIProvider 只回傳 `(rule_id, kind, span)` 陣列。既有 `default_rules()` 規則內容**不變**，包進新的 `RuleBasedPIIProvider`（沿用現有 RegexRule + DetectSecretsRule）
- **新增 audit 欄位 / 合法值**：`llm_calls.jsonl` + `token_usage.jsonl` 在 PII 偵測 call 寫 `role: "pii_detection"` + `sanitizer_pass2_applied: false`（兩欄都是 additive，既有 consumer 不需改 schema；`sanitizer_pass2_applied` 從「永遠 true」放寬為「PII detection 例外」）
- **新增 MockPIIProvider**：給測試用，可以 script 預期回傳的 spans，對齊 MockProvider 的 pattern
- **不引入具體 LLM PII 服務**：`LocalLLMPIIProvider` / `OpenAIPIIDetectionProvider` 留給後續 change 各自 propose；本 change 只立 Protocol + RuleBased + Mock，PII_ALLOWED_INNER_TYPES 初始為 `{RuleBasedPIIProvider, MockPIIProvider}`
- **文件對齊**：`docs/llm-provider.md`、`docs/sanitizer.md`、`CLAUDE.md` 「三段 Sanitizer」段落補新介面與 PII 例外脈絡

## Capabilities

### New Capabilities

- `pii-provider`: PIIProvider Protocol（detect-shaped）+ RuleBasedPIIProvider（包現有 sanitizer rules）+ MockPIIProvider（測試用）+ TrackedProvider PII 雙 allowlist 機制 + 未來 LocalLLMPIIProvider / OpenAIPIIDetectionProvider 的擴充契約

### Modified Capabilities

- `llm-provider`: 將原 union Protocol 拆為 `LLMProvider`（chat-shaped）+ `EmbeddingProvider`（embed-shaped）兩個 Requirement；TrackedProvider 增 `PII_ALLOWED_INNER_TYPES` 雙 allowlist Requirement；audit 紀錄新增 `role: "pii_detection"` 字串值的合法性
- `sanitizer`: SanitizerEngine 改為消費 PIIProvider Protocol；既有 rules 內容不變，只是包裝層級調整（Engine 不再直接持有 `Rule` list，改持有 PIIProvider）
- `usage-tracking`: `llm_calls.jsonl` 新增 `role: "pii_detection"` 合法值與 `sanitizer_pass2_applied: false` 的合法用法（既有「永遠 true」Requirement 改為「PII detection 例外」）

## Impact

- Affected specs:
  - 新增：openspec/specs/pii-provider/spec.md
  - 修改：openspec/specs/llm-provider/spec.md
  - 修改：openspec/specs/sanitizer/spec.md
  - 修改：openspec/specs/usage-tracking/spec.md
- Affected code:
  - 新增：
    - sidecar/src/codebus_agent/providers/pii.py
    - sidecar/tests/providers/test_pii_provider.py
    - sidecar/tests/providers/test_tracked_pii_bypass.py
    - sidecar/tests/sanitizer/test_engine_consumes_pii.py
  - 修改：
    - sidecar/src/codebus_agent/providers/protocol.py
    - sidecar/src/codebus_agent/providers/tracked.py
    - sidecar/src/codebus_agent/providers/__init__.py
    - sidecar/src/codebus_agent/providers/llm_call_logger.py
    - sidecar/src/codebus_agent/providers/usage_tracker.py
    - sidecar/src/codebus_agent/providers/mock.py
    - sidecar/src/codebus_agent/providers/openai_chat.py
    - sidecar/src/codebus_agent/providers/openai_embedding.py
    - sidecar/src/codebus_agent/sanitizer/engine.py
    - sidecar/src/codebus_agent/sanitizer/__init__.py
    - sidecar/tests/providers/test_tracked_provider.py
    - sidecar/tests/sanitizer/test_engine.py
    - docs/llm-provider.md
    - docs/sanitizer.md
    - CLAUDE.md
- Affected dependencies / runtime: 無新增 Python 依賴（純結構重整）；無新增外部服務；無 schema migration（既有 jsonl 格式 additive，不需資料遷移）
- Affected D-XXX：D-033（本 change A 對應）、D-015 不變式增「PIIProvider 例外」、D-022（llm_calls.jsonl schema additive）
