## 1. Tauri keyring PoC（Decision 1: Tauri keyring plugin 選型 — `tauri-plugin-keyring`（OS 直連））

- [x] 1.1 PoC: 在 `tauri/src-tauri/Cargo.toml` 加 `tauri-plugin-keyring`（或直接 `keyring-rs` crate，視 ecosystem 成熟度），三平台 happy path（macOS Keychain / Windows Credential Manager / GNOME Keyring）寫 + 讀 + 刪一輪 sentinel 值；PoC 結果寫進 `docs/decisions.md` D-033 追記決定的選型
- [x] 1.2 RED test：`tauri/src-tauri/tests/keyring_redteam.rs` — 紅隊 14 case：`provider_id` 含 `..` / 換行 / 空字串 / null byte / 超長 / unicode 控制字元 / shell metachar `$( )` / path separator `/` `\` / windows reserved name `CON` / 等；每案斷言 `keyring_set` 拒回 `KEYRING_INVALID_PROVIDER_ID` 不寫 OS keychain（兌現 spec Requirement「Tauri keyring plugin commands」`provider_id` regex `^[a-z][a-z0-9-]{2,40}$`）
- [x] 1.3 GREEN：`tauri/src-tauri/src/keyring.rs` 實作三 IPC commands `keyring_set` / `keyring_get` / `keyring_delete`；`provider_id` 通過 regex 後 host-side 拼 `codebus.<provider_id>.api_key` 命名空間；`keyring.rs` 註冊到 `lib.rs` 的 builder。1.2 全綠
- [x] 1.4 RED test：`tauri/src-tauri/tests/keyring_redteam.rs` 補 happy path 4 case — set→get 回讀同值、set 後 delete 後 get 回 `KEYRING_ENTRY_MISSING`、delete 從未 set 的 id 回 success、set 多個 id 互不干擾（兌現 spec Requirement「Tauri keyring plugin commands」四 scenario）
- [x] 1.5 GREEN：補完 `keyring.rs` 處理 `KEYRING_ENTRY_MISSING` 與成功 case；三平台 CI 全綠

## 2. Sidecar startup config IPC（Decision 2: Sidecar key 注入機制 — startup config IPC（不打 stdin handshake））

- [x] 2.1 RED test：`sidecar/tests/api/test_startup_config.py` — pytest 測 `POST /internal/startup-config` 在 valid bearer + 正確 body schema 下回 204 並寫進 `app.state.provider_keys`；第二次叫拒回 409 `STARTUP_ALREADY_CONFIGURED`；無 bearer 401；endpoint 不在 `/openapi.json` paths 列表（兌現 spec Requirement「Tauri-to-sidecar startup key injection」四 scenario）
- [x] 2.2 GREEN sidecar：在 `sidecar/src/codebus_agent/api/__init__.py` 加 `internal_router`；`internal/startup-config.py` 處理 endpoint，`include_in_schema=False`，setattr `app.state.provider_keys` dict，已配置 flag 防第二次。2.1 全綠
- [x] 2.3 RED test：`sidecar/tests/api/test_startup_config.py` 補 secret-leak 測 — 用 sentinel api_key 經 startup-config 灌入 → 跑一次 LLM call → grep `<workspace>/.codebus/*.jsonl` 與 sidecar stdout / stderr / FastAPI access log，sentinel 值零匹配（兌現 spec Requirement「API keys never written to disk or audit logs」三 scenario）
- [x] 2.4 GREEN sidecar：稽核 LLM call 路徑（`TrackedProvider` / `LLMCallLogger` / `UsageTracker` / SSE error dispatch），確認沒任何 path 把 api_key 寫出；2.3 全綠
- [x] 2.5 改 `tauri/src-tauri/src/sidecar.rs`：sidecar handshake 之後，host 從 keyring 撈所有 `llm.providers[].id` 對應的 api_key，用 bearer 打 `POST /internal/startup-config` 推進 sidecar；失敗就 retry 一次後給使用者明顯 error banner

## 3. RegistryHolder hot-swap（Decision 3: Registry hot-swap — `RegistryHolder` 雙層引用 + SSE 推送）

- [x] 3.1 RED test：[P] `sidecar/tests/providers/test_registry_holder.py` — pytest 測 `holder.current()` 兩次連叫回同 instance（identity）、`swap(new_registry)` 後 `holder.current()` 回新 registry、in-flight task 已抓的 reference 不受 swap 影響（concurrent 模擬）、N concurrent reads + 1 swap 全部回老 OR 新（不會 partial）（兌現 spec Requirement「RegistryHolder enables atomic registry hot-swap」三 scenario）
- [x] 3.2 GREEN sidecar：[P] `sidecar/src/codebus_agent/providers/registry_holder.py` 實作 `RegistryHolder` 包 `asyncio.Lock`、`current()` async、`swap()` async；在 `providers/__init__.py` 匯出。3.1 全綠
- [x] 3.3 GREEN sidecar：把所有 `app.state.providers` 取用點（grep `app.state.providers`）改成透過 `holder.current()`；既有 ~885 pytest 必須仍綠；任一不變 case 拒做 mutation
- [x] 3.4 RED test：[P] `sidecar/tests/api/test_settings_endpoint.py` 加 `provider_config_changed` SSE event 測 — `PUT /settings/bindings` 改兩個 role → SSE app channel 收到一個 event with `data.changed_roles` 為 union（順序不敏感）；改 embed 加 `embed_changed: True`；event payload 不含 api_key 或 `~/.codebus/` 路徑（兌現 spec Requirement「provider_config_changed SSE event surface」三 scenario）
- [x] 3.5 GREEN sidecar：實作 app-level SSE channel `GET /events?channel=app`（bearer 守，沒有 task_id 綁定）；mutation endpoints 觸發後 emit `provider_config_changed` event；50ms 內多次 mutation 合併單 event。3.4 全綠

## 4. Provider pool config schema（Decision 4: Provider pool schema — `llm.providers[]` 陣列 + `llm.bindings`）

- [x] 4.1 RED test：[P] `sidecar/tests/config/test_provider_pool.py` — pytest 測新 schema（`[[llm.providers]]` + `[llm.bindings]`）載入後 in-memory 對應；legacy schema（`[llm.roles]`）載入轉成新格式 + 1 deprecation warning；binding 引用不存在的 provider id 拒 `INVALID_PROVIDER_BINDING`；embed binding 指 chat-typed provider 拒 `INVALID_PROVIDER_TYPE`；`pii.mode=llm` 但 `provider_id` 未填拒 `INVALID_PII_PROVIDER`（兌現 spec Requirement「Config schema supports provider pool with role bindings」四 scenario）
- [x] 4.2 GREEN sidecar：[P] `sidecar/src/codebus_agent/config/provider_pool.py` 實作 loader — 兼容兩種 schema；新格式驗 binding existence + type match + PII allowlist。4.1 全綠

## 5. Sidecar settings mutation endpoints

- [x] 5.1 RED test：`sidecar/tests/api/test_settings_endpoint.py` — pytest 測 `GET /settings/providers` 回 `{providers, bindings, pii_mode}` 不含 `api_key` 欄位；`POST /settings/providers` 上傳 `{id, type, model, base_url}` 不含 api_key 寫進 in-memory 池；`DELETE /settings/providers/{id}` 對 bound provider 拒 409 `PROVIDER_BOUND_TO_ROLE`；`PUT /settings/bindings` 觸發 `RegistryHolder.swap()`；`PUT /settings/pii-mode` 改 mode（兌現 spec Requirement「Sidecar accepts provider config mutation endpoints」三 scenario）
- [x] 5.2 GREEN sidecar：`sidecar/src/codebus_agent/api/settings.py` 實作五 endpoints；`include_in_schema=False`；mutation 後 emit `provider_config_changed`；config 持久化到 disk（不含 api_key）。5.1 全綠
- [x] 5.3 RED test：`sidecar/tests/api/test_healthz_dependency.py` — pytest 測 `/healthz` 回 `dependency.{llm_chat, llm_embed, pii}` 三 lane；冷啟動沒打 startup-config → llm_chat / llm_embed 都是 `not-configured`；config 完整 + smoke check pass → 三 lane 都是 `ready`；Qdrant 不可達 → status 變 `degraded` + `dependency.qdrant: unreachable`（兌現 spec MODIFIED Requirement「Health endpoint」三 scenario）
- [x] 5.4 GREEN sidecar：改 `sidecar/src/codebus_agent/api/healthz.py` 加 `dependency` 欄位三 lane + smoke check（embedding model availability / chat model list）。5.3 全綠

## 6. useProviderConfig composable

- [x] 6.1 RED test：`web/tests/settings/useProviderConfig.spec.ts` — vitest 測 module-level singleton（兩 caller 同 ref `Object.is`）、`loadConfig()` GET `/settings/providers` 後 state 同步、`upsertProvider` POST `/settings/providers`、`deleteProvider` DELETE、`setBinding` PUT、`setPiiMode` PUT；defensive grep：source 不含 `api_key`（兌現 spec Requirement「useProviderConfig composable exposes provider pool state」三 scenario）
- [x] 6.2 GREEN：`web/app/composables/useProviderConfig.ts` module-level singleton 同 `useQaSession` / `useIntervention` 慣例；訂閱 app-level SSE event re-fetch。6.1 全綠
- [x] 6.3 RED test：`web/tests/settings/useProviderConfig.spec.ts` 補 SSE event 測 — mock useSseTask 推 `provider_config_changed`，composable 100ms 內發出 GET `/settings/providers` re-fetch，state 重新 hydrate
- [x] 6.4 GREEN：composable 內 watch SSE event stream + debounced re-fetch。6.3 全綠

## 7. Settings page UI

- [x] 7.1 RED test：`web/tests/settings/ProviderPoolList.spec.ts` — render 測：mount 後渲染既有 providers 為 row，每 row 有 edit / delete 按鈕；click「Add provider」開 `<ProviderEditModal>`；delete 按鈕對 bound provider 顯示阻擋訊息（兌現 spec Requirement「Provider pool CRUD touches keyring and config」)
- [x] 7.2 GREEN：`web/app/components/settings/ProviderPoolList.vue` 實作 list + add / edit / delete UI。7.1 全綠
- [x] 7.3 RED test：`web/tests/settings/ProviderEditModal.spec.ts` — render 測：四欄 `id` / `type` / `model` / `base_url` / `api_key`（含 reveal toggle）；Confirm 順序：先 `keyring_set` IPC、成功才 `useProviderConfig().upsertProvider`；keyring fail → 顯示 error 不調 upsert
- [x] 7.4 GREEN：`web/app/components/settings/ProviderEditModal.vue` 實作；mock keyring IPC via Tauri。7.3 全綠
- [x] 7.5 RED test：`web/tests/settings/RoleBindingTable.spec.ts` — render 測：四 row（reasoning / judge / chat / embed），dropdown 選項由 `useProviderConfig().providers` 過濾出 type-相容 provider；非 embed 改 binding 直接 setBinding；embed 改 binding 觸發 `<EmbeddingChangeConfirmModal>`（兌現 spec Requirement「Role binding change propagates via hot-swap」+「Embedding switch goes through destructive confirm modal」）
- [x] 7.6 GREEN：`web/app/components/settings/RoleBindingTable.vue` 實作。7.5 全綠
- [x] 7.7 RED test：[P] `web/tests/settings/EmbeddingChangeConfirmModal.spec.ts` — render 測：modal 開時顯示 current KB chunk count + 預估 rebuild duration；Cancel 不 setBinding；Confirm 觸發 setBinding + KB rebuild SSE task（mock fetch + sse）；rebuild 期間任何 `/qa` / `/explore` / `/scan` 回 503 `KB_REBUILD_IN_PROGRESS`（兌現 spec Requirement「Embedding switch goes through destructive confirm modal」三 scenario + design Decision 6: Embedding 切換 destructive — 獨立 confirm modal）
- [x] 7.8 GREEN：`web/app/components/settings/EmbeddingChangeConfirmModal.vue` 實作；503 行為由 sidecar 守，前端只負責 UI banner。7.7 全綠
- [x] 7.9 GREEN：`web/app/components/settings/PiiModeToggle.vue` 實作（rule / llm radio，llm 選項在 P0 disable）；`web/app/pages/settings.vue` 三 section 排版兌現 spec Requirement「Settings page renders three sections」
- [x] 7.10 補 design mockup：[P] `design/v1/setting-page.html`（與 `tokens.css` / `shell.css` 對齊既有 mockup 風格）

## 8. Onboarding wizard（Decision 5: Onboarding wizard 結構 — 三步、不可 skip、PII 不出現）

- [x] 8.1 RED test：[P] `web/tests/onboarding/welcome.spec.ts` — render 測：page 渲染 codebus 介紹 + 「需要 LLM 才能跑」+ ToS 連結；Next button always enabled；click 進 `/onboarding/providers`（兌現 spec Requirement「Onboarding wizard exposes three sequential routes」三 scenario）
- [x] 8.2 GREEN：[P] `web/app/pages/onboarding/welcome.vue` 實作。8.1 全綠
- [x] 8.3 RED test：[P] `web/tests/onboarding/providers.spec.ts` — render 測：兩表單（chat / embed）每個四欄；Next button 預設 disabled，chat 全填 仍 disabled，兩表單全填 enable；submit 順序：keyring_set chat → keyring_set embed → upsertProvider chat → upsertProvider embed → setBinding × 4 → route done；任一 keyring fail 中斷不繼續（兌現 spec Requirement「Onboarding writes through keyring and provider config in correct order」三 scenario）
- [x] 8.4 GREEN：`web/app/pages/onboarding/providers.vue` + `web/app/pages/onboarding/done.vue` 實作。8.3 全綠
- [x] 8.5 補 design mockup：[P] `design/v1/onboarding-welcome.html` / `design/v1/onboarding-providers.html` / `design/v1/onboarding-done.html` 三畫面；對齊既有 `design/v1/tokens.css` / `shell.css` 風格

## 9. Startup detection redirect

- [x] 9.1 RED test：`web/tests/onboarding/onboarding-redirect.spec.ts` — vitest 測：cold start `/healthz.dependency.llm_chat: not-configured` → middleware redirect `/onboarding/welcome`；station URL paste（empty keyring）→ redirect；`/onboarding/*` 路由不 redirect；browser back 從 onboarding 到 `/` 仍 redirect 回 onboarding（兌現 spec Requirement「Startup detection redirects to onboarding when any LLM dependency is not configured」四 scenario）
- [x] 9.2 GREEN：`web/app/middleware/onboarding-redirect.global.ts` 實作 Nuxt route middleware；exclude `/onboarding/*`。9.1 全綠
- [x] 9.3 RED test：`web/tests/onboarding/index-page-redirect.spec.ts` — vitest 測 `pages/index.vue` mount 時打 `/healthz`；`not-configured` → `router.replace('/onboarding/welcome')`；ready → 渲染既有 workspace picker UI（兌現 spec Requirement「Index page redirects to onboarding when LLM dependencies are not configured」二 scenario）
- [x] 9.4 GREEN：改 `web/app/pages/index.vue` 加 healthz check + 條件 redirect。9.3 全綠

## 10. TopBar settings entry

- [x] 10.1 RED test：`web/tests/settings/topbar-settings-entry.spec.ts` — render 測：tutorial-level page 上 `<TopBar>` 渲染 `data-testid="topbar-settings"` 按鈕；click 走 router push `/settings`；onboarding routes 上 button 不渲染（兌現 spec Requirement「TopBar exposes a settings entry routed to /settings」二 scenario）
- [x] 10.2 GREEN：改 `web/app/components/layout/TopBar.vue` 加齒輪 button；layout host 接 `open-settings` emit 改成 router push。10.1 全綠

## 11. LLM Call Inspector PII filter（Decision 7: O-04 LLM Call Inspector — 顯示 provider id + filter PII detection）

- [x] 11.1 RED test：`web/tests/audit/LlmCallInspector-pii.spec.ts` — render 測：mount 帶 entry `provider_id: openai-default` → 渲染 `[data-testid="llm-inspector-provider-id"]` 字面 `openai-default`；rows = 3 chat + 2 pii_detection 且 `hidePiiDetection: true` → next 從最後 chat clamp、count `3 / 3`；`hidePiiDetection: true` + 有 pii 時 toggle button 顯示「+ 2 PII detection call(s) hidden」；click toggle emit `toggle-pii-visible`（兌現 spec Requirement「LlmCallInspector renders provider id and filters PII detection role」四 scenario）
- [x] 11.2 GREEN：改 `web/app/components/audit/LlmCallInspector.vue` 加 `hidePiiDetection` prop（default true）+ provider_id chip + toggle button + emit；prev/next 過濾 pii_detection role。11.1 全綠
- [x] 11.3 RED test：`web/tests/audit/AuditPanel-pii.spec.ts` — render 測：`activeTab=llm` + rows 含 pii_detection 且 `hidePiiDetection: true` → row count 排除 pii rows、列表渲染數量正確；其他 6 tabs（sanitize 等）不受 prop 影響（兌現 spec Requirement「AuditPanel filters llm tab rows by role for PII separation」二 scenario）
- [x] 11.4 GREEN：改 `web/app/components/audit/AuditPanel.vue` 加 `hidePiiDetection` prop + 對 llm tab 過濾 + toggle banner emit。11.3 全綠
- [x] 11.5 GREEN：改 `web/app/pages/audit/llm.vue` 與既有 station / explorer page 把 `hidePiiDetection` 接通到 inspector + AuditPanel；ref state 在 page 層儲存

## 12. Sidecar test baseline + web typecheck + e2e

- [x] 12.1 跑 `cd sidecar && uv run pytest` 全綠（既有 ~885 + 新增 ≥ 30 case 共 0 regression）
- [x] 12.2 跑 `cd web && npm run test` 全綠（既有 184 + 新增 ≥ 25 case 共 0 regression）
- [x] 12.3 `cd web && npm run typecheck` zero error baseline 守住
- [ ] 12.4 手動 e2e：(a) 全新 keyring → 進 app → 自動 redirect onboarding → 完成 wizard → 進 entry page → 進 station 跑 Q&A；(b) 進 settings 改 chat provider model → 下個 task 用新 model（看 `llm_calls.jsonl` 的 model 欄）；(c) 進 settings 改 embed provider → confirm modal → KB rebuild → 期間 `/qa` 503 → 完成後 Q&A 正常
- [ ] 12.5 cross-platform PoC：[P] macOS / Windows / Linux（GNOME Keyring）三平台跑 1.1–1.5 + 12.4 happy path（CI matrix 或手動逐台）

## 13. 文件同步 + commit

- [x] 13.1 `docs/decisions.md` D-033 加追記「[x] 2026-XX-XX `provider-settings-and-onboarding` archive」+ 收尾三開放問題（in-flight task 切 provider 行為、setting 改要不要重啟、Linux Secret Service fallback）
- [x] 13.2 `docs/implementation-plan.md` Phase 7 加 step「D-033 B」標 ✅ landed
- [x] 13.3 `docs/llm-provider.md` 加 Provider pool schema（`llm.providers[]` + `llm.bindings`）+ Registry hot-swap 段
- [x] 13.4 `docs/authorization.md §六` 加 PII LLM 模式對 rules version 的影響段
- [x] 13.5 `CLAUDE.md` 加 Setting / Onboarding 啟動流程段（`/healthz.dependency` 三 lane + middleware redirect + keyring trust boundary）
- [ ] 13.6 `pre-commit run --all-files` 全綠後 commit
