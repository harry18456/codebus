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
