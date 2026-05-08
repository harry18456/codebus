## 1. config schema 擴展

- [ ] 1.1 [P] Write failing tests for `LintConfig.auto_fix` defaults：missing `auto_fix` 段→`enabled=true, max_iterations=5`；明確設 `enabled: false`、`max_iterations: 10` 解析正確（落地 **Default config enables fix with max iterations five**）
- [ ] 1.2 Implement `AutoFixConfig` 子結構在 `codebus-core/src/config/schema.rs`：加 `auto_fix: AutoFixConfig` 欄位、`AutoFixConfig { enabled: bool, max_iterations: u32 }`、`Default` impl 預設 `enabled=true, max_iterations=5`
- [ ] 1.3 Implement `lint.auto_fix.enabled` + `lint.auto_fix.max_iterations` 解析在 `codebus-core/src/config/loader.rs`：對應 type-mismatched 走 `warn_type_mismatch`、unknown sub-field silently ignored（沿用既有 tolerance contract）

## 2. fix module 骨架

- [ ] 2.1 [P] Write failing tests for `git_diff_summary(vault_root, base_sha)`：vault clean 回空字串、有變動回 git diff output、`<base_sha>` 不存在時 fall through 到 empty string（不 panic）
- [ ] 2.2 Implement `codebus-core/src/wiki/fix/memory.rs::git_diff_summary` — shell out 到 `git diff <base_sha> -- wiki/`、UTF-8 lossy 轉成 String；最大長度截斷（避免 massive diff 塞爆 prompt，e.g. 30 KB cap）
- [ ] 2.3 [P] Write failing tests for `build_fix_prompt(issues, prior_diff)`：第一輪 `prior_diff = None` 不出現 `<previous_attempt>` block；第二輪 `prior_diff = Some(...)` 含此 block；XML-ish tags 結構正確；issues 全部按 `LintIssue.path` 出現（落地 **Per-iteration prompt batches all current issues with prior diff as memory**）
- [ ] 2.4 Implement `codebus-core/src/wiki/fix/prompt.rs::build_fix_prompt(issues: &[LintIssue], prior_diff: Option<&str>) -> String` — 用 XML-ish tags 構造 prompt body；per-rule fix hints 嵌在 prompt 內 inline
- [ ] 2.5 [P] Write failing tests for `FixReport` enum：`Clean { iterations }` 與 `MaxIter { iterations, remaining_issues }` 兩 variant、`iterations` 計數正確
- [ ] 2.6 Implement `codebus-core/src/wiki/fix/mod.rs::FixReport` enum + serialize-friendly Debug

## 3. lint_and_fix 主循環

- [ ] 3.1 [P] Write failing test for 0-issue 短路：vault 初始 lint clean → return `Clean { iterations: 0 }`；mock provider 的 `invoke` 沒被呼叫過（落地 **Skip the loop entirely when initial lint reports zero issues** + **Clean vault produces zero LLM invocations**）
- [ ] 3.2 [P] Write failing test for max_iter 終止：mock provider 故意不修任何 issue（每輪 invoke 都 no-op），跑 `max_iterations=3` → return `MaxIter { iterations: 3, remaining_issues }`；`invoke` 被呼叫 3 次（落地 **Loop terminates by max iterations cap when issues remain**）
- [ ] 3.3 [P] Write failing test for clean 終止：mock provider 第一輪 invoke 後 lint 變 clean → return `Clean { iterations: 1 }`；`invoke` 只被呼叫 1 次（落地 **Loop terminates with clean state when all issues are fixed**）
- [ ] 3.4 [P] Write failing test for prior diff snapshot：fix loop 開始前 `git rev-parse HEAD` 取一次 sha；之後每 iter 對照同一個 base diff（不是 iter-to-iter diff）；測 base sha 在第 N 輪時與第 1 輪相同
- [ ] 3.5 [P] Write failing test for 全 7 條 rules 都進 prompt：構造 lint result 含所有 7 條 rules 各一個 issue（含 duplicate_slug 與 unexpected_file）；驗證 `build_fix_prompt` output 含全部 7 條（落地 **Duplicate slug issues are forwarded to the LLM** + **Unexpected-file issues are forwarded to the LLM**）
- [ ] 3.6 Implement `codebus-core/src/wiki/fix/mod.rs::lint_and_fix(vault_root, provider, max_iterations) -> io::Result<FixReport>`：snapshot base sha → 0-issue 短路 → loop（lint → 終止判定 → build_prompt → invoke → re-lint）；每輪 stderr 輸出 `lint fix iteration N/M: K issues` 進度（落地 **Provide a single lint-and-fix function shared by both entry points** + **Terminate when issues clear or max iterations reached** + **All lint rules participate in the fix loop**；對應 design 決定 **fix loop 模組位置：`codebus-core/src/wiki/fix/`**、**終止條件：兩條，相信循環**、**全部 7 條 rules 都走 LLM**、**Prompt 結構：單一 batched，含上一輪 diff**、**`--goal` 與 `--fix` 共用同一函數**）

- [ ] 3.7 [P] Write test for **LlmProvider trait is unchanged**：在 `codebus-core/src/llm/provider.rs` 加 lock-in test，斷言 `InvokeOptions` struct 的 fields 是 `system_prompt, user_message, mode, cwd, vault_root`（不多不少）、trait 只有 `invoke` 與 `cancel` 兩個 method（落地 **LlmProvider trait is unchanged**；對應 design 決定 **「上一輪做了什麼」記憶來源：`git diff wiki/`** 中「trait 不動」的承諾）

## 4. goal flow 整合

- [ ] 4.1 [P] Write failing test for goal flow 預設接 lint_and_fix：mock provider、cfg.lint.auto_fix.enabled = true → `run_goal` 跑完，`lint_and_fix` 被呼叫一次；驗證在 `lint_wiki` 之後、`auto_commit` 之前（落地 **Default goal run triggers the fix loop after lint** + **Auto-commit happens once after fix loop terminates**）
- [ ] 4.2 [P] Write failing test for `RunGoalOptions.fix_disabled = true` 短路：fix_disabled 開時 `lint_and_fix` 不被呼叫；goal flow 仍 commit（落地 **--no-fix flag skips the fix loop in goal flow** + **Disabled config skips the fix loop in goal flow**）
- [ ] 4.3 Implement `RunGoalOptions` 加兩個 fields：`fix_disabled: bool`、`fix_max_iterations: u32`；在 `run_goal` 中 lint 之後 `if !fix_disabled { lint_and_fix(p.root, provider, fix_max_iterations)? }` 接邏輯；尊重 fix_disabled 與 max_iter 兩個 override（落地 **Goal flow auto-runs lint_and_fix after ingest completes**；對應 design 決定 **`--goal` 與 `--fix` 共用同一函數**）

## 5. --fix CLI mode

- [ ] 5.1 [P] Write failing test for `run_fix` 函數：vault 存在 + mock provider → 跑完後 `lint_and_fix` 被呼叫、`auto_commit` 被呼叫且 commit message 標 `wiki: lint fix loop`（落地 **--fix mode commits its results to the nested vault git repo**）
- [ ] 5.2 [P] Write failing test for `run_fix` 缺 vault：`<repo>/.codebus/` 不存在 → 回 `io::Error` 訊息含 "codebus init" 或 "codebus --goal" 提示（落地 **--fix mode requires an existing vault**）
- [ ] 5.3 [P] Write failing test for `run_fix` 不跑 ingest：mock provider 收到的 invoke 應該不是 goal-style prompt（system_prompt 不含 schema + index + goal）；驗證 `sync_repo_to_raw` 沒被呼叫（在 raw_dir 留 sentinel file 看會不會被 wipe）（落地 **--fix mode skips ingest**）
- [ ] 5.4 Implement `codebus-cli/src/commands/fix.rs::RunFixOptions` + `run_fix(opts, renderer, log_sink)`：lint vault → `lint_and_fix` → `auto_commit("wiki: lint fix loop")`；vault 不存在時 `io::Error` 訊息引導使用者
- [ ] 5.5 Implement `codebus-cli/src/main.rs::Cli` 加 `--fix: bool`、`--no-fix: bool`、`--fix-max-iter: Option<u32>` 三個 clap field；`dispatch` 中 `--fix` 路徑優先於 `--goal` 與 `--query` 之後、`--check` 之前；新增 `run_fix_cmd`（落地 **Standalone --fix CLI mode targets existing vaults without ingest**；對應 design 決定 **CLI override：`--no-fix` + `--fix-max-iter N`**）

## 6. CLI override 解析

- [ ] 6.1 [P] Write failing test for resolve fix config：`--no-fix` 出現 → 回傳結構 `enabled=false`；同時有 `--no-fix` 與 `--fix-max-iter 10` → `enabled=false`（max_iter 無作用）；只 `--fix-max-iter 7` → `enabled` 走 config 預設、`max_iterations=7`（落地 **--no-fix wins when both flags are present** + **--fix-max-iter overrides config max_iterations**）
- [ ] 6.2 Implement `resolve_fix_config(cli, cfg) -> (fix_disabled, max_iterations)` 在 `main.rs`：`fix_disabled = cli.no_fix || !cfg.lint.auto_fix.enabled`；`max_iterations = cli.fix_max_iter.unwrap_or(cfg.lint.auto_fix.max_iterations)`；同樣作用於 goal 與 fix 兩 path（落地 **Auto-fix is configurable via global config and CLI overrides**；對應 design 決定 **Config schema：`lint.auto_fix` 子結構** + **CLI override：`--no-fix` + `--fix-max-iter N`**）

## 7. --check 不被影響

- [ ] 7.1 Write test：跑 `--check` 時 mock provider 的 `invoke` 永遠不被呼叫（trait object 用 fail-on-call 實作）；確認 `lint_and_fix` 不被觸發（落地 **--check stays read-only** + **--check mode is unchanged by this capability**）

## 8. Conformance gates

- [ ] 8.1 跑 `cargo test --workspace` 全綠（既有 207 + 新增 ~14 tests，無 regression）
- [ ] 8.2 跑 `target/release/codebus.exe --repo D:/side_project/uv --check` 與 `tests/fixtures/uv-vault-snapshot/check-output.txt` byte-equal — 確認 --check 路徑沒 regress
- [ ] 8.3 跑 `cargo clippy --workspace -- -D warnings` clean、`cargo fmt --all -- --check` clean
- [ ] 8.4 buddy-gacha smoke：對 buddy-gacha 跑 `codebus --goal "..."`（預設開 fix），觀察 stderr 顯示 fix iteration 數、實際修了什麼；驗證 ExitCode = 0、auto_commit 一次（不是兩次）
- [ ] 8.5 buddy-gacha smoke 第二輪：跑 `codebus --fix` 對既有 vault，確認獨立 path 正常（沒 raw_sync、沒 ingest LLM 呼叫、commit message 為 `wiki: lint fix loop`）
- [ ] 8.6 buddy-gacha smoke 第三輪：跑 `codebus --goal "..." --no-fix`，確認 escape hatch 真的關掉 fix loop（行為等同 0.2.0、auto_commit 一次但 message 不含 fix loop 字樣）

## 9. Final commit + archive

- [ ] 9.1 Final commit：`feat(fix): lint feedback loop with --goal auto-fix and standalone --fix mode`（單一 commit；如 cool-down 期間發現 regression，依 design 的 **Rollback 策略**：`git revert <hash>` 回前一 commit）
- [ ] 9.2 `spectra archive lint-feedback-loop`
