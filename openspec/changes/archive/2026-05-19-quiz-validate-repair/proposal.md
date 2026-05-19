## Why

`run_quiz_generate` today is a single spawn that strips fences/preamble and returns raw markdown — it has **no validation and no repair**. Malformed questions (missing `## Answer:`, fewer than 4 choices) and broken `[[slug]]` explanation citations are only caught by the frontend `parseQuiz`, which **silently drops** bad blocks at render time. The `goal` verb's output already enjoys deterministic validation (`codebus lint`) plus trust-agent repair (`codebus fix`); quiz output has neither. This change closes that parity gap so a generated quiz is structurally sound and its wiki citations resolve before it is persisted.

## What Changes

- New deterministic quiz validator in `codebus_core` (quiz markdown schema + `[[slug]]` existence against the vault wiki index), exposed as a CLI subcommand the agent can call via its Bash tool (mirroring how the codebus-fix agent calls `codebus lint`).
- `run_quiz_generate` becomes a **trust-agent flow** mirroring `v3-fix-trust-agent`: a single generate spawn whose codebus-quiz SKILL workflow self-validates and self-repairs within its own session (internal cap stated in the SKILL); the library/CLI runs the same validator once after the spawn as the **final verifier**.
- The whole `generate → (agent self-validate → self-repair) → final-verify` pipeline shares **one `on_event` stream, one events.jsonl, one RunLog**. Validator findings are emitted in the existing lint-finding event shape so the CLI stdout renderer, the GUI `QuizLiveStream`, and the 看過程 modal all present the process uniformly.
- On residual failure after the agent's internal cap, the quiz is persisted **best-effort** with a `validation:` frontmatter status marker and a non-fatal warning event — questions are never silently dropped and the verb never hard-fails to no-file.
- **Stage 2 (same change, contract-gated, deferred):** an optional independent model-verify spawn (may use a different model/effort) whose issues feed back into the Stage 1 trust-agent repair path. Its "content is acceptable" acceptance contract is **explicitly deferred** — it MUST be pinned in design before Stage 2 is implemented; Stage 1 ships and is verifiable without it.

## Capabilities

### New Capabilities

(none — this enhances the existing `quiz` verb rather than introducing a new capability)

### Modified Capabilities

- `quiz`: add a Quiz Output Validation and Repair requirement (deterministic schema + wikilink-existence validation, trust-agent self-repair, shared event/log pipeline, best-effort persistence with `validation:` marker, deferred Stage 2 model-verify).
- `cli`: add a `validate` sub-action to the `quiz` subcommand (`codebus quiz validate <file>`, human + `--json`; the eight top-level subcommands are unchanged) and the codebus-quiz agent Bash sandbox whitelist entry that hard-gates the agent to invoking that validator only.
- `skill-bundles`: the codebus-quiz per-verb workflow content gains the generate → self-validate (via Bash) → self-repair → emit loop with an internal iteration cap.
- `lint-feedback-loop`: the shared `codebus hook check-bash` PreToolUse hook (installed by `codebus init`) is extended to also permit `codebus quiz validate ...` in addition to `codebus lint ...`; without this the global vault hook would block the codebus-quiz generate agent's self-validation.

## Impact

- Affected specs: `quiz`, `cli`, `skill-bundles`, `lint-feedback-loop`
- Affected code:
  - New:
    - codebus-core quiz validator module (schema + wikilink-existence rules over the quiz markdown body)
    - codebus-cli `quiz validate` sub-action wiring
  - Modified:
    - codebus-core/src/verb/quiz.rs (trust-agent flow in `run_quiz_generate`: post-spawn final verify, finding events into the shared sink, `validation:` marker on residual failure; generate-spawn toolset + Bash whitelist constants)
    - codebus-cli quiz sub-action (stdin source for the agent self-validate path)
    - codebus-cli `codebus hook check-bash` (allow `codebus quiz validate` alongside `codebus lint`)
    - the codebus-quiz SKILL workflow content (self-validate + self-repair loop, internal cap)
  - Removed: (none)
