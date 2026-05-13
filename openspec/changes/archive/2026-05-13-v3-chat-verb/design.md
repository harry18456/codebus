# v3-chat-verb 技術設計

## Context

本 change 對應 codebus-app v3 roadmap 的「chat verb」一條，position 在 B `v3-run-log-events` 與 C `v3-app-workspace-goal` 之間（roadmap 從 8 條保持 8 條，chat 是新插入的，不是改名）。

完整討論脈絡：`docs/2026-05-13-chat-verb-discussion.md` 是 design source of truth — 含 spike day 結果、3 種架構 pattern 比較、Promote-to-goal 機制 ASCII 圖、scope 衝擊 7 個 deliverable、Doc vs 實作 review 10 條 F# finding。本 design.md 不重述 doc 內容，只擷取 propose / apply 階段必須的設計決策。

關鍵 constraint（不可繞）：

- Claude CLI sandbox 三旗（`--tools` / `--allowedTools` / `--permission-mode`）是 spawn-time hard gate（v2 iter-9 spike 驗證），同一 claude process 不可能 mid-session 從 read-only 切到拿 Write tool
- Session 持久化在 `~/.claude/projects/<cwd-slug>/<session_id>.jsonl`，跨輪靠 `--resume <id>` 接 conversation history
- SKILL.md 是 conversation-level via user-message inject（spike 已驗 session jsonl line 6 證實），首輪載入後跨輪自動沿用
- claude CLI 內建 mid-turn cancel handshake：被中斷 turn 在 resume 時 CLI 自動 inject user-isMeta-true 訊息「Continue from where you left off.」，agent 回 「No response requested.」 收尾（spike 已驗），library 與 SKILL 不需 inject 任何 「interrupted」 邏輯

依賴的既有 module：

- `codebus_core::verb::{goal, query, fix}` — chat 加第 4 個 sub-module
- `codebus_core::agent::claude_cli::{invoke, InvokeAgentOptions, InvokeReport}` — chat 透過此擴充加 `--resume <id>` 支援
- `codebus_core::log::sink::RunLog` — 加 session_id optional 欄位
- `codebus_core::skill_bundle::{write_bundles_if_missing, VERBS, stub_content}` — 從 3 verb 擴成 4 verb

## Goals / Non-Goals

### Goals

- 為 user 提供 multi-turn 對話探索能力（READ-only sandbox），不破壞 query / goal / fix 既有語意
- AI 在判斷對話內容值得 promote 成 wiki page 時主動 emit suggestion，user 一鍵 (y/n) 確認
- 中斷對話 turn 後可在同 session 補新 prompt 接續（mid-turn supplement）
- CLI 端 user 看得到 activity stream（agent thinking 與 tool_use 一行 summary），跟既有 query / goal CLI 行為一致
- 對既有 module 最小改動（不動 `run_goal` 邏輯、不破 27+ test expectation、不擴 stream-json wire format）
- 為 GUI 未來 chat overlay 提供穩定的 verb library surface（透過 `run_chat_turn` 的 `on_event` callback）

### Non-Goals

- AI auto-fire promote（不經 user 確認直接 spawn goal）— 與 「user 一鍵確認」 設計直接衝突
- 同 agent 內 mode-switch 切寫 wiki — sandbox 物理限制
- GUI Cmd+K overlay 任何實作（D 的 scope）
- Classifier fallback path — spike emit signal PASS，不需要
- Cross-vault chat、chat 歷史搜尋與列表與刪除 UI（claude `/resume` picker 替代）
- Promote suggestion edit mode（v1 只接 y/n）
- 修改 `agent-stream-rendering` 的 closed enum（promote-suggestion 是 assistant text-inline convention，不是 stream-json wire format 擴展）

## Decisions

### Promote 走單一 AI-suggest 路徑，沒有 user CLI 指令

User 全程自然語言對話。當 user 想 promote 時直接跟 agent 講（例：「幫我把這段寫成 wiki」），SKILL.md 教 agent 把這類 phrase 當 promote-request 信號、下一輪 emit suggestion。

考慮過：(a) `/goal "..."` user CLI 指令 + AI suggest 雙路徑；(b) 純 AI suggest 單路徑；(c) `:promote` 無參數 escape hatch。選 (b) 理由：(a) surface 重複加違反 「一個對話框」 mental model；(c) 是中庸方案，但 spike emit signal PASS 後沒必要保留 escape hatch。

### Promote signal 走 line marker，不走 classifier

SKILL.md 約束 agent 在判斷對話值得 promote 時 emit line marker（規範性內容寫在 chat-verb spec 內）作為 message 第一行第一字元。CLI 與 library 端用前綴比對 parse。

考慮過：(a) line marker；(b) JSON tail；(c) synthetic tool_use；(d) 每輪結束額外 spawn classifier prompt。選 (a) 理由：spike 4/4 scenario PASS、格式 2/2 穩定；(b) JSON tail 容易破壞 agent 自然回應結構、parse 邊界 case 多；(c) tool_use 假裝沒有對應 toolset 實現、容易把 sandbox semantic 搞亂；(d) classifier 雙倍 cost 加 latency。

Fallback：若量產發現 emit 不穩定，spec 預留 classifier 路線作 v2 改動方案（不在本 change scope）。

### `--resume <id>` 不用 `--continue`

Chat library 第 2 輪起強制走 `--resume <session_id>`。

考慮過：`--continue`（cwd 最近一個 session）。選 `--resume` 理由：`--continue` 在 REPL cross-cwd（user 切 vault）會抓錯 session；顯式傳 id 是 caller-owned state、不依賴 cwd 副作用。Spike 兩個都驗過、session_id reuse 行為等價。

### `agent::invoke` 加 `resume_session_id` 欄位，不另開新 entry point

`InvokeAgentOptions` 加 `resume_session_id: Option<String>` 欄位；`invoke()` cmd builder 條件性加 `--resume <id>` arg。既有 query / goal / fix 三個 caller 一律 pass `None`、行為 byte-equivalent。

考慮過：chat verb 自己 spawn claude process（繞過 `agent::invoke`）。選擇擴 invoke 理由：DRY（sandbox flag composition、env injection、stream-json parsing、cancel signal polling 全 reuse）；既有 verb 自動繼承 resume 能力。

### `VerbLifecycleEvent::PromoteSuggestion { reason }` event variant，不放 `ChatTurnReport` 欄位

Stream parser 在 `StreamEvent::Assistant` 的 text content 第一行檢測到 marker 時，立即 emit `VerbEvent::Lifecycle(VerbLifecycleEvent::PromoteSuggestion { reason })`。CLI 收到後立即跳 (y/n) 互動 prompt。

考慮過：`ChatTurnReport.promote_suggestion: Option<...>` 欄位。選 event 理由：放 report 會逼 CLI 等整個 turn 結束才跳 prompt，bad UX；event channel 是既有 `VerbLifecycleEvent` enum 的自然擴展，module doc 明示「MAY be extended」。

### CLI Ctrl+C signal handler 在 commands/chat.rs，不在 library

chat 是第一個 CLI 端 set `cancel: Some(flag)` 的 verb。SIGINT trap 註冊在 `codebus-cli/src/commands/chat.rs`：

- 第一次 Ctrl+C：flip `AtomicBool` → `invoke()` 內部 line-by-line polling 偵測到 → kill child → drain stdout silently → `run_chat_turn` 回 `Err(VerbError::Cancelled)` 或同 session 帶 truncated state 的 Ok
- CLI 印 「interrupted, send your next message to continue this session」 並回 REPL prompt
- 第二次 Ctrl+C（前次 cancel 已 trigger 後）：exit REPL

`verb/error.rs` 的 `Cancelled` 註解必須 update — 不再「SHALL NOT occur on CLI paths」，改成「chat-verb CLI exposes Ctrl+C; other verb CLI 仍 pass `cancel: None`」。

SIGINT trap crate 選 `ctrlc`（cross-platform，Windows MSVC 是主要開發環境）。

### CLI spawn `codebus goal` 子進程，不從 chat library 內 call `run_goal`

User confirm promote 後，CLI 自己 spawn `codebus goal "<formatted-transcript>"` 子進程（透過 `std::process::Command`）。

考慮過：chat library 內直接 call `codebus_core::verb::goal::run_goal`。選 subprocess 理由：

- `run_goal` 是 long-running（含 fix loop），library 內 call 會 block chat REPL stdin loop（user 沒法看 goal 進度、沒法 Ctrl+C cancel goal）
- subprocess 自己有 stdin、stdout、stderr、user 在 terminal 看到 goal 走 stream-json render 跟單跑 `codebus goal` 體驗一致
- chat 跟 goal 兩條 RunLog row 分開寫（chat 的 RunLog mode 是 chat 加 session_id、goal 的 RunLog mode 是 goal 一如既往）
- 既有 `codebus goal` CLI 行為 byte-equivalent invariant 不破

### Goal verb 0 retroactive（review F3 修正）

`run_goal` step 9 已是 fix loop（既有 module doc 明列），`GOAL_TOOLSET` 已含 Write 與 Edit、`GoalReport.lint_error_count` 與 `lint_warn_count` 已存在。chat 從 promote 流 spawn `codebus goal` 子進程走 default flags 即可，既有 fix loop 自動跑。**不動** `run_goal` 邏輯、**不破** 既有 27+ integration test expectation、**不改** `codebus goal` CLI 行為 byte-equivalent invariant。

### RunLog 加 `session_id: Option<String>` 單一欄位

只擴 chat-verb 需要的 session_id 欄位（serde default `None`、向後兼容 legacy jsonl rows）；chat mode 的 row 寫 `Some(session_id)`、其他 mode row 寫 `None`。

考慮過：加 `lint_result` 與 `fix_result` sub-record（doc 早期版本誤寫）。選擇否決理由：既有 `lint_error_count` 與 `lint_warn_count` 扁平欄位、加 `outcome` 欄位已足夠表達 lint 加 fix 結果（review F2）。

## Implementation Contract

### Observable behaviors

- `codebus chat` CLI 進入 REPL 後 prompt 提示「> 」、user 輸入文字按 enter 觸發一輪對話
- 每輪期間 stdout 印 activity stream：每個 `tool_use` event 印一行（含 emoji 與 ToolName 與 abbreviated input，例 「→ Glob 對 wiki/modules/*.md」）、assistant text chunk 不單獨印（避免洗版）
- agent 輸出含 promote-suggestion line marker 在 message 第一行時，turn 結束後 CLI 跳 「promote to wiki? (y/n)」，user y 觸發 spawn `codebus goal` 子進程、n 繼續 REPL
- Ctrl+C 第一次：印 「interrupted, send your next message to continue this session」 加回 REPL prompt；第二次（cancel 已 trigger 後）：exit REPL
- `exit` 與 `:q` 與 Ctrl+D 任一：graceful exit REPL（待 Q3 確認）

### Library API surface

新加：

- `codebus_core::verb::chat::run_chat_turn(repo, ChatTurnOptions, on_event, cancel) -> Result<ChatTurnReport, VerbError>`
- `codebus_core::verb::chat::ChatTurnOptions { text: String, session_id: Option<String> }`
- `codebus_core::verb::chat::ChatTurnReport { session_id: String, accumulated_tokens: TokenUsage, started_at: String, finished_at: String, agent_exit_code: Option<i32> }`
- `codebus_core::verb::chat::CHAT_TOOLSET: &[&str]` 內容為 Read 加 Glob 加 Grep
- `codebus_core::verb::event::VerbLifecycleEvent::PromoteSuggestion { reason: String }`（新 variant）

修改：

- `codebus_core::agent::claude_cli::InvokeAgentOptions` 加 `pub resume_session_id: Option<String>` 欄位
- `codebus_core::log::sink::RunLog` 加 serde-default-None 的 `pub session_id: Option<String>` 欄位
- `codebus_core::skill_bundle::VERBS` 從含 3 verb 改成含 4 verb（加 chat）
- `codebus_core::skill_bundle::stub_content(verb)` 加 chat dispatch case 回傳 chat SKILL.md 內容
- `codebus_core::verb::error::VerbError::Cancelled` 註解 update

### Acceptance criteria

- `cargo build -p codebus-cli -p codebus-core` 通過、`cargo test` 不破壞既有 27+ goal integration test
- `codebus chat` CLI 在 Windows MSVC 跑通 happy path（spike uv vault 4 scenario 行為可重現）
- Mid-turn Ctrl+C 後同 session 接續行為符合 spike 觀察（claude CLI 內建 handshake auto-inject）
- Promote 確認後 spawn `codebus goal` 子進程走通、RunLog 寫 mode-chat row 加 mode-goal row 各一筆、events.jsonl 對應 spawn lifecycle event 齊全
- `cargo build -p codebus-core` 不破 query 與 goal 與 fix 既有 thin wrapper（caller 一律 pass `resume_session_id: None`，byte-equivalent）

### Scope boundaries

In scope:

- chat verb library 加 SKILL bundle 加 CLI REPL（含 activity stream 加 (y/n) 互動 加 SIGINT trap 加 spawn goal subprocess）
- `agent::invoke` 加 resume 欄位
- RunLog 加 session_id 欄位
- `VerbLifecycleEvent::PromoteSuggestion` variant
- `skill_bundle` module 從 3 verb 擴 4 verb

Out of scope:

- GUI（D 的 scope）
- `agent-stream-rendering` 的 closed enum 擴展
- `run_goal` 邏輯改動
- AI auto-fire promote
- Cross-vault chat
- Chat 歷史 UI

## Risks / Trade-offs

- [Spike emit-signal sample size 小（4 scenario 加 9 spawn）] 對應 Mitigation：在 chat-verb apply 階段補跑 5-10 個 scenario（含多語言混用 加 10+ turn 長對話 加 邊界 promote-request phrasing），結果寫進 changes/v3-chat-verb/notes.md
- [Mid-turn cancel AtomicBool flip path 跟 spike 驗的 timeout-kill path 不同] 對應 Mitigation：用 mock_claude integration test cover AtomicBool path（spec 內 explicit 列為 test scenario）
- [Token cost：N+1 輪 input 帶完整 partial turn N events，cost 比 fresh 第二輪略高] 對應 Mitigation：`ChatTurnReport.accumulated_tokens` 暴露 per-turn token usage 給 CLI 與 GUI 顯示，user 自己看 cost
- [MCP authenticate tools 不在 `--tools` 管制（6 個 mcp_claude_ai_*）] 對應 Mitigation：SKILL.md v0 已明確列為禁用（prompt-level 防線），propose 階段 spec 內 explicit document mcp 不在 binary toolset gate 範圍
- [signal handler crate（ctrlc）在 Windows console 跟 Unix terminal 行為差異] 對應 Mitigation：integration test 在 Windows MSVC 跑 happy path，cross-platform manual verification 集中到 F polish-ship
- [SKILL.md emission rule 若 agent 不配合 emit、user 沒 escape hatch] 對應 Mitigation：不在本 change scope 解；如果量產發現問題，後續 change 加 classifier fallback 或 promote escape hatch

## Migration Plan

純擴展、無 breaking change：

1. `RunLog.session_id: Option<String>` 加 `serde(default)`，既有 jsonl rows 反序列化保持 None
2. `InvokeAgentOptions.resume_session_id: Option<String>` 既有 caller 一律 pass `None`、行為 byte-equivalent
3. `VerbLifecycleEvent::PromoteSuggestion` 是新 variant，既有 match arms 用 wildcard 處理（mod.rs 註解明示 non-exhaustive 期望）
4. `skill_bundle::VERBS` 從 3 變 4，init flow 跑 `write_bundles_if_missing` 對既有 vault 增加 codebus-chat bundle 寫入（如缺，遵守 write-if-missing 不覆蓋）
5. `cli` Subcommand Registration 加 chat 既有 user 不知道也不影響

無 rollback step — 純加 capability、不破 byte-equivalent invariant。

## Open Questions

對應 `docs/2026-05-13-chat-verb-discussion.md` 的待 confirm Q1 加 Q2 加 Q3 加 Q5 加 Q6（Q4 加 Q7 已 spike resolve）：

- Q1: roadmap 順序（B → chat → C → D ...）是否 confirm？預設 confirmed，propose 假設沿用
- Q2: verb name 沿用 `chat` 還是改別的（`talk` 加 `ask` 加 `qa`）？預設 `chat`，spec 用此名
- Q3: CLI quit alias（`exit` 加 `:q` 加 Ctrl+D）三選都接、還是縮減？預設三選都接
- Q5: Goal-internal lint+fix 失敗時（goal 流尾端 fix 失敗）：rollback wiki write、還是留下未 lint 的 wiki page 加 RunLog 標 failed？預設 latter（既有 goal verb 行為，0 retroactive 約束下不動）
- Q6: Activity stream render 詳細度：CLI 端只印 tool 摘要、thinking text chunk 不單獨印；還是 thinking 也要印幾行？預設只印 tool 摘要、thinking 不印（避免洗版）

apply 階段若 Q2 加 Q5 加 Q6 預設值要改，回頭調 spec 加 tasks。
