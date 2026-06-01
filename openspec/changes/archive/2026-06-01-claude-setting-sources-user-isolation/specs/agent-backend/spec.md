## MODIFIED Requirements

### Requirement: Claude Backend Argv Equivalence

`ClaudeBackend` SHALL implement `AgentBackend`. For any `SpawnSpec`, `ClaudeBackend::build_command` SHALL produce a `claude` argv equal to the pre-refactor `build_claude_cmd` for the corresponding inputs, EXCEPT for the additive `--no-session-persistence` flag introduced by the session-persistence gating below AND the additive `--setting-sources project,local` flag introduced by the user-global isolation below. This SHALL include: the `-p /codebus-<verb> "<input>"` slash invocation, the `--tools` / `--allowedTools` / `--permission-mode acceptEdits` flags, the MCP isolation flags (`--strict-mcp-config` plus an empty `--mcp-config`), the user-global setting-source isolation flag (`--setting-sources project,local`), the `--model` / `--effort` flags resolved from config, and `--resume <id>` placement before the toolset flags when `resume_session_id` is `Some`. `ClaudeBackend::parse_stream_line` and `extract_session_id` SHALL produce results identical to the pre-refactor `parse_claude_stream_line` and `sniff_init_session_id`.

`ClaudeBackend::build_command` SHALL gate Claude CLI session persistence on the spawn's verb, mirroring the codex backend's `--ephemeral` gate: for every verb OTHER THAN `Verb::Chat` (i.e. `Goal` / `Query` / `Fix` / `Quiz`, including cross-flow `Verify` spawns) the argv SHALL include the `--no-session-persistence` flag, so these single-shot verbs (which never resume) leave no Claude session rollout on disk. For `Verb::Chat` the argv SHALL NOT include `--no-session-persistence`, because chat is multi-turn and depends on session persistence for `--resume <id>` to continue the conversation. The `--no-session-persistence` flag SHALL be valid only because codebus always spawns Claude in `-p` (print) mode.

`ClaudeBackend::build_command` SHALL hard-isolate the user-global setting layer by unconditionally including `--setting-sources project,local` in the argv, placed AFTER the MCP isolation flags (`--strict-mcp-config` / `--mcp-config`) and BEFORE the optional `--model` flag. This restricts Claude's loaded setting sources to the vault's own project and local layers, excluding the user-global layer (`~/.claude/CLAUDE.md`, `~/.claude/settings.json`, and user-global plugins) so user-global instructions do not bleed into wiki-building agents. The flag SHALL apply to every verb (no escape hatch), mirroring the codex backend's `--ignore-user-config` user isolation. The vault's own project-layer settings — the `.codebus/.claude/settings.json` `check-bash` / `check-read` PreToolUse hook gate and the `.codebus/CLAUDE.md` schema — SHALL remain in effect under this flag.

#### Scenario: Read-only permission excludes write tools

- **WHEN** `build_command` is called with `permission: ReadOnly` and no `command_allowance`
- **THEN** the `--tools` value SHALL contain the read-only tool set (Read / Glob / Grep) AND SHALL NOT contain `Write`, `Edit`, or `Bash`

#### Scenario: command_allowance maps to fine-grained Bash specifier

- **WHEN** `build_command` is called with `command_allowance: Some(["codebus","quiz","validate"])`
- **THEN** the `--allowedTools` value SHALL contain `Bash(codebus quiz validate *)` AND the `--tools` value SHALL contain bare `Bash`

#### Scenario: Argv equals pre-refactor builder except the additive isolation flags

- **WHEN** a `SpawnSpec` is constructed for a goal spawn (`verb: Goal, permission: Workspace`, model/effort resolved)
- **THEN** the argv produced by `ClaudeBackend::build_command` SHALL equal, token-for-token, the argv the pre-refactor `build_claude_cmd` produced for the equivalent `InvokeAgentOptions`, with the addition of the `--no-session-persistence` flag AND the `--setting-sources project,local` flag

#### Scenario: Resume id placed before toolset flags

- **WHEN** `build_command` is called with `resume_session_id: Some("abc-123")`
- **THEN** `--resume abc-123` SHALL appear in the argv before the `--tools` flag

#### Scenario: Single-shot verbs include no-session-persistence

- **WHEN** `build_command` is called for a `SpawnSpec` with `verb` in {`Goal`, `Query`, `Fix`, `Quiz`}
- **THEN** the produced argv SHALL include the `--no-session-persistence` flag

#### Scenario: Chat verb omits no-session-persistence so resume works

- **WHEN** `build_command` is called for a `SpawnSpec` with `verb: Chat` and `resume_session_id: Some("abc-123")`
- **THEN** the produced argv SHALL NOT include `--no-session-persistence` AND SHALL include `--resume abc-123`

#### Scenario: User-global setting sources are excluded

- **WHEN** `build_command` is called for any `SpawnSpec`
- **THEN** the produced argv SHALL include `--setting-sources` with the value `project,local` AND the `--setting-sources` flag SHALL appear after `--strict-mcp-config` AND before `--model` (when `--model` is present)
