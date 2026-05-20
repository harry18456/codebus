## 1. Raw mirror exclusion 規則修正（spec: Raw Mirror with PII Scanner — vault capability）

- [x] 1.1 在 `codebus-core/src/vault/raw_sync.rs` 抽出 helper `fn is_excluded_path(rel: &Path) -> bool`：`.codebus` 與 `.env` 維持 root-only（first segment 比對）；`.git` 改為「任一 path segment 等於 `.git` 即排除」。行為：對相對路徑 `rel` 回傳 bool；root-only `.codebus`/`.env` 與深層 `.git` 各自獨立判斷。驗證：（涵蓋 Raw Mirror with PII Scanner 新增 scenarios）新增 5 個 unit test：`root_dot_git_excluded`、`nested_dot_git_excluded`（`vendor/foo/.git/HEAD`）、`root_dot_codebus_excluded`、`nested_dot_codebus_not_excluded`（`docs/.codebus/notes.md` 不擋）、`root_dot_env_excluded` 全綠。
- [x] 1.2 把 `sync_with_scanner_into` 的 `let first_seg ... ALWAYS_SKIP_AT_ROOT.contains(...)` 區塊換成呼叫 `is_excluded_path(&rel)`。行為：寫入路徑時對任何 `.git` segment 跳過、root `.codebus`/`.env` 仍跳過、深層 `.codebus`/`.env` 不再誤跳過。驗證：新增 integration-style 單元測試 `mirror_skips_nested_dot_git`：建立 `vendor/foo/.git/config` + `vendor/foo/src/main.rs`，跑 `sync_with_scanner` 後 `vendor/foo/.git/` 路徑下零檔、`vendor/foo/src/main.rs` 已鏡像；既有 `always_skip_root_dot_codebus_dot_git_dot_env` 維持綠。
- [x] 1.3 把 `walk_source_for_signal` 的同名 first-segment 判斷也換成 `is_excluded_path(&rel)`，與 #1.2 共用同一個 helper（避免兩處 filter 飄移、重蹈 v3-bug-fixes 的 init→goal drift 誤觸覆轍）。行為：source signal 統計與寫入 mirror 的 filter 完全一致。驗證：新增 unit test `walk_source_for_signal_skips_nested_dot_git`：相同 `vendor/foo/.git/config` + `vendor/foo/src/main.rs` 結構下，回傳的 `(file_count, total_bytes)` 不含 `.git/` 下任何檔；既有 `walk_source_for_signal` 相關測試維持綠。
- [x] 1.4 新增 fixture 測試 `mirror_includes_nested_dot_codebus_user_content`：建立 `docs/.codebus/notes.md`，sync 後該檔出現在 `.codebus/raw/code/docs/.codebus/notes.md`（保證 helper 沒把巢狀 `.codebus` 誤判為排除）。

## 2. 收尾驗證

- [x] 2.1 刪掉 `openspec/changes/raw-sync-nested-git-leak/_tmp_*.md` 暫存檔。驗證：`ls openspec/changes/raw-sync-nested-git-leak/_tmp_*.md` 回空。
- [x] 2.2 全 stack 驗證綠。行為：本 change 所有 spec scenarios 皆有自動測試覆蓋；其他 crate 無回歸。驗證：`cargo test -p codebus-core` 0 failed（含本 change 新增的測試與既有 `raw_sync.rs` mod tests 全部）、`cargo check --workspace` 乾淨、`docs/2026-05-19-raw-sync-nested-git-leak-backlog.md` 標註本 change 已交付（讓 backlog 文件指向 archive）。
