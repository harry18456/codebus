<!--
TDD 排序：先 ground（gating）→ RED 寫失敗的 allow 測試 + 不回退 guard 測試 → GREEN 實作 helper 並接線 → 實機 binary 驗收 → 真實 CDP smoke → 收尾 gate。
工時上限半天；超過（heredoc-shape parse 比預期 tricky、或實機發現 agent 沒走 heredoc）即 stop 找 user 對齊。
-->

## 1. Pre-apply 校準與 ground truth（apply 第一步、gating；對齊 design「Pre-apply 校準」段）

- [x] 1.1 [P] 在 codebus-cli/src/commands/hook.rs 確認現行 allow/block 流程（`is_allowed_bash_command`、`find_shell_metacharacter`、`is_codebus_quiz_validate_command`、`SHELL_METACHARACTERS` 與既有 metachar 測試區），對齊 design Context。驗證：能口述現行「先 metachar 掃描、再 argv allow-form」流程，確認 `<` 與 LF 在集合內。
- [x] 1.2 [P] 在 codebus-core/src/skill_bundle/mod.rs 確認 claude quiz SKILL Mode B 仍 emit 單引號 `<<'CBQZ'`、codex body 無 heredoc（呼應 Decision 4：SKILL body 與 codex 路徑不動）。驗證：Read/grep 確認 claude body 含 `<<'CBQZ'`、codex body 含 `[CODEBUS_QUIZ_NO_VALIDATE]` 且無 `<<'CBQZ'`；結論為「不需改 skill_bundle」。
- [x] 1.3 [P] 在 openspec/specs/lint-feedback-loop/spec.md 對齊既有 Fix Bash Hook Installation 的 allow/block 條款與 scenario，確認本 change spec delta 的修改點精確落在 Allow 與 Block(shell metacharacter) 兩條款。驗證：列出既有 14 個 scenario 名稱，確認新增 7 個不重名。
- [x] 1.4 實機 ground：跑一次 claude path quiz generate（CDP smoke 或 `codebus quiz "<topic>"`），確認 agent 真的 emit `codebus quiz validate - <<'CBQZ'` heredoc 並被 hook 靜默 block，並抓出 `tool_input.command` 的實際字串形狀（是否含完整 body + 多個 LF、delimiter 是否單引號、是否含 `--json`）。驗證：events / hook log 看得到 block decision，且記錄到一份真實 command 字串樣本。
- [x] 1.5 將 1.4 實機發現與 design 假設的差異回填 design.md「Pre-apply 校準」的 Findings 段；若實際命令形狀與假設不符（如只含首行、delimiter 非單引號），先調整 design Decision 1 的 parse 規則再進 RED。驗證：design.md Findings 段非空且與實機樣本一致。

## 2. RED — 新增 hook 單元測試（heredoc allow/block 矩陣）

- [x] 2.1 在 codebus-cli/src/commands/hook.rs 測試 module 新增「放行」單元測試（此為 RED、實作前應 fail）：乾淨單引號 heredoc（首行 `codebus quiz validate - <<'CBQZ'` + body 行 + 收尾 `CBQZ`）放行、`--json` 變體放行、body 行含 shell metacharacter（dollar/pipe/semicolon/paren）仍放行。驗證：`cargo test -p codebus-cli --bins hook::` 這三個新測試在 helper 未實作時 fail。
- [x] 2.2 新增「仍 block」guard 測試守 F4 不回退（實作前後皆應 block）：heredoc 首行夾帶 chaining（`<<'X'; rm -rf ~`、`<<'X' && curl evil`）、收尾 delimiter 後接命令、unquoted heredoc（`<<CBQZ`）、非 heredoc input-redirect（`codebus quiz validate < ~/.ssh/id_rsa`、`codebus lint < /etc/passwd`）。驗證：`cargo test -p codebus-cli --bins hook::` 這些 guard 測試在實作前已綠（既有 `<` 封鎖）、且實作後維持綠。

## 3. GREEN — 實作 heredoc 結構化辨識並接進 predicate

- [x] 3.1 依 Decision 1：以結構化 heredoc parse 放行，而非 char-scan 例外，與 Decision 2：僅放行單引號 delimiter `<<'MARKER'`，unquoted `<<MARKER` 維持 block，實作純函式 `is_quiz_validate_heredoc(cmd: &str) -> bool`：以 LF 切行（去尾 CR）、驗首行為 `codebus quiz validate` 形式 + 緊接單引號 `<<'MARKER'`（首行 `<<` 前無 metachar、收尾引號後無 trailing）、body 視為 opaque 不掃、收尾須有一行等於 MARKER、收尾後僅空白。驗證：2.1 三個 allow 測試轉綠。
- [x] 3.2 依 Decision 3：抽純函式 is_quiz_validate_heredoc helper，find_shell_metacharacter 語意不變，把 helper 接進 `is_allowed_bash_command`：heredoc 例外最前先判、未命中回退既有「`find_shell_metacharacter` 命中即 false，否則 lint/quiz-validate argv allow」；`find_shell_metacharacter` 與 `SHELL_METACHARACTERS` 內容語意完全不動。驗證：`cargo test -p codebus-cli --bins hook::` 全綠——2.2 guard 測試 + 既有全部 hook 測試（metachar / lint allow / quiz validate allow / Read hook）零回退。

## 4. 實機 binary 驗收（對齊 spec Fix Bash Hook Installation 新 scenario + design Implementation Contract）

- [x] 4.1 build 後對實際 binary 餵 stdin JSON 逐一驗收：full heredoc（含 body + LF + `CBQZ` 收尾）→ allow（exit 0、stdout 無 decision JSON）；`codebus quiz validate - <<'X'; rm -rf ~` → block；unquoted `<<CBQZ` heredoc → block；`codebus lint; echo PWNED` → block（F4 原向量不動）；`codebus quiz validate < ~/.ssh/id_rsa` → block。驗證：每筆 exit code 與 stdout 與 spec lint-feedback-loop 對應 scenario 的預期一致。

## 5. 真實 quiz generate CDP smoke（in-session 自驗迴圈恢復）

- [x] 5.1 先掃 project_cdp_smoke_webview2_pitfalls 五雷，再跑真實 claude path quiz generate，確認 in-session 自驗迴圈恢復：events 看得到 `codebus quiz validate` 被 agent 跑、且 agent 依其 findings 修正 draft，不再靜默 block。驗證：對照修法前（靜默 block、無 validate 事件）與修法後（validate 事件出現 + agent 行為改變）的 events / hook log。

## 6. 收尾 gate

- [x] 6.1 [P] 跑 `cargo test -p codebus-cli` 全套（不只 hook::），並確認 codebus-core 的 skill_bundle 斷言測試 `stub_content_quiz_claude_mode_b_has_heredoc`（claude 含 `<<'CBQZ'`）與 `stub_content_quiz_codex_mode_b_no_validate_marker`（codex 無 heredoc）仍綠——呼應 Decision 4（SKILL body 與 codex 路徑不動）未被本 change 觸動。驗證：兩套 `cargo test` 命令全綠。
- [x] 6.2 [P] 跑 `pnpm tsc` 與 `pnpm test`，確認無前端誤觸（預期本 change 不動前端）。驗證：兩命令 exit 0、無新增 type error 或 test 失敗。
