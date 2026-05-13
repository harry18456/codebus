## Why

v3-app 線上 user 在 §4.6.3 「Soft single-shot mode」 push back：「一次問答不太符合實際使用情況。我想要在同一個 session 持續問答，然後可以做到 agentic AI 會問要不要把內容更新到 goal 之類的行為。」

既有三個 verb（goal / query / fix）無一覆蓋「multi-turn 探索式對話 + AI 自主判斷對話內容值得寫進 wiki 時提議 promote」的 use case：

- `query` 是 single-shot 讀答型，無法跨輪累積上下文
- `goal` 是 write 動作，read-only chat 體驗不適合
- `fix` 是 lint 自動修復，與對話互動正交

加上 Claude CLI sandbox 是 spawn-time hard gate（spike `_pii-toolgate-spike` 已驗），同一 agent process 不可能 mid-session 從 read-only 切到拿 Write tool，因此「同 agent 內 mode-switch 改 wiki」物理上做不到 — 必須是新 verb + promote 流走獨立 spawn。

完整討論脈絡（含 spike day ❶❷❸❺❻ 結果與 doc vs 實作 review 10 條 finding）見 `docs/2026-05-13-chat-verb-discussion.md`。

## What Changes

新增「multi-turn 對話 verb」與 AI-initiated promote-to-goal flow：

- 新 verb `codebus chat`：CLI REPL multi-turn 對話，read-only sandbox（`Read,Glob,Grep`），session 透過 Claude CLI `--resume <session_id>` 跨輪沿用
- 新 SKILL bundle `codebus-chat`：multi-turn read-only 工作流定義 + promote-suggestion emission convention（line marker `[CODEBUS_PROMOTE_SUGGESTION] <reason>`，spike ❺ 4/4 scenario PASS、格式 2/2 穩定）
- 新 `VerbLifecycleEvent::PromoteSuggestion { reason: String }` event variant：stream parser 偵測到 marker 立即 emit，CLI/GUI 收到後跳 `(y/n)` 互動 prompt（不等 turn 結束）
- 新 CLI Ctrl+C signal handler infra：chat 是**第一個** CLI 端 set `cancel: Some(flag)` 的 verb，需註冊 SIGINT trap → flip `AtomicBool`；既有 verb CLI 都 `cancel: None`，本 change 內 `verb/error.rs:11` 註解需 update
- `codebus_core::agent::claude_cli::InvokeAgentOptions` 加 `resume_session_id: Option<String>` 欄位 + invoke() cmd builder 加 `--resume <id>` arg；既有 query/goal/fix 三個 caller 一律 pass `None`，行為 byte-equivalent
- `RunLog` schema 加 `session_id: Option<String>` 欄位（serde default `None`，向後兼容）；chat verb 寫入此欄位、其他 verb 不寫
- Promote 確認後 CLI spawn `codebus goal "<transcript>"` 子進程（**不**透過 library call），既有 `run_goal` 內 fix loop（goal.rs:17 step 9）自動跑 lint+fix
- **不動** `run_goal` 邏輯、**不破** A archived `v3-goal-library` 既有 27+ integration test expectation、**不改** `codebus goal` CLI 行為 byte-equivalent invariant

## Non-Goals

- AI auto-fire promote（不經 user 確認直接 spawn goal）— explicit ruled out
- 在 chat agent 內部 mode-switch 切寫 wiki — claude CLI sandbox spawn-time hard gate 物理上做不到
- GUI Cmd+K chat overlay — D `v3-app-chat-cmdk` 的 scope
- Classifier fallback path（每輪 chat 結束額外 spawn 輕量 classifier 判斷 promote-worthy）— spike ❺ emit signal PASS、不需要
- Cross-vault chat（一個 chat session 橫跨多個 vault 的對話）
- Chat 歷史搜尋 / 列表 / 刪除 UI — claude 內建有 `/resume` picker，v1 靠他
- Promote suggestion `(y/n/edit)` 中 edit mode（user 編輯 AI 建議的 goal text 再送）— v1 只接 y/n
- 在 chat-verb 這條 change 內動 `agent-stream-rendering` 的 stream-json wire format（promote-suggestion 是 assistant text-inline convention，不是 stream-json outer event type 的擴展）

## Capabilities

### New Capabilities

- `chat-verb`: Multi-turn 對話 verb 的 library API (`run_chat_turn`)、SKILL bundle 內容約束（含 promote-suggestion line marker schema）、CLI REPL 行為（activity stream render / `(y/n)` interactive prompt / Ctrl+C signal handler）、promote 確認後 spawn goal 子進程的 transcript dump 格式

### Modified Capabilities

- `verb-library`: Module Surface requirement 從 3 sub-modules（goal/query/fix）擴成 4（加 chat）；`VerbLifecycleEvent` enum 加 `PromoteSuggestion { reason: String }` variant；`InvokeAgentOptions` 加 `resume_session_id: Option<String>` 欄位、`invoke()` 條件性加 `--resume <id>` arg；`VerbError::Cancelled` 註解 update（chat-verb CLI 是第一個 set `cancel: Some(flag)`，要破除「SHALL NOT occur on CLI paths」既有約束）
- `skill-bundles`: Bundle Layout requirement 從 3 verbs（goal/query/fix）擴成 4（加 chat）；`VERBS` 常數 + `write_bundles_if_missing` 從 6 outcomes 變 8 outcomes（4 verb × 2 locations）；`stub_content(verb)` dispatch 加 chat case
- `cli`: Subcommand Registration requirement 從「exactly six subcommands」改成「exactly seven」（加 `chat`）；chat 的 Per-Verb Subcommand Behavior（sandbox flags / 不 auto-commit / no RunLog write from `run_chat_turn` 內部 — chat-verb library 自己寫 RunLog）；CLI thin-wrapper pattern **不適用** chat（chat 是 REPL loop + 多次 library call + interactive prompt + spawn 子進程，spec 內 explicit document 此例外）
- `run-log`: RunLog Schema 加 `session_id: Option<String>` optional 欄位（serde default `None`，向後兼容 legacy jsonl rows）；只有 `mode == "chat"` 的 row 寫入此欄位

## Impact

- Affected specs: `chat-verb` (new), `verb-library` (modified), `skill-bundles` (modified), `cli` (modified), `run-log` (modified)
- Affected code:
  - New:
    - codebus-core/src/verb/chat.rs
    - codebus-cli/src/commands/chat.rs
    - .codebus/.claude/skills/codebus-chat/SKILL.md
    - .claude/skills/codebus-chat/SKILL.md
  - Modified:
    - codebus-core/src/verb/mod.rs
    - codebus-core/src/verb/event.rs
    - codebus-core/src/verb/error.rs
    - codebus-core/src/agent/claude_cli.rs
    - codebus-core/src/log/sink.rs
    - codebus-core/src/skill_bundle/mod.rs
    - codebus-cli/src/commands/mod.rs
    - codebus-cli/src/main.rs
    - Cargo.toml (新增 SIGINT trap 用 crate dependency，候選 signal-hook 或 ctrlc)
  - Removed: (none)
