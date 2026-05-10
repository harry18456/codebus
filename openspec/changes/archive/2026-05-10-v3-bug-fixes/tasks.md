## 1. Bug 1 — Init source-signal mismatch fix

- [x] 1.1 在 `codebus-cli/src/commands/init.rs::run` 把 `source_gitignore::ensure_codebus_in_gitignore(repo)` 呼叫從現在的位置（manifest write 之前、raw_sync 之後）移到 **raw_sync 之前**（在 `create_vault_layout` 後、`load_pii_config_with_warning` / `sync_with_scanner` 之前），讓 raw_sync summary.bytes 反映 post-mutation source state；其餘 step 順序不動 — 由 `cargo build -p codebus-cli` 通過 + 手動 cli 測試「init→goal 連續跑、goal 不再印 `~ 同步 source` banner」驗證
- [x] 1.2 在 `codebus-cli/tests/cli_routing.rs` 新增 integration test `init_followed_by_repeat_init_does_not_drift`：跑兩次 init 對同一 repo（第二次走 `Updated` 路徑），第二次 `compute_source_signal` 與第一次 manifest 寫入的 signal `total_bytes` 必須相等 — 保證 .gitignore 不會在 init 之間累積 byte 差異；遵守 Init Subcommand Behavior MODIFIED 段「source `.gitignore` mutation precedes raw mirror」契約

## 2. Bug 2 — locate_vault_root 接受 vault root path

- [x] 2.1 在 `codebus-core/src/wiki/lint/locate.rs::locate_vault_root` 對 `repo_override` 加 vault-root 偵測：當 `repo.join("wiki").is_dir()` 為 true 直接回 `repo.to_path_buf()`；false 才 fall back 到既有 `repo.join(".codebus")` — 遵守 Vault Root Auto-Detection MODIFIED 段「vault root path 直接用、source repo path 才 join」契約
- [x] 2.2 在 `codebus-core/src/wiki/lint/locate.rs` 新增 4 條 unit test：(a) `explicit_repo_override_with_wiki_subdir_uses_path_directly` — `repo_override` 已含 `wiki/` 直接回；(b) `explicit_repo_override_without_wiki_subdir_joins_dot_codebus` — 不含則 join；(c) 既有 `explicit_repo_override_does_not_check_existence` 仍綠（不存在路徑 fall through 走 join）；(d) 既有 `explicit_repo_override_wins_over_cwd` 仍綠 — 由 `cargo test -p codebus-core --lib wiki::lint::locate::tests` 全綠驗證
- [x] 2.3 在 `codebus-cli/tests/lint_flow.rs` 新增 integration test `lint_repo_pointing_at_vault_root_works_same_as_source_repo`：先 init 一個 vault 製造 1 條 lint warn → 跑 `lint --repo <source>` 與 `lint --repo <source>/.codebus` → 兩者 stdout 必須相同（一致報出該 warn 的 vault-relative path）；遵守 Lint Output Formats 既有「vault-relative path」契約

## 3. 整合驗證

- [x] 3.1 跑 `cargo test --workspace` 全綠 — 確認既有 v3-config / v3-render-polish / vault_init / goal_flow / query_flow / fix_flow / lint_flow / cli_routing test suite 在 init 順序變更後仍通過
- [x] 3.2 跑 `spectra validate v3-bug-fixes` 與 `spectra analyze v3-bug-fixes --json` 無 Critical / Warning finding
- [x] 3.3 release build + 手動 CLI 驗證 — 用 `D:/side_project/uv` 跑：(a) 清掉舊 `.codebus/` 後 `init` 緊接 `goal "..."`，goal stdout 不含 SyncStart / SyncDone banner（Bug 1 fix 驗證）；(b) `lint --repo D:/side_project/uv/.codebus` 與 `lint --repo D:/side_project/uv` 兩種寫法輸出一致（Bug 2 fix 驗證）— 寫進 `docs/v3-uv-verification-2026-05-10.md` follow-up 段或附錄
