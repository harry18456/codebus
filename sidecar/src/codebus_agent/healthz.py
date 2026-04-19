"""CLI self-check run when sidecar is invoked with ``--healthz``.

Backs SHALL clauses in
openspec/changes/m1-power-on/specs/app-packaging/spec.md
  Requirement: Packaged binary health check
    Scenario: Healthz flag succeeds after build
    Scenario: Healthz reports degraded if optional dependency missing

The self-check never starts an HTTP server — it probes each
dependency once and returns a `HealthReport`.  Exit code is always 0
(even when degraded) so CI can separate "binary crashed" from
"Qdrant not running yet"; the distinction lives in the `status`
field of the JSON line.
"""
from __future__ import annotations

import asyncio
import os
from urllib.error import URLError
from urllib.request import urlopen

from codebus_agent.health import DependencyStatus, HealthReport, collect

DEFAULT_QDRANT_URL = "http://127.0.0.1:6333"


def _qdrant_url() -> str:
    return os.environ.get("CODEBUS_QDRANT_URL", DEFAULT_QDRANT_URL)


async def _check_qdrant() -> DependencyStatus:
    url = _qdrant_url().rstrip("/")

    def _probe() -> DependencyStatus:
        try:
            with urlopen(f"{url}/readyz", timeout=1.0) as resp:
                return DependencyStatus(ok=resp.status == 200, detail=url)
        except (URLError, TimeoutError, ConnectionError, OSError) as exc:
            return DependencyStatus(
                ok=False, detail=f"{url} ({type(exc).__name__})"
            )

    return await asyncio.to_thread(_probe)


async def run_self_check() -> HealthReport:
    checks = {"qdrant": _check_qdrant}
    return await collect(checks)
