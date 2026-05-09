## Why

codebus-cli 目前的 `lint` 與 `fix` 仍是 stub（只印 `not yet implemented`）。使用者無法驗證 vault `wiki/` 是否符合 schema，goal agent 寫完的內容也沒有自動格式校正。v3 架構已選 agentic AI product 定位（goal/query 都是 spawn `claude -p` agent），但 lint → fix 工作流尚未落地，定位不完整。本 change 把 lint 與 fix 一次落地，以 agent-driven fix loop 完成「使用者下 goal → vault 自動產生且格式乾淨」的端到端體驗。

## What Changes

- **`codebus lint` 從 stub 變實作**：移植 legacy TS 的 7 條 lint 規則到 Rust，輸出 text + JSON 雙格式（JSON 用絕對路徑、text 用 vault-relative path）。lint 永遠 read-only。新增 vault 自動偵測（依 cwd 判斷自己在 source repo root 還是 vault root），讓 CLI 跟 agent 自呼都能跑。
- **`codebus fix` 從 stub 變實作（agent-driven loop）**：CLI 內 spawn `claude -p` 並指定 `--session-id <uuid>`，sandbox 為 `Read,Glob,Grep,Write,Edit,Bash(codebus lint *)`（細粒度 whitelist，禁絕其他 binary）。agent 自跑 `codebus lint --format json` 看 issues、修檔、再跑 lint，session 內 self-loop。agent 結束後 CLI 跑 final lint 校驗；若仍有 issue 且 ping budget > 0 就用 `--resume <uuid>` 帶 follow-up prompt 喚起 agent 補修，最多 1-2 次。
- **fix SKILL.md 寫入完整 workflow**：atomic「拿 lint issues → 修對應 wiki/ 檔 → 結束」契約。loop 由 caller 持有（CLI 或使用者），SKILL.md 不含 loop 邏輯。
- **goal flow 整合 auto-fix**：goal agent 結束後 CLI 內部接著跑 lint → fix loop，commit 摺單顆 `wiki: <goal-text>`（沿用 v2 慣例，把 ingest 寫入 + fix 修改摺在一個 commit）。**BREAKING**：v3-goal 現行「agent 結束即 auto-commit」改為「lint → fix loop 結束才 commit」。
- **init 三個 skill 多寫一份到 source repo `.claude/skills/`**：goal/query/fix 三個 skill bundle 同時寫到 `<repo>/.codebus/.claude/skills/`（既有路徑，CLI spawn 用）與 `<repo>/.claude/skills/`（新增，讓使用者在 source repo root 開 Claude Code 直觸 `/codebus-{goal,query,fix}` 也找得到）。**BREAKING**：與 `skill-bundles` 既有 spec「SHALL NOT write to `<repo>/.claude/skills/`」相反，必須 patch。
- **新增 config**：`lint.fix.{enabled, outer_ping_max}` defaults `true / 2`。讀取自 `~/.codebus/config.yaml`（沿用 v2 全域 config 慣例）。
- **新增 CLI flags**：`--no-fix`（停用 fix 整段，goal/fix 命令共用）、`--fix-max-iter <N>`（覆寫 outer_ping_max）。

## Capabilities

### New Capabilities

- `lint-feedback-loop`: lint 規則集、fix loop（agent self-loop + CLI 外層 ping）、JSON/text 雙格式、agent session pattern（`--session-id`/`--resume`）、config 與 CLI flags 的 spec 集中地。沿用 v2 archived 同名 capability 的命名。

### Modified Capabilities

- `cli`: `lint` 與 `fix` 從 Stub Verb Exit Behavior 移除；新增兩個 verb 的 invocation contract（含 sandbox flags、cwd、commit 行為）；`goal` 的 commit timing 從「agent 結束即 commit」改為「lint → fix loop 結束才 commit」。
- `skill-bundles`: 既有「SHALL NOT write to `<repo>/.claude/skills/`」鬆綁為「SHALL ALSO write to `<repo>/.claude/skills/`」，改為雙位址寫入。

## Impact

- Affected specs: `lint-feedback-loop` (new), `cli` (modified), `skill-bundles` (modified)
- Affected code:
  - New:
    - codebus-core/src/wiki/lint/mod.rs
    - codebus-core/src/wiki/lint/rules.rs
    - codebus-core/src/wiki/lint/locate.rs
    - codebus-core/src/wiki/lint/output.rs
    - codebus-core/src/wiki/fix/mod.rs
    - codebus-core/src/wiki/fix/prompt.rs
    - codebus-core/src/wiki/fix/session.rs
    - codebus-core/src/config/lint_fix.rs
  - Modified:
    - codebus-cli/src/commands/lint.rs
    - codebus-cli/src/commands/fix.rs
    - codebus-cli/src/commands/goal.rs
    - codebus-cli/src/main.rs
    - codebus-core/src/skill_bundle/mod.rs
    - codebus-cli/src/commands/init.rs
