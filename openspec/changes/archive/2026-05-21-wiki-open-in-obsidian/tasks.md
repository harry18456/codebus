## 1. Backend IPC commands（codebus-app-tauri）

- [x] 1.1 在 `codebus-app/src-tauri/src/ipc/wiki.rs` 加 URL-builder 純函數（如 `build_obsidian_url(vault_id: &str, wiki_root: &Path, abs_page: &Path) -> Option<String>`）：算 abs_page 相對 wiki_root 的路徑、反斜線正規化為正斜線、各段 percent-encode、組 `obsidian://open?vault=<id>&file=<rel>`；落實 design Decision「vault id（lookup_vault_id）而非 vault name」（URL 用 id）與「`file=` 相對於 `<vault>/.codebus/wiki/`」（rel 基準）；加單元測試覆蓋 design「Example: relative path + encoding cases」表格四列（modules/uv-lib.md、concepts/project-purpose.md、index.md、含中文段的 percent-encode），落實 `Open Wiki Page In Obsidian` requirement 的 URL 組裝契約；**測試先寫**。Verify: `cargo test --package codebus-app-tauri ipc::wiki::tests::build_obsidian_url` 全綠。
- [x] 1.2 在 `ipc/wiki.rs` 加 `#[tauri::command] get_obsidian_vault_id(vault_path: String) -> Result<Option<String>, AppError>`：呼叫 `codebus_core::vault::obsidian_register::lookup_vault_id(<vault>/.codebus/wiki)`，`Ok(Some)`/`Ok(None)` 原樣傳、`Err` 映射 `AppError`；這是 design Decision「兩個 IPC command：probe（決定按鈕可見性）+ action（開啟）」的 probe 半邊；落實 requirement 的 `get_obsidian_vault_id` 契約與三個 scenario（Some / None / parse-failure→AppError）。加測試：temp obsidian.json 含匹配 entry → Some；無檔 → None；壞 json → Err。Verify: `cargo test --package codebus-app-tauri ipc::wiki::tests::get_obsidian_vault_id` 全綠。
- [x] 1.3 在 `ipc/wiki.rs` 加 `#[tauri::command] open_wiki_in_obsidian(vault_path: String, slug: String) -> Result<(), AppError>`：依 design Implementation Contract 五步驟（lookup_vault_id→None 回 AppError::Invalid field=obsidian / glob 找 slug→無檔回 AppError::Invalid field=slug / 算 rel + build URL / tauri-plugin-opener 開 / 開失敗回 AppError）；這是 design Decision「兩個 IPC command：probe（決定按鈕可見性）+ action（開啟）」的 action 半邊，且落實 design Decision「action command 重新解析 id（不從 frontend 傳 id 進來）」—— 每次 invocation 重新 lookup、不接受 caller 傳 id。加測試：unregistered vault → AppError、unknown slug → AppError、valid slug 經由可注入 opener 或斷言 build_obsidian_url 輸出驗 URL（不實際 spawn Obsidian）。Verify: `cargo test --package codebus-app-tauri ipc::wiki::tests::open_wiki_in_obsidian` 全綠。
- [x] 1.4 在 `codebus-app/src-tauri/src/ipc/mod.rs` 把 `get_obsidian_vault_id` 與 `open_wiki_in_obsidian` 註冊進 `invoke_handler![]`；確認既有 keyring_ipc 的 registered-commands count 測試（若統計總數）相應更新。Verify: `cargo build --package codebus-app-tauri` 通過、app crate 測試不因缺註冊而 fail。

## 2. Frontend IPC wrapper（codebus-app）

- [x] 2.1 在 `codebus-app/src/lib/ipc.ts` 加 `getObsidianVaultId(vaultPath: string): Promise<string | null>` 與 `openWikiInObsidian(vaultPath: string, slug: string): Promise<void>` typed wrapper，command name union 加兩條（`get_obsidian_vault_id` / `open_wiki_in_obsidian`）；落實 design「ipc.ts wrapper」。Verify: `npx tsc --noEmit` 通過、後續 store / WikiPreview 引用不報缺。

## 3. Wiki store vault-id 狀態（codebus-app）

- [x] 3.1 在 `codebus-app/src/store/wiki.ts` 加 `obsidianVaultId: string | null` state 欄位 + 一個 action 在 vault 的 wiki 載入時呼叫 `getObsidianVaultId` 並 cache（probe 失敗 / 回 null 都存 null）；reset / 換 vault 時清空為 null；落實 design「wiki.ts store 加 obsidianVaultId 欄位」。加 store 單元測試：fetch 成功存 id、fetch 回 null 存 null、reset 清空。Verify: `npx vitest run --no-coverage wiki`（或對應 store 測試檔）全綠。

## 4. WikiPreview 按鈕（codebus-app）

- [x] 4.1 在 `codebus-app/src/components/workspace/WikiPreview.test.tsx` 加測試鎖定 `Open Wiki Page In Obsidian` requirement 的按鈕行為：(a) `obsidianVaultId` 非 null + content page → `open-in-obsidian` testid 按鈕存在且 `[Quiz me on this]` 也在；(b) `obsidianVaultId` 非 null + nav page（index.md / log.md）→ 按鈕仍存在、Quiz 不在；(c) `obsidianVaultId` null → 按鈕不在 DOM；(d) 點按鈕呼叫 `openWikiInObsidian(vaultPath, slug)` 一次；**測試初始狀態 FAIL**（按鈕還沒加）。Verify: `npx vitest run --no-coverage WikiPreview` 紅燈含新 case fail。
- [x] 4.2 在 `WikiPreview.tsx` footer action 區（既有 `[Quiz me on this]` 同一個 `mt-10 border-t` 區）加 `[Open in Obsidian]` 按鈕：條件 `obsidianVaultId !== null` 才渲染、content + nav page 都顯示、點擊呼叫 `openWikiInObsidian(vaultPath, currentSlug)`、用 i18n label；落實 design Decision「按鈕在所有 wiki page 顯示（含 nav page）」前端行為。Verify: 4.1 全綠、既有 WikiPreview 測試（Quiz 按鈕 / wikilink）仍綠。

## 5. i18n

- [x] 5.1 在 `codebus-app/src/i18n/messages.ts` 加 `workspace.wiki.openInObsidian` 的 en（"Open in Obsidian"）與 zh-tw（「在 Obsidian 開啟」）label；落實 design「i18n」。Verify: tsc 通過、WikiPreview 測試對 label 的斷言通過。

## 6. End-to-end smoke

- [x] 6.1 在 Windows 跑 manual smoke：cargo tauri dev →（a）開一個已 register Obsidian 的 vault（`codebus init` 過、obsidian.json 有 entry）→ Wiki tab 選一頁 → 看到 `[Open in Obsidian]` → 點 → Obsidian 跳到對應頁；（b）開一個沒 register 的 vault（手動刪 obsidian.json entry 或用沒裝 Obsidian 的環境）→ 按鈕不出現。macOS / Linux GUI 驗收延後至 `v3-app-polish-ship` deferred acceptance registry（per memory `feedback_dont_default_polish_ship`）。Verify: 兩情境皆觀察到對應行為。
