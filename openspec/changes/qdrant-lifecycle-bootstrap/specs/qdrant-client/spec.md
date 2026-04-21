## ADDED Requirements

### Requirement: Qdrant client wrapper module

The sidecar SHALL provide a first-party module `codebus_agent.kb.qdrant_client` that encapsulates construction, configuration, connection probing, and disposal of the `qdrant-client` SDK. Runtime code MUST NOT import `qdrant_client` directly outside this module; existing smoke tests under `sidecar/tests/qdrant/` are permitted to use the SDK directly for black-box verification.

#### Scenario: Wrapper module exists and exposes a public API

- **WHEN** `codebus_agent.kb.qdrant_client` is imported
- **THEN** it MUST export at least `resolve_url`, `build_client`, `probe`, and `ensure_collection` as public callables

#### Scenario: Runtime code does not import qdrant-client SDK directly

- **WHEN** the `sidecar/src/codebus_agent/` tree (excluding `sidecar/tests/`) is inspected for `import qdrant_client` or `from qdrant_client`
- **THEN** only files under `codebus_agent/kb/` MUST contain such imports

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
