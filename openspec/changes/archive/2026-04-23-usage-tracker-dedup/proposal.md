## Problem

`POST /kb/build` 跑完後，`<workspace>/token_usage.jsonl` 裡每一個 embed call 被記錄**兩次**：

```jsonl
{"module": "",         "model": "text-embedding-3-small", "input_tokens": 27, "cost_usd": 5.4e-7}
{"module": "kb_build", "model": "text-embedding-3-small", "input_tokens": 27, "cost_usd": 5.4e-7}
```

這違反既有 spec `usage-tracking / UsageTracker writes token_usage.jsonl` 的 Scenario「One line per chat call」（embed 也適用）的「exactly one new line」契約。Cost 加總會 2x。

問題於 2026-04-22 `kb-build-production-wiring` 收尾的手動煙霧測中浮現（D-032 落地後 factory pattern 讓兩條 record 路徑寫到同一個 workspace path）。

## Root Cause

兩條 path 都在呼 `tracker.record(...)`：

1. **`TrackedProvider.embed`**（`providers/tracked.py`）自動 record，沒帶 `module`：
   ```python
   self._tracker.record(provider=..., model=..., operation="embed", input_tokens=..., cost_usd=...)
   ```
2. **`KnowledgeBase.build`**（`kb/knowledge_base.py`）手動 record，帶 `module="kb_build"`：
   ```python
   self._tracker.record(usage=response.usage, module="kb_build")
   ```

M1 / `module-2-kb-builder-p0` 時代兩條寫**不同 path**（測試用 `/tmp` fixtures），production 端 `wire_kb_dependencies`（D-032 A 方案 factory）讓兩條都拿到同一個 `<workspace>/token_usage.jsonl` path,問題才露出來。

## Proposed Solution

**TrackedProvider 成為唯一 record source**（A 方案 — 對齊 D-021 「強制所有 Provider 呼叫都走 tracker」與 M1 「all calls through TrackedProvider」不變式）：

1. **`TrackedProvider.__init__` 加 `default_module: str | None = None` 參數**：建構時綁定該 provider 隸屬的 module 名稱（KB build → `"kb_build"`、未來 qa_agent / generator 各自帶自己的）
2. **`TrackedProvider.embed` / `chat` 內 `tracker.record(...)` 加上 `module=self._default_module`**
3. **`KnowledgeBase.build` 移除手動 `self._tracker.record(...)` 那行**
4. **`KnowledgeBase.__init__` 的 `usage_tracker` 參數成為 optional 並標記 deprecated**——backward compat 保留型別槽，但 KB 內不再使用（Phase 2+ 完全移除）
5. **`wire_kb_dependencies`（`api/__init__.py`）provider factory 帶 `default_module="kb_build"`**

## Non-Goals

- **重構 `UsageTracker.record(...)` API 形狀**：本 change 只調用方加 `module` 參數，不改 tracker 本身介面（避免影響其他既有 caller）
- **移除 `KnowledgeBase.usage_tracker` 參數**：本 change 保留 backward compat（optional + deprecated），實際拆掉留 Phase 2+ 與 Module 4/5 對齊時一起做
- **Sanitizer Pass 2 / `llm_calls.jsonl` 重複問題的對應檢查**：`LLMCallLogger` 是 TrackedProvider 內單一 source，本來就沒重複，不在範圍
- **追溯 fix 既有 `token_usage.jsonl` 內已有的重複行**：歷史紀錄不動；fix 只對 change 落地後的新 build 生效
- **跨 module 統計 dashboard**：D-021 明文 Phase 2+，本 change 不碰
- **同時把 `KnowledgeBase.usage_tracker` 與 `kb_usage_tracker` factory 從 `wire_kb_dependencies` 撤掉**：上層 wiring 暫保留,直到下個 change 完整盤點 KB build 的 usage_tracker 是否還有別處用途

## Success Criteria

- `POST /kb/build` 跑完後，`<workspace>/token_usage.jsonl` 內每個 embed batch 對應**恰好一行**（不是兩行），`module: "kb_build"` 欄位仍正確
- 既有 `usage-tracking / UsageTracker writes token_usage.jsonl` 的「exactly one new line」Scenario 在 production 路徑下實際成立
- Sum cost by `model` 不再 2x
- 既有 495 自動化測試無 regression（含 `kb-build-production-wiring` Section 6 五個 production 測試）
- 新測試覆蓋：(a) TrackedProvider 帶 `default_module` 時 record 出 module 欄；(b) `KnowledgeBase.build` 完成後 tracker 只被呼叫一次每 batch；(c) `wire_kb_dependencies` 構造的 TrackedProvider 帶 `default_module="kb_build"`
- 手動煙霧測重跑同樣 3-file workspace：`token_usage.jsonl` 行數 = batch 數（非 batch 數 × 2）

## Impact

- **受影響 spec**：
  - `usage-tracking`（modify）：`UsageTracker writes token_usage.jsonl` Requirement 加 Scenario「`module` field reflects TrackedProvider's `default_module`」
  - `llm-provider`（modify）：`TrackedProvider records role in audit log` Requirement 加 Scenario「TrackedProvider records `default_module` in usage line」
- **受影響 code**：
  - `sidecar/src/codebus_agent/providers/tracked.py`（加 param + 串到 record call）
  - `sidecar/src/codebus_agent/kb/knowledge_base.py`（拿掉手動 record）
  - `sidecar/src/codebus_agent/api/__init__.py`（provider factory 帶 `default_module="kb_build"`）
- **受影響測試**：
  - `sidecar/tests/providers/test_tracked_provider.py`（既有測 update）
  - `sidecar/tests/providers/test_default_module.py`（新檔）
  - `sidecar/tests/kb/test_*.py`（依賴 `KnowledgeBase` 的 tracker 直接呼叫的測試需更新斷言）
  - `sidecar/tests/api/test_kb_build_production.py`（`test_usage_tracker_writes_to_workspace_scoped_path` 加「恰好一行 per batch」斷言）
- **受影響文件**：
  - `docs/llm-provider.md`（補 `default_module` 說明）
  - `docs/agent-core.md` / `docs/module-2-kb-builder.md`（提及 KB 不再手動 record）
- **無新依賴 / 無 env var 變動**
