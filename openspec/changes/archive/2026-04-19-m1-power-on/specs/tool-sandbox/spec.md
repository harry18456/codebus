## ADDED Requirements

### Requirement: ToolContext carries workspace type discriminator

The sidecar SHALL define a `ToolContext` Pydantic model that includes a `workspace_type` field typed as `Literal["folder", "topic"]`, per `docs/decisions.md` D-002 and D-023.

#### Scenario: Folder workspace accepted

- **WHEN** a `ToolContext` is constructed with `workspace_type="folder"`
- **THEN** the model MUST validate without raising

#### Scenario: Topic workspace accepted at schema level

- **WHEN** a `ToolContext` is constructed with `workspace_type="topic"`
- **THEN** the model MUST validate without raising, even though topic-mode tool behavior is not yet implemented

#### Scenario: Invalid workspace type rejected

- **WHEN** a `ToolContext` is constructed with any string other than `"folder"` or `"topic"`
- **THEN** Pydantic MUST raise a validation error

### Requirement: ensure_in_workspace blocks path escape

The sidecar SHALL expose a helper `ensure_in_workspace(path, ctx)` that raises a sandbox violation when `path` resolves outside `ctx.workspace_root`, per `docs/tool-sandbox.md §二` and design decision D-local-3.

#### Scenario: In-scope path accepted

- **WHEN** `ensure_in_workspace` is called with a path inside the workspace root
- **THEN** it MUST return a normalized absolute `Path` rooted under the workspace

#### Scenario: Parent-directory escape rejected

- **WHEN** `ensure_in_workspace` is called with a relative path containing `..` segments that resolve outside the workspace
- **THEN** it MUST raise a sandbox violation error and MUST NOT return

#### Scenario: Symlink escape rejected

- **WHEN** the caller passes a path to a symlink whose target lies outside the workspace
- **THEN** `ensure_in_workspace` MUST resolve the symlink and MUST raise a sandbox violation

#### Scenario: Windows UNC path rejected

- **WHEN** `ensure_in_workspace` is called with a UNC path (for example `\\\\server\\share\\file`) on Windows, and the UNC target is not inside the workspace
- **THEN** it MUST raise a sandbox violation

#### Scenario: Windows long-path prefix normalized

- **WHEN** `ensure_in_workspace` is called with a path using the `\\\\?\\` long-path prefix pointing inside the workspace
- **THEN** it MUST normalize the prefix and MUST accept the path

### Requirement: Red team fixture covers known attack vectors

The repository SHALL contain a red-team test fixture that exercises every attack vector listed in `docs/tool-sandbox.md §十五`, per design decision D-local-3.

#### Scenario: All attack vectors present in fixture

- **WHEN** the red-team fixture is enumerated
- **THEN** it MUST include at least one case for each of: relative `..` escape, absolute path outside workspace, symlink escape, Windows junction escape, UNC path, `\\\\?\\` long-path prefix, case-only variants, and trailing-dot or trailing-space filename variants

#### Scenario: Red team suite runs and passes

- **WHEN** `uv run pytest tests/sandbox/` is executed
- **THEN** every red-team case MUST pass, meaning each attack path MUST be rejected by `ensure_in_workspace`
