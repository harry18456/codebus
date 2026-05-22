## MODIFIED Requirements

### Requirement: Interrupted Run Detection

The system SHALL detect interrupted goal runs by comparing `events-*.jsonl` files against `runs-*.jsonl` rows at workspace mount time (via the `list_runs` IPC). For each `events-<started_at_slug>.jsonl` file present under `<vault>/.codebus/log/`, the system SHALL search the `runs-*.jsonl` files for a row whose `started_at` (slugged identically) matches.

When no matching row exists, the system SHALL synthesize a virtual `RunLogSummary` with `outcome="interrupted"` ONLY IF the events file is identifiable as a goal-mode run. An events file is identifiable as a goal-mode run when one of its leading events is a `VerbBanner::Goal` event — only the `goal` verb emits this banner, so `chat`, `query`, `fix`, and `quiz` events files SHALL NOT be identified as goal-mode runs. When the events file is identified as a goal-mode run, the synthesized entry SHALL have `outcome="interrupted"`, `started_at` derived from the slug, `goal` extracted from the `VerbBanner::Goal` event, and `mode="goal"`.

When an orphan events file is NOT identifiable as a goal-mode run (no `VerbBanner::Goal` event among its leading events), the system SHALL NOT synthesize any virtual entry for it, and that events file SHALL NOT contribute any row to the `list_runs` response. This prevents in-progress or interrupted `chat` / `query` / `fix` / `quiz` runs — whose `events-*.jsonl` file exists before their terminal `runs-*.jsonl` row is written — from transiently appearing in the Goals list with empty goal text.

The virtual entry SHALL NOT be written back to any `runs-*.jsonl` file — it exists only in the IPC response. Subsequent re-invocations of `list_runs` SHALL re-derive the virtual entry from the same on-disk state.

If the same events file later gains a matching RunLog row (e.g., because the original `run_goal` process recovered and wrote its terminal RunLog late), the virtual entry SHALL no longer appear in `list_runs` output — the real row supersedes it.

#### Scenario: Orphan goal events file produces virtual interrupted entry

- **WHEN** `list_runs` is invoked AND the vault contains `events-2026-05-13T03-00-00Z.jsonl` whose leading events include a `VerbBanner::Goal` event with `goal_text="describe auth flow"` AND no `runs-*.jsonl` row has `started_at == "2026-05-13T03:00:00Z"`
- **THEN** the returned list contains a virtual entry with `outcome == "interrupted"` AND `mode == "goal"` AND `goal == "describe auth flow"` AND `started_at == "2026-05-13T03:00:00Z"` AND no row is appended to any `runs-*.jsonl` file on disk

#### Scenario: Orphan non-goal events file produces no virtual entry

- **WHEN** `list_runs` is invoked AND the vault contains an orphan `events-2026-05-13T04-00-00Z.jsonl` whose leading events contain NO `VerbBanner::Goal` event (e.g., an in-progress chat / query / fix / quiz run) AND no `runs-*.jsonl` row matches its slug
- **THEN** the returned list contains NO entry for `started_at == "2026-05-13T04:00:00Z"` — neither a virtual `interrupted` entry nor a row with empty goal text

#### Scenario: Real RunLog row supersedes virtual interrupted

- **WHEN** `events-2026-05-13T03-00-00Z.jsonl` exists AND a `runs-2026-05-13.jsonl` row is appended with `started_at == "2026-05-13T03:00:00Z"` and `outcome == "cancelled"` AND `list_runs` is invoked
- **THEN** the returned list contains the real row (`outcome="cancelled"`) AND does NOT contain a virtual `outcome="interrupted"` entry for the same started_at
