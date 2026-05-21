# Tasks

## 1. get_obsidian_vault_id register-then-lookup

- [x] 1.1 RED：在 `codebus-app/src-tauri/src/ipc/wiki.rs` 寫測試 `probe_registers_unregistered_vault`——給一個 obsidian 目錄存在、`obsidian.json` 無此 wiki entry 的 temp 環境，呼叫**新的 probe wrapper** `register_and_resolve_vault_id(wiki_root, Some(json_path))` → 斷言回 `Ok(Some(id))` 且 `obsidian.json` 的 `vaults` map 出現該 wiki 路徑 entry。再寫 `probe_register_is_idempotent`——對同 vault 連呼叫兩次 → 兩次回同一 `Some(id)` 且 `obsidian.json` 對該路徑只有一條 entry。驗證：兩測試失敗（wrapper 尚未定義）。（需求：`Open Wiki Page In Obsidian`）
- [x] 1.2 GREEN：新增 probe 專屬 wrapper `register_and_resolve_vault_id(wiki_root, json_path)`——`Some(p)` 時先呼叫 `obsidian_register::register_at(wiki_root, p)`（忽略 `RegisterOutcome`、fail-soft：ObsidianNotInstalled / IoError 皆不中斷不報錯）再委派既有 `resolve_vault_id`；`None` 時直接委派（回 `Ok(None)` 不寫檔）。`get_obsidian_vault_id` 改呼叫此 wrapper。**`resolve_vault_id` 維持純 lookup 不變**——`open_wiki_in_obsidian`（action）續用它，故「action rejects unregistered vault」spec scenario 與其測試不受影響。在 wiki.rs `use` 補 `register_at`。驗證：1.1 兩測試轉綠。（需求：`Open Wiki Page In Obsidian`）
- [x] 1.3 RED+GREEN：寫測試 `probe_returns_none_and_writes_nothing_when_no_config_dir`——對 wrapper 傳 `json_path = None`（obsidian config dir 不存在）→ 斷言 `Ok(None)` 且過程無任何 `obsidian.json` 被建立/寫入。既有 `resolve_vault_id` 四測試（registered→Some / unregistered→None / no-config-dir→None / parse 失敗→Err）**保持不變**（純 lookup 語意未動）。驗證：新測試通過、既有測試 assertion 不變即過。（需求：`Open Wiki Page In Obsidian`）

## 2. 回歸驗證

- [x] 2.1 跑 `cargo test --package codebus-app-tauri` 全綠；逐一確認 `get_obsidian_vault_id` 四情境覆蓋 spec scenarios（register-unregistered→Some / idempotent 單條 entry / no-config-dir→None 不寫檔 / parse 失敗→Err fail-soft）。前端 `wiki.ts` / `wiki.test.ts` 不需改（IPC 簽名與回傳型別 `Option<String>` 不變，前端契約不動）—— 確認 `npx vitest run --no-coverage` 仍全綠。驗證：`cargo test --package codebus-app-tauri` 退出碼 0 且 vitest 全綠。（需求：`Open Wiki Page In Obsidian`）
