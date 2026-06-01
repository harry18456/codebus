## 1. Implementation

- [x] 1.1 在 `codebus-core/src/agent/claude_cli.rs` 的 `compose_claude_cmd` 函式中，於 MCP 隔離塊（`--strict-mcp-config` + `--mcp-config {"mcpServers":{}}`）之後、`--model` 之前，無條件加入 `.arg("--setting-sources").arg("project,local")`。完成時：產出的 argv 必含 `--setting-sources project,local`，且 token 順序為 `--strict-mcp-config` … `--mcp-config` … `--setting-sources` `project,local` … `--model`（有 model 時）。
- [x] 1.2 在 1.1 新增的旗標處補一段 doc comment，說明此旗標排除 user 全域 setting source（`~/.claude/CLAUDE.md` / settings / plugins），對齊 codex backend 的 `--ignore-user-config`，且仍保留 vault 自家 `.codebus/.claude` hook gate 與 `.codebus/CLAUDE.md` schema（2026-05-31 spike 三方驗過）；風格比照既有 `--strict-mcp-config` 註解（標明無 escape hatch、無條件）。
- [x] 1.3 更新 `compose_claude_cmd` 上方的 argv 順序 doc list：在 MCP isolation 項（目前第 9 項）與 `--model`（目前第 10 項）之間插入新項 `--setting-sources project,local` — user-global setting-source isolation，並順移後續項次編號。

## 2. Tests

- [x] 2.1 在 `codebus-core/src/agent/claude_cli.rs` 的 argv 測試模組（既有 `compose_*` / `pos()` helper 風格那組）新增一條測試：斷言 `--setting-sources` 存在、其後一個 argv token 為 `project,local`、且 `pos("--setting-sources")` 大於 `pos("--strict-mcp-config")`、小於 `pos("--model")`（用帶 model 的 `compose(...)` 變體）。
- [x] 2.2 檢查既有 argv 順序測試（含 `--strict-mcp-config` / `--model` 相對位置、`--no-session-persistence` 位置那幾條）是否因新旗標插入而失敗；若有 byte-equivalence 或相對位置斷言受影響，更新成與新 argv 一致（不放寬無關斷言）。

## 3. Spec Alignment

- [x] 3.1 確認 `agent-backend` spec 的 **Claude Backend Argv Equivalence** requirement 與其所有 scenario 與實作後的 argv 一致：requirement 列舉句含 `--setting-sources project,local`、「Argv equals pre-refactor builder」scenario 含兩個 additive flag、且「User-global setting sources are excluded」scenario 的位置斷言與 2.1 的測試一致（以 `spectra validate` 通過為驗證目標）。

## 4. Verification

- [x] 4.1 `cargo build -p codebus-core` 編譯成功，無新 warning。
- [x] 4.2 `cargo test -p codebus-core` 全綠（特別是 `claude_cli` argv 測試模組）。
- [x] 4.3 `cargo clippy -p codebus-core` 無新增 warning（沿用既有 baseline，不引入新項）。
