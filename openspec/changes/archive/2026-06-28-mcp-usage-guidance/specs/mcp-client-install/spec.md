## ADDED Requirements

### Requirement: Global instruction guidance block on enable

When the user enables the MCP integration for a client, the app SHALL additionally write a codebus wiki-usage guidance block into that client's global instruction file; when the user disables it, the app SHALL remove that block. This makes the registered MCP tools discoverable to the client's agent by stating, in its always-loaded global instructions, that a cross-project wiki library is available and when to reach for it. The same guidance content SHALL be used for both clients.

The guidance SHALL be written as a marked managed block delimited by the literal markers `<!-- codebus:mcp:start -->` and `<!-- codebus:mcp:end -->`. Enabling SHALL perform an idempotent upsert: when both markers are already present and well-ordered, the app SHALL replace the existing block in place; otherwise it SHALL append the block at the end of the file separated by a blank line. Disabling SHALL remove exactly the marked block, collapsing the blank lines its removal leaves, and SHALL be a no-op when the block or the file is absent. The app SHALL preserve every byte outside the markers unchanged and SHALL write the file atomically (temporary file then rename) so a failure never leaves a half-written instruction file. When the global instruction file does not exist, enabling SHALL create it containing only the block.

The global instruction file SHALL be resolved per client: for claude, `CLAUDE.md` under `CLAUDE_CONFIG_DIR` when that environment variable is set, otherwise `CLAUDE.md` under `~/.claude`; for codex, `AGENTS.md` under `CODEX_HOME` when set, otherwise `AGENTS.md` under `~/.codex`.

The guidance-block write SHALL be subordinate to the MCP registration: the client CLI `mcp add` / `mcp remove` is authoritative, and a failure to write or remove the guidance block SHALL be surfaced as a warning but SHALL NOT roll back the registration nor fail the IPC (the command SHALL still return success when the registration itself succeeded). Each client's Settings control SHALL disclose, in visible copy, that enabling also writes — and disabling removes — a codebus guidance block in that client's global instructions.

#### Scenario: Enabling twice upserts a single block

- **WHEN** the user enables the MCP integration for a client twice in a row
- **THEN** the client's global instruction file SHALL contain exactly one codebus marked block, its content replaced in place rather than duplicated

##### Example: idempotent upsert into an empty file

- **GIVEN** the claude global instruction file does not yet exist (or is empty)
- **WHEN** the MCP integration is enabled, then enabled a second time
- **THEN** the file contains exactly one `<!-- codebus:mcp:start -->` … `<!-- codebus:mcp:end -->` block — two enables produce one block, not two

#### Scenario: Disabling removes only the marked block

- **GIVEN** a client's global instruction file contains hand-written content plus a codebus marked block
- **WHEN** the user disables the MCP integration for that client
- **THEN** the codebus marked block SHALL be removed AND every byte of hand-written content outside the markers SHALL remain unchanged

#### Scenario: Per-client file resolution honors environment overrides

- **WHEN** the guidance block is written for the codex client AND `CODEX_HOME` is set
- **THEN** the block SHALL be written to `AGENTS.md` under `CODEX_HOME`, and for the claude client to `CLAUDE.md` under `CLAUDE_CONFIG_DIR` when set, each falling back to `~/.codex` / `~/.claude` when the override is unset

#### Scenario: Guidance write failure does not fail registration

- **WHEN** the client CLI registration succeeds but writing the guidance block fails
- **THEN** the command SHALL surface a warning AND SHALL return success AND SHALL NOT undo the registration

#### Scenario: Settings discloses the global-instruction write

- **WHEN** the Settings MCP-integration control for a client renders
- **THEN** it SHALL display copy stating that enabling also writes, and disabling removes, a codebus guidance block in that client's global instructions
