## ADDED Requirements

### Requirement: Cancel Completes Within Bounded Latency

The system SHALL ensure that a `cancel_goal` invocation against an active goal run reaches a terminal state within a bounded latency window, even when the underlying child process has stopped emitting stdout (for example, the LLM is hung on a network call, the child is waiting on a stalled tool result, or the child is otherwise blocked on I/O that does not surface through stdout). "Terminal state" SHALL mean ALL of the following:

- The entry keyed by the cancelled run's `RunId` SHALL be removed from `AppState.active_runs`.
- A final `goal-stream` event signalling termination SHALL be emitted to the frontend.
- A `spawn_end` event SHALL be appended to the run's events file.
- The runner thread driving `agent::invoke` for that run SHALL return and SHALL NOT remain blocked.

The bounded latency SHALL be at most 1 second in typical operation, measured from the instant the frontend's `cancel_goal` IPC invocation returns to the instant the frontend receives the terminal `goal-stream` event. This invariant SHALL hold regardless of whether the child was actively streaming stdout at the moment of cancellation.

This requirement does NOT introduce a force-timeout: long-running goals SHALL NOT be killed by the system on a timer. The bounded latency applies only to the cancel pathway after the user (or another `cancel_goal` caller) has explicitly requested cancellation.

This requirement strengthens the existing `One Active Goal Run At A Time` requirement's `Spawn allowed after cancel completes` scenario by guaranteeing that "cancel completes" is reachable under all child-state conditions, not only when the child happens to be emitting stdout.

#### Scenario: Cancel reaches terminal state when child is silent

- **WHEN** a goal run is active for vault `V` AND the child process has stopped emitting stdout (LLM hung, waiting on tool result, or network-blocked) AND the frontend invokes `cancel_goal`
- **THEN** within 1 second of the `cancel_goal` IPC invocation returning, the frontend SHALL receive a terminal `goal-stream` event AND `active_runs` SHALL no longer contain the cancelled run's entry AND the run's events file SHALL contain a `spawn_end` event

#### Scenario: Cancel reaches terminal state when child is streaming

- **WHEN** a goal run is active for vault `V` AND the child process is actively streaming stdout AND the frontend invokes `cancel_goal`
- **THEN** within 1 second of the `cancel_goal` IPC invocation returning, the frontend SHALL receive a terminal `goal-stream` event AND `active_runs` SHALL no longer contain the cancelled run's entry AND the run's events file SHALL contain a `spawn_end` event

#### Scenario: Subsequent spawn succeeds after silent-child cancel

- **WHEN** a goal run is cancelled per the `Cancel reaches terminal state when child is silent` scenario AND the frontend subsequently invokes `spawn_goal` for the same vault `V`
- **THEN** the new `spawn_goal` invocation SHALL succeed AND a new run id SHALL be returned

#### Scenario: No force-timeout on long-running goals

- **WHEN** a goal run has been active for any duration without cancellation AND the user has not invoked `cancel_goal`
- **THEN** the system SHALL NOT kill the child process on a timer AND the run SHALL continue until the child exits naturally or the user explicitly cancels
