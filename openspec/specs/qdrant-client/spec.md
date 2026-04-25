# qdrant-client Specification

## Purpose

TBD - created by archiving change 'm1-power-on'. Update Purpose after archive.

## Requirements

### Requirement: Local Qdrant launch recipe

The repository SHALL provide a platform-aware launch script that starts a Qdrant standalone binary locally with persistent storage, per design decision D-local-6 (as updated by `docs/decisions.md` D-027) and `docs/module-2-kb-builder.md §三`. A Docker Compose file SHALL remain available as a fallback for environments that already have Docker.

#### Scenario: Launch scripts exist for every supported OS

- **WHEN** the repository is inspected
- **THEN** it MUST contain both `sidecar/scripts/start-qdrant.sh` (POSIX) and `sidecar/scripts/start-qdrant.ps1` (PowerShell)
- **AND** each script MUST resolve the binary path from `$CODEBUS_QDRANT_BIN` first, defaulting to `~/.codebus/bin/qdrant(.exe)`
- **AND** each script MUST configure persistent storage under `~/.codebus/kb/` unless overridden

#### Scenario: Script emits actionable error when binary is missing

- **WHEN** `start-qdrant` is executed and neither `$CODEBUS_QDRANT_BIN` nor `~/.codebus/bin/qdrant(.exe)` is present
- **THEN** it MUST exit non-zero and print a message referencing the Qdrant release download URL plus the exact path to drop the binary into

#### Scenario: Docker Compose remains available as a fallback

- **WHEN** `sidecar/docker-compose.qdrant.yml` is inspected
- **THEN** it MUST still define a `qdrant` service with a named volume or bind mount targeting `./kb/` for persistent storage so CI and Docker-preferring users have an unbroken path

#### Scenario: Qdrant becomes reachable after the launch script runs

- **WHEN** either `start-qdrant` script or `docker compose -f sidecar/docker-compose.qdrant.yml up -d` is executed
- **THEN** the Qdrant HTTP API on port 6333 MUST respond to `GET /readyz` with status 200 within thirty seconds


<!-- @trace
source: m1-power-on
updated: 2026-04-19
-->

---
### Requirement: qdrant-client connectivity smoke test

The sidecar SHALL include an automated smoke test that proves `qdrant-client` can create a collection, upsert a point, and search it against the local Qdrant instance. The smoke test MUST connect via `CODEBUS_QDRANT_URL` (default `http://127.0.0.1:6333`) so that binary and Docker launch paths exercise the same code.

#### Scenario: Smoke test respects CODEBUS_QDRANT_URL

- **WHEN** the smoke test runs with `CODEBUS_QDRANT_URL` set to a non-default endpoint
- **THEN** it MUST connect to that endpoint rather than the hard-coded default

#### Scenario: Smoke test creates a dummy collection

- **WHEN** the smoke test runs against a local Qdrant instance
- **THEN** it MUST create a collection named `m1-smoke` with vector size 8 and MUST succeed

#### Scenario: Smoke test upserts and retrieves a point

- **WHEN** the smoke test upserts a single point with a known vector and payload into `m1-smoke` and then searches with that same vector
- **THEN** the returned point MUST match the upserted point identifier and payload exactly

#### Scenario: Smoke test cleans up after itself

- **WHEN** the smoke test finishes, whether it passes or fails
- **THEN** it MUST delete the `m1-smoke` collection so repeated runs remain idempotent

<!-- @trace
source: m1-power-on
updated: 2026-04-19
-->

---
### Requirement: Qdrant client wrapper module

The sidecar SHALL provide a first-party module `codebus_agent.kb.qdrant_client` that encapsulates construction, configuration, connection probing, and disposal of the `qdrant-client` SDK. Runtime code MUST NOT import `qdrant_client` directly outside this module; existing smoke tests under `sidecar/tests/qdrant/` are permitted to use the SDK directly for black-box verification.

#### Scenario: Wrapper module exists and exposes a public API

- **WHEN** `codebus_agent.kb.qdrant_client` is imported
- **THEN** it MUST export at least `resolve_url`, `build_client`, `probe`, and `ensure_collection` as public callables

#### Scenario: Runtime code does not import qdrant-client SDK directly

- **WHEN** the `sidecar/src/codebus_agent/` tree (excluding `sidecar/tests/`) is inspected for `import qdrant_client` or `from qdrant_client`
- **THEN** only files under `codebus_agent/kb/` MUST contain such imports


<!-- @trace
source: qdrant-lifecycle-bootstrap
updated: 2026-04-21
code:
  - sidecar/src/codebus_agent/kb/qdrant_client.py
  - docs/decisions.md
  - sidecar/src/codebus_agent/api/__init__.py
  - sidecar/src/codebus_agent/api/main.py
  - sidecar/src/codebus_agent/healthz.py
  - docs/module-2-kb-builder.md
  - sidecar/src/codebus_agent/kb/__init__.py
tests:
  - sidecar/tests/test_e2e_handshake.py
  - sidecar/tests/test_healthz_cli.py
  - sidecar/tests/kb/test_qdrant_client.py
  - sidecar/tests/kb/__init__.py
  - sidecar/tests/test_create_app.py
  - sidecar/tests/kb/test_no_direct_sdk_import.py
  - sidecar/tests/test_healthz.py
  - sidecar/tests/test_main_run.py
-->

---
### Requirement: CODEBUS_QDRANT_URL resolution has a single source of truth

The wrapper module SHALL expose `resolve_url(override: str | None = None) -> str` that returns the Qdrant base URL using the precedence: explicit argument, then `CODEBUS_QDRANT_URL` environment variable, then the default `http://127.0.0.1:6333`. All other sidecar modules and the `--healthz` CLI SHALL delegate URL resolution to this helper rather than reading the environment variable themselves.

#### Scenario: Explicit argument wins over environment

- **WHEN** `resolve_url("http://override.invalid:7000")` is called while `CODEBUS_QDRANT_URL` is set to a different value
- **THEN** it MUST return `"http://override.invalid:7000"`

#### Scenario: Environment variable used when no override

- **WHEN** `resolve_url()` is called with no argument and `CODEBUS_QDRANT_URL=http://env.invalid:9000`
- **THEN** it MUST return `"http://env.invalid:9000"`

#### Scenario: Default returned when nothing configured

- **WHEN** `resolve_url()` is called with no argument and `CODEBUS_QDRANT_URL` is unset
- **THEN** it MUST return `"http://127.0.0.1:6333"`

#### Scenario: healthz CLI uses the shared resolver

- **WHEN** the `codebus_agent.healthz` module resolves the Qdrant URL for its self-check probe
- **THEN** it MUST call `codebus_agent.kb.qdrant_client.resolve_url()` and MUST NOT re-read `CODEBUS_QDRANT_URL` directly


<!-- @trace
source: qdrant-lifecycle-bootstrap
updated: 2026-04-21
code:
  - sidecar/src/codebus_agent/kb/qdrant_client.py
  - docs/decisions.md
  - sidecar/src/codebus_agent/api/__init__.py
  - sidecar/src/codebus_agent/api/main.py
  - sidecar/src/codebus_agent/healthz.py
  - docs/module-2-kb-builder.md
  - sidecar/src/codebus_agent/kb/__init__.py
tests:
  - sidecar/tests/test_e2e_handshake.py
  - sidecar/tests/test_healthz_cli.py
  - sidecar/tests/kb/test_qdrant_client.py
  - sidecar/tests/kb/__init__.py
  - sidecar/tests/test_create_app.py
  - sidecar/tests/kb/test_no_direct_sdk_import.py
  - sidecar/tests/test_healthz.py
  - sidecar/tests/test_main_run.py
-->

---
### Requirement: Qdrant connection probe

The wrapper SHALL expose `probe(url: str, timeout_seconds: float = 1.0) -> DependencyStatus` that issues a single `GET /readyz` against the given URL and reports connectivity without raising. The probe MUST treat network errors, timeouts, and non-200 responses as `ok=false`.

#### Scenario: Reachable Qdrant reports ok

- **WHEN** `probe` targets a URL where `GET /readyz` returns HTTP 200 within the timeout
- **THEN** it MUST return `DependencyStatus(ok=True, detail=<url>)`

#### Scenario: Unreachable Qdrant reports degraded without raising

- **WHEN** `probe` targets a URL where no listener accepts the TCP connection
- **THEN** it MUST return `DependencyStatus(ok=False, detail=<url> + exception type name)` and MUST NOT raise

#### Scenario: Non-200 response reported as not ok

- **WHEN** `probe` receives an HTTP response with status other than 200
- **THEN** the returned `DependencyStatus.ok` MUST be `False`

#### Scenario: Probe detail never leaks exception message

- **WHEN** `probe` catches an exception during connection
- **THEN** `DependencyStatus.detail` MUST include the URL and the exception **type** name and MUST NOT include the exception's `str()` representation


<!-- @trace
source: qdrant-lifecycle-bootstrap
updated: 2026-04-21
code:
  - sidecar/src/codebus_agent/kb/qdrant_client.py
  - docs/decisions.md
  - sidecar/src/codebus_agent/api/__init__.py
  - sidecar/src/codebus_agent/api/main.py
  - sidecar/src/codebus_agent/healthz.py
  - docs/module-2-kb-builder.md
  - sidecar/src/codebus_agent/kb/__init__.py
tests:
  - sidecar/tests/test_e2e_handshake.py
  - sidecar/tests/test_healthz_cli.py
  - sidecar/tests/kb/test_qdrant_client.py
  - sidecar/tests/kb/__init__.py
  - sidecar/tests/test_create_app.py
  - sidecar/tests/kb/test_no_direct_sdk_import.py
  - sidecar/tests/test_healthz.py
  - sidecar/tests/test_main_run.py
-->

---
### Requirement: Async Qdrant client lifecycle bound to FastAPI app

When the sidecar FastAPI application factory is given a Qdrant URL, it SHALL construct a single `AsyncQdrantClient` instance, store it on `app.state.qdrant_client`, and register a shutdown hook that closes the client. Construction MUST NOT perform any network I/O so that a missing Qdrant does not block application startup.

#### Scenario: Client attached when URL is provided

- **WHEN** `create_app(bearer_token=..., qdrant_url="http://127.0.0.1:6333")` returns
- **THEN** the returned app's `state.qdrant_client` MUST be an `AsyncQdrantClient` instance

#### Scenario: No client when URL is omitted

- **WHEN** `create_app(bearer_token=...)` is called without `qdrant_url`
- **THEN** the returned app MUST NOT have a `state.qdrant_client` attribute or MUST have it set to `None`

#### Scenario: Construction is non-blocking

- **WHEN** `create_app(bearer_token=..., qdrant_url="http://127.0.0.1:6333")` is called while no Qdrant process is running
- **THEN** the call MUST return within one second and MUST NOT raise a connection error

#### Scenario: Client closed on app shutdown

- **WHEN** the FastAPI lifespan reaches shutdown for an app that has `state.qdrant_client` set
- **THEN** `AsyncQdrantClient.close()` MUST be invoked exactly once


<!-- @trace
source: qdrant-lifecycle-bootstrap
updated: 2026-04-21
code:
  - sidecar/src/codebus_agent/kb/qdrant_client.py
  - docs/decisions.md
  - sidecar/src/codebus_agent/api/__init__.py
  - sidecar/src/codebus_agent/api/main.py
  - sidecar/src/codebus_agent/healthz.py
  - docs/module-2-kb-builder.md
  - sidecar/src/codebus_agent/kb/__init__.py
tests:
  - sidecar/tests/test_e2e_handshake.py
  - sidecar/tests/test_healthz_cli.py
  - sidecar/tests/kb/test_qdrant_client.py
  - sidecar/tests/kb/__init__.py
  - sidecar/tests/test_create_app.py
  - sidecar/tests/kb/test_no_direct_sdk_import.py
  - sidecar/tests/test_healthz.py
  - sidecar/tests/test_main_run.py
-->

---
### Requirement: Runtime health endpoint reflects Qdrant connectivity

When `create_app` is given a Qdrant URL, the runtime `GET /healthz` response body MUST include a `dependencies.qdrant` entry whose `ok` field reflects the live probe result at request time. The probe for `/healthz` MUST reuse `codebus_agent.kb.qdrant_client.probe` so that runtime and `--healthz` CLI behaviour stay consistent.

#### Scenario: Qdrant reachable, healthz reports ok

- **WHEN** `GET /healthz` is called with a valid bearer against an app built with a reachable Qdrant URL
- **THEN** the response status MUST be 200 and the body MUST contain `{"status": "ok", "dependencies": {"qdrant": {"ok": true, ...}}}`

#### Scenario: Qdrant unreachable, healthz reports degraded

- **WHEN** `GET /healthz` is called with a valid bearer against an app built with an unreachable Qdrant URL
- **THEN** the response status MUST be 200 and the body MUST contain `{"status": "degraded", "dependencies": {"qdrant": {"ok": false, ...}}}`

#### Scenario: No Qdrant URL configured, healthz omits dependency

- **WHEN** `GET /healthz` is called with a valid bearer against an app built without a Qdrant URL
- **THEN** the response body's `dependencies` object MUST NOT contain a `qdrant` key


<!-- @trace
source: qdrant-lifecycle-bootstrap
updated: 2026-04-21
code:
  - sidecar/src/codebus_agent/kb/qdrant_client.py
  - docs/decisions.md
  - sidecar/src/codebus_agent/api/__init__.py
  - sidecar/src/codebus_agent/api/main.py
  - sidecar/src/codebus_agent/healthz.py
  - docs/module-2-kb-builder.md
  - sidecar/src/codebus_agent/kb/__init__.py
tests:
  - sidecar/tests/test_e2e_handshake.py
  - sidecar/tests/test_healthz_cli.py
  - sidecar/tests/kb/test_qdrant_client.py
  - sidecar/tests/kb/__init__.py
  - sidecar/tests/test_create_app.py
  - sidecar/tests/kb/test_no_direct_sdk_import.py
  - sidecar/tests/test_healthz.py
  - sidecar/tests/test_main_run.py
-->

---
### Requirement: Idempotent collection provisioning

The wrapper SHALL expose `ensure_collection(client, name, vector_size, distance="Cosine")` that guarantees a Qdrant collection with the given vector configuration exists. The helper MUST NOT destroy existing data: when a collection with the same name already exists with a different vector size or distance, it MUST raise `QdrantCollectionSchemaError` rather than drop-and-recreate.

#### Scenario: Creates collection when absent

- **WHEN** `ensure_collection(client, "codebus_demo", vector_size=8)` is called and no collection named `codebus_demo` exists
- **THEN** a new collection `codebus_demo` MUST be created with vector size 8 and distance Cosine

#### Scenario: No-op when collection matches

- **WHEN** `ensure_collection(client, "codebus_demo", vector_size=8)` is called twice in succession
- **THEN** the second call MUST NOT raise and MUST NOT alter the collection

#### Scenario: Schema mismatch raises QdrantCollectionSchemaError

- **WHEN** `ensure_collection(client, "codebus_demo", vector_size=16)` is called against an existing `codebus_demo` collection whose vector size is 8
- **THEN** the call MUST raise `QdrantCollectionSchemaError` and MUST NOT modify the existing collection

<!-- @trace
source: qdrant-lifecycle-bootstrap
updated: 2026-04-21
code:
  - sidecar/src/codebus_agent/kb/qdrant_client.py
  - docs/decisions.md
  - sidecar/src/codebus_agent/api/__init__.py
  - sidecar/src/codebus_agent/api/main.py
  - sidecar/src/codebus_agent/healthz.py
  - docs/module-2-kb-builder.md
  - sidecar/src/codebus_agent/kb/__init__.py
tests:
  - sidecar/tests/test_e2e_handshake.py
  - sidecar/tests/test_healthz_cli.py
  - sidecar/tests/kb/test_qdrant_client.py
  - sidecar/tests/kb/__init__.py
  - sidecar/tests/test_create_app.py
  - sidecar/tests/kb/test_no_direct_sdk_import.py
  - sidecar/tests/test_healthz.py
  - sidecar/tests/test_main_run.py
-->

---
### Requirement: KB-facing vector upsert helper

The `codebus_agent.kb.qdrant_client` wrapper module SHALL expose an async helper `upsert_points(client, collection, points)` that writes a batch of points (each carrying `id`, `vector`, and a `KBPayload`-dumped mapping) to the named collection. The helper MUST serialize payloads via `KBPayload.model_dump(mode="json")` so datetime fields round-trip as ISO-8601 strings. Runtime code outside `codebus_agent/kb/` MUST NOT import `qdrant_client` directly; the existing runtime-import restriction continues to apply to the new helper.

#### Scenario: Upsert writes all points to the named collection

- **WHEN** `upsert_points(client, "codebus_demo", [p1, p2, p3])` is called against an existing collection
- **THEN** after the call, searching the collection with each point's vector MUST return that point's id

#### Scenario: Payload datetimes serialized as ISO-8601

- **WHEN** `upsert_points` is called with a point whose payload contains a `created_at` datetime
- **THEN** reading the point back via the SDK MUST yield a string matching ISO-8601 format, and `datetime.fromisoformat(...)` on that string MUST equal the original value


<!-- @trace
source: module-2-kb-builder-p0
updated: 2026-04-21
code:
  - sidecar/uv.lock
  - sidecar/src/codebus_agent/kb/backend.py
  - sidecar/src/codebus_agent/kb/qdrant_client.py
  - sidecar/src/codebus_agent/providers/usage_tracker.py
  - CLAUDE.md
  - sidecar/src/codebus_agent/kb/payload.py
  - sidecar/pyproject.toml
  - sidecar/src/codebus_agent/kb/__init__.py
  - docs/implementation-plan.md
  - sidecar/src/codebus_agent/kb/knowledge_base.py
  - docs/module-2-kb-builder.md
  - sidecar/src/codebus_agent/kb/chunker.py
tests:
  - sidecar/tests/kb/fixtures/sample-doc.md
  - sidecar/tests/kb/conftest.py
  - sidecar/tests/kb/test_strategy.py
  - sidecar/tests/kb/fixtures/sample-code.py
  - sidecar/tests/kb/test_knowledge_base.py
  - sidecar/tests/kb/fixtures/sample-plain.txt
  - sidecar/tests/kb/test_qdrant_kb.py
  - sidecar/tests/kb/test_chunker.py
  - sidecar/tests/kb/test_payload.py
-->

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


<!-- @trace
source: module-2-kb-builder-p0
updated: 2026-04-21
code:
  - sidecar/uv.lock
  - sidecar/src/codebus_agent/kb/backend.py
  - sidecar/src/codebus_agent/kb/qdrant_client.py
  - sidecar/src/codebus_agent/providers/usage_tracker.py
  - CLAUDE.md
  - sidecar/src/codebus_agent/kb/payload.py
  - sidecar/pyproject.toml
  - sidecar/src/codebus_agent/kb/__init__.py
  - docs/implementation-plan.md
  - sidecar/src/codebus_agent/kb/knowledge_base.py
  - docs/module-2-kb-builder.md
  - sidecar/src/codebus_agent/kb/chunker.py
tests:
  - sidecar/tests/kb/fixtures/sample-doc.md
  - sidecar/tests/kb/conftest.py
  - sidecar/tests/kb/test_strategy.py
  - sidecar/tests/kb/fixtures/sample-code.py
  - sidecar/tests/kb/test_knowledge_base.py
  - sidecar/tests/kb/fixtures/sample-plain.txt
  - sidecar/tests/kb/test_qdrant_kb.py
  - sidecar/tests/kb/test_chunker.py
  - sidecar/tests/kb/test_payload.py
-->

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


<!-- @trace
source: module-2-kb-builder-p0
updated: 2026-04-21
code:
  - sidecar/uv.lock
  - sidecar/src/codebus_agent/kb/backend.py
  - sidecar/src/codebus_agent/kb/qdrant_client.py
  - sidecar/src/codebus_agent/providers/usage_tracker.py
  - CLAUDE.md
  - sidecar/src/codebus_agent/kb/payload.py
  - sidecar/pyproject.toml
  - sidecar/src/codebus_agent/kb/__init__.py
  - docs/implementation-plan.md
  - sidecar/src/codebus_agent/kb/knowledge_base.py
  - docs/module-2-kb-builder.md
  - sidecar/src/codebus_agent/kb/chunker.py
tests:
  - sidecar/tests/kb/fixtures/sample-doc.md
  - sidecar/tests/kb/conftest.py
  - sidecar/tests/kb/test_strategy.py
  - sidecar/tests/kb/fixtures/sample-code.py
  - sidecar/tests/kb/test_knowledge_base.py
  - sidecar/tests/kb/fixtures/sample-plain.txt
  - sidecar/tests/kb/test_qdrant_kb.py
  - sidecar/tests/kb/test_chunker.py
  - sidecar/tests/kb/test_payload.py
-->

---
### Requirement: Idempotent KB payload index provisioning

The wrapper SHALL expose `ensure_kb_payload_indices(client, collection)` that creates keyword payload indices for the fields `text_hash` and `related_stations` on the given collection. The helper MUST be idempotent: repeated invocations MUST succeed without raising and MUST NOT alter index configuration once created.

#### Scenario: Indices created when absent

- **WHEN** `ensure_kb_payload_indices(client, "codebus_demo")` is called against a collection that has no payload indices
- **THEN** after the call, filtering searches on `text_hash` and `related_stations` MUST be supported by the collection's index configuration

#### Scenario: Repeated invocation no-op

- **WHEN** `ensure_kb_payload_indices` is called twice in succession on the same collection
- **THEN** the second call MUST NOT raise and MUST NOT modify existing indices

<!-- @trace
source: module-2-kb-builder-p0
updated: 2026-04-21
code:
  - sidecar/uv.lock
  - sidecar/src/codebus_agent/kb/backend.py
  - sidecar/src/codebus_agent/kb/qdrant_client.py
  - sidecar/src/codebus_agent/providers/usage_tracker.py
  - CLAUDE.md
  - sidecar/src/codebus_agent/kb/payload.py
  - sidecar/pyproject.toml
  - sidecar/src/codebus_agent/kb/__init__.py
  - docs/implementation-plan.md
  - sidecar/src/codebus_agent/kb/knowledge_base.py
  - docs/module-2-kb-builder.md
  - sidecar/src/codebus_agent/kb/chunker.py
tests:
  - sidecar/tests/kb/fixtures/sample-doc.md
  - sidecar/tests/kb/conftest.py
  - sidecar/tests/kb/test_strategy.py
  - sidecar/tests/kb/fixtures/sample-code.py
  - sidecar/tests/kb/test_knowledge_base.py
  - sidecar/tests/kb/fixtures/sample-plain.txt
  - sidecar/tests/kb/test_qdrant_kb.py
  - sidecar/tests/kb/test_chunker.py
  - sidecar/tests/kb/test_payload.py
-->
