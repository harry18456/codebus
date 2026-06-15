## MODIFIED Requirements

### Requirement: Codex Stream Parsing

`CodexBackend` SHALL implement `AgentBackend::parse_stream_line` as a format-only mapping from one line of codex `--json` JSONL output to zero or more normalized `StreamEvent` values. A `codex exec --json` line of type `item.completed` whose `item.type` is `command_execution` SHALL map to a `StreamEvent::ToolUse { name: "Shell", input }` carrying the `command` field followed by a `StreamEvent::ToolResult { output, is_error }` where `output` is the `aggregated_output` field and `is_error` is `true` when `exit_code` is non-zero. A line of type `item.completed` whose `item.type` is `agent_message` SHALL map to a `StreamEvent::Thought` carrying the `text` field. A line of type `turn.completed` with a `usage` object SHALL map to a `StreamEvent::Usage` whose token counts are taken from the `usage` object's `input_tokens`, `cached_input_tokens`, `output_tokens`, and `reasoning_output_tokens` fields (mapped to the corresponding `TokenUsage` fields, including the reasoning-token field). A line of type `turn.completed` without a `usage` object SHALL return zero `StreamEvent` values and SHALL NOT emit a usage warning. Lines of type `thread.started`, `turn.started`, and `item.started` SHALL NOT produce a `StreamEvent`. `parse_stream_line` SHALL NOT interpret codebus-semantic `[CODEBUS_*]` markers - those remain a verb-layer concern.

When a `turn.completed` line contains a `usage` object but none of the expected usage fields (`input_tokens`, `cached_input_tokens`, `output_tokens`, `reasoning_output_tokens`) decode as an unsigned integer, the parser SHALL emit exactly one stderr warning prefixed with `warning: codex usage` and SHALL still return the normalized `StreamEvent::Usage` produced by the existing mapping rules. The warning SHALL NOT include the verbatim `usage` JSON object. When at least one expected usage field decodes as an unsigned integer, including a decoded value of zero, the parser SHALL NOT emit this missing-fields warning.

`CodexBackend` SHALL implement `AgentBackend::extract_session_id` to return `Some(id)` for a line of type `thread.started` carrying a `thread_id`, and `None` for every other line.

#### Scenario: Command execution maps to a ToolUse and ToolResult pair

- **WHEN** `parse_stream_line` receives an `item.completed` line with `item.type = command_execution`, `command = "echo hi"`, `aggregated_output = "hi\n"`, and `exit_code = 0`
- **THEN** it SHALL return a `StreamEvent::ToolUse { name: "Shell", ... }` carrying the command followed by a `StreamEvent::ToolResult { output: "hi\n", is_error: false }`

#### Scenario: Non-zero exit code marks the tool result as an error

- **WHEN** `parse_stream_line` receives an `item.completed` `command_execution` line with `exit_code = 1`
- **THEN** the emitted `StreamEvent::ToolResult` SHALL have `is_error = true`

#### Scenario: Agent message maps to a thought

- **WHEN** `parse_stream_line` receives an `item.completed` line with `item.type = agent_message` and `text = "DONE"`
- **THEN** it SHALL return a single `StreamEvent::Thought { text: "DONE" }`

#### Scenario: Turn completion maps usage tokens

- **WHEN** `parse_stream_line` receives a `turn.completed` line whose `usage` has `input_tokens = 30515`, `cached_input_tokens = 22272`, `output_tokens = 43`, and `reasoning_output_tokens = 17`
- **THEN** it SHALL return a `StreamEvent::Usage` whose `TokenUsage` carries those four counts in the corresponding fields and SHALL NOT emit a missing-fields warning

#### Scenario: Turn completion without usage emits no event and no warning

- **WHEN** `parse_stream_line` receives a `turn.completed` line without a `usage` object
- **THEN** it SHALL return zero `StreamEvent` values and SHALL NOT emit a missing-fields warning

#### Scenario: Turn completion with unrecognized usage fields emits a warning

- **WHEN** `parse_stream_line` receives a `turn.completed` line whose `usage` object contains `inputTokenCount = 30515` and `outputTokenCount = 43` but contains none of `input_tokens`, `cached_input_tokens`, `output_tokens`, or `reasoning_output_tokens` as unsigned integers
- **THEN** it SHALL emit exactly one stderr warning prefixed with `warning: codex usage` AND return a `StreamEvent::Usage` whose normalized token counts are all zero or `None` while `TokenUsage.extras` preserves the verbatim `usage` object

#### Scenario: Turn completion with one recognized usage field emits no missing-fields warning

- **WHEN** `parse_stream_line` receives a `turn.completed` line whose `usage` object contains `input_tokens = 12` and `outputTokenCount = 7`
- **THEN** it SHALL return a `StreamEvent::Usage` whose `input_tokens` equals 12 and `output_tokens` equals 0 AND SHALL NOT emit a missing-fields warning

#### Scenario: Thread start yields the session id and no event

- **WHEN** `parse_stream_line` and `extract_session_id` each receive a `thread.started` line with `thread_id = "019e4d0e-..."`
- **THEN** `extract_session_id` SHALL return `Some("019e4d0e-...")` AND `parse_stream_line` SHALL return zero `StreamEvent` values for that line
