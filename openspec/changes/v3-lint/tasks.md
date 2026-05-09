# v3-lint 實作任務

## 1. Lint module port (codebus-core/src/wiki/lint/)

- [x] 1.1 在 `codebus-core/src/wiki/` 新增 `types.rs`（或擴充既有處）定義 `PageType`、`LintIssue`、`LintSeverity`、`LintResult` 型別 — 從 `legacy/v2-rust/codebus-core/src/wiki/types.rs` 移植，並補上 `rule_id` 欄位（kebab-case 規則識別字串）支援 Lint Output Formats 的 JSON 輸出
- [x] 1.2 [P] 實作 Vault 自動偵測 / Vault Root Auto-Detection 在 `codebus-core/src/wiki/lint/locate.rs` — 4 步優先序（cwd 有 wiki/、cwd 有 .codebus/wiki/、--repo 顯式、報錯）
- [x] 1.3 [P] 實作 Lint Rule Set 規則 1（frontmatter parse failure）— 套用既有 `parsePage` 等價邏輯，parse 失敗回 error
- [x] 1.4 [P] 實作 Lint Rule Set 規則 2（cross-folder slug collision）— 在 `lint/rules.rs` 內掃描全 5 個 folder 的 slug 衝突
- [x] 1.5 [P] 實作 Lint Rule Set 規則 3（misplaced root page）— 排除 `index.md`、`log.md`，其餘 `wiki/*.md` 為 warn
- [x] 1.6 [P] 實作 Lint Rule Set 規則 4 與 5（frontmatter related[] 格式驗證 + slug 解析）— 解析 `[[wikilink]]` 格式、檢查 slug 存在
- [x] 1.7 [P] 實作 Lint Rule Set 規則 6（body wikilink resolution）— 含 fenced 跟 inline code 區塊 strip 預處理
- [x] 1.8 [P] 實作 Lint Rule Set 規則 7（nav file presence and integrity）— `index.md`、`log.md` 缺檔 warn + body wikilink 掃描
- [x] 1.9 在 `codebus-core/src/wiki/lint/mod.rs::lint_wiki()` 串接所有規則，回傳 `LintResult` 含 `pages_scanned`、`nav_files_scanned`、`issues`、`error_count`、`warn_count`
- [x] 1.10 加測試驗證 Lint Read-Only Invariant — 在 dirty vault 跑 lint，assert vault 所有檔 byte-identical before/after

## 2. Lint Output Formats (codebus-core/src/wiki/lint/output.rs)

- [x] 2.1 [P] 實作 text 格式器（雙格式輸出之一）— vault-relative path、依檔分組、含 coverage 摘要行（pages + nav files scanned + error/warn counts）
- [x] 2.2 [P] 實作 JSON 格式器（雙格式輸出之二）— 絕對路徑、`vault_root` 欄位、單一 JSON 物件不含人類文字 / emoji / ANSI
- [x] 2.3 加測試驗證 Lint Output Formats — text 格式不洩漏絕對路徑前綴、JSON 格式 `stdout` 可被 `serde_json` 完整 parse

## 3. Lint CLI verb (codebus-cli/src/commands/lint.rs)

- [x] 3.1 從 stub 改寫 `lint.rs` 落地 Lint Subcommand Behavior — 接 `--repo` / `--format` / `--debug`、呼叫 Vault 自動偵測 / Vault Root Auto-Detection、跑 `lint_wiki()`、emit 選定格式
- [x] 3.2 實作 Lint Subcommand Behavior 退出碼邏輯 — 0（無 error）/ 1（有 error）/ 2（無 vault）；warning 不影響退出碼
- [x] 3.3 加 CLI 整合測試 `codebus-cli/tests/lint_flow.rs` — 涵蓋 `codebus lint`、`codebus lint --format json`、`codebus lint --repo <path>`、vault 缺檔情境

## 4. Fix Loop Configuration & flags

- [ ] 4.1 在 `codebus-core/src/config/lint_fix.rs` 實作 Fix Loop Configuration schema — `lint.fix.{enabled, outer_ping_max}` 預設 `true / 2`，從 `~/.codebus/config.yaml` 載入
- [ ] 4.2 在 `codebus-cli/src/main.rs` 加 `--no-fix`（boolean）與 `--fix-max-iter <N>`（positive int）CLI flag 定義，goal 與 fix 子命令共用
- [ ] 4.3 實作 config 與 flag merge 規則 — `--no-fix` 取代 `enabled`、`--fix-max-iter` 覆寫 `outer_ping_max`，兩者並存時 `--no-fix` 贏

## 5. Fix loop foundation (codebus-core/src/wiki/fix/)

- [ ] 5.1 在 `codebus-core/src/wiki/fix/session.rs` 實作 Session continuity 工具 — UUID v4 產生、`--session-id <uuid>` 與 `--resume <uuid>` argument builder
- [ ] 5.2 在 `wiki/fix/mod.rs` 內實作 Fix Loop Agent Sandbox argument builder（Bash whitelist 細粒度）— 拼出 `Read,Glob,Grep,Write,Edit,Bash(codebus lint *)` 字串、`--tools` 與 `--allowedTools` 兩處同步
- [ ] 5.3 [P] 實作 Fix CLI Outer Ping Loop 的 agent-driven self-loop 控制流（CLI 外層 ping 機制）— initial spawn、lint check、ping with `--resume`、依 `outer_ping_max` 終止
- [ ] 5.4 [P] 在 `wiki/fix/prompt.rs` 實作 follow-up prompt 組裝器 — 把剩餘 lint issues serialize 進 prompt body，給外層 ping 用
- [ ] 5.5 加測試覆蓋 Fix CLI Outer Ping Loop 場景：initial lint 乾淨直接退出、post-lint 乾淨終止、`outer_ping_max + 1` 次後仍有 issue 終止

## 6. Fix SKILL.md content

- [x] 6.1 在 `codebus-core/src/skill_bundle/mod.rs` 撰寫 Fix SKILL.md Atomic Contract 內容 — 描述 agent 任務為「拿 lint issues、修對應 wiki/ 檔、結束」；不含 loop / iterate / retry 等自我控制語彙
- [x] 6.2 替換既有 fix stub workflow placeholder（`workflow_section("fix")` 那段），改為新的 fix workflow 字串常數
- [x] 6.3 加測試 assert Fix SKILL.md Atomic Contract — body 不含 "iteration" / "iterate" / "loop" / "retry" / "again" 等 loop 詞彙在自我控制語境
- [x] 6.4 加測試 assert fix SKILL.md 教 agent 用 `codebus lint --format json` 取得 issues 且使用絕對路徑（`issues[].path`）直接餵給 Read/Write/Edit

## 7. Skill bundle 雙位址寫入

- [x] 7.1 修改 Skill Bundle Layout — 擴充 `write_bundles_if_missing` 簽名接受 `vault_root` 與 `repo_root` 兩個路徑參數，雙位址寫入 `<vault>/.codebus/.claude/skills/codebus-{verb}/` 與 `<repo>/.claude/skills/codebus-{verb}/`
- [x] 7.2 確認 Write-If-Missing Semantics 在兩個位址各自獨立判斷 — 不跨位址 propagate 內容、各自 skip 既有檔
- [x] 7.3 加測試：vault-internal SKILL.md 已存在但 repo-root 缺，只寫 repo-root 那份
- [x] 7.4 加測試：vault 與 repo-root 兩份 SKILL.md 對同一 verb 內容 byte-identical

## 8. Init wiring

- [x] 8.1 修改 `codebus-cli/src/commands/init.rs` 呼叫 `write_bundles_if_missing` 帶入 `<vault_root>` 與 `<repo>` 兩參數
- [x] 8.2 修改 init 的 source `.gitignore` mutation 步驟 — 加 `.claude/skills/codebus-goal/`、`codebus-query/`、`codebus-fix/` 三條 ignore pattern
- [x] 8.3 加測試：init 後既不在 vault 也不在 repo-root 創 `codebus-lint/` skill bundle

## 9. Fix CLI verb (codebus-cli/src/commands/fix.rs)

- [ ] 9.1 從 stub 改寫 `fix.rs` 落地 Fix Subcommand Behavior — vault precondition (exit 2 if missing)、`--no-fix` short-circuit (exit 0 + stderr message)、lint pre-check (skip loop if clean)、call fix loop、auto-commit
- [ ] 9.2 實作 Standalone Fix Mode 的 `wiki: lint fix loop` commit message 與「無變動 = no-op commit」行為
- [ ] 9.3 加 CLI 整合測試 `codebus-cli/tests/fix_flow.rs`：vault-missing exit 2、clean vault 跳過 agent、`--no-fix` short-circuit、ping budget 用完後 commit + exit 1
- [ ] 9.4 [P] 加測試驗證 Fix Subcommand Behavior 用 `--fix-max-iter 5` 覆寫 config 的 `outer_ping_max: 2`

## 10. Goal flow integration

- [ ] 10.1 修改 Goal Subcommand Behavior 步驟序列 — 在 goal agent 結束與 auto-commit 之間插入 lint-and-fix phase；變更 step count 從 5 → 6
- [ ] 10.2 將 `--no-fix` / `--fix-max-iter` 從 goal CLI flags 轉發進 fix loop call
- [ ] 10.3 實作 Goal commit 摺單顆 — 單一 `auto_commit("wiki: <goal-text>")` 涵蓋 goal agent 寫入 + fix loop 修改
- [ ] 10.4 更新 Goal Subcommand Behavior 退出碼優先序 — goal agent 失敗優先於 fix exit code，但 auto-commit 失敗最高優先
- [ ] 10.5 加 / 改 `codebus-cli/tests/goal_flow.rs` 測試：lint-and-fix 跑在 agent 結束與 commit 之間、`--no-fix` 跳過 fix phase、fix edits 跟 ingest 寫入摺進同一 commit

## 11. cli spec 一致性 (existing openspec/specs/cli/spec.md sync)

- [ ] 11.1 修改 Debug Flag Output 既有 spec 文字 — 移除「stub verbs 接受 --debug 但不發 [debug] line」段落，改為 per-verb 行為定義
- [ ] 11.2 確認 `Stub Verb Exit Behavior` requirement 移除後無任何 runtime check 或測試還在引用該語義

## 12. /codebus-fix 使用者直觸 模式驗證

- [x] 12.1 加測試模擬 /codebus-fix 使用者直觸（無 CLI 外層）— 直接呼叫 fix SKILL.md 載入後的 atomic 契約、assert agent self-loop 在無 outer ping 下也能跑（loop 控制權在 agent，不依賴 CLI）
- [x] 12.2 在 fix SKILL.md 撰寫文末註記說明：互動模式（user direct）與 CLI spawn 模式共用同一 SKILL.md，但 CLI 模式才有 outer ping 與 auto-commit 強保證

## 13. Verification

- [ ] 13.1 跑 `cargo test -p codebus-core` 確認所有 lint/fix module 單元測試通過
- [ ] 13.2 跑 `cargo test -p codebus-cli` 確認所有 CLI flow 整合測試通過
- [ ] 13.3 手動 e2e 對 sample vault 跑 `codebus init` → `codebus goal "..."` → 觀察 lint+fix log → `git -C .codebus log` 確認 commit 摺單顆
- [ ] 13.4 手動 e2e 對 dirty vault 跑 `codebus fix` → 確認 agent 修檔 + outer ping 補救 + `wiki: lint fix loop` commit
- [ ] 13.5 手動 e2e 在 source repo root 開 Claude Code，使用 `/codebus-query`、`/codebus-fix` skill 直觸成功
