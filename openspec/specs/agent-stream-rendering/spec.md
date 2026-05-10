# agent-stream-rendering Specification

## Purpose

TBD - created by archiving change 'v3-run-log'. Update Purpose after archive.

## Requirements

### Requirement: Stream-JSON Wire Format Parsing

The system SHALL parse the Claude CLI `--output-format stream-json --verbose` line-delimited output into a closed enum of `StreamEvent` variants. Each line of stdout from the spawned `claude -p` child SHALL be passed to the parser, which SHALL return zero or more `StreamEvent` values per line. The four recognized outer event types and their mappings SHALL be:

- `{"type":"assistant", "message":{"content":[...]}}` — for each item in `content`, emit `Thought { text }` for `{"type":"text","text":...}` items (skip when text is empty), `ToolUse { name, input }` for `{"type":"tool_use","name":...,"input":...}` items. `{"type":"thinking",...}` items SHALL be skipped (internal reasoning, not user-facing).
- `{"type":"user", "message":{"content":[...]}}` — for each `{"type":"tool_result","content":...,"is_error":...}` item emit `ToolResult { output, is_error }`. The `output` SHALL be the joined `text` of array-form content, the verbatim string of string-form content, or the JSON-stringified form of any other shape.
- `{"type":"result", "usage":{...}}` — emit exactly one `Usage(TokenUsage)` event whose `input_tokens` / `output_tokens` come from the corresponding fields, `cache_read_tokens` from `cache_read_input_tokens`, `cache_write_tokens` from `cache_creation_input_tokens`, `reasoning_tokens` SHALL be `None`, and `extras` SHALL be the verbatim `usage` JSON object. When the `usage` field is absent the parser SHALL return zero events for that line.
- `{"type":"system",...}` / `{"type":"rate_limit_event",...}` / any unknown future `type` value → zero events (forward-compat).

When the line fails to parse as JSON, the parser SHALL return zero events. The parser SHALL never panic or return an `Err`.

#### Scenario: Assistant text item maps to Thought

- **WHEN** the parser receives the line `{"type":"assistant","message":{"content":[{"type":"text","text":"hello"}]}}`
- **THEN** the returned vec SHALL contain exactly one `Thought { text: "hello" }` event

#### Scenario: Assistant tool_use item maps to ToolUse with input preserved

- **WHEN** the parser receives the line `{"type":"assistant","message":{"content":[{"type":"tool_use","name":"Read","input":{"file_path":"/x.rs"}}]}}`
- **THEN** the returned vec SHALL contain exactly one `ToolUse { name: "Read", input: {"file_path":"/x.rs"} }` event with `input` preserved as a `serde_json::Value`

#### Scenario: User tool_result item maps to ToolResult

- **WHEN** the parser receives the line `{"type":"user","message":{"content":[{"type":"tool_result","content":"file body","is_error":false}]}}`
- **THEN** the returned vec SHALL contain exactly one `ToolResult { output: "file body", is_error: false }` event

#### Scenario: Result event with usage maps to Usage

- **WHEN** the parser receives `{"type":"result","usage":{"input_tokens":100,"output_tokens":50,"cache_read_input_tokens":10,"cache_creation_input_tokens":5}}`
- **THEN** the returned vec SHALL contain exactly one `Usage(TokenUsage)` event whose `input_tokens` equals 100, `output_tokens` equals 50, `cache_read_tokens` equals `Some(10)`, `cache_write_tokens` equals `Some(5)`, `reasoning_tokens` equals `None`, and `extras` equals the verbatim `usage` JSON object

#### Scenario: Result event without usage emits nothing

- **WHEN** the parser receives `{"type":"result","subtype":"end_turn"}` (no `usage` key)
- **THEN** the returned vec SHALL be empty

#### Scenario: System and rate_limit_event are silently skipped

- **WHEN** the parser receives `{"type":"system","subtype":"init"}` or `{"type":"rate_limit_event","reset_at":"..."}` or `{"type":"future_type"}`
- **THEN** the returned vec SHALL be empty (forward-compat)

#### Scenario: Malformed JSON returns empty vec

- **WHEN** the parser receives the line `{"type":"assistant","message":{"content":[{"type":"text",` (truncated mid-line)
- **THEN** the returned vec SHALL be empty AND the parser SHALL NOT panic or return an Err

#### Scenario: Multi-item assistant content emits multiple events

- **WHEN** the parser receives `{"type":"assistant","message":{"content":[{"type":"text","text":"calling"},{"type":"tool_use","name":"Grep","input":{}}]}}`
- **THEN** the returned vec SHALL contain exactly two events in declaration order: `Thought { text: "calling" }` followed by `ToolUse { name: "Grep", input: {} }`

---
### Requirement: Stream Event Terminal Rendering

The system SHALL provide a terminal renderer for `StreamEvent` values that maps each event variant to a human-readable multi-line string. Rendering SHALL respect the `RenderOptions` capability detected per the `cli` capability `Environment-Aware Output Styling` requirement: when `use_emoji` is true the emoji-leading form is used, when false the ASCII symbol fallback is used. The four event renderings SHALL be:

- `Thought { text }` — leader `🤔` (emoji) or `◆` (fallback) followed by the literal label `[Agent 思考]`. When `text` contains a newline the body SHALL be rendered on a new line indented by four spaces; otherwise the body follows the label on the same line separated by a space.
- `ToolUse { name, input }` where `name` equals `"Write"` or `"Edit"` — leader `✍️` (emoji) or `+` (fallback) followed by the literal label `[正在生成]` and a newline; the next line SHALL contain the `file_path` field of `input` (forward-slash normalized for cross-platform display) indented by four spaces; when `file_path` is missing the placeholder `(unknown)` is used.
- `ToolUse { name, input }` for any other `name` — leader `🛠️` (emoji) or `→` (fallback) followed by the literal label `[呼叫工具]` and a newline; the next line SHALL contain `<name>(<args>)` indented by four spaces, where `<args>` is a tool-specific summary of the `input` JSON.
- `ToolResult { output, is_error }` — when `output` matches a Write-success echo (e.g., literal prefix `File created successfully` or `The file has been updated`) the renderer SHALL return an empty string (suppress the redundant echo). Otherwise leader `👀` (emoji) or `←` (fallback) followed by the literal label `[觀察結果]` and a newline; body indented by four spaces. When `output` matches the read-line-count form (e.g., `<file>: <N>L` or `<file>(<N> lines)`) the body SHALL be the substring `(<N> lines)`. When `output` exceeds 200 characters the body SHALL be truncated to the first 200 characters followed by the literal `…`. The `is_error` flag SHALL NOT alter the rendering format (the warn-line context already conveys severity).
- `Usage(_)` — empty string (Usage events are consumed for `RunLog` accumulation, not displayed inline).

The renderer SHALL print the formatted string to stdout via `println!` (newline-terminated). Empty strings SHALL NOT be printed (skip the redundant blank line).

#### Scenario: Thought single-line text appends body to label line

- **WHEN** the renderer receives `Thought { text: "hello" }` with emoji enabled
- **THEN** the rendered string SHALL be `🤔 [Agent 思考] hello`

#### Scenario: Thought multi-line text indents body on new line

- **WHEN** the renderer receives `Thought { text: "line1\nline2" }` with emoji enabled
- **THEN** the rendered string SHALL be `🤔 [Agent 思考]\n    line1\n    line2`

#### Scenario: Thought ASCII fallback uses diamond glyph

- **WHEN** the renderer receives `Thought { text: "x" }` with emoji disabled
- **THEN** the rendered string SHALL begin with `◆ [Agent 思考]`

#### Scenario: ToolUse Write specialization shows file_path

- **WHEN** the renderer receives `ToolUse { name: "Write", input: {"file_path": "/repo/wiki/foo.md"} }` with emoji enabled
- **THEN** the rendered string SHALL be `✍️ [正在生成]\n    /repo/wiki/foo.md`

#### Scenario: ToolUse Read formats name with input summary

- **WHEN** the renderer receives `ToolUse { name: "Read", input: {"file_path": "/x"} }` with emoji enabled
- **THEN** the rendered string SHALL begin with `🛠️ [呼叫工具]` and contain the substring `Read(`

#### Scenario: ToolResult truncates long output at 200 chars

- **WHEN** the renderer receives `ToolResult { output: <500-char string of "x">, is_error: false }` with emoji enabled
- **THEN** the rendered string body SHALL contain exactly the first 200 characters of the input followed by the literal `…`

#### Scenario: ToolResult Write-success echo is suppressed

- **WHEN** the renderer receives `ToolResult { output: "File created successfully at /x.md", is_error: false }`
- **THEN** the rendered string SHALL be empty (the echo would duplicate the preceding ToolUse Write banner)

#### Scenario: Usage event renders nothing

- **WHEN** the renderer receives `Usage(TokenUsage::default())`
- **THEN** the rendered string SHALL be empty

#### Scenario: Path normalization uses forward slashes

- **WHEN** the renderer receives `ToolUse { name: "Write", input: {"file_path": "C:\\repo\\wiki\\foo.md"} }` with emoji enabled
- **THEN** the rendered string body SHALL contain `C:/repo/wiki/foo.md` and SHALL NOT contain backslash separators

##### Example: cycle of Thought → ToolUse Read → ToolResult under emoji-on TTY

- **GIVEN** stdout is a TTY with `NO_COLOR` unset
- **WHEN** the agent emits one `Thought { text: "checking foo" }`, then one `ToolUse { name: "Read", input: {"file_path": "/repo/foo.rs"} }`, then one `ToolResult { output: "fn main() {}\n", is_error: false }`
- **THEN** stdout SHALL contain exactly these lines in order:
  ```
  🤔 [Agent 思考] checking foo
  🛠️ [呼叫工具]
      Read(file_path="/repo/foo.rs")
  👀 [觀察結果]
      fn main() {}
  ```

---
### Requirement: Spawn Stdio Architecture for Stream Capture

The system SHALL spawn the Claude CLI child process with `Stdio::piped()` for both stdout and stderr (departing from the prior `Stdio::inherit()` of v3-render-polish). The argv SHALL include the three flags `--output-format stream-json`, `--verbose`, and `--input-format stream-json` in addition to the existing v3-config flags (`--tools` / `--allowedTools` / `--permission-mode` / optional `--model` / optional `--effort`). The system SHALL consume stdout synchronously line-by-line via a `BufReader::lines()` iterator on the main thread, parsing each line per the `Stream-JSON Wire Format Parsing` requirement and rendering the resulting events per the `Stream Event Terminal Rendering` requirement, while accumulating any `Usage` events into a per-invocation `TokenUsage` total. The system SHALL spawn one background thread that copies stderr to the parent process's stderr (`io::copy(child.stderr.take(), io::stderr())`), preserving agent error messages without interpretation. After the stdout reader reaches EOF, the system SHALL `child.wait()` to collect the final `ExitStatus`, then attempt to join the stderr thread within a 5-second deadline (after which the thread is detached). The system SHALL return an `InvokeReport` containing the `ExitStatus`, the accumulated `TokenUsage`, and RFC 3339 UTC `started_at` / `finished_at` timestamps captured before spawn and after wait respectively.

#### Scenario: Spawn argv includes the three stream-json flags

- **WHEN** the system spawns the Claude CLI child process for any verb
- **THEN** the spawn argv SHALL include `--output-format stream-json` AND `--verbose` AND `--input-format stream-json` in addition to the verb's existing toolset / model / effort flags

#### Scenario: stdout consumed line-by-line, events rendered to parent stdout

- **WHEN** the spawned child writes one stream-json line containing an `assistant` event with a single text content item to its stdout
- **THEN** the parent process SHALL print one rendered `Thought` line to its own stdout AND SHALL NOT block waiting for additional child output before printing it

#### Scenario: stderr passthrough to parent without interpretation

- **WHEN** the spawned child writes the literal byte sequence `Error: rate limit exceeded\n` to its stderr
- **THEN** the parent process's stderr SHALL contain the same byte sequence verbatim AND the system SHALL NOT attempt to parse it

#### Scenario: Usage events accumulated across multiple result events

- **WHEN** the spawned child emits two `result` events with `usage.input_tokens` of 100 and 50 respectively
- **THEN** the returned `InvokeReport.accumulated_tokens.input_tokens` SHALL equal 150 (saturating sum)

#### Scenario: started_at and finished_at bracket the spawn

- **WHEN** the system invokes the child and the child runs for some non-zero duration
- **THEN** the returned `InvokeReport.finished_at` SHALL be greater than or equal to `InvokeReport.started_at` when both are parsed as RFC 3339 timestamps

#### Scenario: Agent crash mid-stream still returns InvokeReport with partial tokens

- **WHEN** the spawned child emits one `assistant` event then exits with status 1 before emitting a `result` event
- **THEN** the system SHALL return `Ok(InvokeReport)` with `exit.code() == Some(1)` AND `accumulated_tokens.input_tokens == 0` (no usage was streamed) AND the rendered `Thought` line SHALL have appeared on stdout
