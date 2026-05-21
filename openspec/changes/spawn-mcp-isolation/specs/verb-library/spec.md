## ADDED Requirements

### Requirement: Agent Spawn MCP Isolation

The `codebus_core::agent::claude_cli::build_claude_cmd` function — the single production code path that spawns the `claude` CLI for every verb (goal, query, fix, chat, quiz, and the goal content-verify spawn), used by both the CLI and the desktop app — SHALL compose the spawned `claude` command's arguments to include MCP load-layer isolation flags so that NO ambient Model Context Protocol (MCP) server is loaded into the spawned agent session.

Specifically, the composed argument vector SHALL include `--strict-mcp-config` and SHALL include `--mcp-config` immediately followed by the literal empty-configuration argument `{"mcpServers":{}}` (passed as a single argument value via `Command::arg`, not through a shell and not as a file path). With `--strict-mcp-config` present, the `claude` process SHALL use only the MCP servers from the supplied `--mcp-config`; because that configuration declares zero servers, no user-scope (`~/.claude.json`), project-scope (`.mcp.json`), or connector-scope MCP server SHALL be loaded, and no `mcp__*` tool SHALL be exposed to the agent.

These two flags SHALL be positioned after `--verbose` and before the optional `--model` / `--effort` flags. This isolation SHALL be unconditional: it applies to every toolset and every model/effort combination, with no configuration option or environment variable to disable it.

The pre-existing argument-order invariant SHALL be preserved: when `InvokeAgentOptions.resume_session_id` is `Some(id)`, `--resume <id>` SHALL still appear before `--tools`; the added MCP flags appear after `--tools` and therefore do not affect that relationship.

The `--tools` and `--allowedTools` flags continue to gate built-in tools only; the MCP isolation flags are the mechanism that gates the MCP load layer (the toolset flags do not, and SHALL NOT be relied upon to, exclude MCP tools).

#### Scenario: Spawn argv carries MCP isolation flags

- **WHEN** `build_claude_cmd` composes the command for any verb spawn
- **THEN** the resulting argument vector SHALL contain `--strict-mcp-config` AND SHALL contain `--mcp-config` immediately followed by the argument `{"mcpServers":{}}`

#### Scenario: MCP isolation flags positioned after toolset flags and before model flags

- **WHEN** `build_claude_cmd` composes the command with `model: Some("claude-opus-4-6")` and `effort: Some("high")`
- **THEN** `--strict-mcp-config` and `--mcp-config` SHALL appear after `--verbose` AND before `--model`

#### Scenario: MCP isolation does not break the resume-before-tools invariant

- **WHEN** `build_claude_cmd` composes the command with `resume_session_id: Some("abc-123")`
- **THEN** `--resume abc-123` SHALL appear before `--tools` AND the `--strict-mcp-config` / `--mcp-config` flags SHALL appear after `--tools`

#### Scenario: Spawned agent exposes no MCP tools

- **WHEN** a `claude -p` process is spawned via the flags `build_claude_cmd` produces, in an environment where ambient MCP servers (user-scope, connector-scope, or a project `.mcp.json`) are configured
- **THEN** the spawned session's reported tool set SHALL contain no `mcp__*` tool AND the reported MCP server list SHALL be empty, while the built-in tools permitted by `--tools` remain available
