## ADDED Requirements

### Requirement: Pass 3 add_to_kb sanitize emits structured audit entry

The Q&A `add_to_kb` write path SHALL invoke `SanitizerEngine.sanitize(text, source=FileSource(path=chunk.source, pass_="qa_add_to_kb"))` for every chunk before any KB upsert or `kb_growth.jsonl` write, per `docs/decisions.md` D-015 and `docs/qa-agent.md §三`. Each `AuditEntry` produced MUST be appended to the workspace-scoped `SanitizerAuditLogger` with `pass_num=3`, completing the three-pass audit chain (Pass 1 = scanner ingestion, Pass 2 = TrackedProvider pre-flight, Pass 3 = Q&A add_to_kb).

`pass_num` is the runtime label written into `<workspace>/.codebus/sanitize_audit.jsonl`; it MUST appear on every line and is the discriminator that downstream consumers (Trust Layer R-01 / O-05 panels, audit replay) use to attribute redactions to a sanitize stage. The `source` field on Pass 3 audit lines MUST be the structured form `{"pass": "qa_add_to_kb", "path": "<chunk.source>"}` matching the existing structured shape used by Pass 1 scanner (`{"pass": "scanner", "path": ...}`).

The `SanitizeSource` discriminated union (`FileSource | MessageSource`) SHALL NOT be extended for Pass 3; the existing `FileSource.pass_` string field is the explicit extension point already promised by the foundational `Sanitizer SHALL provide a stateless engine` Requirement (which states the same class is "reusable by Pass 3 without signature change"). Adding a third union variant is forbidden by this Requirement so the audit schema remains stable across Pass 1 / Pass 3 ingestion sites.

#### Scenario: add_to_kb chunk with secret hits writes pass_num=3 audit line

- **WHEN** Q&A `add_to_kb` is invoked with a chunk containing a string matched by the built-in secret rule set
- **THEN** the appended `<workspace>/.codebus/sanitize_audit.jsonl` line MUST contain `"pass_num": 3`
- **AND** the line's `source` field MUST be the JSON object `{"pass": "qa_add_to_kb", "path": "<chunk.source>"}`
- **AND** the placeholder index MUST start at `1` for that sanitize call (Pass 3 calls share the same per-call index reset semantics as Pass 1 / Pass 2)

#### Scenario: SanitizeSource union not extended

- **WHEN** the codebase is inspected for `SanitizeSource = ` assignments in `codebus_agent.sanitizer`
- **THEN** the right-hand side MUST remain exactly `FileSource | MessageSource` — Pass 3 MUST NOT introduce a new variant such as `Pass3Source` or `QASource`

#### Scenario: Empty post-sanitize chunk still records hit lines

- **WHEN** `add_to_kb` sanitizes a chunk whose entire text gets replaced (post-sanitize text strips to empty)
- **THEN** every triggered redaction MUST still produce a `pass_num=3` line in `sanitize_audit.jsonl`
- **AND** the call MUST proceed to skip the KB upsert and `kb_growth.jsonl` write per the Q&A capability's empty-chunk handling — but the sanitize audit lines MUST NOT be retroactively suppressed
