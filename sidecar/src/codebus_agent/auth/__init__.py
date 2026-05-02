"""Authorization subpackage — bearer middleware + App-level audit subsystem.

Originally a single ``codebus_agent.auth`` module hosting only the
bearer middleware (``m1-power-on``); upgraded to a package by
``auth-flow`` to absorb the App-level authorization audit subsystem
(``paths.py`` / ``audit_logger.py`` / ``service.py`` / ``errors.py``)
without churning callers that did ``from codebus_agent import auth;
auth.install(...)``.

The bearer middleware lives directly in this ``__init__`` so existing
patch paths (e.g. ``patch("codebus_agent.auth.secrets.compare_digest")``)
continue to work unchanged.

The seventh audit layer is *App-level* (cross-workspace), distinct
from the six workspace-level audit JSONL files under
``<workspace>/.codebus/`` whose filenames live in
``codebus_agent._audit_paths``. See ``auth/paths.py`` for the
canonical filename constant.
"""
from __future__ import annotations

import re
import secrets

from fastapi import FastAPI, Request
from starlette.middleware.base import BaseHTTPMiddleware
from starlette.responses import JSONResponse
from starlette.types import ASGIApp

_BEARER_PREFIX = "Bearer "

# Path scope for the SSE query-param bearer fallback (Decision 1 of
# `sidecar-sse-bearer-query-param-fallback`). The query-param transport is
# accepted ONLY on `GET /tasks/<task_id>/events` because browser-native
# `EventSource` cannot set the `Authorization` header. Anchored start/end
# prevent suffix tricks like `/tasks/foo/events/leak`.
_SSE_EVENTS_PATH_RE = re.compile(r"^/tasks/[^/]+/events$")


def _is_sse_events_path(path: str | None) -> bool:
    """Return True iff *path* matches the narrow SSE-events fallback scope."""
    if not path:
        return False
    return _SSE_EVENTS_PATH_RE.match(path) is not None


def generate_token() -> str:
    """Generate a cryptographically strong URL-safe token.

    32 bytes of entropy → 43-char base64 string. Exceeds the 32-char
    minimum asserted by the handshake scenario.
    """
    return secrets.token_urlsafe(32)


class BearerAuthMiddleware(BaseHTTPMiddleware):
    """Reject any request whose Authorization header does not carry the
    exact startup-generated bearer token.

    The comparison uses ``secrets.compare_digest`` to avoid leaking
    token length via response-time timing.
    """

    def __init__(self, app: ASGIApp, expected_token: str) -> None:
        super().__init__(app)
        self._expected = expected_token

    async def dispatch(self, request: Request, call_next):  # type: ignore[override]
        header = request.headers.get("Authorization", "")
        presented: str | None
        if header.startswith(_BEARER_PREFIX):
            presented = header[len(_BEARER_PREFIX):]
        elif _is_sse_events_path(request.url.path):
            # Browser-native EventSource cannot set custom headers, so the
            # SSE events path accepts the bearer via `?bearer=<token>`.
            # Path-scoped fallback only — Decision 1 of
            # `sidecar-sse-bearer-query-param-fallback`.
            presented = request.query_params.get("bearer")
        else:
            presented = None
        if presented is None or not secrets.compare_digest(
            presented, self._expected
        ):
            return JSONResponse({"detail": "unauthorized"}, status_code=401)
        return await call_next(request)


def install(app: FastAPI, bearer_token: str) -> None:
    """Attach the bearer middleware to a FastAPI app."""
    app.add_middleware(BearerAuthMiddleware, expected_token=bearer_token)


__all__ = [
    "BearerAuthMiddleware",
    "generate_token",
    "install",
    "_is_sse_events_path",
]
