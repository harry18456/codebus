# Audit fixtures

> Bootstrap by change `llm-call-inspector-p0`. Vitest only — these files MUST NOT be referenced from `web/app/**` (production).

## `llm-calls.json`

Flat JSON array of `LlmCallEntry` objects matching the schema written by the sidecar `LLMCallLogger._base_entry` (see `sidecar/src/codebus_agent/providers/llm_call_logger.py`). 6 entries cover the spec's required diversity:

| Entry | Role | Module | Sanitizer Pass 2 | Special |
| --- | --- | --- | --- | --- |
| `req_001` | reasoning | explorer | `true` | baseline chat call |
| `req_002` | judge | judge | `true` | baseline judge call |
| `req_003` | embed | kb_build | `false` | embed lane (`response: null`) |
| `req_004` | chat | qa_agent | `true` | qa lane (synthesize answer) |
| `req_005` | reasoning | explorer | `false` | failed call (`response: null`, `error.class: "TimeoutError"`, `cost_usd: 0`, `latency_ms: null`) |
| `req_dup_target` | chat | qa_agent | `true` | designated dedup target — tests pair it with a live-tail SSE event carrying the same `request_id` |

### Spec scenario coverage

| Scenario | Driven by |
| --- | --- |
| `Initial load populates entries from Tauri command` | All 6 entries returned |
| `E_AUDIT_TOO_LARGE surfaces as Error with code in message` | Mocked Tauri rejection (no fixture content involved) |
| `Live-tail appends llm_call SSE events while explorer stream emits them` | Test injects 2 SSE events with new `request_id` values |
| `Live-tail ignores non-llm kinds` | Mocked Tauri returns `[]`, test uses `kind: 'sanitize'` |
| `Dedup by request_id prevents disk + SSE double-push` | SSE event carrying `request_id: "req_dup_target"` (same as 6th fixture entry) |

### Regenerate

If `LLMCallLogger._base_entry` schema changes (new field, renamed field), update each entry above to match. Add a row to the table when introducing new diversity (a new role, a new module).

## `sanitizer-rules.json`

> Bootstrap by change `sanitizer-audit-inspector-p0`.

Snapshot of `GET /sanitizer/rules` response: 5 SanitizerRule entries covering 3 kinds (`email` / `secret` / `id` / `internal-domain` / `allowlist`) and 2 sources (`builtin` × 4 + `user_yaml` × 1). At least one entry uses the `<email RFC 5322>` summary form mandated by spec scenario `pattern_summary is not raw regex source`.

### Spec scenario coverage

| Scenario | Driven by |
| --- | --- |
| `Rules fetched once per session` | All 5 entries — composable resolves snapshot once, cache returns it on second call |
| `lookup returns matching rule` | `lookup('detect_secrets_aws_v1')` returns the AWS row |
| `lookup returns null for unknown rule_id` | Test calls `lookup('nonexistent_rule_xyz')` — fixture deliberately omits that id |
| `Composable does not request full regex source` | Source-grep test on `useSanitizerRules.ts` — fixture is unrelated to that grep |

## `sanitize-audit.jsonl`

> Bootstrap by change `sanitizer-audit-inspector-p0`.

Newline-delimited JSON of `sanitize_audit.jsonl` rows matching the schema written by `SanitizerAuditLogger.append` (see `sidecar/src/codebus_agent/sanitizer/audit.py`). 8 entries cover the spec's required diversity:

| Row | `pass` | `kind` | `source` shape | `extra` | Special |
| --- | --- | --- | --- | --- | --- |
| 1 | 1 | email | dict (`{pass: scanner, path}`) | `{}` | Pass 1 baseline scanner row |
| 2 | 1 | secret | dict (`{pass: scanner, path}`) | `{allowlisted: true}` | allowlisted hit chip rendering |
| 3 | 2 | email | string (`file:<path>`) | `{}` | Pass 2 provider pre-flight, legacy string source form |
| 4 | 2 | secret | string (`message:<id>`) | `{}` | Pass 2 message-source form |
| 5 | 2 | id | string (`file:<path>`) | `{}` | Pass 2 different kind for `kindSummary` reactivity |
| 6 | 3 | internal-domain | string (`file:<path>`) | `{}` | Pass 3 add_to_kb path |
| 7 | 3 | email | dict (`{pass: add_to_kb, path}`) | `{}` | Pass 3 dict-source form |
| 8 | 1 | email | dict (`{pass: scanner, path}`) | `{}` | Second `session_id` for `sessionTimeline` grouping |

### Spec scenario coverage

| Scenario | Driven by |
| --- | --- |
| `Source dict form parsed into human-readable view` | Rows 1, 2, 7, 8 |
| `Source string forms parsed into human-readable view` | Rows 3, 4, 5, 6 |
| `kindSummary counts unique kinds reactively` | All 8 rows |
| `sessionTimeline groups and sorts by ts` | Two distinct `session_id` values across the 8 rows |
| `Composable does not call read_audit_jsonl directly` | Source-grep test on `useSanitizeAudit.ts` |
