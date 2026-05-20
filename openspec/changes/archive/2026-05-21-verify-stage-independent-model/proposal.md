## Why

Quiz 跟 goal 兩個 verb 在生成階段後都會跑 content verify spawn（`quiz-content-verify` / `goal-content-verify` Stage 4 已 ship），但 verify spawn 目前**重用該 verb 自己的 model**：quiz verify 用 `Verb::Quiz`（= 沿用 query 的 haiku）、goal verify 用 `Verb::Goal`（= opus）。User 沒辦法配「便宜 model 出題 / 寫 wiki + 貴 model 把關」這種成本敏感且 reasoning-strong 的 verify 設定。

最常見的使用情境是 quiz 走 haiku 出題（快、便宜），verify 想用 opus 抓 hallucination（reasoning 強）；但目前 quiz verify 也是 haiku，verify 強度跟出題一樣，意義有限。本 change 讓 verify 階段可獨立配 model 跟 effort，把「便宜出 + 貴審」變成可調 knob。

## What Changes

- `Verb` enum 新增 `Verify` 變體，跟既有 `Goal` / `Query` / `Fix` / `Chat` / `Quiz` 並列
- `~/.codebus/config.yaml` `claude_code.system` 與 `claude_code.azure` 各新增 `verify` 子塊（model + effort）
- `verify` 子塊**在 active profile 內必填**（沿用 goal/query/fix 既有 required 模式），預設值 `{ model: opus-4-6, effort: high }` —— 最強 reasoning + 最高 effort，符合「貴審」動機
- `cc_cfg.resolve(Verb::Verify)` 回傳新子塊的解析結果（不再 fallback 到 query 或 verb 自己）
- Quiz verify spawn（`quiz.rs` content verify 階段）改用 `resolve(Verb::Verify)` 結果；repair / generate / plan spawn 維持 `Verb::Quiz`
- Goal verify spawn（`goal.rs` content verify 階段）改用 `resolve(Verb::Verify)` 結果；repair / main spawn 維持 `Verb::Goal`
- RunLog 維持現況 —— 一 verb 一 row，model 欄紀錄主 spawn（goal/quiz）的 model，verify 是內部 sub-spawn 不污染主紀錄
- Settings UI `EndpointSection` 加第 4 個可編輯 row（verify），跟既有 goal/query/fix 同一個 VERBS loop 自動 render；chat row 維持既有 read-only hint 不動
- `STARTER_CONFIG`（`codebus init` 寫的 `~/.codebus/config.yaml` 樣板）加 `verify:` 註解區塊
- 既有 user yaml 沒 `verify:` → 走既有 fail-loud parse error 行為（`fail-loud-on-config-parse-error` philosophy）；附 migration doc 指引手動加 yaml snippet

## Non-Goals

- **chat 獨立 model**：user 2026-05-19 confirmed chat 沿用 query 即可（settings-chat-model 方案 A），不在本 change 範圍
- **RunLog 加 `verify_model` 欄**：維持「一 verb invocation 一 RunLog row」契約；verify model 透明度透過 `events.jsonl` 即可間接看到（events 紀錄每個 spawn）
- **Repair / Generate / Plan spawn 切換**：backlog 明確只套 verify spawn，repair 維持 quiz/goal 的 model 避免 verify-repair 用同個強 model 雙倍燒錢
- **既有 vault 自動 migrate config.yaml**：保持 `write_starter_config_if_missing` 既有 if-missing 語意；release note 引導手動加 yaml 區塊或刪檔 re-init
- **Optional fallback（缺 verify 時用 query / 用 verb 自己）**：違反 fail-loud philosophy；且預設值 opus-4-6 在 fallback 模式下會被掩蓋成「沿用既有 verb model」，動機完全消失
- **新增 `codebus init --migrate-hooks` / `--migrate-config` 子旗標**：破壞 `write_starter_config_if_missing` byte-identical 契約，scope creep
- **獨立 quiz-verify 與 goal-verify 配置**：backlog 明確「單一共用」，避免 user 同時調兩組

## Capabilities

### New Capabilities

(none)

### Modified Capabilities

- `claude-code-config`: `Endpoint Profile Schema` requirement 加 `verify` 為 active profile 必填欄位（system + azure 兩側）；`resolve()` 新增 `Verb::Verify` 解析路徑
- `quiz`: `Quiz Content Verification and Repair` requirement 明定 verify spawn 使用 `Verb::Verify` 解析的 model / effort（不再沿用 `Verb::Quiz`），repair spawn 維持 `Verb::Quiz`
- `verb-library`: `Goal Content Verification and Repair` requirement 同樣明定 verify spawn 使用 `Verb::Verify`，repair spawn 維持 `Verb::Goal`
- `app-shell`: `Settings UI Endpoint Section` requirement 從「三個 verb row」改成「四個 verb row（goal / query / fix / verify）」，新增 verify row 的 model + effort dropdown 行為

## Impact

- Affected specs: `claude-code-config`, `quiz`, `verb-library`, `app-shell`
- Affected code:
  - Modified: codebus-core/src/config/claude_code.rs（`Verb` enum 加 `Verify` 變體 + `resolve()` match arm + 測試）
  - Modified: codebus-core/src/config/endpoint.rs（`SystemProfile` / `AzureProfile` struct 加 `verify` 欄位 + `RawSystemProfile` / `RawAzureProfile` 加 `verify` Option + `validate_system_profile` / `validate_azure_profile` 加 require + `SystemProfile::default()` 加 verify 預設 opus-4-6/high + 測試）
  - Modified: codebus-core/src/config/global_starter.rs（`STARTER_CONFIG` 加 `verify:` 區塊 + `starter_round_trips_to_defaults` 測試自動 cover）
  - Modified: codebus-core/src/verb/quiz.rs（加 `verify_resolved = cc_cfg.resolve(Verb::Verify)` 區域變數；verify closure 內 `run_spawn` 改用 `&verify_resolved`；plan / generate / repair spawn 不動）
  - Modified: codebus-core/src/verb/goal.rs（加 `verify_resolved`；verify closure 內 `run_goal_spawn` 改用 `verify_resolved.model.clone() / .effort.clone()`；main / repair spawn 不動）
  - Modified: codebus-app/src/lib/ipc.ts（`SystemProfile` / `AzureProfile` interface 加 `verify` 欄位）
  - Modified: codebus-app/src/components/settings/EndpointSection.tsx（`VERBS` 陣列加 `"verify"`，row 自動 render；`SYSTEM_PROFILE_DEFAULTS` 加 verify 預設值）
  - Modified: codebus-app/src/components/settings/EndpointSection.test.tsx（4 row 渲染 + 互動測試）
  - Modified: codebus-app/src/components/settings/SettingsModal.test.tsx（整合測試 round-trip 含 verify）
  - Modified: codebus-app/src/i18n/messages.ts（zh-tw + en label + tooltip）
  - New: docs/2026-05-XX-verify-stage-independent-model-migration.md（既有 user 升級 yaml snippet + cost 提醒 + re-init 替代選項）
  - Removed: (none)
