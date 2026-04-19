## ADDED Requirements

### Requirement: Local Qdrant launch recipe

The repository SHALL provide a Docker Compose file that runs Qdrant locally with persistent storage, per design decision D-local-6 and `docs/module-2-kb-builder.md §三`.

#### Scenario: Compose file exists with persistent volume

- **WHEN** `sidecar/docker-compose.qdrant.yml` is inspected
- **THEN** it MUST define a `qdrant` service with a named volume or bind mount targeting `./kb/` for persistent storage

#### Scenario: Qdrant becomes reachable after compose up

- **WHEN** `docker compose -f sidecar/docker-compose.qdrant.yml up -d` is executed
- **THEN** the Qdrant HTTP API on port 6333 MUST respond to `GET /readyz` with status 200 within thirty seconds

### Requirement: qdrant-client connectivity smoke test

The sidecar SHALL include an automated smoke test that proves `qdrant-client` can create a collection, upsert a point, and search it against the local Qdrant instance.

#### Scenario: Smoke test creates a dummy collection

- **WHEN** the smoke test runs against a local Qdrant instance
- **THEN** it MUST create a collection named `m1-smoke` with vector size 8 and MUST succeed

#### Scenario: Smoke test upserts and retrieves a point

- **WHEN** the smoke test upserts a single point with a known vector and payload into `m1-smoke` and then searches with that same vector
- **THEN** the returned point MUST match the upserted point identifier and payload exactly

#### Scenario: Smoke test cleans up after itself

- **WHEN** the smoke test finishes, whether it passes or fails
- **THEN** it MUST delete the `m1-smoke` collection so repeated runs remain idempotent
