## 1. raw_sync API 擴展

- [x] 1.1 [P] Write failing tests for `sync_repo_to_raw_with_scanner`：傳 `NullScanner` 應與既有 `sync_repo_to_raw` byte-equal（落地 **Default scanner configuration preserves 0.2.0 behavior**）
- [x] 1.2 Implement `sync_repo_to_raw_with_scanner(repo, raw_dir, scanner, on_hit)` — 落地 **raw_sync API 形態：新增 with-scanner 入口、保留舊 alias**；既有 `sync_repo_to_raw` 改為 thin wrapper（`NullScanner` + `OnHit::Warn`）以保 backward compat

## 2. Scan invocation 點 + binary fall-through

- [x] 2.1 [P] Write failing tests：UTF-8 file 過 `PiiScanner::scan`、binary file fall through to `fs::copy`、empty file scan 不報錯（落地 **Invoke PiiScanner on each candidate text file before mirroring**）
- [x] 2.2 Implement read-scan-write loop：`fs::read_to_string` Ok → `scanner.scan(content, rel_path)` → 依 `OnHit` dispatch；Err → `fs::copy` 原 path — 落地 **Scanner invocation 點：copy-then-scan vs read-scan-write** 與 **Binary file 處理：fall through 走原 fs::copy**

## 3. OnHit modes

- [x] 3.1 [P] Write failing tests for `OnHit::Warn`：single match、multiple matches、stderr format pin (`warning: PII match in <rel_path>: <pattern_name> at offset <byte_start>`)、檔案 byte-equal mirror（落地 **OnHit::Warn writes a stderr line per match and still mirrors the file**）
- [x] 3.2 [P] Write failing tests for `OnHit::Skip`：dst file 不存在、sibling clean file 仍 mirror、stderr format pin (`skipped: <rel_path> (reason: pii hit <pattern_name>)`)（落地 **OnHit::Skip omits the file from the mirror and writes a stderr line**）
- [x] 3.3 [P] Write failing tests for `OnHit::Mask`：single match in-place、multiple non-overlapping all replaced、line count preserved、no stderr lines（落地 **OnHit::Mask replaces matched substrings with a labeled placeholder**）
- [x] 3.4 Implement `apply_on_hit` dispatcher + `apply_mask` helper — 落地 **Stderr warning 格式：固定 ASCII en-us**、**`OnHit::Mask` 多 match 重疊處理：last-match-wins**、**`OnHit::Skip` 真的整檔不寫 vs 寫空檔**

## 4. patterns_extra trigger

- [x] 4.1 Write failing test：`patterns_extra` 含 `INTERNAL-\d{6}` 對應檔案 trigger via `OnHit::Warn` → stderr 含 `custom-0`（落地 **User-supplied patterns_extra entries trigger matches alongside builtin patterns**）

## 5. main.rs / goal command wiring

- [x] 5.1 改 `codebus-cli/src/main.rs::run_goal_cmd`：從 `cfg.pii` build scanner via `pii::build_scanner`、`Err` 直接 `eprintln!` + `exit(1)` 在 invoke LLM agent 之前（落地 design Open Question「scanner 建構失敗的 fallback」決定為 fail-fast）
- [x] 5.2 改 `codebus-cli/src/commands/goal.rs::run_goal`：`RunGoalOptions` 多 `pii_scanner: &dyn PiiScanner` + `pii_on_hit: OnHit` 兩 field，傳給 `sync_repo_to_raw_with_scanner` — 落地 **--goal flow's raw_sync invokes the configured PII scanner** + spec scenarios「Goal command propagates PII config from global config to raw_sync」、「Default config preserves 0.2.0 behavior in goal flow」、「Scanner construction failure aborts the goal」

## 6. Conformance gates

- [x] 6.1 跑 `cargo test --workspace` 全綠（既有 191 + 新增 ~7-9 tests，無 regression）
- [x] 6.2 跑 `target/release/codebus.exe --repo D:/side_project/uv --check` 與 `tests/fixtures/uv-vault-snapshot/check-output.txt` byte-equal — 沒設 `~/.codebus/config.yaml` 時行為不變（落地 **Test 策略：inline，不開 fixture-vault**）
- [x] 6.3 跑 `cargo clippy --workspace -- -D warnings` clean、`cargo fmt --all -- --check` clean
- [x] 6.4 buddy-gacha smoke：寫 `~/.codebus/config.yaml` 設 `pii.scanner: regex_basic, pii.on_hit: warn`，跑一輪 `--goal`、確認 stderr 沒爆量誤判（buddy.js 內 `SALT = "friend-2026-401"` 不是 pattern 命中、不該 warn）

## 7. Final commit + archive

- [x] 7.1 Cool-down：用自己 `patterns_extra` 設 `INTERNAL-\d{6}` 之類 marker、放到測試 repo 中、跑 codebus 確認真的偵測到（驗證 patterns_extra 端到端）
- [ ] 7.2 Final commit：`feat(pii): wire RegexBasicScanner into raw_sync with three on_hit modes`（單一 commit；如 cool-down 期間發現 regression，依 design 的 **Rollback 策略**：`git revert <hash>` 回前一 commit）
- [ ] 7.3 `spectra archive pii-filter-wire-raw-sync`
