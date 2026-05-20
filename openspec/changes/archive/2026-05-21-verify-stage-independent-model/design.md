## Context

Stage 4 三條 `*-content-verify` change（`quiz-validate-repair` / `quiz-content-verify` / `goal-content-verify`）讓 quiz 與 goal 生成 wiki / quiz md 後跑 **獨立 spawn 驗證內容是否 hallucination**。但目前 verify spawn 用的 model / effort **沿用該 verb 自己的設定**：

- Quiz verify spawn 走 `Verb::Quiz`，而 `Verb::Quiz` 在 `claude-code-config` resolve 中沿用 `Verb::Query`（=haiku-4-5 / low 預設）
- Goal verify spawn 走 `Verb::Goal`（=opus-4-6 / high 預設）

意味著：

- 用 haiku 出 quiz 時，verify 也是 haiku —— reasoning 強度跟出題一樣，hallucination 抓不到
- 用 opus 跑 goal 時，verify 也是 opus —— 每跑一條 goal 雙倍燒高價 model，沒有「便宜出 + 貴審」的 cost saving

User 的成本控制 motivation：**verify 階段獨立配 model**，常見配法是 quiz 出題 haiku、verify 用 opus（強 reasoning 把關）；或 goal 寫 wiki sonnet、verify 用 opus（極致 reasoning）。

2026-05-19 `/spectra-discuss settings-config-coverage` 已收斂走 Change 2 動工。2026-05-20 進一步 discuss 確認 5 條 design assumption 與 2 個額外 finding（RunLog 不紀錄 / migration doc）。詳見 `docs/2026-05-19-settings-config-coverage-backlog.md` 分區二「2026-05-20 discuss 收斂」段落。

既有架構 anchor：

- `Verb` enum：`codebus-core/src/config/claude_code.rs` 的 5 變體
- `SystemProfile` / `AzureProfile` struct：`codebus-core/src/config/endpoint.rs`
- `resolve()` 函數：同上 claude_code.rs，目前 chat / quiz 都 fallback 到 query 子塊
- Verify spawn 兩處：`codebus-core/src/verb/quiz.rs` 的 content verify closure 與 `codebus-core/src/verb/goal.rs` 的 verify closure
- Settings UI：`codebus-app/src/components/settings/EndpointSection.tsx` 用 `VERBS = ["goal","query","fix"]` 陣列 loop render 三個 verb row

## Goals / Non-Goals

**Goals:**

- Verify 階段（quiz 跟 goal 共用）可獨立配 model 與 effort
- 預設值傾向「貴審」（最強 reasoning model + 最高 effort），符合 motivation；user 想用便宜 verify 仍可手動改 yaml
- Settings UI 提供可編輯的 verify row，與既有 goal/query/fix row 一致 UX
- 既有測試契約（fail-loud parse、starter round-trip、effort enum validation）保持不變
- 既有 user 升級路徑明確：fail-loud parse error + migration doc 引導手動加 yaml 區塊

**Non-Goals:**

- chat 獨立 model（user 確認沿用 query 即可）
- RunLog 加 `verify_model` / `verify_effort` 欄
- 多筆 RunLog per verb invocation（一 verb 一 row）
- Repair / generate / plan spawn 切換到 verify model
- 既有 vault 自動 migrate config.yaml（保持 write-if-missing byte-identical 契約）
- Optional fallback（缺 verify 時 fallback 到 query 或 verb 自己）—— 違反 fail-loud philosophy 且讓預設「貴審」動機消失
- 拆分 quiz-verify 與 goal-verify 為兩個 enum 變體（backlog 明確單一共用）

## Decisions

### `Verb::Verify` 變體 + `system.verify` / `azure.verify` config block

新增 closed enum 變體 `Verb::Verify`，跟既有 `Goal / Query / Fix / Chat / Quiz` 並列。對應的 `SystemProfile` 與 `AzureProfile` struct 加 `verify: SystemVerbConfig` / `AzureVerbConfig` 欄位（型別跟既有 goal/query/fix 相同）。`resolve()` 函數的 match arm 加 `Verb::Verify => &self.system.verify` 或 `&az.verify`，**不走 fallback 到 query 或其他 verb**。

Alternatives considered：

- **fallback 到 Verb::Query**：跟既有 chat / quiz 模式對稱，但讓預設 opus-4-6/high「貴審」設計失效 —— rejected
- **Contextual `resolve_verify(parent_verb: Verb)`**：API contortion，違反 closed-enum + 直接 reference 既有設計 —— rejected
- **拆 `Verb::QuizVerify` / `Verb::GoalVerify`**：user 要同時調兩組，違反「單一共用」motivation —— rejected

### Required in active profile（不採 optional fallback）

`verify` 子塊在 active profile 內**必填**（跟既有 goal / query / fix 同等級）。`validate_system_profile` / `validate_azure_profile` 中加 require 條目，缺則回傳 `ConfigLoadError::YamlParse`。非 active profile 維持寬鬆（沿用既有 cold storage 語意）。

Alternatives considered：

- **Optional + 預設 opus-4-6/high**：load 不會 fail，但既有 user yaml 沒填 verify 時下次跑 quiz/goal 突然燒 opus，**cost surprise** —— rejected
- **Optional + fallback to verb's own model**：load 不會 fail，cost 行為跟現況一致，但完全失去本 change 的 motivation（user 改了 verify 才有效；忘了改就沒效，且 default 行為等同沒做） —— rejected
- **Required + 提供新 CLI 子旗標自動補 verify**：破壞 `write_starter_config_if_missing` 既有 byte-identical 契約，scope creep —— rejected；改走 release note + 手動 yaml 加區塊（更透明，user 自己 informed）

Rationale：`fail-loud-on-config-parse-error` 是專案明確 philosophy（2026-05-12 archived change）；加一行 yaml 的成本比 cost surprise 的 surprise 小很多；且 fail-loud 的 parse error message 會直接點出「verify 必填」，比 release note 引導性更強。

### 預設值 `opus-4-6 / high`

`SystemProfile::default().verify` 設 `{ model: SystemModel::Opus4_6, effort: "high" }`。`STARTER_CONFIG` 對應註解標明 verify 區塊與動機（最強 reasoning 把關 quiz/goal 寫出的內容）。

Alternatives considered：

- **`opus-4-7`（最新 model）**：成本更高且 effort=high 已經是 reasoning 最強檔位，多花的 cost 邊際效益低 —— rejected
- **`sonnet-4-6 / medium`**：中庸折衷，但放棄「貴審」訊號；user 想省錢可自己改 yaml，預設值表達設計意圖 —— rejected
- **跟 goal 的 default 對齊（opus-4-6 / high）**：實際就是現在選的；恰好 goal default 也是同設定，consistency 上 OK

### Verify spawn 切換點：2 處，其他 spawn 全部不動

| 檔案 | 變更 |
|---|---|
| quiz.rs（content verify closure） | 既有 `cc_cfg.resolve(Verb::Quiz)` 旁加 `verify_resolved = cc_cfg.resolve(Verb::Verify)`。verify closure 內的 spawn 改傳 `&verify_resolved` |
| goal.rs（content verify closure） | 既有 `cc_cfg.resolve(Verb::Goal)` 旁加 `verify_resolved = cc_cfg.resolve(Verb::Verify)`。verify closure 內的 spawn 改用 `verify_resolved.model.clone()` / `verify_resolved.effort.clone()` |
| 其他 spawn（quiz plan / quiz generate / quiz repair / goal main / goal repair / fix） | **全部不動**，繼續用各自的 resolved 變數（Verb::Quiz / Verb::Goal / Verb::Fix） |

Alternatives considered：

- **Repair 也用 Verb::Verify**：repair 是「修正 verify 找到的缺陷」，如果 verify 用 opus 找出 defect、repair 也用 opus 修，意味同次 verb 跑兩次 opus 燒兩倍 token。Backlog 明確：只套 verify、不套 repair —— rejected

### RunLog 不紀錄 verify model

`goal.rs` 與 `quiz.rs` 寫 RunLog 時的 `model` 欄維持紀錄主 spawn（goal / quiz）的 model，**verify 不額外紀錄**。

Alternatives considered：

- **RunLog 加 `verify_model` 欄**：透明度更好，但動 RunLog spec 是另一條 change 的 scope；且 `events.jsonl` 紀錄每個 spawn 的 SpawnStart 事件含 model 資訊，user 想知道 verify model 可從 events 查到 —— rejected for scope
- **多筆 RunLog（每個 spawn 一筆）**：破壞既有「一 verb invocation 一 row」契約，破壞性大 —— rejected

Rationale：RunLog 語義是「user 主動觸發的 verb 用了什麼 model」，verify 是內部 sub-spawn 不該污染主紀錄。透明度需求由 events.jsonl 補強。

### Settings UI 自動 render（VERBS 陣列擴增）

`EndpointSection.tsx` 的 `VERBS` 陣列加 `"verify"`，render loop 自動產生第 4 個 row。`SYSTEM_PROFILE_DEFAULTS` 同步加 verify 預設值。`ipc.ts` 的 `SystemProfile` / `AzureProfile` interface 加 verify 欄位（型別跟既有相同）。`i18n/messages.ts` 加 zh-tw + en 的 verify row label 與 tooltip。

Chat row（既有 read-only hint 在 loop 外面）**不動**。

Alternatives considered：

- **手動加 verify row 為獨立 JSX block**：跟 chat row 一樣 outside loop，但少了 loop 的 DRY 好處且需要重複 4 段 UI（model select / effort select / aria-invalid wiring / validation hook） —— rejected
- **把 chat row 也搬進 VERBS loop（chat 也可編輯）**：scope creep，violates Non-Goal —— rejected

## Implementation Contract

**Behavior:**

User 在 `~/.codebus/config.yaml` 為 `claude_code.system.verify`（active=system）或 `claude_code.azure.verify`（active=azure）設定 model 與 effort 後，每次跑 `codebus quiz` 或 `codebus goal` 觸發 content verify 階段時，verify spawn 使用該 verify 子塊的 model / effort，**獨立於主 verb 的設定**。Quiz repair 與 generate 持續用 Verb::Quiz 對應的 model；Goal main spawn 與 repair 持續用 Verb::Goal 對應的 model。

既有 user 第一次跑 codebus（升級後 yaml 沒 verify 區塊）：parse fails fast with `ConfigLoadError::YamlParse`，stderr 訊息明確指出 verify 必填（system 與 azure 對應 active profile 各自錯誤訊息）。User 加 yaml 區塊後正常運作。

`codebus init` 第一次寫 `~/.codebus/config.yaml`（檔不存在）時，STARTER_CONFIG 已含註解齊全的 verify 預設區塊 `{ model: opus-4-6, effort: high }`。

Settings UI Endpoint Section 顯示 4 個可編輯 verb row（goal / query / fix / verify），跟既有 3 row 操作一致。chat 仍是獨立 read-only hint row。

**Interface / data shape:**

- `Verb` enum（`codebus-core/src/config/claude_code.rs`）：closed 6 變體 `Goal / Query / Fix / Chat / Quiz / Verify`，serde rename_all snake_case 維持
- `SystemProfile` struct（`endpoint.rs`）：4 欄 `goal / query / fix / verify`，全 required
- `AzureProfile` struct：4 欄 verb + `base_url` + `keyring_service`，全 required（active=azure 時）
- `RawSystemProfile` / `RawAzureProfile` raw deserialization：4 個 `Option<*VerbConfig>` 欄
- `resolve(Verb::Verify)` → 回傳新子塊的 model / effort
- yaml schema 形狀（active=system 範例）：

  ```
  claude_code:
    active: system
    system:
      goal:   { model: opus-4-6,   effort: high   }
      query:  { model: haiku-4-5,  effort: low    }
      fix:    { model: sonnet-4-6, effort: medium }
      verify: { model: opus-4-6,   effort: high   }
  ```

- `EndpointSection.tsx` 的 `VERBS` 常數：`["goal","query","fix","verify"] as const`
- `ipc.ts` `SystemProfile` / `AzureProfile` interface 加 `verify: { model: string; effort: string }`
- `SYSTEM_PROFILE_DEFAULTS.verify = { model: "opus-4-6", effort: "high" }`

**Failure modes:**

- yaml 缺 `claude_code.system.verify`（active=system）→ `ConfigLoadError::YamlParse` 含明確 field path 訊息；codebus 不開
- yaml 缺 `claude_code.azure.verify`（active=azure）→ 同上
- yaml `verify.model` 是 system 模式下的非 enum 值（如 `opus`、`gpt-4`）→ 既有 `SystemModel` 解析 error 行為（fail-loud parse error），訊息指出有效 enum 值
- Settings UI verify row effort 拉到 enum 外的值 → 既有 effort enum validation 邏輯自動 cover（aria-invalid + Save disabled）
- 非 active profile 缺 `verify` 欄 → 沿用既有寬鬆 cold storage 語意，不 error

**Acceptance criteria:**

- `codebus-core` 單元測試：
  - `endpoint.rs` 既有 parse tests 全綠（加 verify 進 valid yaml 樣本）
  - 新測試：`active=system + system.verify 缺` → `ConfigLoadError::YamlParse`
  - 新測試：`active=azure + azure.verify 缺` → `ConfigLoadError::YamlParse`
  - 新測試：`active=system + 只有 azure.verify 缺` → 解析成功（cold storage 寬鬆）
  - `claude_code.rs` 新測試：`resolve(Verb::Verify)` 回傳 system.verify / azure.verify 對應值
  - `global_starter.rs` 既有 `starter_round_trips_to_defaults` 全綠（STARTER_CONFIG 跟 SystemProfile::default 同步加 verify）
- `verb/quiz.rs` 新單元測試：mock cc_cfg 設 system.quiz.model=haiku + system.verify.model=opus，跑 quiz 流程，assert verify spawn 收到的 model = opus，plan/generate/repair spawn 收到的 model = haiku
- `verb/goal.rs` 新單元測試：mock cc_cfg 設 system.goal.model=sonnet + system.verify.model=opus，跑 goal 流程，assert verify spawn 收到 opus，main / repair spawn 收到 sonnet
- `codebus-app` 前端測試：
  - `EndpointSection.test.tsx` 加 4-row 渲染測試（verify row 與 goal/query/fix row 行為一致：model select 4 options + effort select 6 options + aria-invalid wiring）
  - `SettingsModal.test.tsx` 加 round-trip 整合測試（含 verify 欄）
- 既有 `effort` enum validation 既有測試對 verify row 同樣 cover（aria-invalid + Save disabled）
- Migration doc `docs/2026-05-XX-verify-stage-independent-model-migration.md` 存在且 JSON snippet 與 STARTER_CONFIG 的 verify 區塊文字相符（manual review）

**Scope boundaries:**

In scope：

- `Verb::Verify` 變體 + `system.verify` / `azure.verify` config 子塊 schema
- Quiz verify spawn 與 goal verify spawn 兩處切換到 `verify_resolved`
- STARTER_CONFIG 加 verify 區塊
- `SystemProfile::default()` 加 verify 預設
- EndpointSection VERBS 陣列擴增 + 對應 i18n
- ipc.ts `SystemProfile` / `AzureProfile` interface 擴增 + `SYSTEM_PROFILE_DEFAULTS` 對應更新
- 全部對應測試
- Migration doc

Out of scope：

- Chat 獨立 model（保持 settings-chat-model 方案 A read-only hint）
- RunLog schema 改動（保持「一 verb 一 row」契約）
- Quiz repair / generate / plan 與 Goal main / repair spawn 的 model 切換
- 自動 migrate 既有 vault config.yaml
- 新增 CLI migration 旗標

## Risks / Trade-offs

- [既有 user yaml 升級時 codebus 不開] → Mitigation: migration doc 含 yaml snippet + 預期 cost 提醒 + re-init 替代選項，沿用 pretooluse-image-block-migration.md 既有 pattern；release note / wiki 對外指引同步更新
- [預設 opus-4-6 + high 對個人 user cost 變高] → Mitigation: 預設值是「設計意圖 baseline」，user 想省錢可自己改 yaml 改 verify 為 haiku-4-5 / low；migration doc 明確列出 cost 行為
- [User 改 verify=haiku 跟 quiz=haiku 一樣，等於沒做] → 接受：本 change 提供 knob，不強制；user 自行判斷
- [Quiz 流程內 cc_cfg.resolve 多呼叫一次有效能影響] → 可忽略：resolve 是純 struct field 查找，O(1) 無 IO；測試比較變數值不會碰到效能差異
- [新 `Verb::Verify` 變體破壞既有跨 crate `Verb` 序列化（serde rename_all）] → Mitigation: variant 名 `Verify` → kebab `verify`，跟既有 `quiz` / `chat` 同 pattern；無向前 / 向後相容問題，因為 `Verb` 只在 spawn 過程內部使用、不持久化進任何外部 schema
