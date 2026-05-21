## Why

codebus 的 wiki 是 markdown + wikilink 知識庫，跟 Obsidian 的 graph view / backlink / 進階編輯天生互補。`codebus init` 已經把 `.codebus/wiki/` register 進 Obsidian（`obsidian_register::register_vault`），CLI lint 的 OSC 8 hyperlink 也已經用 `obsidian://open?vault=<id>&file=<rel>` 跳轉。但 codebus-app 的 Wiki preview **沒有任何跳 Obsidian 的入口** —— user 想用 graph / backlink 要自己開 Obsidian、找 vault、再點到對應頁，三步 friction。

加一顆「Open in Obsidian」按鈕讓 user 從 app 的 wiki preview 一鍵跳到 Obsidian 對應頁面。這條取代原 backlog `wiki-graph-view`（in-app sigma.js graph）的當下需求：既然 init 已 register Obsidian，跳出去用成熟工具比在 app 內重做 graph 划算。

## What Changes

- 新增 Tauri IPC command `open_wiki_in_obsidian(vault_path: String, slug: String) -> Result<(), AppError>`：backend 解析 vault id（`codebus_core` 的 `lookup_vault_id`）+ slug→相對 wiki 路徑 + 組 `obsidian://open?vault=<id>&file=<rel>` URL + 用 `tauri-plugin-opener` 開
- WikiPreview footer action 區（既有 `[Quiz me on this]` 旁）加 `[Open in Obsidian]` 按鈕
- 按鈕在所有 wiki page 顯示（content + nav 都可開），跟 Quiz 按鈕 content-only 不同
- 當 vault 沒 register 進 Obsidian（`lookup_vault_id` 回 None）時，按鈕**不顯示**（沒 vault id 組不出 URL）
- vault id 識別子用 16-char SHA-256 prefix（跟 CLI lint OSC 8 一致），非 vault name
- `file=` 參數是相對於 `<vault>/.codebus/wiki/`（Obsidian 註冊的 vault root）的路徑

## Non-Goals

- **in-app graph view（sigma.js / graphology）**：原 wiki-graph-view backlog 的範圍，已決定用 open-in-obsidian 取代當下需求；in-app graph 等 v2 真有沒裝 Obsidian 的使用者再開
- **vault name-based URL**：偏離既有 CLI lint 用的 id-based URL，name 有空白 / 編碼問題且可能對不上 Obsidian 註冊
- **沒 register 時 fallback 顯示按鈕讓 OS 處理**：id-based URL 沒 id 根本組不出來；顯示一顆點了無聲失敗的按鈕比隱藏更困惑（跟 name-based 的「一律顯示」邏輯不同）
- **反向（從 Obsidian 操作回 codebus）**：沒這需求
- **改寫 Obsidian register 邏輯**：init 的 register 已 work，本 change 只動 GUI 跳轉
- **自動偵測 Obsidian 是否安裝後 disable 按鈕**：用 `lookup_vault_id` 的 None 即代表「沒 register」涵蓋此情況，不另做安裝偵測
- **PDF / 圖片等非 markdown wiki 檔的特殊處理**：wiki 只有 .md，不在範圍
- **CLI 端加對應命令**（如 `codebus open <slug>`）：本 change 只動 app GUI

## Capabilities

### New Capabilities

(none)

### Modified Capabilities

- `app-workspace`: 加新 Requirement `Open Wiki Page In Obsidian`，自含兩個 IPC command 契約（`get_obsidian_vault_id` probe + `open_wiki_in_obsidian` action）+ 按鈕 UI 行為 + vault id 解析 + 沒註冊時隱藏。既有 `Tauri IPC Commands for Goal Lifecycle and Wiki Read` enumeration **不改** —— 新命令自成一條 requirement，比照該 requirement 內 chat 命令「defined separately」的 precedent

## Impact

- Affected specs: `app-workspace`
- Affected code:
  - New: codebus-app/src-tauri/src/ipc/wiki.rs（加 `open_wiki_in_obsidian` command —— 同檔既有 list_wiki_pages / read_wiki_page）
  - Modified: codebus-app/src-tauri/src/ipc/mod.rs（註冊新 command 進 invoke_handler）
  - Modified: codebus-core/src/vault/obsidian_register.rs（若 `lookup_vault_id` 需暴露給 app crate，確認 pub 可見性；目前已 pub）
  - Modified: codebus-app/src/components/workspace/WikiPreview.tsx（加 Open in Obsidian 按鈕 + 點擊 handler）
  - Modified: codebus-app/src/components/workspace/WikiPreview.test.tsx（按鈕渲染 + 隱藏條件測試）
  - Modified: codebus-app/src/lib/ipc.ts（加 openWikiInObsidian wrapper + command name union）
  - Modified: codebus-app/src/i18n/messages.ts（zh-tw + en 按鈕 label）
  - Modified: codebus-app/src/store/wiki.ts（若需 expose vault-id-available 狀態給 WikiPreview 決定是否顯示按鈕）
  - Removed: (none)
