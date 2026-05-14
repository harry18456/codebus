## Why

Settings UI 的 `effort` 欄位目前是 `<Input>` 純文字輸入，使用者可以打任何字串並存進 yaml。Claude Code CLI `--effort` 實際支援的合法值是 `low` / `medium` / `high` / `xhigh` / `max` / `auto`（六個），但目前 UI 沒有約束，打錯字（如 `hgih`、`maximum`）會被靜默存進設定，跑 verb 時才會以 CLI 旗標的形式失敗 — 是個 silent UX 陷阱。換成限定列舉的下拉選單可以從來源端杜絕無效輸入。

## What Changes

- Settings UI 的 System Profile 與 Azure Profile sub-section 中，每個 verb 列的 `effort` 欄位從 `<Input>`（純文字）改為 `<Select>`（下拉選單）。
- 下拉選單列出 Claude Code 支援的六個合法值：`low`、`medium`、`high`、`xhigh`、`max`、`auto`（順序固定，由低到高，`auto` 殿後作為「讓模型決定」的特殊選項）。System 與 Azure 共用同一份選項清單。
- 不允許空白／未選狀態 — Settings modal 的 client-side validation SHALL 在 effort 不屬於合法集合時阻擋 Save 並標記 `aria-invalid`。
- 不提供 "Custom…" 自填選項。
- 載入既有 yaml 時，若 effort 值不在合法集合內（legacy / 手改），UI SHALL 保留原值於 in-memory state，但 validation 視為 invalid，強制使用者重選後才能 Save。
- Rust 端 `effort: String` 維持不變，IPC 型別 `SystemVerb.effort` / `AzureVerb.effort` 也維持 `string`，向後相容既存 yaml 與 IPC 契約。

## Non-Goals

- 不修改 Rust `endpoint.rs` 的 `effort: String` 型別 — Rust 層仍接受任意字串，避免破壞既有 yaml。
- 不為 `Verb::Chat` 或其他未來 verb 新增 effort 設定 — 沿用既有四 verb 結構。
- 不調整 `claude-code-config` spec 中 effort 的語意 — 合法集合只在 UI 層約束，CLI 與 yaml 仍是 freeform string。
- 不引入新的 i18n 翻譯 key — 六個值維持英文小寫顯示，與 model dropdown 一致。

## Capabilities

### New Capabilities

(none)

### Modified Capabilities

- `app-shell`: `Settings UI Endpoint Section` requirement 中關於 effort 欄位的描述從 "free-text input" 改為 "select with exactly six options"，並新增對應 scenarios。

## Impact

- Affected specs: `app-shell`（修改 `Settings UI Endpoint Section` requirement）
- Affected code:
  - Modified:
    - codebus-app/src/components/settings/EndpointSection.tsx
    - codebus-app/src/components/settings/EndpointSection.test.tsx
    - codebus-app/src/lib/ipc.ts（新增 `SYSTEM_EFFORTS` 常數與型別、`validateClaudeCodeBlock` 加入 effort 合法值檢查）
  - New: (none)
  - Removed: (none)
