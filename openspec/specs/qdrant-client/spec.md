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
code:
  - web/dist
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
code:
  - web/dist
-->
