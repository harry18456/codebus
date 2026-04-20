## Why

依 D-003（Provider 抽象）與 D-012（自寫 ReAct + Instructor），LLM Provider 層目前是「一個 `chat_provider` + 一個 `embed_provider`」的平面設計（`docs/llm-provider.md` §五）。`docs/implementation-plan.md` 步驟 8 的「供應商 API 實作」在 M1 只完成 Mock 層骨架與 TrackedProvider wrapper，真 vendor adapter 尚未接上；而即將進入的 M2（步驟 10 Sanitizer Pass 2 pre-flight）與 M4（步驟 16–22 Explorer ReAct）兩條主線都會**大量直接呼叫 provider**，且不同 call site 對模型等級需求差異很大：

- Explorer ReAct / Tutorial Generator 需要強推理（Opus 等級）
- Relevance Judge 只需單句 yes/no（Haiku 等級即可）
- Q&A ReAct 介於兩者之間（Sonnet 等級）
- Embedding 是完全不同類別（`text-embedding-3-small` / bge）

若維持平面設計，Explorer 升級或 Judge 降級要動 N 個呼叫端；且 Sanitizer Pass 2 的 hook 點若先在平面 provider 上掛好、之後再改成 role 分發，Pass 2 的 provider 包裹鏈會需要重做一次。**趁 M2 Sanitizer 尚未動工，先把 role 維度加進 Provider 層最划算**。

本次改動同步記錄 D-028：vision / 多模態 capability 在 MVP 不做（Scanner 已保留圖片 metadata、Protocol 擴充為 additive），此決策的兩條連動更新（`docs/llm-provider.md §八`、`docs/module-5-generator.md`）也併入本 change 一次收掉。

## What Changes

- **新增 `ProviderRole` enum** — 列舉四個語意角色：`REASONING` / `JUDGE` / `CHAT` / `EMBED`；呼叫端不再直接抓「chat provider」，而是 `registry.get(ProviderRole.REASONING)`
- **新增 `RoleConfig` 結構** — 每個 role 綁定 `provider_id` + `model` + 預設 `temperature` / `max_tokens`；呼叫端可 override 但有安全預設
- **`config.json` schema 改為 role map**（**BREAKING**，僅影響未實作的 config loader；M1 尚未寫 config loader 所以實際不影響既有程式碼）
- **`ProviderRegistry` 升級為 role-aware** — `registry.get(role: ProviderRole)` 回該 role 對應的 Provider（已包 TrackedProvider 不變）
- **TrackedProvider wrapper 每 role 都包** — registry 實例化保證所有 role 的 provider 都經過 TrackedProvider，不變式不變
- **補記 D-028**：`docs/llm-provider.md §八` MVP 不做表加一行「Vision / 多模態 — 延後至 Phase 2，見 D-028」
- **補記 D-028**：`docs/module-5-generator.md` 圖片引用段落明寫「MVP 只 inline markdown `![]()` 相對路徑，不對圖做 LLM 解讀」
- **更新 `docs/agent-core.md`** — ReAct loop 呼叫 provider 處明寫 role（Reasoning / Judge）
- **`llm-provider` capability spec 更新** — 新增 role routing 相關 SHALL 條款

## Non-Goals

- ❌ **Capability probe enum（含 vision）** — D-028 已決，不預埋。未來真要做 vision 時於 Provider Protocol 做 additive 擴充（加 `supports_vision` 屬性 + `images` 參數）
- ❌ **動態 fallback** — rate limit / upstream error 自動切換另一家 provider，`docs/llm-provider.md §八` 已列 MVP 不做；本次也不做
- ❌ **真 vendor adapter（`ContestProvider` 等）實作** — 僅改抽象層與 registry，真 vendor 實作留給後續 change（M3 前必補）
- ❌ **OllamaProvider** — D-003 明定 Phase 2，本次不碰
- ❌ **Cost-based routing** — 依成本自動選 provider，超出 MVP 價值
- ❌ **Per-call 動態切 role** — role 由呼叫端在編寫程式碼時決定（`registry.get(ProviderRole.JUDGE)`），不做 runtime 條件判斷

## Capabilities

### New Capabilities

（無）

### Modified Capabilities

- `llm-provider`: 新增 `ProviderRole` enum、role-based registry lookup、role-level 預設參數；TrackedProvider 包裹不變式延伸至每個 role

## Impact

- **Affected specs**: `llm-provider`（修改）
- **Affected docs**:
  - `docs/llm-provider.md` §二（Protocol）、§五（Provider 選擇規則）、§八（MVP 不做表新增 vision 項）
  - `docs/agent-core.md` §五 / §十三 ReAct loop provider 呼叫處
  - `docs/module-5-generator.md` 圖片引用段落補 D-028 註記
  - `docs/decisions.md` D-003 新增「role routing 已於 2026-04-20 落地（見 llm-role-routing change）」連動註
- **Affected code**:
  - `sidecar/src/codebus_agent/providers/__init__.py`（registry 升級）
  - `sidecar/src/codebus_agent/providers/protocol.py`（新增 `ProviderRole` + 修改 Protocol）
  - `sidecar/src/codebus_agent/providers/registry.py`（role-aware lookup）
  - `sidecar/src/codebus_agent/providers/tracked.py`（每 role 包裹不變）
  - `sidecar/src/codebus_agent/providers/mock.py`（支援多 role MockProvider）
  - `sidecar/tests/providers/`（新增 role routing 測試 + registry guard 測試延伸）
- **Affected config**: 未來的 `config.json` schema（`llm.roles` map），M1 尚未寫 config loader，本 change 先定 schema
- **Dependencies**: 無新增套件
- **Breaking changes**: 無（M1 只實作 Protocol + Mock，尚未有真呼叫端依賴平面 API）
