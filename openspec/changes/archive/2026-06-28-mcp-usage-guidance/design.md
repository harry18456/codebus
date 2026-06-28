## Context

codebus 已能一鍵把自己註冊成 claude code / codex 的 user-scope MCP server（`mcp-client-install`），暴露 vault_list / wiki_list / wiki_search / wiki_read 四個 query-only 工具（`mcp-server`）。但「註冊 ≠ 會被用」：agent 在任意 repo 開工時不會主動 vault_list，也分不清何時該查 wiki。實際用途是「跨專案 wiki 參考庫」——使用者累積多個 codebase 的 wiki，工作時讓 agent 跨庫查既有理解。

本 change 解 discoverability 的兩半：(A) 把 tool descriptions 從「描述機制」改成「也講何時 reach for」；(B) 啟用某 client 的 MCP 時，順帶把使用指引寫進該 client 的全域指令檔、停用時移除。B 是 codebus 第一次寫入使用者的「全域」指令檔（既有只寫 vault 內的 `.codebus/CLAUDE.md`），邊界要乾淨。

## Goals / Non-Goals

**Goals:**

- agent（claude code 與 codex 皆是）拿到的 tool descriptions 講清「這是跨專案 wiki 參考庫、何時用、當參考用要回 source 驗」。
- 啟用某 client 的 MCP＝該 client 全域 md 多一段標記式 wiki 使用指引；停用＝移除該塊；重複啟用不疊塊；塊外內容一字不動。
- claude code 與 codex 兩端都能正常使用（各自正確的全域 md 路徑）。

**Non-Goals:**

- agent 模式 / semantic query MCP 工具（讓 MCP server spawn agent 做一次性 query）——另開 change，會推翻 mcp-server 現有「query-only / RAG out of scope」邊界。
- wiki 新鮮度訊號——參考用途容忍落後，本 change 不做。
- 主動 auto-orient（「每次開工先 wiki」）——guidance 框成 on-demand。
- 把 md guidance 拆成獨立子 toggle——使用者明確要綁在 MCP toggle；不另設開關。
- 寫 project-root / per-repo md——只寫使用者全域 md。

## Decisions

### A：tool descriptions 補「何時用 / 跨專案參考」框架

把 server.rs 四個 `#[tool(description=...)]` 改成「先講這是什麼 + 何時 reach for，再講機制」。vault_list 講「這是你建過的跨專案 codebase wiki 庫」；wiki_search / wiki_list 講「想套用某 codebase 的做法、跨專案參考、或被要求時用」；全部點出「當參考用、load-bearing 細節回 source 驗」。保留既有機制規範（wiki_search 仍明示「pass a keyword, not a sentence」、wiki_read 仍講分頁、多 vault 省略語意不變）。描述保持精簡——agent 每次呼叫都會讀全部工具描述，不可灌水。

- 為何：描述是 agent 選工具時唯一看到的字，現狀只講「怎麼呼叫」不講「何時值得叫」，所以連不到「理解這個/那個 codebase」的任務。

### B-1：標記式 managed block 冪等 upsert / 對稱 remove

在全域 md 用 `<!-- codebus:mcp:start -->` … `<!-- codebus:mcp:end -->` 包住一段 guidance。
- upsert：讀現檔（不存在視為空）；若兩標記都在且 start 在 end 前→替換 [start..end]（含）整段；否則在檔尾以一個空行分隔 append。寫法為原子（temp + rename）。標記塊以外的位元組一字不動。
- remove：若標記在→刪掉 [start..end]（含）並收斂掉因此產生的連續空行；標記不在或檔案不存在→no-op（冪等）。

- 為何標記：使用者全域 md 是手寫珍貴檔，唯有 marker-based managed block 能保證「重複啟用不疊塊、停用乾淨拔除、塊外不動」。這是整個 B 的安全基石。

### B-2：per-client 全域 md 路徑（honor 各自 env）

- claude：`CLAUDE_CONFIG_DIR` 有設則其下 `CLAUDE.md`，否則 `~/.claude/CLAUDE.md`。
- codex：`CODEX_HOME` 有設則其下 `AGENTS.md`，否則 `~/.codex/AGENTS.md`。
兩條路徑/檔名皆已在本機實機確認（`~/.claude/CLAUDE.md`、`~/.codex/AGENTS.md` 都存在）。

- 為何 honor env：尊重使用者 relocate，且給乾淨的測試 hook（測試設 env 指到 tempdir，不碰真 home）。對齊 codebus 既有 `CODEBUS_HOME` 風格。

### B-3：md 寫入綁 install / remove、失敗非致命（MCP 註冊為權威）

`mcp_client_install` 在 client CLI `mcp add` 成功「之後」upsert guidance block；`mcp_client_remove` 在 `mcp remove` 之後 remove block。md 寫入失敗為**非致命**：印 stderr 警告、**不回滾**已成功的註冊、回 `Ok`（status 仍反映「已註冊」）。

- 為何非致命：MCP 註冊（工具真的可用）才是價值核心，guidance 是加值；原子寫已保證 md 不會半寫壞；對自己 home dir 寫入極少失敗。回滾註冊只為了補一段 md 不划算，且「已註冊卻回報 install 失敗」對使用者更困惑。替代（硬失敗）明確不採。

### B-4：Settings 揭露文案 + i18n

McpIntegrationSection 每個 client 列加一句揭露：「啟用會同時在你的 <client> 全域指令加入一段 codebus wiki 使用指引（停用會移除）」。文案走既有 i18n（`messages.ts` 加 `settings.mcp.*` key，zh + en 都補）。

- 為何：audit「困惑開發者」——自動改使用者私人檔不可靜默，列上要說清楚。

### block 內容單一來源常數

guidance block 內文定義成單一常數（claude 與 codex 共用同一段——兩端 MCP 工具相同，指引相同），upsert 時包進標記。

## Implementation Contract

**Behavior（ship 後可觀察）：**

- 啟用 client X 的 MCP 整合：除既有註冊外，X 的全域 md 出現一個 `codebus:mcp` 標記塊、內含 wiki 使用指引；再次啟用不會出現第二塊。
- 停用：除既有移除註冊外，該標記塊被拔除，md 其餘內容不變。
- 四個 MCP 工具的 description 讀起來會講「跨專案 wiki 參考庫 + 何時用 + 回 source 驗」，wiki_search 仍指示用 keyword。

**Interface / data shape：**

- 新 helper（codebus-app/src-tauri/src/ipc/global_md.rs）：`upsert_block(client, content) -> Result<()>` 與 `remove_block(client) -> Result<()>`；client 全域 md 路徑解析函式（honor CLAUDE_CONFIG_DIR / CODEX_HOME）；guidance 內容常數；start/end 標記常數。
- `mcp_client_install` / `mcp_client_remove`（mcp_install.rs）在既有 CLI op 後呼叫上述 helper；IPC 簽章不變（仍 `Result<(), AppError>`）。
- 前端：McpIntegrationSection 多一段揭露文案（i18n key），呼叫流程不變（仍 install/remove）。

**Failure modes：**

- md 寫入失敗：非致命，stderr 警告，註冊保留，回 Ok；原子寫保證 md 不半寫壞。
- remove 時標記塊不存在 / md 檔不存在：no-op（冪等）。
- 全域 md 已有使用者內容：只動標記塊之間，其餘逐位元組保留。

**Acceptance criteria：**

- global_md 單元測試：upsert 進空檔→恰一個標記塊；對已有塊 upsert→仍恰一塊（冪等、內容被替換）；upsert 保留塊外既有內容；remove 只刪標記塊、收斂空行；remove 對無塊/無檔→no-op；路徑解析在設了 CLAUDE_CONFIG_DIR / CODEX_HOME 時指向該處的 CLAUDE.md / AGENTS.md。
- mcp_install 測試：install 後對應 client 全域 md 有標記塊、remove 後沒有（用 env 指 tempdir）；既有 install_args / remove_args / listing 測試仍綠。
- server.rs：描述變更後，斷言 vault_list 描述含「跨專案 / library」類字、wiki_search 描述仍含「keyword」指示；既有 mcp 行為測試仍綠。
- 前端：McpIntegrationSection 測試斷言揭露文案出現；i18n zh/en 皆有對應 key（對齊 app-shell i18n coverage policy）。
- 全套：`cargo test -p codebus-cli` 與 `-p codebus-app-tauri` 綠、`cargo clippy --workspace` 無新警告、codebus-app `npm run test` 與 `npm run typecheck` 綠。

**Pre-apply 校準：**

- 這是 codebus 首次寫使用者「全域」md——確認沒有既有測試/行為假設 codebus 從不碰 `~/.claude` / `~/.codex`（既有只寫 vault 內 `.codebus/`）。
- `mcp_client_install` 目前註冊成功即回 Ok；加非致命 md 寫入維持此契約（仍 Ok），既有 mcp_install 測試只驗 argv 不驗 md、不受影響。
- tool description 改字：先 grep 確認沒有測試硬比對「現行描述完整字串」（會破）；wiki_search「keyword 非整句」指示與 mcp-server spec 對齊、必須保留。
- i18n：messages.ts 加 key 要 zh + en 同步（app-shell「i18n Bundle Coverage Policy」）。

**Scope boundaries：**

- In scope：A 四個描述改字；B 標記塊 upsert/remove 綁 install/remove（claude + codex）；per-client 路徑（honor env）；揭露文案 + i18n；對應測試。
- Out of scope：agent-query / semantic MCP 工具（另開）；新鮮度訊號；md guidance 獨立子 toggle；主動 auto-orient；project-root / per-repo md。

## Risks / Trade-offs

- [自動改使用者全域 md 具侵入性] → 全靠標記塊隔離（塊外不動）+ 對稱移除 + 原子寫 + Settings 揭露文案；皆為硬需求非可選。
- [md 寫入非致命 → 可能靜默缺塊] → 對自己 home 寫入極少失敗；失敗印 stderr；註冊（真價值）不受影響；換取無「已註冊卻回報失敗」的半成功困惑。
- [codex 全域 md 檔名版本相依] → 已實機確認本機為 `~/.codex/AGENTS.md`；honor CODEX_HOME；apply 時若目標 codex 版本不同需再確認檔名。
- [tool 描述灌水會排擠 agent context] → 描述保持精簡、只加一句定位 + 一句何時用，不寫長文。
