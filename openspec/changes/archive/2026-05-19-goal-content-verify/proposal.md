## Why

`quiz-content-verify` (archived) added an independent model that judges whether generated quiz **content** is appropriate, then a bounded repair loop. `goal` produces wiki knowledge pages from source and only has structural quality gates (deterministic `codebus lint` + the trust-agent `fix` loop) — nothing checks that a generated page is *faithful to the source*, *on-goal*, or *correctly placed in the taxonomy*. The user explicitly wants `goal` to gain the same independent-AI content check (this is now a real second consumer, so the shared mechanism is justified rather than speculative). Both the `codebus goal` CLI and the GUI goal flow must get it.

## What Changes

- Extract the verify→repair orchestration that currently lives inline in `run_quiz_generate` into a shared `codebus_core::verb::content_verify` core: an independent read-only verify spawn, a `Q<n> | <defect-type> | <suggestion>` / `CONTENT_OK` parser, and a caller-orchestrated bounded repair loop (hard cap 3, emit/keep-best on cap, best-effort, non-fatal). Callers inject two adapters: "obtain the content to verify" and "apply the repaired content". **quiz's externally observable behavior is unchanged** — it is refactored to consume the shared core (the parser, the cap, the `content_review` semantics stay identical).
- `run_goal` gains an optional content-verify stage that runs **after the fix loop and before `auto_commit`**, gated by a new `goal.content_verify` config key (boolean, default `false`). When enabled it: detects the wiki pages this run created/modified (via the vault git diff against the pre-run state), runs an independent verify spawn judging each changed page against a fixed three-item contract — **unfaithful** (asserts something not grounded in `raw/code/`), **off-goal** (unrelated to this run's goal), **taxonomy-misplaced** (content in the wrong page type/folder) — and on defects runs a bounded repair spawn (a Write-capable agent fed the defects, instructed to fix only the flagged pages) then re-verifies, capped at 3, best-effort.
- Residual defects after the cap are best-effort: a non-fatal warning is surfaced, the `GoalReport` carries a content-review status, no page is reverted, the verb exit code is unchanged, and `auto_commit` still runs (the partially-repaired wiki is committed as usual — content verification never blocks the commit).
- A new `goal.content_verify` config loader (mirroring `config::quiz`'s default-false, forward-compat-tolerant pattern). The codebus-goal SKILL gains a `verify:` mode (parallel to codebus-quiz Mode C) describing the three-item defect contract; the deterministic `codebus lint` / fix loop are unchanged (structural vs content is a separate concern).
- **CLI + GUI parity (both implemented in this change):** `codebus goal` resolves `goal.content_verify` from config and runs the stage; the GUI goal-spawn IPC resolves the same config and runs it too — behavior parity, no new subcommand/IPC, no UI badge (consistent with how `validation` / `content_review` are persisted but not surfaced).

## Non-Goals (optional)

(Captured in design Goals/Non-Goals.)

## Capabilities

### New Capabilities

(none — the shared verify→repair orchestration is an internal codebus-core module under the existing `verb-library` capability surface; its behavior is specified within the `verb-library` capability, not introduced as a new capability)

### Modified Capabilities

- `verb-library`: `run_goal` gains the optional independent content verify + bounded repair stage (3-defect contract, post-fix/pre-commit, default-off config gate, best-effort residual, `GoalReport` content-review status); the verify→repair orchestration is factored into a shared core that `run_quiz_generate` also consumes with its quiz behavior unchanged.
- `cli`: `codebus goal` resolves `goal.content_verify` from the shared config and runs the stage when enabled; no new subcommand; persisted/run-log behavior reflects the content-review outcome; exit code unchanged.
- `skill-bundles`: the codebus-goal SKILL gains a `verify:` mode defining the three-item content defect contract and per-page output format, kept separate from the deterministic lint/fix rules.
- `app-workspace`: the GUI goal-spawn IPC resolves `goal.content_verify` and runs the same content verify→repair stage (behavior parity with the CLI; no new IPC, no UI element).

## Impact

- Affected specs: `verb-library`, `cli`, `skill-bundles`, `app-workspace`
- Affected code:
  - New:
    - codebus-core shared content-verify module under verb-library (verify-output parser, bounded repair-loop orchestration, content-review status type)
    - codebus-core `config::goal` loader (`goal.content_verify`, default false)
  - Modified:
    - codebus-core/src/verb/quiz.rs (consume the shared core; external behavior unchanged)
    - codebus-core/src/verb/goal.rs (post-fix/pre-commit content-verify stage; `GoalReport` content-review status; changed-page detection via vault git diff)
    - codebus-core/src/skill_bundle/mod.rs (codebus-goal SKILL `verify:` mode)
    - codebus-cli/src/commands/goal.rs (resolve `goal.content_verify`, thread into `run_goal`)
    - codebus-app/src-tauri/src/ipc/goals.rs (resolve `goal.content_verify`, thread into `run_goal` — GUI parity)
    - codebus-cli/tests/bins/mock_claude.rs + goal flow tests (verify/repair mock behaviors)
  - Removed: (none)
