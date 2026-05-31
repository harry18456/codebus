## ADDED Requirements

### Requirement: Chat Session Persistence Retained

The `claude` child process spawned for a `chat` turn SHALL retain Claude CLI session persistence so that a subsequent turn can resume the same conversation via `--resume <id>`. Specifically, the chat spawn argv SHALL NOT include the `--no-session-persistence` flag that the single-shot verbs (`goal` / `query` / `fix` / `quiz`) carry per the `agent-backend` capability `Claude Backend Argv Equivalence` requirement. This guarantees that the session rollout the first turn creates remains on disk for the `--resume` path asserted by the `Subsequent turn resumes via --resume flag` scenario of this capability. The session-persistence gating SHALL be keyed on the spawn verb being `Verb::Chat`, not on the presence of `resume_session_id` (the first chat turn has no resume id yet but still must persist its session for the second turn).

#### Scenario: First chat turn persists its session

- **WHEN** `run_chat_turn` spawns the `claude` child for a first turn (`session_id: None`)
- **THEN** the spawned argv SHALL NOT include `--no-session-persistence` so the Claude CLI writes a session rollout the next turn can resume

#### Scenario: Resuming chat turn omits no-session-persistence and passes resume id

- **WHEN** `run_chat_turn` spawns the `claude` child for a turn with `session_id: Some("abc-123")`
- **THEN** the spawned argv SHALL NOT include `--no-session-persistence` AND SHALL include `--resume abc-123`
