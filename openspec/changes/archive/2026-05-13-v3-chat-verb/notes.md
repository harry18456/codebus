# v3-chat-verb — implementation notes

Captures items that the formal artifact set (proposal / design / specs / tasks)
intentionally defers to manual verification or future follow-up.

## Spike sample补強 (task 9.2)

The design `Risks / Trade-offs` first item flagged that the spike `❺` sample
size was small (4 scenarios / 9 spawn). The propose-time recommendation was to
補跑 5-10 個 mock_claude scenarios in apply phase covering:

- 多語言混用 (English question + Chinese prompt switch mid-conversation)
- 10+ turn 長對話
- 邊界 promote-request phrasing ("這個值得記下來嗎", "save this", "記一下")

**Status: deferred to follow-up apply work after first live use.** The
mock_claude binary can drive deterministic stream-json but cannot validate the
agent-side judgment that the SKILL.md rules are tuning toward — `❺` validates
prompt → emission stability, which intrinsically requires a real claude
agent. The mock-claude-driven integration tests in
`codebus-cli/tests/chat_flow.rs` cover the CLI surface and library-level
parsing logic; live SKILL judgment补强 is best done by manual exploration of a
real vault when the chat verb sees its first users.

The cargo-testable scope (11 scenarios in `chat_flow.rs`) covers every
chat-verb spec scenario that does not require live agent judgment:

- vault precondition + exit aliases (`exit` / `:q` / Ctrl+D / empty input)
- first turn returns session_id + writes RunLog `mode=chat`
- N+1 turn drives `--resume <session_id>` argv
- activity stream renders tool_use as one-line summaries
- assistant text rendered once at turn-end, not per chunk
- 3 turns produce 3 RunLog rows sharing the same session_id
- promote-suggestion `(y/n)` prompt + decline returns to REPL
- chat does not auto_commit

## Windows MSVC manual happy path (task 10.3)

Design `Acceptance criteria` includes "Windows MSVC 跑通 happy path (spike uv
vault 4 scenario 行為可重現)".

### 2026-05-13 live verification — partial

Ran `codebus chat` against `D:/side_project/uv` (target/debug build) with
single-turn S2-style prompt. **Live PASS** for the foundational REPL +
spawn + render + log path:

```
$ cd D:/side_project/uv
$ printf 'what is uv-lib in one short sentence?\nexit\n' | codebus.exe chat
> → Read D:\side_project\uv\.codebus\wiki\index.md
→ Glob "**uv-lib**" in D:\side_project\uv\.codebus\wiki
→ Read D:\side_project\uv\.codebus\wiki\modules\uv-lib.md
I'll look up what uv-lib is in the wiki and code.**uv-lib** is the library
entry point (`crates/uv/src/lib.rs`) of the `uv` binary crate that hosts the
`run()` function and dispatches CLI commands to their respective handlers.
> $
$ echo exit=$?
exit=0
```

Verified in this run:

- Real `claude` binary (2.1.140) spawned by `codebus chat`
- Activity stream rendered 3 `tool_use` events as one-line `→ <Tool> <input>` summaries
- Assistant text rendered ONCE at turn end (no per-chunk洗版)
- Prompt symbol `> ` displayed at turn boundaries
- `exit` alias terminated REPL gracefully (exit 0)
- Vault precondition passed (cwd contains `.codebus/`)
- SKILL bundle loaded — agent stayed inside read-only Read/Glob (no Write/Edit attempts)
- RunLog row written with full chat semantics (extracted from
  `D:/side_project/uv/.codebus/log/runs-2026-05-13.jsonl`):
  ```
  mode=chat
  goal="what is uv-lib in one short sentence?"
  session_id="cce20d32-9c15-41c9-98d4-0a8a68eee839"
  outcome="succeeded"
  tokens.input_tokens=26
  ```
- Claude session jsonl persisted at
  `~/.claude/projects/D--side-project-uv--codebus/cce20d32-9c15-41c9-98d4-0a8a68eee839.jsonl`
  (proves `--resume <id>` mechanism would work for turn 2)
- events.jsonl written: 11 events for the single turn at
  `.codebus/log/events-2026-05-13T14-56-21Z.jsonl`

### S1–S4 live verification — complete (2026-05-13)

All four spike ❺ scenarios re-run live against `D:/side_project/uv` with the
current `target/debug/codebus.exe` + real claude 2.1.140. Total cost ≈ $1.5 USD.

| Scenario | Turns | Marker emit | Wiki write | Session id | Result |
|---|---|---|---|---|---|
| S2 (single factual lookup) | 1 chat | none (expected) | no | n/a | ✅ |
| S1 (explicit promote + y → spawn goal) | 3 chat + 1 goal subprocess | T3 中文 reason | 1 new + 4 updated, commit `b2a2e7a` | `bbb6c099-...` | ✅ |
| S3 (natural consolidation) | 3 chat (decline at T3) | T3 English reason | no (declined) | `56a92142-...` | ✅ |
| S4 (ambiguous existing wiki page) | 2 chat | none (correct — agent points to `wiki/modules/uv-auth-lib.md`) | no | `891c6991-...` | ✅ |

**End-to-end critical chain validated in live mode:**

- `codebus chat` REPL → spawn real claude binary
- Activity stream renders `→ Tool input` one-liners (Read / Glob / Grep observed)
- Assistant text buffered + printed once at turn end (no per-chunk洗版)
- Multi-turn `--resume <session_id>` works (S1, S3, S4 each have multiple
  chat RunLog rows sharing the same session_id)
- Promote-suggestion line marker detection + `(y/n)` confirm prompt
- User `y` → CLI spawns `codebus goal` subprocess via
  `std::process::Command` (NOT library call)
- Goal subprocess runs full ingest flow: source exploration, page
  generation, lint phase, fix loop, `git auto_commit` → returns to chat
  REPL prompt → `exit` terminates with status 0
- Per-turn RunLog rows persisted with full chat semantics
  (`mode=chat`, `session_id=Some`, `wiki_changed=false`, `outcome=succeeded`)
- Goal subprocess writes its OWN RunLog row (`mode=goal`, no `session_id`,
  `wiki_changed=true` when wiki actually changed) — chat + goal rows
  coexist without overlap

### Findings (not bugs, recorded for posterity)

1. **S3 emit-timing variance**: agent emitted at T3 (3-turn consolidation
   point) instead of T4 (spike ❺ observation). Both are within the SKILL
   rule "chained 3+ related questions reaching a durable understanding".
   No SKILL change needed.
2. **Piped-stdin cosmetic**: when the `[suggest] promote to wiki? (y/n) `
   prompt is followed by `read_line` against a piped stdin, the user's
   typed `y\n` is not echoed back to stdout, so the next stdout output
   (goal subprocess banner) appears concatenated on the same line in
   captured logs. Pure terminal-cosmetic — in real interactive terminal
   mode the keystroke echo provides the visual newline. Not a behavior
   bug; no v1 fix needed.

### Real side effects in the uv vault

S1's promote spawned a real goal flow that wrote one new page + updated
four existing pages in `D:/side_project/uv/.codebus/wiki/`, then
auto-committed to the nested `.codebus/` git repo as commit `b2a2e7a`.
The vault therefore now contains genuinely-authored chat-promote wiki
content. To revert if desired:

```
git -C D:/side_project/uv/.codebus reset --hard HEAD~1
```

Otherwise the new pages stand as actual durable wiki content produced
by the chat-verb promote flow.

## Clippy strict-mode baseline (task 10.1)

`cargo build --workspace` exits 0 cleanly. `cargo clippy --workspace -- -D
warnings` fails on 6 pre-existing baseline issues in files NOT modified by
v3-chat-verb:

- `codebus-core/src/log/events/jsonl_sink.rs:52` — `target_path is never used`
- `codebus-core/src/config/endpoint.rs:48,226` — `this impl can be derived`
- `codebus-core/src/config/log.rs:30` — `this impl can be derived`
- `codebus-core/src/config/pii.rs:46` — `large size difference between variants`
- `codebus-core/src/skill_bundle/mod.rs:6` — `doc list item overindented`
  (in the original module-level doc comment, not in chat additions)
- 3 warnings in `codebus-cli/src/commands/{config,hook,lint}.rs` — `needless_borrow`

chat-verb code (`codebus-core/src/verb/chat.rs`,
`codebus-core/src/skill_bundle/mod.rs` chat dispatch + CHAT_SKILL_CONTENT,
`codebus-cli/src/commands/chat.rs`, `codebus-cli/tests/chat_flow.rs`,
`codebus-cli/tests/bins/mock_claude.rs` chat behaviors) introduces 0 new
clippy warnings. Task 10.1 verification target — "不出新警告" — is satisfied
on the introduced surface. Fixing the baseline is out of scope.

## Cancel scenarios

Two layers:

1. **Library-level cancel** — `verb::chat::run_chat_turn` honours the
   `cancel: Option<Arc<AtomicBool>>` flag per `Cancellation Signal Polling`
   (verb-library capability) via the shared `agent::invoke` polling loop.
   `agent::invoke` polls the flag after each stdout line; flipped→true
   triggers `child.kill()` + drain mode + returns `InvokeReport` with
   killed exit. `run_chat_turn` then writes a cancelled `RunLog` (mode=chat
   + session_id from init event) and returns `Err(VerbError::Cancelled)`.
2. **CLI signal handler** — `commands/chat.rs` registers a `ctrlc::set_handler`
   that flips the flag on first Ctrl+C and calls `std::process::exit(0)`
   directly on a second Ctrl+C (so a blocked stdin read at the REPL prompt
   cannot trap the user).

### Tested
- **Library-level cancel** is directly verified by `codebus-cli/tests/chat_cancel.rs`
  using mock_claude's `chat-trickle-cancel` behavior: emit init, sleep 800ms,
  emit tool_use, sleep 800ms, emit result. Test thread spawns
  `run_chat_turn` in a worker thread, sleeps 250ms (lets init line be
  processed), flips the AtomicBool, joins. **Verified**: worker returned
  `Err(VerbError::Cancelled)`; cancelled `RunLog` row persisted to vault
  with `outcome="cancelled"` + `mode="chat"` + the init-event session_id.
  Wall time per test: ~0.9s.

### Not tested
- **CLI signal handler glue** (the `ctrlc::set_handler` binding in
  `commands/chat.rs` that flips the flag on first Ctrl+C and exits on
  second). Driving real OS-level SIGINT/SIGBREAK on a child process cross-
  platform requires a signal-injection harness this codebase doesn't have
  (Unix `nix::sys::signal::kill` + Windows `GenerateConsoleCtrlEvent` have
  different attached-console requirements). The handler is 7 lines, audit-
  reviewed, and exercises only the library-level cancel path which IS tested.
  **Status: deferred to manual live verification.**

## Quote-bearing user prompts (defensive verification)

User input with embedded `"` characters is propagated verbatim to the
spawned claude as a single argv element — `Command::arg` doesn't
shell-escape, so the user's literal text reaches claude as-is. The chat
verb's `format!("/codebus-chat \"{}\"", options.text)` slash-command
template matches the pre-existing pattern used by goal/query verbs;
the embedded-quote case is **pinned** by the `chat_passes_user_prompt_with_embedded_quotes_verbatim_to_claude`
test in `chat_flow.rs` (asserts the user phrase appears intact in
mock_claude's argv log).

How claude's own slash-command parser handles the quoted form is
out-of-scope for codebus — we treat claude as the boundary and verify
verbatim propagation up to that boundary. Pre-existing pattern across
all spawn verbs; not a chat-verb-specific risk.

## Render options for activity stream

`commands/chat.rs::tool_prefix` selects `→` for ASCII / `📖 Read`,
`🔍 Glob`, `🔎 Grep` for emoji-enabled terminals per
`RenderOptions.use_emoji`. Unit tests `tool_prefix_uses_arrow_when_emoji_disabled`
and `tool_prefix_uses_distinct_emoji_per_known_tool` pin the contract.
Aligns with `Activity Stream Render` requirement's
"`→ ` or emoji-leading form per render options" spec wording.

`chat_flow.rs` integration tests run with `TERM=dumb` + `NO_COLOR=1`,
so they exercise the `→` ASCII branch. Live S1–S4 also ran with the
emoji-disabled branch (per user terminal). The emoji-leading branch is
covered by unit tests only — full live emoji verification would require
a terminal where `RenderOptions::detect()` returns `use_emoji=true`.
