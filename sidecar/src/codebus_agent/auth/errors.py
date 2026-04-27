"""Auth-flow HTTP error code constants — sync HTTP only.

Backs SHALL clauses in
``openspec/changes/auth-flow/specs/authorization-audit/spec.md``
(``Requirement: Four sync sidecar endpoints under bearer middleware``)
and ``openspec/changes/auth-flow/specs/sidecar-runtime/spec.md``
(``Requirement: Authorization endpoints registration``).

These four codes are **deliberately disjoint** from
``codebus_agent.api.tasks.ERROR_CODES`` (the SSE wire-error frozenset
defined by ``sidecar-runtime`` ``Background task error containment``).
The auth endpoints return synchronous HTTP responses with shape
``{"detail": {"code": "AUTH_*", "message": "<safe>"}}`` — they never
flow through the SSE task channel. Mixing the two error code spaces
breaks the "frozenset is the SSE channel's canonical error code set"
invariant and the drift guards that depend on it (design D-A11).

P0 closed set. A rules-version-mismatch code is reserved for the P1
follow-up that adds rules-version comparison logic; it is
intentionally NOT defined here yet (the constant name will be
introduced when the P1 change lands the comparison + meta.json
mechanism).
"""
from __future__ import annotations

__all__ = [
    "AUTH_INVALID_REQUEST",
    "AUTH_NOT_CONFIGURED",
    "AUTH_NO_ACTIVE_GRANT",
    "AUTH_WORKSPACE_INVALID",
]


AUTH_WORKSPACE_INVALID = "AUTH_WORKSPACE_INVALID"
AUTH_NO_ACTIVE_GRANT = "AUTH_NO_ACTIVE_GRANT"
AUTH_INVALID_REQUEST = "AUTH_INVALID_REQUEST"
AUTH_NOT_CONFIGURED = "AUTH_NOT_CONFIGURED"
