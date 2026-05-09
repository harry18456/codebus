## Why

v3-lint 剛 ship 後實機 e2e 揭露兩個現實：

1. **`Bash(codebus lint *)` 在 `--allowedTools` 不是 hard restriction**。實測 agent 在 `acceptEdits` + `-p` 模式下能跑 `echo` 等不在 whitelist 的命令（spec 寫的「SHALL NOT grant permission to invoke any other binary」沒有真實強制力）。
2. **Atomic contract（agent 不得自跑 lint）是過度設計**。CLI 反正會跑 final lint 作為 exit code 權威依據；agent 自跑 lint loop 不會破壞最終驗證，反而能省掉 CLI 每次外層 ping 的 spawn startup 成本。Outer ping 機制本身在 agent 自治模型下變成冗餘。

這兩個發現指向同一方向：**信任 agent 的修補過程，但用 hook 守住硬邊界**。本 change 把 fix loop 改寫成 trust-agent 模型 — 去掉 atomic contract / outer ping / `--fix-max-iter`，agent 在 session 內自由 loop；同時加 PreToolUse hook 真正鎖死 Bash 只能跑 `codebus lint`。

## What Changes

- **去掉 Fix CLI Outer Ping Loop**：`run_fix_loop` 從「spawn → lint → ping with `--resume` → lint → ...（多輪）」改為「lint precheck → spawn 一次 → lint final check → exit」單發模型。Agent 在 session 內想跑幾輪 lint+edit 都行，session 結束 CLI 只做最後 verifier。
- **去掉 `outer_ping_max` config 與 `--fix-max-iter` flag**：`LintFixConfig` 只剩 `enabled`；`--no-fix` 保留（仍是有用的 escape hatch）。
- **去掉 atomic contract 在 fix SKILL.md**：移除「ONE round of repair」「Loop control belongs to the caller」「MUST NOT spawn nested fix invocations or loop internally」等限制語。改寫成 trust-agent 描述 — agent 自由探索 + 修補，session 結束就退出。
- **去掉 `wiki/fix/prompt.rs` 的 `followup_prompt`**：沒 ping 就沒 follow-up；保留 `initial_prompt()` 即可（其實也可以併進 `mod.rs`）。
- **新增 PreToolUse Bash hook 機制**：
  - 新 hidden CLI subcommand `codebus hook check-bash`：從 stdin 讀 PreToolUse JSON、解析 `tool_input.command`、比對允許 pattern（`codebus` 或 `<path>/codebus(.exe)?` 後接 `lint`）、通過則 exit 0、不通過則印 `{"decision":"block","reason":"..."}` exit 0。
  - `init` 多寫 `<vault>/.codebus/.claude/settings.json`（write-if-missing），內容含 PreToolUse Bash hook 指向 `codebus hook check-bash`。Hook 對 fix spawn（cwd=vault）自動生效，因為 Claude Code 從 cwd 的 `.claude/settings.json` 自動載入。
  - vault 內部 `.gitignore` 加 `.claude/settings.local.json`（local override 不進 vault git）。
- **fix sandbox 維持雙層**：`--tools` 仍含 bare `Bash`、`--allowedTools` 仍含 `Bash(codebus lint *)`（auto-approval scope，避免 prompt）。Hook 是新增的 hard gate 層，不替代 allowedTools。
- **goal flow lint-and-fix phase 簡化**：goal 內部 call `run_fix_loop` 從多輪 ping 變單發；commit 摺單顆語意不變。

## Capabilities

### New Capabilities

（無）

### Modified Capabilities

- `lint-feedback-loop`：
  - REMOVE `Fix CLI Outer Ping Loop` requirement（取代為 `Fix Single-Shot Verification`）
  - MODIFY `Fix Loop Configuration` — 移除 `outer_ping_max` 與 `--fix-max-iter`，只剩 `enabled` 與 `--no-fix`
  - MODIFY `Fix SKILL.md Atomic Contract` → 改名 `Fix SKILL.md Workflow`，移除 atomic / no-loop / no-nested-spawn 等限制語
  - MODIFY `Standalone Fix Mode` — 拿掉 ping budget 退出碼路徑；exit 0 lint 乾淨、exit 1 有 issue
  - ADD `Fix Bash Hook Installation` — init 寫 settings.json 與 hook subcommand 接口契約
- `cli`：
  - MODIFY `Fix Subcommand Behavior` — 移除 `--fix-max-iter` flag；簡化步驟描述
  - MODIFY `Goal Subcommand Behavior` — 移除 `--fix-max-iter` forwarding

## Impact

- Affected specs: `lint-feedback-loop` (modified), `cli` (modified)
- Affected code:
  - New:
    - codebus-cli/src/commands/hook.rs
  - Modified:
    - codebus-core/src/wiki/fix/mod.rs
    - codebus-core/src/wiki/fix/prompt.rs
    - codebus-core/src/config/lint_fix.rs
    - codebus-core/src/skill_bundle/mod.rs
    - codebus-cli/src/main.rs
    - codebus-cli/src/commands/mod.rs
    - codebus-cli/src/commands/init.rs
    - codebus-cli/src/commands/fix.rs
    - codebus-cli/src/commands/goal.rs
    - codebus-cli/tests/cli_routing.rs
    - codebus-cli/tests/fix_flow.rs
    - codebus-cli/tests/goal_flow.rs
  - Removed:
    - codebus-core/src/wiki/fix/session.rs (UUID + SessionAction 用法在單發模型下不需要；session 不再跨輪)
