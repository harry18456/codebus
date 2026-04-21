## ADDED Requirements

### Requirement: Workspace scan endpoint registration

The FastAPI sidecar SHALL register a `POST /scan` route that implements the `folder-scanner` capability. The route MUST be mounted under the same bearer-protected middleware as `/healthz`; it MUST NOT introduce a new authentication path, a new bind address, or bypass the ephemeral loopback constraint established by the bind-port requirement. The route's presence MUST NOT change the synchronous shape of `/healthz` or the stdout handshake.

#### Scenario: Scan route requires bearer token

- **WHEN** a client sends `POST /scan` without the `Authorization: Bearer <token>` header
- **THEN** the sidecar returns HTTP 401 and the scanner code path is not invoked.

#### Scenario: Scan route shares the loopback bind

- **WHEN** the sidecar starts and the stdout handshake prints `{"port": N, "bearer": "..."}`
- **THEN** both `/healthz` and `/scan` are reachable on `127.0.0.1:N` with the same bearer token, and neither endpoint is reachable on any non-loopback interface.

#### Scenario: Existing endpoints unchanged

- **WHEN** a client calls `GET /healthz` with a valid bearer token after `/scan` is registered
- **THEN** the response shape matches the existing Health endpoint contract (dependency statuses and overall status unchanged) and the response does not reference the scan endpoint.
