## ADDED Requirements

### Requirement: Sidecar startup remains available when Qdrant is unreachable

The sidecar entry point SHALL complete its startup sequence (bind ephemeral loopback port, emit stdout handshake, serve `GET /healthz`) even when the Qdrant URL it has been configured with is unreachable. A missing or unresponsive Qdrant MUST NOT cause the process to exit non-zero, block handshake emission, or prevent the bearer-authenticated HTTP server from accepting requests. This aligns with design decision D-009 (local-first) and D-027 (user-managed Qdrant binary).

#### Scenario: Sidecar starts with no Qdrant listener

- **WHEN** the sidecar is launched while no process is listening on the configured Qdrant URL
- **THEN** the handshake JSON line MUST still be emitted to stdout within the existing startup budget
- **AND** `GET /healthz` with a valid bearer MUST respond with HTTP 200 and body `{"status": "degraded", "dependencies": {"qdrant": {"ok": false, ...}}}`

#### Scenario: Sidecar startup not delayed waiting for Qdrant

- **WHEN** the sidecar is launched while no Qdrant listener exists
- **THEN** the time between process spawn and handshake emission MUST NOT be measurably increased by Qdrant-related probes (probe timeout MUST be bounded by one second and MUST NOT run during handshake emission)

---

### Requirement: Sidecar entry point wires Qdrant URL into app factory

The CLI entry point `codebus_agent.api.main:run` SHALL resolve the Qdrant base URL via `codebus_agent.kb.qdrant_client.resolve_url()` and pass the result to `codebus_agent.api.create_app` as the `qdrant_url` keyword argument, so that runtime `/healthz` reflects live Qdrant connectivity. When the CLI is invoked with `--healthz`, the same resolver MUST be used to pick the URL for the self-check.

#### Scenario: Environment variable propagates to runtime healthz

- **WHEN** the sidecar is launched with `CODEBUS_QDRANT_URL=http://custom.invalid:7000`
- **THEN** `GET /healthz` responses MUST include a `dependencies.qdrant` entry whose `detail` field reports `http://custom.invalid:7000`

#### Scenario: Default URL used when environment unset

- **WHEN** the sidecar is launched with `CODEBUS_QDRANT_URL` unset
- **THEN** `GET /healthz` responses MUST include a `dependencies.qdrant` entry whose `detail` field reports `http://127.0.0.1:6333`

#### Scenario: --healthz CLI shares the same resolver

- **WHEN** the sidecar is invoked with `--healthz` and `CODEBUS_QDRANT_URL=http://custom.invalid:7000`
- **THEN** the printed JSON line's `dependencies.qdrant.detail` MUST reference `http://custom.invalid:7000`
