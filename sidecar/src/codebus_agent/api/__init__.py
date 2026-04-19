"""FastAPI application factory.

Backs openspec/changes/m1-power-on/specs/sidecar-runtime/spec.md
(Requirements: Bearer token authentication, Health endpoint).

Behaviour — bearer middleware, /healthz — is driven in by the tests
that follow (tasks 2.3+ / 2.5+).  This module keeps only the factory
signature so downstream tests can import it.
"""
from __future__ import annotations

from fastapi import FastAPI

from codebus_agent import auth
from codebus_agent.health import DependencyCheck, collect


def create_app(
    bearer_token: str,
    dependency_checks: dict[str, DependencyCheck] | None = None,
) -> FastAPI:
    """Build the sidecar FastAPI application.

    The bearer token is passed in at construction time so it lives only
    in memory for the lifetime of this process, per D-local-2.

    ``dependency_checks`` is injected so tests (and M2+ wiring) can plug
    in Qdrant / LLM / KB probes without this module depending on them.
    """
    if not bearer_token or len(bearer_token) < 32:
        raise ValueError("bearer_token must be at least 32 characters")
    app = FastAPI(title="codebus-sidecar", version="0.1.0")
    app.state.bearer_token = bearer_token
    app.state.dependency_checks = dependency_checks or {}
    auth.install(app, bearer_token)

    @app.get("/healthz")
    async def healthz() -> dict[str, object]:
        report = await collect(app.state.dependency_checks)
        return report.to_dict()

    return app
