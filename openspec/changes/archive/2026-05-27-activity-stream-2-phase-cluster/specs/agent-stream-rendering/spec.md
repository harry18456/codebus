## ADDED Requirements

### Requirement: Stream Event Tool Classification

The `StreamEvent::ToolUse` variant SHALL carry an optional `tool_kind` field that classifies the tool call into one of five semantic categories used by downstream renderers (CLI and GUI) to group tool calls into READING CODEBASE and WRITING WIKI phase clusters. The classification SHALL be defined as a separate enum `ToolKind` with `serde(rename_all = "snake_case")`:

| Variant | Semantic | Examples |
| --- | --- | --- |
| `Read` | Read file content / list directory / pattern match | `cat`, `head`, `ls`, `find`, `rg`, `grep`, `wc` |
| `Inspect` | Read environment / neutral introspection, no file content | `git status`, `git log`, `git diff`, `ps`, `env`, `which`, `npm ls` |
| `Mutation` | Mutate state (file / git / system / install) | `rm`, `mv`, `mkdir`, `git commit`, `npm install`, redirection writes |
| `OtherRead` | Future / unknown tool, read-like default | (extension hook for new tools not in the canonical table) |
| `OtherWrite` | Future / unknown tool, write-like default | (extension hook for new tools not in the canonical table) |

The field SHALL be named `tool_kind` (NOT `kind`) because `StreamEvent` already uses `#[serde(tag = "kind", rename_all = "snake_case")]` for its variant discriminator and a second `kind` field would collide. The field SHALL be `Option<ToolKind>` so that legacy events emitted before this requirement landed remain deserializable; the parser SHALL NOT reject a `ToolUse` event that omits `tool_kind`.

The classification SHALL be supplied by the emitting agent skill (e.g. `codebus-goal`), NOT inferred by the parser. When the emitter cannot determine intent, it SHALL default to `Inspect` (the safest unknown — `Inspect` groups under READING CODEBASE and avoids the false signal of `Mutation` causing the user to think the agent wrote something).

#### Scenario: ToolUse round-trips with tool_kind preserved

- **WHEN** the parser receives the line `{"type":"assistant","message":{"content":[{"type":"tool_use","name":"Bash","input":{"command":"git status"},"tool_kind":"inspect"}]}}`
- **THEN** the returned vec SHALL contain exactly one `ToolUse { name: "Bash", input: {"command":"git status"}, tool_kind: Some(ToolKind::Inspect) }` event

##### Example: full enum coverage

| `tool_kind` JSON value | Rust variant |
| --- | --- |
| `"read"` | `ToolKind::Read` |
| `"inspect"` | `ToolKind::Inspect` |
| `"mutation"` | `ToolKind::Mutation` |
| `"other_read"` | `ToolKind::OtherRead` |
| `"other_write"` | `ToolKind::OtherWrite` |

#### Scenario: ToolUse without tool_kind deserializes as None

- **WHEN** the parser receives the line `{"type":"assistant","message":{"content":[{"type":"tool_use","name":"Read","input":{"file_path":"/x.rs"}}]}}`
- **THEN** the returned vec SHALL contain exactly one `ToolUse { name: "Read", input: {"file_path":"/x.rs"}, tool_kind: None }` event AND the parser SHALL NOT log a warning or error

#### Scenario: Invalid tool_kind value is rejected for the entire event

- **WHEN** the parser receives a line whose `tool_kind` value is the string `"garbage"` not present in the enum
- **THEN** the parser SHALL return zero events for that line AND SHALL NOT panic AND SHALL NOT propagate an `Err`

#### Scenario: Codex parser forwards tool_kind identically

- **WHEN** the Codex stream parser receives an event with `tool_kind: "mutation"`
- **THEN** the resulting `StreamEvent::ToolUse` event SHALL carry `tool_kind: Some(ToolKind::Mutation)` with the same value the Claude parser would have produced
