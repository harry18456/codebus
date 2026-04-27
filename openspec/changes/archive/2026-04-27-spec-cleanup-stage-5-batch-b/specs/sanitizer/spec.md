## MODIFIED Requirements

### Requirement: Pass 3 add_to_kb sanitize emits structured audit entry

The Q&A `add_to_kb` write path SHALL invoke `SanitizerEngine.sanitize(text, source=FileSource(path=chunk.source, pass_="qa_add_to_kb"))` for every chunk before any KB upsert or `kb_growth.jsonl` write, per `docs/decisions.md` D-015 and `docs/qa-agent.md §三`. Each `AuditEntry` produced MUST be appended to the workspace-scoped `SanitizerAuditLogger` with `pass_num=3` (Python keyword argument), completing the three-pass audit chain (Pass 1 = scanner ingestion, Pass 2 = TrackedProvider pre-flight, Pass 3 = Q&A add_to_kb).

**Naming convention — Python param vs JSONL key are intentionally different**: `pass_num` is the **Python keyword argument name** on `SanitizerAuditLogger.append(*, entry, pass_num, rules_version, session_id)`; the corresponding **JSONL key written to `<workspace>/.codebus/sanitize_audit.jsonl` is bare `pass`** (the integer value 1, 2, or 3 stays the same). The mismatch is deliberate: the Python name avoids shadowing the `pass` reserved keyword, while the JSONL key stays terse for readability in the audit panel. Implementers MUST NOT introduce `pass_num` as a JSONL key (e.g. by renaming the line dict's `"pass"` key) — every consumer (Trust Layer R-01 / O-05 panels, golden replay diff, audit join queries) reads the bare `pass` key directly.

`pass_num` (Python) / `pass` (JSONL) is the runtime label that downstream consumers use to attribute redactions to a sanitize stage. The `source` field on Pass 3 audit lines MUST be the structured form `{"pass": "qa_add_to_kb", "path": "<chunk.source>"}` matching the existing structured shape used by Pass 1 scanner (`{"pass": "scanner", "path": ...}`). Note that the `pass` *value* inside the `source` object is a free-form string label (e.g. `"qa_add_to_kb"` / `"scanner"` / `"explorer_read_file"` / `"find_callers"` / `"grep_search"`) distinct from the integer-valued top-level `pass` key on the same JSONL line — both keys legitimately coexist with different semantics: the top-level integer marks which of the three Sanitize passes fired, the nested string marks which call-site within that pass produced the entry.

The `SanitizeSource` discriminated union (`FileSource | MessageSource`) SHALL NOT be extended for Pass 3; the existing `FileSource.pass_` string field is the explicit extension point already promised by the foundational `Sanitizer SHALL provide a stateless engine` Requirement (which states the same class is "reusable by Pass 3 without signature change"). Adding a third union variant is forbidden by this Requirement so the audit schema remains stable across Pass 1 / Pass 3 ingestion sites.

#### Scenario: add_to_kb chunk with secret hits writes pass=3 audit line

- **WHEN** Q&A `add_to_kb` is invoked with a chunk containing a string matched by the built-in secret rule set
- **THEN** the appended `<workspace>/.codebus/sanitize_audit.jsonl` line MUST contain the JSONL key `"pass"` with integer value `3` (NOT a key named `"pass_num"` — the Python param name does not propagate to the wire schema)
- **AND** the line's `source` field MUST be the JSON object `{"pass": "qa_add_to_kb", "path": "<chunk.source>"}` (the inner `pass` string label is distinct from the top-level integer `pass` key on the same line — both legitimately coexist)
- **AND** the placeholder index MUST start at `1` for that sanitize call (Pass 3 calls share the same per-call index reset semantics as Pass 1 / Pass 2)

#### Scenario: SanitizeSource union not extended

- **WHEN** the codebase is inspected for `SanitizeSource = ` assignments in `codebus_agent.sanitizer`
- **THEN** the right-hand side MUST remain exactly `FileSource | MessageSource` — Pass 3 MUST NOT introduce a new variant such as `Pass3Source` or `QASource`

#### Scenario: Empty post-sanitize chunk still records hit lines

- **WHEN** `add_to_kb` sanitizes a chunk whose entire text gets replaced (post-sanitize text strips to empty)
- **THEN** every triggered redaction MUST still produce a `pass=3` line in `sanitize_audit.jsonl` (using the JSONL key `"pass"` with integer value `3`, not `"pass_num"`)
- **AND** the call MUST proceed to skip the KB upsert and `kb_growth.jsonl` write per the Q&A capability's empty-chunk handling — but the sanitize audit lines MUST NOT be retroactively suppressed

#### Scenario: JSONL key is bare `pass`, never `pass_num`

- **WHEN** any test reads a line from `<workspace>/.codebus/sanitize_audit.jsonl` produced by `SanitizerAuditLogger.append`
- **THEN** the parsed JSON object MUST contain a key named exactly `"pass"` with an integer value in `{1, 2, 3}`
- **AND** the parsed JSON object MUST NOT contain a key named `"pass_num"` at the top level (the Python keyword argument name MUST NOT leak to the wire schema)
- **AND** this rule applies uniformly to all three passes (Pass 1 scanner / Pass 2 provider pre-flight / Pass 3 add_to_kb) — none of them MUST emit `"pass_num"` as a JSONL top-level key
