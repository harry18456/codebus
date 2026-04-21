"""CLI self-check run when sidecar is invoked with ``--healthz``.

Backs SHALL clauses in
openspec/changes/m1-power-on/specs/app-packaging/spec.md
  Requirement: Packaged binary health check
    Scenario: Healthz flag succeeds after build
    Scenario: Healthz reports degraded if optional dependency missing

and openspec/changes/qdrant-lifecycle-bootstrap/specs/qdrant-client/spec.md
  Requirement: CODEBUS_QDRANT_URL resolution has a single source of truth
    Scenario: healthz CLI uses the shared resolver

The self-check never starts an HTTP server — it probes each
dependency once and returns a `HealthReport`.  Exit code is always 0
(even when degraded) so CI can separate "binary crashed" from
"Qdrant not running yet"; the distinction lives in the `status`
field of the JSON line.
"""
from __future__ import annotations

import asyncio

from codebus_agent.health import DependencyStatus, HealthReport, collect
from codebus_agent.kb import qdrant_client as _kb_qdrant


async def _check_qdrant() -> DependencyStatus:
    url = _kb_qdrant.resolve_url()
    return await asyncio.to_thread(_kb_qdrant.probe, url)


async def run_self_check() -> HealthReport:
    checks = {"qdrant": _check_qdrant}
    return await collect(checks)
