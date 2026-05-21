## Problem

codebus-app 的 Wiki preview 永遠不顯示 `[Open in Obsidian]` 按鈕，即使使用者已安裝 Obsidian。`wiki-open-in-obsidian` feature 設計時明文假設「`codebus init` 已把 `.codebus/wiki/` 註冊進使用者層級 `obsidian.json`」（見 `app-workspace` spec `Open Wiki Page In Obsidian` requirement），但這個假設**只對 CLI `codebus init` 成立**。

## Root Cause

app 建立 / 綁定 vault 走 `vault_list.rs::add_vault_at` → `run_init(&canonical, &init_opts(), ...)`，而 `init_opts()` 寫死 `no_obsidian_register: true`，且 app 全域**沒有任何** `obsidian_register::register_vault` 呼叫。因此 app 建立的 vault 從不進 `obsidian.json`；前端載入 wiki 時呼叫的 `get_obsidian_vault_id` → `lookup_vault_id` 查不到該 wiki 路徑 → 回 `Ok(None)` → WikiPreview 在 `None` 時完全不渲染按鈕。CLI `codebus init`（預設 `no_obsidian_register: false`）會註冊，所以 CLI 建的 vault 才看得到按鈕。

此外 app 沒有「開啟既有 vault」的後端 hook：vault 選取是前端狀態，唯一在每個 vault 被檢視時必經的後端觸點是前端 wiki store 載入時呼叫的 `get_obsidian_vault_id`。

## Proposed Solution

把 `get_obsidian_vault_id` 由純查找改為 **register-then-lookup**：先 idempotent、fail-soft 地呼叫 `obsidian_register::register_vault(<vault>/.codebus/wiki)`，再回傳 `lookup_vault_id` 的結果。

- **涵蓋現有 + 新 vault**：前端載入任一 vault 的 wiki 時都會呼叫此命令，所以新建與既有（升級前已加入清單、不會再跑 init）vault 都會在下次檢視時被註冊。這正是選用 view-time 觸點而非改 init-time 的理由 —— init-time 註冊無法回頭涵蓋已加入的 vault。
- **fail-soft 無 regression**：Obsidian 未安裝時 `register_vault` 回 `ObsidianNotInstalled`（不寫檔），後續 `lookup_vault_id` 回 `None`、按鈕維持隱藏，與現狀一致。`register_vault` 的任何 IoError 也忽略、續行 lookup。
- **idempotent**：重複檢視同一 vault 只更新既有 entry 的 timestamp，不新增重複條目（`register_at` 既有語意）。
- `open_wiki_in_obsidian`（action）維持既有「重新解析 id」行為不變。

## Non-Goals (optional)

- 不改 `codebus init` / CLI 的註冊行為（CLI 既有 `register_vault` 已 work）。
- 不在 app 端 unregister / 移除 obsidian.json 條目（只新增，與 CLI init 一致）。
- 不改 `vault_list.rs::init_opts()` 的 `no_obsidian_register: true`（view-time 觸點已涵蓋新 vault；不另開第二條寫入路徑以免重複機制）。
- 不偵測 Obsidian 是否安裝後 disable 按鈕（沿用 `None` → 隱藏的既有語意）。
- 不動 `goals.rs` 的 goal-time auto-init 註冊旗標。

## Success Criteria

- 在已安裝 Obsidian 的機器上，於 app 開啟**任一** vault（新建或既有）的 Wiki preview → `get_obsidian_vault_id` 回 `Some(id)` → `[Open in Obsidian]` 按鈕渲染。
- `get_obsidian_vault_id` 呼叫後，該 vault 的 wiki 路徑出現在 `obsidian.json` 的 `vaults` map（idempotent：重複呼叫不新增重複條目）。
- Obsidian 未安裝（config dir 不存在）時 `get_obsidian_vault_id` 仍回 `Ok(None)`、不寫任何檔、不報錯，按鈕維持隱藏。
- `obsidian.json` 存在但壞掉（parse 失敗）時回 `Err(AppError)`（fail-soft，前端視同 None），不 crash。
- 既有 app-tauri 測試（`get_obsidian_vault_id` 對 registered / unregistered / no-config-dir / 壞 json 四情境）更新後通過。

## Impact

- Affected specs: `app-workspace`（modified：`Open Wiki Page In Obsidian` requirement 的 `get_obsidian_vault_id` 行為改為先註冊再解析）
- Affected code:
  - Modified:
    - codebus-app/src-tauri/src/ipc/wiki.rs
