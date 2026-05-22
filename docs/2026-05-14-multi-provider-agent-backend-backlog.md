# Backlog: multi-provider agent backend（Codex CLI + Azure endpoint）

**Date:** 2026-05-14
**Surfaced during:** backlog 討論（v3-app-chat-cmdk apply 期間）
**Severity:** feature gap（架構擴充性）
**Owner:** harry
**Status:** unblocked — 2026-05-20 codex CLI 0.132.0 spike 確認 contract 完整、second-impl 條件滿足

---

## 2026-05-20 更新：codex 0.132.0 spike 結果（contract 完整）

2026-05-20 同日連跑兩家 agentic CLI spike：

- `agy` 1.0.0（Antigravity，2026-05-19 上市）— 缺 `--tools` 白名單、無 `--output-format stream-json`、`-p` mode 看不到 agentic tool loop 證據。**不適合作為 second-impl 對標**
- `codex` 0.132.0（OpenAI Codex CLI）— **所有 codebus 需要的 contract 都有**，部分還比 Claude CLI 更乾淨

### Codex 0.132.0 seam 對映表（vs Claude）

| Seam | Claude CLI | Codex 0.132.0 | 整合工作 |
|---|---|---|---|
| Non-interactive | `claude -p` | `codex exec` | 直接對映 |
| Structured stream | `--output-format stream-json --verbose` | **`--json`** | 寫第二支 parser |
| Tool call event in non-interactive | ✓ ToolUse + ToolResult | ✓ `item.started/completed type:command_execution` 一對 | 映成 `ToolUse + ToolResult` |
| Session resume | `--resume <id>` | `resume`/`fork` 子命令 | 直接對映 |
| Sandbox 模型 | `--permission-mode acceptEdits`（單級） | **`--sandbox read-only/workspace-write/danger-full-access`**（三級，比 Claude 細）| chat → `read-only`，goal/fix → `workspace-write` |
| Approval policy | implicit via permission-mode | **`--ask-for-approval untrusted/on-request/never`** | 多一個顯式控制軸 |
| Hook system | `.claude/settings.json` PreToolUse | **存在**（`--dangerously-bypass-hook-trust` flag + `.rules` execpolicy）| `.codex/` 或同等位置寫 hook config |
| Skill bundle format | `.claude/skills/<name>/SKILL.md`（yaml frontmatter + md） | **`~/.codex/skills/<name>/SKILL.md`** —— 完全相同 yaml + md 格式 | **共用內容、雙寫 `.codebus/.claude/skills/` 跟 `.codebus/.codex/skills/`** |
| Plugin system | ✗ | **`.codex-plugin/plugin.json` + marketplace** | 可選，先用 skill 不走 plugin |
| MCP | ✗ | **client + server 全支援** | 未來可選 |
| Schema constraint | ✗ | **`--output-schema <FILE>`**（codex 獨有）| 未來 quiz/goal verify 可利用 |
| Token usage | input/output/cache_read/cache_create | input/output/**cached/reasoning_output**（更乾淨）| `TokenUsage` 已有 `reasoning_tokens` 欄位，直接對應 |

### Stream event shape 實測

`codex exec --json --sandbox read-only "list files... then say done"` 輸出：

```jsonl
{"type":"thread.started","thread_id":"019e4574-..."}      ← session_id
{"type":"turn.started"}
{"type":"item.started","item":{"type":"command_execution","command":"powershell ...","status":"in_progress"}}
{"type":"item.completed","item":{"type":"command_execution","aggregated_output":"...","exit_code":0,"status":"completed"}}
{"type":"item.completed","item":{"type":"agent_message","text":"done"}}
{"type":"turn.completed","usage":{"input_tokens":30515,"cached_input_tokens":22272,"output_tokens":43,"reasoning_output_tokens":0}}
```

對映到既有 `StreamEvent`：

| Codex event | codebus `StreamEvent` |
|---|---|
| `thread.started.thread_id` | session_id 抽取點（chat verb 用） |
| `item.started type:command_execution` | 可選 skip（只是 intent，不必 emit） |
| `item.completed type:command_execution` | `ToolUse {name:"Bash", input:{command}} + ToolResult {output:aggregated_output, is_error: exit_code != 0}` 一對 |
| `item.completed type:agent_message` | `Thought { text }` |
| `turn.completed.usage` | `Usage(TokenUsage)` |

### 工程量重估

原 backlog 估「重（1 週以上；spike 結果影響估算）」，spike 完後估**約 1-2 週**：

- 不是「重做」，是「加 `CodexBackend` impl + `parse_codex_stream_line` + skill bundle 雙寫 + config schema 加 codex profile + agent::invoke routing」
- skill bundle 完全共用（同 yaml frontmatter + md 格式），雙寫成本只是 `vault/init.rs` 多 copy 一次
- sandbox 對映比 Claude 還乾淨（chat `read-only` 是 codex 原生 primitive，不需 prompt-layer defense-in-depth）

### 重要校準（vs 早期 framing）

之前 backlog 寫「codebus 一直以來定位為 multi-AI-provider 工具，但目前實作完全硬耦合到 `claude` binary」—— **這個 framing 不完整**。實際上：

- 資料層（`StreamEvent` / `TokenUsage` / `RunLog`）在 `v3-run-log-events`（Stage 2）已 normalized，**只剩一支 `parse_claude_stream_line` 是 Claude 專屬**
- 真正卡的不是「codebus 抽象不夠」，是「過去沒有合格的 second-impl 對標目標」
- agy 不合格（contract 不完整）、codex 合格（contract 完整）

### 何時動（更新）

原列「v3-app-polish-ship（F）之後」—— user 2026-05-20 明確 deprioritize polish-ship，本條跟 polish-ship 沒有硬依賴順序，**可在 user 自己想動時起 `/spectra-propose`**。

---

## 2026-05-22 grounding + 拆解計畫（discuss 結論，逐點對過實作）

Stage 1（archived change `agent-backend-seam`）已完成：`AgentBackend` trait（3 method：`build_command`/`parse_stream_line`/`extract_session_id`）+ `ClaudeBackend`（`compose_claude_cmd` 組 argv，含 `--strict-mcp-config` MCP 隔離，完好）+ 中性 `SpawnSpec`（`verb`/`prompt`/`permission`/`command_allowance`/`resume_session_id`）+ `invoke` 迴圈吃 `&dyn AgentBackend` + verb 改組 SpawnSpec + config 統一 `agent.providers.<name>.*`。

逐點對實作核對結果（**有兩處與「純填 dispatch」直覺不符**）：

- ✅ **trait 已 ready、Codex 是純加法**：`invoke(&dyn AgentBackend, ...)`（`codebus-core/src/agent/claude_cli.rs`）。加 `codex_backend.rs` 實作 3 方法即可。
- ⚠️ **沒有 runtime backend dispatch**：每個 verb **硬編 `ClaudeBackend::new(...)`**（`codebus-core/src/verb/{goal,query,fix,chat,quiz}.rs`）。Stage 2 要新增「provider → `Box<dyn AgentBackend>`」選擇層 + 改每個 verb 的建構點。
- ⚠️ **config 主動拒絕非 claude**：`codebus-core/src/config/endpoint.rs` 解析時 `active_provider` 只接受 `claude`，否則 reject（有對應測試）。Stage 2 要解除此 guard。
- ❌ **～～GUI/CLI config 脫鉤～～（此條 2026-05-22 discuss 已證實 STALE，下面是更正）**：原寫「前端 `ipc.ts`/`settings.ts` 仍用舊 `claude_code.*`、寫到 core 已不讀的 key」。**實測為錯**——`settings.ts` 的讀(`readClaudeCodeBlock`)寫(`updateClaudeCode`)早在 Stage 1 同一個 commit `6a7ea0f` 就已遷到 `agent.providers.claude`（`git log -S` 確認）；CLI `config.rs` 透過 core loader 讀，也對齊。前端殘留的 `claude_code.*` 只是 `validateClaudeCodeBlock` 的**純前端 validation field-path 識別碼**（不送後端）+ `AgenticProvider="claude_code"` 字面值 + 型別名，**無功能性脫鉤**。
- ⚠️ **真正的 config bug（更正後）**：舊格式 **top-level `claude_code:`**（內層已是 system/azure profile 形狀，但沒搬進 `agent.providers.claude:`）→ `parse_claude_code_yaml` 找不到 `agent` 區塊 → 回 `Missing` → `load_claude_code_config`（`claude_code.rs:153`）**靜默套 `ClaudeCodeConfig::default()`，連警告都不印**。既有的扁平-legacy 警告機制不認這個形狀，掉進縫隙。harry 自己的 config 正中此 bug（設的 `sonnet-4-6` goal 被當 `opus-4-6` 跑）。**Severity：silent failure。但見下方拆解 § 的處置決定。**
- ⚠️ **skill bundle 只寫 `.claude/`**：`codebus-core/src/skill_bundle/mod.rs`（`base.join(".claude")`）無 `.codex`。雙寫 `.codex/skills/` 為淨新增。
- ❓ **MCP 隔離 spike 缺口**：2026-05-20 spike 表未涵蓋 codex 的 MCP 載入層隔離旗標。Claude 端 `--strict-mcp-config` 已是必備（archived `spawn-mcp-isolation`），CodexBackend 必須有等價隔離，否則重蹈 MCP 洩漏。**動 CodexBackend 前必補 spike。**

### 拆解計畫（2026-05-22 discuss 更正：原 3 change → 砍 #1 變 2 change）

**為何砍 `agent-config-rewire`：** 它原本兩個賣點都垮了——
1. 「修 GUI config 脫鉤」：上面已證實 settings.ts 在 Stage 1 就遷好了，脫鉤不存在。
2. 「真正的 top-level `claude_code:` 靜默丟棄 bug」：唯一中招的 config 是 harry 自己的（solo、pre-release、無外部 user）。**處置決定：harry 手動把自己的 config 從 `claude_code:` 搬到 `agent.providers.claude:`（或重設），不寫任何回溯相容/遷移程式碼**——為一個只影響開發者本人、手改 30 秒可解的舊檔寫 back-compat shim，正是 polish-ship / speculative-abstract（見 memory `feedback_dont_default_polish_ship`、`feedback_dont_speculative_abstract`）。harry 已於 2026-05-22 刪除/搬移自己的 config。
   - 剩下的 dispatch 層 + 解除 guard 都是純 codex 前置、無 standalone 可觀察行為（dispatch 在只有 claude 時是 pass-through，guard 放寬在沒有非-claude config 時無意義）。0 第二消費者 → 不獨立成 change，**併進 #2（原 #3）codex-backend**，等真有 CodexBackend 當第二消費者時才落地。

剩 2 個 change（依序）：

1. **`codex-spike`（investigation）**：實機跑 codex 當前版，確認 contract 仍成立 + 找出 MCP 載入層隔離旗標（Claude 端 `--strict-mcp-config` 等價物，缺則重蹈 MCP 洩漏）+ sandbox 對映（query/chat→read-only、goal/fix→workspace-write）。產出餵 #2。
2. **`codex-backend`（依賴 #1 spike）**：含三部分——(a) **dispatch + guard**（原 agent-config-rewire 內容）：新增 provider→`Box<dyn AgentBackend>` 選擇層改掉 5 個 verb 硬編 `ClaudeBackend::new(...)`、解除 `endpoint.rs` `active_provider` 非-claude reject guard、`RawProviders` 加 `codex` 欄位；(b) **CodexBackend**：`codex_backend.rs` 實作 trait 3 方法、`parse_codex_stream_line`、session id（`thread.started.thread_id`）；(c) **config + bundle**：`agent.providers.codex.*` profile + Azure variant、skill bundle 雙寫 `.codex/skills/`。前端 validation field-path 若要支援 codex 端點 UI，順手改成 provider-aware。

### Driver（2026-05-22 已確認）

**有真實 driver：harry 近期要實際跑 codex。** 依上方更正後的 2-change 計畫執行：先 `codex-spike`（investigation）→ 再 `codex-backend`（dispatch+guard+CodexBackend+config/bundle，依賴 spike 產出）。原 `agent-config-rewire` 已砍（空殼 change 於 2026-05-22 移除）。

（原 driver 問題：有具體要用 codex / Azure OpenAI 的場景嗎？還是先把能力備好？— 已回答：近期要實際跑 codex，非僅備用。）

---

## 2026-05-22 spike 實機結果（codex 0.132.0，取代「先做 codex-spike change」）

discuss 決定 spike 不走 Spectra（investigation 跟 spec-driven schema 不合），直接實機跑、findings 記在此。環境：`codex-cli 0.132.0`（與 2026-05-20 同版）、Windows、已用 ChatGPT 登入。

### 1. Event stream shape — 確認成立，且比 2026-05-20 表更細

實跑 `codex exec --json --skip-git-repo-check --ephemeral --ignore-user-config --ignore-rules -s read-only "<跑 echo 再回 DONE>"`，實得 JSONL：

```jsonl
{"type":"thread.started","thread_id":"019e4d0e-..."}
{"type":"turn.started"}
{"type":"item.completed","item":{"id":"item_0","type":"agent_message","text":"I'll run the requested..."}}
{"type":"item.started","item":{"id":"item_1","type":"command_execution","command":"\"...powershell.exe\" -Command \"Write-Output 'hello-from-codex'\"","aggregated_output":"","exit_code":null,"status":"in_progress"}}
{"type":"item.completed","item":{"id":"item_1","type":"command_execution","command":"...","aggregated_output":"hello-from-codex\r\n","exit_code":0,"status":"completed"}}
{"type":"item.completed","item":{"id":"item_2","type":"agent_message","text":"DONE"}}
{"type":"turn.completed","usage":{"input_tokens":32334,"cached_input_tokens":29440,"output_tokens":77,"reasoning_output_tokens":0}}
```

對 2026-05-20 對映表的**修正/補充**（餵 `parse_codex_stream_line`）：

- **items 帶 `id`**（`item_0`/`item_1`/…）：用來配對同一 item 的 `started`/`completed`（command_execution 一對共用 `item_1`）。
- **`agent_message` 會出現多次**：中途敘事（"I'll run…"）+ 最終答案（"DONE"）都是 `agent_message`。映 `Thought{text}` 對；但**最後一則 agent_message 才是答案**——verb 層需比照 Claude「取最後一則 assistant 文字為結果」。
- **`command_execution.command` 是完整 shell 調用字串**，Windows 下被包成 `powershell.exe -Command "..."`（非裸 `echo`）。映 `ToolUse` 時 `name` 用中性 `"Shell"` 比 `"Bash"` 準。`exit_code` 在 `in_progress` 為 `null`、`completed` 為實際碼 → `ToolResult.is_error = exit_code != 0`。
- **usage 欄位實名**：`input_tokens` / `cached_input_tokens` / `output_tokens` / `reasoning_output_tokens`（2026-05-20 寫的「cached/reasoning_output」是簡寫，實名如此）。對映 `TokenUsage`（已有 `reasoning_tokens` 欄）。

### 2. Sandbox — 三級確認

`codex exec -s` `[possible values: read-only, workspace-write, danger-full-access]`。對映維持：query/chat→`read-only`、goal/fix→`workspace-write`。實測 read-only 下 `Write-Output`（命令執行）仍可跑——read-only 擋的是**磁碟寫入**、不擋命令執行，符合文件語意。

### 3. ⚠️ Approval policy — `codex exec` 沒有 `-a/--ask-for-approval`

`-a/--ask-for-approval untrusted/on-request/never` 是**互動模式 `codex` 才有的 flag**，`codex exec` 不接受（實測 `-a` 報 `unexpected argument`）。exec 本身非互動、預設不問。2026-05-20 表的 approval-policy 列只適用互動模式，**codebus 走 exec → 此軸不存在、不必對映**。

### 4. MCP / plugin 隔離 — 完整圖（多次更正後的定稿，全部對過官方文件）

> 本節經歷三次修正:初稿誤稱「`--ignore-user-config` = `--strict-mcp-config` 等價、更徹底」(over-claim)；二稿改成「擋不住 project 層、是缺口」；定稿(查 managed-config 文件 + trust 文件)如下。`--strict-config` 黑箱探針失敗、不採信。

**權威來源**:[config-reference](https://developers.openai.com/codex/config-reference)、[config-advanced](https://developers.openai.com/codex/config-advanced)、[cli/reference](https://developers.openai.com/codex/cli/reference)、[enterprise/managed-configuration](https://developers.openai.com/codex/enterprise/managed-configuration)、issues [#9695](https://github.com/openai/codex/issues/9695)/[#15433](https://github.com/openai/codex/issues/15433)。

**(A) config 分層 + trust 是總開關**
- 三層:user 全域 `~/.codex/config.toml` → **project 層 `.codex/config.toml`**(codex 從 project root[`.git` 偵測]走到 cwd 沿路載入,最近的覆蓋)→ CLI `-c`/`-m`/`-s`。mcp_servers 與 plugins **兩層都可出現**。
- **trust 是 project `.codex/` 整棵樹的總開關,一視同仁**:untrusted → config.toml + hooks + rules + **`.codex/skills/` 全部不載**(#9695)。trust 狀態存在 `~/.codex/config.toml` 的 `[projects.*]`。

**(B) 旗標各擋什麼**
- `--ignore-user-config`:只「不載 `$CODEX_HOME/config.toml`」= **只擋 user 全域層**(全域 mcp_servers/plugins/trust 清單)。**擋不住 project 層**。（無 `--no-config` 旗標,已確認。）
- `--ignore-rules`:涵蓋 **user + project** execpolicy `.rules`。
- 連帶效應:`--ignore-user-config` 使 trust 清單未載 → 目標 project 多半變 untrusted → project `.codex/` 整棵被忽略。**這同時擋掉惡意 project config,也擋掉 codebus 自己的 `.codebus/.codex/skills/`**（trust 一視同仁的代價）。

**(C) 真正的乾淨原語 = `requirements.toml`（受管設定層，codex 版 `--strict-mcp-config`，且更硬）**
- 「`mcp_servers` allowlist **空的 → codex 停用所有 MCP server**」;非空則只放行 name+identity 吻合者。亦可強制 sandbox mode / network / command execution rules / feature flags。
- **絕對優先**:與 config.toml/profiles/CLI override 衝突時 codex 退回相容值 → **蓋過 project 層、使用者繞不過**。
- ⚠️ **交付是機器層、非 per-spawn**:Windows `%ProgramData%\OpenAI\Codex\requirements.toml`、Unix `/etc/codex/requirements.toml`、或 MDM `requirements_toml_base64`、或 cloud-managed。**全機器生效**(會連 harry 自己互動用的 codex 一起管),且裝它需要對應權限。沒有「本次啟動指定一份 requirements」的旗標。

**(D) raw/code（cwd 之下）不被當設定/指令自動載入**
- config/skills/AGENTS 的搜尋是 project-root→cwd,**不往 cwd 之下鑽**。`.codebus/raw/code/` 在 cwd 之下 → 其中的 `.codex/config.toml`、`AGENTS.md` **不會被自動載入**。TEST B 實測:`raw/code/AGENTS.md` 放暗號,模型答 UNKNOWN(沒自動知道)。
- 它們唯一的影響面:agent **分析程式碼時把它們當資料讀**,可能照惡意指令做(prompt-injection-via-content)——這是內容注入、非設定載入,read-only sandbox 壓得住,且 Claude 分析同一鏡像同樣有此風險。**非 config 隔離問題。**

**codebus 的設計矛盾（codex-backend 動工前要拍板）**
> 想要 `.codebus/.codex/skills/`(自家 skill)被載入 → 需 **trusted**;不想要任意 repo 的 `.codex/config.toml` 注入 → 需 **untrusted**。trust 一視同仁,不能只要 skill 不要 config。

候選解(codex-backend 評估):
1. **requirements.toml 空 MCP allowlist**(機器層裝一次)硬擋所有 MCP/plugin,**搭配 trusted**(讓自家 skill 載入)+ `--ignore-user-config` + `--ignore-rules` + per-verb `-s`。最強隔離,但 requirements 全機器生效=安裝步驟、且影響 harry 自己的 codex。
2. **untrusted + 不靠 project skills**:skills 改放全域 `~/.codex/skills/`（非 trust-gated;但污染使用者全域、且 `--ignore-user-config` 是否影響全域 skills 待驗）。
3. **spawn 前掃描**目標 repo 路徑有無 `.codex/config.toml`,有則警告/拒絕(輕量但非強隔離)。

**(E) 實機驗證結果（2026-05-22，用啟動 banner `reasoning effort` 當 canary——harry 全域設 medium，可靠）**
- ✅ `--ignore-user-config` 確實擋 user 層:帶旗標 banner=`none`、不帶=`medium`(Test 1/1b)。
- ✅ **未信任 repo 的 project `.codex/config.toml` 不洩漏**:throwaway git repo T(不在 trust 清單)放 `model_reasoning_effort=high`,帶/不帶 `--ignore-user-config`、cwd 在 root 或子目錄,banner **從不顯示 high**(Test 2/2b/2c)→ 任意/剛 clone 的 repo 預設未信任,其 `.codex/config.toml`(MCP/plugin 注入)**自動被擋**。注入僅在 repo 已被信任時才發生。
- ✅ **未信任 repo 的 `.codex/skills/` 也一起被擋**:同 throwaway git repo 放 codeword skill,`--ignore-user-config` 下模型答 UNKNOWN(skill 未載入)。**證實 trust 一視同仁(#9695)**。⚠️ 連帶更正 §5b:先前「skill 雙寫已驗可行」是在**無 `.git`** 的 dir 測的(無 project → 無 trust gate),不具代表性;**真實 codebus 拓樸(vault 在 git repo 內)下,自家 skill 載入受 trust 管**。
- 💡 **解法候選(部分文件支持,marker 行為待實測)**:用 `project_root_markers` 把 **`.codebus/` 設為 codex 的 project root** → 搜尋範圍限縮在 `.codebus/`(codebus 全控)、**被分析 repo 根的 `.codex/` 在 root 之上被排除**;再只信任 `.codebus/` → 自家 skill 載入 + repo 注入排除,**化解 trust 矛盾**。

**(F) 完整隔離配方 — 端到端實機驗證（2026-05-22，harry 授權動環境、已還原成位元組相同）**

> 🔴 **2026-05-22 最終更正(用「暗號放 description + 驗證無 command_execution」清淨法,推翻先前 KIWI 結論)**:先前說「未信任 → `.codex/skills/` 不註冊、只能靠 AGENTS.md」是**誤判**。KIWI 測試暗號放在 **body**、又禁讀檔,模型拿不到 body 才答 UNKNOWN——但 **description 一直有注入(= skill 有註冊)**。重測(暗號只在 description、stream 確認無讀檔):隔離配方 + 未信任下 `.codebus/.codex/skills/` 的 description **照樣注入**(`skill=BRAVO-2`)。**結論修正:codex skill 與 AGENTS.md 兩條通道對 codebus 都可用,不需 trust、不需 requirements.toml。`project_root_markers` 才是排除被分析 repo 注入(skills + AGENTS.md)的關鍵**——實證 ①a 有 marker→repo 的 CHARLIE-3/EVIL-9 被排除;①b 無 marker→兩者雙雙注入。

最終配方是**全 per-spawn 旗標 + codebus 自寫的 `.codebus/AGENTS.md`(可選:+ `.codebus/.codex/skills/`),不需機器層 requirements.toml、不需持久化 trust**。`.codebus/` 內的 AGENTS.md 與 skills 在隔離配方下都會被 codex 採用;`project_root_markers` 把 `.codebus/` 設 root 後,被分析 repo 的 `.codex/`+`AGENTS.md` 全部排除。

**推薦配方(整合測試 INTEG-A/B/C 一次套全通過)**:
```
codex exec --json \
  --ignore-user-config \                       # 擋 user 全域 config(mcp_servers/plugin-enable/trust 清單)
  --disable apps \                             # 擋 plugin(codex_apps,如 hugging-face)
  --ignore-rules \                             # 擋 user+project execpolicy .rules
  -c "project_root_markers=['<codebus-marker>']" \  # .codebus/ 設為 project root → 排除被分析 repo 的 .codex/ + AGENTS.md
  -s read-only|workspace-write \               # per-verb 檔案/網路沙箱(read-only profile 含 network:Restricted)
  -C <repo>/.codebus                           # cwd=vault;codebus 在此放 AGENTS.md(+ 鏡射 CLAUDE.md 內容)驅動行為
```

| 注入渠道 | 阻擋 | 實證(本次硬驗) |
|---|---|---|
| user 全域 config.toml(mcp_servers + plugin enable) | `--ignore-user-config` | ✅ reasoning canary medium→none |
| config.toml `[mcp_servers.*]`(node_repl) | `--ignore-user-config` | ✅ INTEG-C 工具清單無 node_repl |
| **plugins（codex_apps）** | **`--disable apps`** | ✅ 工具清單 hugging_face 消失 + **實際叫用 `_hf_whoami` 回 TOOL-NOT-AVAILABLE**(非自報) |
| 被分析 repo 根 `.codex/config.toml` | `project_root_markers`→`.codebus/` 設 root | ✅ INTEG-A model=gpt-5.5(非 repo 的 zzz-MALICIOUS) |
| **被分析 repo 根 `AGENTS.md` 注入** | 同上 marker | ✅ INTEG-B repoA=NONE、無 [PWNED](repo 的 EVIL-AGENTS 被排除) |
| 被分析 repo `.codex/skills/` | marker(排除)+ `-s` 沙箱(讀不到 cwd 之上) | ✅ INTEG-B repoS=NONE |
| execpolicy `.rules` | `--ignore-rules` | doc(user+project) |

**重要更正(差點 over-claim)**:`AGENTS.md` **會從被分析 repo 根注入**(`.codebus/` 之上),且**不受 trust 管、`--ignore-user-config` 也擋不掉**——INJ-3 實證:codeword 只放 repo 根,模型答出 REPO-SECRET-9。一度因「append token 指令模型沒照做」誤判 repo AGENTS.md 沒載入(INJ-1),清淨 codeword 法揭穿。**唯一可靠阻擋 = `project_root_markers` 把 root 限縮到 `.codebus/`**(MARKER-AGENTS 實證:同樣設定下 codeword 變 UNKNOWN、自家 VAULT-OK-7 仍生效)。

**其他已釐清的點**:
- **`-c projects.'<path>'.trust_level` 灌不進 trust**(三種路徑寫法都 medium=沒生效)→ trust 只能靠持久化 config.toml 或互動;codex 應是刻意不讓 CLI override 信任(安全設計)。**故配方改走 AGENTS.md、完全不依賴 trust。**
- requirements.toml 在本機可寫(harry 擁有 `%ProgramData%\OpenAI\Codex\`,免管理員)且生效(canary:`-s workspace-write` 被夾回 read-only、報錯點名該檔);但**空 `[mcp_servers]` 擋 config MCP 卻擋不掉 plugin**(plugin 走 codex_apps)→ 既然 `--ignore-user-config`+`--disable apps` 已是 per-spawn 全擋,**requirements.toml 非必要**(留作機器層硬底線選項)。
- 全域 `~/.codex/skills/<name>/` 直接放(我那個放法)**不會自動 surface**(帶不帶旗標都 UNKNOWN)→ 全域 skill 非可行替代;能用的是 AGENTS.md。

**tool-gating parity(另列,非隔離漏洞)**:`--disable apps`+`--ignore-user-config` 後仍有 codex **內建**工具:`web.run`(網路,但 read-only 沙箱 network:Restricted)、`image_gen`、`spawn_agent`(多代理)、`view_image`、`apply_patch`、`functions.shell_command`。這些非攻擊者注入,但若要對齊 codebus 對 Claude 的 per-verb `--tools` 白名單,codex-backend 要找等價的工具限縮機制(待查:requirements `rules`/command execution 或 feature flags;非本 spike 阻塞項)。

### 5. `AGENTS.md` — 既是工作項(生成自家)又是注入面(repo 根),靠 marker 化解（2026-05-22 兩度更正,定稿見 §4(F)）

**經兩次更正。** 早期測 `/tmp/agtest`(parent vs cwd 同問 codeword)→ 答 cwd 的 VAULT,我誤推「上層不覆蓋 = 無注入風險」。**錯**——那只證明「衝突值由 cwd 勝出」,沒證明 repo 那層沒載入。INJ-3 用清淨法(codeword 只放 repo 根)實證:**repo 根的 `AGENTS.md` 確實載入並注入(答出 REPO-SECRET-9),且不受 trust 管、`--ignore-user-config` 擋不掉**。

所以 `AGENTS.md` 有兩面:

- **工作項面**:codex 的權威指示檔就是 `AGENTS.md`(對應 Claude `CLAUDE.md`)。codex-backend 要**生成 `.codebus/AGENTS.md`**,鏡射 `.codebus/CLAUDE.md` 的 taxonomy/frontmatter/語言政策。實證:`.codebus/AGENTS.md` 不受 trust 管、always 載入(DURIAN/VAULT-OK test)→ 這正是 codebus 驅動 codex 的可靠通道(不必依賴 trust-gated 的 skills)。
- **注入面**:被分析 repo 根的 `AGENTS.md` 會注入。**阻擋 = `project_root_markers` 把 `.codebus/` 設 root**(MARKER-AGENTS + INTEG-B 實證:repo 的 EVIL-AGENTS 被排除、自家仍生效)。這是 §4(F) 配方的必要一環,**非可選**。
- (`~/.codex/AGENTS.md` 全域層 harry 為 0 byte;且 `project_root_markers` 限縮 root 後不在搜尋路徑。)

### 5b. Skill 探索 — 專案層 `<cwd>/.codex/skills/` 確認可讀（實證，非照抄假設）

2026-05-20 backlog 寫「`~/.codex/skills/<name>/SKILL.md`」只點到**全域**層。本次補驗**專案層**(codebus 需要的層級,因 spawn cwd=`.codebus/`)：

- 建 `/tmp/agtest/vault/.codex/skills/codeword-skill/SKILL.md`（yaml frontmatter `name`/`description` + body），`codex exec -C /tmp/agtest/vault --ignore-user-config ...` 問 codeword → 模型**啟動即自報**「I'm using the `codeword-skill`…」（證明 codex 自動發現專案層 skill、把 frontmatter 注入 context），再 `Get-Content` 讀 body → 答 `BANANA-7`。
- 機制與 Claude skill 相同：name/description 常駐、body 按需載入（codex 透過 read-only 沙箱下的 shell `Get-Content` 讀 body，read-only 允許）。
- **`.agent/skills/` 對照測試（實證,不是只看答案）**：建 `/tmp/agtest2/vault/.agent/skills/codeword-skill/SKILL.md`(codeword MANGO-9)同樣跑。模型答對了 MANGO-9,**但 event stream 顯示完全不同路徑**：item_0「I'll check the workspace」(不知道有 skill）→ `Get-ChildItem -Force` → `rg --files` → grep "codeword" → 才撈到檔。即 **`.agent/` 不是 codex 認得的 skill 目錄**;會答對純因該檔在 cwd、模型剛好全文搜尋撈到（任何含關鍵字的 `notes.txt` 同樣會被撈）。**對比 `.codex/`：模型啟動即自報 "I'm using the codeword-skill"(frontmatter 已注入 context)= 真正的 skill 註冊/漸進揭露。**
- **結論**：skill 目錄是 **`.codex/`**(codex 慣例 `$CODEX_HOME`/`~/.codex`)。**`.agent/` 不通**（非 skill 機制、只是被當普通檔瞎找）、**非兩者通吃**。⚠️ 教訓：只看「有沒有答對」會誤判成兩者皆可——必須看 event stream 區分「skill 註冊」vs「workspace 全文搜尋撈到」。
- ✅ **更新(見 §4(F) 🔴 最終更正)**:重測證實 `.codebus/.codex/skills/` 的 description 在隔離配方 + 未信任下**照樣注入(會註冊)**——先前「受 trust 管、不載」是 KIWI body+禁讀檔的誤判。**carry-over (b) 的 skill 雙寫無條件可行**(只要 `project_root_markers` 把 `.codebus/` 設 root 排除 repo 注入)。
- 全域內建 skill 在 `~/.codex/skills/.system/`（imagegen / openai-docs / plugin-creator），有 `.codex-system-skills.marker`；user skill 走 `~/.codex/skills/<name>/`。全域 skills 是否受 `--ignore-user-config` 影響**尚未實測**(§4(E) 待驗)。

### 6. codebus 該用的 `codex exec` flag 清單

`--json`（JSONL）、`--skip-git-repo-check`、`--ephemeral`（不落 session 檔）、`-C/--cd <DIR>`（工作根）、`--add-dir`（額外可寫目錄）、`-m/--model`、`-s/--sandbox`、`--ignore-user-config`（MCP/config 隔離）、`--ignore-rules`（execpolicy 隔離）、`--output-schema <FILE>`（codex 獨有，quiz/goal verify 可用）、`-o/--output-last-message <FILE>`、`-c key=value`（TOML 覆寫）。

注意：stdin 非 TTY 時印 stderr `"Reading additional input from stdin..."`——**codebus spawn 必須管好 stdin（關閉/餵空 EOF），否則 codex exec 會卡住等 stdin 永不返回**（實測:背景無 TTY 時掛 60s+ 0 輸出;前景 `$null | codex` 立即正常）。

### 7. Azure OpenAI — 實機通過（2026-05-22,用 harry 真實端點 + Claude 同把 key 實測）

實打 `https://2026msf13.cognitiveservices.azure.com`(deployment `gpt-5.4`),codex 回 `AZURE-OK` + usage(含 `reasoning_output_tokens`)。可運作 provider config:
```toml
model_provider = "azure"
[model_providers.azure]
base_url = "https://2026msf13.cognitiveservices.azure.com/openai"   # codex 自接 /responses
wire_api = "responses"                                              # ⚠️ chat 已不支援(0.132.0 砍掉)
env_key = "AZURE_KEY"
query_params   = { "api-version" = "2025-04-01-preview" }
env_http_headers = { "api-key" = "AZURE_KEY" }                      # Azure 用 api-key header,非 Bearer
```
+ `-m gpt-5.4`(deployment 名當 model,放 body)。key 從 keyring `codebus-azure`(target `default.codebus-azure`)讀。

**要點**:(1) **必走 Responses API**——harry 給的 `/chat/completions` 路徑不能用,codex 0.132.0 `wire_api="chat"` 報「no longer supported」;base_url 給 `/openai`,codex 接 `/responses`。(2) api-version `2025-04-01-preview` 在 Responses API 可用。(3) auth = `api-key` header(`env_http_headers`),非 Authorization Bearer。(4) **codebus config 設計**:harry 要 codex 的 Azure 設定與 Claude **分開**——codex-backend 給 `agent.providers.codex` 自己的 azure profile（自己的 base_url / api-version / keyring_service;key 可暫共用 `codebus-azure` 但 config 獨立）。

### spike 結論

contract 完整成立、sandbox 對映確認、AGENTS.md/skill/raw-code 治理面全釐清、**Azure OpenAI 實機通過(§7)**、resume 可用、config 不被污染(`--ignore-user-config` 防寫)。**隔離面已端到端實機驗出乾淨配方(§4(F),INTEG-A/B/C 一次套全通過)**:全 per-spawn 旗標 + 自寫 `.codebus/AGENTS.md`,**不需機器層 requirements.toml、不需持久化 trust**——
```
--ignore-user-config + --disable apps + --ignore-rules
+ -c project_root_markers=['<codebus-marker>'] + -s <per-verb> + -C <repo>/.codebus
```
擋下:user 全域 config/MCP、plugin(codex_apps,硬驗 `_hf_whoami`→TOOL-NOT-AVAILABLE)、被分析 repo 的 `.codex/config`+`AGENTS.md`+`.codex/skills`(`project_root_markers` 限縮 root 排除——實證 ①a/①b)、execpolicy。codebus 靠 `.codebus/AGENTS.md`（+ 可選 `.codebus/.codex/skills/`,兩者隔離下都會註冊/載入)驅動。

codex-backend carry-over:(a) 落地上述配方(已驗,直接寫 spec);(b) 生成 `.codebus/AGENTS.md`(鏡射 CLAUDE.md)+ 可選 skill 雙寫;(c) `agent.providers.codex` config schema(含 system + 獨立 azure profile,§7)。

**tool-gating 補述(實測,非阻塞)**:codex 權限模型是**沙箱**(`-s`)非工具白名單——`-s read-only` profile `network: Restricted`,實測 shell 對外連線被擋(read-only 與 workspace-write 皆然)。`web.run` 雖在工具清單,但**實測無可觀察外洩**(任何 run 都無 `web_search_call` 事件;動態 UUID 回傳相同假值=模型唬爛非真 fetch)。故無 demonstrated egress。內建 spawn_agent/image_gen 同屬沙箱邊界內、非攻擊者注入。先前「web.run 是 egress」是 hallucination confound、已更正。

---

## 觀察

codebus 一直以來定位為 multi-AI-provider 工具，但目前實作完全硬耦合到 `claude` binary：

```rust
// codebus-core/src/agent/invoke.rs（示意）
// 直接 spawn "claude" binary，假設 --output-format stream-json
// VerbEvent 對應 Claude 專屬 event schema
```

OpenAI 於 2025 年 4 月發布 **Codex CLI**——一個 terminal-based coding agent（類似 Claude Code 但底層是 GPT-4o / o3 / o4-mini）。要支援 Codex 需要在 `codebus_core` 引入 provider 抽象層。

Azure OpenAI 是 Codex 的 enterprise deployment variant（相同 binary，不同 endpoint + auth config）。

## Proposed fix

新提一條 change：`v3-multi-provider`

### AgentBackend trait

```rust
// codebus-core/src/agent/backend.rs（示意）
pub trait AgentBackend: Send + Sync {
    fn spawn(&self, opts: SpawnOpts) -> Result<AgentHandle>;
    fn event_schema(&self) -> EventSchema;
}

pub struct ClaudeBackend { /* 現有邏輯搬過來 */ }
pub struct CodexBackend  { endpoint: Option<Url>, model: CodexModel }
```

- `VerbEvent` 需要標準化（或 backend 各自 emit normalized event）
- `codebus-app` IPC 層不感知 backend 差異

### Codex CLI 差異點

| 面向 | Claude CLI | Codex CLI |
|------|-----------|-----------|
| Output format | `--output-format stream-json` | 不同 event schema（需查文件）|
| 工具白名單 | `--tools Read,Glob,...` | 不同 flag |
| Sandbox | `--disallow-tools` / cwd 隔離 | 不同機制 |
| Auth | `~/.claude` config | `OPENAI_API_KEY` env var |

### Azure variant

只需在 `CodexBackend` 加 `endpoint: Option<Url>` config 欄位，指向 Azure OpenAI endpoint。
Auth 改用 Azure AD token 或 API key，binary 相同。

### Tasks（粗估）

1. spec ADDED `multi-provider`：定義 `AgentBackend` 介面 + event normalization 規格
2. Spike：Codex CLI event schema 研究（確認 stream-json 等價格式）
3. `codebus-core/src/agent/backend/`：trait + ClaudeBackend 搬遷
4. `codebus-core/src/agent/backend/codex.rs`：CodexBackend 實作
5. Config schema 加 `agent.provider: claude | codex`、`agent.codex.endpoint: Option<Url>`
6. Settings UI 新增 provider 選擇（也可 CLI flag）
7. Integration test：兩個 backend 各自跑 smoke test

工程量：重（1 週以上；spike 結果影響估算）。

## Out of scope

- 同時跑多個 provider（v1 always single active provider）
- 非 CLI-based provider（直接打 REST API 不走 binary）— 另行評估
- MyCoder CLI 整合 — 獨立 backlog，共用本 change 的 AgentBackend 抽象

## 依賴

- 必須在 D `v3-app-chat-cmdk` archive 之後（IPC surface 穩定再動 backend 層）
- MyCoder backlog 依賴本 change 的 trait 定義

## 何時動

v3-app-polish-ship（F）之後，或 E + F archive 且確認有 Codex CLI 採用需求時。
先 spike Codex event schema，估算確定後再 propose。

## API 優化 backlog（2026-05-23）

### codex Azure 改用官方 v1 Responses 寫法（簡化,非急）

目前 `CodexBackend` 的 azure 組合是 **pre-v1 舊式**:`base_url=…/openai` + `query_params.api-version=…` + `env_http_headers.api-key=…`(api-key header)。**能動**(2026-05-23 實機 + CDP 在真實 app 多輪驗過),但官方文件(Microsoft Learn《Codex with Azure OpenAI》2026-05-13 更新)現在推薦更簡單的 **v1 Responses API**:

```toml
model_provider = "azure"
[model_providers.azure]
base_url = "https://<resource>.openai.azure.com/openai/v1"   # 多了 /v1
env_key  = "AZURE_OPENAI_API_KEY"                            # 單純 env var
wire_api = "responses"
# 不需要 query_params.api-version、不需要 env_http_headers.api-key
```

**優化方向**:`base_url` 收尾改 `/openai/v1`、丟掉 `api-version` 與自訂 `api-key` header、改用 `env_key`。要先實機驗 v1 端點對 harry 的 resource(`2026msf13`)是否可用 + 認證是否吃 `env_key`(Bearer)而非 api-key header。**Entra ID 官方仍不支援。** 來源:learn.microsoft.com/azure/foundry/openai/how-to/codex、developers.openai.com/codex/config-advanced、openai/codex#2024。

**狀態**:純優化(現行舊式可用),非阻塞。等下次碰 codex azure 時一併驗 v1 再切。
