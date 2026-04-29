## MODIFIED Requirements

### Requirement: AuditPanel surfaces seven workspace-level audit JSONL tabs

The `AuditPanel.vue` component SHALL render exactly seven tabs in the order `sanitize`, `tool`, `reasoning`, `token`, `llm`, `kb_growth`, `generator`, mirroring the seven workspace-level audit JSONL files under `<workspace>/.codebus/` declared by CLAUDE.md (`七層 Audit JSONL` section). The component MUST expose an `activeTab` prop accepting any of these seven keys; passing an unrecognised key MUST be a TypeScript compile-time error.

The component MUST NOT render rows from in-source sample data. The `CB_AUDIT_SAMPLES` literal from `design/v1/shell.js` is mockup-only fixture data per `design/v1/README.md §四`; the production component MUST receive its rows via a `rows` prop (or equivalent injection) and MUST render an empty state when the array is empty. No `web/app/` source file may contain a literal copy of `CB_AUDIT_SAMPLES` or any element of it.

The component SHALL emit `select-row` with the clicked row's index in the current `rows` prop when the user clicks a row in the body. The emit MUST fire for every tab uniformly — even tabs with no overlay wiring at the parent level (the parent decides whether to react to the emit). The emit signature MUST equal `(e: 'select-row', index: number) => void`. The component MUST NOT internally render any inspector / drawer / modal in response to the click; row-click → overlay binding is a parent-layer concern. (Rationale: keeps AuditPanel a dumb display surface so per-kind inspectors — `LlmCallInspector` for `llm`, future SanitizerDiff for `sanitize`, etc. — can be hosted at page level rather than coupled into AuditPanel.)

#### Scenario: All seven tabs render in canonical order

- **WHEN** `<AuditPanel />` mounts with default props
- **THEN** the rendered tab strip MUST contain exactly seven button elements
- **AND** their `data-tab` attribute values MUST equal `sanitize`, `tool`, `reasoning`, `token`, `llm`, `kb_growth`, `generator` in that left-to-right order

#### Scenario: Empty rows show empty state, never sample data

- **WHEN** `<AuditPanel :active-tab="'sanitize'" :rows="[]" />` is rendered
- **THEN** the body region MUST display a documented empty-state message
- **AND** the rendered DOM MUST NOT contain text matching `secret`, `pii_id`, `src/config.py`, `tests/fixtures/.env.test`, or any other string from `design/v1/shell.js::CB_AUDIT_SAMPLES.sanitize[*]`

#### Scenario: No CB_AUDIT_SAMPLES literal under web/app/

- **WHEN** the entire `web/app/` tree is grepped for the symbol `CB_AUDIT_SAMPLES`
- **THEN** zero matches MUST be returned
- **AND** any sample-style data needed for testing MUST live in `web/tests/` (when the test framework lands in Phase B), not in production source

#### Scenario: Row click emits select-row with the clicked index

- **WHEN** the user clicks the third row inside an `<AuditPanel :active-tab="'llm'" :rows="threeEntries" />`
- **THEN** the component MUST emit `select-row` exactly once
- **AND** the emit's payload MUST equal `2` (zero-based index of the clicked row)

#### Scenario: select-row fires for every tab regardless of parent wiring

- **WHEN** the user clicks a row in `<AuditPanel :active-tab="'tool'" :rows="someEntries" />` while the parent does not bind the emit
- **THEN** the component MUST still emit `select-row` (no conditional suppression based on tab)
- **AND** the emit MUST NOT throw when the parent has no listener

#### Scenario: AuditPanel does not render any inspector / drawer / modal of its own

- **WHEN** `<AuditPanel :active-tab="'llm'" :rows="entries" />` is mounted in isolation (no parent overlay wiring)
- **THEN** clicking a row MUST NOT cause any new DOM element with class containing `inspector`, `drawer`, `modal`, or `overlay` to mount inside the AuditPanel root
- **AND** any inspector overlay MUST be hosted by the parent page, not by AuditPanel itself
