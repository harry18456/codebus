## 1. Verify spec aligns with shipped code

對每條 requirement 對照 `src/core/wiki/lint.ts` / `src/ui/lint-report.ts` / `src/commands/{goal,check}.ts` / 既有 tests，確認 spec 沒寫進實際不存在的行為。如果發現 spec 跟 code 真的不一致，**不在這個 change 裡修 code**——分裂出獨立 bug-fix change。

- [x] [P] 1.1 Auto-lint runs at end of every ingest in soft mode：對 `src/commands/goal.ts` 的 `runGoal` 結尾流程確認 lint 在 enrich/stale-detect 之後、autoCommit 之前跑、try/catch 吞 error、結果回傳給 caller。確認 `tests/commands/goal.test.ts` 涵蓋 `lint: null` 的 fallback case。
- [x] [P] 1.2 Standalone `--check` command runs lint as a read-only operation：對 `src/commands/check.ts` 跟 `src/cli.ts` 的 `--check` 分支確認沒呼叫 provider、沒跑 init、沒 commit、沒 mkdir、exit code 對 errorCount 0/>0 二分。確認 `tests/commands/check.test.ts` 涵蓋兩個 exit case + missing vault throw。
- [x] [P] 1.3 Lint enforces frontmatter and related[] integrity at error severity：對 `src/core/wiki/lint.ts` 的 §3 page-walk 段（含 frontmatter parse + related[] format check + slug existence）跟 `tests/core/wiki/lint.test.ts` 對應 3 個 test case 確認嚴重度都是 error。
- [x] [P] 1.4 Lint emits warnings for structural and Obsidian-compatibility violations：對 `src/core/wiki/lint.ts` 的 §2 collision check + §3 folder/type mismatch + §4 wiki/ root walk + §5 special files presence check 確認嚴重度都是 warn。對 5 個對應 test 比對。
- [x] [P] 1.5 Wikilink catalog includes nav files and goal guides as valid targets：對 `src/core/wiki/lint.ts` §1b（special files 加進 pageSlugs）跟 §1c（goal guides 加進 pageSlugs）+ 條件「only if file exists」確認。對 test「resolves wikilinks pointing at existing special files」+「resolves [[goal-slug]] wikilink」核對。
- [x] [P] 1.6 Lint scans body wikilinks in nav files and goal guides：對 `src/core/wiki/lint.ts` §5（special files body scan）+ §6（goal guides body scan）+ `scanBodyWikilinks` helper 確認。對 4 個 nav file body broken-wikilink test 核對。
- [x] [P] 1.7 Lint result schema and report format：對 `LintResult` interface（pagesScanned / navFilesScanned / issues / errorCount / warnCount）+ `printLintReport` 的 `formatCoverage` helper 確認字串格式 `N pages + M nav files scanned`。對 `tests/core/wiki/lint.test.ts` 的「counts all existing nav files」test 核對。

## 2. Add starting-spec banner to superpowers documents

讓未來實作者一看就知道 superpowers 是初期發想快照、不是 source of truth。Banner 內容兩份檔同樣文字，明確標示 phase 1 brainstorming snapshot + drift 預期 + 不要從這裡實作。

- [x] 2.1 在 `docs/superpowers/specs/2026-05-04-codebus-v2-phase1-design.md` 第一行加 `> ⚠️ **Starting spec — design-intent snapshot from phase 1 brainstorming.** Source of truth for shipped behavior is `openspec/specs/`. Drift from current code is expected; do not implement from this file.` banner，跟原文之間留空行
- [x] 2.2 在 `docs/superpowers/plans/2026-05-04-codebus-v2-phase1.md` 第一行加同樣 banner（一字不差，方便未來 grep 鎖死）
- [x] 2.3 寫 `docs/superpowers/README.md` governance doc：說明 superpowers 目錄的角色（phase 1 brainstorming snapshots、不維護不更新）+ source of truth 在 `openspec/specs/` + 新 capability 變更走 `/spectra-propose` 流程 + cross-link CLAUDE.md 對應段落

## 3. Acceptance

- [x] 3.1 跑 `npm test` 確認沒 regression（26 file / 155 tests pass — +3 vs. baseline 152，新增的 tests 對應 1.1/1.2/1.3 補進來的 retroactive coverage）
- [x] 3.2 跑 `spectra validate lint-coverage` 確認 spec / proposal / tasks 互相對得上（valid）
- [x] 3.3 跑 `spectra analyze lint-coverage --json` 看是否有 critical/warning 級 finding（0 findings — Coverage/Ambiguity/Gaps clean，Consistency skipped 因 design artifact 不存在）
- [x] 3.4 commit + 寫 commit message 標明這是 retroactive spec coverage（沒動 code）
