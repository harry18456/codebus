# Chat verb — multi-turn 模式討論

> 2026-05-13 roadmap-level 討論紀錄（pre-formal-discuss）。觸發於 `v3-run-log-events` propose 前，user 提出 query 一次性問答不符實際使用，想做 multi-turn chat + agent 提示 promote 成 goal。
>
> 上游：`docs/2026-05-11-app-ux-flow-design.md` §4.6（原 Cmd+K 「soft single-shot」設計 — 本次討論翻轉）、`docs/v3-app-roadmap.md` §Sequence、`docs/2026-05-13-v3-run-log-events-discussion.md`（B change 跟本討論正交不衝突）。

## TL;DR

**新插一條 change `v3-chat-verb`，CLI-first，排在 B 與 C 之間**。chat 是新 verb（不改 query），multi-turn 但全程 read-only sandbox；想寫 wiki 時 user 在 REPL 打 `/goal "..."` → CLI 抓 transcript 重 spawn `codebus goal`（新 sandbox 拿到 Write/Edit）。

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
| **SKILL.md 是 conversation-level**（待 spike 驗證） | `/codebus-query "..."` 在第一輪載 codebus-query/SKILL.md；後續輪 `--continue` 沿用同份 SKILL — 需 spike 確認 |

底線：**chat 必須是新 verb，跟 query / goal 並列**，不能在內部 mode-switch。

## 三種架構 pattern 比較

| Pattern | 設計 | 評價 |
|---|---|---|
| **A** | 同 session agent 自主切 mode | ❌ **駁回**。sandbox spawn-time 鎖死 |
| **B** | read-only chat + 按鈕 promote 成 goal | ✅ 可行 |
| **C** | 新 verb `codebus chat`，跟 query 並存 | ✅ **採用**。語意清楚、CLI script 不破、Claude `--continue` 完美匹配 |

Pattern C 採用理由：

1. **Scope 可控** — 不動 query / goal / fix，既有 27+ integration test 不需改
2. **語意清楚** — query「我趕時間給我一句」；chat「我要對話探索」；goal「把這個寫進 wiki」— 三個動詞三種 mental model
3. **Promote 機制乾淨** — chat 抓 transcript → spawn goal，不需 agent 內 mode switch
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

```
chat session（read-only Read/Glob/Grep）        goal spawn（write Read+Glob+Grep+Write+Edit）
─────────────────────────────────────            ──────────────────────────────────────────
turn 1: 「auth 怎麼運作」          ──┐
turn 2: 「JWT 也講」                │ ← codebus-cli 累積 transcript 在 memory
turn 3: 「/goal 寫成 wiki」  ───────┘    （透過 on_event 看到的 Stream events 重建）
                                       │
                                       ▼
                                  format prompt：
                                  「Based on this conversation:
                                   ... 全部 user / assistant 對話 ...

                                   Write: <user 第 3 turn 的 instruction>」
                                       │
                                       ▼
                                  spawn `codebus goal "<formatted>"`
                                  （新 spawn 拿到 Write / Edit toolset）
```

**Transcript 由 CLI 自己累積**（on_event 看到的 Stream 重建），不依賴 claude session 檔格式。Promote 是 codebus 層概念，claude 不知道。

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
| ❹ user 輸入 `/goal "..."` 怎麼脫離 chat？ | CLI 端 parse — 不丟給 claude；直接 exit REPL 後 spawn goal | （CLI 端決定，不真是 spike 但要設計） |

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

**Scope:**

- `codebus_core::verb::chat::run_chat_turn` + `ChatTurnOptions` / `ChatTurnReport`
- `codebus-cli/src/commands/chat.rs` REPL（stdin loop + transcript accumulator + `/goal` in-REPL command）
- 新 SKILL bundle `codebus-chat/SKILL.md`（multi-turn read-only；引導 agent 在合適點提示「這段可以 promote 成 wiki」但**不**自動 promote）
- RunLog schema 加 `session_id: Option<String>`（Backwards-compat via serde default `None`）
- Integration tests via mock_claude（per-turn spawn + session_id passthrough）

**Not in scope:**

- Agent 自動觸發 promote（先 user 主動，UX 驗證後再加）
- GUI 任何東西（D 的 scope）
- Cross-vault chat
- Chat 歷史搜尋 / 列表 / 刪除（claude 內建有 `/resume` picker，第一輪先靠他）

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
| chat mode 怎麼做？ | 新 verb，per-turn library + caller-owned REPL；spawn-time sandbox 鎖 read-only；promote 由 CLI 抓 transcript 重新 spawn goal |
| B 推遲嗎？ | **不推遲**。B 跟 chat 正交；先做 B 解 GUI run-log 基建 |
| CLI 先做嗎？ | **是**。spike 驗證 → CLI REPL → SKILL.md 調整 → 之後 GUI 拿成熟的 surface |
| roadmap 怎麼變？ | 7 條 → 8 條，chat 插在 B 跟 C 中間；D 改名 + scope 多 turn |
| 動工順序？ | 今天 propose B → spike day → discuss chat → propose chat → apply chat → C → D → E → F |

## Side note — design doc §4.6 要改

§4.6.3「Soft single-shot mode」段落整個翻轉。等 chat-verb apply 完、D 開 propose 時，由 D 的 propose / design 同步改寫 design doc §4.6。本討論不直接改 §4.6（避免改動 D 動工前就漂移）。

## 待 confirm

1. 上面整套順序（B → spike → chat → C → D ...）OK 嗎？
2. chat verb name `chat` 沿用，還是改別的（`talk` / `ask` / `qa`）？
3. CLI 有沒有 quit alias（`exit` / `:q` / Ctrl+D）偏好？
