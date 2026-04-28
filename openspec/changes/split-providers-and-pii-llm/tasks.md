## 1. PII Provider 抽象骨架（Decision 1 / 8）

- [ ] [P] 1.1 在 `sidecar/src/codebus_agent/providers/pii.py` 宣告 `PIISpan` frozen dataclass（5 欄）+ `PIIProvider` Protocol（async detect-shaped），落實 spec《PIIProvider Protocol exposes detect-shaped interface》— 對應 Decision 1: PIIProvider Protocol 採 detect-shaped、回傳 spans 與 Decision 8: 檔案位置 — providers/pii.py（不放 sanitizer/）；同時 export 到 `providers/__init__.py`
- [ ] 1.2 RED：在 `sidecar/tests/providers/test_pii_provider.py` 寫失敗測試覆蓋 spec《RuleBasedPIIProvider wraps existing default_rules》四個 scenario（default 構造、多 match 排序、empty input、async 無 IO 一拍解 resolve）；同步覆蓋 spec《MockPIIProvider supports test scripting》四個 scenario（script 控制回傳、無 script 回 empty、script 用盡回 empty、calls 記錄輸入）— 確認測試紅
- [ ] 1.3 GREEN：在 `providers/pii.py` 實作 `RuleBasedPIIProvider`（包現有 `default_rules()`、不改任何 `rule_id` / `kind`，落實 Decision 4: RuleBasedPIIProvider 包現有 default_rules()，rules 內容不動）+ `MockPIIProvider`（script 行為），讓 1.2 測試全綠

## 2. LLMProvider / EmbeddingProvider Protocol 拆分（Decision 7）

- [ ] [P] 2.1 修改 `sidecar/src/codebus_agent/providers/protocol.py`：把 `LLMProvider` Protocol 變窄（只留 `chat`，移除 `embed`）、新增 `EmbeddingProvider` Protocol（只 `embed`），兩者皆 `@runtime_checkable`，對應 spec MODIFIED《LLMProvider protocol》與 ADDED《EmbeddingProvider protocol》；驗證 `OpenAIChatProvider` / `OpenAIEmbeddingProvider` 既有實作不需改即滿足窄 Protocol，與 Decision 7: MockProvider 同時實作 LLMProvider + EmbeddingProvider，不拆兩個 Mock class 一致（`MockProvider` 同時 `isinstance` 兩個窄 Protocol）
- [ ] 2.2 RED + GREEN：在 `sidecar/tests/providers/test_protocols_narrowed.py` 寫測試覆蓋三個 ADDED scenario（Protocol declares only chat / Protocol declares only embed / MockProvider satisfies both narrowed protocols），紅綠循環

## 3. TrackedProvider PII 雙 allowlist + Mode Dispatch（Decision 2）

- [ ] 3.1 RED：在 `sidecar/tests/providers/test_tracked_pii_bypass.py` 寫失敗測試覆蓋 spec《TrackedProvider gates PII inner classes via PII_ALLOWED_INNER_TYPES》四個 scenario（初始 allowlist 內容、雙 allowlist disjoint、非 allowlisted PII inner reject、source-grep test pins allowlist to spec）+ spec《TrackedProvider auto-bypasses Pass 2 for PII inner》四個 scenario（PII mode bypasses Pass 2 / 不接受 skip_sanitizer flag / mode 一次決定不可變 / wrong-mode 呼叫 raise）— 對應 Decision 2: TrackedProvider 用 marker dispatch，不拆 TrackedPIIProvider；確認紅
- [ ] 3.2 GREEN：修改 `sidecar/src/codebus_agent/providers/tracked.py`：新增 `PII_ALLOWED_INNER_TYPES = frozenset({RuleBasedPIIProvider, MockPIIProvider})`、在 `__init__` 用 `type(inner)` 決定 `_mode`（`"llm"` / `"pii"`）、`chat` / `embed` / `detect` 各加 `_assert_mode()` guard、PII mode 完全不呼 `sanitizer.sanitize`；同時對齊 spec MODIFIED《Outbound LLM traffic gated by TrackedProvider whitelist》（disjoint allowlist + 錯誤訊息分流提示）與 MODIFIED《TrackedProvider applies Sanitizer Pass 2 before dispatch》（Pass 2 僅 LLM mode 適用）讓 3.1 測試全綠

## 4. SanitizerEngine 重構消費 PIIProvider（Decision 3 / 4）

- [ ] 4.1 RED：在 `sidecar/tests/sanitizer/test_engine_consumes_pii.py` 寫失敗測試覆蓋 spec MODIFIED《SanitizerEngine exposes pure `sanitize` interface》六個 scenario（Pass 1 async sanitize / 同值同 placeholder / index 跨 call 不洩 / fail-closed / 拒絕 legacy `rules` kwarg / engine 無直接 rule import），對應 Decision 3: PIIProvider.detect() 一律 async；確認紅
- [ ] 4.2 GREEN：refactor `sidecar/src/codebus_agent/sanitizer/engine.py`：建構式改為 `SanitizerEngine(pii_provider: PIIProvider, *, config=None)`、`sanitize` 改為 `async def`、內部 `_gather_matches` 換成 `await self._pii_provider.detect(text)`、移除 `Rule` / `default_rules` 等 import，讓 4.1 測試全綠
- [ ] 4.3 加 helper `make_default_engine() -> SanitizerEngine` 在 `sanitizer/__init__.py`：回傳 `SanitizerEngine(pii_provider=RuleBasedPIIProvider(), config=...)`，落實 spec MODIFIED《Built-in rule set covers Secret, PII, internal-identifier kinds》— 加 scenario《rule_id stability across structural change》cross-reference 測試（compare `rule_id` 與 fixture）

## 5. Pass 1 / Pass 2 / Pass 3 呼叫端 async 遷移

- [ ] 5.1 Pass 1 Scanner：把 `sidecar/src/codebus_agent/scanner/service.py` 與 `sidecar/src/codebus_agent/api/scan.py` 內所有 `engine.sanitize(...)` 改成 `await engine.sanitize(...)`；確認 sanitize call site 已在 async function 或 `asyncio.to_thread` 內；run `uv run pytest tests/api/test_scan_stream.py tests/sanitizer/` 驗證
- [ ] 5.2 Pass 2 TrackedProvider：把 `tracked.py::_sanitize_messages` / `_sanitize_texts` 改 async 並 `await sanitize(...)`；`chat` / `embed` 內 call site 加 `await`；run `uv run pytest tests/providers/test_tracked_provider.py tests/providers/test_tracked_pass2.py` 驗證
- [ ] 5.3 Pass 3 Q&A `add_to_kb`：把 `sidecar/src/codebus_agent/agent/tools/add_to_kb.py` 與 `sidecar/src/codebus_agent/api/qa.py` 內 sanitize call 改 `await`；run `uv run pytest tests/agent/tools/` 與 `tests/api/test_qa.py`（若存在）驗證

## 6. Audit Schema 與 Drift 防護（Decision 5 / 6）

- [ ] 6.1 落實 spec ADDED《AuditRole enumerates legal role values in llm_calls.jsonl》：在 `providers/llm_call_logger.py` 或 `providers/protocol.py` 宣告 `AuditRole = Literal["reasoning", "judge", "chat", "embed", "pii_detection"]`；對應 Decision 6: ProviderRole enum 不加 PII_DETECTION，audit role 走獨立 Literal；對應 Decision 5: PII 偵測 audit 共用 llm_calls.jsonl，加 role: "pii_detection"；本 change 不寫入 `pii_detection` 值（無 PII LLM provider 實作）但 schema 完成
- [ ] 6.2 寫 closed set 防護測試 `sidecar/tests/providers/test_audit_role_closed_set.py`：四個 scenario（closed set / pii pairs false / llm pairs true / this change emits no pii_detection lines）；run `uv run pytest tests/providers/test_audit_role_closed_set.py`
- [ ] 6.3 寫 PII_ALLOWED_INNER_TYPES vs spec drift 防護測試 `sidecar/tests/providers/test_pii_allowlist_drift.py`：source-grep `tracked.py` 取 `PII_ALLOWED_INNER_TYPES` 字面內容並斷言等於 `{RuleBasedPIIProvider, MockPIIProvider}`；同時斷言 `ALLOWED_INNER_TYPES` 與 `PII_ALLOWED_INNER_TYPES` disjoint；run `uv run pytest tests/providers/test_pii_allowlist_drift.py`

## 7. 既有測試對齊與既有 caller fix-up

- [ ] 7.1 修 `sidecar/tests/providers/test_tracked_provider.py` / `test_tracked_pass2.py` / `test_tracked_role_audit.py` 等既有測試，把 `TrackedProvider(MockProvider(), ...)` 構造對齊新 mode dispatch 簽名；保留既有 chat/embed 行為驗證
- [ ] 7.2 修 `sidecar/tests/sanitizer/test_engine.py` 既有測試：把 `SanitizerEngine(rules=...)` 改為 `SanitizerEngine(pii_provider=RuleBasedPIIProvider(rules=...))` 或 `make_default_engine()`；`sanitize(...)` 全改 `await sanitize(...)`
- [ ] 7.3 修 `sidecar/tests/test_sanitizer_safety_chain_integration.py` 跨子系統測試：對齊 async sanitize 與新 TrackedProvider 構造
- [ ] 7.4 跑全套 `cd sidecar && uv run pytest`：確保 ~885 baseline 全綠（除 known skip 外無 regression）

## 8. 文件對齊與 Forward-looking Hook（Decision 5 / 6）

- [ ] 8.1 對齊 `docs/llm-provider.md`：拆 `LLMProvider` / `EmbeddingProvider` 描述、補 `PIIProvider` 段落、補雙 allowlist 與 mode dispatch 語意；首段引 D-033
- [ ] 8.2 對齊 `docs/sanitizer.md`：Engine 消費 PIIProvider 的層級調整、async sanitize 影響、規則內容仍由 `RuleBasedPIIProvider` 包既有 `default_rules()` 不變
- [ ] 8.3 對齊 `CLAUDE.md` 「三段 Sanitizer」段落：補 PII LLM 例外脈絡（D-015 invariant 例外條件 = inner ∈ PII_ALLOWED_INNER_TYPES）；補 spec MODIFIED《Future LLM-based PII providers extend allowlist additively》前瞻條款的存在（未來加 `LocalLLMPIIProvider` / `OpenAIPIIDetectionProvider` 要過 `/spectra-propose`）
- [ ] 8.4 跑 `pre-commit run --all-files`、`bash tests/precommit_gate_test.sh`、`bash tests/precommit_violation_test.sh` 確認 commit gate 全綠
