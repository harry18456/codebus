## Why

`~/.codebus/config.yaml` 有多個使用者會想調的 knob（`pii.on_hit`、`lint.fix.enabled`、`quiz.content_verify`、`goal.content_verify`、`log` 停用、`pii.patterns_extra`），但 app Settings 面板沒有對應入口，使用者只能手改 YAML。後端存檔（`save_global_config`）已具備未知 section round-trip 保留能力（已有測試 `round_trip_preserves_unknown_sections`），缺口純粹在前端 UI。此外 chat verb 沿用 query 的 model/effort 卻在 Settings 完全不可見，使用者不知道 chat 用哪個 model。

## What Changes

- Settings modal 新增 5 個可編輯欄位：
  - `pii.on_hit`（Select：warn / skip / mask；UI 文案明示 Critical 級不受此設定影響、永遠 mask）
  - `lint.fix.enabled`（Toggle）
  - `quiz.content_verify`（Toggle，含「開啟會多花 verify/repair spawn」成本提示）
  - `goal.content_verify`（Toggle，含同樣成本提示）
  - log 完全停用選項（於既有 log 區塊加一個寫入 `log.sink: none` 的選項）
- Settings modal 新增 `pii.patterns_extra` 編輯區：純 regex 字串列表（新增 / 刪除），**無 label**，對齊 `codebus-core/src/config/pii.rs` 既有 `Vec<String>` 實作；前端即時 regex 驗證、無效 pattern 顯示錯誤不可儲存
- Endpoint Section 新增一列 **唯讀** chat 列，顯示「沿用 query（現：<model> / <effort>）」，與 query 列聯動；不改任何 schema（方案 A）
- 後端不動：`codebus-app/src-tauri/src/ipc/config.rs` 的 `save_global_config` 已保留未知 section；`codebus-core` config 模組不修改
- 修正文件：`docs/2026-05-14-pii-settings-ui-backlog.md` 的 schema 描述（誤寫為 `pii.extra_rules` 物件陣列，實際為 `pii.patterns_extra` 純字串陣列）

## Non-Goals

- chat verb 獨立 model/effort（schema change，留待後續 Change 2 `settings-config-verify-model` 一併評估）
- verify 階段獨立 model（schema change，Change 2）
- `CODEBUS_CLAUDE_BIN` 暴露為 Settings 欄位（env 層，留 backlog）
- per-repo `.codebus/CLAUDE.md` 的 GUI 編輯器（獨立大議題）
- 任何 `codebus-core` config schema 變更、任何 `ipc/config.rs` 後端邏輯變更
- `pii.patterns_extra` 加 label 顯示名稱（會破壞既有 CLI 讀取契約，明確不做）
- 併入 `v3-app-polish-ship`（F）— 本 change 獨立，保持 F 的 release-gate 焦點

## Capabilities

### New Capabilities

(none)

### Modified Capabilities

- `app-shell`: 「Global Settings Modal Field Set」requirement 由「恰好七個欄位」擴充為涵蓋本 change 新增欄位；「Settings modal has no theme or language controls」場景的欄位數斷言一併更新

## Impact

- Affected specs: `app-shell`（modified）
- Affected code:
  - Modified:
    - codebus-app/src/components/settings/SettingsModal.tsx
    - codebus-app/src/components/settings/EndpointSection.tsx
    - codebus-app/src/components/settings/SettingsModal.test.tsx
    - codebus-app/src/i18n/（zh-tw / en 字串資源新增 key）
    - docs/2026-05-14-pii-settings-ui-backlog.md（修正 schema 描述）
  - New:
    - (無新檔；如測試拆分需要可新增 EndpointSection chat-row 測試檔)
  - Removed:
    - (無)
- 後端 / core 不受影響：`codebus-app/src-tauri/src/ipc/config.rs`、`codebus-core/src/config/*` 不修改
