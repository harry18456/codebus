"""Authorization HTTP endpoints — sync, bearer-protected, no SSE channel.

Backs SHALL clauses in
``openspec/changes/auth-flow/specs/authorization-audit/spec.md``
(``Requirement: Four sync sidecar endpoints under bearer middleware``)
and ``openspec/changes/auth-flow/specs/sidecar-runtime/spec.md``
(``Requirement: Authorization endpoints registration``).

Four endpoints under prefix ``/auth/``:
  * ``POST /auth/grant``  — validate workspace, write grant_issued, register session
  * ``POST /auth/deny``   — write grant_denied, no session
  * ``POST /auth/revoke`` — clear session, write grant_revoked
  * ``GET  /auth/status`` — read audit log, return AuthStatusResponse

All four are subject to the same bearer middleware as every other
sidecar route. None of them use the ``TaskRegistry`` or extend the
``task_id`` regex; auth flow is sync HTTP, not SSE background work.

In-memory session dict (``_session_dict``) is module-private state per
design D-A5: P0 accepts that sidecar restart loses sessions, since
audit log persistence is the canonical authorization record. The
trade-off appears in capability spec scenario `R-3`.
"""
from __future__ import annotations

from datetime import datetime, timezone
from pathlib import Path
from typing import Any

from fastapi import APIRouter, HTTPException, Request, status

from codebus_agent.auth.errors import (
    AUTH_INVALID_REQUEST,
    AUTH_NOT_CONFIGURED,
    AUTH_NO_ACTIVE_GRANT,
    AUTH_WORKSPACE_INVALID,
)
from codebus_agent.auth.service import (
    AuthStatusResponse,
    DenyRequest,
    GrantRequest,
    GrantResponse,
    RevokeRequest,
    extract_acked_kinds,
    find_last_grant_for_workspace,
    fresh_session_id,
    validate_scenario_invariants,
    workspace_id_for_path,
)
from codebus_agent.sanitizer import RULES_VERSION

router = APIRouter(prefix="/auth")


# Module-private session table. ``_GrantSession`` is intentionally a
# plain dict (not a Pydantic model) so we can mutate `granted_at` /
# `workspace_id` without re-validating on every read.
_session_dict: dict[str, dict[str, Any]] = {}


def _detail(code: str, message: str) -> dict[str, dict[str, str]]:
    return {"detail": {"code": code, "message": message}}


def _get_factory(request: Request):
    factory = getattr(request.app.state, "auth_audit_logger_factory", None)
    if factory is None:
        raise HTTPException(
            status_code=status.HTTP_503_SERVICE_UNAVAILABLE,
            detail={
                "code": AUTH_NOT_CONFIGURED,
                "message": "auth_audit_logger_factory is not configured on app.state",
            },
        )
    return factory


def _iso_utc_now() -> str:
    return datetime.now(timezone.utc).isoformat(timespec="milliseconds")


def _reject_topic_mode(workspace_type: str) -> None:
    if workspace_type == "topic":
        raise HTTPException(
            status_code=status.HTTP_501_NOT_IMPLEMENTED,
            detail={
                "code": AUTH_INVALID_REQUEST,
                "message": "topic mode reserved for Phase 2",
            },
        )


@router.post("/grant", response_model=GrantResponse)
def post_grant(req: GrantRequest, request: Request) -> GrantResponse:
    factory = _get_factory(request)
    _reject_topic_mode(req.workspace_type)

    # workspace_root validation (capability spec scenario "POST /auth/grant
    # rejects invalid workspace path"). At this point workspace_type is
    # MUST be "folder" — topic was filtered above.
    raw_path = req.workspace_source.get("path") if isinstance(req.workspace_source, dict) else None
    if not isinstance(raw_path, str) or not raw_path:
        raise HTTPException(
            status_code=status.HTTP_400_BAD_REQUEST,
            detail={
                "code": AUTH_WORKSPACE_INVALID,
                "message": "workspace_source.path missing or empty",
            },
        )
    workspace_path = Path(raw_path)
    if not workspace_path.is_dir():
        raise HTTPException(
            status_code=status.HTTP_400_BAD_REQUEST,
            detail={
                "code": AUTH_WORKSPACE_INVALID,
                "message": "workspace path does not exist or is not a directory",
            },
        )

    audit_logger = factory()

    # Scenario invariant validation against audit log history.
    workspace_id = workspace_id_for_path(workspace_path)
    last_grant = find_last_grant_for_workspace(workspace_id, audit_logger.path)
    try:
        validate_scenario_invariants(req, last_grant)
    except ValueError as exc:
        raise HTTPException(
            status_code=status.HTTP_400_BAD_REQUEST,
            detail={"code": AUTH_INVALID_REQUEST, "message": str(exc)},
        ) from exc

    session_id = fresh_session_id()
    granted_at = _iso_utc_now()

    audit_logger.write_grant_issued(
        session_id=session_id,
        workspace_id=workspace_id,
        workspace_type=req.workspace_type,
        workspace_source=req.workspace_source,
        scenario=req.scenario,
        scope=req.scope.model_dump(),
        sanitizer_rules_version=req.sanitizer_rules_version,
        user_ack=list(req.user_ack),
    )

    _session_dict[session_id] = {
        "workspace_id": workspace_id,
        "granted_at": granted_at,
    }

    return GrantResponse(
        session_id=session_id,
        workspace_id=workspace_id,
        granted_at=granted_at,
    )


@router.post("/deny", status_code=status.HTTP_204_NO_CONTENT)
def post_deny(req: DenyRequest, request: Request) -> None:
    factory = _get_factory(request)
    _reject_topic_mode(req.workspace_type)

    audit_logger = factory()
    audit_logger.write_grant_denied(
        session_id=fresh_session_id(),
        workspace_type=req.workspace_type,
        workspace_source=req.workspace_source,
        scenario=req.scenario,
        reason=req.reason,
    )


@router.post("/revoke", status_code=status.HTTP_204_NO_CONTENT)
def post_revoke(req: RevokeRequest, request: Request) -> None:
    factory = _get_factory(request)

    session = _session_dict.get(req.session_id)
    if session is None:
        raise HTTPException(
            status_code=status.HTTP_404_NOT_FOUND,
            detail={
                "code": AUTH_NO_ACTIVE_GRANT,
                "message": "no active grant matches the supplied session_id",
            },
        )

    audit_logger = factory()
    audit_logger.write_grant_revoked(
        session_id=req.session_id,
        workspace_id=session["workspace_id"],
        grant_ts=session["granted_at"],
        trigger=req.trigger,
    )
    del _session_dict[req.session_id]


@router.get("/status", response_model=AuthStatusResponse)
def get_status(workspace_id: str, request: Request) -> AuthStatusResponse:
    factory = _get_factory(request)
    audit_logger = factory()

    last_grant = find_last_grant_for_workspace(workspace_id, audit_logger.path)
    if last_grant is None:
        return AuthStatusResponse(
            has_active_grant=False,
            session_id=None,
            last_grant=None,
            current_rules_version=RULES_VERSION,
        )

    session_id = last_grant.get("session_id")
    has_active = (
        isinstance(session_id, str) and session_id in _session_dict
    )
    return AuthStatusResponse(
        has_active_grant=bool(has_active),
        session_id=session_id if has_active else None,
        last_grant=last_grant,
        current_rules_version=RULES_VERSION,
    )


__all__ = ["router"]
