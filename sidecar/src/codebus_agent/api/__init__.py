"""FastAPI application factory.

Backs:
- openspec/changes/m1-power-on/specs/sidecar-runtime/spec.md
    Requirements: Bearer token authentication, Health endpoint
- openspec/changes/qdrant-lifecycle-bootstrap/specs/qdrant-client/spec.md
    Requirements:
      - Async Qdrant client lifecycle bound to FastAPI app
      - Runtime health endpoint reflects Qdrant connectivity

Qdrant is wired in as a first-class runtime dependency: if the caller
passes ``qdrant_url``, the factory constructs a single ``AsyncQdrantClient``
on ``app.state.qdrant_client`` (per design「single async client，app state
常駐」) and auto-registers a probe-backed dependency check so ``/healthz``
mirrors live connectivity. Construction never touches the network, so
a missing Qdrant does not block startup (design「degraded-but-alive」).
"""
from __future__ import annotations

import asyncio

from fastapi import FastAPI

from codebus_agent import auth
from codebus_agent.health import DependencyCheck, DependencyStatus, collect
from codebus_agent.kb import qdrant_client as _kb_qdrant


def create_app(
    bearer_token: str,
    dependency_checks: dict[str, DependencyCheck] | None = None,
    qdrant_url: str | None = None,
) -> FastAPI:
    """Build the sidecar FastAPI application.

    The bearer token is passed in at construction time so it lives only
    in memory for the lifetime of this process, per D-local-2.

    ``dependency_checks`` is injected so tests (and M2+ wiring) can plug
    in custom probes. When ``qdrant_url`` is given, a Qdrant probe is
    auto-bound under the ``"qdrant"`` key unless the caller overrides it.
    """
    if not bearer_token or len(bearer_token) < 32:
        raise ValueError("bearer_token must be at least 32 characters")
    app = FastAPI(title="codebus-sidecar", version="0.1.0")
    app.state.bearer_token = bearer_token
    app.state.qdrant_client = None

    checks: dict[str, DependencyCheck] = dict(dependency_checks or {})

    if qdrant_url is not None:
        app.state.qdrant_client = _kb_qdrant.build_client(qdrant_url)

        if "qdrant" not in checks:
            async def _qdrant_probe() -> DependencyStatus:
                return await asyncio.to_thread(_kb_qdrant.probe, qdrant_url)

            checks["qdrant"] = _qdrant_probe

        @app.on_event("shutdown")
        async def _close_qdrant() -> None:
            client = getattr(app.state, "qdrant_client", None)
            if client is not None:
                await client.close()

    app.state.dependency_checks = checks
    auth.install(app, bearer_token)

    @app.get("/healthz")
    async def healthz() -> dict[str, object]:
        report = await collect(app.state.dependency_checks)
        return report.to_dict()

    return app
