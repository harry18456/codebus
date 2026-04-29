## MODIFIED Requirements

### Requirement: AuditPanel surfaces seven workspace-level audit JSONL tabs

The `AuditPanel.vue` component SHALL render exactly seven tabs in the order `sanitize`, `tool`, `reasoning`, `token`, `llm`, `kb_growth`, `generator`, mirroring the seven workspace-level audit JSONL files under `<workspace>/.codebus/` declared by CLAUDE.md (`七層 Audit JSONL` section). The component MUST expose an `activeTab` prop accepting any of these seven keys; passing an unrecognised key MUST be a TypeScript compile-time error.

The component MUST NOT render rows from in-source sample data. The `CB_AUDIT_SAMPLES` literal from `design/v1/shell.js` is mockup-only fixture data per `design/v1/README.md §四`; the production component MUST receive its rows via a `rows` prop (or equivalent injection) and MUST render an empty state when the array is empty. No `web/app/` source file may contain a literal copy of `CB_AUDIT_SAMPLES` or any element of it.

The component SHALL emit `select-row` with the clicked row's index in the current `rows` prop when the user clicks a row in the body. The emit MUST fire for every tab uniformly — even tabs with no overlay wiring at the parent level (the parent decides whether to react to the emit). The emit signature MUST equal `(e: 'select-row', index: number) => void`. The component MUST NOT internally render any inspector / drawer / modal in response to the click; row-click → overlay binding is a parent-layer concern. (Rationale: keeps AuditPanel a dumb display surface so per-kind inspectors — `LlmCallInspector` for `llm`, `SanitizerAuditInspector` for `sanitize`, etc. — can be hosted at page level rather than coupled into AuditPanel.)

The `sanitize` tab body MUST render each row with a placeholder identifier chip showing `<REDACTED:{kind}#{placeholder_index}>` derived from the row's `kind` and `placeholder_index` fields. The chip MUST use the `purple` token family (`bg-purple/12`, `text-purple`, `border-purple/40` or equivalent token-based utilities) — sanitizer is the exclusive owner of `purple` per the existing `Purple stays sanitizer-exclusive` Scenario in `Requirement: Design tokens originate from a single source`. The chip MUST NOT use `red`, `orange`, `yellow`, or any other token color regardless of the row's `kind` value, because those tokens are reserved for other audit-row semantics (kill / coverage / warning).

The `sanitize` tab MUST display a `pass` chip on each row showing one of the literal strings `Pass 1` / `Pass 2` / `Pass 3` (the integer `1` / `2` / `3` from the row's `pass` field mapped to human-readable labels via the same lookup the inspector uses). The chip MUST NOT show numeric `1`/`2`/`3` alone, because numeric pass values are not self-describing to a non-engineering audit reviewer.

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

#### Scenario: Sanitize tab placeholder chip uses purple token exclusively

- **WHEN** `<AuditPanel :active-tab="'sanitize'" :rows="[{rule_id:'aws_access_key',kind:'secret',placeholder_index:1,pass:1,...}]" />` is rendered
- **THEN** the row MUST contain a chip whose visible text equals `<REDACTED:secret#1>`
- **AND** the chip's class list MUST include a `purple`-family token utility (e.g., `bg-purple/12`, `text-purple`, `border-purple/40`)
- **AND** the chip's class list MUST NOT include any of `bg-red`, `bg-orange`, `bg-yellow`, `bg-green`, `bg-accent`, `bg-accent-2`, or their `text-` / `border-` variants

#### Scenario: Sanitize tab pass chip shows human-readable label, not numeric

- **WHEN** a `sanitize` row with `pass: 2` is rendered in the AuditPanel body
- **THEN** the rendered DOM MUST contain a chip with visible text exactly `Pass 2`
- **AND** the chip MUST NOT have visible text equal to the bare numeric `2`
- **AND** the equivalent labels for `pass: 1` and `pass: 3` MUST be `Pass 1` and `Pass 3` respectively

#### Scenario: Sanitize tab row click is hosted by parent SanitizerAuditInspector, not AuditPanel

- **WHEN** `<AuditPanel :active-tab="'sanitize'" :rows="entries" @select-row="parentHandler" />` is mounted with a parent that hosts `<SanitizerAuditInspector>`
- **THEN** clicking a row MUST emit `select-row` to the parent (consistent with the cross-tab emit contract)
- **AND** AuditPanel MUST NOT mount any DOM matching `SanitizerAuditInspector`, `inspector`, `drawer`, or `modal` inside its own root in response to the click
- **AND** the parent's `<SanitizerAuditInspector>` MUST be the only DOM that surfaces the row's full metadata view
