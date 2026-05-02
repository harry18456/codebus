## MODIFIED Requirements

### Requirement: useSseTask consumes bearer through useSidecar

The `useSseTask` composable SHALL accept a `taskId: string` matching `^(scan|kb|explore|generate|qa)_[0-9a-f]{8}$` and connect to the SSE endpoint `<base_url>/tasks/<task_id>/events` via the browser-native `EventSource` API. The composable MUST obtain the bearer token by calling `useSidecar()` and MUST NOT receive bearer/base-url values as direct arguments — passing those as parameters would tempt callers to bypass the IPC-only rule.

The composable MUST implement automatic reconnection with exponential backoff (initial delay 1 s, doubling per attempt, capped at 30 s); the final delay MUST surface to the caller via a reactive `status` field with values drawn from the closed set `{"connecting", "open", "reconnecting", "closed", "error"}`. The reactive return surface MUST expose `events` (array of received SSE messages, capped at 1000 entries with FIFO eviction), `status`, `error`, and a `close()` function that disconnects the EventSource immediately.

The composable MUST distinguish between two distinct error sources from the underlying `EventSource`:

1. **Connection-level errors** — fired by the browser when the SSE connection drops, fails to open, or is closed by the server. These dispatch as a generic `Event` (NOT a `MessageEvent`) and MUST be handled exclusively by the `EventSource.onerror` reconnection path; they MUST NOT push any entry into the reactive `events` array.
2. **Server-emitted `error` events** — fired when the sidecar transmits an SSE message with `event: error\ndata: <json>`. These dispatch as a `MessageEvent` whose `data` field is the JSON string. They MUST be appended to the `events` array as `{type: "error", data: <parsed json>}`.

The composable MUST NOT register `'error'` inside the catch-all `addEventListener` loop alongside other named events (`progress`, `done`, etc.), because EventSource's connection-error event shares the `'error'` name with server-emitted `event: error` SSE messages and a single shared handler cannot tell them apart. Instead, the composable MUST register a dedicated `'error'` listener whose callback gates the push by checking `event instanceof MessageEvent && typeof event.data === 'string'` before treating it as a server message.

#### Scenario: Bearer arrives via useSidecar, not parameters

- **WHEN** `useSseTask`'s function signature is inspected
- **THEN** the parameter list MUST be exactly `(taskId: string)` — no `bearer`, `token`, `baseUrl`, `headers`, or equivalent values may be accepted
- **AND** the implementation MUST call `useSidecar()` to obtain the bearer at runtime

#### Scenario: Invalid task_id rejected pre-connect

- **WHEN** `useSseTask("scan_INVALID")` is invoked (pattern violates the regex)
- **THEN** the composable MUST return a closed-state result without opening any `EventSource`
- **AND** `status.value` MUST equal `"error"` and `error.value` MUST reference the regex constraint

#### Scenario: Reconnect uses exponential backoff capped at 30 s

- **WHEN** the SSE connection drops mid-stream
- **THEN** the composable MUST attempt reconnection after delays of 1 s, 2 s, 4 s, 8 s, 16 s, 30 s, 30 s, ... in that sequence
- **AND** while waiting between attempts, `status.value` MUST equal `"reconnecting"`
- **AND** when a reconnect succeeds, `status.value` MUST flip back to `"open"`

#### Scenario: Events array capped at 1000 entries

- **WHEN** the SSE stream delivers a 1001st event without `close()` being called
- **THEN** the `events` reactive array MUST drop the oldest entry and append the newest
- **AND** the array length MUST remain exactly 1000 after the FIFO eviction

#### Scenario: Named error listener ignores connection-level errors

- **WHEN** the underlying `EventSource` dispatches a generic `Event` named `"error"` (e.g., the server closes the connection cleanly after a `done` event, or the network drops)
- **THEN** the composable's dedicated `'error'` listener MUST NOT push any entry into the `events` reactive array
- **AND** the `EventSource.onerror` reconnection path MUST still execute (close + scheduleReconnect)
- **AND** when the same EventSource later dispatches a `MessageEvent` named `"error"` with a JSON `data` string (i.e., a server-emitted `event: error\ndata: {...}` SSE message), the composable MUST append exactly one entry `{type: "error", data: <parsed json>}` to the `events` array
