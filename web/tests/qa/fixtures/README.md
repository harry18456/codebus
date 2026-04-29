# Q&A fixtures

> Bootstrap by change `qa-overlay-p0`. Vitest only — these files MUST NOT be referenced from `web/app/**` (production).

## `qa-stream.json`

JSON array of `{ type, data }` SSE event records mirroring the wire shape that `useSseTask` exposes (`SseEvent[]`). Covers the full Q&A turn lifecycle:

| Event | Fixture coverage |
| --- | --- |
| `rag_hits` | 1 event with 3 hits, all carrying `related_stations` |
| `agent_thought` | 2 events (steps 0 / 1) with `thought` + `action` payloads |
| `agent_action_result` | 3 events — step 0 (tokens_used: 0 placeholder), step 1 (read_file success), step 1 (search whose `observation` starts with `error:` so the `isError` heuristic flips, mirroring `agent-console-p0`) |
| `kb_growth` | 2 events; the first uses `entry_id: "a14f9c2e"` which **also appears in `kb-growth.json`** to drive the disk + SSE dedup test |
| `qa_answer` | 1 event with 2 citations, both carrying `related_stations` for the station-chip emit test |
| `done` | 1 terminal event |

### Spec scenario coverage

| Scenario | Driven by |
| --- | --- |
| `rag_hits event populates the active turn's ragHits` | The single `rag_hits` event with 3 hits |
| `kb_growth dedup by entry_id` | Two `kb_growth` events with distinct ids; reuse `a14f9c2e` against `kb-growth.json` |
| `done event flips status exactly once` | Terminal `done` event |
| `<QaTurnCard> renders four phases per turn — All four phases render when turn is complete` | The complete sequence yields a turn with ragHits + reactSteps + answer all populated |
| `<QaTurnCard> error status surfaces error message` | Action result with `error:` prefix triggers the isError heuristic |

## `kb-growth.json`

JSON array of `KbGrowthEntry` objects matching the schema written by sidecar `KBGrowthLogger.write` (`sidecar/src/codebus_agent/kb/growth_logger.py`). Four entries cover:

| Entry | Special |
| --- | --- |
| `9d3f0a7b` | Old session, single `related_stations` |
| `f1e22ac4` | Old session, `sanitize_stats.hits > 0` (sanitize fired during ingestion) |
| `a14f9c2e` | **Designated dedup target** — the same `entry_id` also appears in the `kb_growth` SSE event in `qa-stream.json`. Tests pair the disk row with a live-tail event carrying the same id to verify `useAuditJsonl` dedup. |
| `c0a9d7e3` | Different session, validates session_id grouping |

### Spec scenario coverage

| Scenario | Driven by |
| --- | --- |
| `kb_growth live-tail appends QA SSE events into entries` | Disk = 4 entries; SSE adds 2 with one new id (`b27c001f`) ⇒ entries.length = 5 |
| `Dedup by entry_id prevents disk + SSE double-push` | SSE event with `entry_id: "a14f9c2e"` finds existing disk row; entries.length unchanged |
| `liveTailFromQaSession ignored when kind is not kb_growth` | Disk read with `kind: 'tool'` ignores all SSE events |

### Regenerate

If `KBGrowthLogger.write` schema changes (new field, renamed field), update each entry above to match. Add a row to the table when introducing new diversity (a new `event_type`, a new sanitize_stats shape).
