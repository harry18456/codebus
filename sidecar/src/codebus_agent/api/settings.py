"""Sidecar settings mutation endpoints + app-level SSE channel.

Backs SHALL clauses in
openspec/changes/provider-settings-and-onboarding/specs/sidecar-runtime/spec.md
  Requirement: Sidecar accepts provider config mutation endpoints
  Requirement: provider_config_changed SSE event surface
  Requirement: RegistryHolder enables atomic registry hot-swap (consumer)

Endpoints registered here:
  - GET    /settings/providers     → snapshot (no api_key)
  - POST   /settings/providers     → upsert one ProviderSpec (no api_key)
  - DELETE /settings/providers/{id}→ remove or 409 PROVIDER_BOUND_TO_ROLE
  - PUT    /settings/bindings      → swap RegistryHolder + emit event
  - PUT    /settings/pii-mode      → toggle rule | llm
  - GET    /events?channel=app     → SSE channel for app-level events

All endpoints carry `include_in_schema=False` per Decision 2 invariant.
"""
from __future__ import annotations

import asyncio
import json
import logging
from typing import Any

from fastapi import APIRouter, HTTPException, Query, Request, Response, status
from pydantic import BaseModel, ConfigDict, Field
from sse_starlette.sse import EventSourceResponse

from codebus_agent.config.llm_config_store import save_llm_config
from codebus_agent.config.provider_pool import (
    INVALID_PII_PROVIDER,
    ProviderPoolSnapshot,
    ProviderSpec,
)

from .events_broker import AppEventBroker, _STREAM_CLOSE_SENTINEL

logger = logging.getLogger(__name__)

router = APIRouter()


# Provider id regex from `keyring-integration` spec — re-used here so
# the same validation runs server-side.
_PROVIDER_ID_PATTERN = r"^[a-z][a-z0-9-]{2,40}$"

_VALID_PROVIDER_TYPES = frozenset({"openai_chat", "openai_embedding"})
_EMBEDDING_TYPES = frozenset({"openai_embedding"})
_PII_ALLOWED_TYPES: frozenset[str] = frozenset()  # P0 — empty


class _ProviderSpecBody(BaseModel):
    """Body schema for `POST /settings/providers`.

    `model_config = ConfigDict(extra="forbid")` rejects any extra
    field — including `api_key` — with a 422 so a buggy client can't
    sneak the secret onto the wire (D-033 B Decision 1 invariant).
    """

    model_config = ConfigDict(extra="forbid")

    id: str = Field(pattern=_PROVIDER_ID_PATTERN)
    type: str
    model: str
    base_url: str


class _BindingsBody(BaseModel):
    model_config = ConfigDict(extra="forbid")

    reasoning: str | None = None
    judge: str | None = None
    chat: str | None = None
    embed: str | None = None


class _PiiModeBody(BaseModel):
    model_config = ConfigDict(extra="forbid")

    mode: str
    provider_id: str | None = None


def _snapshot_dict(snapshot: ProviderPoolSnapshot) -> dict[str, Any]:
    """Serialize a snapshot for the wire — never includes `api_key`."""
    return {
        "providers": [
            {
                "id": p.id,
                "type": p.type,
                "model": p.model,
                "base_url": p.base_url,
            }
            for p in snapshot.providers
        ],
        "bindings": dict(snapshot.bindings),
        "pii_mode": snapshot.pii_mode,
        "pii_provider_id": snapshot.pii_provider_id,
    }


def _require_snapshot(request: Request) -> ProviderPoolSnapshot:
    snapshot = getattr(request.app.state, "provider_pool_snapshot", None)
    if snapshot is None:
        raise HTTPException(
            status_code=status.HTTP_503_SERVICE_UNAVAILABLE,
            detail={"code": "PROVIDER_POOL_NOT_INITIALIZED"},
        )
    return snapshot


def _persist_snapshot(request: Request) -> None:
    """Mirror the in-memory snapshot to ``~/.codebus/llm-config.json``.

    Called from every mutation endpoint after the in-memory state has
    been updated. Failure here is logged but does NOT 5xx the mutation —
    the in-memory state already reflects the user's intent and the
    next successful mutation will retry the disk write. This trades
    durability for liveness: a transient ENOSPC won't strand the
    user mid-onboarding.
    """
    snapshot = getattr(request.app.state, "provider_pool_snapshot", None)
    if snapshot is None:
        return
    try:
        save_llm_config(snapshot)
    except OSError as e:
        logger.error("save_llm_config failed: %s", e)


def _require_broker(request: Request) -> AppEventBroker:
    broker = getattr(request.app.state, "app_event_broker", None)
    if broker is None:
        raise HTTPException(
            status_code=status.HTTP_503_SERVICE_UNAVAILABLE,
            detail={"code": "APP_EVENT_BROKER_NOT_INITIALIZED"},
        )
    return broker


@router.get("/settings/providers", include_in_schema=False)
async def get_providers(request: Request) -> dict[str, Any]:
    """Return the current pool snapshot. API keys MUST NOT appear."""
    snapshot = _require_snapshot(request)
    return _snapshot_dict(snapshot)


@router.post(
    "/settings/providers",
    status_code=status.HTTP_204_NO_CONTENT,
    include_in_schema=False,
)
async def upsert_provider(
    body: _ProviderSpecBody, request: Request
) -> Response:
    """Add or replace a `ProviderSpec` in the in-memory pool."""
    if body.type not in _VALID_PROVIDER_TYPES:
        raise HTTPException(
            status_code=status.HTTP_400_BAD_REQUEST,
            detail={
                "code": "INVALID_PROVIDER_TYPE",
                "allowed": sorted(_VALID_PROVIDER_TYPES),
            },
        )
    snapshot = _require_snapshot(request)
    spec = ProviderSpec(
        id=body.id, type=body.type, model=body.model, base_url=body.base_url
    )
    new_providers = tuple(p for p in snapshot.providers if p.id != spec.id) + (
        spec,
    )
    request.app.state.provider_pool_snapshot = ProviderPoolSnapshot(
        providers=new_providers,
        bindings=dict(snapshot.bindings),
        pii_mode=snapshot.pii_mode,
        pii_provider_id=snapshot.pii_provider_id,
    )
    _persist_snapshot(request)
    broker = _require_broker(request)
    await broker.emit_provider_config_changed(
        changed_roles=[],
        embed_changed=False,
        providers_pool_changed=True,
    )
    return Response(status_code=status.HTTP_204_NO_CONTENT)


@router.delete("/settings/providers/{provider_id}", include_in_schema=False)
async def delete_provider(provider_id: str, request: Request) -> Response:
    """Delete a provider; 409 if any role still binds to it."""
    snapshot = _require_snapshot(request)
    bound_roles = sorted(
        role
        for role, bound_id in snapshot.bindings.items()
        if bound_id == provider_id
    )
    if bound_roles:
        raise HTTPException(
            status_code=status.HTTP_409_CONFLICT,
            detail={
                "code": "PROVIDER_BOUND_TO_ROLE",
                "provider_id": provider_id,
                "roles": bound_roles,
            },
        )
    new_providers = tuple(p for p in snapshot.providers if p.id != provider_id)
    request.app.state.provider_pool_snapshot = ProviderPoolSnapshot(
        providers=new_providers,
        bindings=dict(snapshot.bindings),
        pii_mode=snapshot.pii_mode,
        pii_provider_id=snapshot.pii_provider_id,
    )
    _persist_snapshot(request)
    broker = _require_broker(request)
    await broker.emit_provider_config_changed(
        changed_roles=[],
        embed_changed=False,
        providers_pool_changed=True,
    )
    return Response(status_code=status.HTTP_204_NO_CONTENT)


@router.put(
    "/settings/bindings",
    status_code=status.HTTP_204_NO_CONTENT,
    include_in_schema=False,
)
async def put_bindings(body: _BindingsBody, request: Request) -> Response:
    """Update role bindings + swap the active `RegistryHolder` reference."""
    snapshot = _require_snapshot(request)
    by_id = {p.id: p for p in snapshot.providers}

    new_bindings = dict(snapshot.bindings)
    changed: list[str] = []
    embed_changed = False

    for role in ("reasoning", "judge", "chat", "embed"):
        new_value = getattr(body, role)
        if new_value is None:
            continue
        if new_value not in by_id:
            raise HTTPException(
                status_code=status.HTTP_400_BAD_REQUEST,
                detail={
                    "code": "INVALID_PROVIDER_BINDING",
                    "role": role,
                    "provider_id": new_value,
                },
            )
        if role == "embed" and by_id[new_value].type not in _EMBEDDING_TYPES:
            raise HTTPException(
                status_code=status.HTTP_400_BAD_REQUEST,
                detail={
                    "code": "INVALID_PROVIDER_TYPE",
                    "role": role,
                    "provider_id": new_value,
                },
            )
        if new_bindings.get(role) != new_value:
            changed.append(role)
            if role == "embed":
                embed_changed = True
            new_bindings[role] = new_value

    request.app.state.provider_pool_snapshot = ProviderPoolSnapshot(
        providers=snapshot.providers,
        bindings=new_bindings,
        pii_mode=snapshot.pii_mode,
        pii_provider_id=snapshot.pii_provider_id,
    )
    _persist_snapshot(request)

    holder = getattr(request.app.state, "providers", None)
    factory = getattr(request.app.state, "registry_factory", None)
    if holder is not None and factory is not None:
        keys = getattr(request.app.state, "provider_keys", {})
        new_registry = factory(
            request.app.state.provider_pool_snapshot, dict(keys)
        )
        await holder.swap(new_registry)

    if changed:
        broker = _require_broker(request)
        await broker.emit_provider_config_changed(
            changed_roles=changed,
            embed_changed=embed_changed,
            providers_pool_changed=False,
        )
    return Response(status_code=status.HTTP_204_NO_CONTENT)


@router.put(
    "/settings/pii-mode",
    status_code=status.HTTP_204_NO_CONTENT,
    include_in_schema=False,
)
async def put_pii_mode(body: _PiiModeBody, request: Request) -> Response:
    """Toggle `rule | llm`; `llm` requires a PII-allowlisted provider."""
    if body.mode not in ("rule", "llm"):
        raise HTTPException(
            status_code=status.HTTP_400_BAD_REQUEST,
            detail={"code": INVALID_PII_PROVIDER, "reason": "unknown mode"},
        )
    snapshot = _require_snapshot(request)
    if body.mode == "llm":
        if not body.provider_id:
            raise HTTPException(
                status_code=status.HTTP_400_BAD_REQUEST,
                detail={
                    "code": INVALID_PII_PROVIDER,
                    "reason": "provider_id required when mode is llm",
                },
            )
        provider = next(
            (p for p in snapshot.providers if p.id == body.provider_id),
            None,
        )
        if provider is None:
            raise HTTPException(
                status_code=status.HTTP_400_BAD_REQUEST,
                detail={
                    "code": INVALID_PII_PROVIDER,
                    "reason": f"unknown provider_id {body.provider_id!r}",
                },
            )
        if provider.type not in _PII_ALLOWED_TYPES:
            raise HTTPException(
                status_code=status.HTTP_400_BAD_REQUEST,
                detail={
                    "code": INVALID_PII_PROVIDER,
                    "reason": (
                        f"provider type {provider.type!r} not in PII allowlist"
                    ),
                },
            )

    request.app.state.provider_pool_snapshot = ProviderPoolSnapshot(
        providers=snapshot.providers,
        bindings=dict(snapshot.bindings),
        pii_mode=body.mode,  # type: ignore[arg-type]
        pii_provider_id=body.provider_id,
    )
    _persist_snapshot(request)
    broker = _require_broker(request)
    await broker.emit_provider_config_changed(
        changed_roles=[],
        embed_changed=False,
        providers_pool_changed=False,
    )
    return Response(status_code=status.HTTP_204_NO_CONTENT)


@router.get("/events", include_in_schema=False)
async def stream_app_events(
    request: Request, channel: str = Query(...)
) -> EventSourceResponse:
    """Subscribe to the app-level event stream.

    Per spec: `GET /events?channel=app` is the only supported channel
    today. Other values get a 400 so the surface stays explicit.
    """
    if channel != "app":
        raise HTTPException(
            status_code=status.HTTP_400_BAD_REQUEST,
            detail={"code": "UNKNOWN_CHANNEL", "channel": channel},
        )
    broker = _require_broker(request)
    queue = broker.subscribe()

    async def _generator():
        try:
            while True:
                event = await queue.get()
                if event is _STREAM_CLOSE_SENTINEL:
                    return
                yield {
                    "data": json.dumps(
                        event, separators=(",", ":"), ensure_ascii=False
                    )
                }
        finally:
            broker.unsubscribe(queue)

    return EventSourceResponse(_generator())


__all__ = ["router"]
