# v3-fix-trust-agent 實作任務

## 1. Hook subcommand (codebus-cli/src/commands/hook.rs)

- [x] 1.1 新增 `codebus-cli/src/commands/hook.rs` 含 hidden 子命令 `codebus hook check-bash`（clap `#[command(hide = true)]`），實作「Hook 命令格式：`codebus hook check-bash`」設計決策的 stdin/stdout 介面契約
- [x] 1.2 [P] 實作 PreToolUse JSON stdin parser — 讀完整 stdin、`serde_json::from_str` 解析、抽出 `tool_input.command` 字串；parse 失敗走 fail-closed 分支（per Fix Bash Hook Installation 的 `fail-closed on malformed input` scenario）
- [x] 1.3 [P] 實作「Bash 命令 pattern 比對細節」設計決策 — argv 拆分（簡單空白）、第一 token basename normalize（去目錄、Windows case-insensitive `codebus.exe`/`codebus.EXE`、Unix case-sensitive）、第二 token 必須是 `lint`
- [x] 1.4 [P] 實作 allow / block 決策輸出 — allow: exit 0 不印 decision；block: exit 0 印 `{"decision":"block","reason":"<msg>"}` JSON 到 stdout
- [x] 1.5 把 `Hook` 子命令接進 `codebus-cli/src/main.rs` 的 `Command` enum 與 dispatch（`#[command(hide = true)]` 不在 `--help` 列出）
- [x] 1.6 加單元測試覆蓋 Fix Bash Hook Installation 的 6 個 hook scenario（bare `codebus lint *` allow、absolute path allow、non-codebus binary block、codebus 其他子命令 block、malformed JSON fail-closed block、Windows `.exe` 大小寫變體 allow）

## 2. Init 寫 settings.json + gitignore

- [x] 2.1 在 `codebus-core/src/skill_bundle/mod.rs`（或新增 `codebus-core/src/vault/settings.rs`）實作 settings.json writer — 寫到 `<vault_root>/.claude/settings.json`，內容含 `hooks.PreToolUse` 陣列指向 `codebus hook check-bash`，落實「settings.json 位置：只 vault-internal」設計決策
- [x] 2.2 套用 write-if-missing 語意（per Fix Bash Hook Installation 的 `Init does not overwrite existing settings.json` scenario）— 已存在則 byte-identical
- [x] 2.3 `codebus-cli/src/commands/init.rs` 呼叫 settings.json writer，順序在 skill bundles 步驟之後
- [x] 2.4 `codebus-cli/src/commands/init.rs` 的 vault internal `.gitignore` 寫入步驟（`merge_internal_gitignore` 或等效機制）加 `.claude/settings.local.json`，落實「vault internal `.gitignore` 加 `.claude/settings.local.json`」設計決策
- [x] 2.5 確認 init **不**寫 `<repo>/.claude/settings.json`（per Fix Bash Hook Installation 的 `Init does not write settings.json to repo root` scenario）
- [x] 2.6 加測試：fresh vault init 後 `<vault>/.claude/settings.json` parse 為合法 JSON 含 PreToolUse Bash hook；既有 settings.json 不被覆寫；vault internal `.gitignore` 含 `.claude/settings.local.json` 行；source repo `<repo>/.claude/settings.json` 不存在

## 3. Drop SessionAction / session.rs / 跨輪機制

- [x] 3.1 刪除整檔 `codebus-core/src/wiki/fix/session.rs`，落實「Drop SessionAction / session.rs 整套」設計決策
- [x] 3.2 從 `codebus-core/src/agent/claude_cli.rs` 移除 `SessionAction` enum 與 `InvokeAgentOptions::session` field
- [x] 3.3 `invoke()` 函式拿掉 `--session-id` / `--resume` 兩個 spawn arg
- [x] 3.4 從 `codebus-core/src/agent/mod.rs` `pub use` 移除 `SessionAction`
- [x] 3.5 [P] 更新 `codebus-cli/src/commands/goal.rs` 移除 `session: None` 欄位（struct 縮減後不再有此 field）
- [x] 3.6 [P] 更新 `codebus-cli/src/commands/query.rs` 移除 `session: None` 欄位
- [x] 3.7 更新 `codebus-core/src/agent/claude_cli.rs::tests` 移除 SessionAction 相關 assertions

## 4. Config / flag 簡化

- [x] 4.1 從 `codebus-core/src/config/lint_fix.rs::LintFixConfig` 移除 `outer_ping_max` field（per Fix Loop Configuration 簡化）
- [x] 4.2 簡化 `LintFixConfig::merge_cli_overrides` 簽名 — 從 `(no_fix, fix_max_iter)` 改為 `(no_fix)` 單參數
- [x] 4.3 [P] 在 `LintFixConfig` 反序列化保留 forward-compat — `lint.fix.outer_ping_max` 不引發 parse error 但靜默忽略（per Fix Loop Configuration 的 `Legacy outer_ping_max key is silently ignored` scenario）
- [x] 4.4 [P] 從 `codebus-cli/src/main.rs::Cli` struct 移除 `--fix-max-iter` 全域 flag（per Fix Subcommand Behavior 與 Goal Subcommand Behavior 的 `--fix-max-iter is no longer recognized` scenarios）
- [x] 4.5 更新 `lint_fix.rs::tests` 移除 `outer_ping_max` 與 `fix_max_iter` 相關測試；加 legacy key forward-compat 測試

## 5. Trust-agent 單發 fix loop

- [x] 5.1 重寫 `codebus-core/src/wiki/fix/mod.rs::run_fix_loop` 為「Trust-agent single-shot 模型」設計決策的單發流程 — 簽名簡化為 `fn run_fix_loop(vault_root: PathBuf) -> Result<FixReport, FixError>`，內部僅 spawn 一次 agent + 不做外層 ping，落實 Fix Single-Shot Verification 的 5 步流程；同時移除 v3-lint 既有 Fix CLI Outer Ping Loop 程式邏輯（要符合 REMOVED Fix CLI Outer Ping Loop 規範）
- [x] 5.2 簡化 `FixLoopReport` 結構 — 移除 `agent_invocations` 多輪計數（永遠 0 或 1）、移除 `TerminationReason::PingBudgetExhausted` enum variant
- [x] 5.3 [P] 刪除 `codebus-core/src/wiki/fix/prompt.rs::followup_prompt` 函式與相關測試（單發模型不需要跨輪 prompt）；保留 `initial_prompt` 或 inline 進 `mod.rs`
- [x] 5.4 [P] 加測試：spawn 後 lint 還有 issue 時不再 spawn；spawn 命令列無 `--session-id`/`--resume`/`--continue`（per Fix Single-Shot Verification 的 `Fix spawn arguments contain no session continuity flags` scenario）

## 6. Fix CLI verb 更新

- [x] 6.1 從 `codebus-cli/src/commands/fix.rs::run` 簽名移除 `fix_max_iter: Option<u32>` 參數
- [x] 6.2 配合 5.1 新 `run_fix_loop` 簽名更新呼叫處
- [x] 6.3 落實 Fix Subcommand Behavior 與 Standalone Fix Mode 的 6 步流程（lint precheck / spawn 一次 / wait / final lint / commit `wiki: lint fix loop` / exit）
- [x] 6.4 更新 `codebus-cli/src/main.rs` 的 `Fix` dispatch 不再傳 `fix_max_iter`
- [x] 6.5 更新 `codebus-cli/tests/fix_flow.rs` — 移除 `fix_max_iter_flag_overrides_default` 測試；改加 `--fix-max-iter` 被 clap 拒絕的測試；新增「spawn 一次」測試（驗證 mock-claude.log 只記錄一次 invocation）

## 7. Goal flow 整合更新

- [x] 7.1 從 `codebus-cli/src/commands/goal.rs::run` 簽名移除 `fix_max_iter` 參數
- [x] 7.2 lint-and-fix phase 改呼叫 5.1 的新 `run_fix_loop(vault_root)` 簽名
- [x] 7.3 落實 Goal Subcommand Behavior 既有 6 步順序（步驟內容不變，只是不再 forward `--fix-max-iter`）
- [x] 7.4 更新 `codebus-cli/src/main.rs` 的 `Goal` dispatch 不再傳 `fix_max_iter`
- [x] 7.5 更新 `codebus-cli/tests/goal_flow.rs` — 移除 `fix_max_iter` forwarding 相關測試；加 `--fix-max-iter` 被 clap 拒絕測試

## 8. Fix SKILL.md 改寫

- [x] 8.1 重寫 `codebus-core/src/skill_bundle/mod.rs::FIX_WORKFLOW` 字串常數 — 移除 atomic contract 段、移除「ONE round of repair」「Loop control belongs to the caller」「MUST NOT spawn nested fix invocations or loop internally」字面語句，落實 Fix SKILL.md Atomic Contract 的新描述（requirement heading 保留 v3-lint 名稱以維持 spec 穩定，但內容全面重寫）
- [x] 8.2 [P] 加入「self-directed loop」段 — 描述 agent 可在 session 內自由 lint+edit 多輪（受 Bash hook 限定只能跑 codebus lint），CLI 只當 final verifier
- [x] 8.3 [P] 維持「Trust the absolute paths」段（agent 用 lint JSON `issues[].path` 絕對路徑直接餵 Read/Write/Edit；該段在 v3-lint 已寫，不動）
- [x] 8.4 更新 `skill_bundle/mod.rs::tests` — 移除 `fix_workflow_body_has_no_loop_control_imperatives` 對 atomic 字面語句的反向 assert；改為 assert 新的 trust-agent 描述存在（per Fix SKILL.md Atomic Contract 的三個 scenario）

## 9. 既有 cli_routing 測試掃尾

- [x] 9.1 檢查 `codebus-cli/tests/cli_routing.rs` 對 `--fix-max-iter` 全域 flag 是否有殘留 reference；移除
- [x] 9.2 確認 `--no-fix` 仍是有效 global flag，相關測試（如有）保留

## 10. Verification

- [x] 10.1 跑 `cargo test -p codebus-core` 確認 wiki/fix、config/lint_fix、skill_bundle、agent 各模組單元測試通過
- [x] 10.2 跑 `cargo test -p codebus-cli` 確認 lint_flow / fix_flow / goal_flow / query_flow / cli_routing 全綠
- [x] 10.3 手動 e2e 對 sample dirty vault 跑 `codebus fix` → 觀察 spawn 一次（不再多輪 ping）+ `wiki: lint fix loop` commit
- [x] 10.4 手動 e2e 確認 hook 真擋：在 vault 內以 `claude -p "echo TEST" --tools Bash --allowedTools Bash --permission-mode acceptEdits` 試跑，確認被 PreToolUse Bash hook block
- [x] 10.5 手動 e2e 確認 hook 真放：`claude -p "codebus lint --format json" --tools Bash --allowedTools Bash --permission-mode acceptEdits` 在 vault 內可通過 hook 執行
- [x] 10.6 手動驗證 BREAKING：`codebus fix --fix-max-iter 5` 被 clap reject、`codebus goal "X" --fix-max-iter 5` 被 clap reject
- [x] 10.7 手動驗證 forward-compat：`~/.codebus/config.yaml` 含 `lint.fix.outer_ping_max: 10` 時 `codebus fix` 不報錯（值靜默忽略）
- [x] 10.8 手動驗證 既有 v3-lint vault re-run init 後 `<vault>/.claude/settings.json` 補上、既有 `<vault>/.claude/skills/codebus-fix/SKILL.md` 不被覆寫（write-if-missing）
