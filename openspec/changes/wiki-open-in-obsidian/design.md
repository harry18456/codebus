## Context

codebus 的 wiki（`.codebus/wiki/`）是 markdown + wikilink 知識庫。`codebus init` 透過 `codebus_core::vault::obsidian_register::register_vault` 把 wiki 目錄註冊進使用者層級的 `obsidian.json`，CLI lint 的 OSC 8 hyperlink 已經用 `obsidian://open?vault=<id>&file=<rel>` 讓終端機點擊跳 Obsidian（`obsidian_register::lookup_vault_id` 回傳 16-char SHA-256 prefix 作為 `<id>`）。

但 codebus-app 的 Wiki preview（`WikiPreview.tsx`）只渲染 markdown body + 一個 content-page-only 的 `[Quiz me on this]` footer 按鈕，**沒有跳 Obsidian 的入口**。本 change 在 footer 加一顆「Open in Obsidian」按鈕。

取代原 `wiki-graph-view` backlog（in-app sigma.js graph）的當下需求 —— 既然 init 已 register Obsidian，跳出去用成熟工具比 app 內重做 graph 划算。

既有架構 anchor：

- `codebus-core/src/vault/obsidian_register.rs`：`register_vault` + `lookup_vault_id(wiki_path) -> io::Result<Option<String>>`（已 pub，CLI lint 已用）
- `codebus-app/src-tauri/src/ipc/wiki.rs`：既有 `list_wiki_pages` / `read_wiki_page`，`WikiPageMeta { slug, path (absolute), title }`
- `codebus-app/src-tauri/src/lib.rs`：`tauri_plugin_opener::init()` 已掛載
- `codebus-app/src/components/workspace/WikiPreview.tsx`：footer action 區（`mt-10 border-t` 之後）有 `[Quiz me on this]`，`currentPath` 只有 slug
- `codebus-app/src/store/wiki.ts`：`currentPath: string | null`（slug）、`pages` 清單（含 absolute path）

## Goals / Non-Goals

**Goals:**

- Wiki preview footer 一鍵跳 Obsidian 對應頁
- vault id 識別子跟 CLI lint OSC 8 一致（id 非 name）
- Obsidian 未註冊時按鈕不顯示（不留死按鈕）
- URL 組裝 + slug→相對路徑解析集中在 Rust（跟 lookup_vault_id 同層），frontend 保持 dumb

**Non-Goals:**

- in-app graph view（sigma.js / graphology）
- vault name-based URL
- 未註冊時 fallback 顯示按鈕讓 OS 處理
- 從 Obsidian 反向操作回 codebus
- 改寫 init 的 Obsidian register 邏輯
- 獨立的「Obsidian 安裝偵測」（用 lookup_vault_id 的 None 即涵蓋）
- CLI 端對應命令

## Decisions

### vault id（lookup_vault_id）而非 vault name

URL 用 `obsidian://open?vault=<id>&file=<rel>`，`<id>` 來自 `lookup_vault_id`（16-char SHA-256 prefix）。

Alternatives considered：

- **vault name**：name 有空白 / 中文 / 編碼問題，且可能對不上 Obsidian 內部註冊的 vault key —— rejected；且偏離 CLI lint 既有 id-based 行為

### 兩個 IPC command：probe（決定按鈕可見性）+ action（開啟）

- `get_obsidian_vault_id(vault_path: String) -> Result<Option<String>, AppError>`：回傳 vault id 或 None。Frontend 在 wiki 載入時呼叫一次、cache 在 store；按鈕**僅當 id 非 None 時顯示**
- `open_wiki_in_obsidian(vault_path: String, slug: String) -> Result<(), AppError>`：backend 重新解析 id + slug→相對路徑 + 組 URL + 用 opener 開

Alternatives considered：

- **單一 command（按鈕一律顯示、點了才知道有沒有 id、無 id 時回錯誤 + toast）**：少一個 command，但違反「沒註冊就隱藏按鈕」的 UX 決策（assumption #5），會留一顆有時無作用的按鈕 —— rejected
- **Frontend 自己組 URL + 用 `@tauri-apps/plugin-opener` JS API 開（只留 probe command）**：少一個 backend command，但 URL 組裝 + 相對路徑計算 + URL encode 散到 TS，跟 CLI lint 在 Rust 組同款 URL 的邏輯重複且易分歧 —— rejected；URL 邏輯集中在 Rust 一處

**Trade-off 明示：** 「沒註冊隱藏按鈕」這個 UX 決策的代價就是多一個 probe command（`get_obsidian_vault_id`）。若接受「按鈕一律顯示、無 id 時點擊給 inline 提示」可省下 probe command 與 store 狀態，但留死按鈕。本 change 選隱藏。

### action command 重新解析 id（不從 frontend 傳 id 進來）

`open_wiki_in_obsidian` 自己呼叫 `lookup_vault_id`，不接受 frontend 傳入的 cached id。

Alternatives considered：

- **frontend 把 probe 拿到的 id 傳回 action command**：省一次 lookup，但若 user 在 app 開著時改動 Obsidian 註冊狀態，cached id 會 stale；重新解析更 robust 且 lookup 是 O(1) 檔案讀取 —— rejected

### `file=` 相對於 `<vault>/.codebus/wiki/`

slug → 從 `WikiPageMeta.path`（absolute）找到對應檔 → 減去 `<vault>/.codebus/wiki/` 前綴 → URL-encode → 作為 `file=` 參數。例：slug `uv-lib` 在 `modules/` → `file=modules/uv-lib.md`。

Alternatives considered：

- **`file=<slug>`（不含 folder / 副檔名）**：Obsidian 的 file 參數要相對 vault root 的完整路徑，光 slug 開不到子資料夾的頁 —— rejected

### 按鈕在所有 wiki page 顯示（含 nav page）

`[Open in Obsidian]` 在 content + nav page（index.md / log.md）都顯示，跟 `[Quiz me on this]`（content-only）不同。

Alternatives considered：

- **比照 Quiz 只在 content page 顯示**：但 user 可能想在 Obsidian 看 index / log 的 graph context，沒理由擋 nav page —— rejected

## Implementation Contract

**Behavior:**

當 vault 已 register 進 Obsidian（`get_obsidian_vault_id` 回 Some），WikiPreview footer（既有 `[Quiz me on this]` 旁、`mt-10 border-t` 區）渲染 `[Open in Obsidian]` 按鈕。content page 兩顆都顯示；nav page 只顯示 Open in Obsidian。點擊呼叫 `open_wiki_in_obsidian(vaultPath, currentSlug)` → Obsidian 跳到該頁。

當 vault 未 register（`get_obsidian_vault_id` 回 None），按鈕**完全不渲染**（DOM 不存在）。

**Interface / data shape:**

- 新 IPC command `get_obsidian_vault_id(vault_path: String) -> Result<Option<String>, AppError>`，定義於 `codebus-app/src-tauri/src/ipc/wiki.rs`，呼叫 `codebus_core::vault::obsidian_register::lookup_vault_id(<vault>/.codebus/wiki)`；`lookup_vault_id` 的 `Err`（obsidian.json 存在但讀不了 / parse 失敗）映射為 `AppError`，`Ok(None)` 原樣傳回
- 新 IPC command `open_wiki_in_obsidian(vault_path: String, slug: String) -> Result<(), AppError>`，同檔；解析步驟：
  1. `lookup_vault_id` → None 時回 `AppError::Invalid { field: "obsidian", message: "vault not registered in Obsidian" }`
  2. glob wiki 找 slug 對應檔 → 找不到回 `AppError::Invalid { field: "slug", message: "no such wiki page" }`
  3. 算 rel = abspath 減 `<vault>/.codebus/wiki/`，正規化為正斜線，URL-encode path 各段
  4. 組 `obsidian://open?vault=<id>&file=<rel>`
  5. 用 tauri-plugin-opener 的 Rust API 開該 URL；開啟失敗回 `AppError`
- 兩個 command 註冊進 `codebus-app/src-tauri/src/ipc/mod.rs` 的 invoke_handler
- `ipc.ts`：`getObsidianVaultId(vaultPath): Promise<string | null>` + `openWikiInObsidian(vaultPath, slug): Promise<void>` wrapper，command name union 加兩條
- `wiki.ts` store：加 `obsidianVaultId: string | null` 欄位 + load 時 fetch + reset 時清空
- `WikiPreview.tsx`：footer 區條件渲染按鈕（`obsidianVaultId !== null`），點擊呼叫 `openWikiInObsidian`
- i18n：`workspace.wiki.openInObsidian`（en: "Open in Obsidian"，zh-tw: 「在 Obsidian 開啟」）

**Failure modes:**

- vault 未 register → probe 回 None → 按鈕不渲染（主要路徑，非錯誤）
- obsidian.json 存在但讀取 / parse 失敗 → probe 回 `AppError` → frontend 視為「不可用」、按鈕不渲染（fail-soft，不彈錯）
- action command slug 找不到對應檔 → `AppError::Invalid`；frontend 顯示既有錯誤 toast pattern
- opener 開啟失敗（Obsidian 安裝但 URL handler 沒註冊）→ `AppError`；frontend 錯誤 toast
- 用 action 時 vault 剛好變未註冊（race）→ command 內 lookup_vault_id 回 None → `AppError::Invalid`，frontend toast

**Acceptance criteria:**

- `codebus-app-tauri` 測試：`get_obsidian_vault_id` 對「有註冊的 temp obsidian.json」回 Some(id)、對「無 obsidian.json」回 None、對「壞 json」回 Err→AppError；`open_wiki_in_obsidian` 對 valid slug 組出正確 `obsidian://open?vault=<id>&file=<rel>` URL（用可注入的 opener mock 或 URL-builder 純函數驗 URL 字串，不實際 spawn Obsidian）、對 unknown slug 回 AppError、對 unregistered vault 回 AppError
- URL-builder 純函數單元測試：slug 在子資料夾 → `file=modules/uv-lib.md`；含空白 / 中文的路徑正確 URL-encode；正斜線正規化（Windows abspath 反斜線轉正斜線）
- `codebus-app` 前端測試（`WikiPreview.test.tsx`）：`obsidianVaultId` 非 null → 按鈕渲染（content + nav page 都有）；`obsidianVaultId` null → 按鈕不存在 DOM；點擊呼叫 `openWikiInObsidian(vaultPath, slug)` 一次
- 手動 smoke（Windows）：cargo tauri dev → 開一個 register 過 Obsidian 的 vault → wiki preview 看到 Open in Obsidian → 點 → Obsidian 跳對頁；開一個沒 register 的 vault → 按鈕不出現

**Scope boundaries:**

In scope：兩個 IPC command（probe + action）+ URL-builder 純函數 + WikiPreview 按鈕 + 條件渲染 + wiki store vault-id 欄位 + ipc.ts wrapper + i18n + 對應測試。

Out of scope：in-app graph view、name-based URL、未註冊 fallback 顯示、反向操作、改 init register、CLI 命令、PDF/圖片 wiki 檔。

## Risks / Trade-offs

- [多一個 probe command 增加 IPC surface] → Mitigation: 接受，這是「沒註冊隱藏按鈕」UX 的成本；probe 是輕量 O(1) 檔案讀取，wiki load 時呼叫一次
- [Obsidian URL handler 在某些 OS / 安裝狀態下沒註冊，點了沒反應] → Mitigation: action command 回 AppError、frontend toast；無法在點擊前 100% 確認 URL handler 可用（只能確認 vault 有 register），這是 OS 層限制
- [vault id stale（user 開著 app 時改 Obsidian 註冊）] → Mitigation: action command 每次重新 lookup_vault_id，不信任 cached id；只有按鈕可見性用 cached（可接受，最壞情況是按鈕顯示但點了回 AppError toast）
- [跨平台路徑分隔符] → Mitigation: URL-builder 純函數把 abspath 反斜線正規化為正斜線後再算 rel + encode，單元測試覆蓋 Windows / Unix 路徑
