## ADDED Requirements

### Requirement: Cancellation Polling Not Coupled To Stdout

The `agent::invoke` cancellation polling SHALL NOT be reactive to child stdout activity alone. When the supplied `cancel: Arc<AtomicBool>` is flipped to `true` while the child process has stopped emitting stdout (for example, the LLM is hung on a network call, the child is waiting on a stalled tool result, or the child is otherwise blocked on I/O that does not surface through stdout), `invoke` SHALL still observe the cancel flag and SHALL still terminate the child within a bounded latency window. The bounded latency SHALL be at most 200 ms in typical operation, measured from the instant the flag is set to `true` to the instant the child process receives a platform termination signal (`SIGTERM` on Unix, `TerminateProcess` on Windows).

After termination, `invoke` SHALL drain remaining stdout, reap the child via `child.wait()`, join any auxiliary watcher threads it spawned (no detached threads), and return `Ok(InvokeReport)` with the `exit` field reflecting the killed state.

The provider-agnostic property already established by the `Invocation Loop Drives Backend Trait` requirement SHALL continue to hold: this bounded-latency cancellation SHALL be enforced inside `invoke` itself, SHALL apply uniformly to every `&dyn AgentBackend` implementation, and SHALL NOT introduce provider-specific branching.

#### Scenario: Cancel observed when child has gone silent

- **WHEN** `invoke` is running against a child process that has stopped writing to stdout (for example, a fake binary that spawns and sleeps for 30 seconds without output) AND the caller sets `cancel.store(true)`
- **THEN** within 200 ms of the flag being set, the child process SHALL receive a platform termination signal AND `invoke` SHALL return `Ok(InvokeReport)` with `exit.success() == false`

##### Example: silent-child cancel latency

- **GIVEN** a fake binary `sleep 30` is spawned via `invoke` and emits no stdout
- **WHEN** the calling code sets `cancel.store(true, Ordering::SeqCst)` at time `t`
- **THEN** `invoke` SHALL return no later than `t + 200ms` AND the returned `exit.success()` SHALL be `false`

#### Scenario: Cancel observed while child is streaming

- **WHEN** `invoke` is running against a child process that is actively streaming stdout lines AND the caller sets `cancel.store(true)`
- **THEN** the existing per-line cancel check inside the main loop SHALL kill the child within a single line iteration AND `invoke` SHALL return `Ok(InvokeReport)` with `exit.success() == false`

#### Scenario: No cancel flag, normal completion

- **WHEN** `invoke` is called with `cancel = None` OR the cancel flag is never flipped AND the child process exits normally
- **THEN** any auxiliary watcher thread spawned to monitor the cancel flag SHALL terminate before `invoke` returns AND `invoke` SHALL NOT leak the watcher thread AND `invoke` SHALL return `Ok(InvokeReport)` reflecting the child's actual exit status

#### Scenario: Polling mechanism is provider-agnostic

- **WHEN** `invoke` enforces the bounded cancellation latency
- **THEN** the mechanism SHALL live inside `invoke` itself AND SHALL NOT reference the `claude` or `codex` binary name, provider-specific argv flags, or provider-specific stream-json field names AND SHALL apply identically to every `&dyn AgentBackend` implementation
