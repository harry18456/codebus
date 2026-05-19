## MODIFIED Requirements

### Requirement: Quiz Storage Layout and Retry Semantics

Each quiz attempt SHALL be persisted as one file at `<vault>/.codebus/quiz/<slug>/<iso-timestamp>.md`, where `<slug>` is the page slug for `wiki_preview` trigger or the topic slug for `ai_planned` trigger. Persisting an attempt SHALL NOT overwrite or delete any prior attempt file. The generated attempt markdown SHALL remain immutable after persistence. Quiz history SHALL be derived by scanning this directory tree, not by correlating run-log entries.

Each attempt MAY additionally have a sibling progress sidecar at `<vault>/.codebus/quiz/<slug>/<iso-timestamp>.progress.json` recording the user's answering state for that attempt. The sidecar SHALL store ONLY the non-derivable data: `schema_version` (integer), `answers` (ordered list of `{ q: 1-based integer, selected: "A"|"B"|"C"|"D", correct: boolean }`), `status` (`in_progress` or `completed`), `started_at`, `completed_at` (RFC3339; `completed_at` null until completed), and an OPTIONAL `cursor` (`{ q: 1-based integer, revealed: boolean }`) recording the question the user is currently viewing and whether it was already submitted. Derived quantities — total question count, answered count, correct count, score, pass/fail — SHALL NOT be stored in the sidecar; they SHALL be recomputed from `answers` and the attempt markdown so the sidecar has a single source of truth and cannot hold self-contradictory fields. `cursor` is navigation state (not a derived quantity) and MAY be absent: a sidecar without `cursor` SHALL remain valid and SHALL be read by treating the resume position as the last answered question in its submitted state. An absent sidecar SHALL mean the attempt is not started (answered 0; total parsed from the markdown body's `## Q` headings).

The sidecar SHALL be written atomically (write to a temporary file in the same directory, then rename over the target) so an interrupted write cannot corrupt it. Reading the sidecar SHALL be tolerant: a missing file yields the not-started state (not an error); a malformed or unparseable file SHALL be treated as not-started rather than panicking; unknown JSON keys SHALL be ignored; a `schema_version` newer than known SHALL still best-effort read the known fields. The sidecar is additive — it SHALL NOT modify or replace the immutable attempt markdown, and the retry semantics below are unchanged by it.

Retry SHALL be a plain re-invocation of the same flow (Goal: `run_quiz_plan` then `run_quiz_generate`; Page: `run_quiz_generate`) with the same inputs. The system SHALL NOT inject previous question stems as negative context and SHALL NOT guarantee that a retry produces different questions. User-facing surfaces SHALL NOT claim that retry always yields new questions.

#### Scenario: Retry creates a new non-destructive file

- **WHEN** a quiz on the same scope is generated twice
- **THEN** two distinct timestamped files SHALL exist under the same `<slug>` directory AND the earlier file's contents SHALL be unchanged

#### Scenario: Retry questions are not guaranteed distinct

- **WHEN** the same flow is re-invoked with identical inputs
- **THEN** the system SHALL NOT pass any record of the prior questions into the second invocation AND the second quiz MAY repeat questions from the first

#### Scenario: Absent sidecar means not started

- **GIVEN** an attempt markdown with 5 `## Q` headings and no sidecar file
- **WHEN** the attempt's progress is read
- **THEN** the result SHALL be the not-started state with total 5 and answered 0

#### Scenario: Sidecar stores only non-derivable data

- **WHEN** the user has answered 3 of 5 questions and the sidecar is written
- **THEN** the sidecar SHALL contain `answers` with 3 entries, `status: in_progress`, `started_at`, and `completed_at: null` AND SHALL NOT contain stored `answered`, `correct`, `score`, or pass/fail fields

#### Scenario: Malformed sidecar is treated as not started

- **GIVEN** a `*.progress.json` whose contents are not valid progress JSON
- **WHEN** the attempt's progress is read
- **THEN** the read SHALL NOT panic AND SHALL yield the not-started state

#### Scenario: Sidecar write is atomic and non-destructive to markdown

- **WHEN** progress is written twice for the same attempt
- **THEN** the final sidecar SHALL reflect the second write AND no temporary file SHALL remain AND the attempt's `.md` file SHALL be byte-unchanged
