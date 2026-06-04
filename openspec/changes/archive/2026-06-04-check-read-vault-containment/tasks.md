# Implementation Tasks

## 1. vault root 來源（已驗證）

- [x] 1.1 依已驗證來源實作 vault root 取得：`codebus hook check-read` 從 PreToolUse stdin 的 `cwd` 欄位取 vault root（2026-06-04 實機探針證 stdin 帶 `cwd`＝vault root；stdin 缺 `cwd` 時備援 hook 子程序 `std::env::current_dir()`），不引入新 persistent config 欄位。行為：containment 比對基準＝stdin `cwd` 解析出的 vault root。驗證：unit test 餵含 `cwd` 的 PreToolUse JSON，斷言 in-vault path allow、out-of-vault path block 皆以該 `cwd` 為基準。（落實 design Decision「vault root 來源：PreToolUse stdin `cwd`（首選）、hook 子程序 cwd（備援）— 2026-06-04 實機驗證」）

## 2. Config key `read_path_containment`

- [x] 2.1 [P] 在 `codebus-core` 的 `HooksConfig` 新增 `read_path_containment: bool`（預設 `true`、fail-safe），與既有 `read_image_block` 平行解析。行為：config 缺 key / 非 bool / 缺 `hooks` 段時解析為 `true`。驗證：unit test `read_path_containment_defaults_true` 與 `read_path_containment_non_bool_resolves_true` 綠。（落實 design Decision「獨立 config key `read_path_containment`（預設 true），與 read_image_block 分離」）
- [x] 2.2 [P] `codebus init` 寫的 starter config `hooks` 段加入 `read_path_containment: true` 與說明註解。行為：fresh 全域 config 含該 key 與註解。驗證：starter-config 產生測試斷言 body 含 `read_path_containment: true`。

## 3. hook check-read containment（TDD）

- [x] 3.1 RED：在 `codebus-cli/src/commands/hook.rs` 寫核心 containment 失敗測試 — Read `file_path` 在 vault 外→block、vault 內相對路徑→allow、Glob/Grep `path` 在 vault 外→block、Glob/Grep 省略 `path`→allow（不 fail-closed）、fix 的 vault 內絕對路徑→allow。行為：定義 containment 的 in/out 對照可被測。驗證：新測試先紅（功能未實作）。（覆蓋 `Requirement: Vault Containment Read Gate`）
- [x] 3.2 RED：寫邊界與開關失敗測試 — Windows backslash / drive-letter 大小寫差異的 vault 內路徑→allow（`#[cfg(windows)]`）、`read_path_containment: false`→跳過 containment、缺 key→fail-safe 視為 `true`、`read_image_block: false` 只跳 denylist 而 containment 仍依 `read_path_containment` 生效。驗證：新測試先紅。
- [x] 3.3 GREEN：`ToolInput` 新增 `path: Option<serde_json::Value>`；`check_read` 依 `tool_name` 取 `file_path`(Read)/`path`(Glob/Grep)，在 denylist 前加 canonicalize-then-contain 前置 gate（vault root 與 target 同套 canonicalize 比 prefix），fail-closed 分流為「Read 缺 file_path→block、Glob/Grep 缺 path→allow」。行為：3.1/3.2 描述的對照全部成立、denylist 保留為 vault 內 defense-in-depth。驗證：`cargo test -p codebus-cli` 的 hook 測試（含 3.1/3.2）全綠。（實作 `Requirement: Vault Containment Read Gate` 與 `Requirement: PII Image Read Hook Installation` 的 path 欄位連動；落實 design Decision「vault-root containment allowlist 取代 denylist 作為主 gate」與 Decision「必須 canonicalize-then-contain，禁止 ban-absolute」）

## 4. settings REQUIRED_HOOKS + drift guard

- [x] 4.1 在 `codebus-core` 的 `REQUIRED_HOOKS` 新增 `Glob`、`Grep` 兩個 `RequiredHook`（→ `codebus hook check-read`），`DEFAULT_SETTINGS_JSON` 連動產出四 matcher。行為：fresh vault 的 `settings.json` `hooks.PreToolUse` 含 Bash/Read/Glob/Grep 四 matcher。驗證：更新並通過 `default_settings_json_matches_required_hooks_exactly` 與 `settings_json_contains_both_bash_and_read_matcher_entries`（擴為四 matcher 斷言）。（落實 design Decision「Glob/Grep 覆蓋走 REQUIRED_HOOKS 加 matcher（單一真相）」，連動 `Requirement: PII Image Read Hook Installation` 的 matcher 安裝）
- [x] 4.2 在 `codebus-cli/tests/lint_flow.rs` 補 vault-gate-integrity 場景：缺 `Glob`、缺 `Grep` 各 flag 一條 error、四缺產四條；確認 `vault_gate_integrity` rule 隨 `REQUIRED_HOOKS` 自動覆蓋（rule 本體無需改）。行為：缺任一 required hook 的 vault 跑 `codebus lint` 被精確 flag。驗證：`cargo test -p codebus-cli --test lint_flow` 綠，且 rule unit test `issues.len()` 對齊四 required hook。（覆蓋 `Requirement: Vault Gate Integrity Check`）

## 5. Migration + docs

- [x] 5.1 補本 repo 自身 `.codebus/.claude/settings.json` 為四 matcher（手動加 Glob/Grep 片段），並撰寫既有 vault 的 release-note 升級指引（手動 JSON 片段或於新位置 re-init）。行為：本 repo dogfood vault gate 完整。驗證：本 repo 跑 `codebus lint` 不再出現 `vault-gate-integrity` issue。
- [x] 5.2 [P] 更新 `docs/security.md` §5 與 `README` 的讀取邊界表述：check-read 現為 vault-root containment、Glob/Grep 已覆蓋、新增 `hooks.read_path_containment` 開關與 emergency escape hatch 說明。行為：對外文件與實作一致、不再 over/under-claim。驗證：grep `docs/security.md` 含 `read_path_containment` 與 `Glob`/`Grep` 覆蓋敘述、且不殘留「Glob/Grep 未 gate」舊述。

## 6. Live smoke（apply 末，本 repo `.codebus` vault）

- [x] 6.1 真實 codebus 端到端：跑一次 `codebus query`（claude provider）確認正常讀 `raw/code/`、`wiki/` 不被誤擋，且對 vault 外路徑的 Grep 被 containment block；跑一次 `codebus fix` 確認 agent 用 lint 絕對路徑 Read/Edit `wiki/` 檔通過、fix 正常完成。行為：正常 verb 零 regression、破口已封。驗證：兩次 run 的 stream-json / RunLog 觀察（query 無誤擋且 vault 外 Grep 出現 block decision、fix outcome 成功）。
