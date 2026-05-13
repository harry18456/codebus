# Chat verb — multi-turn 模式討論

> 2026-05-13 roadmap-level 討論紀錄（pre-formal-discuss）。觸發於 `v3-run-log-events` propose 前，user 提出 query 一次性問答不符實際使用，想做 multi-turn chat + agent 提示 promote 成 goal。
>
> 上游：`docs/2026-05-11-app-ux-flow-design.md` §4.6（原 Cmd+K 「soft single-shot」設計 — 本次討論翻轉）、`docs/v3-app-roadmap.md` §Sequence、`docs/2026-05-13-v3-run-log-events-discussion.md`（B change 跟本討論正交不衝突）。

## TL;DR

**新插一條 change `v3-chat-verb`，CLI-first，排在 B 與 C 之間**。chat 是新 verb（不改 query），multi-turn 但全程 read-only sandbox。Promote 走**單一 AI-initiated 路徑**：chat agent 判斷對話內容值得寫 wiki 時 emit 結構化 suggestion，CLI/GUI 顯示 `[suggest] promote? (y/n)` 由 user 一鍵確認 → CLI 抓 transcript 重 spawn `codebus goal`（新 sandbox 拿到 Write/Edit）；goal verb 內部結尾自動接 lint+fix step（chat-verb 內 retroactive 擴展 A `v3-goal-library`）。User 想強制 promote 時不打指令、直接跟 AI 自然語言講「幫我把這段寫成 wiki」即可，SKILL.md 引導 agent 收到這類 user request 就 emit suggestion。

**Interactive UX 兩條核心要求**（2026-05-13 post-spike alignment）：

1. **Activity stream 視覺化** — chat 期間 user 要看得到 agent thinking + tool_use（`→ Read raw/code/auth.py` 之類），跟 CLI goal/query 既有行為一致。靠 `run_chat_turn` 的 `on_event` callback emit（A 已建好的 channel），CLI / GUI 各自 render。
2. **中斷後同 session 補充** — turn N 跑到一半 user 發現問錯，能 Ctrl+C 中斷、然後在**同一 session** 補新 prompt（保留 turn 1..N-1 transcript，不開新 session）。Cancel 機制 reuse A 的 `cancel: Option<Arc<AtomicBool>>`；但「中斷後 `--resume` 進 turn N+1 conversation history 是否乾淨」是新 spike（spike ❻）。

**B `v3-run-log-events` 不推遲** — 兩者正交；B 先做完 chat 才在它的 schema 上加一個 `session_id: Option<String>` optional 欄位。

Roadmap：7 條 → 8 條。D `v3-app-query-cmdk` 改名 `v3-app-chat-cmdk`、scope 重寫成 multi-turn overlay + Promote 按鈕。

## 觸發點

User push back §4.6.3「Soft single-shot mode」：「一次問答不太符合實際使用情況。我想要在同一個 session 持續問答，然後可以做到 agentic AI 會問要不要把內容更新到 goal 之類的行為。」

§4.6.3 原文：「Submitting a new question discards the current answer and starts a fresh agent run (no conversation memory)」— 當時為了 scope 控制走的捷徑，現在 user 認為應該翻轉。

## Claude CLI 三項硬限制（不可繞）

| 限制 | 影響 |
|---|---|
| **Sandbox spawn-time hard gate** — `--tools` / `--allowedTools` 是 spawn 時鎖死的 hard gate（v2 iter-9 spike verified） | 一個 claude process 跑到一半拿不到新 tool。**「同 session agent 從聊天切到改 wiki」做不到**，必須是兩個 spawn |
| **Session 持久化在 `~/.claude/projects/`** — `claude -p --continue` 仍 spawn 新 process，conversation history 從 disk 讀回 | 每輪 chat 都是獨立 spawn / 獨立 RunLog entry |
| **SKILL.md 是 conversation-level via user-message inject**（spike ❶❻ 已驗，2026-05-13） | `/codebus-chat "..."` 在第一輪 claude CLI 把整份 SKILL.md 作為 `type:user` message 塞進 conversation history（session jsonl line 6 證實，**不是 system prompt**）；後續輪 `--resume <id>` 沿用同份 history → SKILL 一路在 context |

底線：**chat 必須是新 verb，跟 query / goal 並列**，不能在內部 mode-switch。

## 三種架構 pattern 比較

| Pattern | 設計 | 評價 |
|---|---|---|
| **A** | 同 session agent 自主切 mode | ❌ **駁回**。sandbox spawn-time 鎖死 |
| **B** | read-only chat + 按鈕 promote 成 goal | ✅ 可行，採用做為 promote UX 形式 |
| **C** | 新 verb `codebus chat`，跟 query 並存 | ✅ **採用**。語意清楚、CLI script 不破、Claude `--continue` 完美匹配 |

最終設計是 **C 的 verb 切分 + B 的 promote UX**：新 verb `codebus chat`，promote 走 AI suggest + user 一鍵確認的 button-style flow（單一路徑，沒有 user CLI 指令）。

Pattern C 採用理由：

1. **Scope 可控** — 不動 query / goal / fix verb 任一邏輯（goal.rs:17 step 9 既有 fix loop 已足夠 — chat 從 promote 流 spawn `codebus goal "..."` 子進程 default flags 跑 lint+fix）；既有 27+ integration test 0 修改
2. **語意清楚** — query「我趕時間給我一句」；chat「我要對話探索 + AI 覺得值得寫就提議」；goal「把這個寫進 wiki + 自動 lint+fix」— 三個動詞三種 mental model
3. **Promote 機制乾淨** — chat agent emit suggestion → user `(y/n)` 確認 → CLI 抓 transcript → spawn goal（agent 不需內部 mode switch；sandbox spawn-time gate 保留）
4. **Claude `--continue` 完美匹配** — 每輪獨立 spawn / 獨立 RunLog，conversation history 由 claude cache 帶

## REPL 該誰擁有？— Library per-turn，caller 跑 loop

```
方案 C1：library 內部跑 REPL          方案 C2：library per-turn，caller 跑 loop
─────────────────────────             ──────────────────────────────────────
run_chat(on_event, on_prompt)         run_chat_turn(prompt, session_id) -> session_id
  library 內部 stdin / IPC 接 user      caller (CLI / GUI) 控 REPL UX

❌ CLI stdin 跟 Tauri event 完全異質    ✓ Library stateless about session structure
   逼 library 知道 caller 型態           ✓ session_id 只是 String 在 layer 間傳
                                       ✓ 每 turn 獨立 spawn = 既有 invoke() reuse
                                       ✓ 跟既有 verb library pattern 一致
```

**選 C2**。

```rust
pub struct ChatTurnOptions {
    pub text: String,
    pub session_id: Option<String>,  // None = 新 session 第一輪
}

pub struct ChatTurnReport {
    pub accumulated_tokens: TokenUsage,
    pub started_at: String,
    pub finished_at: String,
    pub agent_exit_code: Option<i32>,
    pub session_id: String,           // claude 給的；caller 帶到下一輪
}

pub fn run_chat_turn(
    repo: &Path,
    options: ChatTurnOptions,
    on_event: impl FnMut(VerbEvent),
    cancel: Option<Arc<AtomicBool>>,
) -> Result<ChatTurnReport, VerbError>
```

CLI `commands/chat.rs` 跑 REPL loop；GUI Cmd+K 端用同一 function。

## Promote-to-goal 機制

Promote 走**單一 AI-initiated 路徑**。User 在 REPL 全程只輸入自然語言；想 promote 時跟 AI 講（例：「幫我把這段寫成 wiki」），AI 收到後在回應中 emit 結構化 suggestion，CLI 偵測到後跳出 `(y/n)` 互動。**沒有 `/goal "..."` 之類的 user CLI 指令**。

```
chat session（read-only Read/Glob/Grep）                    goal spawn
─────────────────────────────────────                       （Read+Glob+Grep+Write+Edit；結尾自動 lint+fix）
turn 1: user「auth 怎麼運作」      ──┐                       ─────────────────────────────────────
turn 2: user「JWT 也講」             │ ← codebus-cli 累積 transcript
turn 3: user「幫我把這段寫成 wiki」  │   （SKILL.md 教 agent 收到 promote-request
        （或 AI 自主判斷時機）        │    user phrase → 下一輪 emit suggestion）
turn 4: agent 回應 emit：            │
        「[CODEBUS_PROMOTE_          │
         SUGGESTION] auth + JWT      │
         適合寫成 wiki」              │── CLI 偵測 → 印 [suggest] promote? (y/n)
                                     │   user 輸入 y → 用 agent 建議 reason 當 goal text
                                     ▼
                              format prompt：
                              「Based on this conversation:
                               ... 全部 user / assistant 對話 ...

                               Write: <agent suggestion 中的 reason>」
                                     │
                                     ▼
                              spawn `codebus goal "<formatted>"`
                              （新 spawn 拿到 Write/Edit toolset；
                                結尾 internal step 自動跑 lint+fix）
```

**設計選擇**：

1. **單一觸發路徑**（純 AI suggest）。User 全程自然語言，沒有 `/goal` 之類 CLI 指令。User 想強制 promote 時直接跟 AI 講，SKILL.md 教 agent 把這類 user phrase 當 promote-request 信號、下一輪 emit suggestion。
2. **AI 不 auto-fire**。Agent emit suggestion 後 user 必須 `(y/n)` 一鍵確認才 spawn goal — 沒有「AI 自己改 wiki」的執行路徑。
3. **訊號 schema 待 spike ❺**。圖中 `[CODEBUS_PROMOTE_SUGGESTION] <reason>` line marker 是 spike 草稿，真實 schema（line marker / JSON tail / synthetic tool_use）由 spike 結果決定。Spike ❺ 也要驗證「user 自然語言講要 promote」這條觸發路徑的 emit 穩定性 — 比 AI 自主判斷觸發更該穩定（畢竟 user 已經明示意圖）。
4. **Goal verb 結尾自動 lint+fix 是 0 retroactive**。`run_goal` step 9 已是 fix loop（goal.rs:17 module doc 明列），chat-verb 端 spawn `codebus goal "..."` 子進程走 default flags（`no_fix=false`）即可，**不動 `run_goal` 邏輯、不改 A archived 既有 test expectation**。
5. **Transcript 由 CLI 自己累積**（on_event 看到的 Stream 重建），不依賴 claude session 檔格式。Promote 是 codebus 層概念，claude 不知道；suggestion emission 是 SKILL.md prompt 層級的 convention，agent 配合產出而已。
6. **Escape hatch**：若 spike ❺ 結果 AI emission 不穩定（漏 emit / 錯時機），fallback 不是「加 `/goal` 指令」而是走 classifier 路線（每輪 chat 結束額外 spawn 輕量 classifier 判斷 promote-worthy），維持「對話框」mental model 不變。

**Transcript dump 格式（CLI 端組給 goal spawn 用）：**

```text
Based on this conversation:

<user>: auth 怎麼運作
<assistant>: ...（assistant 第一輪回答）...
<user>: JWT 也講
<assistant>: ...（assistant 第二輪回答）...
<user>: 幫我把這段寫成 wiki
<assistant>: [CODEBUS_PROMOTE_SUGGESTION] auth + JWT 適合寫成 wiki

Write: auth + JWT 適合寫成 wiki
```

- transcript 由 CLI 透過 `run_chat_turn` 的 `on_event` callback 累積（assistant text chunks + user inputs）
- `<user>:` / `<assistant>:` literal label 角色標記（不是 XML 避免衝突）
- 最後一段 `Write: <goal text>` 是 agent suggestion 中的 reason 字串
- 整個 string 當成 single prompt 餵給新 spawn 的 `codebus goal "<formatted>"`（新 spawn 拿到 Read/Glob/Grep/Write/Edit toolset）

**Quit alias**：建議 `exit` + `:q` + Ctrl+D 都接（CLI REPL 慣例都支援，cost 低；待 user confirm，列在 §待 confirm Q3）

## 為什麼 CLI-first

| 理由 | 細節 |
|---|---|
| v3 一貫 CLI-first | 10 主線 + foundation + A 都是這個 pattern；無理由打破 |
| Spike 風險只能 CLI 驗證 | 下節四個 spike 都跑 shell；GUI 假設 spike 過了才能信任 surface |
| SKILL.md 要 CLI 調過 GUI 才能消費 | 在 terminal 跑 chat 看 agent 行為 → 改 SKILL → 收斂 → GUI 包進 Cmd+K |
| CLI user 直接受惠 | dev workflow 在 terminal 跑 chat 也是真實 use case |

## Spike 必須先做完才能 propose

| Spike | 測試方法 | 失敗影響 |
|---|---|---|
| ❶ `claude -p --continue` 沿用第一輪的 SKILL / system prompt？ | 第一輪 `/codebus-chat "hello"`，第二輪 `--continue "are you still in codebus-chat mode?"` | 若不沿用：每輪要重 inject schema rules → SKILL.md 改 design |
| ❷ session_id 怎麼拿？ | spawn 後 grep stream-json stdout 找 `"session_id":"..."` | 若沒 emit：從 `~/.claude/projects/` 目錄掃最新檔取 id → 醜但可行 |
| ❸ `--resume <id>` + 三旗 sandbox 衝突？ | spawn `claude -p --resume <id> --tools Read,Glob,Grep --permission-mode acceptEdits` | 若 resume 失效：用 `--continue` 而非 `--resume`（限同 cwd 最近一個） |

❶ 失敗最痛，❷ 中等，❸ 影響 CLI flag 結構。**全部 1 小時 shell loop spike 可解**。

## Roadmap 插隊位置

| # | Change | 狀態 | 變動 |
|---|---|---|---|
| 1 | foundation | ✅ archived | — |
| A | v3-goal-library | ✅ archived | — |
| **B** | **v3-run-log-events** | **準備 propose** | **保留原計畫不動** |
| **NEW** | **v3-chat-verb** | **新插入** | CLI chat REPL + verb library + codebus-chat SKILL + RunLog `session_id` 欄位 |
| C | v3-app-workspace-goal | 未動 | 不變 |
| D | ~~v3-app-query-cmdk~~ → **v3-app-chat-cmdk** | 改名 + scope | 從「one-shot query overlay」改成「multi-turn chat overlay + Promote to goal 按鈕」；重寫 §4.6 |
| E | v3-app-quiz | 未動 | — |
| F | v3-app-polish-ship | 未動 | — |

**為什麼 B 先 / chat 在 B 之後**：

| B 先做的理由 | chat 在 B 之後的理由 |
|---|---|
| B scope 小、ship 快 | chat 需要 events.jsonl 做 promote 的 transcript 來源（雖然 CLI 端自己累積也行，但 GUI 端 reload 需要） |
| B 已 ship 完整 design / spec / discussion doc | chat 需要 `outcome=cancelled` 處理 mid-session abort |
| RunLog 加 `session_id: Option<String>` 是 chat 在 B 上**一個 optional 欄位**的擴展 | events.jsonl 設計 session-neutral，多 turn = 多檔，GUI 屆時用 session_id group |

**對 B 零 retroactive 改動。**

## v3-chat-verb 預估 deliverables

> 2026-05-13 post-spike + post-review 版本。Doc vs 實作 review 後縮減了既有 module 改動範圍（不動 `run_goal` 邏輯、不加 RunLog sub-record），但新增了 `agent::invoke` 修改 + CLI signal handler infra + `VerbLifecycleEvent` variant 三條漏寫的 deliverable。完整 review 過程見 §Doc vs 實作 review 段。

**Scope（post-alignment + post-review）：**

新加 module / file：

- `codebus_core::verb::chat::run_chat_turn` + `ChatTurnOptions { text, session_id: Option<String> }` / `ChatTurnReport { session_id: String, accumulated_tokens, started_at, finished_at, agent_exit_code }`（cancel signal reuse A 機制；**no `promote_suggestion` field** — 走 event channel）
- 新 SKILL bundle `codebus-chat/SKILL.md`（multi-turn read-only + promote-suggestion emission convention + 禁用 mcp/LSP tool；起點用 spike ❺ uv vault v0 草稿）
- `codebus-cli/src/commands/chat.rs` REPL — **非 thin wrapper**（既有 query/goal/fix CLI 都是 one-shot thin wrapper，chat 是第一個 REPL state machine + 多次 `run_chat_turn` 呼叫 + interactive prompt + spawn 子進程 goal 的 CLI）：
  - stdin loop + transcript accumulator + AI suggest 的 `(y/n)` inline 確認；全程自然語言、無 CLI 指令 parser
  - **Activity stream render**：`on_event` 收到 assistant thinking chunks / tool_use 時印簡短一行（例：`→ Read raw/code/auth.py`、`💭 thinking about JWT flow...`），不要 dump 完整 tool input/output
  - **Ctrl+C signal handler infra**（chat 是**第一個** CLI 端設 `cancel: Some(flag)` 的 verb；既有 verb CLI 都 `cancel: None`）：在 commands/chat.rs 內 register SIGINT trap（`signal-hook` 或 `ctrlc` crate）→ flip `AtomicBool`；第一次 Ctrl+C → 印 `[interrupted, drop the next message to continue this session]`；第二次 Ctrl+C（cancel 已 trigger 後）→ exit REPL
  - User 確認 promote 後 spawn `codebus goal "<formatted-transcript>"` 子進程（複用既有 goal 二進位，**不**透過 library call — 避免 chat REPL 阻塞）

對既有 module 的改動（**最小化**）：

- `codebus_core::agent::claude_cli` — `InvokeAgentOptions` 加 `resume_session_id: Option<String>`；`invoke()` 內建 cmd 時 `if let Some(id) = ... { cmd.arg("--resume").arg(id); }`（this 改動會被 query/goal/fix 三個既有 verb 自動繼承，但他們都 pass `None`，所以行為 byte-equivalent）
- `codebus_core::verb::event::VerbLifecycleEvent` — 加 `PromoteSuggestion { reason: String }` variant（既有 module doc 明示「MAY be extended」）
- `codebus_core::verb::error::VerbError` — error.rs:11 註解 update（「Cancelled SHALL NOT occur on CLI paths」改成「except chat-verb CLI which exposes Ctrl+C」）
- `codebus_core::log::sink::RunLog` — 加 `session_id: Option<String>`（serde default None、向後兼容）；**不加** lint_result / fix_result sub-record（既有 `lint_error_count` / `lint_warn_count` 扁平欄位已足夠）
- **不動** `codebus_core::verb::goal::run_goal`（goal.rs:17 step 9 已是 fix loop；chat 從 promote 流走 `codebus goal "..."` 子進程 default flags 即可，0 retroactive）
- **不改** A 留下的 27+ goal integration test expectation（goal 行為不變）

Integration tests via mock_claude：

- chat per-turn spawn + session_id passthrough
- promote suggestion event parsing (`[CODEBUS_PROMOTE_SUGGESTION] ...` line marker)
- mid-turn cancel + resume（**包括 AtomicBool cancel signal path**，不只外部 kill — spike ❻ 只驗了 timeout-kill path）
- 子進程 promote-to-goal spawn 整合測試

**Not in scope:**

- AI **auto-fire** promote（不經 user 確認直接 spawn goal — explicit ruled out）
- GUI 任何東西（D 的 scope）
- Cross-vault chat
- Chat 歷史搜尋 / 列表 / 刪除（claude 內建有 `/resume` picker，第一輪先靠他）
- Promote suggestion `(y/n/edit)` 中 edit mode（v1 只接 y/n）
- Classifier fallback（spike ❺ PASS、emit signal 走通就不需要）

## 動工順序

```
今天         → /spectra-propose v3-run-log-events        ← B 照原計畫
之後         → spike day（1 小時 ~ 半天）四個 spike       ← chat 動工前驗證
spike 後     → /spectra-discuss v3-chat-verb              ← 帶 spike 結果進 discuss
discuss 後   → /spectra-propose v3-chat-verb              ← chat 正式 propose
propose 後   → /spectra-apply v3-chat-verb                ← CLI REPL + verb lib + SKILL
chat ship 後 → C / D / E / F                              ← D 重寫 §4.6
```

## 結論

| 問題 | 答案 |
|---|---|
| chat mode 怎麼做？ | 新 verb，per-turn library + caller-owned REPL；spawn-time sandbox 鎖 read-only；promote 走單一 AI suggest + user `(y/n)` 確認路徑（user 全程自然語言、無 CLI 指令）；user 確認後 CLI **spawn `codebus goal` 子進程**（不透過 library call）；goal 內既有 fix loop 自動跑（0 retroactive）；turn 進行中 CLI 印 activity stream（assistant thinking + tool_use 一行 summary）；Ctrl+C 中斷後同 session `--resume` 接 turn N+1（claude CLI 內建 handshake、library 0 額外工作） |
| B 推遲嗎？ | **不推遲**。B 跟 chat 正交；先做 B 解 GUI run-log 基建 |
| CLI 先做嗎？ | **是**。spike 驗證 → CLI REPL → SKILL.md 調整 → 之後 GUI 拿成熟的 surface |
| roadmap 怎麼變？ | 7 條 → 8 條，chat 插在 B 跟 C 中間；D 改名 + scope 多 turn |
| 動工順序？ | 今天 propose B → spike day（❶❷❸❺❻ done；❹ superseded）→ discuss chat → propose chat → apply chat → C → D → E → F |

## Side note — design doc §4.6 要改

§4.6.3「Soft single-shot mode」段落整個翻轉。等 chat-verb apply 完、D 開 propose 時，由 D 的 propose / design 同步改寫 design doc §4.6。本討論不直接改 §4.6（避免改動 D 動工前就漂移）。

## 待 confirm

> 2026-05-13 post-spike update：Q1（順序）持續 hold；新加 Q4–Q6 反映 user 對齊（見 §Scope re-alignment）；Q2 / Q3 沿用。

1. 上面整套順序（B → spike → chat → C → D ...）OK 嗎？
2. chat verb name `chat` 沿用，還是改別的（`talk` / `ask` / `qa`）？
3. CLI 有沒有 quit alias（`exit` / `:q` / Ctrl+D）偏好？
4. ~~AI-initiated promote 走「emit signal」or「classifier」？~~ **Resolved (spike ❺ PASS)**：走 emit signal `[CODEBUS_PROMOTE_SUGGESTION] <reason>` line marker。Classifier 從 scope 拿掉、只 spec note 留 fallback option。
5. Goal-internal lint+fix 失敗時：rollback wiki write、還是留下未 lint 的 wiki page + RunLog 標 `outcome=lint_failed`？
6. Activity stream render 詳細度：CLI 端只印 `→ Read <path>` / `→ Grep <pattern>` 之類 tool 摘要 + thinking 不印；還是 thinking 也要印幾行？
7. ~~中斷後接續：fork-session vs SKILL rescue？~~ **Resolved (spike ❻ PASS)**：兩個都不用，claude CLI 內建 handshake 協議（自動 inject `"Continue from where you left off."` + agent emit `"No response requested."` 收尾）。Library 只要 `cancel` signal + `--resume <id>` 即可。

## Spike results (2026-05-13)

四個 spike 全跑完。三個 shell spike 在 `D:/side_project/uv/.codebus` 跑，第四個是設計決策。Claude CLI 版本 `2.1.140`。Session id 全程 `fab54091-9e72-4a26-86ac-9dbe200ab50c` 三輪同一個。

### ❶ `claude -p --continue` 沿用第一輪 SKILL/system prompt？— **PASS**

- Turn 1：`/codebus-query "..."` spawn 帶 `--tools Read,Glob,Grep --allowedTools Read,Glob,Grep --permission-mode acceptEdits`
- Turn 2：純 `--continue` spawn（**沒帶** 任何 sandbox flag）「Are you still operating under the codebus-query SKILL? Quote one rule verbatim.」
- Agent 回應引用 SKILL.md 內 verbatim 文字：`"This workflow is strictly read-only. The agent MUST NOT use Write or Edit to mutate any file inside wiki/, raw/, or anywhere else inside the vault."`（對應 `D:/side_project/uv/.claude/skills/codebus-query/SKILL.md` §Read-Only Invariant 段）

**結論：SKILL.md 是 conversation-level 持久化，`--continue` 沿用 conversation history 包含首輪 skill 載入結果。後續輪不必重複 `/codebus-chat` slash command 觸發。**

**意外發現（影響 propose 階段最大的點）**：Turn 2 沒帶 sandbox flag 結果 init event 顯示 `tools` 包含完整 toolset（Bash, Edit, Write, NotebookEdit, ToolSearch, Skill, Task ...），`permissionMode` 是 `"default"`。也就是：

> **`--continue` resume conversation history，但完全不沿用前一輪的 spawn flags。每次 spawn 都要重新 pass `--tools` / `--allowedTools` / `--permission-mode`，否則 sandbox 默認解鎖到完整 toolset。**

Implication：`run_chat_turn` library 在每輪 invoke claude 時必須嚴格傳遞所有三個 sandbox flag，不能省（chat-verb 必須在 library 層 enforce，避免 caller 忘記）。

SKILL prompt 層面的 read-only invariant 仍然在 agent context（Turn 2 agent 引文時自報 "I MUST NOT use Write"），但這只是 prompt 約束；binary-layer enforcement 必須 spawn-time flag 才有。chat-verb 設計上必須兩層同存。

### ❷ session_id 哪裡拿？— **PASS（多處可選）**

Stream-json stdout 每一個 event 都帶 `session_id`，最早出現在 hook 與 init event 第 1–3 行：

```jsonl
{"type":"system","subtype":"hook_started","...","session_id":"fab54091-9e72-4a26-86ac-9dbe200ab50c"}
{"type":"system","subtype":"hook_response","...","session_id":"fab54091-..."}
{"type":"system","subtype":"init","cwd":"D:\\side_project\\uv\\.codebus","session_id":"fab54091-...","tools":[...],...}
```

最後一筆 `{"type":"result",...,"session_id":"fab54091-..."}` 也有，是 spawn 結束後最穩固的取得點。

**Canonical recovery 策略（library 內 parse 順序）**：

1. 第一條 `type=system,subtype=init` 的 event → 取 `session_id` 欄位
2. 找不到 init（罕見錯誤路徑）→ fallback 最後一條 `type=result` event 的 `session_id`
3. 都沒有 → 才掃 `~/.claude/projects/<cwd-slug>/` 最新 `.jsonl` 取 stem。**不建議**：cwd-slug 命名規則 `D:\side_project\uv\.codebus → D--side-project-uv--codebus` 雖然 deterministic（drive 字母大寫 + 反斜線 → `--`），但官方無契約，未來可能改。

Implication：library `run_chat_turn` 內部接 stream-json 時，從 init event 抓 `session_id` 寫進 `ChatTurnReport.session_id`，是 1 行 jsonl parse 就能拿到，**沒有需要 fallback 掃檔的情境**。`docs/2026-05-13-chat-verb-discussion.md` §「REPL 該誰擁有」中 `ChatTurnReport.session_id: String` 不需改為 Option。

### ❸ `--resume <id>` + 三旗 sandbox 並存？— **PASS**

第三輪 spawn：`claude -p --resume fab54091-... "Now try to use the Write tool to create /tmp/should-not-exist.txt..." --tools Read,Glob,Grep --allowedTools Read,Glob,Grep --permission-mode acceptEdits`

Init event 顯示：

```
SESSION: fab54091-9e72-4a26-86ac-9dbe200ab50c  ← 沿用
CWD: D:\side_project\uv\.codebus
TOOLS: ['Glob', 'Grep', 'LSP', 'Read', mcp__... ]  ← 只剩 read-only set（外加 LSP 與 mcp）
PERM: acceptEdits  ← spawn 旗生效
```

Agent 回應：「I cannot use the Write tool because the codebus-query SKILL enforces a strictly read-only invariant ... and the binary-layer toolset was gated to `Read,Glob,Grep` at spawn time so Write is not even available to call.」

**結論：`--resume <id>` 跟 sandbox 三旗完全並存。Session conversation history 沿用，但每輪都重新鎖 toolset。`permission_denials` 是空陣列（沒嘗試 deny，因為 Write 根本不在 toolset）。**

額外觀察：

- 即使 `--tools Read,Glob,Grep`，init event tools 仍包含 `LSP` 與全部 `mcp__claude_ai_*` 系列（Figma / Gmail / Drive / Calendar 6 個 auth tool）。LSP 與 mcp 不在 `--tools` 白名單管制範圍。對 chat verb 無實質影響（mcp 都 `needs-auth` 沒 token），但 library API 文件應註明「sandbox 鎖的是 built-in toolset，LSP/MCP 另外管」。
- `--continue` 與 `--resume` 都導致同一個 `session_id` reuse — 兩者在 session-id 行為等價，差別只在 `--continue` = cwd 最近一個、`--resume <id>` = 顯式指定。chat verb 第 N 輪用 `--resume` 比較顯式可靠（CLI REPL 跨 vault 切換 cwd 後 `--continue` 會抓錯 session）。

### ❺ Chat agent 能否穩定 emit 結構化 promote-suggestion 訊號？— **PASS**

寫了 `codebus-chat/SKILL.md` v0 草稿（部署到 `D:/side_project/uv/.claude/skills/codebus-chat/` 與 mirror 到 `.codebus/.claude/skills/`），內容含：

- multi-turn read-only invariant + 禁用 Write/Edit/NotebookEdit/`mcp__*`
- Promote-suggestion emission rule：marker 格式 `[CODEBUS_PROMOTE_SUGGESTION] <reason 5-15 words>`，必須在 message 第一行第一字元、每 message 最多一次
- When to emit / when NOT to emit 各 3 條範例
- Format rules + Language Override（marker 永遠 literal English，reason follow user language）

跑 4 個 scenario 共 9 個 spawn turn：

| Scenario | 預期 | 實測 | 結果 |
|---|---|---|---|
| S1 — explicit user 「幫我把 uv-lib + uv-child 寫成 wiki」（3 turn） | T1/T2 不 emit，T3 emit | T1/T2 不 emit，**T3 emit**：`[CODEBUS_PROMOTE_SUGGESTION] uv-lib 與 uv-child 的關係與子進程處理` | ✅ |
| S2 — single factual lookup「which file defines main entry?」 | 不 emit | 不 emit（回答 `crates/uv/src/bin/uv.rs.`） | ✅ |
| S3 — natural consolidation（4 turn：cache architecture / clean / venv / summarize） | T1-T3 不 emit，T4 consolidation 時 emit | T1-T3 不 emit，**T4 emit**：`[CODEBUS_PROMOTE_SUGGESTION] uv cache lifecycle from bucket creation through venv reuse to cleanup` | ✅ |
| S4 — ambiguous「should this be saved?」（existing wiki page 已存在的 uv-auth-lib） | 不 emit，agent 指向現有 wiki page | 不 emit；agent 主動回應「`wiki/modules/uv-auth-lib.md` 已經存在且覆蓋完整，你只是單次查詢」 | ✅ |

**結論：4/4 scenario 行為符合 SKILL 規則，emit 格式 2/2 穩定（marker 永遠在 message 開頭、reason 簡短具體）。**

Format 細節觀察：

- Marker line 一致都是 `[CODEBUS_PROMOTE_SUGGESTION] <reason>` 後一個換行，然後接 markdown header / 列點等正文 — line marker 不混在內文裡，CLI parser 用 `line.starts_with("[CODEBUS_PROMOTE_SUGGESTION] ")` 抓得到
- Reason 內容**確實是 wiki page 主題層級**（不是 how-to-write），跟 SKILL 寫的「naming what the page would cover, not how to write it」吻合
- S4 agent 不僅沒誤觸發、還主動查 `wiki/` 找到 `uv-auth-lib.md` 引導 user — SKILL 的 "An existing wiki page already covers the topic — point the user there instead" 規則被遵守

Sample 大小：4 scenario 是 minimum viable。Propose 階段建議再跑 5-10 個 scenario 補 sample。Failure path（classifier fallback）這次 spike 沒觸發到，**可以從 chat-verb scope 拿掉**（只在 spec note 標「若量產發現 emit 不穩定再改 classifier」）。

### ❻ 中斷 turn N 後 `--resume` 進 turn N+1 conversation history 行為？— **PASS**

Spike 步驟：

1. Spawn turn N：long prompt「List all `wiki/modules/*.md` via Glob, then Read each one in order, summarize all 6」配 `timeout 6s` 強制中斷（exit 124 = timeout-killed）
2. 抓 session id `578be84f-4766-4f98-b23a-2ce107dae77c`，turn N 只完成 Glob、剛拿到 tool_result（152 chars list），尚未 Read 任何檔案就被砍
3. Spawn turn N+1：`--resume <id>` + 新 prompt「I interrupted you. Change of plan — just Read uv-lib.md and venv.md only, summarize each in one short sentence. Skip the other 4.」
4. Inspect `~/.claude/projects/D--side-project-uv--codebus/578be84f-....jsonl` 看 conversation history

**Turn N+1 行為**：agent 正確只 Read 兩個指定檔案（uv-lib.md + venv.md），summarize 兩句即停。**完全沒 continue 原本 6 檔案計畫**。

**Session jsonl 揭露的 claude CLI 內建 cancel/resume 協議**：

| Line | type | 內容 |
|---|---|---|
| 5 | user (`/codebus-chat ...`) | turn N 原始 prompt |
| 6 | user（SKILL 全文） | SKILL.md 整份是 user message inject，**不是 system prompt** |
| 9 | assistant tool_use Glob | turn N 第一個 tool 呼叫 |
| 10 | user tool_result（152 chars） | Glob 結果 |
| 13 | **user `{isMeta:true, text:"Continue from where you left off."}`** | **claude CLI 自動 inject 的 handshake** |
| 14 | **assistant text `"No response requested."`** | **agent 自動 emit 的閉合 response（收尾 turn N）** |
| 15 | user (turn N+1 prompt) | 真正的補充 prompt 「I interrupted you. Change of plan ...」 |
| 16-19 | assistant tool_use Read + tool_result × 2 | turn N+1 工作 |
| 20 | assistant text | turn N+1 答覆 |

**意義**：claude CLI **本身**有完整 mid-turn cancel handshake 協議：

1. Turn N 被殺後 session jsonl 保留 partial 狀態
2. 下個 `--resume <id>` spawn 時，CLI **自動 inject** `{type:user, isMeta:true, text:"Continue from where you left off."}` 給 agent
3. Agent emit 一個 minimal ack `"No response requested."` 把 turn N 閉合
4. 然後新的 user prompt（library 帶來的）才送進去處理
5. Agent 看 history 時 turn N 是「已被中斷、agent 已 acknowledge」狀態，不會試圖 continue

**對 chat-verb library 設計的 implication（強簡化）**：

- `cancel: Option<Arc<AtomicBool>>` 只負責 trigger spawn 收尾，**不需要** library 自己 inject 任何 "interrupted" 文字到 history
- Turn N+1 直接 `--resume <session_id>` + 新 prompt 就 work，**不需要** `--fork-session`
- **不需要** SKILL prompt-level rescue（claude CLI 已處理）
- 原 §「待 confirm」Q7（`--fork-session` vs SKILL rescue）→ **無需決策，兩個都不用**

額外觀察：

- SKILL.md 是 inject 在 user message 內（line 6），不是 system prompt。這跟 spike ❶ Turn 2 沒帶 `--tools` 但 SKILL 仍在 conversation context 的觀察一致 — SKILL 透過 user-message 路徑、跟 conversation history 同生命週期。
- Turn N+1 spawn 的 input 帶完整 partial turn N events（Glob tool_use + result + handshake + ack response），所以 token cost 比「fresh 第二輪」略高。對 v1 chat scope 不是問題，但 propose 階段 token budget 估算要納入。

### 對 propose 階段的彙整 implication

| Spike | 對 spec / design / tasks 的影響 |
|---|---|
| ❶ | `run_chat_turn` library API 每輪呼叫 claude 必須 hardcode 三旗 sandbox flag（不能讓 caller 傳）；SKILL.md 在 first turn 自動 inject 後不必每輪重 `/codebus-chat`；first turn vs N+1 turn 的 invoke path 差異點**僅在 `resume_session_id` 欄位**：first turn `None`、N+1 turn `Some(<previous session_id>)`；**強制走 `--resume <id>`，不用 `--continue`**（`--continue` cross-cwd 抓最近一個易出錯，REPL 切 vault 會撞 race） |
| ❷ | `ChatTurnReport.session_id: String` 非 Option（init event 必有）；library parse stream-json 找第一條 init event 的 `session_id`；不需要 `~/.claude/projects/` 掃檔 fallback 進 v1 scope |
| ❸ | `--resume <id>` 是 N+1 turn 的選擇而非 `--continue`（顯式可靠、跨 cwd 安全）；mcp / LSP 不被 `--tools` 管制這點寫進 SKILL.md notes（read-only 防線只能靠 prompt + binary toolset gate 雙層） |
| ❺ | promote-suggestion 走 line marker `[CODEBUS_PROMOTE_SUGGESTION] <reason>` 路線（emit signal）— spec 確定走 emit、不走 classifier；CLI/library 端 parse 用 `line.starts_with(...)`；SKILL 規則照 v0 草稿（uv vault 部署版）做 propose 起點；classifier 路線從 chat-verb scope 拿掉、只在 spec 留「若量產發現不穩定再改」note |
| ❻ | Cancel + resume 完全靠 `cancel: Option<Arc<AtomicBool>>` + `--resume <id>` 一條路；library / SKILL **不需**任何「interrupted」inject 邏輯（claude CLI 內建 handshake）；§待 confirm Q7（fork-session vs SKILL rescue）→ **答案：兩個都不用** |

### 殘留 risk / 開放問題

1. **❺ sample size 小**（4 scenario / 9 spawn）。Propose 階段建議再跑 5-10 個 scenario 確認 emission 在更多語境（多語言混用、長對話 10+ turn、邊界 promote-request phrasing）仍穩。但**不阻塞** propose / discuss，spike pass 即進 discuss。
2. **Token budget**：Spike turn 多輪後 `cache_creation_input_tokens` 上升明顯（spike turn1=25308 → turn2=44742 ≈ 翻倍），❻ resume 後 turn N+1 input 還帶 partial turn N events。建議 propose 階段把 `per-turn token usage` 放進 `ChatTurnReport` 給 CLI / GUI 顯示，方便 user 看 cost。
3. **MCP authenticate tools 在 toolset**：6 個 `mcp__claude_ai_*` 不在 `--tools` 管制；若 user 在 IDE 已 authenticate 過，這些 tool 可能 spawn 內可用。SKILL.md v0 已明確列為禁用，靠 prompt-level 防線。
4. **Activity stream UX**：spike ❺ stream-json 確認 `tool_use` event 帶 `name` + `input`（一行 summary render 資料齊全）；assistant `text` chunk 也可拿到 thinking-style 內容。但 v1 CLI render 詳細度（Q6）仍待 user 確認。

### Spike 環境細節

- Vault: `D:/side_project/uv/.codebus`
- SKILL bundle 來源:
  - codebus-query: `D:/side_project/uv/.claude/skills/codebus-query/SKILL.md`（spike ❶❷❸ 用）
  - codebus-chat v0 草稿: `D:/side_project/uv/.claude/skills/codebus-chat/SKILL.md` + mirror `.codebus/.claude/skills/codebus-chat/SKILL.md`（spike ❺❻ 用）
- Stream-json artifact：
  - 早期 spike: `docs/spike-artifacts/spike-turn{1,2,3}.jsonl`（spike ❶❷❸）
  - Spike ❺: `docs/spike-artifacts/spike5-s{1,2,3,4}-t{N}.jsonl`（9 個檔）
  - Spike ❻: `docs/spike-artifacts/spike6-t{1,2}.jsonl`
- Sessions：
  - 早期 spike: `fab54091-9e72-4a26-86ac-9dbe200ab50c`
  - S1 (uv-lib + uv-child): `6e2c2964-0534-4426-96da-c0c93e819bb7`
  - S2 (entry point): `0480eb8f-b5e6-4026-b90d-4536f0fc0571`
  - S3 (cache lifecycle): `031db89d-3ac7-40a6-ab40-2c109657fdfa`
  - S4 (uv-auth-lib): `1082c513-bdf7-40eb-952a-5117e35c25b6`
  - ❻ cancel/resume: `578be84f-4766-4f98-b23a-2ce107dae77c`
- Total spike cost：早期 ❶❷❸ ≈ $0.7、❺ 9 spawn ≈ $1-2、❻ 2 spawn ≈ $0.3，**合計 ≈ $2-3 USD**

## Doc vs 實作 review (2026-05-13 post-spike)

Spike day 結束、`/spectra-discuss v3-chat-verb` 進場時做的 cross-check。讀完 `codebus-core/src/verb/{event,error,mod,goal,query}.rs`、`codebus-core/src/agent/claude_cli.rs`、`codebus-core/src/log/sink.rs`、`codebus-cli/src/commands/goal.rs` 後比對 doc 內所有對既有實作的陳述。**10 條 finding** 已 inline 修正回前段，本段保留為 propose / apply 階段 actionable check list（避免漏項）。

### Critical（已動 §Scope re-alignment + §預估 deliverables）

| # | Finding | Source 證據 | Inline 修正位置 |
|---|---|---|---|
| **F1** | `InvokeAgentOptions` 沒有 `resume_session_id` 欄位，`invoke()` cmd builder 也沒 `--resume` arg — doc 整篇講「每輪用 `--resume <id>`」但漏寫修改 deliverable | `agent/claude_cli.rs:42-50` + `:106-128` | §scope 衝擊 (f) + §預估 deliverables「對既有 module 的改動」首條 |
| **F2** | `RunLog` 已有扁平 `lint_error_count`/`lint_warn_count` + `outcome` 欄位 — doc 寫的「`lint_result` / `fix_result` sub-record」是錯（重複資訊） | `log/sink.rs:51-82` | §scope 衝擊 (g) + §預估 deliverables「對既有 module 的改動」末條 |
| **F3** | Goal verb fix loop 在 `run_goal` step 9 **已存在**（goal.rs:17 module doc 明列）— doc 寫的「結尾追加 lint+fix step」+「27+ test expectation update」+「retroactive A extension」全部誇大 | `verb/goal.rs:17` + `GOAL_TOOLSET` 已含 Write/Edit (`:54`) + `GoalReport.lint_error_count`/`lint_warn_count` 已存在 (`:71`) | §scope 衝擊 (c) 改成 0 retroactive + §預估 deliverables「不動 `run_goal`」 + §結論「chat mode 怎麼做」 + §Roadmap 影響工作量 2x → 1.5x |
| **F4** | PromoteSuggestion event 應走 `VerbLifecycleEvent` 新 variant，不放 `ChatTurnReport` 欄位 — `event.rs:14-18` 註解明示「MAY be extended」；放 report 會逼 CLI 等 turn 完才能跳 `(y/n)`，bad UX | `verb/event.rs:14-18, 131-148` | §scope 衝擊 (a) commit event 路線 + §預估 deliverables 移除 `promote_suggestion` 欄位 |
| **F5** | CLI Ctrl+C signal handler infra 是 chat-verb 新加 — `error.rs:11` 註解明寫「Cancelled SHALL NOT occur on CLI paths (CLI passes cancel: None)」+ `commands/goal.rs:63` 證實既有 CLI 全 `None` — chat 是第一個 CLI 端 `cancel: Some(flag)` 的 verb，需要 signal-hook/ctrlc crate + 註解 update | `verb/error.rs:11` + `cli/commands/goal.rs:63` | §scope 衝擊 (e) 補 SIGINT trap 細節 + §預估 deliverables「Ctrl+C signal handler infra」段 + 待 apply 時 update `error.rs:11` 註解 |

### 中等（spec 內必須明寫）

| # | Finding | Inline 修正位置 |
|---|---|---|
| **F6** | chat library 強制走 `--resume <id>`，**不用 `--continue`**（cross-cwd race 風險）— doc §implication ❶ 措辭模糊 | §對 propose 階段彙整 implication ❶ hard-commit 已更新 |
| **F7** | SKILL inject 機制驗證 — 是 user-message inject 不是 system prompt（spike ❻ session jsonl line 6 證實）— doc §三項硬限制 row 3 仍寫「待 spike」 | §三項硬限制 row 3 已更新 |
| **F8** | CLI chat **不是 thin wrapper** — `verb/mod.rs:18-23` 描述既有 thin wrapper pattern 是 one-shot 結構；chat 是 REPL loop + 多次 `run_chat_turn` + interactive prompt + spawn 子進程 — doc 應明寫破例 | §預估 deliverables CLI REPL bullet 已明寫「非 thin wrapper」 |

### Minor（spec 註記 / apply 時注意）

| # | Finding | Inline 修正位置 |
|---|---|---|
| **F9** | Spike ❻ 只驗 timeout-kill path，AtomicBool flip path 沒覆蓋 — 兩種對 session jsonl 留下狀態可能不同；風險低但需 mock_claude integration test 補 cover | §預估 deliverables「Integration tests」段已加「包括 AtomicBool cancel signal path」 |
| **F10** | `--input-format stream-json` 限制 — prompt 必須走 `-p <arg>` 字串、不能透過 stdin；每輪是獨立 process — doc implicitly 假設但沒 cross-ref | apply 階段在 `verb/chat.rs` 模組註解 inline cross-reference `agent/claude_cli.rs:17` 註解 |

### 整體 review 結論

**最大簡化**：F2 + F3 兩條合起來 — chat-verb 對 codebus-core 既有 module 的改動從 doc 之前寫的「動 goal 邏輯 + 改 RunLog schema + 改 27+ test」縮成「改 `agent::invoke` 加 optional 欄位 + RunLog 加 optional 欄位 + 加一個 `VerbLifecycleEvent` variant + 改 `error.rs:11` 一條註解」，**整體既有 module 改動量小到可以 spec 一段表格列完**。chat 自己的新 module（verb/chat + commands/chat + SKILL bundle）才是大頭。

**最大補漏**：F1（`agent::invoke` 改動）是 doc 寫不出來、靠 review 才浮現的 critical 漏寫。沒這條，propose 進來會發現 chat verb 無法產生 `--resume <id>` arg、卡住。

**Doc 內部一致性**：本段加完後 doc 應該自上而下一致（§TL;DR / §三項硬限制 / §Promote-to-goal / §scope 衝擊 / §預估 deliverables / §結論 / §implication 全 sync）。若 propose 進場時發現任何 stale 描述、回頭追到本段 F# 編號可以快速 cross-reference。

## Scope re-alignment (2026-05-13 post-spike)

Spike 結果回報後 user 對 chat-verb 的 mental model 補了三句話：

> 「我的想像最終 app 有個對話框可以一直問問題，
> 然後 AI **覺得不錯可以更新至 wiki 的就會做**，
> 然後 **wiki 更新本身也會牽涉到 lint 跟 fix**」

對應確認三個 design point：

| # | Q | A (verbatim) |
|---|---|---|
| 1 | Promote 觸發語意 | **AI 提議、user 一鍵確認**（suggest + wait，不是 auto-fire） |
| 2 | Wiki 更新後 lint/fix 接點 | **Goal verb 內部自動跑 lint+fix**（不是 chat 端 chain 兩 spawn、不是 user 主動再點） |
| 3 | AI-initiated promote 的 scope 切點 | **塞進 v3-chat-verb scope**（不切獨立 change） |

### 對 chat-verb scope 的衝擊

原 §「v3-chat-verb 預估 deliverables」list 假設 user-initiated `/goal` 是唯一 promote 路徑、且沒明寫 activity stream / mid-turn interrupt / agent::invoke 修改 / CLI signal handler。Post-alignment + post-review 重整為 **七組 deliverable**：

| 新 deliverable | 動到 | 風險 |
|---|---|---|
| (a) chat agent 結構化 emit「promote suggestion」訊號 | `codebus-chat/SKILL.md`（新 bundle）；`run_chat_turn` event stream parsing；**`VerbLifecycleEvent::PromoteSuggestion { reason }` 新 variant**（非 `ChatTurnReport` 欄位 — stream 時即 emit，CLI 才能不等 turn 結束就跳 prompt） | Spike ❺ PASS（emit signal 訊號穩定）；訊號 schema 走 line marker `[CODEBUS_PROMOTE_SUGGESTION] <reason>` |
| (b) Promote suggestion → user confirm → spawn goal flow | `codebus-cli/src/commands/chat.rs` REPL 監聽 PromoteSuggestion event → 印 `[suggest] promote? (y/n)` 互動；user `y` → spawn `codebus goal "<transcript>"` 子進程（不透過 library call、避免 REPL 阻塞）；GUI 端 D 屆時對應 Cmd+K overlay button | CLI inline prompt 在 streaming 中插入互動的時序要小心；edit mode 排在後續 polish |
| (c) ~~Goal verb 內部 lint+fix step~~ → **0 retroactive**（review 修正） | **不動** `run_goal`：goal.rs:17 step 9 已是 fix loop，chat 從 promote 流走 `codebus goal "..."` 子進程 default flags（`no_fix=false`）即可；**A 留下的 27+ integration test expectation 不改**；goal 行為 byte-equivalent | — |
| (d) Activity stream render（CLI 端 v1）| `codebus-cli/src/commands/chat.rs` 在 `on_event` callback 內 render assistant thinking chunks + tool_use 一行 summary（不 dump 詳細 input/output）；reuse goal/query 既有 render helper 為佳 | render 太詳細會洗版；太簡略又看不到 tool — propose 階段定 render contract |
| (e) Mid-turn cancel + same-session resume | `codebus-cli/src/commands/chat.rs` 註冊 SIGINT trap → flip `AtomicBool` 給 `run_chat_turn`；下一輪 prompt 用 `--resume <session_id>` 進 turn N+1。**chat 是第一個 CLI 端 set cancel: Some(flag) 的 verb**（既有 verb CLI 都 `cancel: None`，見 `error.rs:11` 註解，註解要 update） | Spike ❻ PASS（claude CLI 內建 handshake）；但 spike 只驗 timeout-kill path，AtomicBool flip path 進 mock_claude test cover |
| (f) `agent::invoke` 加 resume 支援（**doc 漏寫，review 補**） | `InvokeAgentOptions` 加 `resume_session_id: Option<String>` 欄位；`invoke()` 內 cmd builder 加 `if let Some(id)=... { cmd.arg("--resume").arg(id); }`；query/goal/fix 三個既有 caller 自動 pass `None` → 行為 byte-equivalent | 改動 v3-foundation archived module，但純加 optional 欄位、既有測試不受影響 |
| (g) RunLog 加 chat session_id（**review 修正**） | `RunLog` 加 `session_id: Option<String>` 一欄（serde default `None`）；**不加** `lint_result` / `fix_result` sub-record（既有 `lint_error_count` / `lint_warn_count` 扁平欄位已足夠 — doc 之前寫錯） | 純擴展欄位、向後兼容 |

### 新增 Spike ❺ + ❻ — **都已 PASS（2026-05-13）**

| Spike | 結果 | 詳細結果段落 |
|---|---|---|
| ❺ chat agent 能否穩定 emit 結構化 promote-suggestion 訊號？ | ✅ PASS — 4/4 scenario 行為符合 SKILL 規則，emit 格式 2/2 穩定 | 見 §Spike results §❺ |
| ❻ 中斷 turn N 後 `--resume` 進 turn N+1 conversation history 行為？ | ✅ PASS — claude CLI 內建 cancel handshake 協議，library 完全不用 inject anything；resume 後 agent 完全不會 continue 舊計畫 | 見 §Spike results §❻ |

### Goal-internal lint+fix — **0 retroactive**（review F3 修正）

Doc 早期版本誤寫此項。Review (F3) 修正：`run_goal` step 9 已是 fix loop（`verb/goal.rs:17` module doc），`GOAL_TOOLSET` 已含 Write/Edit、`GoalReport.lint_error_count` / `lint_warn_count` 已存在。chat-verb 對 goal 的整體影響：

- **不動** `codebus_core::verb::goal::run_goal` 邏輯
- **不動** A 留下的 27+ integration test 的 expectation
- **不動** `RunLog` 的 lint 相關欄位（既有 `lint_error_count` / `lint_warn_count` 扁平欄位足夠 — review F2）
- chat-verb 端 spawn `codebus goal "<transcript>"` 子進程跑 default flags（`no_fix=false, force_resync=false`），既有 fix loop 自動跑
- 唯一動到 RunLog 的是加 `session_id: Option<String>` 一欄關聯 chat session

`codebus goal` CLI 行為 byte-equivalent invariant **不破**（A archived 假設保持）。

### Roadmap 影響

| # | Change | 原狀 | Post-alignment |
|---|---|---|---|
| chat | v3-chat-verb | 上面 §預估 deliverables 列 5 項 | + AI-initiated promote 三組 deliverable + spike ❺ + goal-library retroactive extension |
| D | v3-app-chat-cmdk | 「multi-turn overlay + Promote 按鈕」 | 加「AI suggest pill 按鈕」（GUI 端對應 emit signal 視覺呈現） |

**順序不變**（B → chat → C → D → E → F），只是 chat 這條變胖。預估 chat ship 從原本「CLI REPL + verb lib + SKILL + RunLog 加欄位」變成「+ promote suggestion mechanism + activity stream render + mid-turn cancel/resume + agent::invoke resume 支援 + CLI SIGINT signal infra」。Doc vs 實作 review 後 goal-internal lint+fix 改成 0 retroactive、RunLog sub-record 簡化掉，工作量約原本的 **1.5x**（review 前估 2x；F2/F3 修正後砍掉一段）。

> Scope / Not-in-scope 詳列見前段 §v3-chat-verb 預估 deliverables（已 post-alignment）。
