# Design Discussion: multi-provider agent backend

**Date:** 2026-05-21
**Status:** 設計收斂，待 `/spectra-propose`
**Surfaced during:** `/spectra-discuss multi-provider`
**Owner:** harry
**Inputs:** `docs/2026-05-14-multi-provider-agent-backend-backlog.md`（codex 0.132.0 spike + seam 對映表）

---

## Go / no-go：Go

Driver 兩者皆成立：

1. **真實使用需求** —— 實際要用 codex / Azure OpenAI 跑 codebus。
2. **策略性 seam 驗證** —— 用合格 second-impl（codex）逼出乾淨的 provider 抽象，避免未來硬耦合 debt。

專案尚未 release，故視為**全新設計**：無遷移、無向後相容 shim、無 legacy schema 偵測。

**首要設計目標：未來加一家新 AI provider 要輕鬆。** 加 codex 只是第一個 second-impl；衡量設計好壞的主軸是「加下一家 provider 多痛」。

---

## 核心架構：窄腰 seam，provider 封裝所有差異

### 邊界原則

外殼（shell）對「安全 / 工具 / model / argv / stream 格式」**完全無知**。每家 provider 的安全姿態、要提供哪些 tool、能不能隔離 MCP —— 全是該 provider 自己的內部行為。

```
┌─ 外層：provider-agnostic 共用骨架 ───────────────────────┐
│  只知道：active_provider 路由、SpawnSpec(意圖)、           │
│          vault root、session id                          │
│  擁有：  vault 前置、RunLog、event fan-out、              │
│          spawn / stream / cancel 迴圈、語意 marker 解析    │
│  不知道：tool、sandbox、MCP、安全、model、argv、stream格式  │
└───────────────────────┬─────────────────────────────────┘
                        │ SpawnSpec  (inward)
                        │ normalized StreamEvent + TokenUsage  (outward)
                        ▼
┌─ AgentBackend（一家 provider = 一個模組 = 加 provider 的全部成本）┐
│  自己的 config schema（per-verb model / sandbox / ...）       │
│  build_command(spec) → 決定 tool / sandbox / argv             │
│  自己的安全姿態（含 MCP：能隔離就隔離，不能就照跑 + 警告）       │
│  parse_stream_line（只管格式 JSONL → StreamEvent）             │
│  extract_session_id                                          │
└──────────────────────────────────────────────────────────────┘
```

### 外殼契約 = 兩個方向，兩件事

| 方向 | 是什麼 | 內容 |
|---|---|---|
| **外殼 → provider** | trait method 集合（要提供哪些 function） | `build_command(spec)`、`parse_stream_line(line)`、`extract_session_id(line)` |
| **provider → 外殼** | 回吐的資料形狀（response 長怎樣） | normalized `StreamEvent` + `TokenUsage` |

```rust
trait AgentBackend {
    fn build_command(&self, spec: SpawnSpec) -> Command;
    fn parse_stream_line(&self, line: &str) -> Vec<StreamEvent>; // 只管格式：JSONL → StreamEvent
    fn extract_session_id(&self, line: &str) -> Option<String>;  // codex thread.started / claude system.init
}
```

**這個 trait 定義本身就是「加一家 provider 的待辦清單」。** 實作這三個 method、吐出規定的 response 形狀，新 provider 就接上，外殼一行不改。

### 唯一不可下放的硬契約

**`StreamEvent` enum + `TokenUsage` 回吐** —— 這是所有 provider 的共同語言，也是 UI / RunLog 的唯一輸入。此形狀已在 `v3-run-log-events` normalized 過，本 change 只是把它正式升格為「provider 契約」。

- 分析：`StreamEvent`（Thought / ToolUse / ToolResult / Usage）夠泛用，codex 事件已驗證可乾淨對映。未來若有 provider 吐出塞不進的東西 → 擴 enum（共同演進），不是下放。UI 必須有單一輸入語言才能運作，此條沒得讓。

---

## 餵 backend 的單位是「一次 spawn」(SpawnSpec)，不是 verb

**現實校準（review 2026-05-21 發現）**：一個 verb 不等於一次 spawn。`verb/quiz.rs` 的 quiz = **plan spawn → 確認 gate → generate spawn → content-verify 迴圈 spawn**，最多 3 種 spawn，各自不同 toolset / model：

| spawn | permission | 特殊 |
|---|---|---|
| quiz-plan | read-only | — |
| quiz-generate | read-only + Bash | 細粒度白名單 `Bash(codebus quiz validate *)` |
| content-verify | read-only | `Verb::Verify`（resolve 到 verify model/effort） |

故「1 VerbKind = 1 invocation」是錯的。中性單位是**單次 spawn 的意圖** `SpawnSpec`：verb 層決定每個 spawn 的意圖（provider-無關），backend 決定怎麼用自己的 CLI 實現。

```rust
struct SpawnSpec {
    verb: Verb,                        // 既有 enum: Goal|Query|Fix|Chat|Quiz|Verify
                                       //   - 決定 skill：Claude 組成 "/codebus-{verb}"，codex 自己決定怎麼遞 prompt
                                       //   - 決定 model：backend 用 verb 經「自己的」config resolve(verb) 解出 model/effort/sandbox
    input: String,                     // 使用者輸入（topic / goal 文字）
    permission: Permission,            // ReadOnly | Workspace —— 不可由 verb 推導（見下）
    command_allowance: Option<CommandPrefix>, // 中性「指令前綴」，例: ["codebus","quiz","validate"]；Claude→Bash(... *)、codex→盡力/警告
    resume_session_id: Option<String>,
}
```

- **重用既有 `Verb` enum，不另立 `role`/`SpawnRole`（review 2026-05-21 發現）**：`config/claude_code.rs` 已有 `Verb`(Goal|Query|Fix|Chat|Quiz|Verify) + `resolve(Verb)`，後者已編碼 `Chat→query`、`Quiz→query`、`Verify→verify` 的 model 對映。早期 sketch 的 `role: SpawnRole` 是冗餘第二軸（且漏 Quiz、誤把 Verify 當 role）。model 解析 = backend 對自己的 config 跑等價 `resolve(verb)`。
- **`permission` 為何不可由 verb 推導**：quiz-plan 與 quiz-generate **同 `Verb::Quiz` 但不同 permission**（plan=ReadOnly、generate=ReadOnly+Bash）。故 permission / command_allowance / resume 都是 per-spawn 欄位，不是 per-verb。
- **多 spawn 的 verb 用既有 enum 即涵蓋**：quiz flow = spawn(`Verb::Quiz`,plan) → spawn(`Verb::Quiz`,generate) → spawn(`Verb::Verify`,content-verify)。
- 為何不用 per-verb method（`run_goal`/`run_quiz`）：那樣加一個**新 verb** 就要動**每一家 provider**，反而讓擴充變痛。`SpawnSpec` 是資料，加 verb 不動 backend。

### SpawnSpec 欄位一律中性，不洩 Claude 專屬編碼（review 2026-05-21 發現）

外層既然 provider-無關，`SpawnSpec` 就不能塞 Claude 形狀的字串，否則加第三家 provider 時外層還得反推 Claude 語法：

- **不用 `slash_command: String`**（= `/codebus-quiz "..."`，Claude `-p` 叫用形式）→ 改 `verb + input`。Claude backend 組成 `/codebus-{verb} "input"`；codex backend 自己決定怎麼遞（見開放項：codex slash 叫用未驗證）。
- **`command_allowance` 用中性前綴**（非 `"codebus quiz validate *"` 這種 Claude `--allowedTools` glob 語法）。Claude backend 格式化成 `Bash(codebus quiz validate *)`；codex backend 盡力對映 / 不能對映時警告。

### 避免把 codebus 語意 marker 解析下放（review 發現）

codebase 有**兩種**解析，必須分層：

| 解析種類 | 例子 | 該放哪 |
|---|---|---|
| **provider stream 格式** | JSONL line → `StreamEvent`（`parse_claude_stream_line` vs `parse_codex_stream_line`） | **provider 內**（`parse_stream_line`） |
| **codebus 語意 marker** | `StreamEvent::Thought` text → `[CODEBUS_QUIZ_SCOPE]` / `[CODEBUS_PROMOTE_SUGGESTION]` | **共用 verb 層**（不可下放） |

`[CODEBUS_*]` marker 是 codebus 與自己 SKILL prompt 的協定；SKILL 雙寫、兩家內容相同 → marker 解析是 **provider-無關** 的，跟 chat/quiz verb 綁定。

→ 故 trait **不含** `parse_result`（早期 sketch 的這個 method 是設計失誤，會誘導把 marker 解析錯誤塞進 provider）。provider 只負責格式層 `parse_stream_line`。

---

## Capability 哲學：缺能力不出局

**不設 hard gate。** 即使某 provider 缺某能力（含 MCP 隔離這種安全項），仍要讓它跑。處理方式是 provider 內部盡力 + 缺口處 surface warning，而非外層阻擋。

- 範例（MCP）：若 codex 無法隔離 ambient MCP，跑起來時環境裡的 ambient MCP 工具會洩進 spawn。處理 = codex backend 開跑前印清楚 warning，由 harry 知情下決定，codebus 不替他擋掉。
- **範例（細粒度指令白名單，review 發現）**：quiz-generate 靠 `Bash(codebus quiz validate *)` 把 agent 的 Bash **硬限制在只能跑這一條**（Claude `--allowedTools` 細粒度能力）。codex 只有 `--sandbox` 三級、**無逐指令白名單** → codex 的 quiz-generate agent 會有比 Claude 寬的指令存取。依本哲學照跑 + warning，但這是**有安全意涵**的具名 capability 落差，`SpawnSpec.command_allowance` 由 codex backend 盡力對映、不能完全對映時警告。
- 安全是 **provider 的內部責任**，不是外層的 gate。

---

## Config schema：全新統一 provider 格式

外層信封一致、內層 verb 形狀 per-provider（因功能不對等）。

```yaml
agent:
  active_provider: claude          # claude | codex
  providers:
    claude:
      active_endpoint: system      # system | azure（azure = Anthropic-on-Azure，既有語意）
      system:
        goal:  { model: opus-4-7,  effort: high }
        query: { model: haiku-4-5, effort: low }
        # fix / verify ...
      azure:
        base_url: https://<res>.cognitiveservices.azure.com/anthropic
        keyring_service: codebus-azure
        # verb 設定 ...
    codex:
      active_endpoint: system      # system | azure（azure = OpenAI-on-Azure）
      system:                       # api.openai.com
        goal:  { model: <m>, sandbox: workspace-write }
        query: { model: <m>, sandbox: read-only }
      azure:
        base_url: https://<res>.openai.azure.com/...
        keyring_service: codebus-codex-azure
```

正交兩軸：

```
              ┌─ provider: claude ─┬─ endpoint: system (api.anthropic.com)
  agent ──────┤                    └─ endpoint: azure  (Anthropic-on-Azure)
              └─ provider: codex  ─┬─ endpoint: system (api.openai.com)
                                   └─ endpoint: azure  (OpenAI-on-Azure)
```

- `provider` 選 binary、`endpoint` 選 base_url + auth。OpenAI-on-Azure 是 codex provider 底下的 azure endpoint，**不是**頂層 provider。
- 內層 per-verb 設定**不共用** struct：claude verb = `{model, effort}`，codex verb = `{model, sandbox, ...}`。
- **刪除既有 `claude_code` schema + `ParseOutcome::Legacy` 偵測**（`config/endpoint.rs`），無遷移、無 compat。

---

## 共用骨架留外層（不下放）

RunLog / vault 前置 / 編排 / spawn-cancel 迴圈留在外層共用骨架。

- 分析：RunLog / vault 是 codebus 自己的 provider-無關概念；編排裡唯一 provider-specific 的部分（config slice、怎麼 spawn）早已委派 backend。留骨架不卡任何 provider，且若下放會逼每家 provider 重抄 RunLog → 違背擴充性。

`invoke()`（`agent/claude_cli.rs:112`）的 spawn / stdio / cancel / accumulate 迴圈本就 provider-agnostic，只剩 3 個 Claude-專屬呼叫點（`build_claude_cmd` / `sniff_init_session_id` / `parse_claude_stream_line`）—— 這 3 點正是抽進 trait 的內容，迴圈改吃 `&dyn AgentBackend`。

---

## 工作切片（propose 時細化）

1. 定義 `AgentBackend` trait（`build_command` / `parse_stream_line` / `extract_session_id`）+ `SpawnSpec`（重用既有 `Verb`，不立 `SpawnRole`）/ `Permission` / `CommandPrefix`（外殼契約）；各 backend 對自己 config 跑等價 `resolve(verb)`
2. verb 層改造：把各 spawn 的意圖組成 `SpawnSpec`（quiz 三 spawn 各一個），marker 解析**保留**在 verb 層
3. `ClaudeBackend`：把現有 `claude_cli.rs` 的 argv 組裝 / `parse_claude_stream_line` / session sniff 搬進去；`verb`+`input` → `-p /codebus-{verb} "input"`、`permission`+`command_allowance` → `--tools`/`--allowedTools`/`--permission-mode`
4. `CodexBackend`：`codex exec --json` argv + `parse_codex_stream_line` + 自己的安全姿態（含 MCP 探查 task）；prompt 遞送方式待 slash-叫用探查決定、`permission` → `--sandbox` 級別、`command_allowance` 盡力對映 + 不能對映時警告
5. 全新統一 config schema（刪 `claude_code` + legacy 偵測）
6. `agent::invoke` routing（依 active_provider 選 backend）
7. skill bundle 雙寫 `.codebus/.codex/skills/`（格式相同）
8. 兩 backend 各自 smoke test

### 從「gate」降級為「task」的項目

- **codex MCP 隔離探查**：原為 propose 前置阻塞，現降級為 codex backend 內部 task —— 因為「能不能隔離 MCP」是 codex backend 的內部行為，外層不過問。apply 時邊做邊查 codex 的 MCP 控制旗標。

---

## 開放項（留給 propose / apply）

- **codex slash-command 叫用是否可行**（codex backend 內部 task，需本機跑 codex CLI）：backlog spike 只跑過 raw prompt（`codex exec "list files..."`），**從未驗證 `codex exec "/codebus-quiz ..."` 能否按 slash 名叫起 skill**。「skill 檔放在 `~/.codex/skills/` 格式相同」≠「exec 解析 slash 叫用」。若不行，codex backend 的 prompt 遞送機制需另設（inline skill / 別的叫法）—— 這正是 `SpawnSpec` 用 `verb+input` 而非 `slash_command` 的原因。與 MCP 同等級的 codex 未知。
- codex 的 MCP 控制旗標實測（codex backend 內部 task，需本機跑 codex CLI）
- codex `--output-schema` 等獨門能力：**先不進 trait**（YAGNI），等 codebus 有 quiz/goal-verify 真要用時再長
- MyCoder CLI（第三家）：本 trait 長出來後再評估（獨立 backlog `docs/2026-05-14-mycoder-cli-backlog.md`）
