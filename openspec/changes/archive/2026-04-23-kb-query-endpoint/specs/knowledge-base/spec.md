## ADDED Requirements

### Requirement: POST /kb/query endpoint

The sidecar SHALL expose a synchronous `POST /kb/query` HTTP endpoint that accepts a JSON body `{workspace_root: str, text: str, top_k: int = 8, filter_path: str | None = None, filter_source_kind: list[str] | None = None}` and returns a `200 OK` response with body `{"hits": [...]}` where each entry conforms to the `KBHit` schema (point_id / score / payload). The endpoint SHALL embed `text` via the workspace-scoped TrackedProvider (per `KB build production dependency wiring` factory), search the workspace's Qdrant collection, and return hits ordered by descending score, delegating to `KnowledgeBase.query(...)`. Unlike `POST /kb/build`, this endpoint MUST be synchronous (no task handle, no SSE) because typical query latency is below 1 second.

#### Scenario: Successful query returns hits ordered by score

- **WHEN** the caller posts `{"workspace_root": "/abs/ws", "text": "storage", "top_k": 3}` against a populated workspace with a valid bearer token
- **THEN** the response status MUST be `200`, the response body MUST contain `"hits"` (a list of at most 3 entries), and each entry MUST contain `point_id`, `score`, and `payload` fields with scores monotonically non-increasing

#### Scenario: Empty collection returns empty hits list with 200

- **WHEN** the caller queries a workspace whose Qdrant collection does not exist or contains no points
- **THEN** the response status MUST be `200` with body `{"hits": []}` (no `404`); callers handle the "no results" case identically whether the collection is unbuilt or simply unmatched

#### Scenario: Missing OpenAI API key returns 503 KB_NOT_CONFIGURED

- **WHEN** the sidecar was started without `CODEBUS_OPENAI_API_KEY` and the caller posts to `/kb/query`
- **THEN** the response MUST be `503` with body `{"detail": {"code": "KB_NOT_CONFIGURED", ...}}`, mirroring the `POST /kb/build` graceful-degrade contract — query needs the embedding provider to embed `text` into a vector

#### Scenario: Invalid request body returns 422

- **WHEN** the caller posts a body missing `text`, or with `top_k <= 0`, or with `top_k > 50`
- **THEN** the response MUST be `422` (Pydantic validation error); no Qdrant call MUST be made and no OpenAI embed MUST be attempted

#### Scenario: filter_path narrows results in HTTP path

- **WHEN** the caller posts `{"workspace_root": "/abs/ws", "text": "x", "filter_path": "src/foo.py"}`
- **THEN** every hit returned MUST have `payload.file_path == "src/foo.py"`, matching the existing `KnowledgeBase query and find_similar API` Requirement scenario "filter_path restricts results"

#### Scenario: Bearer token required

- **WHEN** the caller posts to `/kb/query` without an `Authorization: Bearer <token>` header
- **THEN** the response MUST be `401` and no embed call or Qdrant query MUST be attempted

#### Scenario: Query usage recorded with module=kb_query

- **WHEN** a successful `/kb/query` call completes against `workspace_root=/abs/ws`
- **THEN** at least one line MUST be appended to `/abs/ws/token_usage.jsonl` with `operation="embed"` and `module="kb_query"` (per the `usage-tracking` capability `module field` semantics — the query path's TrackedProvider factory MUST tag with `default_module="kb_query"` distinct from build's `"kb_build"`)
