## Why

The archived `quiz-validate-repair` change gave quiz a deterministic final-verify (schema + `[[slug]]` existence) plus an agent self-repair loop, and recorded an explicit deferred Stage 2 (design D6): an *independent model* judging whether the quiz **content** is actually good. The deterministic validator cannot tell that a marked answer is factually wrong, that a question asserts something the planned pages never say, or that a question is off-topic versus what the user asked. This change implements that Stage 2 so a generated quiz is not just well-formed but content-defensible before it is persisted.

## What Changes

- A new independent **verify spawn** runs after the deterministic final-verify in `run_quiz_generate`: a second agent reads the planned pages plus the generated quiz and emits per-question defect findings against a fixed five-item contract (answer wrong vs pages; out-of-scope assertion; not-exactly-one-correct; degenerate distractors; off-topic vs the user's requested topic — the last only when a topic exists, i.e. the Goal flow).
- Verify findings feed the existing Stage-1 trust-agent repair path (the D6 insertion point already accepts external findings of the same shape): the generate agent revises the flagged questions and re-verifies, bounded by a hard internal iteration cap with an emit-best-on-cap fallback (mirrors Stage-1).
- The persisted quiz gains a `content_review: ok | flagged` caller frontmatter field (plus flagged question numbers when flagged); residual flags after the cap are best-effort — a non-fatal warning is emitted, no question is dropped, and the verb exit code is unchanged.
- A new `quiz.content_verify` config key (boolean, default **false**) gates the whole verify+repair stage so existing users do not silently pay extra model spawns. The codebus CLI reads this key; it never reads the app-only `app.*` namespace.
- The codebus-quiz SKILL gains a third `verify:` mode (alongside `plan:` / `generate:`) describing the five-item defect contract the verify agent applies; the deterministic `codebus quiz validate` sub-action is unchanged (structure/citation only — content judgement is a separate concern).
- `run_quiz_generate` accepts the originating topic (Goal flow) so the off-topic check can run; the Page flow passes no topic and that one check is skipped.

## Non-Goals (optional)

(Captured in design Goals/Non-Goals.)

## Capabilities

### New Capabilities

(none — this enhances the existing `quiz` verb; no new capability spec)

### Modified Capabilities

- `quiz`: add a Quiz Content Verification and Repair requirement (independent verify spawn, five-item defect contract, bounded repair via the Stage-1 trust-agent path, `content_review` marker, best-effort residual handling, default-off config gate, topic threading for the off-topic check).
- `cli`: extend Quiz Subcommand Behavior — the `quiz.content_verify` config key is read by the CLI, the persisted frontmatter carries `content_review`, and the live stream shows the extra verify/repair spawns. No new subcommand or sub-action (verify is internal); Subcommand Registration is unchanged.
- `skill-bundles`: the codebus-quiz SKILL gains a `verify:` mode defining the five-item content defect contract.
- `app-workspace`: the `spawn_quiz_generate` Tauri IPC resolves the shared `quiz.content_verify` config and threads the originating topic (from the `trigger`: `AiPlanned`→topic, `WikiPreview`→none) into `run_quiz_generate`, so the GUI runs the same content verify→repair stage and persists `content_review` (behavior parity with the CLI; no new IPC, no UI badge).

## Impact

- Affected specs: `quiz`, `cli`, `skill-bundles`, `app-workspace`
- Affected code:
  - Modified:
    - codebus-core/src/verb/quiz.rs (post-final-verify independent verify spawn + bounded repair cycle reusing the Stage-1 trust-agent path; `QuizReport` gains a content-review status; `run_quiz_generate` accepts an optional originating topic)
    - codebus-core quiz persistence (caller frontmatter gains `content_review`)
    - codebus-core quiz config loader (`quiz.content_verify` key, default false)
    - codebus-core codebus-quiz SKILL content (new `verify:` mode, five-item defect contract)
    - codebus-cli quiz command (thread the Goal-flow topic into `run_quiz_generate`)
    - codebus-app/src-tauri/src/ipc/quiz.rs (`spawn_quiz_generate` resolves shared `quiz.content_verify` + derives topic from `trigger`, injecting both into `QuizGenerateOptions` — behavior parity with the CLI)
    - codebus-cli quiz mock-claude test bin (new `quiz-verify-*` behaviors for wiring tests)
  - New: (none — no new module or subcommand)
  - Removed: (none)
