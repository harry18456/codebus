## 1. 副檔名 predicate 與 CheckRead 子命令（hook.rs）

- [x] 1.1 在 `codebus-cli/src/commands/hook.rs` 加 unit tests 鎖定 `is_image_path` 行為契約：blocklist 副檔名（png/jpg/jpeg/gif/webp/bmp/tiff/tif/pdf/ico/heic/heif/avif）大小寫混合命中 / `.svg` 與 `.md` 與 `.rs` 與無副檔名 allow / Windows 反斜線與 Unix 正斜線路徑都正確抽副檔名；測試**初始狀態 FAIL**。Verify: `cargo test --package codebus-cli is_image_path` 顯示新測試 fail（紅燈）。
- [x] 1.2 實作 `is_image_path` predicate 與 `IMAGE_BLOCKLIST` const 陣列，落實 design Decision「Blacklist 副檔名清單（不採 whitelist / magic bytes）」與「ASCII case-insensitive 跨平台一致（偏離 is_codebus_binary 的 OS-split）」；副檔名比較統一 `to_ascii_lowercase`，路徑分隔符同時處理 `/` 與 `\`，函數 doc comment 註明刻意偏離 `is_codebus_binary` OS-split 行為。Verify: 1.1 全綠。
- [x] 1.3 加 unit tests 鎖定 `check_read` fail-closed 契約：stdin 空 / 非 JSON / 缺 `tool_input.file_path` / `file_path` 為 number / `file_path` 為空字串都需 emit block decision JSON；落實 design Decision「Fail-closed 沿用 check-bash 模式」；測試**初始狀態 FAIL**。Verify: `cargo test --package codebus-cli check_read_fail_closed` 紅燈。
- [x] 1.4 加 `HookArgs::CheckRead` variant 與 `check_read()` 函數，鏡射 `check_bash` 結構（複用 `PreToolUseInput` struct、`emit_block`、`json_escape`），落實 design Decision「新增 `check-read` subcommand 鏡射 `check-bash`」；block reason 字串包含被擋的 file_path。Verify: 1.3 全綠且 1.1 仍綠；`cargo test --package codebus-cli hook` 全綠。

## 2. settings.json template（settings.rs）

- [x] 2.1 修改 `codebus-core/src/vault/settings.rs` 既有測試 `settings_json_parses_as_valid_json_with_pretooluse_bash_hook`（或改名拆兩條），加 assertion 驗證 `hooks.PreToolUse` 陣列同時含 Bash matcher（command 為 `codebus hook check-bash`）**與** Read matcher（command 為 `codebus hook check-read`），落實 spec Requirement「PII Image Read Hook Installation」第一條 scenario；測試**初始狀態 FAIL**。Verify: `cargo test --package codebus-core vault::settings` 紅燈。
- [x] 2.2 更新 `DEFAULT_SETTINGS_JSON` 常數加第二條 Read matcher entry，落實 design Decision「既有 vault 升級走 release note（不自動 migrate）」前半（fresh vault 直接寫兩條）；保持 `write_settings_if_missing` 寫入語意不變。Verify: 2.1 全綠。
- [x] 2.3 驗證 `does_not_overwrite_existing_settings_json` 測試仍綠（既有 vault 不被覆寫，落實 design Decision 後半），且 `writes_settings_json_on_fresh_vault` 仍綠。Verify: `cargo test --package codebus-core vault::settings` 全綠。

## 3. CLI end-to-end 整合測試

- [x] 3.1 在 `codebus-cli/tests/` 加 integration test，模擬 PreToolUse 流程：手構符合 Claude Code PreToolUse schema 的 JSON（含 image 與 non-image 兩組 case）作為 stdin 餵給 `codebus hook check-read`，assert image 組回傳含 `decision: block` 的 stdout JSON、non-image 組 stdout 空，覆蓋 spec Requirement「PII Image Read Hook Installation」的端到端契約。Verify: `cargo test --package codebus-cli check_read_e2e` 全綠。

## 4. 既有 vault 升級指引文件

- [x] 4.1 在 `docs/` 新增 `2026-05-20-pretooluse-image-block-migration.md` 或附加段落到既有 release-notes-style 文件，列出 manual JSON snippet（與 `DEFAULT_SETTINGS_JSON` 的 Read matcher entry 完全一致）讓既有 vault user 手動加入 `<vault>/.codebus/.claude/settings.json`，落實 design Decision「既有 vault 升級走 release note」；文件需明確告知「不會自動 migrate、re-init 是替代選項」。Verify: 文件存在、JSON snippet 字面相符 settings.rs DEFAULT_SETTINGS_JSON 的 Read entry；manual review。

## 5. App + CLI 手動驗收（Windows）

- [x] 5.1 在 Windows 上跑端到端 smoke：(a) 在一個含 `screenshot.png` 的 repo 跑 `codebus init` 後檢視 `.codebus/.claude/settings.json` 確認兩條 entry 都在；(b) 跑 `codebus goal "summarise the screenshot"` 觀察 agent Read 該圖檔被 block reason 阻擋、agent 流程繼續；(c) 跑 `cargo tauri dev` 進 codebus-app GUI 同樣對該 repo 跑 goal flow，觀察 mini-stream UX 上有 block reason 出現（CLI 與 App 都走同 hook、行為應一致）。Verify: 三項皆觀察到 block decision；macOS / Linux 驗收延後至 `v3-app-polish-ship` deferred acceptance registry（per memory feedback_dont_default_polish_ship）。
