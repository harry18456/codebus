# 2026-05-24 — Codex provider 實測報告 + Phase 5 spike 改方向

## 目的

驗證 `Phase 5 spike` 真正想答的問題：「codex Mode B caller-side validate 兜底是否流暢」。原本假設 codex quiz Mode A → generate → caller validate 是 working baseline，只是 in-session self-validate 缺失。

實機跑下去發現**前提就不成立**：codex 5/5 verbs 沒一個完全 work。Phase 5 spike 設計的問題場景走不到。

## 測試環境

| 項目 | 值 |
|---|---|
| codebus binary | 3.0.0（commit `10de31d` 後重新 `cargo install`） |
| codex CLI | `codex-cli 0.133.0`（`C:\Users\harry\AppData\Roaming\npm\codex`） |
| claude CLI | claude code（既有設定）|
| Vault | `/tmp/exp-vault`（今日早 goal 5 runs 累積，7 wiki pages：3 modules + 3 processes + 2 entities + 2 nav）|
| claude provider | Azure (`claude-opus-4-6-2026V2` 系列，per `~/.codebus/config.yaml`)|
| codex provider | Azure (`gpt-5.4` deployment，per `~/.codebus/config.yaml`)|
| 切換方式 | 改 `~/.codebus/config.yaml` 的 `agent.active_provider` |

## 測試結果矩陣

| Verb | claude | codex | 對等性 |
|------|--------|-------|--------|
| **goal** | ✅ ingest 新 wiki page、commit 創建、`content_review: ok` | ❌ **沒寫任何 wiki page**、commit 沒推進、agent 走分析摘要模式 | codex broken |
| **query** | ✅ 引用 vault `[[wikilinks]]` grounded、no-match 不 fabricate | ⚠️ generic JWT 知識答（pseudocode）、結尾 offer「explain in Node/Go/Python JWT library」、沒讀 vault wiki | codex 失 vault grounding |
| **chat** | ✅ multi-turn 讀 vault、no-match 不 fabricate、Scope Guard work | ⚠️ Read wiki + 引用 wikilinks、但 agent emit meta-comment「I found this is a documentation vault rather than application source」表明 SKILL 沒完整 trigger | codex 通但 SKILL mode 不正 |
| **fix** | ✅ Read → Edit、移 broken wikilink、lint 1 warn → 0、commit 創建 | ❌ Agent 識別問題但拒絕：「I could not apply the fix because this session is running with a read-only filesystem sandbox」 | codex sandbox 配置 bug |
| **quiz** | ✅ plan → generate → verify 三 spawn、`content_review: ok` | ❌ **plan 階段死** — 完全沒 emit `[CODEBUS_QUIZ_SCOPE]` marker、codebus 印「error: quiz plan spawn produced no marker」 | codex SKILL trigger broken |

claude path 全 5/5 work。codex path 0/5 完整 work（2/5 部分 work、3/5 全壞）。

## 原始觀察（per-run 摘要）

### codex/goal — 沒寫 wiki

Prompt: `summarize how the auth and database modules interact`

Agent 行為：
- Read 既有 wiki pages（auth-module.md / db-module.md / jwt-payload.md / insert-user.md 等）
- Emit 大段分析 prose 描述 `issue_jwt(user_id)` 用 `sub` claim 作 DB join 等
- 結尾：「One caveat: this workspace contains the generated wiki pages, not the actual `src/auth.py` and `src/db.py`, so the interaction is reconstructed from documentation rather than verified against live source.」
- **沒 Write 任何 wiki page**

codebus 端：
- 印「lint 中... ok 0 errors」+ 「commit 4b31242」+ 「掰掰~下車囉」
- 但 commit `4b31242` 是**前一個** claude/fix 的 commit hash（goal 沒生成新 commit、wiki 沒新增 page）
- exit 0，**silent success but actual failure** —— codebus 沒偵測到 goal SKILL 沒進入 ingest workflow

### codex/query — 失 vault grounding

Prompt: `how does JWT verification handle invalid signatures?`

Agent 行為：
- 沒 Read 任何 vault file（無 tool_use Glob/Read 對 wiki/）
- Emit generic JWT 教學：「JWT verification typically uses HMAC...」、pseudocode `if !verifySignature(token, key): reject("unauthorized")`
- 結尾 offer：「If you want, I can also explain how this is usually implemented in a Node, Go, or Python JWT library.」

對比 claude/query 答案：引 vault `[[jwt-issue-and-verify]]`、`[[auth-module]]` 內具體 `jwt.decode` 行為與「No algorithm pinning」security caveat、全 grounded。

### codex/chat — 半 work

Prompt（stdin single-shot）: `what does the auth module do?`

Agent 行為：
- Read wiki pages（auth-module.md / jwt-issue-and-verify.md / jwt-payload.md）
- 引用 wikilinks (`[auth-module.md](path)` `[jwt-issue-and-verify.md](path)`)
- Grounded answer 描述 `issue_jwt` / `verify_jwt`、symmetric signing、no algorithm pinning caveat
- 但 emit meta-comment：「I found this is a documentation vault rather than application source. I'm reading the auth module doc plus the JWT process docs」+ 結尾「One limitation: this vault only contains documentation that references `src/auth.py`; the actual `src/auth.py` source is not present here」

對比 claude/chat：直接答、不 emit「I found this is...」這種 implementation-plan 風格 meta-thought。SKILL Mode 沒完整 trigger 但 agent 自己決定讀 wiki，所以 user-facing 看似 OK。

### codex/fix — sandbox 配置 bug

Setup：在 `wiki/entities/users-table.md` body 追加 `Also see [[another-fake-page]] for context.` 製造 broken wikilink。`codebus lint` 確認 1 warning。

Agent 行為：
- 跑 PowerShell `Get-ChildItem wiki -Recurse` enumerate pages + 收 slugs + regex 掃 `[[...]]` 找 dangling — 確實找到 `[[another-fake-page]]`
- 識別問題：「I found one concrete wiki integrity issue: `[users-table.md](...)` contains a dangling wikilink `[[another-fake-page]]`」
- **拒絕修**：「I could not apply the fix because this session is running with a read-only filesystem sandbox. The minimal repair is to remove that line from `wiki/entities/users-table.md`」

codebus 端：
- 印「lint 中... ok 0 errors, 1 warnings」+「commit 8edfdda」+「✗ fix: 0 error(s), 1 warning(s) remain after agent terminated」
- exit 0 但 fix 沒成功

對比 claude/fix：成功 Read 後 Edit、移 broken wikilink、lint 變 0 warning、commit `4b31242`。

**根因推斷**：`codebus-core/src/agent/codex_backend.rs` 的 `build_command` 對 `SpawnSpec.permission = Workspace` 沒映射成 codex `-s workspace-write`，仍跑 `-s read-only`。要查 source。

### codex/quiz — Mode A 就死

Prompt: `codebus quiz "JWT issuance and verification" --count 3`

Agent 行為（plan spawn 階段）：
- 第一句 thought：「I'm treating this as a repo task for the `codebus-quiz` project and starting by locating the relevant auth/JWT code so I can produce a concrete implementation plan instead of a generic one.」
- Glob/Grep/Get-Content 探索 vault 結構 + 讀 CLAUDE.md + manifest.yaml + 既有 quiz files
- Emit「**Plan**」+ 5 步驟 + 「**Planned coverage**」+ 「**Expected questions**」摘要
- **完全沒 emit `[CODEBUS_QUIZ_SCOPE]`** 或 `[CODEBUS_QUIZ_NO_MATCH]` first line
- 結尾：「I did not modify files because the workspace is read-only in this session.」

codebus 端：
- 印「error: quiz plan spawn produced no [CODEBUS_QUIZ_SCOPE]/[CODEBUS_QUIZ_NO_MATCH] marker on any line; spawn output head: I'm treating this as a repo task for the `codebus-quiz` project and starting by locating the relevant auth/JWT code...」
- exit 0 但 quiz flow 沒進 generate stage

關鍵：agent 把 SKILL invocation 字串「codebus-quiz」當成「一個 GitHub repo 叫 codebus-quiz」、把 task 當「對該 repo 做 implementation plan」。**SKILL trigger 完全沒進 Mode A 流程**。

## Bug Cluster 分類

### Cluster A — SKILL invocation 沒 trigger（影響：goal、query、quiz Mode A、chat 半通）

**症狀共通點**：

| Verb | Agent 第一句 thought / 行為 |
|---|---|
| goal | 把任務當「分析 auth/db 互動」query 處理，沒 Write wiki |
| query | 不讀 vault、用 generic 知識答 |
| chat | meta-comment「I found this is a documentation vault rather than application source」 |
| quiz | 「I'm treating this as a repo task for the `codebus-quiz` project」 |

**推斷根因**：codex 0.133.0 收到 `$codebus-<verb> <args>` 後，agent 沒進入 SKILL mode（沒 follow SKILL body 的「emit [CODEBUS_QUIZ_SCOPE] first line」/「Write wiki/<type>/<slug>.md」等 SKILL contract），改走 generic「reply to user task」mode。

**已知條件**（per `memory/project_multi_provider_driver_confirmed.md`）：
- 2026-05-22 codex spike 用 codex 0.132.0、端到端 work（Phase 2 SKILL split + Phase 3 SpawnSpec restructure 之後）
- 2026-05-24 實機跑 codex 0.133.0、全 broken

**Hypothesis（待 diagnose）**：
1. **codex 0.132 → 0.133 regression** — native skill invocation 機制改 / token 處理改
2. **SKILL frontmatter description 對 codex 新版預期不符** — codex 對 description 的 trigger 條件改
3. **codebus codex_backend build_command 對 $-prefix 形式組裝錯** — argv 拼錯 / shell escaping 破

### Cluster B — Permission 配置 bug（影響：fix）

**症狀**：fix verb 需要 workspace-write，但 codex spawn 跑在 read-only sandbox，agent 識別出問題拒絕修。

**推斷根因**：`codebus-core/src/agent/codex_backend.rs::build_command` 對 `SpawnSpec.permission = Workspace` 的 codex `-s` 參數映射有 bug、或沒在 args 注入 `-s workspace-write`。

**對應 spec**：`agent-backend` spec 的 SpawnSpec Permission 要求應 work；要驗 claude_backend.rs 和 codex_backend.rs 兩邊對 Permission::Workspace 的處理一致性。

A 與 B 是兩個獨立的 bug，要分別 fix。

## 接下來的工作（明天）

### P1 — `codex-skill-trigger-fix`（最高優先）

**目標**：診斷 + 修正 codex 0.133.0 上 SKILL invocation 失效。

**步驟**：

1. **Reproducer**（快速）：
   ```
   cd /tmp/exp-vault
   sed -i 's/active_provider: claude/active_provider: codex/' ~/.codebus/config.yaml
   codebus quiz "JWT issuance and verification" --count 3
   # 預期看到 'error: quiz plan spawn produced no [CODEBUS_QUIZ_SCOPE]...'
   ```

2. **Diagnose 三層**：
   - (a) **codex CLI 0.132 vs 0.133 行為比對**：能不能裝 0.132.0、跑同樣指令、看是否 work？diff 兩版 release notes / SKILL invocation 機制差
   - (b) **codebus 端 argv**：用 `CODEBUS_CODEX_BIN=/tmp/log-argv.exe` shim 攔下 spawn argv（per memory `project_webview2_cdp_real_frontend` 同類技巧），確認 `$codebus-quiz plan: ...` 真的被傳進去、shell escaping 沒壞
   - (c) **codex 收到 prompt 後是否進 SKILL mode**：codex spawn 加 `--verbose` 或檢視 stream events 看 `skill_invocation` 之類事件

3. **可能修法**（依 diagnose 結果）：
   - 若 codex CLI regression → 改用 `/codebus-<verb>` description-match（per memory `project_codex_skill_invocation_mechanism`，token cost +25% 但通用）
   - 若 SKILL frontmatter format 問題 → 改 `codex-skills/codebus-<verb>/SKILL.md` 的 description 風格
   - 若 codebus argv 組裝 bug → 修 `codebus-core/src/agent/codex_backend.rs::build_command`

4. **驗證**：5 verb × codex provider 重跑，2/5 部分通的（chat / query）也要 grounded、3/5 全壞的（goal / fix / quiz）要恢復。

### P2 — `codex-fix-sandbox-write`

**目標**：fix verb 在 codex provider 端開 workspace-write。

**步驟**：

1. Read `codebus-core/src/agent/codex_backend.rs::build_command`、找 `SpawnSpec.permission` 對應 codex argv 的位置
2. 比對 `claude_backend.rs` 對 `Permission::Workspace` 的處理：claude 是 `--tools Read,Glob,Grep,Write,Edit`；codex 應該是 `-s workspace-write`
3. 寫 unit test 用 mock spawn 攔 argv 驗 `-s workspace-write` 出現在 fix verb 的 codex argv
4. 實機驗證：codex provider 上 `codebus fix` 能真的 Edit wiki page、lint 通過

注意：P1 fix 後 P2 才有意義（P1 不修，agent 連 fix SKILL 都沒 trigger）。

### P3（降優先）— Phase 5 spike 改方向

原本 Phase 5 想答「codex Mode B in-session per-command allowance 可不可行」。實機揭示**前提不成立**（Mode A 就死）。

P1 修好後再回頭評估：
- 若 P1 後 codex quiz 三 stage 都 work（含 generate Mode B + caller-side validate），P5 spike 重新 valuable
- 若 codex CLI sandbox 真的沒 per-command allowance 中間態（這個還沒驗），P5 結論可能仍是「caller-side validate 兜底是設計」

### P4（housekeeping，不急）

- Update memory `project_multi_provider_driver_confirmed.md`：加 2026-05-24 實測 codex 0.133.0 broken 段落、撤回「近期可實際跑 codex」claim
- Update inventory doc：若 P1 修法觸發 SKILL body 改、相關 finding（F26 `$` vs `/` invocation）可能需要重新評估
- 把這份實驗 doc 連結進 README 或 docs index（可選）

## 暫存狀態

- Active provider: claude（已還原）
- exp-vault: 乾淨（broken wikilink 已清、lint 0 warn）
- `~/.codebus/config.yaml`: 還原（無 `.bk` 殘留）
- `/tmp/codex-quiz-run.log` / `/tmp/codex-goal.log` / `/tmp/codex-query.log` / `/tmp/codex-chat.log` / `/tmp/codex-fix.log` / `/tmp/claude-fix.log` 仍在 `/tmp`（明天若想回看可用）

明天接手：先 reproduce P1（5 分鐘確認問題仍在）→ diagnose 三層 → 寫 propose → apply。
