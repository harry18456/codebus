## MODIFIED Requirements

### Requirement: SanitizerAuditLogger appends each replacement to JSONL

The sidecar SHALL expose a `SanitizerAuditLogger` that writes one JSON object per replacement to `{workspace}/.codebus/sanitize_audit.jsonl`. The file SHALL be append-only and each line SHALL be a single JSON object terminated by `\n`.

The `sanitize_audit.jsonl` filename literal MUST appear in `sidecar/src/codebus_agent/_audit_paths.py` as the canonical `_SANITIZE_AUDIT_FILENAME` constant. All other modules in `sidecar/src/codebus_agent/` that reference the filename MUST import the constant from `_audit_paths` (or its backward-compat shim `codebus_agent.api._audit_paths`); they MUST NOT redeclare the literal string. This rule generalises the equivalent constraint that the `kb-growth` capability already imposes on `kb_growth.jsonl`, extending the single-source contract to all seven workspace-level audit JSONL filenames.

Each audit line's `pass` integer MUST agree with the line's `source` shape per the cross-cutting `pass_num to source-type invariant`: `pass=1` (Pass 1, file-reading-stage Sanitize) MUST carry a file-source (`source` JSON-serialized shape reflects `FileSource(path=..., pass_=...)`); `pass=2` (Pass 2, Provider pre-flight Sanitize) MUST carry a message-source (`source` JSON-serialized shape reflects `MessageSource(message_id=...)`); `pass=3` (Pass 3, Q&A `add_to_kb` Sanitize) MAY carry either file-source or message-source because the Q&A path can sanitize file-derived chunks or chat-channel content (`docs/decisions.md` D-016). This invariant is the single semantic anchor that lets Trust Layer R-01 panel group Pass 1 vs Pass 2 redactions by source type without inspecting the underlying source string format.

#### Scenario: Audit log line contains required fields

- **WHEN** a Pass 1 sanitize replaces one value in `src/app.py`
- **THEN** `sanitize_audit.jsonl` MUST have one appended line containing fields `ts` (ISO 8601 UTC), `schema_version` (integer), `rules_version` (string), `pass` (integer 1, 2, or 3), `session_id` (UUID string), `source` (string prefixed with `file:` or `message:`), `rule_id` (string), `kind` (string), `placeholder_index` (integer), and `extra` (object)
- **AND** the line MUST NOT contain the original pre-sanitize value nor the surrounding context text

#### Scenario: Schema version starts at 1

- **WHEN** any audit line is written by this change
- **THEN** its `schema_version` field MUST equal `1`

#### Scenario: Concurrent writes append atomically

- **WHEN** two Pass calls write audit entries from two threads in the same process
- **THEN** every written line MUST be a complete JSON object terminated by `\n` and MUST NOT be interleaved with another line's bytes

#### Scenario: Filename literal is single-sourced in canonical leaf module

- **WHEN** any test scans `sidecar/src/codebus_agent/` for the regex `['\"][\w_-]+\.jsonl['\"]` (any `.jsonl` quoted string literal)
- **THEN** every match MUST originate from `sidecar/src/codebus_agent/_audit_paths.py`
- **AND** no other module in the package tree MUST contain a `*.jsonl` quoted string literal
- **AND** the rule applies to all seven workspace-level audit filenames: `sanitize_audit.jsonl`, `tool_audit.jsonl`, `token_usage.jsonl`, `llm_calls.jsonl`, `reasoning_log.jsonl`, `generator_log.jsonl`, `kb_growth.jsonl`

#### Scenario: pass_num to source-type invariant

- **WHEN** any test scans `<workspace>/.codebus/sanitize_audit.jsonl` for every line written by sidecar production code
- **THEN** every line whose `pass` field equals `1` MUST carry a `source` whose serialized shape reflects `FileSource(path=..., pass_=...)` — NEVER a `MessageSource(...)` shape
- **AND** every line whose `pass` field equals `2` MUST carry a `source` whose serialized shape reflects `MessageSource(message_id=...)` — NEVER a `FileSource(...)` shape
- **AND** every line whose `pass` field equals `3` MAY carry either a `FileSource` or `MessageSource` shape (Q&A `add_to_kb` accepts both per D-016)

#### Scenario: Explorer tool error path runs Pass 2 sanitize

- **WHEN** the Explorer ReAct loop's `_execute_tools` catches an exception from a tool invocation and the resulting error string is about to be written into `ToolResult.output`
- **THEN** the production code path MUST invoke `SanitizerEngine.sanitize(error_text, source=MessageSource(message_id=f"explorer_step_{step_idx}_tool_error"))` before populating `ToolResult.output`
- **AND** any sanitize hits MUST produce one `<workspace>/.codebus/sanitize_audit.jsonl` line per hit with `pass=2` and `source` shape reflecting `MessageSource(message_id=...)`
- **AND** the `ToolResult.output` ultimately written into the `Step` log MUST be the post-sanitize string (containing `<REDACTED:>` placeholders for any redactions)
