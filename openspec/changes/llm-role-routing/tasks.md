## 1. ProviderRole 與 RoleConfig 型別（TDD）— 對應 design「ProviderRole 用四值 enum（不含 vision / multimodal 維度）」與「RoleConfig 欄位與預設值」

- [x] 1.1 先寫 test：`sidecar/tests/providers/test_role_enum.py` — 涵蓋 spec「ProviderRole enumerates call-site categories」兩個 scenario（ProviderRole 用四值 enum 四成員存在、StrEnum 相等性）
- [x] 1.2 [P] 先寫 test：`sidecar/tests/providers/test_role_config.py` — 涵蓋 spec「RoleConfig binds provider, model, and default parameters per role」兩個 scenario（RoleConfig 欄位與預設值、frozen dataclass）
- [x] 1.3 在 `sidecar/src/codebus_agent/providers/protocol.py` 新增 `ProviderRole` `StrEnum` 四成員（REASONING / JUDGE / CHAT / EMBED），對應 design「ProviderRole 用四值 enum（不含 vision / multimodal 維度）」
- [x] 1.4 在 `sidecar/src/codebus_agent/providers/protocol.py` 新增 `RoleConfig` frozen dataclass，實作 design「RoleConfig 欄位與預設值」決定的 `temperature=0.2` / `max_tokens=None` 預設
- [x] 1.5 執行 `uv run pytest tests/providers/test_role_enum.py tests/providers/test_role_config.py` 驗證 1.1 / 1.2 全綠

## 2. Registry 升級為 role-aware（TDD）— 對應 design「Registry 仍維持實例化階段 guard、不在 runtime 檢查」

- [x] 2.1 先寫 test：`sidecar/tests/providers/test_registry_role_dispatch.py` — 涵蓋 spec「Registry dispatches provider by role」兩個 scenario（role-specific 分發、missing role raise）
- [x] 2.2 [P] 先寫 test：`sidecar/tests/providers/test_registry_guard_roles.py` — 涵蓋 spec「Registry enforces TrackedProvider wrapping per role」兩個 scenario（unwrapped 任一 role raise、全 wrapped 成功），驗證 design「Registry 仍維持實例化階段 guard、不在 runtime 檢查」
- [x] 2.3 修改 `sidecar/src/codebus_agent/providers/registry.py`：`ProviderRegistry.__init__` 改接 `dict[ProviderRole, LLMProvider]`、`get(role)` 方法；依 design「Registry 仍維持實例化階段 guard、不在 runtime 檢查」把 TrackedProvider 包裹檢查延伸到每 role、runtime 不再檢查
- [x] 2.4 執行 `uv run pytest tests/providers/test_registry_role_dispatch.py tests/providers/test_registry_guard_roles.py` 驗證 2.1 / 2.2 全綠

## 3. TrackedProvider role 綁定 + 稽核 log 擴充（TDD）— 對應 design「TrackedProvider 自動感知 role（不改呼叫端簽章）」

- [x] 3.1 先寫 test：`sidecar/tests/providers/test_tracked_role_audit.py` — 涵蓋 spec「TrackedProvider records role in audit log」兩個 scenario（role 欄位寫入、additive 不破壞舊 schema），驗證 design「TrackedProvider 自動感知 role（不改呼叫端簽章）」
- [x] 3.2 修改 `sidecar/src/codebus_agent/providers/tracked.py`：依 design「TrackedProvider 自動感知 role（不改呼叫端簽章）」`TrackedProvider.__init__` 新增 `role: ProviderRole` 參數；包裹的 chat / embed / chat_structured / chat_stream 呼叫均向 `LLMCallLogger` 傳入 role，呼叫端簽章不變
- [x] 3.3 修改 `sidecar/src/codebus_agent/providers/llm_call_logger.py`（或等價位置）：audit record 的 dict 新增 `role` 欄位；確認其餘 M1 欄位（timestamp / provider_id / model / sanitizer_pass2_applied / prompt_tokens / completion_tokens）不變
- [x] 3.4 執行 `uv run pytest tests/providers/test_tracked_role_audit.py` 驗證 3.1 全綠

## 4. MockProvider 支援多 role 識別（TDD）— 對應 design「MockProvider 支援多 role 識別」

- [x] 4.1 先寫 test：`sidecar/tests/providers/test_mock_role_awareness.py` — 涵蓋 spec「MockProvider records role for audit reachability」兩個 scenario（role attribute、未設 role 時 backward compatible），驗證 design「MockProvider 支援多 role 識別」
- [x] 4.2 修改 `sidecar/src/codebus_agent/providers/mock.py`：依 design「MockProvider 支援多 role 識別」`MockProvider.__init__` 新增 `role: ProviderRole | None = None`
- [x] 4.3 執行 `uv run pytest tests/providers/test_mock_role_awareness.py` 驗證 4.1 全綠
- [x] 4.4 回歸跑 M1 既有的 `sidecar/tests/providers/` 整個目錄，確認「Mock chat satisfies response_model」「Mock script controls output」「Mock embed returns deterministic vector」三個 M1 scenario 仍綠

## 5. Config schema 定義（TDD）— 對應 design「Config schema 採 role map，保留 `llm_disabled` kill switch」

- [x] 5.1 先寫 test：`sidecar/tests/providers/test_role_config_schema.py` — 涵蓋 spec「Config schema declares llm.roles map」兩個 scenario（解析成 RoleConfig、rejects unknown role key），驗證 design「Config schema 採 role map，保留 `llm_disabled` kill switch」
- [x] 5.2 在 `sidecar/src/codebus_agent/providers/config.py`（新檔）依 design「Config schema 採 role map，保留 `llm_disabled` kill switch」新增 `parse_llm_roles(config: dict) -> dict[ProviderRole, RoleConfig]` 函式；校驗未知 role key 時 raise `ValueError` 列出四個合法 role；`llm_disabled` 旗標在 schema 中保留
- [x] 5.3 執行 `uv run pytest tests/providers/test_role_config_schema.py` 驗證 5.1 全綠

## 6. Zero outbound 不變式延伸（TDD）

- [x] 6.1 先寫 test：`sidecar/tests/providers/test_no_outbound_per_role.py` — 涵蓋修改後的「No outbound LLM traffic during M1」兩個 scenario（每 role underlying 必為 MockProvider、integration 測試 respx/socket patch 零外呼）
- [x] 6.2 跑 `uv run pytest tests/providers/test_no_outbound_per_role.py` 確認零外呼不變式 scenarios 全綠

## 7. Doc 連動更新（含 D-028 補記）

- [x] 7.1 更新 `docs/llm-provider.md` §二：Protocol 段落加 `ProviderRole` enum、`RoleConfig` dataclass 完整範例
- [x] 7.2 [P] 更新 `docs/llm-provider.md` §五：config 範例改為 `llm.roles` map 格式，移除舊 `chat_provider` / `embed_provider` 寫法
- [x] 7.3 [P] 更新 `docs/llm-provider.md` §八：MVP 不做表新增「Vision / 多模態 — 延後至 Phase 2，見 D-028」行
- [x] 7.4 [P] 更新 `docs/module-5-generator.md` 圖片引用段落：明寫「MVP 只 inline markdown `![]()` 相對路徑，不對圖做 LLM 解讀（見 D-028）」
- [x] 7.5 [P] 更新 `docs/agent-core.md` §五 / §十三：ReAct loop 呼叫 provider 處改寫為 `registry.get(ProviderRole.REASONING)` / `registry.get(ProviderRole.JUDGE)`
- [x] 7.6 [P] 更新 `docs/decisions.md` D-003 附註：補「role routing 已於 2026-04-20 落地（見 llm-role-routing change）」一行

## 8. Integration smoke + commit

- [x] 8.1 執行 `uv run pytest` 全測回歸，確認 M1 既有 ~94 個 test 仍全綠（扣除 Qdrant / symlink auto-skip 者）
- [x] 8.2 執行 `pre-commit run --all-files` 確認無 lint / format 違規
- [ ] 8.3 commit 本次改動，訊息格式：`refactor(providers): add ProviderRole routing and role-aware registry`（另一 commit 文件：`docs(providers): update llm-provider and module-5 for role routing + D-028`）
