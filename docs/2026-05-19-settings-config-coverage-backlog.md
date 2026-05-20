# Backlog: Settings 設定面板完整化（config ↔ app Settings 覆蓋盤點）

**Date:** 2026-05-19
**Surfaced during:** roadmap review，user 要求窮舉所有 config 並對齊 app Settings 露出狀況
**Severity:** UX gap（設定不透明 / 部分 knob 無 GUI 入口）
**Owner:** harry
**Status:** Change 1 ✅ ship（`settings-config-frontend`, 2026-05-20）；Change 2 todo（user 2026-05-20 確認要做：「便宜出 + 貴審」motivation 成立）

---

## 討論結論（2026-05-19 /spectra-discuss）

切 **2 條 change**：

- **Change 1（純前端，輕，無依賴，先做）**：分區一 5 欄位
  （`pii.on_hit` / `lint.fix.enabled` / `quiz.content_verify` /
  `goal.content_verify` / log 關閉）+ `pii.patterns_extra`（**純 regex
  字串列表、無 label**，對齊 `config/pii.rs` 實作）+ chat 唯讀列
  （方案 A：EndpointSection 顯示「沿用 query（現：…）」，不改 schema）。
  backend 不動（`save_global_config` 已 round-trip 保留未知 section）。
  **不併入 F polish-ship**（保持 F 的 release-gate 焦點）。
- **Change 2（跨層，中，接 Change 1 後）**：verify 階段獨立 model。
  schema 加 `Verb::Verify` + `claude_code.system/azure.verify`，**單一
  共用設定**（不分 quiz-verify / goal-verify）；**只套驗證 spawn，
  repair 維持 quiz/goal model**；EndpointSection 多一列可編輯；
  `verb/quiz.rs` + goal verify path 改 `resolve(Verb::Verify)`。
  動機：user 要能獨立控制、且可能設**更強/更貴** model 把關
  （便宜 model 出題 + 貴 model 審查）。

決策修正：
- 原分區二「verify 獨立 model = 暫不做」**已推翻**，user 有明確成本控制動機 → 列為 Change 2 正式做。
- chat 確認**不需**獨立 model（user：「不用」）→ 永遠方案 A。
- `CODEBUS_CLAUDE_BIN` 確認不做，留 backlog。

---

## 觀察

對全 codebase 做過一次 config 來源窮舉（CLI + core + app + env + vault 檔）。
`~/.codebus/config.yaml` 裡使用者會想在 Settings 調的 knob，與 app
`SettingsModal.tsx` / `EndpointSection.tsx` 現況對照如下：

| Namespace.key | 預設 | app Settings 現況 |
|---|---|---|
| `claude_code.active` + system/azure 每 verb model/effort + base_url + keyring_service | system / 內嵌 | ✅ EndpointSection 完整 |
| `claude_code` Azure API key（OS keyring，非 yaml） | 無 | ✅ keyring 管理 UI |
| `pii.scanner` | regex_basic | ✅ 有（regex_basic / none） |
| `pii.patterns_extra` | `[]` | ❌ 缺 → 既有 `pii-settings-ui` |
| `pii.on_hit` | warn | ❌ 缺（**無人追蹤**） |
| `lint.fix.enabled` | true | ❌ 缺（**無人追蹤**） |
| `quiz.default_length` | 5 | ✅ 有（slider 3–10） |
| `quiz.content_verify` | false | ❌ 缺（**無人追蹤**） |
| `goal.content_verify` | false | ❌ 缺（**無人追蹤**） |
| `log.dir`（+sink jsonl） | per-vault | ✅ 有（目錄選擇器） |
| `log` 完全關（`sink: none`） | — | ❌ 缺（**無人追蹤**） |
| `app.quiz.pass_threshold` | 80 | ✅ 有（slider 50–100） |
| chat verb 專屬 model/effort | 重用 query | ❌ 缺 → 既有 `settings-chat-model` |
| verify 階段專屬 model | 重用 quiz/goal | ❌ 連 config key 都沒有（**設計題，新**） |

非 yaml 設定來源（盤點完整性，列出但多數不該進 Settings UI）：

- env：`CODEBUS_HOME` / `CODEBUS_AZURE_KEY` / `NO_COLOR` → 環境層，不做 UI；
  `CODEBUS_CLAUDE_BIN`（自訂 claude 執行檔路徑，預設 `claude`）→ **可選**做一欄
- `<repo>/.codebus/CLAUDE.md` per-repo schema → user 可編，但目前只能靠外部
  編輯器，app **無 UI**（獨立議題，先記不展開）
- `~/.codebus/app-state.json`（vault 清單）→ app 自管，已有加/移除 vault UI，
  不需 Settings
- 終端 emoji/color/hyperlink → 純環境偵測，**已確認無 config key**（舊 roadmap
  的 `emoji:` 已砍，現況驗證過，不要再找）

## 與既有 backlog 的關係（避免重複追蹤）

本條是**盤點 + 收斂入口**，不重做以下兩條已存在的：

- **`pii-settings-ui`（2026-05-14）** — 涵蓋 `pii` 自訂 regex。
  ⚠️ **該文件 schema 寫錯**：提案寫 `pii.extra_rules:` 為 `{label, pattern}`
  物件陣列，但**實作的 config key 是 `pii.patterns_extra:` 純 regex 字串
  陣列**（見 `codebus-core/src/config/pii.rs`）。動工前必須對齊：要嘛 UI 直接
  存 `patterns_extra`（無 label，最省），要嘛擴 schema 加 label（破壞既有
  CLI 讀取，需評估）。此 discrepancy 記在這裡，動 `pii-settings-ui` 前先看。
- **`settings-chat-model`（2026-05-14）** — 涵蓋 chat verb model/effort
  （方案 A read-only hint / 方案 B 獨立 config section）。不在本條重做。

## 分區一：簡單前端補完（backend 已 passthrough，純 UI）

`save_global_config` 已能保留未知 section round-trip（見
`codebus-app/src-tauri/src/ipc/config.rs` 測試 `round_trip_preserves_unknown_sections`），
所以以下全部**不需動 backend**，只在 `SettingsModal.tsx` 加輸入元件：

| 欄位 | 控件型態 | 備註 |
|---|---|---|
| `pii.on_hit` | Select：warn / skip / mask | 需 UI 文案說明 Critical 級**不受此影響**（security floor 永遠 mask），別讓 user 以為能關 |
| `lint.fix.enabled` | Toggle | 單純布林 |
| `quiz.content_verify` | Toggle | 開啟會多花 verify/repair spawn（成本提示） |
| `goal.content_verify` | Toggle | 同上 |
| `log` 完全關 | 把現有 log 區塊加一個「停用 logging」選項 → 寫 `sink: none` | 目前只能設 dir，無法整個關 |
| `CODEBUS_CLAUDE_BIN`（可選） | Text 輸入 | 嚴格說是 env 不是 yaml；要做需先決定落點（env 不持久；可能改成 config key，屬小設計題，非純 UI——若要做拆到分區二） |

`pii.patterns_extra` 不列在這分區（歸 `pii-settings-ui`，且有上述 schema
discrepancy 要先解）。

### Tasks（分區一，粗估）

1. `SettingsModal.tsx`：加 `pii.on_hit` Select（含 Critical-floor 說明文案）
2. 加 `lint.fix.enabled` Toggle
3. 加 `quiz.content_verify` / `goal.content_verify` 兩個 Toggle（含成本提示）
4. log 區塊加「停用」選項（寫 `log.sink: none`）+ ResetButton 邏輯調整
5. i18n 字串補（zh-tw）
6. vitest：每個新控件 update→dirty→save payload 正確；reset 行為
7. SettingsModal snapshot 更新

工程量：輕-中（1-2 個半天，全前端）。

## 分區二：schema / 設計題（不是補 UI 就好）

| 項目 | 為什麼是設計題 | 對應 |
|---|---|---|
| chat 專屬 model/effort | 需新增 `claude_code.*.chat` section + `chat.rs` 改讀 `Verb::Chat`（目前 fallback query） | 既有 `settings-chat-model` 方案 B |
| verify 階段專屬 model | 目前 verify/repair spawn 重用 `Verb::Quiz`(=Query) / goal 的 model（`verb/quiz.rs:574,616` `&resolved`）。要讓 user 單獨選需新 Verb variant + 新 config section。先要回答：verify 值不值得獨立 model（便宜 model 驗證 vs 同一個）？ | 本條新增，無既有追蹤 |

兩項都牽動 `codebus-core` config schema + `EndpointSection` VERBS 陣列 +
ipc.ts interface + 測試，非純前端。建議走 `/spectra-discuss` 先收斂「verify
獨立 model 要不要做、chat 走方案 A 還 B」，再決定是否併入 `settings-chat-model`
一起 propose。

### 2026-05-20 discuss 收斂：verify 階段獨立 model（Change 2 詳細設計）

User 確認動機「便宜出 + 貴審」成立，5 條 assumption 全 confirmed。對應的具體設計：

**Schema：**

- `Verb` enum（[`codebus-core/src/config/claude_code.rs:28-44`](../codebus-core/src/config/claude_code.rs)）新增 `Verify` 變體
- `SystemProfile` / `AzureProfile`（[`endpoint.rs:93-138`](../codebus-core/src/config/endpoint.rs)）加 `verify: SystemVerbConfig` / `AzureVerbConfig` 欄位
- `RawSystemProfile` / `RawAzureProfile`（line 192-214）的 raw 解析加 `verify: Option<...>`
- `validate_system_profile` / `validate_azure_profile`（line 264-352）各加一條 `require_…("claude_code.system.verify: required when active=system")`
- `SystemProfile::default()`（line 100-117）加 `verify: { model: opus-4-6, effort: high }` —— 最強 reasoning model + 最高 effort，符合「貴審」動機
- `resolve()`（[`claude_code.rs:58-95`](../codebus-core/src/config/claude_code.rs)）match arm 加 `Verb::Verify => &self.system.verify` / `&az.verify`
- `STARTER_CONFIG`（[`global_starter.rs:36-131`](../codebus-core/src/config/global_starter.rs)）加 `verify:` block + 註解；`starter_round_trips_to_defaults` 測試自動 cover

**Required in active profile（不走 optional fallback）：**

- 失敗模式：existing user yaml 沒 `verify:` → `ConfigLoadError::YamlParse` codebus 不開
- 理由：(1) `fail-loud-on-config-parse-error` 是專案 philosophy；(2) cost surprise（默默升 opus）是更壞的失敗模式；(3) 加一行 yaml 成本 < 帳單暴增成本
- **必附 migration doc**（`docs/2026-05-XX-settings-verify-model-migration.md`），格式仿 `2026-05-20-pretooluse-image-block-migration.md`：明確 yaml snippet + 預期 cost 提醒 + re-init 替代選項
- 同步更新 release note / wiki 對外指引

**Verify spawn 切換點（共 2 處，rest 不動）：**

| 檔案 | 行 | 動作 |
|---|---|---|
| `verb/quiz.rs:318` | 加 `let verify_resolved = cc_cfg.resolve(Verb::Verify);` 旁邊既有 `resolved` |
| `verb/quiz.rs:568-574` | verify closure 的 `run_spawn` 改傳 `&verify_resolved` |
| `verb/quiz.rs:403, 502, 610` | plan / generate / repair spawn 全部維持 `&resolved`（Verb::Quiz） |
| `verb/goal.rs:300` | 加 `let verify_resolved = cc_cfg.resolve(Verb::Verify);` 旁邊既有 `goal_resolved` |
| `verb/goal.rs:463-471` | verify closure 的 `run_goal_spawn` 改用 `verify_resolved.model.clone() / .effort.clone()` |
| `verb/goal.rs:328-329, 506-511` | 主 spawn / repair spawn 維持 `goal_resolved` |

**RunLog 不紀錄 verify model**（明確 decision）：

`goal.rs:354/423` 跟 `quiz.rs:664-665` 寫進 RunLog 的 model 欄維持 main spawn（Verb::Goal / Verb::Quiz）。verify 是內部 sub-spawn，不污染「一 verb 一 row」契約。trade-off：user 看 log 不知道 verify 用了什麼 model；接受，因為改 RunLog schema 是另一條 change（scope creep）。

**EndpointSection 自動 render（不動 template）：**

- [`EndpointSection.tsx:48`](../codebus-app/src/components/settings/EndpointSection.tsx) `VERBS = ["goal","query","fix"]` → 加 `"verify"`，第 4 個 row 自動 render 為可編輯（跟 chat 的 read-only hint 不同）
- `SYSTEM_PROFILE_DEFAULTS`（前端對應 SystemProfile::default()）加 `verify: { model: "opus-4-6", effort: "high" }`
- `ipc.ts` `SystemProfile` / `AzureProfile` interface 加 `verify` 欄位
- i18n：zh-tw label 「驗證階段」+ tooltip「建議用 reasoning 強的 model 把關 quiz/goal 寫的內容」

**測試動到的：**

- `endpoint.rs` 既有 parse tests（valid system / valid azure / missing verb / unversioned model 等）—— 都要加 `verify` 進有效 yaml 樣本；不加會 fail
- `starter_round_trips_to_defaults` —— SystemProfile::default 跟 STARTER_CONFIG 必須同步加 verify，否則 round-trip 失敗
- `claude_code.rs` 加 `resolve(Verb::Verify)` 的測試
- `verb/quiz.rs` + `verb/goal.rs` 加測試確認 verify spawn 拿到的 model = `verify_resolved.model` 而非 `resolved.model`（用 mock cc_cfg 設不同值）
- `EndpointSection.test.tsx` 第 4 row 渲染 + 互動測試
- `SettingsModal.test.tsx` 整合測試 round-trip 含 verify

**建議 change 名：** `verify-stage-independent-model`（強調「verify 階段獨立」而不是「settings UI」—— scope 跨 backend schema + frontend UI 兩端）

**Out of scope（本 change 不做）：**

- chat 走方案 B（chat 獨立 model/effort）—— user 2026-05-19 確認方案 A 沿用 query 即可，不需 chat 獨立
- RunLog 加 `verify_model` 欄 —— 維持「一 verb 一 row」契約
- 多筆 RunLog per verb invocation —— 同上
- Repair / Generate / Plan spawn 切換 —— backlog 明確只套 verify spawn
- 既有 vault 自動 migrate config.yaml —— release note 引導手動加

## Out of scope

- per-repo `.codebus/CLAUDE.md` 的 GUI 編輯器（獨立大議題，本條不含）
- env 變數（`CODEBUS_HOME` 等）做成 Settings（環境層，不適合）
- 終端樣式 config 化（已確認設計上刻意移除，不復活）

## 何時動

- 分區一：低風險純前端，可在 F `v3-app-polish-ship` 內順手做，或之前獨立小 change
- 分區二 + `pii-settings-ui` schema 對齊 + `settings-chat-model`：建議
  `/spectra-discuss` 一次把「Settings 設定面板要做到多完整」收斂，再決定切幾條
  change。不要各自零碎 propose（會重蹈分散追蹤）
