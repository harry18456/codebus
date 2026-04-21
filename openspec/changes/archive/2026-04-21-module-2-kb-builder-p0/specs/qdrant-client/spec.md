## ADDED Requirements

### Requirement: KB-facing vector upsert helper

The `codebus_agent.kb.qdrant_client` wrapper module SHALL expose an async helper `upsert_points(client, collection, points)` that writes a batch of points (each carrying `id`, `vector`, and a `KBPayload`-dumped mapping) to the named collection. The helper MUST serialize payloads via `KBPayload.model_dump(mode="json")` so datetime fields round-trip as ISO-8601 strings. Runtime code outside `codebus_agent/kb/` MUST NOT import `qdrant_client` directly; the existing runtime-import restriction continues to apply to the new helper.

#### Scenario: Upsert writes all points to the named collection

- **WHEN** `upsert_points(client, "codebus_demo", [p1, p2, p3])` is called against an existing collection
- **THEN** after the call, searching the collection with each point's vector MUST return that point's id

#### Scenario: Payload datetimes serialized as ISO-8601

- **WHEN** `upsert_points` is called with a point whose payload contains a `created_at` datetime
- **THEN** reading the point back via the SDK MUST yield a string matching ISO-8601 format, and `datetime.fromisoformat(...)` on that string MUST equal the original value

---

### Requirement: KB-facing vector search helper

The wrapper SHALL expose `search_points(client, collection, vector, *, limit, query_filter=None)` that returns hits with `id`, `score`, and the deserialized `KBPayload`. The helper MUST accept an optional `query_filter` dict that applies Qdrant payload filtering (at minimum supporting equality on `file_path` and `source_kind` and membership on `related_stations`). The helper MUST NOT raise when the collection is empty; it MUST return an empty list.

#### Scenario: Search returns scored hits ordered by score

- **WHEN** `search_points(client, "codebus_demo", vec, limit=5)` is called against a collection holding at least 5 points
- **THEN** the returned list MUST have at most 5 entries and scores MUST be monotonically non-increasing

#### Scenario: Empty collection returns empty list

- **WHEN** `search_points` is called against a freshly created, empty collection
- **THEN** the return value MUST be `[]` and no exception MUST be raised

#### Scenario: Filter on file_path restricts results

- **WHEN** `search_points(client, collection, vec, limit=10, query_filter={"file_path": "src/x.ts"})` is called
- **THEN** every returned hit's payload `file_path` MUST equal `"src/x.ts"`

---

### Requirement: Hash existence helper for deduplication

The wrapper SHALL expose `exists_by_hash(client, collection, text_hash) -> bool` that returns `True` when the collection contains at least one point whose payload `text_hash` equals the given value, and `False` otherwise. The helper MUST return `False` when the collection does not exist rather than raising.

#### Scenario: Hash present returns True

- **WHEN** `exists_by_hash(client, collection, h)` is called against a collection that contains a point with payload `text_hash=h`
- **THEN** the return value MUST be `True`

#### Scenario: Hash absent returns False

- **WHEN** `exists_by_hash(client, collection, "deadbeef" * 8)` is called against a collection with no matching point
- **THEN** the return value MUST be `False`

#### Scenario: Missing collection reports False, not exception

- **WHEN** `exists_by_hash` is called against a collection name that does not exist
- **THEN** the return value MUST be `False` and no exception MUST propagate

---

### Requirement: Idempotent KB payload index provisioning

The wrapper SHALL expose `ensure_kb_payload_indices(client, collection)` that creates keyword payload indices for the fields `text_hash` and `related_stations` on the given collection. The helper MUST be idempotent: repeated invocations MUST succeed without raising and MUST NOT alter index configuration once created.

#### Scenario: Indices created when absent

- **WHEN** `ensure_kb_payload_indices(client, "codebus_demo")` is called against a collection that has no payload indices
- **THEN** after the call, filtering searches on `text_hash` and `related_stations` MUST be supported by the collection's index configuration

#### Scenario: Repeated invocation no-op

- **WHEN** `ensure_kb_payload_indices` is called twice in succession on the same collection
- **THEN** the second call MUST NOT raise and MUST NOT modify existing indices
