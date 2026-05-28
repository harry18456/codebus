## MODIFIED Requirements

### Requirement: Codex Backend Argv Composition

`CodexBackend` SHALL implement `AgentBackend::build_command` by translating the provider-neutral `SpawnSpec` into a `codex exec` invocation. The composed command SHALL always include the per-spawn isolation flags verified by the 2026-05-22 spike: `--json`, `--ignore-user-config`, `--disable apps`, `--ignore-rules`, `--skip-git-repo-check`, `--ephemeral`, a `-c project_root_markers=['<vault-marker>']` override naming a vault-unique marker file so the codex project root is pinned to the `.codebus/` vault directory, AND a `-c web_search=disabled` config override that turns off codex's hosted web search tool so the agent cannot fetch external URLs at runtime. The codex binary path SHALL be resolvable via a `CODEBUS_CODEX_BIN` environment override defaulting to `codex`.

`SpawnSpec.permission` SHALL map to the codex sandbox flag `-s`: `Permission::ReadOnly` SHALL map to `read-only` and `Permission::Workspace` SHALL map to `workspace-write`. The resolved per-verb model SHALL be passed as `-m <model>` and the resolved per-verb effort SHALL be passed as `-c model_reasoning_effort=<effort>` (codex's `.codex/config.toml` is trust-gated and SHALL NOT be relied on for these values). When `SpawnSpec.resume_session_id` is `Some(id)`, the command SHALL use the `codex exec resume <id>` subcommand form. The fully-composed prompt string SHALL be passed as the exec prompt argument, and the child process stdin SHALL be closed or fed empty input so `codex exec` does not block waiting on stdin.

`SpawnSpec.command_allowance` has no codex equivalent (codex gates command execution by sandbox, not by a per-command allowlist). When `command_allowance` is `Some(...)`, `CodexBackend` SHALL proceed without a per-command gate and SHALL emit a single warning rather than failing the spawn (no hard gate).

The `-c web_search=disabled` override is required because codex's `--disable` flag accepts only built-in sub-feature IDs (`apps`, `image_generation`, etc.) and does NOT accept `web_search`; the hosted web search tool can only be turned off through a config-key override. Without this override the agent retains the ability to fetch arbitrary URLs at runtime, which violates the codebus offline / sandbox-bounded contract. Image generation is intentionally NOT disabled by this requirement.

#### Scenario: Read-only permission maps to read-only sandbox

- **WHEN** `build_command` is called with a `SpawnSpec` whose `permission` is `Permission::ReadOnly`
- **THEN** the composed argv SHALL contain `-s read-only` and SHALL NOT contain `workspace-write` or `danger-full-access`

#### Scenario: Workspace permission maps to workspace-write sandbox

- **WHEN** `build_command` is called with a `SpawnSpec` whose `permission` is `Permission::Workspace`
- **THEN** the composed argv SHALL contain `-s workspace-write`

#### Scenario: Isolation flags always present

- **WHEN** `build_command` is called with any `SpawnSpec`
- **THEN** the composed argv SHALL contain all of `--ignore-user-config`, `--disable apps`, `--ignore-rules`, a `project_root_markers` override pinning the project root to the vault directory, AND a `-c web_search=disabled` pair that turns off codex's hosted web search tool

#### Scenario: Model and effort passed as CLI flags

- **WHEN** `build_command` resolves a verb to model `gpt-5.4` and effort `high`
- **THEN** the composed argv SHALL contain `-m gpt-5.4` and a `-c model_reasoning_effort=high` override

#### Scenario: Resume id uses the resume subcommand

- **WHEN** `build_command` is called with `resume_session_id = Some("019e-...")`
- **THEN** the composed command SHALL use the `codex exec resume 019e-...` subcommand form

#### Scenario: Command allowance degrades with a warning

- **WHEN** `build_command` is called with `command_allowance = Some(...)`
- **THEN** the spawn SHALL proceed without a per-command gate AND a single warning SHALL be emitted AND the build SHALL NOT fail
