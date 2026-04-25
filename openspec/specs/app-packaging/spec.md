# app-packaging Specification

## Purpose

TBD - created by archiving change 'm1-power-on'. Update Purpose after archive.

## Requirements

### Requirement: PyInstaller onefile sidecar binary

The sidecar project SHALL ship a PyInstaller spec file that produces a single-file executable, per design decision D-local-5 and `docs/dev-setup.md`.

#### Scenario: PyInstaller spec exists and builds

- **WHEN** `pyinstaller sidecar/codebus-sidecar.spec` is run on the host platform
- **THEN** PyInstaller MUST produce one executable file at `sidecar/dist/codebus-sidecar` (or `codebus-sidecar.exe` on Windows)

#### Scenario: Hidden imports declared

- **WHEN** the spec file is inspected
- **THEN** it MUST list `uvicorn.protocols.http.auto`, `instructor`, and `qdrant_client` in its hidden-import set


<!-- @trace
source: m1-power-on
updated: 2026-04-19
-->

---
### Requirement: Packaged binary health check

The packaged sidecar binary SHALL expose a `--healthz` command-line flag that performs a self-check without starting the HTTP server, so CI and packaging smoke tests can validate the build.

#### Scenario: Healthz flag succeeds after build

- **WHEN** the packaged binary is invoked with `--healthz`
- **THEN** it MUST exit with status code 0 and MUST print a single line containing `"status": "ok"`

#### Scenario: Healthz reports degraded if optional dependency missing

- **WHEN** the packaged binary is invoked with `--healthz` while no Qdrant service is reachable
- **THEN** it MUST still exit with status code 0 and MUST print a line containing `"status": "degraded"` together with the unreachable dependency


<!-- @trace
source: m1-power-on
updated: 2026-04-19
-->

---
### Requirement: Tauri external binary integration

The Tauri application SHALL embed the packaged sidecar binary through its `externalBin` configuration, so `cargo tauri build` produces an artifact that starts the sidecar automatically.

#### Scenario: externalBin points at packaged sidecar

- **WHEN** `tauri/src-tauri/tauri.conf.json` is inspected
- **THEN** its `tauri.bundle.externalBin` array MUST include the packaged sidecar binary path

#### Scenario: Bundled app launches sidecar

- **WHEN** the Tauri production bundle is launched on the host platform
- **THEN** the sidecar process MUST start as a child of the Tauri process and MUST complete the stdout handshake within ten seconds

<!-- @trace
source: m1-power-on
updated: 2026-04-19
-->
