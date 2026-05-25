## ADDED Requirements

### Requirement: Codex Multi-Line Prompt Stdin Routing

The codex backend SHALL route any prompt that contains a newline character (`\n`) through the child process's stdin pipe instead of the prompt argv element, because Rust's standard library rejects newline-containing argv elements with `InvalidInput: batch file arguments are invalid` since 1.77 when the spawned executable resolves to a Windows `.cmd` or `.bat` shim — and codex's npm install on Windows is exactly such a `.cmd` shim. The routing decision SHALL be a pure function of the formatted prompt (single function, `format_codex_prompt`, shared by `build_command` and `stdin_payload` so they cannot disagree).

When the formatted prompt contains a newline:

1. `CodexBackend::build_command` SHALL pass `-` (a single hyphen) as the prompt argv element. `codex exec` interprets `-` as "read the prompt from stdin" per its CLI contract.
2. `CodexBackend::stdin_payload(spec)` SHALL return `Some(formatted_prompt)` so the invocation loop pipes stdin and writes the payload.
3. No argv element SHALL contain `\n`, `\r`, or `\0`, regardless of which `-c` config overrides or model flags are present.

When the formatted prompt is single-line (no `\n`):

1. `CodexBackend::build_command` SHALL pass the formatted prompt as the final argv element (preserving the existing visible-argv contract used by tests and observability tools).
2. `CodexBackend::stdin_payload(spec)` SHALL return `None` so the invocation loop keeps stdin closed (the historical codex contract — codex exec blocks waiting on stdin when given an open empty pipe with no data).

The verify-stage and repair-stage spawns from the `verb::goal` and `verb::quiz` content-verify pipeline routinely produce multi-line prompts (`goal=<task>\n\nCHANGED PAGES:\n<paths>` and `goal=<task>\n\nCONTENT DEFECTS:\n<defects>\n\nFLAGGED PAGES:\n<pages>`); these SHALL route through stdin and SHALL NOT fail with the historical `spawn agent: batch file arguments are invalid` error.

#### Scenario: Multi-line verify prompt is passed via stdin

- **WHEN** `CodexBackend::build_command` is invoked with a `SpawnSpec` whose `sub_mode = Some("verify")` and whose `input` contains newlines (e.g. `goal=X\n\nCHANGED PAGES:\nwiki/a.md`)
- **THEN** the composed argv's final element SHALL be `-`, no argv element SHALL contain `\n`, and `CodexBackend::stdin_payload(&spec)` SHALL return `Some("$codebus-goal verify: goal=X\n\nCHANGED PAGES:\nwiki/a.md")` so the invocation loop writes that string to the child's stdin

#### Scenario: Single-line plan prompt stays in argv

- **WHEN** `CodexBackend::build_command` is invoked with a `SpawnSpec` whose `input` contains no newlines (e.g. `JWT issuance and verification`)
- **THEN** the composed argv's final element SHALL be the fully formatted prompt (e.g. `$codebus-quiz plan: JWT issuance and verification`), AND `CodexBackend::stdin_payload(&spec)` SHALL return `None` (stdin stays closed)

#### Scenario: codebus verify-stage spawn does not fail with batch-file argv error

- **WHEN** `codebus goal` or `codebus quiz` runs against an initialized vault on Windows with `active_provider=codex` AND the content-verify stage assembles its multi-line prompt
- **THEN** the spawn SHALL NOT print `spawn agent: batch file arguments are invalid` (or the underlying `InvalidInput` from Rust's stdlib) AND the verify stage SHALL execute and emit either `CONTENT_OK` or `<id> | <defect-type> | <suggestion>` lines per the `verb::content_verify` line grammar

### Requirement: Codex Sandbox Write Enablement Override

The codex backend's `build_command` SHALL inject a platform-specific sandbox elevation override into the per-spawn argv whenever the resolved `SpawnSpec.permission` is `Permission::Workspace`, so that the codex agent's filesystem sandbox actually permits writes to the vault working directory. The override SHALL be passed as an additional `-c <key>=<value>` argument (unquoted; codex parses each `-c` value as TOML and falls back to a literal string when the value is not valid TOML, so the bare identifier becomes a string literal without any embedded quotes that would interfere with the Windows `.cmd` shim's re-quoting) alongside the existing isolation recipe flags (`--ignore-user-config`, `--disable apps`, `--ignore-rules`, `-s workspace-write`, `-c project_root_markers=...`) without removing or weakening any of them.

This requirement exists because `--ignore-user-config` strips the user's per-platform sandbox enablement default from the loaded config, which leaves codex's sandbox at a conservative read-only baseline even when the CLI flag `-s workspace-write` is also present. The override re-establishes the write-capable baseline per-spawn without re-introducing any user-config-derived MCP servers, plugins, personality presets, or trust list.

On Windows, the override SHALL be `-c windows.sandbox=unelevated`. The value `unelevated` (not `elevated`) is required so that codex's Windows sandbox runs as the current user; `elevated` requires the parent process to already be admin and aborts subprocess spawning otherwise (`windows sandbox: spawn setup refresh` error). Codex 0.133.0 accepts only `elevated` or `unelevated` for `windows.sandbox`; other values are rejected with `unknown variant`. On macOS and Linux, the corresponding override may be a no-op or may require a different key; behavior on those platforms is intentionally deferred and SHALL be considered a follow-up change once those platforms are exercised. The codex backend SHALL pass the Windows override unconditionally when constructing the argv on any host platform — codex's TOML config tolerates unknown-platform tables, and the override is harmless on non-Windows hosts.

When `SpawnSpec.permission` is `Permission::ReadOnly`, the override SHALL still be safe to include (codex's sandbox stays read-only because `-s read-only` takes effect; the elevation override only applies to the workspace-write path).

#### Scenario: Workspace permission argv includes sandbox elevation override

- **WHEN** `CodexBackend::build_command` is invoked with a `SpawnSpec` whose `permission` is `Permission::Workspace`
- **THEN** the composed argv SHALL contain a `-c windows.sandbox=unelevated` pair (the `-c` flag followed by the `windows.sandbox=unelevated` value) alongside the existing `-s workspace-write` mapping, and SHALL NOT contain `--dangerously-bypass-approvals-and-sandbox` or any equivalent sandbox-bypass flag

#### Scenario: Read-only permission argv still includes the override harmlessly

- **WHEN** `CodexBackend::build_command` is invoked with a `SpawnSpec` whose `permission` is `Permission::ReadOnly`
- **THEN** the composed argv SHALL contain `-s read-only`, MAY contain the `-c windows.sandbox=unelevated` override (codex's sandbox stays read-only because the sandbox-mode flag governs), and SHALL NOT contain `--dangerously-bypass-approvals-and-sandbox`

#### Scenario: Workspace-write spawn against initialized vault actually writes

- **WHEN** active provider is `codex` AND the user runs a `Permission::Workspace` verb (`goal` or `fix`) against an initialized vault on Windows AND the agent attempts an `apply_patch` or equivalent file write under the vault directory
- **THEN** the write SHALL succeed and the resulting file change SHALL be observable via filesystem inspection after the spawn completes, and the agent SHALL NOT self-classify as "read-only filesystem sandbox" or "approvals are disabled" in its final message
