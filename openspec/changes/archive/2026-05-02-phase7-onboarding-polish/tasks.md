## 1. ToS contextual：測試先寫（TDD red）

- [x] 1.1 [P] 在 `web/tests/onboarding/welcome.spec.ts` 加 case「Welcome page contains no provider-specific ToS link」：mount welcome 後斷言 DOM 不含 `href` 涵蓋 `openai.com`、`anthropic.com` 等 provider ToS URL pattern 的 anchor；同時斷言 legal-acknowledgement copy 不含「OpenAI」具名字串
- [x] 1.2 [P] 在 `web/tests/onboarding/providers.spec.ts` 加 case「Providers page renders contextual ToS link per type」：chat form `type=openai_chat` 時 render anchor `href=https://openai.com/policies/terms-of-use/`；embed form `type=openai_embedding` 同樣 render；mock 一個 `type=unknown_future` 不在常數 map 內時 anchor 不 render
- [x] 1.3 跑 `cd web && npm run test -- onboarding/` 確認 1.1 / 1.2 兩 case 紅（fail），符合 TDD red 階段

## 2. ToS contextual：實作（TDD green）

- [x] 2.1 在 `web/app/utils/` 加 `provider-tos.ts`，export const `PROVIDER_TYPE_TOS_URL: Record<ProviderType, string>`，P0 涵蓋 `openai_chat` / `openai_embedding` 兩 entry 皆指向 `https://openai.com/policies/terms-of-use/`；export helper `getTosUrl(type)` 在 type 無對應 entry 時回 `null`
- [x] 2.2 修 `web/app/pages/onboarding/welcome.vue`：拿掉「Read the OpenAI Terms of Service before continuing」整段（含 anchor 與 paragraph wrapper）；保留 codebus 介紹與 LLM 需求 generic 文字 — 對齊 spec 的 `Onboarding wizard exposes three sequential routes` Requirement 第一條 narrative 改寫
- [x] 2.3 修 `web/app/pages/onboarding/providers.vue`：在 chat fieldset 內 import `getTosUrl`，依 `chat.type` lookup 後 render `<a>` ToS link（含 `data-testid="onboarding-chat-tos-link"`）；embed fieldset 同樣 render（`data-testid="onboarding-embed-tos-link"`）；`getTosUrl()` 回 null 時 anchor 不 render
- [x] 2.4 跑 `cd web && npm run test -- onboarding/` 確認 1.1 / 1.2 兩 case 綠（pass）

## 3. provider-onboarding 文案 i18n zh-TW

- [x] 3.1 [P] 翻譯 `web/app/pages/onboarding/welcome.vue`：標題、介紹段、Next 按鈕全部改 zh-TW；保留技術術語 `chat model` / `embedding model` / `API key` / `keychain` / `macOS Keychain` / `Windows Credential Manager` / `Linux Secret Service` 為英文
- [x] 3.2 [P] 翻譯 `web/app/pages/onboarding/providers.vue`：標題「Configure providers」、legend「Chat」/「Embedding」、placeholder「provider id」/「model」/「api key」、error 文案「Chat keyring failed:」/「Embedding keyring failed:」、按鈕「Next」改 zh-TW；保留 `provider id` / `api key` / `base_url` / `model` 為英文 placeholder（技術欄位識別字）
- [x] 3.3 [P] 翻譯 `web/app/pages/onboarding/done.vue`：標題「All set」、確認段、按鈕「Start」改 zh-TW

## 4. provider-settings 文案 i18n zh-TW

- [x] 4.1 [P] 翻譯 `web/app/pages/settings.vue`：頁標題「Settings」與副標「Configure LLM providers, role bindings, and PII detection mode.」改 zh-TW
- [x] 4.2 [P] 翻譯 `web/app/components/settings/ProviderPoolList.vue`：所有可見文字（標題、Add provider 按鈕、edit / delete affordance、blocked-delete message「Provider <id> is bound to:」）改 zh-TW
- [x] 4.3 [P] 翻譯 `web/app/components/settings/RoleBindingTable.vue`：role 名 / 表頭 / 提示文案改 zh-TW；保留 `reasoning` / `judge` / `chat` / `embed` 四個 role identifier 為英文
- [x] 4.4 [P] 翻譯 `web/app/components/settings/PiiModeToggle.vue`：`rule` / `llm` 兩 radio label + 描述改 zh-TW；保留 `rule` / `llm` 為英文 enum 值
- [x] 4.5 [P] 翻譯 `web/app/components/settings/ProviderEditModal.vue`：欄位 label、按鈕「Save」/「Cancel」、error 文字改 zh-TW
- [x] 4.6 [P] 翻譯 `web/app/components/settings/EmbeddingChangeConfirmModal.vue`：warning copy（含 KB rebuild 提示）、估時段、Cancel / Confirm 按鈕改 zh-TW

## 5. 既有測試 baseline 守住

- [x] 5.1 跑 `cd web && npm run test` 全綠（既有 234 + task 1.1 / 1.2 新增 = 0 regression）
- [x] 5.2 跑 `cd web && npm run typecheck` zero error baseline 守住

## 6. 文件同步

- [x] 6.1 grep `docs/` 與 `README.md` 是否有引用「OpenAI Terms of Service」具名字串作為 onboarding wizard 描述 — 有則同步改為 generic「provider's terms of service」描述
- [x] 6.2 `docs/decisions.md`：本 change 不引入新 D-XXX ADR（ToS 解綁屬 D-033 follow-up，i18n 屬 CLAUDE.md §溝通語言已存規範執行），若 archive 階段判斷需要則屆時補

## 7. Apply 期 ingest：providers 頁視覺 polish（manual e2e 第 1 輪發現）

> 由 user 跑 `cargo tauri dev` 走 task 12.4 路徑 (a) 時截圖回報，三個視覺漏洞： (1) fieldset `<legend>` 跟 border 重疊（沒背景色透出 border 線）、(2) h1「設定 LLM 提供者」對比過弱看起來像 ghost text、(3) 垂直 padding 過大 + grid place-items-center 把 content 推至中央，nuxi devtools 浮起時 form 容易看不全。

- [x] 7.1 修 `web/app/pages/onboarding/providers.vue` fieldset `<legend>`：兩個 legend（Chat / Embedding）加 `px-2 bg-surface-1` class 蓋住 fieldset border 那條穿過 legend 的線
- [x] 7.2 修 `web/app/pages/onboarding/providers.vue` h1 對比：標題「設定 LLM 提供者」加 `text-white` 覆蓋 token 預設值，避免 ghost text 觀感
- [x] 7.3 修 `web/app/pages/onboarding/providers.vue` 垂直 layout：`<main>` 從 `grid place-items-center min-h-screen p-8` 改 `flex flex-col items-center min-h-screen py-6 px-8`（從頂排起、不垂直居中），避免 nuxi devtools / 視窗縮小時 form 內容超出可視區

## 8. Apply 期 ingest：external link 用 tauri-plugin-opener

> Tauri 2 webview 預設不放行 anchor `target="_blank"` 開外部 URL（安全策略），導致 providers 頁的 ToS link 點了沒反應。改用官方 `tauri-plugin-opener` 走 IPC 開系統瀏覽器，後續任何 external link 都用同一條路。

- [x] 8.1 `tauri/src-tauri/Cargo.toml` 加 `tauri-plugin-opener = "2"` dependency
- [x] 8.2 `tauri/src-tauri/src/lib.rs` 在 builder 鏈加 `.plugin(tauri_plugin_opener::init())`
- [x] 8.3 `tauri/src-tauri/capabilities/default.json` `permissions` array 加 `"opener:default"`
- [x] 8.4 `web/package.json` 加 `@tauri-apps/plugin-opener` dependency 並 `npm install`
- [x] 8.5 `web/app/utils/` 加 `external-link.ts`：export `openExternal(url)` helper，Tauri 環境 import `@tauri-apps/plugin-opener` 呼 `openUrl()`，非 Tauri（vitest）fallback 到 `window.open(url, '_blank')`
- [x] 8.6 修 `web/app/pages/onboarding/providers.vue`：兩個 ToS anchor 改 `@click.prevent="openExternal(chatTosUrl)"` / `embedTosUrl`；移除 `target="_blank"` rel attribute 仍保留以避免 anchor 看起來不可點
- [x] 8.7 跑 `npm run test -- onboarding/` 全綠（providers test 加 mock for `~/utils/external-link`）

## 9. Apply 期 ingest：CORS allow_methods 漏 PUT/DELETE（D-033 B regression）

> 提交 onboarding wizard 走 `PUT /settings/bindings` × 4，但 sidecar CORS `allow_methods` 只有 GET/POST/OPTIONS → preflight 不放行 PUT → browser fetch 拋 "Failed to fetch"。同樣影響 settings page 切 binding 與 deleteProvider（DELETE）。

- [x] 9.1 `sidecar/tests/test_cors_preflight_smoke.py` 加紅測：PUT preflight from `http://localhost:3000` 應在 `access-control-allow-methods` header 出現「PUT」
- [x] 9.2 同檔加 DELETE preflight 紅測
- [x] 9.3 修 `sidecar/src/codebus_agent/api/__init__.py` CORSMiddleware `allow_methods` 加 `"PUT"`, `"DELETE"`, `"PATCH"`
- [x] 9.4 跑 `cd sidecar && uv run pytest tests/test_cors_preflight_smoke.py -v` 全綠
- [x] 9.5 重打 PyInstaller binary：`cd sidecar && uv run pyinstaller codebus-sidecar.spec --noconfirm`
- [x] 9.6 使用者 Ctrl+C `cargo tauri dev` 重啟以載入新 binary（手動）

## 10. Apply 期 ingest：startup-config idempotent 鎖死導致 onboarding 死循環（D-033 B 設計缺陷）

> 路徑 (a) 跑出來：填完 key、按「開始」→ middleware 又把使用者 redirect 回 welcome。根因：sidecar `/internal/startup-config` D-033 B 設計為 idempotent 一次性（boot 後 409），但 onboarding 完成後沒辦法再推 key 給 sidecar，於是 `dependency.llm_chat: not-configured` 永遠不變 → 死循環。修法：放寬 idempotent 鎖，改覆蓋式；提交 onboarding 後前端 invoke `push_startup_config_cmd` 把新 key 推給 sidecar。

- [x] 10.1 sidecar 加紅測：第二次 POST `/internal/startup-config` 應覆蓋 `app.state.provider_keys`，回 204 而非 409（既有「409 二次拒絕」測試同步移除或改寫）
- [x] 10.2 修 `sidecar/src/codebus_agent/api/startup_config.py`：拿掉 `startup_config_applied` 409 短路；每次 POST 直接覆蓋 `provider_keys`；保留 `startup_config_applied` flag 給 `_resolve_llm_lane` 等邏輯參考但不再阻擋
- [x] 10.3 跑 `cd sidecar && uv run pytest tests/api/test_startup_config.py -v` 全綠
- [x] 10.4 修 `web/app/pages/onboarding/providers.vue` `onNext`：setBinding × 4 完成後 invoke Tauri `push_startup_config_cmd` 並傳 `providerIds: [chat.id, embed.id]`；失敗仍 route 到 done（sidecar 重啟後 boot startup-config 會再撈一次，所以不是阻塞錯誤），但寫 console error
- [x] 10.5 寫 `web/tests/onboarding/providers.spec.ts` 加 case：成功提交時 `push_startup_config_cmd` 被 invoke 一次且 providerIds 包含 chat + embed id
- [x] 10.6 加 `specs/keyring-integration/spec.md` delta：MODIFY 把 idempotent 不變式拿掉，narrative 改為「sidecar accepts repeat startup-config POSTs to support post-onboarding key delivery; the latest body always wins」
- [x] 10.7 跑 `cd sidecar && uv run pytest -q` 全綠 + `cd web && npm run test --silent --run` 全綠
- [x] 10.8 重打 PyInstaller：`cd sidecar && uv run pyinstaller codebus-sidecar.spec --noconfirm`
- [x] 10.9 使用者 Ctrl+C `cargo tauri dev` 重啟以載入新 binary（手動）

## 11. Apply 期 ingest：boot 階段沒人推 startup-config（D-033 B 設計缺陷延伸）

> 修完 #10 後再驗仍卡 onboarding loop。根因：`push_startup_config_cmd` 只有 onboarding submit 時被 invoke；正常 boot（已配置使用者）keyring 已有 keys 但沒人推給 sidecar，`provider_keys` 為空 → middleware 永遠 redirect。需要 boot 階段自動 push 一次。

- [x] 11.1 加 `web/app/plugins/sidecar-startup-config.client.ts` Nuxt client plugin：等 useSidecar handshake 完成 → loadConfig → 拿 provider_ids → invoke `push_startup_config_cmd`；providers 為空（首次 boot）跳過；任何步驟失敗 log to console 且不阻 app boot
- [x] 11.2 跑 `cd web && npm run test --silent --run` 全綠（既有 241 不破，plugin 不加單獨 vitest — Tauri IPC + Nuxt plugin chain mock 太重，由實機 e2e 驗）
- [x] 11.3 跑 `cd web && npm run typecheck` zero error 守住
- [x] 11.4 使用者 Ctrl+C 重啟 cargo tauri dev（純前端改動，不需重打 sidecar binary）
- [x] 11.5 重新走路徑 (a) 冷啟動 + 已配置 boot 兩種情境驗 onboarding loop fix

## 12. Apply 期 ingest：done page 變成假成功 + plugin 沒被 register

> 使用者實機驗：(1) plugin 沒看到任何 console log → 大機率 object-form plugin 沒被 Nuxt 4 載入；(2) done page 顯示「一切就緒」但其實 sidecar 還是 not-configured（onNext catch 了 push error 還 route 到 done），按 Start 又被 redirect 回 welcome → loop。修：A. onNext 改成「push 必須成功 + healthz 必須真的 ready」才 route done；B. plugin 改 function form 對齊既有 mdc plugin convention，加無條件 console log 確認載入。

- [x] 12.1 改 `web/app/plugins/sidecar-startup-config.client.ts`：object form 改 `defineNuxtPlugin(async (nuxtApp) => { ... })` function form；setup 第一行 `console.log('[sidecar-startup-config] plugin loaded')` 無條件印出供使用者驗證 plugin 是否被 Nuxt register
- [x] 12.2 改 `web/app/pages/onboarding/providers.vue` `onNext`：push_startup_config_cmd 改成「失敗 → set error.value + return（不 route）」；push 成功後 fetch `/healthz` 驗 `dependency.llm_chat === 'ready'` && `dependency.llm_embed === 'ready'`；驗失敗 set error.value + return；都過才 router.push('/onboarding/done')
- [x] 12.3 改 `web/tests/onboarding/providers.spec.ts`：既有 success case 加 healthz mock 回 `{ dependency: { llm_chat: 'ready', llm_embed: 'ready' } }`；既有「push fail still route」case 改成「push fail NOT route」（顯示 error 留原地）；加新 case「push success but healthz lane not-configured → NOT route」
- [x] 12.4 跑 `cd web && npm run test --silent --run` 全綠
- [x] 12.5 跑 `cd web && npm run typecheck` zero error
- [x] 12.6 使用者 Ctrl+C 重啟 cargo tauri dev → 開 devtools console → 重新整理 → **應該看到 `[sidecar-startup-config] plugin loaded`**（A 也獨立修死循環，B 是診斷）

## 13. Apply 期 ingest：smoke probe vs keys 脫鉤（D-033 B 真正的卡關 root cause）

> 我自己起 sidecar binary 模擬整條 onboarding 流程：所有 mutation 204 成功、snapshot 內容正確、startup-config POST 後 `app.state.provider_keys` 也寫對 — 但 healthz 仍 not-configured。Trace 到 `_resolve_llm_lane` 的第三個 if：boot 時若沒 `CODEBUS_OPENAI_API_KEY` env var，sidecar register 一個永遠回 `not-configured` 的 smoke probe；此 probe status 直接 propagate 到 lane，完全 ignore 後續 startup-config 推進來的 keys。D-033 B 換 keyring + startup-config 模型但 smoke probe 還停在 env-var 模型，兩套脫鉤 = 真 root cause。

- [x] 13.1 `sidecar/tests/api/test_healthz_dependency.py` 加紅測：boot 時無 env var + setBinding 後 push startup-config → healthz `dependency.llm_chat` 應為 `ready`（既有測試若有 assert not-configured 也同步改）
- [x] 13.2 修 `sidecar/src/codebus_agent/api/__init__.py::_resolve_llm_lane`：把 `smoke.status == 'not-configured'` 視為 stale 訊號（env-var 模型遺物），改回 `ready`；只有 `smoke.ok` 為 False 且 status 非 not-configured 才 return `unreachable`
- [x] 13.3 跑 `cd sidecar && uv run pytest tests/api/test_healthz_dependency.py tests/api/test_startup_config.py -v` 全綠
- [x] 13.4 跑 `cd sidecar && uv run pytest -q` 全綠 baseline 守住
- [x] 13.5 重打 PyInstaller binary：`cd sidecar && uv run pyinstaller codebus-sidecar.spec --noconfirm`
- [x] 13.6 使用者 Ctrl+C cargo tauri dev 重啟 → 走 onboarding (a) 路徑 → 應該真的進 entry page 不再 loop

## 14. Apply 期 ingest：Provider pool / bindings / pii_mode 磁碟持久化（D-033 B 補洞）

> D-033 B archive 描述「persists the config (without api_key) to disk」但實作只動 `app.state.provider_pool_snapshot` (in-memory)。Sidecar 一重啟（cargo tauri dev / 系統重啟）pool 立刻消失，使用者每次都要重做 onboarding。修：加 `~/.codebus/llm-config.json`（App-level，與 keyring 同 scope；不含 API key — keys 仍只在 OS keyring）。Boot 讀回；5 個 mutation endpoint 寫完 in-memory 後 atomic write。

- [x] 14.1 `sidecar/src/codebus_agent/auth/paths.py` 加常數 `_LLM_CONFIG_FILENAME = "llm-config.json"` + helper `llm_config_path() -> Path`（reuse 既有 `app_codebus_dir()`）
- [x] 14.2 加 `sidecar/src/codebus_agent/config/llm_config_store.py`：(a) `save_llm_config(snapshot)` atomic write（先 `.tmp` 後 `os.replace`；snapshot 純 metadata，無 api_key）；(b) `load_llm_config_or_default()` 讀回 `ProviderPoolSnapshot`，檔案不存在 / corrupt JSON 都回空 default + log warn；(c) export schema constant `LLM_CONFIG_SCHEMA_VERSION = 1`
- [x] 14.3 加 `sidecar/tests/config/test_llm_config_store.py` 紅測：(a) `test_save_then_load_round_trip` save 後 load 回相同 snapshot；(b) `test_load_returns_empty_when_file_missing` 檔不存在 → 空 default；(c) `test_load_returns_empty_on_corrupt_json` JSON 解析失敗 → 空 default + warn log；(d) `test_save_atomic_uses_tmp_then_replace` 過程不留 partial state；(e) `test_save_does_not_write_api_key`（不該存在的欄位）
- [x] 14.4 跑 task 14.3 確認紅
- [x] 14.5 改 `sidecar/src/codebus_agent/api/__init__.py::create_app`：boot 階段呼 `load_llm_config_or_default()` 取代當前寫死的「空 ProviderPoolSnapshot」init；caller 可透過 kwarg `llm_config_loader: Callable | None = None` override（測試用）
- [x] 14.6 改 `sidecar/src/codebus_agent/api/settings.py` 5 個 mutation endpoints（POST /settings/providers / DELETE /settings/providers/{id} / PUT /settings/bindings / PUT /settings/pii-mode；同時看是否有第 5 個如 PUT /settings/providers）：寫完 `app.state.provider_pool_snapshot` 後同步 `save_llm_config(new_snapshot)`；save 失敗 log error 但仍回 204（in-memory 已寫，下次 mutation 再嘗試）
- [x] 14.7 加 sidecar integration test：POST /settings/providers → 檔案內容 mirror new snapshot；create_app 後 pre-write file → snapshot 反映 file 內容
- [x] 14.8 spec MODIFY `specs/keyring-integration/spec.md` 或 `provider-settings/spec.md` delta：加 Requirement「Provider pool / bindings / pii_mode persisted to ~/.codebus/llm-config.json」+ scenarios（API key 不寫此檔；boot 讀回；mutation 寫回）
- [x] 14.9 跑 `cd sidecar && uv run pytest -q` 全綠 baseline
- [x] 14.10 重打 PyInstaller binary
- [x] 14.11 使用者 Ctrl+C cargo tauri dev 重啟 → 第一次走 onboarding → 重啟 app → 應該**直接進 entry page 不再 redirect onboarding**（plugin 看到 pool 不空 + push keys → healthz ready）
