## MODIFIED Requirements

### Requirement: Interrupted Run Detection

The system SHALL detect interrupted goal runs by comparing `events-*.jsonl` files against `runs-*.jsonl` rows at workspace mount time (via the `list_runs` IPC). For each `events-<started_at_slug>.jsonl` file present under `<vault>/.codebus/log/`, the system SHALL search the `runs-*.jsonl` files for a row whose `started_at` (slugged identically) matches.

When no matching `runs-*.jsonl` row exists AND the events file is identifiable as a goal-mode run (one of its leading events is a `VerbBanner::Goal` event), the system SHALL surface a virtual `RunLogSummary` whose `outcome` field SHALL be determined by whether the run is still alive: if the slug is currently present in the process-wide `active_runs` map (the in-memory cancel-flag registry maintained by `spawn_goal` / cleanup), `outcome` SHALL be `"running"` and `interrupt_reason` SHALL be absent; otherwise `outcome` SHALL be `"interrupted"` and `interrupt_reason` SHALL be `AppClose`. In both cases the synthesized entry SHALL have `started_at` derived from the slug, `goal` extracted from the `VerbBanner::Goal` event, `mode="goal"`, and an empty `finished_at`. This dual projection prevents the GUI from misreading an in-flight goal (whose terminal RunLog has not yet been written) as interrupted just because the user navigated to the Lobby and back.

When an orphan events file is NOT identifiable as a goal-mode run (no `VerbBanner::Goal` event among its leading events), the system SHALL NOT surface any virtual entry for it, and that events file SHALL NOT contribute any row to the `list_runs` response. This prevents in-progress or interrupted `chat` / `query` / `fix` / `quiz` runs — whose `events-*.jsonl` file exists before their terminal `runs-*.jsonl` row is written — from transiently appearing in the Goals list with empty goal text.

The virtual entry SHALL NOT be written back to any `runs-*.jsonl` file — it exists only in the IPC response. Subsequent re-invocations of `list_runs` SHALL re-derive the virtual entry from the same on-disk state plus the current `active_runs` snapshot.

If the same events file later gains a matching RunLog row (e.g., because the original `run_goal` process recovered and wrote its terminal RunLog late), the virtual entry SHALL no longer appear in `list_runs` output — the real row supersedes it.

#### Scenario: Orphan goal events file with no active_runs entry produces interrupted virtual entry

- **WHEN** `list_runs` is invoked AND the vault contains `events-2026-05-13T03-00-00Z.jsonl` whose leading events include a `VerbBanner::Goal` event with `goal_text="describe auth flow"` AND no `runs-*.jsonl` row has `started_at == "2026-05-13T03:00:00Z"` AND `active_runs` does NOT contain the slug
- **THEN** the returned list contains a virtual entry with `outcome == "interrupted"` AND `mode == "goal"` AND `goal == "describe auth flow"` AND `started_at == "2026-05-13T03:00:00Z"` AND `interrupt_reason == "app_close"` AND no row is appended to any `runs-*.jsonl` file on disk

#### Scenario: Orphan goal events file with live active_runs entry produces running virtual entry

- **WHEN** `list_runs` is invoked AND the vault contains `events-2026-05-28T07-39-26Z.jsonl` whose leading events include a `VerbBanner::Goal` event with `goal_text="smoke probe goal"` AND no `runs-*.jsonl` row matches its slug AND `active_runs` currently contains an entry keyed by `"2026-05-28T07-39-26Z"` (the in-flight spawn from the current app session)
- **THEN** the returned list contains a virtual entry with `outcome == "running"` AND `mode == "goal"` AND `goal == "smoke probe goal"` AND `started_at == "2026-05-28T07:39:26Z"` AND `interrupt_reason` absent AND `finished_at` empty

#### Scenario: Orphan non-goal events file produces no virtual entry

- **WHEN** `list_runs` is invoked AND the vault contains an orphan `events-2026-05-13T04-00-00Z.jsonl` whose leading events contain NO `VerbBanner::Goal` event (e.g., an in-progress chat / query / fix / quiz run) AND no `runs-*.jsonl` row matches its slug
- **THEN** the returned list contains NO entry for `started_at == "2026-05-13T04:00:00Z"` — neither a virtual `interrupted` entry, a `running` entry, nor a row with empty goal text

#### Scenario: Real RunLog row supersedes virtual interrupted

- **WHEN** `events-2026-05-13T03-00-00Z.jsonl` exists AND a `runs-2026-05-13.jsonl` row is appended with `started_at == "2026-05-13T03:00:00Z"` and `outcome == "cancelled"` AND `list_runs` is invoked
- **THEN** the returned list contains the real row (`outcome="cancelled"`) AND does NOT contain a virtual `outcome="interrupted"` or `outcome="running"` entry for the same started_at

## ADDED Requirements

### Requirement: Cross-Vault Goal Spawn Permitted

The system SHALL permit a `spawn_goal` invocation against vault `V2` to succeed even when `active_runs` contains a goal-mode entry associated with a different vault `V1`, in accordance with the existing `One Active Goal Run At A Time` requirement statement "switching vaults (back to lobby then opening a different vault) does not carry the constraint across". The "per vault" qualifier in the same requirement SHALL be enforced by associating each `active_runs` entry with the vault path under which it was inserted, and SHALL be queried via a vault-scoped predicate when evaluating the pre-spawn guard for `spawn_goal`. Goal-mode entries inserted under vault `V1` SHALL NOT cause the pre-spawn guard for vault `V2` to reject.

The vault-scoped enforcement SHALL apply symmetrically to the chat and quiz mode pre-spawn guards: a chat turn or quiz run active under vault `V1` SHALL NOT block a chat turn or quiz run, respectively, against a different vault `V2`. Same-vault same-mode mutual exclusion (the existing scenarios under `One Active Goal Run At A Time`, `Chat Turn Lifecycle`, and any quiz double-spawn guard) SHALL remain unchanged.

#### Scenario: Cross-vault goal spawn allowed while another vault has an active goal

- **WHEN** a goal-mode entry exists in `active_runs` associated with vault `V1` AND the frontend invokes `spawn_goal` with vault path `V2` where `V2 != V1`
- **THEN** the IPC call SHALL resolve with `Ok(<new_run_id>)` AND `active_runs` SHALL contain BOTH the prior goal-mode entry for `V1` AND the newly inserted goal-mode entry for `V2` simultaneously

#### Scenario: Same-vault same-mode exclusion preserved after vault scope landed

- **WHEN** a goal-mode entry already exists in `active_runs` associated with vault `V1` AND the frontend invokes `spawn_goal` with the same vault path `V1`
- **THEN** the IPC call SHALL reject with `AppError` having `kind: "invalid"`, `field: "active_runs"`, AND `message` containing the substring "already active"

#### Scenario: Cross-vault chat spawn allowed

- **WHEN** a chat-mode entry (keyed with the `chat-` prefix) exists in `active_runs` associated with vault `V1` AND the frontend invokes `spawn_chat_turn` with a different vault path `V2`
- **THEN** the IPC call SHALL resolve successfully AND `active_runs` SHALL contain BOTH chat-mode entries simultaneously, one under `V1` AND one under `V2`

#### Scenario: Cross-vault quiz spawn allowed

- **WHEN** a quiz-mode entry (keyed with the `quiz-` prefix) exists in `active_runs` associated with vault `V1` AND the frontend invokes `spawn_quiz_plan` or `spawn_quiz_generate` with a different vault path `V2`
- **THEN** the IPC call SHALL resolve successfully AND `active_runs` SHALL contain BOTH quiz-mode entries simultaneously, one under `V1` AND one under `V2`
