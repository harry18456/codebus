## MODIFIED Requirements

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
