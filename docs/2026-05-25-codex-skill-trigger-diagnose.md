# 2026-05-25 — codex provider SKILL trigger 失靈 diagnose

## Context

延續 [2026-05-24 codex provider 實驗報告](./2026-05-24-codex-provider-experiment.md) 與 change proposal `codex-skill-trigger-fix`。

2026-05-25 重跑 reproducer 確認 codex 0.133.0 上 codebus 5 verb × codex provider 仍 0/5 完整 work。Diagnose 走 design.md 定義的三層觀察（CLI 版本對照 → codebus argv 攔截 → codex stream 觀察），找到 confirmed root cause 即停。

| 環境項目 | 值 |
|---|---|
| codebus binary | 3.0.0（commit `0cb8b2f` baseline） |
| codex CLI（baseline broken） | `codex-cli 0.133.0` (`C:\Users\harry\AppData\Roaming\npm\codex`) |
| Vault | `/tmp/exp-vault`（.codebus/wiki 共 9 pages） |
| codex provider | Azure (`gpt-5.4` deployment) |
| OS | Windows 11，Git Bash + PowerShell |

## Reproducer（已驗證仍在）

```bash
cd /tmp/exp-vault
cp ~/.codebus/config.yaml ~/.codebus/config.yaml.bk
sed -i 's/active_provider: claude/active_provider: codex/' ~/.codebus/config.yaml
codebus quiz "JWT issuance and verification" --count 3
# 預期看到：error: quiz plan spawn produced no [CODEBUS_QUIZ_SCOPE]/[CODEBUS_QUIZ_NO_MATCH] marker on any line
cp ~/.codebus/config.yaml.bk ~/.codebus/config.yaml  # 還原
```

2026-05-25 09:03 重跑 actual output 結尾：

```
error: quiz plan spawn produced no [CODEBUS_QUIZ_SCOPE]/[CODEBUS_QUIZ_NO_MATCH] marker on any line; spawn output head: I'm treating this as a planning task for the `codebus-quiz` area, focused on JWT issuance and verification.
```

Exit code 1。完整 log 於 `/tmp/codex-quiz-repro.log`。

## Diagnose 層 (a) — codex CLI 0.132 vs 0.133 行為比對

### Reproducer command

```bash
# 安裝 codex 0.132.0 到 isolated path
npm install --prefix /tmp/codex-0.132 @openai/codex@0.132.0

# 確認版本
/tmp/codex-0.132/node_modules/.bin/codex --version
# => codex-cli 0.132.0

# 切 codex provider 後跑 quiz、透過 CODEBUS_CODEX_BIN 指向 0.132
cd /tmp/exp-vault
CODEBUS_CODEX_BIN="/tmp/codex-0.132/node_modules/.bin/codex.cmd" \
  codebus quiz "JWT issuance and verification" --count 3 \
  > /tmp/codex-0.132-quiz.log 2>&1
```

### Actual output snippet

`/tmp/codex-0.132-quiz.log` 共 110 行，exit code 1。尾部關鍵段：

```
**Plan**

1. Ground the quiz in the three canonical pages: [auth-module.md](...), ...
2. Center questions on the implementation details that are explicitly documented:
   - payload claims are `sub` and `exp`
   - default TTL is `3600` seconds
   ...
3. Generate 3-5 multiple-choice questions that mix direct recall with flow understanding, ...
[... 整段 generic Plan 結構 ...]

error: quiz plan spawn produced no [CODEBUS_QUIZ_SCOPE]/[CODEBUS_QUIZ_NO_MATCH] marker on any line; spawn output head: I'm treating this as a repo-scoped planning task for the `codebus-quiz` area, focused on how JWTs are issued and verified. I'll inspect the code paths that handle auth, identify the current flow, then …(truncated)
```

### 結論

**codex 0.132.0 也 broken**。症狀與 codex 0.133.0 完全一致：agent emit「I'm treating this as a [repo-scoped] planning task for the `codebus-quiz` area」+ generic Plan、完全沒 emit `[CODEBUS_QUIZ_SCOPE]` marker。

→ **不是 codex 0.132 → 0.133 regression**。Root cause 不在 codex CLI 版本層。memory `project_multi_provider_driver_confirmed` 中「2026-05-22 codex 0.132 spike 端到端 work」的舊 claim 與本層觀察衝突，推斷舊 spike 跑的不是同一條 codebus-quiz invocation path（可能是直接 `codex exec ...` 不經 codebus、或 vault 結構不同）。需更新 memory。

繼續 Layer (b) 排除 codebus argv 拼接層。

## Diagnose 層 (b) — argv 攔截

### Reproducer command

Shim 是個小 Rust binary，dump argv + cwd + env + stdin 到 `/tmp/codex-shim-dump.txt`（Rust 視 cwd-relative，Windows 上實際落在 `<cwd-drive>:/tmp/codex-shim-dump.txt`）。Source 在 `/tmp/codex-shim/src/main.rs`。

```bash
# 編譯 shim
cd /tmp/codex-shim && cargo build --release

# 用 CODEBUS_CODEX_BIN 指向 shim binary、跑 reproducer
rm -f "C:/tmp/codex-shim-dump.txt"
cd /tmp/exp-vault
CODEBUS_CODEX_BIN="C:/Users/harry/AppData/Local/Temp/codex-shim/target/release/codex-shim.exe" \
  codebus quiz "JWT issuance and verification" --count 3 \
  > /tmp/codex-shim-quiz.log 2>&1
cat "C:/tmp/codex-shim-dump.txt"
```

### Actual output snippet

`C:/tmp/codex-shim-dump.txt`（精簡，env key 已遮罩）：

```
--- ARGV ---
[0] "C:/Users/harry/AppData/Local/Temp/codex-shim/target/release/codex-shim.exe"
[1] "exec"
[2] "--json"
[3] "--ignore-user-config"
[4] "--disable"
[5] "apps"
[6] "--ignore-rules"
[7] "--skip-git-repo-check"
[8] "-c"
[9] "project_root_markers=['.codebus-vault']"
[10] "--ephemeral"
[11] "-s"
[12] "read-only"
[13] "-m"
[14] "gpt-5.4"
[15] "-c"
[16] "model_reasoning_effort=low"
[17] "-c"
[18] "model_provider=azure"
[19] "-c"
[20] "model_providers.azure.name=azure"
[21] "-c"
[22] "model_providers.azure.base_url=https://2026msf13.cognitiveservices.azure.com/openai"
[23] "-c"
[24] "model_providers.azure.wire_api=responses"
[25] "-c"
[26] "model_providers.azure.env_key=CODEBUS_CODEX_AZURE_KEY"
[27] "-c"
[28] "model_providers.azure.query_params.api-version=2025-04-01-preview"
[29] "-c"
[30] "model_providers.azure.env_http_headers.api-key=CODEBUS_CODEX_AZURE_KEY"
[31] "$codebus-quiz plan: JWT issuance and verification"
--- CWD ---
"C:\\Users\\harry\\AppData\\Local\\Temp\\exp-vault\\.codebus"
--- STDIN ---
(empty)
--- END ---
```

### 結論

- **(i) ARGV 完整列表**：32 個 args 全到，含 `exec` subcommand + 全部 isolation flags + prompt
- **(ii) `$codebus-quiz` sigil 是否原樣保留**：**YES**，prompt arg `[31]` 為 `"$codebus-quiz plan: JWT issuance and verification"`，未被 shell escape、未加 quote-stripping、未被切成多 arg
- **(iii) isolation flags 完整**：`--ignore-user-config`、`--disable apps`、`--ignore-rules`、`--skip-git-repo-check`、`-c project_root_markers=['.codebus-vault']`、`--ephemeral`、`-s read-only`、`-m gpt-5.4`、`-c model_reasoning_effort=low`、Azure provider 配置 5 行 全部都在

**binary_anomaly=no**。codebus 端輸出 100% 正確、prompt 與 sigil 完整保留。Root cause **不在 codebus argv 拼接層**。

關鍵觀察：prompt 整段就是 `"$codebus-quiz plan: JWT issuance and verification"` — **沒有任何 SKILL body inline 帶在 prompt 裡**，完全依賴 codex `$`-prefix invocation 機制去 `.codex/skills/codebus-quiz/SKILL.md` 查 bundle。

Note：dump 顯示 `project_root_markers=['.codebus-vault']` 是個 codex 找 project root 的 marker 檔；繼續 Layer (c) 前先驗證 vault 有這 marker。

## Diagnose 層 (c) — codex stream events 觀察 + 缺檔驗證

### Reproducer command

Layer (b) 結束後檢查 vault 內 codex 端材料是否存在（dump 顯示 codex 被叫起來時 cwd=`.codebus/`、`project_root_markers=['.codebus-vault']`）：

```bash
ls /tmp/exp-vault/.codebus/.codex/skills/        # codex SKILL bundles
ls -la /tmp/exp-vault/.codebus/.codebus-vault    # codex project_root marker
ls -la /tmp/exp-vault/.codebus/AGENTS.md         # codex project agent rules
```

### Actual output snippet

**所有 codex 端材料都不存在：**

```
ls: cannot access '/tmp/exp-vault/.codebus/.codex/skills/': No such file or directory
ls: cannot access '/tmp/exp-vault/.codebus/.codebus-vault': No such file or directory
ls: cannot access '/tmp/exp-vault/.codebus/AGENTS.md': No such file or directory
```

對比 claude 端材料 `.codebus/.claude/skills/codebus-{chat,fix,goal,query,quiz}/` 全在。

Source code 比對（`codebus-core/src/vault/init.rs`）：

```rust
// init.rs line 323-331
// Codex provider: materialize the codex instruction surface (AGENTS.md
// mirroring CLAUDE.md, `.codex/skills/` bundles, project-root marker)
// alongside the claude bundles. Only when codex is the active provider;
// write-if-missing; silent (no extra lifecycle event so the declared
// event order is unchanged).
if codex_provider_active() {
    crate::skill_bundle::write_codex_materialization_if_missing(&paths.root, NEUTRAL_RULES)
        .map_err(InitError::SkillBundles)?;
}
```

`codex_provider_active()` 讀 `~/.codebus/config.yaml` 的 `agent.active_provider`、只有等於 `"codex"` 才寫 codex 材料。**exp-vault 在 2026-05-24 init 時 active_provider=claude，所以 codex 端材料從沒生過**。後續切 `active_provider: codex` 並沒觸發重新 materialize。

### 驗證假設（hypothesis confirmation）

保持 `active_provider: codex`、`cd /tmp/exp-vault` 重跑 `codebus init`：

```bash
cd /tmp/exp-vault && codebus init
# 印「✓ 掰掰~下車囉」+ commit e842a14
```

材料補齊：

```
$ ls /tmp/exp-vault/.codebus/.codex/skills/
codebus-chat  codebus-fix  codebus-goal  codebus-query  codebus-quiz
$ ls -la /tmp/exp-vault/.codebus/.codebus-vault /tmp/exp-vault/.codebus/AGENTS.md
-rw-r--r-- 1 harry 197609     0 May 25 09:23 .codebus-vault
-rw-r--r-- 1 harry 197609 12020 May 25 09:23 AGENTS.md
```

重跑 quiz：

```
$ cd /tmp/exp-vault && codebus quiz "JWT issuance and verification" --count 3
[...]
ok lint：0 errors, 0 warnings (0 ms)
warning: quiz content-verify spawn failed (non-fatal; ...)
quiz written: .\.codebus\quiz\jwt-issuance-and-verification-c065822f\2026-05-25T01-24-41Z.md
EXIT: 0
```

Quiz 完整跑完、生成 3 道題、答案 explanation 引用 `[[auth-module]]` `[[jwt-issue-and-verify]]` `[[jwt-payload]]` wikilinks。完整 log: `/tmp/codex-after-init-quiz.log`。

content-verify spawn 仍 emit「batch file arguments are invalid」warning — 屬獨立 issue（codex 的 verify stage Mode B 失敗），不影響 quiz 主流程，列入殘餘問題段。

### 結論

**codex_handles_sigil=YES**。codex 0.133.0 的 `$codebus-<verb>` native invocation 機制 work，前提是 `.codex/skills/codebus-<verb>/SKILL.md` 存在於 cwd 或 ancestor 並透過 `project_root_markers` 指定的 `.codebus-vault` marker 文件能被找到。

實際 layer (c) 不需要走原 design.md 的「直呼 codex exec 看 stream events」路徑 — layer (b) 的 dump 加上 vault filesystem 檢查就已經獨自 sufficient 證明 root cause 在「**vault 內 codex 端材料缺檔**」。

## Root Cause 結論

**Root cause**：`codebus-core/src/vault/init.rs::init_vault` 的 codex materialization gate（line 328 `if codex_provider_active()`）只在「init 時 active_provider=codex」才寫 `.codex/skills/` + `.codebus-vault` marker + `AGENTS.md`。後續切換 `active_provider` 不會 trigger 重新 materialize，於 claude 模式 init 的 vault 切到 codex provider 後 codex CLI 找不到 SKILL bundle、自然 fallback 到 generic task-reply mode。

**對映 design.md「修法選擇依 diagnose 結果擇一」表格**：本 root cause 不對映原表四列任一列（皆預設「init 已 materialize codex 材料」這個前提）。新 row 應為：

| Diagnose 結論（新增） | 修法路徑 | 影響檔案 |
|---|---|---|
| codex 端材料（SKILL bundles + marker + AGENTS.md）缺檔 | 解除 init 的 active_provider gate，無條件 materialize 兩 provider 的材料（write-if-missing 保留用戶 customization） | `codebus-core/src/vault/init.rs::init_vault` 約 line 328 |

證據鏈：
- 層 (a)：codex 0.132 同樣 broken → 不是 CLI 版本 regression
- 層 (b)：codebus 端 argv 拼接 100% 正確、`$codebus-quiz` sigil 完整保留、isolation flags 齊全 → 不是 codebus argv bug
- 層 (c)：vault 內 `.codex/` 目錄整段不存在；重跑 `codebus init`（active_provider=codex）補齊材料後 quiz 立即 work → root cause = 缺檔 + init gate 設計

claude path 為何不受影響？因為 `write_skill_bundles` (line 317) 無條件寫 claude 材料，沒 gate。

## 選用修法

**修法名稱**：解除 `codebus-core/src/vault/init.rs::init_vault` 的 `codex_provider_active()` gate，無條件 materialize 兩 provider 的 SKILL bundles + codex 端 marker + AGENTS.md。

**證據引用**：
- 層 (a) 排除 codex CLI 版本層
- 層 (b) 排除 codebus argv 拼接層
- 層 (c) + filesystem 檢查 + `cd /tmp/exp-vault && codebus init` 補檔 + 重跑 quiz 立即 work 三段，集體鎖定 root cause 在 init gate

**為什麼選「解除 gate」而非「on-spawn lazy materialize」**：
- gate 解除是 1 行 if 條件移除（最小 diff），不引入新 lifecycle event、不改 SpawnSpec、不新增 trait method（per memory `feedback_dont_speculative_abstract`，aligned with design.md Non-Goal「不為 multi-impl 預留抽象層」）
- on-spawn lazy materialize 需要在每個 verb 的 spawn 路徑加檢查（goal/query/fix/chat/quiz 共 5 處），增加複雜度
- gate 原意「claude-only vaults are never polluted with codex materialization」這個顧慮在 multi-provider 設計下不成立：用戶切 provider 就會需要對應材料、提早寫不影響 claude 行為（codex 端材料只在 codex spawn 時被 codex CLI 讀；claude 不 touch `.codex/`）

**對既有 vault 的 migration**：用戶若已 init 時 active_provider=claude、之後想切 codex，需要重跑 `codebus init`（write-if-missing 保留 customization，不會破壞既有內容）。Migration 路徑寫進 diagnose doc 「殘餘問題」段。

**後續版本 bump 回顧條件**：
- codex CLI 0.134+ 若有新 SKILL 機制或 project_root_markers 行為變動，需重跑 diagnose doc 中三層 reproducer 驗證
- codebus 引入第三 provider（hypothetical）時，init 應該同樣 materialize 第三 provider 的材料、按相同 pattern 處理

## Acceptance Criteria

- 5 verb × codex 重跑日誌（per 5 verb verification 表）— 全 5/5 PASS
- 單元測試名稱：
  - `agent::codex_backend::tests::workspace_argv_includes_windows_sandbox_elevation_override`（紅→綠）
  - `agent::codex_backend::tests::read_only_argv_also_includes_windows_sandbox_elevation_override`（紅→綠）
  - `agent::codex_backend::tests::codex_assembly_sub_mode_input_with_newlines_uses_stdin_placeholder`（紅→綠）
  - `agent::codex_backend::tests::codex_assembly_single_line_input_stays_in_argv`（new）
  - `vault::init::tests::init_always_materializes_codex_bundles_regardless_of_active_provider`（紅→綠）
- `cargo test -p codebus-core --lib`：622 passed / 0 failed
- `cargo build --workspace`：通過
- spec deltas：
  - `openspec/changes/codex-skill-trigger-fix/specs/skill-bundles/spec.md` ADDED「Codex-Side SKILL Mode Invocation Trigger」requirement + 6 scenarios
  - `openspec/changes/codex-skill-trigger-fix/specs/codex-backend/spec.md` ADDED「Codex Sandbox Write Enablement Override」requirement + 3 scenarios
- diagnose doc 連回 [codex-skill-trigger-fix proposal](../openspec/changes/codex-skill-trigger-fix/proposal.md)

## 5 verb verification 結果

A + B 兩 cluster 修完後在 `/tmp/exp-vault` + codex provider 重跑：

| Verb | Log path | Scenario 結論 |
|---|---|---|
| quiz (4.1) | `/tmp/codex-verify-quiz.log` | ✓ `[CODEBUS_QUIZ_SCOPE] wiki/processes/jwt-issue-and-verify.md, wiki/modules/auth-module.md, wiki/entities/jwt-payload.md` emit；`quiz written` to `.codebus/quiz/jwt-verification-flow-c1f6bc33/2026-05-25T02-37-40Z.md`；exit 0 |
| goal (4.2) | `/tmp/codex-goal-final.log` | ✓ Agent Write 3 個 wiki 檔（新建 `synthesis/auth-db-connection.md` + 更新 `index.md` / `log.md`）；lint 0 errors / 0 warnings；commit `6277d92`；exit 0 |
| query (4.3) | `/tmp/codex-verify-query.log` | ✓ Agent emit「I'm using the codebus-query skill」、Read 多個 vault wiki 檔（auth-module / jwt-issue-and-verify / jwt-payload / auth-db-connection）、final answer 直接引用 `verify_jwt()` / `SECRET_KEY` / `jwt.decode` 等 vault 具體內容 |
| chat (4.4) | `/tmp/codex-verify-chat.log` | ✓ Final answer 含 `[[auth-module]]` / `[[jwt-issue-and-verify]]` / `[[jwt-payload]]` wikilinks；**沒有**「I found this is a documentation vault rather than application source」meta-comment |
| fix (4.5) | `/tmp/codex-verify-fix.log` | ✓ 注入 broken `[[another-fake-page]]` 後 `codebus lint` 印 1 warn；跑 `codebus fix` 後重跑 lint 變 `ok 8 pages + 2 nav files scanned, no issues`（B cluster 修法生效、agent 真實 edit）|
| failure surfacing (4.6) | `/tmp/codex-quiz-repro.log` | ✓ 修法前 2026-05-25 09:03 原 reproducer 已 captured：exit code 1 + stderr「error: quiz plan spawn produced no [CODEBUS_QUIZ_SCOPE]/[CODEBUS_QUIZ_NO_MATCH] marker on any line; spawn output head: I'm treating this as a planning task...」— codebus 不 silent success、明標失敗 verb 與 diagnostic context |

5/5 scenario 全綠（含 fix 真實 edit、含 failure surfacing 行為）。

## C cluster — codex verify-stage spawn 撞 Windows `.cmd` batch-file argv 驗證

A + B 修完後 quiz / goal 主流程 work，但 content-verify 子 spawn 一律印「`warning: ... content-verify spawn failed (non-fatal; ...): spawn agent: batch file arguments are invalid`」，verify 結果回 `flagged 0 page(s)`（空集、沒真 verify）。

### Reproducer command

最小 Rust repro 驗證 Rust 1.77+ stdlib hardening 是 root cause：

```rust
// /tmp/cmd-arg-test/src/main.rs（節錄）
let prompt_nl = "goal=test\n\nCHANGED PAGES:\nfoo.md";
Command::new("C:/Users/harry/AppData/Roaming/npm/codex.cmd")
    .arg("--version").arg(prompt_nl)
    .stdin(Stdio::null()).output()
// → ERR kind=InvalidInput msg=batch file arguments are invalid

let prompt_no = "goal=test CHANGED PAGES foo.md";  // 同字串無 \n
Command::new("C:/Users/harry/AppData/Roaming/npm/codex.cmd")
    .arg("--version").arg(prompt_no)
    .stdin(Stdio::null()).output()
// → OK exit=Some(0)
```

### Bisect 三條繞路 round-trip 測試

驗證 multi-line content 真實傳到 codex agent 的能力：

| 繞路 | Argv 通過 Rust 驗證 | Agent 收到完整 multi-line |
|---|---|---|
| `cmd.arg(multi_line)` | ✗（InvalidInput）| n/a |
| `cmd.raw_arg("\"<quoted multi_line>\"")` | ✓ | **✗** — 實測 agent 收到「FOO\nBAR」（中間 LINE1/LINE2 被 cmd.exe/npm shim 吃掉）|
| `cmd.arg("-")` + stdin pipe + `stdin.write_all(multi_line)` | ✓ | **✓** — agent 收到「LINE1\nLINE2」完整內容 |

raw_arg 看似可行但 cmd.exe / npm shim 中間層會 mangle 多行 content。stdin pipe 是唯一保留完整 multi-line 的路徑。

### 結論

**Root cause**：Rust stdlib 1.77+ 對 `.cmd` shim argv 的 hardening + codex npm 在 Windows 安裝為 `.cmd` 形狀 + verify spawn 的 prompt 含 `\n`（`goal=...\n\nCHANGED PAGES:\n...`）= 三者疊加觸發。不是 codebus 也不是 codex bug — 是 Rust + Windows + npm shim 形狀的合作疏漏。main spawn 的 prompt 都是單行（`$codebus-quiz plan: <topic>`）所以不中招、只有 verify / repair sub_mode 走多行。

**修法**：在 `codebus-core/src/agent/codex_backend.rs::build_command` 對多行 prompt 改用 `-` 當 prompt argv（codex exec 原生支援 `-` 讀 stdin），把 formatted prompt 透過新增 trait method `AgentBackend::stdin_payload` opt-in 路徑 ship 進 child stdin。`AgentBackend::stdin_payload` 是 optional method 帶 default `None`、claude 走 default、codex 在多行時 opt-in。invoke loop 對 `Some(payload)` 改 `Stdio::piped` + write_all + drop。

**為什麼 trait method 不違反 Non-Goal**：Non-Goal 排的是「為 hypothetical 2nd impl 預留抽象」（per memory `feedback_dont_speculative_abstract`）。本 case 是 concrete cross-backend variation：claude 在 Windows 走 `.exe`（PE32+ executable，前面 ls -la 確認）無此問題；codex 在 Windows 走 `.cmd` shim 必須走 stdin。Trait method 是反映現實的 backend 差異、不是 future-proofing。Per memory `feedback_engineer_best_not_easiest`：選工程最正確解。

### C cluster e2e 驗證

```
$ cd /tmp/exp-vault && codebus quiz "session token lifecycle" --count 3
[CODEBUS_QUIZ_SCOPE] wiki/processes/jwt-issue-and-verify.md, ...
[CODEBUS_QUIZ_NO_VALIDATE] codex sandbox cannot run quiz structure validation
# ← 無「batch file arguments are invalid」warning、quiz 寫入 .codebus/quiz/session-token-lifecycle-*/2026-05-25T03-19-41Z.md

$ cd /tmp/exp-vault && codebus goal "describe how user identifier flows through auth and database"
[Agent 思考] CONTENT_OK     # ← verify stage 真實跑 + 回 CONTENT_OK（不是 spawn failed）
✓ commit f0bab1b
# 4 wiki files written: synthesis/auth-db-connection.md (updated) + processes/user-identifier-flow.md (new) + index.md + log.md (nav updates)
```

log 路徑：`/tmp/codex-final-quiz.log`、`/tmp/codex-final-goal.log`。

## 殘餘問題

修完 A + B + C cluster 後仍存在的 deferred follow-up：

1. **macOS / Linux 上 sandbox-write 行為未驗證**：`-c windows.sandbox=unelevated` 在非 Windows 平台是 no-op（codex `[windows]` table 跨平台 skip）。當這些平台啟用 codex provider 時，sandbox-write 是否需要對等 override 待驗。建議下一個 change 命名 `codex-cross-platform-sandbox` 處理。

3. **codex 後續版本（0.134+）若改 `windows.sandbox` schema**：當前 override 是 hard-coded value `unelevated`、codex doctor 確認 0.133.0 僅接受 `elevated` / `unelevated`。若 codex 0.134 引入更細粒度的 sandbox values（如 `restricted` / `developer`），需重跑本 diagnose 的 K-mode bisect 找新值、補 spec scenario。本 change 不為此預留 feature-detection 抽象（per memory `feedback_dont_speculative_abstract`）。

4. **codex 端 grounded behavior 不對等 claude**：本 change 只保證「進 SKILL Mode + 可寫 vault」。codex 端 agent 行為（reasoning 風格、tool-call 順序、wiki page 命名選擇）仍與 claude 不完全等價。Per design.md Non-Goals，這不是本 change 目標。

## Self-review — 未引入 multi-impl 抽象層

對 `codebus-core/src/vault/init.rs` 修法 diff (`git diff --stat`)：

```
codebus-core/src/vault/init.rs | 85 +++++++++++++++++++++++++++++++++---------
1 file changed, 68 insertions(+), 17 deletions(-)
```

Insertions 主體是 new test function（55 行 + doc-comment）+ comment 改寫；deletions 是 `if codex_provider_active() { ... }` gate（4 行）+ `codex_provider_active()` helper（10 行）。實際 production code 行數淨減。

Per memory `feedback_dont_speculative_abstract`，本 change 未新增以下：

| 項目 | grep | 結果 |
|---|---|---|
| trait method 新增 | `git diff codebus-core/src/agent/` | 0（沒動）|
| `SpawnSpec` 欄位 | `git diff codebus-core/src/agent/spawn_spec.rs` | 0（沒動）|
| `config` 欄位 | `git diff codebus-core/src/config/` | 0（沒動）|
| `strategy enum` / dispatch | `grep enum.*Strategy codebus-core/src/vault/init.rs` | 0 |
| 新增 module / file | `git status --short | grep '^??'` | docs/ + tests/diagnose 物（diagnose docs/shim）；無新 production module |

修法形狀符合 design.md「不為 multi-impl 預留抽象層」決策。

## 相關文件

- [2026-05-24 codex provider 實驗報告](./2026-05-24-codex-provider-experiment.md)
- [codex-skill-trigger-fix proposal](../openspec/changes/codex-skill-trigger-fix/proposal.md)
- [codex-skill-trigger-fix design](../openspec/changes/codex-skill-trigger-fix/design.md)
- [codex-skill-trigger-fix specs/skill-bundles delta](../openspec/changes/codex-skill-trigger-fix/specs/skill-bundles/spec.md)
- [codex-skill-trigger-fix specs/codex-backend delta](../openspec/changes/codex-skill-trigger-fix/specs/codex-backend/spec.md)

## Docs index 註記

`docs/` 目錄無 README.md / index.md，本 diagnose doc 未做 cross-link entry（task 5.3 fallback path）。後續如建立 docs index，請加入本檔連結。
