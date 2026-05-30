## 1. Land the spec alignment deltas

- [x] 1.1 Apply the `run-log` delta for **RunLog Schema and Per-Invocation Capture**: confirm the per-invocation verb enumeration and the `mode` value set both include `quiz`, the `goal` / `wiki_changed` / `lint_error_count` / `session_id` field semantics document the quiz case, and the new `Quiz RunLog records pages-joined goal and quiz mode` / `Quiz plan sub-step writes no RunLog` scenarios plus the serialized-quiz example are present and consistent with `codebus-core/src/verb/quiz.rs`.
- [x] 1.2 Apply the `cli` delta for **Verb RunLog Capture and Persistence**: confirm the RunLog-capturing subcommand list includes `quiz`, the generate-not-plan nuance is stated, and the `Quiz subcommand appends one RunLog with mode quiz` scenario is present.
- [x] 1.3 Apply the `app-workspace` delta for **Goals Overview List and Filter**: confirm `quiz` is added to the non-goal modes excluded from the Goals list (both the requirement prose and the `Goals overview filters to goal mode only` scenario).

## 2. Lock quiz RunLog behavior with a regression test

> Grounding correction (apply, 2026-05-30): the original draft assumed an existing
> `run_quiz_generate_persists_runlog_and_events` unit test in `codebus-core/src/verb/quiz.rs`.
> No such test exists, and `codebus-core` unit tests cannot exercise `run_quiz_generate`
> because it spawns the agent (the unit layer has no mock agent). The runnable home that
> actually drives the full generate flow and writes a real RunLog is the CLI integration
> test file `codebus-cli/tests/quiz_flow.rs`, which spawns the binary with `mock-claude`
> (matches the `quiz` capability design D8: the CLI layer owns end-to-end mock spawn tests
> for the quiz verb). The CLI `codebus quiz "<topic>"` is Goal-scope, so `RunLog.goal`
> equals `options.pages.join(",")` over the plan-resolved pages, NOT the literal
> `"wiki/modules/auth.md"` (that value came from the core page-scope fixture).

- [x] 2.1 In `codebus-cli/tests/quiz_flow.rs`, add a regression test (e.g. `quiz_goal_match_runlog_has_quiz_mode_and_pages_goal`) that runs `codebus quiz "how does auth work"` with the existing `quiz-goal-match` mock behavior, reads the written `runs-*.jsonl` under `<repo>/.codebus/log/`, and asserts the persisted RunLog row contains `"mode":"quiz"` AND `"wiki_changed":false` AND a `goal` value equal to the comma-joined plan-resolved pages. Ground the `goal` assertion on the already-verified fact that the `quiz-goal-match` planned scope includes `wiki/concepts/jwt-token-lifecycle.md` (the existing `quiz_goal_match_writes_file_with_caller_frontmatter` test asserts this page in the quiz `.md` frontmatter), so assert the RunLog `goal` contains that page path. This locks the **RunLog Schema and Per-Invocation Capture** quiz contract against the code.
- [x] 2.2 Assert exactly one RunLog row exists for the invocation (plan writes none, generate writes one — per the `Quiz plan sub-step writes no RunLog` scenario). Do NOT assert on `session_id` presence: whether the `quiz-goal-match` mock generate spawn emits an `init` event carrying a `session_id` is an internal mock detail, so asserting it would be brittle. The `Some(...)`-for-quiz semantic is documented by the run-log capability "Quiz RunLog records pages-joined goal and quiz mode" scenario; add a one-line code comment in the test stating why `session_id` is intentionally not asserted so a future reader does not mistake the omission for a gap. Assertions use raw-substring matching on the serialized JSON line (no extra dependency required), mirroring the run-log spec's own serialization examples.
- [x] 2.3 Run `cargo test -p codebus-cli --test quiz_flow quiz_goal_match_runlog` and confirm the test passes. As a sanity check that the lock is real, temporarily change the asserted `mode` literal to a wrong value, confirm the test FAILS, then revert.

## 3. Verify spec↔code consistency (grounding)

- [x] 3.1 Re-read the `RunLog { .. }` construction in `codebus-core/src/verb/quiz.rs` (the `run_log` binding immediately before `write_run_log`) and confirm the landed deltas match the actual field writes: `mode: "quiz"`, `goal: goal_text` where `goal_text = options.pages.join(",")`, `session_id: gen_report.session_id.clone()`, `wiki_changed: false`, `lint_error_count: findings.len()`, `lint_warn_count: 0`. Correct any delta wording that drifts from the code before archive.
- [x] 3.2 ~~Confirm this change introduces no production logic change: inspect `git diff` and verify the only non-`openspec/` edit is the new test added to `codebus-cli/tests/quiz_flow.rs` in section 2.~~ **Superseded by 5.4** — section 5 adds a second non-`openspec/` edit (a one-line comment in `codebus-cli/src/main.rs`). The full, current non-`openspec/` edit set is verified in task 5.4.

## 4. Validate change artifacts

- [x] 4.1 Run `spectra validate run-log-spec-include-quiz` and `spectra analyze run-log-spec-include-quiz`; resolve any Critical or Warning findings so the change is apply-clean before archive.

## 5. Addendum — same-class drift: --debug verbose rendering also omits quiz

> Grounding (apply, 2026-05-30): same root cause as the RunLog `mode` omission — quiz was
> added late and the agent-spawning-verb enumeration was not updated. Quiz IS an
> agent-spawning verb that verbose-renders under `--debug`, proven by:
> (1) `codebus-cli/src/main.rs` dispatch passes the same `&render_opts` snapshot (with
>     `render_opts.verbose = cli.debug`) to `commands::quiz::run(..., cli.debug, &render_opts)`;
> (2) `codebus-cli/src/commands/quiz.rs` clones it and `print_event(&s, &render_for_closure)`
>     on `VerbEvent::Stream`, identical to goal/query/fix/chat.
> Sweep result: every OTHER `goal/query/fix/chat/quiz` enumeration across `openspec/specs`
> (codex-backend, skill-bundles, verb-library, agent-backend) ALREADY includes quiz; the only
> two runtime/agent-spawning lists that omitted it were the cli `Debug Flag Output` requirement
> (this section) and the cli/run-log RunLog lists (already fixed in sections 1–2).
> False-positive guard honored: `app-shell` settings-UI verb list (`goal/query/fix/verify/chat`)
> is a config-verb list with `verify` — NOT a runtime/agent-spawning list — and is left untouched.

- [x] 5.1 Update the `cli` delta to MODIFY the **Debug Flag Output** requirement: change the verbose-applies-to enumeration from `goal` / `query` / `fix` / `chat` to `goal` / `query` / `fix` / `chat` / `quiz`. Reproduce the rest of the live requirement verbatim (including the "it does not alter how non-agent subcommands render" clause and the four existing scenarios), and add one scenario asserting `codebus quiz --debug` inherits verbose agent-stream rendering.
- [x] 5.2 Update the comment in `codebus-cli/src/main.rs` above `render_opts.verbose = cli.debug` so the "Applies to the agent-spawning verbs ... (goal / query / fix / chat)" line reads `(goal / query / fix / chat / quiz)`. Comment-only — no production logic change.
- [x] 5.3 Confirm no other `openspec/specs` file carries a `goal/query/fix/chat`-style agent-spawning-verb or runtime-mode enumeration that omits quiz (sweep recorded in the grounding note above); confirm the `app-shell` settings-UI config-verb list is correctly left untouched per the false-positive guard.
- [x] 5.4 Re-verify the full non-`openspec/` edit set for the whole change: `git diff` SHALL show exactly two non-`openspec/` edits — the new test in `codebus-cli/tests/quiz_flow.rs` (section 2) and the one-line comment in `codebus-cli/src/main.rs` (5.2) — both comment-or-test-only, no production logic change. Run `cargo build -p codebus-cli` to confirm the comment edit compiles.
