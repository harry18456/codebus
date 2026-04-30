## Context

D-033 切的 A / B 兩個 change 中，A（`split-providers-and-pii-llm`，2026-04-29 archive）已把 Provider 抽象從單一 Protocol 拆成三介面（`LLMProvider` / `EmbeddingProvider` / `PIIProvider`）+ marker `PII_ALLOWED_INNER_TYPES` 雙 allowlist + Sanitizer 消費 `PIIProvider`，但**刻意不動 Registry lifecycle**（D-033 不變式 6），因為 mutability 牽動 Tauri 啟動流程、in-flight task 行為、SSE 推送，必須與 setting UI / onboarding wizard / keyring 一起設計才合理。

Phase 6 已在 2026-04-30 全部 archive（步驟 25.5 / 26 / 26.5 / 27 / 28 / 28.5 / 28.6 / 29 / 30），Trust Layer 四站 + 三介入點 + Q&A overlay 通電完畢。**本 change 是 D-033 B 落地節點**，把 setting page + onboarding wizard + Tauri keyring 補上，讓 Demo ready 條件「使用者下載 app → 跑 onboarding → 填 provider → 進主畫面開始學」端到端通電。

當前痛點：

1. **API key 走 env var**：`OPENAI_API_KEY` 等從 `.env` 讀；desktop app 沒 OS keychain 整合，不適合終端使用者
2. **啟動時 sidecar graceful degraded 沒前端引導**：sidecar `/healthz` 報 `dependency.openai_chat: not-configured`（A archive 後加的欄位），前端不知道要做什麼，使用者進主畫面什麼都跑不動
3. **Provider 切換需重啟**：`ProviderRegistry` 建構期 freeze（A 不動 lifecycle），改 model / api_key / role binding 都要重啟 sidecar；不接受 demo 期使用者操作

本 design 鎖五個關鍵技術決策（覆蓋 D-033 §B 開放問題 1 / 2 / 3）+ 三個跨層 invariant（API key 不入 audit / Embedding destructive / Onboarding 不可 skip），避免 apply 期撞牆。

## Goals / Non-Goals

**Goals:**

- 三介面 provider（LLM / Embedding / PII）都可在 setting page 內配置；不需編輯 yaml + 重啟
- API key 走 OS-native keychain（macOS Keychain / Windows Credential Manager / Linux Secret Service）；磁碟、env var、audit 都不留原值
- Onboarding wizard 引導首次啟動的使用者三步配齊 chat × 1 + embed × 1，不可 skip
- 啟動時自動偵測 `/healthz.dependency`，未配置 → redirect onboarding
- Hot-swap：除 embedding 切換是 destructive、走 confirm modal、明示 rebuild KB 外，其他 provider / role binding 變更即時生效；in-flight task 跑完現場、下個 task 用新 binding
- O-04 LLM Call Inspector row 顯示 provider id；PII detection lane 預設過濾、banner 折疊（兌現 D-033 不變式 3「PII LLM call 仍要 audit」+ §B 對前端的影響 §3）
- Tauri keyring 跨平台 PoC：macOS / Windows / GNOME Keyring 三平台 happy path 全綠

**Non-Goals:**

- 不做 multi-tenant API key（per-workspace 各一份）
- 不做 in-flight task 中斷重跑：Generator / Explorer 跑到一半切 chat provider 走「跑完現場 + 下個 task 生效」
- 不做 master password / vault unlock（不疊 stronghold）
- 不做 LLM PII provider 在 onboarding 內配置（PII rule-based 預設）
- 不做 setting 進階配置（temperature / max_tokens / system prompt / retry policy 等）— 只配 provider id / model / base_url / api_key / role binding
- 不做沒桌面環境 Linux 的 keyring fallback PoC（留打磨期）
- 不重畫 Phase 6 已落地的 R-01 / O-01 / O-04 / O-05 內容（per D-033 §B 對前端的影響「不會回頭改 R-01 / O-04 / O-05 內容本身」）

## Decisions

### Decision 1: Tauri keyring plugin 選型 — `tauri-plugin-keyring`（OS 直連）

**選擇**：用社群 plugin `tauri-plugin-keyring`（或對等的 `keyring-rs` crate 直接整合 — 視 Tauri 2.0 plugin ecosystem 成熟度而定，apply 期 PoC 後拍板），直接走 OS-native keychain。**不**用 `tauri-plugin-stronghold`（IronOxide vault）。

**為何不選**：

- (X) `tauri-plugin-stronghold`：多一道 master password 解鎖；對 onboarding flow 不友善（首次使用者要記住第二把密碼），且 vault 是 app 自管 file，跨 device sync 反而難（macOS Keychain 自動 iCloud sync）— 違反 D-033 決策 2
- (X) AES-256 + machine-derived key 自管加密：自寫密鑰派生風險高，落入 OS keychain 才是業界 best practice
- (X) 環境變數 / `.env` 檔（現況）：plain text 落磁碟，跨裝置不同步，desktop app 不適用

**Rationale**：OS keychain 跟 Tauri bearer token 同 trust boundary（記憶體常駐 + OS 持久化 + 不入 audit），是同一條 secret-handling 不變式延伸（D-033 §不變式 1）。社群 plugin 已有 macOS / Windows / Linux Secret Service 三平台實作，apply 期 PoC 主要驗證 Linux 桌面環境穩定度。

**Invariant**：

- API key 在三個地方有：OS keychain（持久化）、Tauri host 記憶體（跨 sidecar lifecycle 快取）、sidecar 記憶體（單次 process lifetime）
- 不在以下地方：磁碟（除 OS keychain 自管 db 外）、env var（onboarding 寫進去後即從 env clear）、`llm_calls.jsonl` / 任何 audit JSONL（即使 sanitizer 開高分數誤判也不能寫，因為從來沒被當 LLM call payload 傳過）、錯誤訊息（HTTPException detail / SSE error event 都過 sanitizer）

### Decision 2: Sidecar key 注入機制 — startup config IPC（不打 stdin handshake）

**選擇**：sidecar 既有 stdin 第一行印 `{"port": <int>, "bearer": "<≥32 chars>"}` 後（per `sidecar-runtime` Requirement「Handshake via stdout first line」），Tauri 緊跟著透過 sidecar 新 endpoint `POST /internal/startup-config`（loopback only + bearer auth + body 含 `provider_keys: dict[provider_id, api_key]`）一次性把 keys 注入；sidecar 收到後寫進 `app.state.provider_keys` 記憶體 dict，不落盤、不入 audit。

**為何不選**：

- (X) stdin 第二行 / 第 N 行：handshake 已經是契約（單一 JSON line），加更多行打破 PyInstaller 打包後的 stdout buffering 假設，紅隊測試覆蓋面要重做
- (X) 環境變數 spawn 時注入：env var 在 Linux `/proc/<pid>/environ` 可讀（其他 user 也能看），破 D-033 不變式 1
- (X) 配置檔：違反「不寫磁碟」不變式
- (X) 命令列 args：與 env var 同樣可被 `ps` 看到

**Rationale**：sidecar 已有 bearer auth + loopback bind，新增一個 internal endpoint 重用現有 trust boundary。`POST /internal/startup-config` 名字明示 phase（startup only），收到後翻 idempotent flag，第二次叫拒絕（防 IPC injection）。

**Invariant**：

- `/internal/startup-config` 只能在 sidecar boot 後 5 秒內被叫一次；超時後或第二次叫拒回 409 + `code: STARTUP_ALREADY_CONFIGURED`
- endpoint 不出現在 OpenAPI spec / `/openapi.json`（透過 `include_in_schema=False` 隱藏 — 仍可叫但不在公開 API surface 上）
- 收到後立刻清除 request body 的 in-memory 暫存（FastAPI 自動 GC，但設 explicit `del` 確保）

### Decision 3: Registry hot-swap — `RegistryHolder` 雙層引用 + SSE 推送

**選擇**：加 `RegistryHolder` class（內層仍是 immutable `ProviderRegistry`、外層 `holder.swap(new_registry)` 換 reference）；既有 `app.state.providers: ProviderRegistry` 改成 `app.state.providers: RegistryHolder`，所有取 provider 的點透過 `holder.current()` 拿。Setting page 改 binding 觸發新 SSE event `provider_config_changed: { changed_roles: [...], embed_changed: bool }`；前端收到後 reload `useProviderConfig` state，顯示「下個 task 用新 binding」 toast。

**為何不選**：

- (X) `replace_role(role, new_provider)` mutator：要對既有 immutable Registry 開後門，破壞 D-003 + llm-role-routing 的「freeze 後不可變」契約
- (X) 全廠重啟 sidecar：使用者體驗差（in-flight task 全失）
- (X) Pinia reactive store：違反 D-026 既有 stack，沒必要

**Rationale**：Registry 內層仍 immutable（每次 swap 都做新的 frozen instance），外層 holder 是 mutex-protected reference swap — 既保留 immutable lifecycle 的 audit value（每筆 LLM call 對得回某個 frozen Registry instance），又支援 hot-swap。In-flight task 早就用 `holder.current()` 拿到當下 Registry 的 reference，後續 swap 不影響他們手上的 reference（Python 對 reference 取值是值快照）。

**Invariant**：

- `holder.current()` 必須回 immutable Registry — 任何 caller 拿到後都不能 mutate
- `swap()` 是 atomic（用 `threading.Lock` 或 `asyncio.Lock` 包，視 caller context — sidecar 全 async 用 asyncio.Lock）
- swap 後舊 Registry 的 reference 仍可以被 in-flight task 持有；GC 在所有 task 完成後自動回收
- Embedding 切換 destructive：`provider_config_changed.embed_changed=True` 時 sidecar 必須 emit 額外 event `kb_rebuild_required: { current_kb_size: <int> }`，前端走 confirm modal 二次確認後才實際切（rebuild KB 是 user explicit action）

### Decision 4: Provider pool schema — `llm.providers[]` 陣列 + `llm.bindings`

**選擇**：config schema 從一對一（`llm.roles.<role>.provider_id`）擴成兩層：

```toml
[[llm.providers]]
id = "openai-default"
type = "openai_chat"     # or "openai_embedding" or "anthropic_chat" 未來
model = "gpt-4o-mini"
base_url = "https://api.openai.com/v1"
# api_key 不在 config，從 keyring 讀

[[llm.providers]]
id = "openai-embed-3-small"
type = "openai_embedding"
model = "text-embedding-3-small"
base_url = "https://api.openai.com/v1"

[llm.bindings]
reasoning = "openai-default"
judge = "openai-default"
chat = "openai-default"
embed = "openai-embed-3-small"

[llm.pii]
mode = "rule"            # or "llm"
provider_id = ""         # required when mode == "llm"
```

**為何不選**：

- (X) 維持一對一 + 加 alias：要保留向後相容但 schema 變兩種形態（migration 複雜）；fresh start 切兩層更乾淨
- (X) 把 provider 跟 binding 合併（`llm.roles.<role>.{type, model, base_url}`）：每個 role 重複輸入相同 OpenAI base_url，UX 差；切 model 要改四個地方
- (X) 一個 provider 多 model：把 model 從 provider 拆出，但 OpenAI 的 chat / embedding 不同 endpoint，且 model 變更影響 audit 字面量 — 模型仍是 provider 的一部分

**Rationale**：兩層拆開符合直覺（先有 provider，再決定哪個 role 用哪個 provider）；UI CRUD 也好做（一個 list 管 provider，一個 table 管 binding）。Reasoning / judge / chat 三 role 預設共用同一個 chat provider — onboarding 第二步只要選一個 chat provider + 一個 embed provider 就夠。

**Invariant**：

- `llm.bindings.<role>.provider_id` 必須對應 `llm.providers[].id` — 不存在的 id 啟動時 raise `INVALID_PROVIDER_BINDING`
- `llm.bindings.embed` 必須指向 `type == "openai_embedding"` 的 provider；指錯型別 raise `INVALID_PROVIDER_TYPE`
- `llm.pii.mode == "llm"` 時 `provider_id` 必填且必須指向 PII allowlist 內的 provider type（current: 無；P1+ 才加 `LocalLLMPIIProvider`）

### Decision 5: Onboarding wizard 結構 — 三步、不可 skip、PII 不出現

**選擇**：路由 `/onboarding/welcome` → `/onboarding/providers` → `/onboarding/done`。**Welcome** 純文案 1 張（codebus 是什麼 + 「需要 LLM 才能跑」+「會用到的 OpenAI / Anthropic API 公司 ToS 連結」）；**Providers** 1 步輸入 chat provider（type 下拉 / model 下拉 / base_url 預填 default / api_key 輸入 + reveal toggle）+ embed provider（同樣四欄）；**Done** 1 張顯示「設定完成，進到 entry page」+ CTA 按鈕。**不允許 skip**：每步「下一步」按鈕 disabled 直到當步必填值齊；最後一步前不寫進 keyring + bindings。

**為何不選**：

- (X) Multi-page feature tour：使用者直接點下一步沒在看，UX 有反效果（D-033 決策 4）
- (X) Inline coach mark only：不夠引導沒 LLM 的使用者，他們進主畫面什麼都跑不動
- (X) Onboarding 內配 PII：違反 D-033 決策 6「PII rule-based 預設足夠 demo」；多一步降低完成率
- (X) 允許 skip：違反不變式 5「沒設 provider 進主畫面什麼都做不了」

**Rationale**：簡短不可 skip 才確保使用者進主畫面前 sidecar 已 ready；避免「sidecar graceful degraded 但前端不知道」的窘境。Welcome → Providers → Done 三步是現代 desktop app onboarding 的最小可工作集（看 Linear / Raycast / Tana 一致）。

**Invariant**：

- 進入主畫面（任何 `/tutorial/*` / `/explorer/*` / `/audit/*` / `/workspace/grant`）前，前端 middleware（Nuxt route middleware）打 `/healthz`；任一 dependency `not-configured` → redirect `/onboarding/welcome`
- 已完成 onboarding 後刪 keyring（手動或 OS reset）→ 下次進 app 重新跑 onboarding（自動偵測）
- 從 `/onboarding/*` 路由按瀏覽器後退到 `/`：根 route 偵測 `/healthz` 仍未 ready → 再 redirect 回 `/onboarding/welcome`（防後退跳出 wizard）

### Decision 6: Embedding 切換 destructive — 獨立 confirm modal

**選擇**：Setting page 的 RoleBindingTable 對 `embed` 那一列 click「change」按鈕時，**不**直接切；先彈 `<EmbeddingChangeConfirmModal>`，文案「切換 embedding 會 rebuild 整個 KB（當前 N chunks），重建期間 Q&A / Generator 不可用」+ 「rebuild 過程約 X 分鐘」（用既有 KB chunk count 推估）+ Cancel / Confirm 兩按鈕。Confirm 後才實際 swap registry + 觸發 KB rebuild SSE task；rebuild 完成前其他需 embed 的 task（Q&A `kb_search` / `add_to_kb` / Module 1 scanner）走「rebuild in progress」 503。

**為何不選**：

- (X) 與其他 binding 切換同一 confirm modal（reuse `<InterventionConfirmModal>` 的 switch kind）：文案要硬扛 Embedding 特殊性（destructive），與 skip / regen / switch workspace 三個介入點語意不同層 — 共用 modal 反而模糊
- (X) 不 confirm 直接切：手滑點 KB rebuild = data loss（chunks 不會丟但 embedding 重算成本高），符合「destructive 須 confirm」原則
- (X) 切 embedding 不 rebuild KB（保留舊 vector）：embedding 模型不同 vector space 不可比，搜尋結果完全錯誤 — 必須 rebuild

**Rationale**：Embedding 切換是唯一 user explicit 觸發 KB rebuild 的入口（Module 1 重掃工作區是另一條路徑，那個本來就 destructive），文案明示「current N chunks → rebuild」讓使用者知道代價。

**Invariant**：

- `<EmbeddingChangeConfirmModal>` 與 `<InterventionConfirmModal>`（D-020 介入點）視覺上分流（不同 z-index 或不同顏色），避免混淆
- KB rebuild 期間 `/qa` / `/explore` / `/scan?stream=true` 一律 503 + `code: KB_REBUILD_IN_PROGRESS`；前端對應元件顯示 banner

### Decision 7: O-04 LLM Call Inspector — 顯示 provider id + filter PII detection

**選擇**：`<LlmCallInspector>` 既有 row 加一欄 `provider_id`（從 audit row `model` + `base_url` 反推或直接從新欄 `provider_id`）；filter dropdown 加「Hide PII detection calls」toggle，預設 ON；下方 banner「另有 N 筆 PII 偵測 call（toggle off 顯示）」摺疊；PII row 視覺差異化（淡灰色 + 🔍 icon）。`useAuditJsonl<LlmCallEntry>` 不變動 schema，新欄位透過 `LlmCallEntry` interface 加 `role: "pii_detection" | "reasoning" | "judge" | "chat" | "embed"` + `provider_id: string`（D-033 §B 對前端的影響 §3 預留 hook 在 A archive 時已 type 加好，本 change 接通 filter 邏輯）

**為何不選**：

- (X) 把 PII call 寫到單獨 jsonl：違反 D-033 不變式 3「PII LLM call 仍要 audit，共用 llm_calls.jsonl」
- (X) PII call 完全不顯示：使用者要查「為什麼這筆敏感字被偵測」沒地方看
- (X) PII 預設 ON 顯示：主 stream 雜訊太多（每筆使用者輸入過 PII 偵測就一筆）

**Rationale**：D-033 不變式 3 要求 audit + 默認分流顯示；toggle 預設 ON 守 UX 簡潔，banner 留可展開的入口守 transparency。

## Risks / Trade-offs

- **Risk: Tauri keyring 在無桌面環境 Linux 失敗** → Mitigation: keyring 操作 wrap try/except，失敗 fallback 到記憶體暫存 + 顯眼 banner「本次 session 後 API key 不保留，請手動加 GNOME Keyring / KWallet」；P0 不做完整 fallback PoC（Non-Goals 已記）
- **Risk: in-flight task 持有舊 Registry reference 但使用者已切 provider，audit 帶舊 provider id** → Mitigation: 接受此行為（in-flight task 跑完現場是 Decision 3 明示），llm_calls.jsonl 帶舊 provider id 是事實，非 bug；UI 顯示 provider id 即可讓使用者察覺
- **Risk: keyring set 失敗（OS keychain 拒絕）→ onboarding 卡住** → Mitigation: keyring IPC 失敗時前端顯示明確錯誤訊息（「無法寫入 OS keychain，請檢查權限」）+ 「跳過此 session」escape hatch（API key 只存記憶體，下次重啟要重輸）；不變式 5 仍守（沒 ready 不能進主畫面），但允許「就這次先用」單次例外
- **Risk: SSE event `provider_config_changed` 推送時前端剛好 disconnect** → Mitigation: 前端進 `/settings` 時主動 GET `/healthz` 拉一次 dependency snapshot，當作 source of truth；SSE 是 nice-to-have 即時 push，不是唯一 channel
- **Trade-off: setting page UI scope 收緊到「只配 provider id / model / base_url / api_key + role binding + PII mode」**：使用者要改 temperature / max_tokens / system prompt 仍要編輯 yaml — 可被 demo 期 power user 反映，但 P0 接受（Non-Goals 已記）
- **Trade-off: PII LLM provider 不在 onboarding**：使用者要切 LLM PII 必須先進主畫面才看得到 setting → 違反「沒 ready 不能進」嚴格解釋；但 PII rule-based 預設 ready，只是「pii: rule」狀態，不算 not-configured，所以實際進得了主畫面 — 這個邊界值得 setting page 文案明示

## Migration Plan

A archive 後 sidecar config 仍是舊一對一 schema（`llm.roles.<role>.provider_id`）。本 change apply 期：

1. **Schema migration**：`config/provider_pool.py` 加 loader 兼容兩種 schema — 偵測到舊 schema（`llm.roles` 存在）→ 自動轉成新格式 in-memory（pseudo `providers[]` 從 roles 反推）+ console warning「請改用新 schema」；新 schema（`llm.providers[]` + `llm.bindings`）走原生 path
2. **既有 fixture / test config**：sidecar 既有 ~885 個 pytest 多用 mock provider，不依賴實 config schema；保留現況
3. **Tauri 啟動流程改動**：先 PoC keyring（macOS / Windows / Linux 三平台 happy path），驗證後才動 `tauri/src-tauri/src/sidecar.rs` 啟動順序；PoC 失敗回頭看 Decision 1 替代方案
4. **Rollback strategy**：本 change 純加新功能 + 改 schema loader（向後相容），rollback = revert commit；keyring 寫進去的值留在 OS keychain（手動清）— 屬使用者資產不刪
5. **Feature flag 不需要**：onboarding wizard 是新路由，setting page 是新路由，沒舊行為要切；keyring 是新依賴，bundle 後跟 app 一起發

## Open Questions

- (apply 期解) Tauri 2.0 的 `tauri-plugin-keyring` 與直接用 `keyring-rs` crate 哪個更穩 — PoC 期間在三平台跑 happy path 後拍板
- (apply 期解) `provider_config_changed` SSE event 要不要也走 `task_id` 機制（讓前端 `useSseTask` 能訂閱）— 預設用 app-level SSE channel（不是 per-task），但 sidecar 目前沒這個 pattern；可能要加 `/events?channel=app` endpoint
- (P1) Linux 無桌面環境的 keyring fallback：D-033 §B 開放問題 3，留打磨期再評估；可能需要選 `pass` (gpg-based) 或寫到 plain-text + 警告
- (P1+) Multi-account onboarding：使用者有兩個 OpenAI 帳號（個人 / 公司）想切換 — 目前 setting page 是 single-account 模型，未來要不要加 account profile 概念
- (P1+) Cloud sync setting：keyring 走 macOS iCloud Keychain 自動同步，但 Windows / Linux 沒對等機制 — 跨 device 一致性留後續
