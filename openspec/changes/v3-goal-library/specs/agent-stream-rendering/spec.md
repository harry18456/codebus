## MODIFIED Requirements

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
