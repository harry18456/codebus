## ADDED Requirements

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

### Requirement: Tools declare their auditable field whitelist

Each tool registered with `ToolSandbox` SHALL declare an `audit_fields: list[str]` attribute at registration. The sandbox SHALL raise a `ValueError` at registration time if the attribute is missing or is not a list of strings.

#### Scenario: Tool without audit_fields rejected at registration

- **WHEN** a tool is registered whose class lacks an `audit_fields` attribute
- **THEN** the registration call MUST raise `ValueError` whose message names the tool class and the missing attribute

#### Scenario: Empty audit_fields permitted

- **WHEN** a tool declares `audit_fields = []`
- **THEN** registration MUST succeed
- **AND** resulting `args_summary` objects MUST equal `{}` on every invocation

### Requirement: Schema version on every tool audit line

Every line written to `tool_audit.jsonl` by this change SHALL include `"schema_version": 1`. Future changes that add fields MUST either keep `schema_version` at `1` when the addition is additive (new keys alongside existing) or bump to `2` when the semantic of an existing field changes.

#### Scenario: Initial schema version

- **WHEN** the first tool invocation in a workspace writes an audit line
- **THEN** the line's `schema_version` field MUST equal `1`
