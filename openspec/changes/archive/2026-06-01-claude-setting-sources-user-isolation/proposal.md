## Why

codebus spawn claude 蓋 wiki 時，`compose_claude_cmd`（`codebus-core/src/agent/claude_cli.rs`）只隔離了 MCP load 層（`--strict-mcp-config` + 空 `--mcp-config`），但沒有隔離 user 全域設定。使用者 `~/.claude/CLAUDE.md` / `~/.claude/settings.json` / 全域 plugins 會 bleed 進每一個蓋 wiki 的 claude agent，污染其行為——這是 functional bug 不只衛生問題：使用者全域的「一律用 zh-tw 溝通」鐵則、TDD 強制、agent-orchestration 規則等會帶偏 agent 的 wiki 產出（例如把輸出語言鎖死成 user 偏好，而非 codebus schema 規定的「跟隨 prompt context 語言」）。這與 codex 路徑用 `--ignore-user-config` 做的 user 隔離不對稱。

## What Changes

- 在 `compose_claude_cmd` 的 MCP 隔離塊（`--strict-mcp-config` + `--mcp-config`）之後、`--model` 之前，**無條件**加上 `--setting-sources project,local`，排除 user 全域 setting source（CLAUDE.md / settings / plugins），同時保留 vault 自家 project 層（`.codebus/.claude/settings.json` 的 `check-bash` / `check-read` PreToolUse hook gate）與 vault `.codebus/CLAUDE.md` schema。
- 更新 `compose_claude_cmd` 的 argv 順序 doc list（補一項 `--setting-sources project,local`）與一段 doc comment（說明對齊 codex `--ignore-user-config`，spike 三方驗過）。
- 新增一條 argv 順序單元測試：斷言 `--setting-sources` 存在、值為 `project,local`、位置在 `--strict-mcp-config` 之後且在 `--model` 之前。
- 更新 `agent-backend` spec 的「Claude Backend Argv Equivalence」requirement：requirement 列舉句加入 `--setting-sources project,local`；「Argv equals pre-refactor builder except the session-persistence flag」scenario 改為「with the addition of `--no-session-persistence` AND `--setting-sources project,local`」；新增一條斷言旗標存在與位置的 scenario。

2026-05-31 spike（real claude 2.1.158 + haiku, Windows）三方驗證 `--setting-sources project,local`：(a) vault `check-bash` hook 仍 fire（非白名單 Bash 被擋）；(b) user `~/.claude/CLAUDE.md` zh-tw 鐵則被排除（探針答 NO-LANG-RULE）；(c) vault `.codebus/CLAUDE.md` schema 仍保留（探針答出 5/5 wiki type bucket）。官方 memory 文件背書「`CLAUDE.local.md` is skipped if you exclude `local` from `--setting-sources`」。spike 已否決 `--bare`（會 skip 所有 hooks 含自家 check-bash，且 auth 只認 API key、本機走 OAuth 直接 Not logged in）。

Non-Goals：不改 codex 路徑；不提供 escape hatch 讓 user 全域重新注入（採無條件硬隔離，與 MCP 隔離塊一致）；不抽 trait、無第二 consumer（單一 call site）；不碰 `--bare`；不動 `docs/BACKLOG.md` 或其他不相關 sweep。

## Impact

- Affected specs: agent-backend（Claude Backend Argv Equivalence requirement + scenarios）
- Affected code:
  - Modified: codebus-core/src/agent/claude_cli.rs
