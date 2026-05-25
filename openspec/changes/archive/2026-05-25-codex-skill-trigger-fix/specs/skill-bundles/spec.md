## ADDED Requirements

### Requirement: Codex-Side SKILL Mode Invocation Trigger

When the codex provider is active and a `codebus` CLI verb (`goal`, `query`, `fix`, `chat`, or `quiz`) spawns a codex agent against an initialized vault, the SKILL bundle materialization plus the codex backend prompt composition SHALL together cause the agent to enter the verb-specific SKILL Mode workflow, not a generic task-reply mode. "Enter SKILL Mode" is defined by the per-verb observable proxy conditions in the scenarios below — codebus SHALL NOT rely on codex CLI internal state to assert this requirement; only externally observable behavior on stdout, stream events, and vault filesystem mutations counts.

The trigger mechanism (sigil form, prompt prefix, SKILL.md frontmatter shape) is an implementation detail of `codebus-core/src/agent/codex_backend.rs` plus `codebus-core/src/vault/init/skills/codex_*.rs` and MAY change across codex CLI versions without spec amendment, provided the observable proxy conditions continue to hold. When the codex CLI version in use prevents all candidate trigger mechanisms from satisfying the proxy conditions, the codebus CLI SHALL surface a non-silent error or warning on stderr identifying the failure rather than reporting success while the SKILL workflow is bypassed.

#### Scenario: Quiz plan spawn emits scope marker

- **WHEN** active provider is `codex` AND the user runs `codebus quiz "<topic>" --count <n>` against an initialized vault containing at least one wiki page on the topic
- **THEN** the first stream line of the plan spawn output SHALL be either `[CODEBUS_QUIZ_SCOPE] ...` or `[CODEBUS_QUIZ_NO_MATCH] ...`, and the codebus CLI SHALL NOT exit with the error `quiz plan spawn produced no [CODEBUS_QUIZ_SCOPE]/[CODEBUS_QUIZ_NO_MATCH] marker on any line`

#### Scenario: Goal spawn writes at least one wiki page

- **WHEN** active provider is `codex` AND the user runs `codebus goal "<task>"` against an initialized vault AND the task description plausibly implies vault content updates
- **THEN** the agent SHALL write at least one new or modified file under `<vault>/.codebus/wiki/**/*.md` observable via the agent stream's tool-call events and via filesystem inspection after the spawn completes

#### Scenario: Query spawn reads vault wiki

- **WHEN** active provider is `codex` AND the user runs `codebus query "<question>"` against an initialized vault containing wiki pages relevant to the question
- **THEN** the agent stream SHALL contain at least one tool-call event reading a file under `<vault>/.codebus/wiki/**/*.md` (via `Read`, `Glob`, `Grep`, or the codex equivalent), and the agent's final answer SHALL reference at least one vault `[[wikilink]]` or wiki page path

#### Scenario: Chat spawn does not emit vault-vs-source meta-comment

- **WHEN** active provider is `codex` AND the user runs `codebus chat` against an initialized vault and feeds a single-shot question on stdin
- **THEN** the agent's response text SHALL NOT contain a meta-comment of the form "I found this is a documentation vault rather than application source" or equivalent phrasing indicating the agent treated the vault as an unexpected workspace shape; the agent SHALL answer the question grounded in vault content without surfacing its own workspace classification

#### Scenario: Fix spawn enters fix SKILL workflow and repairs the lint warning

- **WHEN** active provider is `codex` AND the user runs `codebus fix` against an initialized vault that has at least one `codebus lint` warning
- **THEN** the agent's first reasoning or tool-call activity SHALL be scoped to repairing the identified lint warning (e.g., locating the offending wiki page, reading it, applying an edit), NOT generic "treat this as a planning task for the codebus-fix project" exploration, AND after the agent terminates the previously-failing `codebus lint` warning SHALL no longer be reported by a re-run of `codebus lint`

#### Scenario: Codex SKILL trigger failure is surfaced, not silenced

- **WHEN** active provider is `codex` AND for any of the five verbs the codex agent fails to enter SKILL Mode (proxy conditions above do not hold)
- **THEN** the codebus CLI SHALL exit with a non-zero status or emit a stderr error or warning that identifies the failing verb and points to actionable diagnostic context, and SHALL NOT print a success summary that masks the failure

