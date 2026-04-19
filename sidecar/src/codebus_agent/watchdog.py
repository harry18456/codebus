"""Parent-process watchdog.

Backs openspec/changes/m1-power-on/specs/sidecar-runtime/spec.md
  Requirement: Parent-process watchdog
  Design:       D-local-2 (sidecar self-terminates when parent disappears)

Cross-platform pid liveness via psutil — avoids the Windows/Unix
divergence of ``os.kill(pid, 0)``.
"""
from __future__ import annotations

import asyncio

import psutil

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
