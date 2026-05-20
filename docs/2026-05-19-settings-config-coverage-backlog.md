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

## Out of scope

- per-repo `.codebus/CLAUDE.md` 的 GUI 編輯器（獨立大議題，本條不含）
- env 變數（`CODEBUS_HOME` 等）做成 Settings（環境層，不適合）
- 終端樣式 config 化（已確認設計上刻意移除，不復活）

## 何時動

- 分區一：低風險純前端，可在 F `v3-app-polish-ship` 內順手做，或之前獨立小 change
- 分區二 + `pii-settings-ui` schema 對齊 + `settings-chat-model`：建議
  `/spectra-discuss` 一次把「Settings 設定面板要做到多完整」收斂，再決定切幾條
  change。不要各自零碎 propose（會重蹈分散追蹤）
