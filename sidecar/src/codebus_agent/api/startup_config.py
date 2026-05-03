"""``POST /internal/startup-config`` — Tauri-to-sidecar key injection.

Backs SHALL clauses in
``openspec/changes/phase7-onboarding-polish/specs/keyring-integration/spec.md``
  Requirement: Tauri-to-sidecar startup key injection (idempotent lock relaxed)

Trust boundary contract:
  * Endpoint is bearer-authenticated through the existing app-wide
    middleware — no exemption added here.
  * Repeatable: callable any number of times during a sidecar process
    lifetime. The latest body REPLACES ``app.state.provider_keys``
    wholesale (not merged). D-033 B's original "idempotent 409 lock"
    is relaxed in `phase7-onboarding-polish` so onboarding can push
    keys after the user enters them — the trust boundary is unchanged
    (bearer + loopback + Tauri-only caller).
  * ``include_in_schema=False`` keeps the path out of
    ``GET /openapi.json``. The endpoint is callable but not
    advertised on the public API surface.
  * The handler stores the keys in ``app.state.provider_keys`` only;
    nothing is logged / persisted / passed to the sanitizer audit
    chain (D-033 §B unfounded invariant 1).
"""
from __future__ import annotations

import logging

from fastapi import APIRouter, Request, Response, status
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

    On any call: HTTP 204 with no body. ``app.state.provider_keys`` is
    REPLACED wholesale by the supplied dict and
    ``app.state.startup_config_applied`` flag is set to True so other
    code paths can still distinguish "boot has injected at least once"
    from "no injection yet". Repeat calls overwrite the previous keys
    — the latest body always wins (relaxed in
    `phase7-onboarding-polish` so onboarding can push keys after the
    user enters them).

    After storing keys, the handler re-runs ``wire_kb_dependencies`` so
    the lazily-constructed factories on ``app.state.kb_*`` /
    ``app.state.llm_*_provider`` pick up the freshly resolved
    per-binding keys (D-033 B integration fix — without this, the
    endpoint stored keys but production traffic still hit the legacy
    env-var-only provider constructors and ``POST /kb/build`` returned
    503 ``KB_NOT_CONFIGURED`` despite onboarding completing).

    The handler intentionally does NOT log the api_key values. Only
    the count is logged for operational visibility.
    """
    app = request.app
    app.state.provider_keys = dict(body.provider_keys)
    app.state.startup_config_applied = True
    logger.info(
        "startup-config applied: %d provider key(s) loaded",
        len(app.state.provider_keys),
    )
    # Late import avoids the circular dependency between this leaf
    # router module and the FastAPI factory that imports it.
    from codebus_agent.api import wire_kb_dependencies

    wire_kb_dependencies(
        app,
        openai_api_key=None,  # legacy fallback unused — keys are in app.state.provider_keys
        qdrant_url=getattr(app.state, "qdrant_url", None),
    )
    return Response(status_code=status.HTTP_204_NO_CONTENT)


__all__ = ["router"]
