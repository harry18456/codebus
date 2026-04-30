"""``POST /internal/startup-config`` — Tauri-to-sidecar key injection.

Backs SHALL clauses in
``openspec/changes/provider-settings-and-onboarding/specs/keyring-integration/spec.md``
  Requirement: Tauri-to-sidecar startup key injection (4 scenarios)

Trust boundary contract:
  * Endpoint is bearer-authenticated through the existing app-wide
    middleware — no exemption added here.
  * Idempotent: SHALL be called at most once per sidecar process
    lifetime. The second call within the same process is rejected
    with 409 ``STARTUP_ALREADY_CONFIGURED`` and MUST NOT mutate
    ``app.state.provider_keys``.
  * ``include_in_schema=False`` keeps the path out of
    ``GET /openapi.json``. The endpoint is callable but not
    advertised on the public API surface.
  * The handler stores the keys in ``app.state.provider_keys`` only;
    nothing is logged / persisted / passed to the sanitizer audit
    chain (D-033 §B unfounded invariant 1).
"""
from __future__ import annotations

import logging

from fastapi import APIRouter, HTTPException, Request, Response, status
from pydantic import BaseModel

logger = logging.getLogger(__name__)

router = APIRouter(prefix="/internal")


class _StartupConfigBody(BaseModel):
    """Body schema accepted by ``POST /internal/startup-config``.

    Note: keep this Pydantic model strictly minimal. Adding fields
    here is a trust-boundary change and MUST go through
    ``/spectra-propose``. ``provider_keys`` is required so an empty
    body returns 422 (rather than silently accepting no-op state).
    """

    provider_keys: dict[str, str]


@router.post(
    "/startup-config",
    status_code=status.HTTP_204_NO_CONTENT,
    include_in_schema=False,
)
async def post_startup_config(body: _StartupConfigBody, request: Request) -> Response:
    """Inject API keys collected from the OS keychain into sidecar memory.

    On success: HTTP 204 with no body. ``app.state.provider_keys`` is
    set to the supplied dict and ``app.state.startup_config_applied``
    flag is flipped to True.

    On second call: HTTP 409 with ``{"detail": {"code": "STARTUP_ALREADY_CONFIGURED"}}``.
    The state from the first call is preserved (the second body is
    discarded — the handler never touches state until after the
    idempotent check passes).

    The handler intentionally does NOT log the api_key values. Only
    the count is logged for operational visibility.
    """
    app = request.app
    if getattr(app.state, "startup_config_applied", False):
        raise HTTPException(
            status_code=status.HTTP_409_CONFLICT,
            detail={"code": "STARTUP_ALREADY_CONFIGURED"},
        )
    app.state.provider_keys = dict(body.provider_keys)
    app.state.startup_config_applied = True
    logger.info(
        "startup-config applied: %d provider key(s) loaded",
        len(app.state.provider_keys),
    )
    return Response(status_code=status.HTTP_204_NO_CONTENT)


__all__ = ["router"]
