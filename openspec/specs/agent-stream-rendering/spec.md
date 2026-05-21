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

The system SHALL provide a terminal renderer for `StreamEvent` values that maps each event variant to a human-readable multi-line string. Rendering SHALL respect the `RenderOptions` capability detected per the `cli` capability `Environment-Aware Output Styling` requirement: when `use_emoji` is true the emoji-leading form is used, when false the ASCII symbol fallback is used.

Rendering detail SHALL additionally depend on the `RenderOptions.verbose` flag. When `verbose` is false (the default, and the only mode prior to this change) the renderer SHALL use the compact form defined below. When `verbose` is true (set by the CLI when `--debug` is passed, per the `cli` capability `Debug Flag Output` requirement) the renderer SHALL use the full form defined below, surfacing complete tool input and complete tool result without summarization, truncation, or suppression.

The four event renderings SHALL be:

- `Thought { text }` — leader `🤔` (emoji) or `◆` (fallback) followed by the literal label `[Agent 思考]`. When `text` contains a newline the body SHALL be rendered on a new line indented by four spaces; otherwise the body follows the label on the same line separated by a space. This rendering is identical in both compact and verbose modes.
- `ToolUse { name, input }` where `name` equals `"Write"` or `"Edit"` — leader `✍️` (emoji) or `+` (fallback) followed by the literal label `[正在生成]` and a newline. In compact mode the next line SHALL contain the `file_path` field of `input` (forward-slash normalized for cross-platform display) indented by four spaces; when `file_path` is missing the placeholder `(unknown)` is used. In verbose mode the body SHALL instead contain the complete `input` JSON (including the written / edited content) indented by four spaces.
- `ToolUse { name, input }` for any other `name` — leader `🛠️` (emoji) or `→` (fallback) followed by the literal label `[呼叫工具]` and a newline; the next line SHALL contain `<name>(<args>)` indented by four spaces. In compact mode `<args>` is a tool-specific summary of the `input` JSON (arrays and objects collapsed to counts). In verbose mode the body SHALL contain `<name>` followed by the complete `input` JSON (no collapsing) indented by four spaces.
- `ToolResult { output, is_error }` — leader `👀` (emoji) or `←` (fallback) followed by the literal label `[觀察結果]` and a newline; body indented by four spaces. In compact mode: when `output` matches a Write-success echo (e.g., literal prefix `File created successfully` or `The file has been updated`) the renderer SHALL return an empty string (suppress the redundant echo); when `output` matches the read-line-count form (e.g., `<file>: <N>L` or `<file>(<N> lines)`) the body SHALL be the substring `(<N> lines)`; when `output` exceeds 200 characters the body SHALL be truncated to the first 200 characters followed by the literal `…`. In verbose mode the body SHALL be the complete `output` verbatim — no Write-echo suppression, no read-line-count substitution, no 200-character truncation. The `is_error` flag SHALL NOT alter the rendering format in either mode (the warn-line context already conveys severity).
- `Usage(_)` — empty string in both modes (Usage events are consumed for `RunLog` accumulation, not displayed inline).

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

#### Scenario: ToolUse Write specialization shows file_path in compact mode

- **WHEN** the renderer receives `ToolUse { name: "Write", input: {"file_path": "/repo/wiki/foo.md"} }` with emoji enabled AND `verbose` is false
- **THEN** the rendered string SHALL be `✍️ [正在生成]\n    /repo/wiki/foo.md`

#### Scenario: ToolUse Read formats name with input summary in compact mode

- **WHEN** the renderer receives `ToolUse { name: "Read", input: {"file_path": "/x"} }` with emoji enabled AND `verbose` is false
- **THEN** the rendered string SHALL begin with `🛠️ [呼叫工具]` and contain the substring `Read(`

#### Scenario: ToolResult truncates long output at 200 chars in compact mode

- **WHEN** the renderer receives `ToolResult { output: <500-char string of "x">, is_error: false }` with emoji enabled AND `verbose` is false
- **THEN** the rendered string body SHALL contain exactly the first 200 characters of the input followed by the literal `…`

#### Scenario: ToolResult Write-success echo is suppressed in compact mode

- **WHEN** the renderer receives `ToolResult { output: "File created successfully at /x.md", is_error: false }` AND `verbose` is false
- **THEN** the rendered string SHALL be empty (the echo would duplicate the preceding ToolUse Write banner)

#### Scenario: Usage event renders nothing

- **WHEN** the renderer receives `Usage(TokenUsage::default())`
- **THEN** the rendered string SHALL be empty

#### Scenario: Path normalization uses forward slashes

- **WHEN** the renderer receives `ToolUse { name: "Write", input: {"file_path": "C:\\repo\\wiki\\foo.md"} }` with emoji enabled AND `verbose` is false
- **THEN** the rendered string body SHALL contain `C:/repo/wiki/foo.md` and SHALL NOT contain backslash separators

#### Scenario: Verbose mode renders full tool result without truncation

- **WHEN** the renderer receives `ToolResult { output: <500-char string of "x">, is_error: false }` AND `verbose` is true
- **THEN** the rendered string body SHALL contain all 500 characters of the input AND SHALL NOT contain the literal `…` truncation marker

#### Scenario: Verbose mode renders read result fully instead of line count

- **WHEN** the renderer receives `ToolResult { output: "fn main() {}\n(3 lines)", is_error: false }` AND `verbose` is true
- **THEN** the rendered string body SHALL contain the substring `fn main() {}` (the full output) rather than only `(3 lines)`

#### Scenario: Verbose mode does not suppress Write-success echo

- **WHEN** the renderer receives `ToolResult { output: "File created successfully at /x.md", is_error: false }` AND `verbose` is true
- **THEN** the rendered string SHALL NOT be empty AND SHALL contain the substring `File created successfully`

#### Scenario: Verbose mode renders full tool input for Write

- **WHEN** the renderer receives `ToolUse { name: "Write", input: {"file_path": "/x.md", "content": "hello world body"} }` with emoji enabled AND `verbose` is true
- **THEN** the rendered string SHALL contain the substring `hello world body` (the full written content), not just the file path

#### Scenario: Verbose mode renders full tool input for other tools

- **WHEN** the renderer receives `ToolUse { name: "Grep", input: {"pattern": "needle", "glob": ["*.rs", "*.toml"]} }` with emoji enabled AND `verbose` is true
- **THEN** the rendered string SHALL contain the substring `needle` AND SHALL contain `*.rs` (the array expanded, not collapsed to a count)

---
### Requirement: Spawn Stdio Architecture for Stream Capture

The system SHALL spawn the Claude CLI child process with `Stdio::piped()` for both stdout and stderr (departing from the prior `Stdio::inherit()` of v3-render-polish). The argv SHALL include the three flags `--output-format stream-json`, `--verbose`, and `--input-format stream-json` in addition to the existing v3-config flags (`--tools` / `--allowedTools` / `--permission-mode` / optional `--model` / optional `--effort`). The system SHALL consume stdout synchronously line-by-line via a `BufReader::lines()` iterator on the main thread, parsing each line per the `Stream-JSON Wire Format Parsing` requirement and delivering the resulting `StreamEvent` values to a caller-supplied `on_event: impl FnMut(StreamEvent)` closure (the closure SHALL be the sole consumer of `StreamEvent` values produced by `invoke`; the function SHALL NOT call `print_event` or otherwise render events directly), while accumulating any `Usage` events into a per-invocation `TokenUsage` total. The CLI thin wrapper invoking `agent::invoke` SHALL pass a closure that calls `print_event` per the `Stream Event Terminal Rendering` requirement, preserving the parent-stdout rendering behavior for CLI users. The system SHALL spawn one background thread that copies stderr to the parent process's stderr (`io::copy(child.stderr.take(), io::stderr())`), preserving agent error messages without interpretation. After the stdout reader reaches EOF, the system SHALL `child.wait()` to collect the final `ExitStatus`, then attempt to join the stderr thread within a 5-second deadline (after which the thread is detached). The system SHALL return an `InvokeReport` containing the `ExitStatus`, the accumulated `TokenUsage`, and RFC 3339 UTC `started_at` / `finished_at` timestamps captured before spawn and after wait respectively.

The `invoke` function SHALL additionally accept a `cancel: Option<Arc<AtomicBool>>` parameter and SHALL read the flag with `Ordering::Relaxed` after processing each stdout line. When the flag is observed as `true`, the function SHALL invoke `child.kill()` on the spawned child, SHALL drain remaining stdout on a best-effort basis without invoking `on_event` further, SHALL `child.wait()` to reap the child, and SHALL return `Ok(InvokeReport)` with `exit` reflecting the killed state. When `cancel` is `None`, the function SHALL behave exactly as when no cancel signal is provided (single discriminant check per line iteration; no other overhead).

#### Scenario: Spawn argv includes the three stream-json flags

- **WHEN** the system spawns the Claude CLI child process for any verb
- **THEN** the spawn argv SHALL include `--output-format stream-json` AND `--verbose` AND `--input-format stream-json` in addition to the verb's existing toolset / model / effort flags

#### Scenario: stdout consumed line-by-line, events delivered to caller closure

- **WHEN** the spawned child writes one stream-json line containing an `assistant` event with a single text content item to its stdout
- **THEN** the caller-supplied `on_event` closure SHALL be invoked exactly once with the parsed `StreamEvent::Thought { text }` value AND the function SHALL NOT block waiting for additional child output before invoking the closure AND the function SHALL NOT call `print_event` directly

#### Scenario: CLI thin wrapper preserves parent-stdout rendering

- **WHEN** `codebus-cli/src/commands/goal.rs` (or query.rs / fix.rs) invokes the verb library function which in turn calls `agent::invoke` AND the CLI thin wrapper passes a closure that calls `print_event(&event, &render_opts)`
- **THEN** the parent process stdout SHALL contain the rendered `Thought` / `ToolUse` / `ToolResult` lines exactly as before this change AND existing `goal_flow.rs` / `query_flow.rs` / `fix_flow.rs` integration tests SHALL pass without modification to their golden assertions

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
- **THEN** the system SHALL return `Ok(InvokeReport)` with `exit.code() == Some(1)` AND `accumulated_tokens.input_tokens == 0` (no usage was streamed) AND the `on_event` closure SHALL have been invoked once with the `StreamEvent::Thought { text }` derived from the assistant event

#### Scenario: Cancel flag flipped halts loop within one line

- **WHEN** `invoke` is invoked with `cancel: Some(flag)` AND the caller flips `flag` to `true` after the second stream line has been processed
- **THEN** the function SHALL invoke `child.kill()` no later than after processing the third line AND SHALL return `Ok(InvokeReport)` whose `exit.success()` SHALL be `false` AND no further `on_event` invocation SHALL occur after the kill

#### Scenario: Cancel none preserves existing behavior

- **WHEN** `invoke` is invoked with `cancel: None`
- **THEN** the function SHALL behave identically to a call with a never-flipped flag (no polling overhead beyond the `None` discriminant check) AND SHALL NOT panic if `child.kill()` is reachable
