## Summary

Align `run-log/spec.md` (plus two same-class drifted specs) with the already-shipped `quiz` verb behavior: the quiz generate spawn writes a `RunLog` with `mode == "quiz"`. This is a spec-consistency alignment only ??it introduces no new behavior and changes no production logic.

## Motivation

When the `quiz` verb landed, `codebus-core/src/verb/quiz.rs` began writing a `RunLog` with `mode: "quiz"`, `goal: options.pages.join(",")` (the comma-joined selected page paths), and the generate spawn's `session_id`. The authoritative `quiz` capability already documents this (`run_quiz_generate` SHALL persist a RunLog mode `quiz`; `run_quiz_plan` SHALL NOT). But the `run-log` capability was never updated when quiz shipped ??its SHALL-level normative enumerations (the per-invocation verb list, the `mode` value set, and the `goal` / `session_id` field semantics) still list only `goal` / `query` / `fix` / `chat`, directly contradicting both the code and the `quiz` capability.

The code-side comments were already aligned in commit 56ddd82 (sink RunLog + InvokeReport.session_id). This change covers the spec side only.

Two further specs carry the identical drift and are swept in here to avoid piecemeal fixes:

- The `cli` capability "Verb RunLog Capture and Persistence" requirement enumerates the RunLog-capturing subcommands as `goal` / `query` / `fix` / `chat`, omitting `quiz`.
- The `app-workspace` capability "Goals Overview List and Filter" requirement lists the non-goal modes excluded from the Goals list as `chat` / `query` / `fix`, omitting `quiz`.

## Proposed Solution

- `run-log` "RunLog Schema and Per-Invocation Capture": add `quiz` to the per-invocation verb enumeration and to the `mode` value set; document the quiz semantics of the `goal`, `wiki_changed`, `lint_error_count`, and `session_id` fields; note that the quiz RunLog is written by the generate spawn (the plan sub-step writes none); add quiz RunLog scenarios plus a serialized-quiz example.
- `cli` "Verb RunLog Capture and Persistence": add `quiz` as a RunLog-capturing subcommand with the generate-not-plan nuance and a scenario.
- `app-workspace` "Goals Overview List and Filter": add `quiz` to the non-goal modes that SHALL NOT appear in the Goals list.
- Lock the documented behavior against the code with a regression test that parses the persisted run-log jsonl and asserts the quiz RunLog `mode` and `goal`.

## Non-Goals

- No production logic change. The quiz verb already writes `mode: "quiz"`; no verb, sink, or IPC code is touched.
- No change to the `quiz` capability ??it is already authoritative and correct.
- No speculative abstraction for hypothetical future verbs; only the existing quiz verb is documented.
- Automated assertion of `session_id == Some(...)` for quiz is out of scope: the `codebus-core` unit-test harness runs no mock agent, so the generate spawn emits no init event and `session_id` is `None` there. The `Some` semantic is documented by the run-log spec scenario; the regression test locks only the deterministically-assertable fields (`mode`, `goal`, `wiki_changed`).
- Excluded look-alikes that are a different drift class: the `app-shell` settings-UI verb list and the `claude-code-config` Verb resolution enum (both config-profile verbs, NOT runtime RunLog modes), the `agent-stream-rendering` render "mode" (verbosity, not RunLog mode), and the `events-log` sink example (illustrative ??it omits chat as well).

## Alternatives Considered

- Fix only `run-log` and defer `cli` / `app-workspace`: rejected ??they carry the identical SHALL-level drift; piecemeal fixes erode trust and the sweep was explicitly in scope.
- Add a separate standalone "Quiz RunLog" requirement instead of modifying the existing schema requirement: rejected ??quiz is one more case of the same single schema-and-capture requirement; a parallel requirement would duplicate field semantics and risk future divergence.

## Impact

- Affected specs:
  - Modified: run-log (RunLog Schema and Per-Invocation Capture), cli (Verb RunLog Capture and Persistence), app-workspace (Goals Overview List and Filter)
- Affected code:
  - Modified: codebus-core/src/verb/quiz.rs (regression test only ??strengthen the existing run_quiz_generate_persists_runlog_and_events test to assert mode and goal; no production change)

