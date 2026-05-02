## MODIFIED Requirements

### Requirement: Bearer token authentication

All sidecar HTTP endpoints SHALL require a valid bearer token matching the startup-generated token, per design decision D-local-2 and `docs/sidecar-api.md §一`. The bearer token MUST be presented via the `Authorization: Bearer <token>` header. As a single, narrowly-scoped exception, the SSE events endpoint (path matching the regex `^/tasks/[^/]+/events$`) MAY also accept the bearer token via the `?bearer=<token>` query parameter, because browser-native `EventSource` cannot set custom request headers (this aligns the sidecar with the `frontend-shell` Requirement "useSseTask consumes bearer through useSidecar"). The query-parameter transport SHALL NOT be accepted on any other endpoint, ensuring the bearer never lands in access logs, browser history, or `Referer` headers for non-SSE traffic.

The token comparison SHALL use `secrets.compare_digest` regardless of which transport (header or query parameter) carried it, so timing-attack mitigation applies symmetrically.

#### Scenario: Missing bearer rejected

- **WHEN** a request arrives without an `Authorization` header AND (if the request targets a non-SSE path) without a `?bearer=` query parameter
- **THEN** the sidecar MUST respond with HTTP 401

#### Scenario: Wrong bearer rejected

- **WHEN** a request arrives with an `Authorization: Bearer` value that does not equal the startup token
- **THEN** the sidecar MUST respond with HTTP 401

#### Scenario: Correct bearer accepted

- **WHEN** a request arrives with the matching bearer token in the `Authorization` header
- **THEN** the sidecar MUST process the request and respond according to the endpoint's contract

#### Scenario: SSE events endpoint accepts bearer via query parameter

- **WHEN** a `GET /tasks/<task_id>/events` request arrives without an `Authorization` header but with `?bearer=<token>` matching the startup token
- **THEN** the sidecar MUST accept the request and proceed to stream the SSE event channel
- **AND** when both `Authorization` header and `?bearer=` query parameter are present and both match the startup token, the request MUST be accepted (header takes precedence; either valid transport satisfies the requirement)

#### Scenario: Non-SSE endpoints reject query-parameter bearer

- **WHEN** any HTTP request to a path NOT matching the regex `^/tasks/[^/]+/events$` arrives without an `Authorization` header but with `?bearer=<token>` matching the startup token (for example, `POST /scan?bearer=...`)
- **THEN** the sidecar MUST respond with HTTP 401
- **AND** the response MUST be indistinguishable from a request that omitted the bearer entirely (same status code, no leakage of which transport was attempted)

#### Scenario: Wrong bearer in query parameter rejected

- **WHEN** a `GET /tasks/<task_id>/events` request arrives with `?bearer=<token>` whose value does not equal the startup token
- **THEN** the sidecar MUST respond with HTTP 401
