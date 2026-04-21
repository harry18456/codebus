# sidecar-runtime Specification

## Purpose

TBD - created by archiving change 'm1-power-on'. Update Purpose after archive.

## Requirements

### Requirement: FastAPI sidecar binds ephemeral loopback port

The sidecar process SHALL start a FastAPI application bound exclusively to `127.0.0.1` on a port assigned by the operating system (ephemeral), per design decision D-local-1.

#### Scenario: Random port chosen at startup

- **WHEN** the sidecar process is launched twice in succession
- **THEN** each run MUST bind to a different ephemeral port number

#### Scenario: Not reachable from non-loopback interfaces

- **WHEN** a client on a non-loopback interface (any address other than `127.0.0.1` or `::1`) attempts to open a TCP connection to the sidecar port
- **THEN** the connection MUST fail to establish


<!-- @trace
source: m1-power-on
updated: 2026-04-19
code:
  - web/dist
-->

---
### Requirement: Bearer token authentication

All sidecar HTTP endpoints SHALL require a `Authorization: Bearer <token>` header matching the startup-generated token, per design decision D-local-2 and `docs/sidecar-api.md §一`.

#### Scenario: Missing bearer rejected

- **WHEN** a request arrives without an `Authorization` header
- **THEN** the sidecar MUST respond with HTTP 401

#### Scenario: Wrong bearer rejected

- **WHEN** a request arrives with an `Authorization: Bearer` value that does not equal the startup token
- **THEN** the sidecar MUST respond with HTTP 401

#### Scenario: Correct bearer accepted

- **WHEN** a request arrives with the matching bearer token
- **THEN** the sidecar MUST process the request and respond according to the endpoint's contract


<!-- @trace
source: m1-power-on
updated: 2026-04-19
code:
  - web/dist
-->

---
### Requirement: Health endpoint

The sidecar SHALL expose `GET /healthz` returning a JSON payload reflecting its readiness state.

#### Scenario: Healthy state

- **WHEN** `GET /healthz` is called with a valid bearer and all dependencies are reachable
- **THEN** the response status MUST be 200 and the body MUST contain `{"status": "ok"}`

#### Scenario: Degraded state

- **WHEN** `GET /healthz` is called with a valid bearer and an external dependency (for example Qdrant) is unreachable
- **THEN** the response status MUST be 200 and the body MUST contain `{"status": "degraded"}` together with a `dependencies` object naming each unreachable dependency


<!-- @trace
source: m1-power-on
updated: 2026-04-19
code:
  - web/dist
-->

---
### Requirement: Handshake via stdout first line

At startup the sidecar SHALL emit a single-line JSON handshake to stdout so the parent Tauri process can discover the port and bearer token, per design decision D-local-1.

#### Scenario: Handshake line format

- **WHEN** the sidecar process starts
- **THEN** the first line written to stdout MUST be valid JSON containing the keys `port` (integer) and `bearer` (string of at least 32 characters)

#### Scenario: Parent reads handshake and succeeds ping

- **WHEN** the parent Tauri process reads the handshake line and issues `GET /healthz` with the supplied bearer against the supplied port
- **THEN** the response MUST be HTTP 200


<!-- @trace
source: m1-power-on
updated: 2026-04-19
code:
  - web/dist
-->

---
### Requirement: Parent-process watchdog

The sidecar SHALL self-terminate when its parent process disappears, so that orphaned sidecars do not keep loopback ports bound, per design decision D-local-2.

#### Scenario: Parent exits unexpectedly

- **WHEN** the parent process identified by `--parent-pid` exits
- **THEN** the sidecar MUST exit within five seconds and MUST release the bound port

<!-- @trace
source: m1-power-on
updated: 2026-04-19
code:
  - web/dist
-->

---
### Requirement: Sidecar startup remains available when Qdrant is unreachable

The sidecar entry point SHALL complete its startup sequence (bind ephemeral loopback port, emit stdout handshake, serve `GET /healthz`) even when the Qdrant URL it has been configured with is unreachable. A missing or unresponsive Qdrant MUST NOT cause the process to exit non-zero, block handshake emission, or prevent the bearer-authenticated HTTP server from accepting requests. This aligns with design decision D-009 (local-first) and D-027 (user-managed Qdrant binary).

#### Scenario: Sidecar starts with no Qdrant listener

- **WHEN** the sidecar is launched while no process is listening on the configured Qdrant URL
- **THEN** the handshake JSON line MUST still be emitted to stdout within the existing startup budget
- **AND** `GET /healthz` with a valid bearer MUST respond with HTTP 200 and body `{"status": "degraded", "dependencies": {"qdrant": {"ok": false, ...}}}`

#### Scenario: Sidecar startup not delayed waiting for Qdrant

- **WHEN** the sidecar is launched while no Qdrant listener exists
- **THEN** the time between process spawn and handshake emission MUST NOT be measurably increased by Qdrant-related probes (probe timeout MUST be bounded by one second and MUST NOT run during handshake emission)


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
