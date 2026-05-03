## ADDED Requirements

### Requirement: Sidecar-managed Qdrant child process

The sidecar SHALL automatically ensure a Qdrant instance is reachable on `127.0.0.1:6333` after handshake emission and before serving HTTP traffic, by spawning a Qdrant child process when no existing instance is detected. The Qdrant binary path SHALL be resolved in the following order: (1) the absolute path in environment variable `CODEBUS_QDRANT_BIN` if set; (2) `~/.codebus/bin/qdrant.exe` on Windows or `~/.codebus/bin/qdrant` on POSIX. When neither path resolves to an executable, the sidecar SHALL log a warning naming the resolution path and the fallback dev tool (`sidecar/scripts/start-qdrant.{ps1,sh}`), then continue startup in degraded mode without raising.

The spawn step SHALL set environment variables `QDRANT__STORAGE__STORAGE_PATH` and `QDRANT__STORAGE__SNAPSHOTS_PATH` that are bit-equivalent to the values resolved by `sidecar/scripts/start-qdrant.ps1` for the same user (defaults: `~/.codebus/kb` and `~/.codebus/kb/snapshots`; overridable via `CODEBUS_QDRANT_STORAGE`). After spawn, the sidecar SHALL poll `GET http://127.0.0.1:6333/healthz` every 200 ms for at most 10 seconds; the first 2xx response confirms readiness. On poll timeout the sidecar SHALL terminate the spawned child and continue startup in degraded mode.

#### Scenario: Spawn skipped when Qdrant already reachable

- **WHEN** the sidecar boots and `GET http://127.0.0.1:6333/healthz` returns a 2xx response within 500 ms
- **THEN** the sidecar MUST log an info-level message indicating reuse and MUST NOT spawn a Qdrant child process
- **AND** the existing Qdrant instance MUST continue to be used for the sidecar's lifetime

#### Scenario: Spawn happens when port 6333 is unreachable

- **WHEN** the sidecar boots, `GET http://127.0.0.1:6333/healthz` does not respond within 500 ms, and a Qdrant binary is found at the resolved path
- **THEN** the sidecar MUST spawn the binary as a child process with the canonical storage env vars set
- **AND** the sidecar MUST poll `/healthz` until a 2xx response is received (within 10 s) before completing its own startup

#### Scenario: Binary not found degrades to fallback

- **WHEN** neither `CODEBUS_QDRANT_BIN` nor `~/.codebus/bin/qdrant{.exe}` resolves to an executable
- **THEN** the sidecar MUST log a warning naming the path checked and the fallback dev tool path
- **AND** the sidecar MUST continue startup without raising
- **AND** `GET /healthz` MUST report `dependency.qdrant: "unreachable"`

#### Scenario: Spawn timeout terminates orphaned child

- **WHEN** the sidecar spawns Qdrant but `/healthz` does not return 2xx within the 10 s budget
- **THEN** the sidecar MUST send `terminate()` to the spawned child before continuing
- **AND** the sidecar MUST continue startup in degraded mode (no raise)

#### Scenario: Storage env vars match dev tool resolution

- **WHEN** both `sidecar/scripts/start-qdrant.ps1` and the sidecar auto-spawn execute on the same user with the same `CODEBUS_QDRANT_STORAGE` env (or both unset)
- **THEN** the resolved `QDRANT__STORAGE__STORAGE_PATH` MUST be byte-equivalent across the two paths
- **AND** the resolved `QDRANT__STORAGE__SNAPSHOTS_PATH` MUST equal `<storage_path>/snapshots` in both cases
