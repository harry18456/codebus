## 1. A — tool descriptions 補「何時用」（codebus-cli）

落實 design 決策「A：tool descriptions 補「何時用 / 跨專案參考」框架」。

- [x] 1.1 [P] 改 server.rs 四個 #[tool] description：加「跨專案 codebus wiki 參考庫」定位 + 何時 reach for + 回 source 驗，保留 wiki_search「pass a keyword, not a sentence」與 wiki_read 分頁說明。驗證：unit test 斷言 vault_list 描述含 library / 跨專案類字、wiki_search 描述仍含 keyword 指示；既有 mcp 行為測試綠；`cargo test -p codebus-cli` 綠。滿足 spec「Tool descriptions convey the cross-project wiki-library use case」。

## 2. B core — global_md helper（codebus-app/src-tauri）

落實 design 決策「B-1：標記式 managed block 冪等 upsert / 對稱 remove」「B-2：per-client 全域 md 路徑（honor 各自 env）」與「block 內容單一來源常數」。

- [x] 2.1 [P] 新增 global_md.rs：標記式（codebus:mcp:start/end）block 冪等 upsert（有就替換、無就 append、塊外位元組不動、原子寫、檔案不存在就建）+ 對稱 remove（只刪該塊、收斂空行、無塊/無檔 no-op）+ per-client 路徑解析（claude `CLAUDE_CONFIG_DIR`→CLAUDE.md 否則 ~/.claude/CLAUDE.md、codex `CODEX_HOME`→AGENTS.md 否則 ~/.codex/AGENTS.md）+ guidance 內容與標記常數。驗證：unit test——upsert 進空檔→恰一塊、二次 upsert→仍恰一塊（內容替換）、保留塊外內容、remove 只刪塊+收斂空行、remove 對無塊/無檔→no-op、設 env 後路徑指向 tempdir 的 CLAUDE.md / AGENTS.md；`cargo test -p codebus-app-tauri` 綠。滿足 spec「Global instruction guidance block on enable」。

## 3. B wire — 綁 install / remove（codebus-app/src-tauri）

落實 design 決策「B-3：md 寫入綁 install / remove、失敗非致命（MCP 註冊為權威）」。

- [x] 3.1 mcp_client_install 在 client CLI mcp add 成功「後」呼 upsert_block、mcp_client_remove 在 mcp remove「後」呼 remove_block；md 寫入失敗為非致命（stderr 警告、不回滾註冊、IPC 仍回 Ok）。依賴 task 2.1。驗證：integration test（env 指 tempdir home）——install 後該 client 全域 md 有標記塊、remove 後沒有；模擬 md 失敗時 install 仍回 Ok 且註冊保留；既有 install_args / remove_args / listing 測試仍綠。

## 4. B disclosure — Settings 揭露文案（codebus-app frontend）

落實 design 決策「B-4：Settings 揭露文案 + i18n」。

- [x] 4.1 [P] McpIntegrationSection 每個 client 列加揭露文案（「啟用會同時在你的 <client> 全域指令加入一段 codebus wiki 使用指引，停用會移除」），messages.ts 補對應 settings.mcp.* key 的 zh + en。驗證：McpIntegrationSection 測試斷言揭露文案 render；i18n zh/en 皆有該 key（對齊 app-shell i18n coverage policy）；`npm run test` 與 `npm run typecheck` 綠。滿足 spec「Global instruction guidance block on enable」。

## 5. 驗證與品質

- [x] 5.1 CI 等價全套確認無回歸。驗證：`cargo test -p codebus-cli` 與 `cargo test -p codebus-app-tauri` 綠、`cargo clippy --workspace` 無新警告、codebus-app `npm run test` 與 `npm run typecheck` 綠。
- [x] 5.2 toggle round-trip 端到端佐證（不花 API）：以 task 3.1 的 integration test（env→tempdir）為權威證明 install→md 寫塊 / remove→拔塊 / 塊外不動；若 app 可開則補一次 CDP smoke 走真 toggle 確認 GUI→IPC 串接，未跑則於 report 註明。驗證：對應 integration test 綠、report 寫明是否補 GUI smoke。
