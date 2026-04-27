"""Defensive test — auth HTTP error codes MUST be disjoint from SSE ERROR_CODES.

Covers ``authorization-audit`` capability scenario:
- "Auth HTTP error codes disjoint from SSE wire-error codes"
And ``sidecar-runtime`` Modified Requirement scenario:
- "Auth error codes disjoint from SSE ERROR_CODES frozenset"

Design D-A11: sync HTTP error code constants live in
``codebus_agent.auth.errors``; the SSE wire-error frozenset stays
``api.tasks.ERROR_CODES`` (closed set of ten codes per
``Background task error containment``). Mixing the two invalidates the
"frozenset is the SSE channel's canonical error code set" invariant
and breaks downstream drift guards.
"""
from __future__ import annotations

from codebus_agent.api.tasks import ERROR_CODES
from codebus_agent.auth.errors import (
    AUTH_INVALID_REQUEST,
    AUTH_NO_ACTIVE_GRANT,
    AUTH_NOT_CONFIGURED,
    AUTH_WORKSPACE_INVALID,
)


def test_auth_codes_disjoint_from_sse_error_codes() -> None:
    auth_codes = {
        AUTH_WORKSPACE_INVALID,
        AUTH_NO_ACTIVE_GRANT,
        AUTH_INVALID_REQUEST,
        AUTH_NOT_CONFIGURED,
    }
    intersection = auth_codes & ERROR_CODES
    assert intersection == set(), (
        "Auth HTTP error codes MUST NOT appear in api.tasks.ERROR_CODES "
        f"(SSE-channel frozenset); offenders: {intersection}"
    )


def test_sse_error_codes_remain_exact_ten_elements() -> None:
    """Belt-and-braces: pin SSE ERROR_CODES size so adding an auth code
    later cannot silently slip past the disjoint test (the disjoint
    test passes if both sets are empty, e.g. import collapse — pin
    size to detect).
    """
    assert len(ERROR_CODES) == 10


def test_auth_codes_carry_string_constant_values() -> None:
    """The four constants MUST equal their own variable names verbatim
    (string self-identity). Catches accidental rename drift.
    """
    assert AUTH_WORKSPACE_INVALID == "AUTH_WORKSPACE_INVALID"
    assert AUTH_NO_ACTIVE_GRANT == "AUTH_NO_ACTIVE_GRANT"
    assert AUTH_INVALID_REQUEST == "AUTH_INVALID_REQUEST"
    assert AUTH_NOT_CONFIGURED == "AUTH_NOT_CONFIGURED"
