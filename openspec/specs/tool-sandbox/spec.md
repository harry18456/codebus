# tool-sandbox Specification

## Purpose

TBD - created by archiving change 'm1-power-on'. Update Purpose after archive.

## Requirements

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


<!-- @trace
source: m1-power-on
updated: 2026-04-19
code:
  - web/dist
-->

---
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


<!-- @trace
source: m1-power-on
updated: 2026-04-19
code:
  - web/dist
-->

---
### Requirement: Red team fixture covers known attack vectors

The repository SHALL contain a red-team test fixture that exercises every attack vector listed in `docs/tool-sandbox.md §十五`, per design decision D-local-3.

#### Scenario: All attack vectors present in fixture

- **WHEN** the red-team fixture is enumerated
- **THEN** it MUST include at least one case for each of: relative `..` escape, absolute path outside workspace, symlink escape, Windows junction escape, UNC path, `\\\\?\\` long-path prefix, case-only variants, and trailing-dot or trailing-space filename variants

#### Scenario: Red team suite runs and passes

- **WHEN** `uv run pytest tests/sandbox/` is executed
- **THEN** every red-team case MUST pass, meaning each attack path MUST be rejected by `ensure_in_workspace`

<!-- @trace
source: m1-power-on
updated: 2026-04-19
code:
  - web/dist
-->

---
### Requirement: ToolSandbox appends every invocation to tool_audit.jsonl

The `ToolSandbox` SHALL write one JSON object per tool invocation to `{workspace}/.codebus/tool_audit.jsonl` regardless of whether the invocation succeeded, was denied by `ensure_in_workspace`, or raised. The file SHALL be append-only and each line SHALL be a single JSON object terminated by `\n`, per `docs/security.md §四` and `docs/decisions.md` D-017.

#### Scenario: Successful invocation writes allowed audit line

- **WHEN** a registered tool is invoked with arguments whose paths all pass `ensure_in_workspace`
- **AND** the tool body executes without raising
- **THEN** `tool_audit.jsonl` MUST have exactly one appended line with `"allowed": true`, `"denial_reason": null`, and a `"resolved_path"` field set to the normalized absolute path returned by `ensure_in_workspace`

#### Scenario: Denied invocation writes denial audit line

- **WHEN** a tool is invoked with a path that `ensure_in_workspace` rejects as a sandbox violation
- **THEN** `tool_audit.jsonl` MUST have exactly one appended line with `"allowed": false` and a non-null `"denial_reason"` drawn from the closed set `path_escape` / `symlink_outside` / `unc_path` / `long_path_prefix_invalid` / `case_variant` / `trailing_whitespace`
- **AND** the tool body MUST NOT be executed

#### Scenario: Audit line contains required fields

- **WHEN** any tool invocation produces an audit line
- **THEN** the line MUST contain fields `ts` (ISO 8601 UTC), `schema_version` (integer), `workspace_type` (`"folder"` or `"topic"`), `tool_name` (string), `args_summary` (object), `resolved_path` (string or null), `allowed` (boolean), `denial_reason` (string or null), and `session_id` (UUID string)

#### Scenario: args_summary excludes sensitive values

- **WHEN** a tool is invoked with an argument named `query` that contains free-form text
- **AND** that argument is NOT declared in the tool's `audit_fields` whitelist
- **THEN** the `args_summary` object MUST NOT include the raw `query` value
- **AND** MUST instead include only the keys listed in the tool's `audit_fields` (for example `{"path": "src/app.py"}`)

<!-- @trace
source: sanitizer-safety-chain
updated: 2026-04-21
code:
  - sidecar/src/codebus_agent/sandbox.py
-->


<!-- @trace
source: sanitizer-safety-chain
updated: 2026-04-21
code:
  - sidecar/src/codebus_agent/sandbox.py
  - sidecar/src/codebus_agent/providers/tracked.py
  - sidecar/uv.lock
  - sidecar/src/codebus_agent/sanitizer/engine.py
  - sidecar/src/codebus_agent/sanitizer/audit.py
  - sidecar/src/codebus_agent/sanitizer/__init__.py
  - sidecar/src/codebus_agent/sanitizer/config.py
  - sidecar/pyproject.toml
  - sidecar/src/codebus_agent/sanitizer/rules.py
tests:
  - sidecar/tests/providers/test_registry_guard_roles.py
  - sidecar/tests/providers/test_tracked_provider.py
  - sidecar/tests/sanitizer/fixtures/internal_ids_sample.txt
  - sidecar/tests/sandbox/test_tool_audit.py
  - sidecar/tests/sanitizer/fixtures/pii_sample.txt
  - sidecar/tests/sanitizer/test_config.py
  - sidecar/tests/sanitizer/test_audit.py
  - sidecar/tests/providers/test_no_outbound_per_role.py
  - sidecar/tests/providers/test_registry_role_dispatch.py
  - sidecar/tests/sanitizer/__init__.py
  - sidecar/tests/providers/test_tracked_role_audit.py
  - sidecar/tests/providers/test_tracked_pass2.py
  - sidecar/tests/sanitizer/test_allowlist.py
  - sidecar/tests/sanitizer/test_rules.py
  - sidecar/tests/test_sanitizer_safety_chain_integration.py
  - sidecar/tests/sanitizer/fixtures/secret_sample.txt
  - sidecar/tests/test_phase9_jsonl_acceptance.py
  - sidecar/tests/providers/test_registry.py
  - sidecar/tests/sanitizer/test_engine.py
-->

---
### Requirement: Tools declare their auditable field whitelist

Each tool registered with `ToolSandbox` SHALL declare an `audit_fields: list[str]` attribute at registration. The sandbox SHALL raise a `ValueError` at registration time if the attribute is missing or is not a list of strings.

#### Scenario: Tool without audit_fields rejected at registration

- **WHEN** a tool is registered whose class lacks an `audit_fields` attribute
- **THEN** the registration call MUST raise `ValueError` whose message names the tool class and the missing attribute

#### Scenario: Empty audit_fields permitted

- **WHEN** a tool declares `audit_fields = []`
- **THEN** registration MUST succeed
- **AND** resulting `args_summary` objects MUST equal `{}` on every invocation

<!-- @trace
source: sanitizer-safety-chain
updated: 2026-04-21
code:
  - sidecar/src/codebus_agent/sandbox.py
-->


<!-- @trace
source: sanitizer-safety-chain
updated: 2026-04-21
code:
  - sidecar/src/codebus_agent/sandbox.py
  - sidecar/src/codebus_agent/providers/tracked.py
  - sidecar/uv.lock
  - sidecar/src/codebus_agent/sanitizer/engine.py
  - sidecar/src/codebus_agent/sanitizer/audit.py
  - sidecar/src/codebus_agent/sanitizer/__init__.py
  - sidecar/src/codebus_agent/sanitizer/config.py
  - sidecar/pyproject.toml
  - sidecar/src/codebus_agent/sanitizer/rules.py
tests:
  - sidecar/tests/providers/test_registry_guard_roles.py
  - sidecar/tests/providers/test_tracked_provider.py
  - sidecar/tests/sanitizer/fixtures/internal_ids_sample.txt
  - sidecar/tests/sandbox/test_tool_audit.py
  - sidecar/tests/sanitizer/fixtures/pii_sample.txt
  - sidecar/tests/sanitizer/test_config.py
  - sidecar/tests/sanitizer/test_audit.py
  - sidecar/tests/providers/test_no_outbound_per_role.py
  - sidecar/tests/providers/test_registry_role_dispatch.py
  - sidecar/tests/sanitizer/__init__.py
  - sidecar/tests/providers/test_tracked_role_audit.py
  - sidecar/tests/providers/test_tracked_pass2.py
  - sidecar/tests/sanitizer/test_allowlist.py
  - sidecar/tests/sanitizer/test_rules.py
  - sidecar/tests/test_sanitizer_safety_chain_integration.py
  - sidecar/tests/sanitizer/fixtures/secret_sample.txt
  - sidecar/tests/test_phase9_jsonl_acceptance.py
  - sidecar/tests/providers/test_registry.py
  - sidecar/tests/sanitizer/test_engine.py
-->

---
### Requirement: Schema version on every tool audit line

Every line written to `tool_audit.jsonl` by this change SHALL include `"schema_version": 1`. Future changes that add fields MUST either keep `schema_version` at `1` when the addition is additive (new keys alongside existing) or bump to `2` when the semantic of an existing field changes.

#### Scenario: Initial schema version

- **WHEN** the first tool invocation in a workspace writes an audit line
- **THEN** the line's `schema_version` field MUST equal `1`

<!-- @trace
source: sanitizer-safety-chain
updated: 2026-04-21
code:
  - sidecar/src/codebus_agent/sandbox.py
-->

<!-- @trace
source: sanitizer-safety-chain
updated: 2026-04-21
code:
  - sidecar/src/codebus_agent/sandbox.py
  - sidecar/src/codebus_agent/providers/tracked.py
  - sidecar/uv.lock
  - sidecar/src/codebus_agent/sanitizer/engine.py
  - sidecar/src/codebus_agent/sanitizer/audit.py
  - sidecar/src/codebus_agent/sanitizer/__init__.py
  - sidecar/src/codebus_agent/sanitizer/config.py
  - sidecar/pyproject.toml
  - sidecar/src/codebus_agent/sanitizer/rules.py
tests:
  - sidecar/tests/providers/test_registry_guard_roles.py
  - sidecar/tests/providers/test_tracked_provider.py
  - sidecar/tests/sanitizer/fixtures/internal_ids_sample.txt
  - sidecar/tests/sandbox/test_tool_audit.py
  - sidecar/tests/sanitizer/fixtures/pii_sample.txt
  - sidecar/tests/sanitizer/test_config.py
  - sidecar/tests/sanitizer/test_audit.py
  - sidecar/tests/providers/test_no_outbound_per_role.py
  - sidecar/tests/providers/test_registry_role_dispatch.py
  - sidecar/tests/sanitizer/__init__.py
  - sidecar/tests/providers/test_tracked_role_audit.py
  - sidecar/tests/providers/test_tracked_pass2.py
  - sidecar/tests/sanitizer/test_allowlist.py
  - sidecar/tests/sanitizer/test_rules.py
  - sidecar/tests/test_sanitizer_safety_chain_integration.py
  - sidecar/tests/sanitizer/fixtures/secret_sample.txt
  - sidecar/tests/test_phase9_jsonl_acceptance.py
  - sidecar/tests/providers/test_registry.py
  - sidecar/tests/sanitizer/test_engine.py
-->
