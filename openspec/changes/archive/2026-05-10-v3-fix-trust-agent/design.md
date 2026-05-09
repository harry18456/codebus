# v3-fix-trust-agent 設計

## Context

v3-lint 在 2026-05-09 ship 後，實機 e2e 對真實 claude API 跑 `codebus fix` 揭露兩個 v3-lint 設計時沒考慮到的現實：

1. **Bash whitelist 不是 hard restriction**：`--allowedTools "Bash(codebus lint *)"` + `--permission-mode acceptEdits` 在 `-p` 非互動模式下，agent 仍能跑 `echo` 等任意命令。`--allowedTools` 是 auto-approval scope（不在 scope 內的 tool 在互動模式 prompt、`-p` 模式行為交給 Claude Code 內部判斷），不是「only these are allowed」。實測證實 `echo MARKER-7777` 通過、`rm -rf /tmp/...` 被擋住但是被 Claude Code 內建的 working-dir 防護擋的，跟我們的 whitelist 無關。

2. **Atomic contract 是過度設計**：v3-lint 設計時為了「防 agent 自欺」與「deterministic 終止」加了 atomic contract（agent 不得自跑 lint）+ outer_ping_max。實機跑下來，CLI 反正會跑 final lint 作為 exit code 權威依據，agent 中間自欺不影響最終驗證；Claude Code 自己有 max-turns / context limit / token budget 自然擋住無限 loop。Outer ping 機制每次 ping 都是新 spawn 帶 startup 成本，而且需要組 follow-up prompt 把 issues serialize 成 JSON 塞進 prompt body — agent 在 session 內共用 context 反而更 token-efficient。

兩個發現指向同一方向：**信任 agent 做修補（process 不重要），用 hook 守住硬邊界（boundary 才重要）**。本 change 把 fix loop 改寫為 trust-agent 單發模型 + PreToolUse hook hard gate。

獨立驗證（2026-05-09 在 `/tmp/hook-test` 跑）：PreToolUse hook 在 `-p` 模式下的 `{"decision":"block","reason":"..."}` 從 stdout 輸出能真擋 Bash tool 呼叫，agent 收到 block 訊息並理解。Hook 不分 mode 都會跑，settings.json 從 cwd 的 `.claude/settings.json` 自動載入。

## Goals and Non-Goals

**Goals**：

- fix loop 從 multi-spawn outer ping 改為 single-spawn + final verify（trust-agent）
- 移除 `outer_ping_max` config、`--fix-max-iter` flag、`SessionAction`/`--session-id`/`--resume`、`followup_prompt`、`atomic contract` 等所有 ping 機制相關概念
- 加 PreToolUse Bash hook 把「只能跑 codebus lint」變真實 hard gate
- 維持「fix sandbox 雙層」設計：`--tools` 的 bare `Bash` + `--allowedTools` 的 `Bash(codebus lint *)`（auto-approval）+ hook（hard gate）
- 程式碼大幅簡化：`wiki/fix` 模組從 ~600 行減到 ~200 行

**Non-Goals**：

- 不動 goal/query verb 的 toolset 與 sandbox（它們沒 Bash、沒 atomic contract、沒 ping，本 change 對它們是 no-op 變更）
- 不加 Task tool 給任何 verb（另開 follow-up change）
- 不建構通用 hook 框架，只實作 fix 用的這一個
- 不動 lint 規則集本身（v3-lint 的 7 條規則維持原樣）
- 不保留 v3-lint 的 `outer_ping_max` config alias（v3 系列是 fresh start，clean break）
- 不加 hook 對 goal/query 的擴張（它們沒 Bash 用不到）

## Decisions

### Trust-agent single-shot 模型

新流程（取代 v3-lint 的 outer ping loop）：

```
1. CLI: lint precheck. 0 issues → exit 0 (skip spawn)
2. CLI: spawn `claude -p "/codebus-fix"` once with cwd=vault, stdin closed
3. Agent in-session: 自由 lint+edit 多輪（受 Claude Code 自身 token/turn limit 自然擋住）
4. CLI: lint final check after agent terminates
5. CLI: auto_commit "wiki: lint fix loop" (no-op if no changes)
6. CLI: exit 0 (lint clean) / 1 (issues remain)
```

替代 A：保留 outer ping mechanism — 否決，atomic contract 的核心理由（防 agent 自欺）被「CLI final lint」替代後，ping 變冗餘成本。

替代 B：完全去掉 CLI 的 lint precheck 與 final check，全交 agent — 否決，CLI final check 是 exit code 權威依據，沒它無從判斷「fix 成不成功」。

### PreToolUse Bash hook

新增 `<vault>/.codebus/.claude/settings.json`（init write-if-missing），內容：

```json
{
  "hooks": {
    "PreToolUse": [
      {
        "matcher": "Bash",
        "hooks": [
          {
            "type": "command",
            "command": "codebus hook check-bash"
          }
        ]
      }
    ]
  }
}
```

Claude Code 從 cwd 的 `.claude/settings.json` 自動載入，因此 fix spawn（cwd=vault）天然觸發 hook；goal/query 因為 toolset 不含 Bash，hook 雖有但沒 Bash 命令會經過它，no-op。

替代 A：用 `--settings <path>` flag 顯式指定每次 spawn 的 settings — 否決，多一層路徑管理，靜態 vault-resident config 更乾淨。

替代 B：用 `--disallowedTools "Bash(echo *)" "Bash(curl *)" ...` 黑名單 — 否決，黑名單列不完，永遠有漏網。

替代 C：完全不給 agent Bash（D 方案，prior session 討論過）— 否決，使用者直觸 `/codebus-fix` 在已 PATH 安裝 codebus 的環境下，能用 Bash 跑 lint 比每次都靠外部 inject issues 更自然。

### Hook 命令格式：`codebus hook check-bash`

CLI 多一個隱藏子命令（`#[command(hide = true)]`），不在 `--help` 列出（這是 Claude Code hook 內部接口，不是使用者 surface）。Stdin 讀 PreToolUse JSON、stdout 印 decision JSON、exit 0。

接口契約：
- **Input**（stdin JSON）：`{"tool_name": "Bash", "tool_input": {"command": "<cmd>"}, ...}`
- **Allow rule**：`<cmd>` argv 第一字符合下列任一：(a) 字面 `codebus`；(b) 路徑結尾為 `codebus`（Unix）或 `codebus.exe`（Windows，case-insensitive）；後接 argv 第二字 `lint`，或只有 `<binary>`（沒有後續 args，雖然不太可能但保險）。
- **Allow output**：exit 0、stdout 不印任何 decision（缺 decision 視為 allow）。
- **Block output**：exit 0、stdout 印 `{"decision":"block","reason":"<msg>"}`，agent 收到 reason。
- **Error / unparseable input**：exit 0 + block decision（fail-closed）— 防止 hook 自己 bug 時 silently allow。

替代：fail-open（hook 錯誤時放行）— 否決，這違反 hook 存在的目的（hard gate）。fail-closed 即使 hook 壞掉，至少使用者馬上感受到（fix 整個跑不動 → 直接報修），比 silent bypass 安全。

### Bash 命令 pattern 比對細節

Hook 比對 `tool_input.command` 字串：

1. 用 shell-style argv 拆分（簡單空白拆，不處理引號 escape — codebus 自己呼叫不會有複雜 quoting）
2. 第一個 token = binary path
3. Normalize：
   - 取 file basename（去除目錄路徑）
   - 大小寫不敏感（Windows codebus.EXE / codebus.exe）
   - 比對是否 `codebus` 或 `codebus.exe`
4. 第二個 token 必須是 `lint`（或 argv 只有 1 個 token = 純 `codebus`，但這沒實際用途，可以選擇 deny）
5. 通過 → allow；不過 → block + reason

替代：用 regex 字面比對 `^codebus lint( |$)|/codebus(.exe)? lint( |$)` — 否決，regex 對路徑分隔符 / Windows backslash / case insensitivity 處理會比 argv 拆分更脆弱。

### settings.json 位置：只 vault-internal

寫到 `<vault>/.codebus/.claude/settings.json`，**不**寫到 `<repo>/.claude/settings.json`（source repo level）。理由：

- fix CLI spawn 時 cwd=`<repo>/.codebus/`，Claude Code 從這裡找 `.claude/settings.json` → vault-internal 那份正確生效
- 使用者直觸 `/codebus-fix` 從 source repo root 開 Claude Code，cwd 在 `<repo>/`，會載入 `<repo>/.claude/settings.json` — 但使用者那邊可能已經有自己的 settings.json，codebus 不該蓋
- 取捨：使用者直觸模式沒有 hook 保護，agent 仍能跑任意 Bash。但使用者直觸是手動模式，使用者本人就在現場可以 ctrl-c，安全風險可接受

### Drop SessionAction / session.rs 整套

單發模型不需要 session 跨輪 → 移除：

- `codebus-core/src/wiki/fix/session.rs`（整檔刪）
- `codebus-core/src/agent/claude_cli.rs::SessionAction` enum
- `InvokeAgentOptions::session` field
- spawn 時 `--session-id` / `--resume` 兩個 arg

當未來真有 session 需求（例如 v3-task-tool 或別的 change），重新引入。Per `feedback_dont_speculative_abstract` memory：no consumer = no abstract.

### vault internal `.gitignore` 加 `.claude/settings.local.json`

讓使用者自己加的 hook（local override）不會被 vault git 追蹤，符合 Claude Code 慣例。

## Risks / Trade-offs

- [Agent 在 session 內無限 loop] → Claude Code 自己有 max-turns / context budget 自然擋；codebus 不再加自己的 cap。失控 case 是 agent 跑了很多 token 後仍沒修好 → CLI final lint 報 exit 1 → 使用者重新跑 fix。可接受
- [Hook fail-closed 把 fix 整個搞死] → 真實 case：hook subcommand 有 bug，整個 vault 的 fix 都不能跑。Mitigation：(a) hook subcommand 內含完整單元測試；(b) 文件教使用者刪 `<vault>/.codebus/.claude/settings.json` 後重跑 init 補回正確版
- [使用者直觸 `/codebus-fix` 沒 hook 保護] → 設計取捨；使用者本人在現場是足夠 mitigation
- [BREAKING：移除 `--fix-max-iter` flag] → 既有腳本如果有用會被 clap reject。檢索 codebus repo 內無使用；外部 user 風險低（v3-lint 才剛 ship）
- [BREAKING：移除 `lint.fix.outer_ping_max` config key] → 已寫進 `~/.codebus/config.yaml` 的使用者：靜默忽略（unknown key forward-compat 機制；v3-lint 的 `lint_fix.rs` 已用 `serde(default)` 處理）；不會報錯，只是新值沒效果
- [BREAKING：移除 atomic contract 在 fix SKILL.md] → 既有 vault 已有 fix SKILL.md：write-if-missing 不會自動更新使用者已寫死的 SKILL.md。使用者需要手動刪 SKILL.md 後重跑 init，或接受用舊版 SKILL.md（仍能 work，因為新 CLI 不依賴 SKILL.md 講過的 atomic semantics）
- [BREAKING：移除 SessionAction enum] → 是 public surface 變動。檢索 repo 內 callers 只有 fix 用，goal/query 一直傳 None；外部使用者風險為零（codebus-core 是 internal lib，沒 published crate）

## Migration Plan

- **既有 v3-lint vault re-run init**：write-if-missing 加上新 `<vault>/.codebus/.claude/settings.json`、新 `.gitignore` 行（已存在的 `.gitignore` 用 `creates_or_appends_missing_required_lines` 邏輯），fix SKILL.md 不動（保留使用者可能改過的版本）。使用者要 atomic contract 移除生效需要手動刪 fix SKILL.md
- **既有 v3-lint config (`~/.codebus/config.yaml` 含 `lint.fix.outer_ping_max`)**：silently ignored；新版只讀 `lint.fix.enabled`
- **v3-lint 寫死的 fix SKILL.md atomic contract 段**：影響不大；新 CLI 流程跑單發 + final lint，agent 怎麼解讀「ONE round」「Loop control belongs to caller」都不影響結果
- **沒有資料 migration**（vault 內容 / lint 規則不變）

## Open Questions

- Hook 對 `codebus lint --format json` 是否 allow 沒爭議；對 `codebus lint --repo /some/other/path` 要不要 allow？目前 pattern 是 `codebus lint *` 通配，會 allow。妥當嗎？（agent 應該不會用 `--repo` 指外面的 vault，但 hook 不擋）→ 暫時保持寬鬆，spec 階段定。
- 未來如果有 `codebus query` skill 也想跑 lint（讓 agent 自己 lint 確認 vault 狀態），hook 機制也要擴及 query？目前 query 沒 Bash，hook 對它無效；要等真的需要時再開 follow-up change。
