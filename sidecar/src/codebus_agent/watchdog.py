"""Parent-process watchdog.

Backs openspec/changes/m1-power-on/specs/sidecar-runtime/spec.md
  Requirement: Parent-process watchdog
  Design:       D-local-2 (sidecar self-terminates when parent disappears)

And openspec/changes/qdrant-auto-spawn/specs/sidecar-runtime/spec.md
  Requirement: Qdrant child process supervision lifecycle
  Scenario:     Tauri parent exit triggers child termination via watchdog

Cross-platform pid liveness via psutil — avoids the Windows/Unix
divergence of ``os.kill(pid, 0)``.
"""
from __future__ import annotations

import asyncio
import logging
import os
from typing import Callable

import psutil

logger = logging.getLogger(__name__)

_DEFAULT_POLL_INTERVAL_S = 1.0


async def watch_parent(
    parent_pid: int,
    poll_interval_s: float = _DEFAULT_POLL_INTERVAL_S,
) -> None:
    """Return once ``parent_pid`` is no longer a running process.

    The poll interval is 1 s by default so the worst-case lag from
    parent-exit to sidecar-exit is ~1 s (well inside the 5 s SHALL).
    Tests override the interval for tighter bounds.
    """
    while psutil.pid_exists(parent_pid):
        await asyncio.sleep(poll_interval_s)


async def supervise_parent(
    parent_pid: int,
    *,
    on_exit: Callable[[], None] | None = None,
    poll_interval_s: float = _DEFAULT_POLL_INTERVAL_S,
    exit_code: int = 0,
) -> None:
    """Watch the parent and force-exit the sidecar when it disappears.

    Sequence:
      1. ``await watch_parent(parent_pid, poll_interval_s)``
      2. If ``on_exit`` is not None, call it (best-effort — exceptions
         are swallowed and logged so cleanup failure cannot block the
         force-exit). This is where the Qdrant child cleanup hook runs.
      3. ``os._exit(exit_code)`` — bypasses the FastAPI / uvicorn
         graceful shutdown that can hang on open connections, meeting
         the 5 s SHALL on watchdog-driven termination.

    Backs spec scenario "Tauri parent exit triggers child termination
    via watchdog" — the on_exit hook MUST run BEFORE os._exit so the
    Qdrant child receives SIGTERM before the sidecar process vanishes.
    """
    await watch_parent(parent_pid=parent_pid, poll_interval_s=poll_interval_s)

    if on_exit is not None:
        try:
            on_exit()
        except Exception as exc:  # noqa: BLE001 — best-effort cleanup
            logger.warning("supervise_parent on_exit hook raised: %s", exc)

    os._exit(exit_code)
