"""Sanity check: CORS preflight from the Tauri WebView dev origin
short-circuits before bearer middleware rejects it as unauthenticated.

Backs the dev-mode `cargo tauri dev` smoke flow — without this the
WebView (loaded from http://localhost:3000) cannot call sidecar
endpoints because the OPTIONS preflight gets 401-ed by bearer auth
and the browser never issues the real GET / POST.
"""

from __future__ import annotations

from fastapi.testclient import TestClient

from codebus_agent.api import create_app


def _client() -> TestClient:
    app = create_app(bearer_token="x" * 40)
    return TestClient(app)


def test_preflight_from_localhost_3000_short_circuits_before_bearer() -> None:
    client = _client()
    response = client.options(
        "/healthz",
        headers={
            "Origin": "http://localhost:3000",
            "Access-Control-Request-Method": "GET",
            "Access-Control-Request-Headers": "Authorization",
        },
    )
    assert response.status_code == 200, (
        f"preflight must short-circuit at CORS, got {response.status_code} "
        f"(probably bearer middleware ate it before CORS could respond)"
    )
    assert (
        response.headers.get("access-control-allow-origin")
        == "http://localhost:3000"
    ), response.headers
    assert "GET" in response.headers.get("access-control-allow-methods", "")
    assert "Authorization" in response.headers.get(
        "access-control-allow-headers", ""
    )


def test_preflight_from_tauri_localhost_origin() -> None:
    client = _client()
    response = client.options(
        "/healthz",
        headers={
            "Origin": "http://tauri.localhost",
            "Access-Control-Request-Method": "GET",
            "Access-Control-Request-Headers": "Authorization",
        },
    )
    assert response.status_code == 200
    assert (
        response.headers.get("access-control-allow-origin")
        == "http://tauri.localhost"
    )


def test_preflight_from_disallowed_origin_does_not_grant_cors() -> None:
    client = _client()
    response = client.options(
        "/healthz",
        headers={
            "Origin": "http://evil.example.com",
            "Access-Control-Request-Method": "GET",
            "Access-Control-Request-Headers": "Authorization",
        },
    )
    # CORSMiddleware does not throw on unknown origins; it just omits
    # the Allow-Origin header so the browser refuses the response.
    assert "access-control-allow-origin" not in {k.lower() for k in response.headers}


def test_real_get_with_bearer_passes_after_cors_attaches_origin_header() -> None:
    client = _client()
    response = client.get(
        "/healthz",
        headers={
            "Origin": "http://localhost:3000",
            "Authorization": "Bearer " + "x" * 40,
        },
    )
    assert response.status_code == 200
    assert (
        response.headers.get("access-control-allow-origin")
        == "http://localhost:3000"
    )


def test_real_get_without_bearer_still_401_even_with_allowed_origin() -> None:
    """CORS allowlist MUST NOT bypass bearer auth — Allow-Origin is a
    browser hint, not an authentication primitive."""
    client = _client()
    response = client.get(
        "/healthz",
        headers={"Origin": "http://localhost:3000"},
    )
    assert response.status_code == 401
