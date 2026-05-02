"""SSE bearer query-param fallback tests — backs spec
``openspec/specs/sidecar-runtime/spec.md`` Requirement
"Bearer token authentication" (modified by
``sidecar-sse-bearer-query-param-fallback``).
"""
from __future__ import annotations

import secrets

import httpx
import pytest
from fastapi import FastAPI

from codebus_agent.auth import BearerAuthMiddleware, _is_sse_events_path


@pytest.mark.parametrize(
    "path, expected",
    [
        ("/tasks/scan_a1b2c3d4/events", True),
        ("/tasks/foo/events/leak", False),
        ("/scan", False),
        ("/tasks/scan_xxx/result", False),
        ("", False),
        (None, False),
    ],
)
def test_is_sse_events_path(path: str | None, expected: bool) -> None:
    """Scenario coverage for the path-scoping regex (Decision 1)."""
    assert _is_sse_events_path(path) is expected


@pytest.fixture
def bearer() -> str:
    return secrets.token_urlsafe(32)


@pytest.fixture
def middleware_app(bearer: str) -> FastAPI:
    """Minimal FastAPI app exercising only ``BearerAuthMiddleware``.

    Mounts a dummy GET ``/tasks/{task_id}/events`` and POST ``/scan`` so
    the middleware can be tested without pulling in the full sidecar
    create_app() dependency tree.
    """
    app = FastAPI()
    app.add_middleware(BearerAuthMiddleware, expected_token=bearer)

    @app.get("/tasks/{task_id}/events")
    def events(task_id: str) -> dict[str, str]:
        return {"task_id": task_id}

    @app.post("/scan")
    def scan() -> dict[str, str]:
        return {"status": "reached"}

    return app


@pytest.mark.asyncio
async def test_sse_events_accepts_query_bearer(
    middleware_app: FastAPI, bearer: str
) -> None:
    """Scenario: SSE events endpoint accepts bearer via query parameter."""
    transport = httpx.ASGITransport(app=middleware_app)
    async with httpx.AsyncClient(
        transport=transport, base_url="http://testserver"
    ) as client:
        response = await client.get(
            f"/tasks/scan_xxx/events?bearer={bearer}",
        )
    assert response.status_code == 200
    assert response.json() == {"task_id": "scan_xxx"}


@pytest.mark.asyncio
async def test_sse_events_rejects_wrong_query_bearer(
    middleware_app: FastAPI,
) -> None:
    """Scenario: Wrong bearer in query parameter rejected."""
    transport = httpx.ASGITransport(app=middleware_app)
    async with httpx.AsyncClient(
        transport=transport, base_url="http://testserver"
    ) as client:
        response = await client.get(
            "/tasks/scan_xxx/events?bearer=not-the-real-token",
        )
    assert response.status_code == 401


@pytest.mark.asyncio
async def test_non_sse_endpoint_rejects_query_bearer(
    middleware_app: FastAPI, bearer: str
) -> None:
    """Scenario: Non-SSE endpoints reject query-parameter bearer.

    POST /scan?bearer=<correct> without Authorization header MUST return
    401 — path-scoped fallback (Decision 4: default deny + narrow allow).
    """
    transport = httpx.ASGITransport(app=middleware_app)
    async with httpx.AsyncClient(
        transport=transport, base_url="http://testserver"
    ) as client:
        response = await client.post(f"/scan?bearer={bearer}")
    assert response.status_code == 401


@pytest.mark.asyncio
async def test_sse_events_accepts_header_bearer(
    middleware_app: FastAPI, bearer: str
) -> None:
    """Scenario: Correct bearer accepted (header path on SSE endpoint).

    Existing reasoning_log SSE test uses the header path; the
    query-param fallback must not break it.
    """
    transport = httpx.ASGITransport(app=middleware_app)
    async with httpx.AsyncClient(
        transport=transport, base_url="http://testserver"
    ) as client:
        response = await client.get(
            "/tasks/scan_xxx/events",
            headers={"Authorization": f"Bearer {bearer}"},
        )
    assert response.status_code == 200


@pytest.mark.asyncio
async def test_sse_events_accepts_both_header_and_query(
    middleware_app: FastAPI, bearer: str
) -> None:
    """Scenario: SSE endpoint accepts when both transports valid.

    Header takes precedence; either valid transport satisfies the
    requirement.
    """
    transport = httpx.ASGITransport(app=middleware_app)
    async with httpx.AsyncClient(
        transport=transport, base_url="http://testserver"
    ) as client:
        response = await client.get(
            f"/tasks/scan_xxx/events?bearer={bearer}",
            headers={"Authorization": f"Bearer {bearer}"},
        )
    assert response.status_code == 200
