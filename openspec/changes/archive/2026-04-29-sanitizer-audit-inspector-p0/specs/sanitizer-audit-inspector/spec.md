## ADDED Requirements

### Requirement: `SanitizerAuditInspector` overlay renders metadata-only view of a sanitize_audit row

The `SanitizerAuditInspector.vue` component SHALL accept a `row` prop typed as the runtime shape of a single `sanitize_audit.jsonl` line and render exactly the ten declared metadata fields visible to the user: `ts`, `schema_version`, `rules_version`, `pass`, `session_id`, `source`, `rule_id`, `kind`, `placeholder_index`, and `extra`. The component MUST NOT render, fetch, or compute any value that could reveal the pre-sanitize raw text — including but not limited to: source-file content reads, KB chunk lookups by `placeholder_index`, regex re-execution against any string that resembles the original match, or attempts to invert the placeholder back to the original value via heuristics. (Rationale: D-015 declares sanitizer replacement as one-way and forbids reverse-mapping storage; the inspector exists to let users verify the audit trail, not to bypass it.)

The component MUST display the `pass` integer as one of three human-readable labels: `1` → `Pass 1 · Scanner (KB ingestion)`, `2` → `Pass 2 · Provider pre-flight (LLM call)`, `3` → `Pass 3 · Q&A add_to_kb`. The label mapping MUST live in a single TypeScript constant exported from the inspector's module so the same lookup is used by `AuditPanel`'s row-level chip and the inspector's header — drift between row chip and inspector is a defensive-test failure.

The component MUST display the `source` field handling both wire shapes the sidecar emits: structured dict `{"pass": "scanner", "path": "<path>"}` and string forms `"file:<path>"` / `"message:<message_id>"`. The displayed value MUST NOT be a raw `JSON.stringify` of the wire payload; the component MUST extract the human-readable parts (path or message id) and render a labeled row. Unknown source shapes MUST render as `(unknown source format)` rather than throwing.

The component MUST display the `extra` field as a generic key-value list (one row per key in `extra`). When a key equals the literal string `allowlisted` and the value is `true`, the component MUST render a green checkmark chip `✓ allowlisted` rather than the generic `key: value` row, because allowlisting is a frequent and semantically distinct case worth surfacing prominently.

The component MUST NOT render an inspector close button that mutates the `sanitize_audit.jsonl` file or any other audit log; the close action is a UI-only state change scoped to the parent page. The component MUST NOT expose any "delete row" / "edit row" / "redact further" affordance — audit logs are append-only per the existing `sanitizer` capability invariant.

#### Scenario: All ten metadata fields rendered when row has full payload

- **WHEN** `<SanitizerAuditInspector :row="{ts:'2026-04-29T08:30:01.123Z', schema_version:1, rules_version:'2026.04', pass:2, session_id:'sess_abc123', source:{pass:'provider', path:'src/auth.ts'}, rule_id:'aws_access_key', kind:'secret', placeholder_index:1, extra:{}}" />` is mounted
- **THEN** the rendered DOM MUST contain visible text for each of the ten field values (rendered with their canonical labels)
- **AND** the placeholder identifier `<REDACTED:secret#1>` MUST be visible at least once in the inspector header

#### Scenario: Pass integer mapped to human-readable label

- **WHEN** the inspector renders a row with `pass: 1`
- **THEN** the visible header text MUST contain the substring `Pass 1 · Scanner (KB ingestion)`
- **AND** the same row rendered with `pass: 2` MUST instead contain `Pass 2 · Provider pre-flight (LLM call)`
- **AND** the same row rendered with `pass: 3` MUST instead contain `Pass 3 · Q&A add_to_kb`

#### Scenario: No raw value reconstruction attempted

- **WHEN** the inspector mounts and processes a row with `rule_id: 'aws_access_key'`, `kind: 'secret'`, `source: 'file:src/auth.ts'`
- **THEN** the inspector's network log MUST NOT show any HTTP call to a sidecar endpoint that reads the source file at `src/auth.ts`
- **AND** the inspector's DOM MUST NOT contain any text matching `AKIA[0-9A-Z]{16}` or other regex patterns associated with the row's `rule_id`
- **AND** the inspector MUST NOT call `read_audit_jsonl` for any audit_kind other than `sanitize` (e.g., reading `kb_growth` to look up the `placeholder_index` against KB chunks is forbidden)

#### Scenario: extra.allowlisted=true rendered as checkmark chip

- **WHEN** the inspector renders a row with `extra: {allowlisted: true}`
- **THEN** the rendered DOM MUST contain a chip with visible text `✓ allowlisted` (with green token color)
- **AND** the rendered DOM MUST NOT contain a generic `allowlisted: true` key-value row alongside the chip (the chip replaces the generic rendering)

#### Scenario: Unknown source shape renders fallback, not throws

- **WHEN** the inspector receives a row whose `source` field is an unrecognised dict shape such as `{foo: "bar"}`
- **THEN** the source row MUST render visible text `(unknown source format)` and a `<details>` collapsible whose expanded content shows the raw JSON
- **AND** the component MUST NOT throw an exception or log an `error`-level console message

#### Scenario: No mutation affordances exposed

- **WHEN** the inspector is mounted with any valid row
- **THEN** the rendered DOM MUST NOT contain any button, link, or interactive element whose label or aria-label contains the substrings `delete`, `edit`, `remove`, `redact further`, or `modify`
- **AND** the close action (if present) MUST only emit a UI-state event (e.g., `close`) and MUST NOT issue any HTTP call

---
### Requirement: `SanitizerAuditInspector` displays a D-015 banner verbatim

The component SHALL display a sticky banner at the top of the overlay with the literal text:

```
Audit metadata only · raw values are not retained per D-015.
Placeholder reveal requires a future audit-unlock capability.
```

The banner MUST appear on every render — it MUST NOT be dismissible, collapsible, hidden behind a toggle, or conditionally rendered based on `row` content. (Rationale: D-015 is the load-bearing invariant the inspector exists to validate; suppressing the banner under any condition would let an implementer or reviewer mistake the inspector's intent.)

The banner's exact string MUST live in a single exported TypeScript constant `SANITIZER_AUDIT_BANNER` so the same string can be reused by `/audit/sanitizer` standalone page and the AuditPanel `sanitize` tab sticky header — drift between sites is a defensive-test failure.

#### Scenario: Banner is always rendered

- **WHEN** the inspector is mounted with any valid row
- **THEN** the rendered DOM MUST contain visible text matching the literal string `Audit metadata only · raw values are not retained per D-015.`
- **AND** the rendered DOM MUST contain visible text matching the literal string `Placeholder reveal requires a future audit-unlock capability.`

#### Scenario: Banner cannot be hidden by props or user action

- **WHEN** the inspector is mounted with any combination of props
- **THEN** there MUST NOT exist a prop whose name matches `hideBanner`, `dismissBanner`, `bannerVisible`, or any case-insensitive variant
- **AND** there MUST NOT exist a button or interactive element whose click would remove the banner from the DOM

#### Scenario: Banner string lives in a single constant

- **WHEN** the entire `web/app/` tree is grepped for the literal string `raw values are not retained per D-015`
- **THEN** the only matching file MUST be the inspector's TypeScript module exporting `SANITIZER_AUDIT_BANNER`
- **AND** the AuditPanel `sanitize` tab sticky header and `pages/audit/sanitizer.vue` MUST reference the constant by import, not by inline string literal

---
### Requirement: `useSanitizeAudit` composable parses sanitize_audit rows into a view-model

The `useSanitizeAudit` composable SHALL be a thin wrapper over `useAuditJsonl('sanitize')` that augments raw rows with three derived view-model fields without mutating the source rows: `sourceView` (the human-readable parsed source), `placeholderToken` (the rendered chip text `<REDACTED:{kind}#{placeholder_index}>`), and `passLabel` (the same human-readable label `SanitizerAuditInspector` uses).

The composable MUST also expose two derived reactive collections:

1. `kindSummary` — a `Map<string, number>` mapping each unique `kind` value in the current row set to its count, recomputed reactively when rows change.
2. `sessionTimeline` — a `Map<string, Row[]>` mapping each unique `session_id` to its rows sorted by `ts` ascending, recomputed reactively when rows change.

The composable MUST NOT call `read_audit_jsonl`, mount any HTTP listener, or maintain its own polling timer — those concerns belong to the underlying `useAuditJsonl('sanitize')`. The composable MUST NOT touch `useSanitizerRules`; rule explainer integration is the inspector's parent-layer concern, not the composable's.

The composable MUST NOT cache or persist any row data outside the reactive computed wrappers — its lifetime is bounded by the consuming component's lifecycle, mirroring `useAuditJsonl`'s lifetime model.

#### Scenario: Source dict form parsed into human-readable view

- **WHEN** `useSanitizeAudit` processes a row with `source: {pass: 'scanner', path: 'src/auth.ts'}`
- **THEN** the row's `sourceView` MUST equal `{kind: 'file', pass: 'scanner', path: 'src/auth.ts', label: 'Scanner · src/auth.ts'}`

#### Scenario: Source string forms parsed into human-readable view

- **WHEN** `useSanitizeAudit` processes a row with `source: 'file:src/config.py'`
- **THEN** the row's `sourceView` MUST equal `{kind: 'file', pass: null, path: 'src/config.py', label: 'src/config.py'}`
- **AND** a row with `source: 'message:msg_abc123'` MUST yield `{kind: 'message', pass: null, message_id: 'msg_abc123', label: 'message msg_abc123'}`

#### Scenario: kindSummary counts unique kinds reactively

- **WHEN** the underlying rows array contains 5 rows with kinds `['secret', 'pii', 'pii', 'internal', 'secret']`
- **THEN** `kindSummary.value` MUST equal `Map { 'secret' => 2, 'pii' => 2, 'internal' => 1 }`
- **AND** when a sixth row with `kind: 'pii'` is appended, `kindSummary.value` MUST recompute to `Map { 'secret' => 2, 'pii' => 3, 'internal' => 1 }`

#### Scenario: sessionTimeline groups and sorts by ts

- **WHEN** the rows contain three entries from `session_id: 'sess_a'` with timestamps `['2026-04-29T08:00Z', '2026-04-29T07:30Z', '2026-04-29T08:15Z']` and one from `session_id: 'sess_b'`
- **THEN** `sessionTimeline.value.get('sess_a')` MUST be a length-3 array sorted in ascending `ts` order: `'07:30Z'` first, `'08:00Z'` second, `'08:15Z'` third
- **AND** `sessionTimeline.value.get('sess_b')` MUST be a length-1 array

#### Scenario: Composable does not call read_audit_jsonl directly

- **WHEN** the entire `web/app/composables/useSanitizeAudit.ts` source is grepped for the symbol `read_audit_jsonl`
- **THEN** zero matches MUST be returned (the symbol may only appear via the imported `useAuditJsonl` dependency, not at this composable's call site)

---
### Requirement: `useSanitizerRules` composable fetches rules registry from sidecar

The `useSanitizerRules` composable SHALL fetch the sanitizer rules registry from the sidecar via `useSidecar().fetch('/sanitizer/rules')` exactly once per session, caching the result in a module-level `Ref` so subsequent calls within the same sidecar process lifetime return the cached snapshot without re-fetching. (Rationale per design Decision 2 and Decision 5: rules are immutable within a sidecar process; bumping `rules_version` requires sidecar restart per `docs/sanitizer.md §六`.)

The composable MUST expose a `lookup(rule_id: string): SanitizerRule | null` function returning the registry entry whose `rule_id` matches, or `null` when no entry matches. The function MUST NOT throw on unknown `rule_id` — inspector display falls back to the raw `rule_id` string when lookup returns `null`.

The composable MUST NOT request additional fields from the sidecar beyond the documented endpoint shape (see `Requirement: GET /sanitizer/rules sidecar endpoint`); specifically, it MUST NOT request full regex source via a query parameter, even if a future endpoint variant exposes one — that decision is reserved for a separate change.

#### Scenario: Rules fetched once per session

- **WHEN** `useSanitizerRules` is invoked from two different components mounted at different times within the same Nuxt session
- **THEN** the underlying `useSidecar().fetch('/sanitizer/rules')` call MUST execute exactly once
- **AND** the second consumer MUST receive the cached snapshot (verifiable via a network spy)

#### Scenario: lookup returns matching rule

- **WHEN** the rules registry contains a rule `{rule_id: 'aws_access_key', kind: 'secret', description: 'AWS access key (static credential)', pattern_summary: 'AKIA[0-9A-Z]{16}', source: 'builtin'}`
- **AND** `lookup('aws_access_key')` is called
- **THEN** the return value MUST equal that rule object

#### Scenario: lookup returns null for unknown rule_id

- **WHEN** `lookup('nonexistent_rule_xyz')` is called against any rules registry that does not contain that rule_id
- **THEN** the return value MUST be `null`
- **AND** the function MUST NOT throw

#### Scenario: Composable does not request full regex source

- **WHEN** the entire `web/app/composables/useSanitizerRules.ts` source is grepped for the substrings `pattern_full`, `regex_full`, `?full=true`, or `&full=true`
- **THEN** zero matches MUST be returned

---
### Requirement: `GET /sanitizer/rules` sidecar endpoint exposes rules registry snapshot

The sidecar SHALL expose a `GET /sanitizer/rules` endpoint that returns the current effective rules registry snapshot as JSON. The endpoint MUST require the standard sidecar bearer token (consistent with all other sidecar endpoints). The endpoint MUST be read-only — it MUST NOT accept query parameters that mutate state, MUST NOT accept a request body, and MUST NOT trigger any sanitizer engine state change as a side effect of being called.

The response payload MUST conform to this schema:

```json
{
  "rules_version": "<semver string>",
  "rules": [
    {
      "rule_id": "<string>",
      "kind": "<string>",
      "description": "<string>",
      "pattern_summary": "<string>",
      "source": "builtin"
    }
  ]
}
```

The `rules_version` field MUST equal the value the `SanitizerEngine` writes into `sanitize_audit.jsonl` — drift between endpoint response and audit log entries is a defensive-test failure.

The `pattern_summary` field MUST be a human-readable summary string suitable for display in the inspector's rule explainer (e.g., `"AKIA[0-9A-Z]{16}"` for AWS keys, `"<email RFC 5322>"` for email regex). It MUST NOT be the full executable regex source code, because exposing executable patterns inflates response size and offers limited value to non-engineering audit reviewers per design Decision 2.

The `source` field MUST be one of the literal strings `"builtin"` or `"user_yaml"`, marking each rule's origin (built-in registry vs. `~/.codebus/sanitizer.local.yaml`). Future rule sources require an explicit spec extension before being added to this enum.

When the rules registry is empty (an unusual but valid state during startup), the endpoint MUST return `200 OK` with `rules: []` rather than `404`.

#### Scenario: Authenticated GET returns rules list

- **WHEN** the endpoint is called with a valid bearer token
- **THEN** the response status MUST be `200 OK`
- **AND** the response body MUST be a JSON object with keys exactly `rules_version` and `rules`
- **AND** every entry in `rules` MUST have keys exactly `rule_id`, `kind`, `description`, `pattern_summary`, `source`

#### Scenario: Missing bearer rejected

- **WHEN** the endpoint is called without an `Authorization: Bearer ...` header
- **THEN** the response status MUST be `401`
- **AND** no rules data MUST be included in the response body

#### Scenario: rules_version matches sanitize_audit.jsonl writes

- **WHEN** the endpoint is called and returns `rules_version: "2026.04"`
- **AND** the same sidecar process subsequently writes a row to `sanitize_audit.jsonl`
- **THEN** that audit row's `rules_version` field MUST equal `"2026.04"`

#### Scenario: Endpoint is read-only

- **WHEN** the endpoint is called multiple times in succession
- **THEN** no `sanitize_audit.jsonl` rows MUST be written as a result of the calls
- **AND** the `SanitizerEngine` rules registry in memory MUST remain unchanged (verifiable via a follow-up call returning identical `rules` array)

#### Scenario: pattern_summary is not raw regex source

- **WHEN** any rule in the response has `rule_id: 'aws_access_key'`
- **THEN** that rule's `pattern_summary` MUST NOT contain the substring `(?P<` (named capture groups), `(?:`, or other features that would only appear in executable Python regex source but not in a human-readable summary
- **AND** the `pattern_summary` length MUST NOT exceed 80 characters

#### Scenario: Empty registry returns 200 with empty rules

- **WHEN** the registry is empty (no built-in rules and no user yaml)
- **THEN** the response status MUST be `200 OK`
- **AND** the response body's `rules` array MUST be empty `[]`

---
### Requirement: `/audit/sanitizer` standalone page surfaces inspector outside R-01 workspace

The Nuxt page at `web/app/pages/audit/sanitizer.vue` SHALL render the same `SanitizerAuditInspector` overlay used inside R-01 station pages and the Explorer console page, but reachable via the route `/audit/sanitizer` without requiring an active workspace context. The page MUST accept an optional `?workspace=<workspace_id>` query parameter to scope which `<workspace>/.codebus/sanitize_audit.jsonl` is read; absent the parameter, the page MUST render an empty state explaining how to provide the workspace.

The page MUST display the D-015 banner from the `SANITIZER_AUDIT_BANNER` constant at the top, before any row rendering, mirroring its presentation inside the inspector overlay.

The page MUST NOT issue any sidecar call other than `GET /sanitizer/rules` and the `read_audit_jsonl` Tauri command for `audit_kind: 'sanitize'`. The page MUST NOT invoke any LLM provider, scanner, generator, or other audit-kind reads — it is a focused reviewer page, not a workspace surface.

#### Scenario: Page renders inspector with workspace query

- **WHEN** the user navigates to `/audit/sanitizer?workspace=<existing_workspace_id>`
- **THEN** the page MUST render the `SanitizerAuditInspector` listing the rows from that workspace's `sanitize_audit.jsonl`
- **AND** the D-015 banner MUST be visible at the top of the page
- **AND** the page MUST NOT show any R-01 station chrome (no station nav, no MOC index, no workspace breadcrumb)

#### Scenario: Page renders empty state without workspace query

- **WHEN** the user navigates to `/audit/sanitizer` without any query parameter
- **THEN** the page MUST render a documented empty-state message instructing the user to provide `?workspace=...`
- **AND** the page MUST NOT issue any `read_audit_jsonl` Tauri call (no workspace to read)
- **AND** the D-015 banner MUST still be visible at the top

#### Scenario: Page does not call non-sanitize audit reads

- **WHEN** the page is mounted with a valid `workspace` query parameter
- **THEN** the network spy MUST observe at most one Tauri `read_audit_jsonl` call with `audit_kind: 'sanitize'`
- **AND** the spy MUST NOT observe any other `read_audit_jsonl` call (no `llm`, `tool`, `reasoning`, `token`, `kb_growth`, or `generator` reads)
- **AND** the spy MUST NOT observe any HTTP call to sidecar endpoints other than `/sanitizer/rules` and the standard health/IPC bootstrapping
