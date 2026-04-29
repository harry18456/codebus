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
