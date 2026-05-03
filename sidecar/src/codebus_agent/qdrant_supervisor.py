"""Qdrant child process supervisor — D-027 sidecar-managed auto-spawn.

Backs SHALL clauses in
``openspec/changes/qdrant-auto-spawn/specs/qdrant-client/spec.md``
  Requirement: Sidecar-managed Qdrant child process
And ``specs/sidecar-runtime/spec.md``
  Requirement: Qdrant child process supervision lifecycle

Trust boundary contract:
  * `maybe_spawn_qdrant()` NEVER raises — caller treats `None` as
    "Qdrant unavailable, run in degraded mode" per D-027 invariant.
  * The Qdrant child is the sidecar's responsibility for the
    duration of its process; `cleanup_qdrant_child()` MUST be
    invoked from atexit / parent_pid watchdog / signal handlers
    (three independent termination paths).
  * Binary resolution is a defense-in-depth check: file must exist
    before subprocess.Popen is invoked, so a missing binary turns
    into a degraded-mode log line, not an OS-level spawn error.
  * Storage env vars MUST stay byte-equivalent with
    `sidecar/scripts/start-qdrant.{ps1,sh}` so the dev tool and
    sidecar auto-spawn share the same on-disk state.
"""
from __future__ import annotations

import logging
import os
import subprocess
import time
import urllib.request
from pathlib import Path
from typing import Callable

logger = logging.getLogger(__name__)

# Reuse-first probe timeout — short so cold boot does not block on
# detection when the port is genuinely unreachable.
_PROBE_TIMEOUT_S: float = 0.5

# Post-spawn readiness poll. 10s gives Qdrant enough cold-start budget
# on slow disks while keeping sidecar handshake responsive.
_POLL_BUDGET_S: float = 10.0
_POLL_INTERVAL_S: float = 0.2

# Graceful terminate window before kill(). Qdrant flushes its WAL on
# SIGTERM; 5s is the standard graceful shutdown allowance per
# Decision 3 (sidecar-runtime spec).
_TERMINATE_TIMEOUT_S: float = 5.0

_HEALTHZ_URL: str = "http://127.0.0.1:6333/healthz"


def _home_dir() -> Path:
    """Resolve the user's home dir cross-platform.

    Precedence: HOME (POSIX + git-bash on Windows) → USERPROFILE
    (native Windows) → fall back to Path.home() (resolves via pwd).
    Tests monkeypatch HOME / USERPROFILE so a fake home can be
    injected without touching the real disk.
    """
    for var in ("HOME", "USERPROFILE"):
        candidate = os.environ.get(var)
        if candidate:
            return Path(candidate)
    return Path.home()


def _resolve_binary_path() -> Path | None:
    """Locate the Qdrant binary per spec resolution order.

    Order:
      1. ``$CODEBUS_QDRANT_BIN`` if set and points to an existing file
      2. ``~/.codebus/bin/qdrant.exe`` on Windows or
         ``~/.codebus/bin/qdrant`` on POSIX

    Returns ``None`` when neither resolves to an executable file —
    caller MUST log a warning and continue in degraded mode.
    """
    override = os.environ.get("CODEBUS_QDRANT_BIN")
    if override:
        candidate = Path(override)
        if candidate.is_file():
            return candidate

    suffix = ".exe" if os.name == "nt" else ""
    fallback = _home_dir() / ".codebus" / "bin" / f"qdrant{suffix}"
    if fallback.is_file():
        return fallback

    return None


def _resolve_storage_paths() -> tuple[Path, Path]:
    """Resolve the (storage, snapshots) directory pair.

    Mirrors ``sidecar/scripts/start-qdrant.{ps1,sh}`` exactly so the
    dev tool and sidecar auto-spawn share the same on-disk state for
    a given user (Spec scenario "Storage env vars match dev tool
    resolution").
    """
    storage_override = os.environ.get("CODEBUS_QDRANT_STORAGE")
    if storage_override:
        storage = Path(storage_override)
    else:
        storage = _home_dir() / ".codebus" / "kb"
    snapshots = storage / "snapshots"
    return storage, snapshots


def _probe_reachable(timeout: float = _PROBE_TIMEOUT_S) -> bool:
    """Return True iff GET /healthz responds 2xx within ``timeout``."""
    try:
        with urllib.request.urlopen(_HEALTHZ_URL, timeout=timeout) as resp:
            return 200 <= resp.status < 300
    except Exception:
        return False


def _poll_until_ready(deadline: float, interval: float) -> bool:
    """Spin on /healthz until 2xx or wall clock passes ``deadline``."""
    while time.monotonic() < deadline:
        if _probe_reachable(timeout=_PROBE_TIMEOUT_S):
            return True
        time.sleep(interval)
    return False


def maybe_spawn_qdrant(parent_pid: int | None = None) -> subprocess.Popen | None:  # noqa: ARG001
    """Ensure Qdrant is reachable on 127.0.0.1:6333; spawn child if not.

    Returns the spawned ``subprocess.Popen`` handle for caller
    cleanup, or ``None`` in two cases:
      * Reuse path: an existing Qdrant already serves /healthz — no
        spawn happens; caller has no handle to clean up
      * Degraded path: binary missing or post-spawn poll timeout —
        sidecar continues startup with /healthz reporting unreachable

    ``parent_pid`` is accepted for API symmetry with future watchdog
    extensions but currently unused; the supervisor does not pass it
    to Qdrant (Qdrant has no parent-pid concept of its own).

    NEVER raises. Failure modes log + return None per D-027
    "degraded-but-alive" invariant.
    """
    if _probe_reachable():
        logger.info("Qdrant already reachable on %s; reusing", _HEALTHZ_URL)
        return None

    binary = _resolve_binary_path()
    if binary is None:
        override = os.environ.get("CODEBUS_QDRANT_BIN", "<unset>")
        suffix = ".exe" if os.name == "nt" else ""
        fallback = _home_dir() / ".codebus" / "bin" / f"qdrant{suffix}"
        logger.warning(
            "Qdrant binary not found "
            "(CODEBUS_QDRANT_BIN=%s, fallback=%s); "
            "sidecar will start in degraded mode. "
            "Run sidecar/scripts/start-qdrant.{ps1,sh} manually if you want to "
            "use Qdrant features.",
            override,
            fallback,
        )
        return None

    storage, snapshots = _resolve_storage_paths()
    storage.mkdir(parents=True, exist_ok=True)
    snapshots.mkdir(parents=True, exist_ok=True)

    env = {
        **os.environ,
        "QDRANT__STORAGE__STORAGE_PATH": str(storage),
        "QDRANT__STORAGE__SNAPSHOTS_PATH": str(snapshots),
    }

    logger.info("spawning Qdrant from %s with storage=%s", binary, storage)
    try:
        proc = subprocess.Popen(  # noqa: S603 — binary path validated above
            [str(binary)],
            env=env,
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
        )
    except OSError as exc:
        logger.warning(
            "Qdrant spawn failed (%s); sidecar will start in degraded mode",
            exc,
        )
        return None

    deadline = time.monotonic() + _POLL_BUDGET_S
    if _poll_until_ready(deadline=deadline, interval=_POLL_INTERVAL_S):
        logger.info("Qdrant child PID %s ready on %s", proc.pid, _HEALTHZ_URL)
        return proc

    logger.warning(
        "Qdrant child PID %s did not become ready within %.1fs; terminating",
        proc.pid,
        _POLL_BUDGET_S,
    )
    cleanup_qdrant_child(proc)
    return None


def cleanup_qdrant_child(proc: subprocess.Popen | None) -> None:
    """Terminate the Qdrant child process gracefully.

    Idempotent: safe to call multiple times against the same handle
    or against ``None`` (Spec scenario "Cleanup is idempotent under
    multiple exit paths").

    Sequence:
      1. ``Popen.poll()`` — if non-None, child already exited; no-op
      2. ``Popen.terminate()`` — graceful SIGTERM (Qdrant flushes WAL)
      3. ``Popen.wait(timeout=5)`` — wait up to 5s for graceful exit
      4. On timeout: ``Popen.kill()`` — force SIGKILL + drain
    """
    if proc is None:
        return
    if proc.poll() is not None:
        return  # child already exited; no-op idempotent path

    try:
        proc.terminate()
    except OSError as exc:
        logger.warning("Qdrant terminate() failed: %s", exc)
        return

    try:
        proc.wait(timeout=_TERMINATE_TIMEOUT_S)
        return
    except subprocess.TimeoutExpired:
        logger.warning(
            "Qdrant did not exit within %.1fs; sending kill()",
            _TERMINATE_TIMEOUT_S,
        )

    try:
        proc.kill()
        proc.wait()
    except OSError as exc:
        logger.warning("Qdrant kill() failed: %s", exc)


def install_signal_cleanup(proc: subprocess.Popen | None) -> None:
    """Install a signal handler that runs ``cleanup_qdrant_child`` first.

    POSIX:   SIGTERM
    Windows: SIGBREAK (CTRL_BREAK_EVENT) — SIGTERM on Windows behaves
             differently and TerminateProcess does not deliver a signal
             handler at all (OS-level termination, not interceptable),
             so SIGBREAK is the closest practical equivalent for the
             keyboard / parent shell scenario.

    No-op when ``proc`` is ``None`` (Spec scenario "Spawn skipped when
    Qdrant already reachable" — there is no child to clean).

    The handler:
      1. Calls ``cleanup_qdrant_child(proc)``
      2. Restores the default disposition for the signal
      3. Re-raises the signal at the current process so the normal
         exit path proceeds (matches POSIX convention for cleanup-then-
         re-raise patterns)
    """
    if proc is None:
        return

    import signal

    sig: int
    if os.name == "nt":
        # SIGBREAK exists on Windows Python; mypy may not see it.
        sig = signal.SIGBREAK  # type: ignore[attr-defined]
    else:
        sig = signal.SIGTERM

    def _handler(signum: int, _frame: object) -> None:
        cleanup_qdrant_child(proc)
        signal.signal(signum, signal.SIG_DFL)
        os.kill(os.getpid(), signum)

    signal.signal(sig, _handler)


def register_cleanup(proc: subprocess.Popen | None) -> Callable[[], None]:
    """Register an ``atexit`` hook + return the same callable.

    The returned callable can additionally be passed to other exit
    paths (parent_pid watchdog, signal handlers) so all three
    termination sources converge on a single idempotent cleanup
    function.
    """
    def _hook() -> None:
        cleanup_qdrant_child(proc)

    if proc is not None:
        import atexit

        atexit.register(_hook)
    return _hook


__all__ = [
    "maybe_spawn_qdrant",
    "cleanup_qdrant_child",
    "register_cleanup",
    "install_signal_cleanup",
]
