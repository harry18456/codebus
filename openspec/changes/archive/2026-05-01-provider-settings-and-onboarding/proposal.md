## Why

D-033 Change A（`split-providers-and-pii-llm`，2026-04-29 archive）已把 Provider 抽象拆三介面（`LLMProvider` / `EmbeddingProvider` / `PIIProvider`）+ marker 雙 allowlist + Sanitizer 消費 PIIProvider 落地，但留下三個阻礙 demo ready 的缺口：(1) API key 仍走 env var、沒 OS keychain 安全儲存；(2) 啟動時若 provider 未配置 sidecar 走 graceful degraded 但前端**沒有引導 user 設**；(3) 使用者不能在 app 內切 provider / model — 改 provider 得編輯 yaml + 重啟。Phase 6 前端 Trust Layer 四站（R-01 / O-01 / O-04 / O-05）+ 三介入點（step 29）+ Q&A overlay（step 30）已在 2026-04-30 全部 archive，本 change 是 **D-033 雙 change 序列的 B 半**，把 setting page + onboarding wizard + Tauri keyring 補上，讓 Demo ready 條件「使用者下載 app → 跑 onboarding → 填 provider → 進主畫面開始學」端到端通電。

## What Changes

- **Tauri keyring 整合**：`tauri/src-tauri/Cargo.toml` 加 `tauri-plugin-keyring` 依賴；新增 IPC commands `keyring_set` / `keyring_get` / `keyring_delete`（key 命名空間 `codebus.<provider_id>.api_key`）；Tauri 啟動 sidecar 前從 keyring 把所有 provider keys 讀出，透過 sidecar 既有 stdin handshake 之後的 startup config IPC 注入記憶體 — 對齊 D-033 不變式 1「API key 與 bearer 同等敏感」（不寫磁碟、不入 audit、`llm_calls.jsonl` 禁止出現原值）
- **Provider pool schema**：sidecar config 從現行 `llm.roles.<role>.provider_id` 一對一，擴成 `llm.providers[]` 陣列 + `llm.bindings.<role>.provider_id` 兩層；A archive 後 Registry 仍是建構期 freeze，本 change 加 `RegistryHolder`（內層 immutable / 外層 swap reference）達成 hot-swap，不破壞 in-flight task — 兌現 D-033 不變式 6「A 不動 Registry lifecycle，mutability 留給 B」
- **Setting page `/settings`**：新路由；UI 三段（Provider 池子 CRUD / Role binding / PII 模式 rule | llm）；**Embedding 切換 destructive** — 走獨立 confirm modal，告知會 rebuild KB（不變式 4）；其他 provider / role binding 變更即時生效，sidecar 透過新 SSE event `provider_config_changed` 推送通知，in-flight task 跑完現場、下個 task 用新 binding
- **Onboarding wizard `/onboarding/{welcome|providers|done}`**：三步流程（Welcome 1 張 + Providers 1 步 chat × 1 + embed × 1 / reasoning / judge / chat 三 role 預設共用同一個 + Done 1 張）；**不允許 skip**；PII 預設 rule-based，不在 onboarding 出現（D-033 決策 6）
- **啟動偵測**：前端 `pages/index.vue` 進入時打 `/healthz`，回 `dependency: { llm: "not-configured" | "ready", embed: ... }` 任一 not-configured → `router.push('/onboarding/welcome')`；既有 healthz 接口擴 `dependency` 欄位，向後相容（舊欄位不動）
- **TopBar 入口**：`<TopBar>` 加齒輪 icon，click → `/settings`；目前 `open-settings` emit 已存在但無 listener，本 change 接通
- **新 composable `useProviderConfig()`**：`web/app/composables/`，封裝 provider pool / role bindings / PII mode 狀態 + Tauri keyring IPC 呼叫；module-level singleton 同 `useQaSession` / `useIntervention` 慣例
- **Audit panel 多 provider id 兼容**：O-04 LLM Call Inspector 預留的 `role: "pii_detection"` filter hook（D-033 §B 對前端的影響 §3）正式接通；row 顯示 provider id；filter 預設過濾 PII 偵測 call 不顯示在主 stream，banner 「另有 N 筆 PII 偵測 call」可展開

## Non-Goals (optional)

- **不做 multi-tenant API key**（每使用者一份，不分 workspace）— 設計仍允許未來擴展但 P0 不支持
- **不做 in-flight task 中斷重跑** — Embedding 切換 destructive 但 in-flight task 不強制中斷，rebuild KB 是 user explicit action（confirm modal）；Generator / Explorer 跑到一半切 chat provider 走「跑完現場 + 下個 task 生效」（D-033 §B 開放問題 1）
- **不做 master password / vault unlock** — Tauri keyring 用 OS keychain 直連，不疊 `tauri-plugin-stronghold` 多一道密碼（D-033 決策 2 對 onboarding 不友善）
- **不做 LLM PII provider 在 onboarding 內配置** — D-033 決策 6，PII rule-based 預設足夠 demo；要切 LLM PII 進 setting page 第三段
- **不做 setting page 進階配置**（temperature / max_tokens / system prompt 等）— P0 只配 provider id + model + base_url + api_key + role binding；其他留 P1+
- **不做沒桌面環境 Linux 的 keyring fallback PoC** — D-033 §B 開放問題 3 留打磨期，P0 只在 GNOME / KDE 桌面 Linux 跑 happy path

## Capabilities

### New Capabilities

- `provider-settings`: Setting page UI + provider pool CRUD + role binding + PII mode toggle + setting-side hot-swap 通知契約
- `provider-onboarding`: Onboarding wizard 三步流程 + 啟動偵測 + 不可 skip 守則
- `keyring-integration`: Tauri keyring plugin IPC commands + Tauri-to-sidecar key 注入路徑 + key naming scheme + audit 不入 keyring 不變式

### Modified Capabilities

- `sidecar-runtime`: `/healthz` 擴 `dependency` 欄位（llm / embed / pii 三 lane 各自 ready / not-configured）+ 新 SSE event `provider_config_changed` + Registry 加 `RegistryHolder` hot-swap 機制 + config schema 改 `llm.providers[]` + `llm.bindings`
- `frontend-shell`: 新 `<TopBar>` 齒輪入口 → `/settings` route 正式接通；既有 emit `open-settings` 變 router push；`pages/index.vue` 加 onboarding 偵測 redirect
- `llm-call-inspector`: 顯示 provider id；filter PII detection role；`role: "pii_detection"` 預設不在主 stream（D-033 不變式 3 落地）

## Impact

- Affected specs:
  - 新：openspec/specs/provider-settings/spec.md、openspec/specs/provider-onboarding/spec.md、openspec/specs/keyring-integration/spec.md
  - 改：openspec/specs/sidecar-runtime/spec.md、openspec/specs/frontend-shell/spec.md、openspec/specs/llm-call-inspector/spec.md
- Affected code:
  - New:
    - tauri/src-tauri/src/keyring.rs
    - tauri/src-tauri/tests/keyring_redteam.rs
    - sidecar/src/codebus_agent/providers/registry_holder.py
    - sidecar/src/codebus_agent/api/settings.py
    - sidecar/src/codebus_agent/config/provider_pool.py
    - sidecar/tests/providers/test_registry_holder.py
    - sidecar/tests/api/test_settings_endpoint.py
    - sidecar/tests/api/test_healthz_dependency.py
    - sidecar/tests/config/test_provider_pool.py
    - web/app/pages/settings.vue
    - web/app/pages/onboarding/welcome.vue
    - web/app/pages/onboarding/providers.vue
    - web/app/pages/onboarding/done.vue
    - web/app/composables/useProviderConfig.ts
    - web/app/components/settings/ProviderPoolList.vue
    - web/app/components/settings/ProviderEditModal.vue
    - web/app/components/settings/RoleBindingTable.vue
    - web/app/components/settings/PiiModeToggle.vue
    - web/app/components/settings/EmbeddingChangeConfirmModal.vue
    - web/tests/settings/useProviderConfig.spec.ts
    - web/tests/settings/ProviderPoolList.spec.ts
    - web/tests/settings/ProviderEditModal.spec.ts
    - web/tests/settings/RoleBindingTable.spec.ts
    - web/tests/settings/EmbeddingChangeConfirmModal.spec.ts
    - web/tests/onboarding/welcome.spec.ts
    - web/tests/onboarding/providers.spec.ts
    - web/tests/onboarding/onboarding-redirect.spec.ts
    - design/v1/onboarding-welcome.html
    - design/v1/onboarding-providers.html
    - design/v1/onboarding-done.html
    - design/v1/setting-page.html
  - Modified:
    - tauri/src-tauri/Cargo.toml
    - tauri/src-tauri/src/lib.rs
    - tauri/src-tauri/src/sidecar.rs
    - sidecar/src/codebus_agent/api/__init__.py
    - sidecar/src/codebus_agent/api/healthz.py
    - sidecar/src/codebus_agent/providers/registry.py
    - sidecar/src/codebus_agent/providers/__init__.py
    - web/app/components/layout/TopBar.vue
    - web/app/pages/index.vue
    - web/app/components/audit/LlmCallInspector.vue
    - docs/decisions.md
    - docs/implementation-plan.md
    - docs/llm-provider.md
    - docs/authorization.md
    - CLAUDE.md
  - Removed:（無）
- Affected docs:
  - docs/decisions.md（D-033 加追記「[x] `provider-settings-and-onboarding` archive」+ 三開放問題收尾）
  - docs/implementation-plan.md（Phase 7 加 step「D-033 B」標 ✅ 或新編號）
  - docs/llm-provider.md（Provider pool schema + Registry hot-swap）
  - docs/authorization.md §六（PII LLM 模式對 rules version 的影響）
  - CLAUDE.md（Setting / Onboarding 啟動流程段）
