"""Auth service helpers — workspace_id derivation, session_id generation,
audit log lookup, and scenario invariant validation.

Backs SHALL clauses in
``openspec/changes/auth-flow/specs/authorization-audit/spec.md``:

  Requirement: Three-event audit schema with workspace_type discriminator
    Scenario: workspace_id is path-derived and stable
    Scenario: session_id is fresh UUIDv4 per grant
  Requirement: scope upgrade detection reads the latest grant from audit log

Pydantic schemas (``GrantRequest``, ``DenyRequest``, ``RevokeRequest``,
``GrantResponse``, ``AuthStatusResponse``) live alongside helpers so the
HTTP router (``api/auth.py``) imports a single module for all
auth-flow types.
"""
from __future__ import annotations

import hashlib
import json
import uuid
from pathlib import Path
from typing import Any, Literal

from pydantic import BaseModel, ConfigDict, Field

__all__ = [
    "AuthStatusResponse",
    "DenyRequest",
    "GrantRequest",
    "GrantResponse",
    "RevokeRequest",
    "Scope",
    "WorkspaceSourceFolder",
    "WorkspaceSourceTopic",
    "extract_acked_kinds",
    "find_last_grant_for_workspace",
    "fresh_session_id",
    "validate_scenario_invariants",
    "workspace_id_for_path",
]


# ---------------------------------------------------------------------------
# Identifier derivation
# ---------------------------------------------------------------------------


def workspace_id_for_path(path: Path) -> str:
    """Derive a stable, path-based workspace identifier.

    SHA-256 of the canonical lowercased POSIX path; prefix ``"ws_"`` and
    truncate to 12 hex characters → 15-char string total. Same path on
    same host yields same id across sidecar restarts (capability spec
    scenario "workspace_id is path-derived and stable").

    Lowercasing the canonical form gives case-insensitive equality on
    Windows (``C:/Projects/Timeline`` ≡ ``c:/projects/timeline``)
    without altering POSIX semantics — POSIX users using mixed-case
    paths intentionally accept the resulting collision (they are the
    same logical workspace).
    """
    canonical = Path(path).as_posix().lower()
    digest = hashlib.sha256(canonical.encode("utf-8")).hexdigest()
    return f"ws_{digest[:12]}"


def fresh_session_id() -> str:
    """Generate a fresh UUIDv4 session identifier (decimal canonical form).

    The session id MUST be decoupled from the bearer token (capability
    spec scenario "session_id is fresh UUIDv4 per grant"); the bearer
    is a transport secret and MUST NOT flow into audit logs.
    """
    return str(uuid.uuid4())


# ---------------------------------------------------------------------------
# Audit log inspection
# ---------------------------------------------------------------------------


def find_last_grant_for_workspace(
    workspace_id: str, audit_path: Path
) -> dict | None:
    """Return the most recent ``grant_issued`` entry for ``workspace_id``.

    Linear scan of the App-level authorization audit log. P0 does not
    maintain an in-memory cache (capability spec scenario "GET
    /auth/status reads audit log fresh on each call").
    """
    audit_path = Path(audit_path)
    if not audit_path.exists():
        return None

    last: dict | None = None
    with audit_path.open(encoding="utf-8") as fp:
        for raw in fp:
            raw = raw.strip()
            if not raw:
                continue
            try:
                entry = json.loads(raw)
            except json.JSONDecodeError:
                continue
            if (
                entry.get("event") == "grant_issued"
                and entry.get("workspace_id") == workspace_id
            ):
                last = entry
    return last


def extract_acked_kinds(grant_entry: dict) -> set[str]:
    """Extract the set of previously-acked sanitizer kinds from a grant.

    Reads ``grant_entry.user_ack`` and returns the suffixes of every
    flag starting with ``"new_kind:"``. Non-grant entries return the
    empty set so callers can pass any audit row defensively.
    """
    if not isinstance(grant_entry, dict):
        return set()
    if grant_entry.get("event") != "grant_issued":
        return set()
    user_ack = grant_entry.get("user_ack") or []
    return {
        flag.removeprefix("new_kind:")
        for flag in user_ack
        if isinstance(flag, str) and flag.startswith("new_kind:")
    }


# ---------------------------------------------------------------------------
# Pydantic schemas
# ---------------------------------------------------------------------------


class WorkspaceSourceFolder(BaseModel):
    model_config = ConfigDict(extra="forbid")

    path: str


class WorkspaceSourceTopic(BaseModel):
    """Phase 2 schema slot. Handler returns 501 in MVP (D-002)."""

    model_config = ConfigDict(extra="forbid")

    query: str
    seed_urls: list[str] = Field(default_factory=list)
    domain_allowlist: list[str] = Field(default_factory=list)


class Scope(BaseModel):
    model_config = ConfigDict(extra="forbid")

    llm_provider: str
    llm_model: str
    outbound_endpoint: str


class GrantRequest(BaseModel):
    model_config = ConfigDict(extra="forbid")

    workspace_type: Literal["folder", "topic"]
    workspace_source: dict[str, Any]
    scenario: Literal["first_run", "scope_reconfirm", "scope_upgrade_new_kind"]
    scope: Scope
    sanitizer_rules_version: str
    user_ack: list[str]


class GrantResponse(BaseModel):
    model_config = ConfigDict(extra="forbid")

    session_id: str
    workspace_id: str
    granted_at: str


class DenyRequest(BaseModel):
    model_config = ConfigDict(extra="forbid")

    workspace_type: Literal["folder", "topic"]
    workspace_source: dict[str, Any]
    scenario: Literal["first_run", "scope_reconfirm", "scope_upgrade_new_kind"]
    reason: Literal["user_cancelled", "app_closed"]


class RevokeRequest(BaseModel):
    model_config = ConfigDict(extra="forbid")

    session_id: str
    trigger: Literal["settings_revoke"]


class AuthStatusResponse(BaseModel):
    model_config = ConfigDict(extra="forbid")

    has_active_grant: bool
    session_id: str | None
    last_grant: dict[str, Any] | None
    current_rules_version: str


# ---------------------------------------------------------------------------
# Scenario invariant validation
# ---------------------------------------------------------------------------


def _new_kinds_in_request(req: GrantRequest) -> set[str]:
    return {
        flag.removeprefix("new_kind:")
        for flag in req.user_ack
        if isinstance(flag, str) and flag.startswith("new_kind:")
    }


def validate_scenario_invariants(
    req: GrantRequest, last_grant: dict | None
) -> None:
    """Enforce scenario ↔ audit-history invariants (raise ValueError on drift).

    Capability spec ``Requirement: scope upgrade detection reads the
    latest grant from audit log`` — three Scenarios:
    - first_run rejected when prior grant exists
    - scope_upgrade_new_kind on workspace with no prior grant rejected
    - scope_upgrade_new_kind requires non-empty diff
    Plus scope_reconfirm MUST NOT introduce a new kind (handled in
    "scope_reconfirm allowed when no new kinds introduced" by negation).
    """
    requested_new_kinds = _new_kinds_in_request(req)
    acked_kinds = extract_acked_kinds(last_grant) if last_grant else set()
    diff = requested_new_kinds - acked_kinds

    if req.scenario == "first_run":
        if last_grant is not None:
            raise ValueError(
                "scenario=first_run is forbidden when a prior grant_issued "
                "exists for this workspace"
            )
        return

    if req.scenario == "scope_upgrade_new_kind":
        if last_grant is None:
            raise ValueError(
                "scenario=scope_upgrade_new_kind requires a prior grant_issued"
            )
        if not diff:
            raise ValueError(
                "scenario=scope_upgrade_new_kind requires at least one new "
                "kind not present in the prior grant's user_ack"
            )
        return

    if req.scenario == "scope_reconfirm":
        if diff:
            raise ValueError(
                "scenario=scope_reconfirm MUST NOT introduce new kinds; "
                f"unacknowledged new kinds: {sorted(diff)}"
            )
        return
