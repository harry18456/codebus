## ADDED Requirements

### Requirement: Qdrant child process supervision lifecycle

When the sidecar spawns a Qdrant child process per the `qdrant-client` capability's `Sidecar-managed Qdrant child process` requirement, the sidecar SHALL supervise the child for the duration of its own process lifetime and SHALL terminate the child on every exit path. Three independent termination paths SHALL be installed:

1. `atexit.register(...)` — fires on normal `sys.exit` and on uncaught exceptions that propagate past `asyncio.run`
2. The `--parent-pid` watchdog — when the Tauri parent disappears, the watchdog SHALL invoke a cleanup hook BEFORE calling `os._exit(0)`
3. Signal handlers — `SIGTERM` on POSIX and `CTRL_BREAK_EVENT` on Windows SHALL trigger the same cleanup

The cleanup routine SHALL first call `Popen.terminate()` on the child (graceful shutdown so Qdrant can flush its WAL), then `wait(timeout=5)`; on timeout the routine SHALL call `Popen.kill()`. The cleanup routine MUST be idempotent — a second invocation against an already-exited child MUST be a no-op (verified by `Popen.poll()` returning a non-`None` exit code).

The spawn step SHALL occur after the sidecar's handshake JSON line has been emitted to stdout and BEFORE `asyncio.run(_serve(...))` opens the FastAPI listener. Failure to spawn (binary missing, poll timeout) SHALL NOT block startup; the sidecar SHALL proceed with `qdrant_client = None` semantics so `/healthz` reports `dependency.qdrant: "unreachable"` rather than failing to bind a port.

#### Scenario: Tauri parent exit triggers child termination via watchdog

- **WHEN** the sidecar is running with `--parent-pid <pid>` and the parent process exits
- **THEN** the watchdog MUST detect the parent's disappearance within 5 seconds (existing watchdog budget)
- **AND** the watchdog MUST invoke the Qdrant cleanup hook BEFORE calling `os._exit(0)`
- **AND** the spawned Qdrant child MUST be terminated (verified by `Popen.wait()` returning a non-`None` exit code)

#### Scenario: Normal sidecar shutdown via atexit

- **WHEN** the sidecar exits via `sys.exit(0)` after a uvicorn graceful shutdown
- **THEN** the `atexit`-registered cleanup hook MUST fire
- **AND** the spawned Qdrant child MUST receive `SIGTERM` (via `Popen.terminate()`)
- **AND** if the child does not exit within 5 seconds, `Popen.kill()` MUST be called

#### Scenario: Cleanup is idempotent under multiple exit paths

- **WHEN** both the `atexit` hook and the watchdog hook fire (e.g., uvicorn raises during shutdown after parent already exited)
- **THEN** the second cleanup invocation MUST be a no-op (no exception, no second `terminate()`)
- **AND** the cleanup hook MUST detect the already-exited state via `Popen.poll()` returning non-`None`

#### Scenario: Spawn never blocks sidecar startup

- **WHEN** the Qdrant binary is missing or the `/healthz` poll times out at 10 s
- **THEN** the sidecar MUST continue startup
- **AND** the FastAPI listener MUST bind successfully (handshake-advertised port reachable)
- **AND** `GET /healthz` MUST return 200 with `dependency.qdrant: "unreachable"` and `status: "degraded"`

#### Scenario: Concurrent sidecar instances reuse a single Qdrant

- **WHEN** sidecar A has spawned Qdrant on `127.0.0.1:6333` and sidecar B starts on the same host
- **THEN** sidecar B's reuse probe MUST observe 2xx on `GET /healthz` and skip spawning
- **AND** sidecar B MUST NOT register cleanup hooks for sidecar A's child (it does not own the handle)
- **AND** when sidecar B exits, the Qdrant child MUST continue running under sidecar A's supervision
