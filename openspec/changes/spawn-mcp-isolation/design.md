## Context

codebus 在 `codebus_core::agent::claude_cli::build_claude_cmd`（`codebus-core/src/agent/claude_cli.rs`）組出 `claude -p` 的 argv，這是 production 環境唯一 spawn `claude` 的地方。CLI（`codebus-cli/src/commands/*`）與 app（`codebus-app-tauri/src/ipc/*`）都經由 codebus-core 的 verb（goal/query/fix/chat/quiz）→ `agent::invoke` → `build_claude_cmd`，因此此處是共用 chokepoint。

目前 argv（穩定、有測試覆蓋順序）：

1. `-p <slash_command>`
2. `--resume <id>`（僅當 `resume_session_id` 為 `Some`）
3. `--tools <csv>`
4. `--allowedTools <csv>`
5. `--permission-mode acceptEdits`
6. `--output-format stream-json`
7. `--verbose`
8. `--model <m>`（選用）
9. `--effort <e>`（選用）

`--tools` / `--allowedTools` 把工具集鎖成明確清單以建立唯讀 / 受限 sandbox。但 2026-05-21 實測證明這兩個旗標只管「內建工具」，**對 MCP 工具無效**——ambient MCP（user-scope `~/.claude.json`、claude.ai 連接器、project `.mcp.json`）的工具仍會被載入並暴露給 spawn 出來的 agent。

## Goals / Non-Goals

**Goals:**

- 在 `build_claude_cmd` 的載入層硬隔離 MCP：spawn 出來的 codebus agent 看不到也載不到任何 ambient MCP server / 工具。
- 單點修改即同時對 CLI 與 app、所有 verb（含 goal content-verify 二次 spawn）一致生效。
- 維持既有 argv 不變式（特別是 `--resume` 位於 `--tools` 之前）。

**Non-Goals:**

- 不提供 MCP allowlist / 設定開關 / escape hatch（全有全無隔離）。
- 不改 `--tools` / `--allowedTools` 既有內建工具閘的內容與語意。
- 不改 codebus 自身 `codebus mcp`（codebus 作為 MCP server）功能。
- 不改 verb 編排、RunLog、stream 解析、resume、model/effort 等其他行為。

## Decisions

### 用 `--strict-mcp-config` + inline 空 `--mcp-config`

spawn 時固定追加 `--strict-mcp-config` 與 `--mcp-config '{"mcpServers":{}}'`（inline JSON 字面值，非檔案路徑）。`--strict-mcp-config` 令 claude 只採用 `--mcp-config` 提供的 server、忽略所有 ambient 來源；空 server map 即「零 MCP」。

Alternatives considered：

- **只下 `--strict-mcp-config`、不給 `--mcp-config`**：行為依賴「strict 但無 config = 零 server」這個未經本機證實的假設 —— rejected，明確給空 config 才是契約清楚、實測過的形式。
- **`--mcp-config` 指向一份 repo 內或 temp 的空檔案**：要管理檔案生命週期（temp 寫入 / 清理 / 跨平台路徑） —— rejected，inline JSON 已實測被接受（2026-05-21），零檔案開銷。

### argv 位置：接在內建旗標尾、`--model`/`--effort` 之前

新增兩個旗標固定插在 `--verbose` 之後、`--model`/`--effort`（選用）之前。如此 `--resume <id>` 仍在 `--tools` 之前的既有不變式不受影響（新旗標都在 `--tools` 之後）。位置固定以利測試斷言。

Alternatives considered：

- **插在 argv 最末（`--model`/`--effort` 之後）**：也可行，但讓選用旗標夾在中間較難寫穩定斷言 —— 選固定在 verbose 後、選用旗標前。

### 無條件套用、無 escape hatch

所有 verb、所有 toolset / model / effort 組合一律帶這兩個旗標。solo dev 階段 codebus agent 無消費外部 MCP 的需求（YAGNI）；日後若真要讓某 verb 用特定 MCP，再開獨立 change 引入受控 allowlist。

## Implementation Contract

**Behavior:**

任何由 codebus（CLI 或 app、任一 verb）spawn 的 `claude -p` 子行程，啟動時不載入任何 ambient MCP server，session 的工具集只剩 `--tools` 准許的內建工具；`mcp__*` 工具與 `mcp_servers` 皆為空。

**Interface / data shape:**

- `build_claude_cmd`（`codebus-core/src/agent/claude_cli.rs`）產出的 `Command` argv SHALL 額外包含：`--strict-mcp-config`，以及 `--mcp-config` 後接字面值 `{"mcpServers":{}}`。
- 兩旗標位置固定於 `--verbose` 之後、選用的 `--model` / `--effort` 之前。
- 既有不變式維持：當 `resume_session_id` 為 `Some(id)` 時 `--resume <id>` 仍在 `--tools` 之前；新增旗標不破壞此關係。

**Failure modes:**

- 這兩個旗標是 Claude Code 穩定旗標；spawn 行為與既有一致（spawn 失敗仍回 `io::Error`，無新錯誤型別）。
- 隔離為靜默且絕對：被忽略的 ambient MCP 不會產生使用者可見訊息（這是預期，非錯誤）。

**Acceptance criteria:**

- 單元測試斷言 `build_claude_cmd` 的 argv 同時含 `--strict-mcp-config` 與 `--mcp-config {"mcpServers":{}}`，且對「有/無 resume」「有/無 model/effort」組合皆成立。
- 既有 `--resume` 順序測試（`invoke_appends_resume_flag_when_session_id_some`）仍綠：`--resume` 在 `--tools` 之前。
- 既有 `invoke_omits_resume_flag_when_session_id_none` 等「argv 形狀」斷言更新為含新旗標的新基準。
- `cargo test --package codebus-core` 全綠；既有 CLI 整合測試（mock_claude 路徑，忽略旗標）與 `codebus-app-tauri` 測試不因新增旗標失敗。

**Scope boundaries:**

In scope：`build_claude_cmd` 加兩個 MCP 隔離旗標 + 對應 / 更新的單元測試 + `verb-library` spec delta（新增一條 requirement）。

Out of scope：CLI / app surface 各自的程式碼（無需改，自動繼承）、MCP allowlist / 設定、`codebus mcp` server 功能、其他 spawn 行為。

## Risks / Trade-offs

- [未來若有「codebus agent 需要某個 MCP」的正當需求] → Mitigation: 屆時開獨立 change 引入受控 allowlist；現在 YAGNI，全隔離成本最低且最安全。
- [Claude Code 旗標語意未來變動] → Mitigation: 本 change 行為已於 2026-05-21 本機 `claude -p` 實測（inline 空 config + strict → init 的 `mcp__*` 與 `mcp_servers` 皆空）；旗標屬穩定 CLI surface。
- [inline JSON 在某些 shell / 平台的跳脫問題] → Mitigation: argv 經 `Command::arg` 傳遞（非經 shell），字面值不受 shell 解析影響；跨平台一致。
