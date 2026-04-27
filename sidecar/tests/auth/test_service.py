"""Auth service helpers — workspace_id derivation, session_id generation,
audit log lookup, and scenario invariant validation.

Covers ``authorization-audit`` capability scenarios:
- "workspace_id is path-derived and stable"
- "session_id is fresh UUIDv4 per grant"
- "scope upgrade detection reads the latest grant from audit log"
- first_run / scope_reconfirm / scope_upgrade_new_kind validation
"""
from __future__ import annotations

import json
import uuid
from pathlib import Path

import pytest

from codebus_agent.auth.service import (
    fresh_session_id,
    workspace_id_for_path,
)


def test_workspace_id_stable_across_calls(tmp_path: Path) -> None:
    workspace = tmp_path / "projects" / "timeline"
    workspace.mkdir(parents=True)
    a = workspace_id_for_path(workspace)
    b = workspace_id_for_path(workspace)
    assert a == b


def test_workspace_id_format_15_chars_starts_with_ws(tmp_path: Path) -> None:
    workspace = tmp_path / "projects" / "timeline"
    workspace.mkdir(parents=True)
    wid = workspace_id_for_path(workspace)
    assert wid.startswith("ws_")
    assert len(wid) == 15
    suffix = wid.removeprefix("ws_")
    assert len(suffix) == 12
    int(suffix, 16)  # MUST be valid hex; raises if not


def test_workspace_id_case_insensitive(tmp_path: Path) -> None:
    """Resolve same path with different casing on platforms where path
    canonicalisation is case-insensitive (Windows). On case-sensitive
    POSIX systems we simulate via the lowercased canonical form
    contract — both inputs hash through the same SHA-256 input.
    """
    upper = Path("C:/Projects/Timeline")
    lower = Path("c:/projects/timeline")
    assert workspace_id_for_path(upper) == workspace_id_for_path(lower)


def test_workspace_id_different_paths_differ(tmp_path: Path) -> None:
    a = workspace_id_for_path(tmp_path / "alpha")
    b = workspace_id_for_path(tmp_path / "beta")
    assert a != b


def test_fresh_session_id_is_uuid4() -> None:
    sid = fresh_session_id()
    parsed = uuid.UUID(sid)
    assert parsed.version == 4


def test_two_session_ids_differ() -> None:
    a = fresh_session_id()
    b = fresh_session_id()
    assert a != b


# --- audit log lookup + scenario invariants (added in task 7.x) -----


def _write_audit_lines(path: Path, *entries: dict) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open("a", encoding="utf-8") as fp:
        for entry in entries:
            fp.write(json.dumps(entry) + "\n")


def test_find_last_grant_returns_none_for_empty_log(tmp_path: Path) -> None:
    from codebus_agent.auth.service import find_last_grant_for_workspace

    audit_path = tmp_path / "authorization_audit.jsonl"
    assert find_last_grant_for_workspace("ws_nonexistent", audit_path) is None


def test_find_last_grant_returns_none_when_path_missing(tmp_path: Path) -> None:
    from codebus_agent.auth.service import find_last_grant_for_workspace

    audit_path = tmp_path / "does" / "not" / "exist.jsonl"
    assert find_last_grant_for_workspace("ws_x", audit_path) is None


def test_find_last_grant_returns_latest_match(tmp_path: Path) -> None:
    from codebus_agent.auth.service import find_last_grant_for_workspace

    audit_path = tmp_path / "authorization_audit.jsonl"
    _write_audit_lines(
        audit_path,
        {"event": "grant_issued", "workspace_id": "ws_a", "ts": "2026-04-19T10:00:00Z", "marker": 1},
        {"event": "grant_denied", "workspace_id": "ws_a", "ts": "2026-04-19T10:01:00Z"},
        {"event": "grant_issued", "workspace_id": "ws_b", "ts": "2026-04-19T10:02:00Z", "marker": 2},
        {"event": "grant_issued", "workspace_id": "ws_a", "ts": "2026-04-19T10:03:00Z", "marker": 3},
    )

    last_a = find_last_grant_for_workspace("ws_a", audit_path)
    assert last_a is not None
    assert last_a["marker"] == 3

    last_b = find_last_grant_for_workspace("ws_b", audit_path)
    assert last_b is not None
    assert last_b["marker"] == 2


def test_extract_acked_kinds_strips_prefix() -> None:
    from codebus_agent.auth.service import extract_acked_kinds

    grant = {
        "event": "grant_issued",
        "user_ack": [
            "raw_stays_local",
            "no_kb_persist",
            "outbound_to_anthropic",
            "new_kind:secret",
            "new_kind:email",
        ],
    }
    assert extract_acked_kinds(grant) == {"secret", "email"}


def test_extract_acked_kinds_empty_for_non_grant_entry() -> None:
    from codebus_agent.auth.service import extract_acked_kinds

    assert extract_acked_kinds({"event": "grant_denied"}) == set()
    assert extract_acked_kinds({}) == set()


def test_validate_first_run_with_prior_raises() -> None:
    from codebus_agent.auth.service import (
        GrantRequest,
        validate_scenario_invariants,
    )

    request = GrantRequest(
        workspace_type="folder",
        workspace_source={"path": "C:/projects/x"},
        scenario="first_run",
        scope={
            "llm_provider": "anthropic",
            "llm_model": "claude-haiku-4.5",
            "outbound_endpoint": "api.anthropic.com",
        },
        sanitizer_rules_version="2026-04-20-1",
        user_ack=["raw_stays_local", "no_kb_persist", "outbound_to_anthropic"],
    )
    last_grant = {"event": "grant_issued", "user_ack": []}

    with pytest.raises(ValueError):
        validate_scenario_invariants(request, last_grant)


def test_validate_scope_upgrade_without_prior_raises() -> None:
    from codebus_agent.auth.service import (
        GrantRequest,
        validate_scenario_invariants,
    )

    request = GrantRequest(
        workspace_type="folder",
        workspace_source={"path": "C:/projects/x"},
        scenario="scope_upgrade_new_kind",
        scope={
            "llm_provider": "anthropic",
            "llm_model": "claude-haiku-4.5",
            "outbound_endpoint": "api.anthropic.com",
        },
        sanitizer_rules_version="2026-04-20-1",
        user_ack=[
            "raw_stays_local",
            "no_kb_persist",
            "outbound_to_anthropic",
            "new_kind:secret",
        ],
    )
    with pytest.raises(ValueError):
        validate_scenario_invariants(request, last_grant=None)


def test_validate_scope_upgrade_no_diff_raises() -> None:
    from codebus_agent.auth.service import (
        GrantRequest,
        validate_scenario_invariants,
    )

    request = GrantRequest(
        workspace_type="folder",
        workspace_source={"path": "C:/projects/x"},
        scenario="scope_upgrade_new_kind",
        scope={
            "llm_provider": "anthropic",
            "llm_model": "claude-haiku-4.5",
            "outbound_endpoint": "api.anthropic.com",
        },
        sanitizer_rules_version="2026-04-20-1",
        user_ack=[
            "raw_stays_local",
            "no_kb_persist",
            "outbound_to_anthropic",
            "new_kind:secret",
        ],
    )
    last_grant = {
        "event": "grant_issued",
        "user_ack": [
            "raw_stays_local",
            "no_kb_persist",
            "outbound_to_anthropic",
            "new_kind:secret",
        ],
    }
    with pytest.raises(ValueError):
        validate_scenario_invariants(request, last_grant)


def test_validate_scope_reconfirm_introducing_new_kind_raises() -> None:
    from codebus_agent.auth.service import (
        GrantRequest,
        validate_scenario_invariants,
    )

    request = GrantRequest(
        workspace_type="folder",
        workspace_source={"path": "C:/projects/x"},
        scenario="scope_reconfirm",
        scope={
            "llm_provider": "anthropic",
            "llm_model": "claude-haiku-4.5",
            "outbound_endpoint": "api.anthropic.com",
        },
        sanitizer_rules_version="2026-04-20-1",
        user_ack=[
            "raw_stays_local",
            "no_kb_persist",
            "outbound_to_anthropic",
            "new_kind:secret",
        ],
    )
    last_grant = {"event": "grant_issued", "user_ack": []}
    with pytest.raises(ValueError):
        validate_scenario_invariants(request, last_grant)


def test_validate_scope_reconfirm_subset_passes() -> None:
    from codebus_agent.auth.service import (
        GrantRequest,
        validate_scenario_invariants,
    )

    request = GrantRequest(
        workspace_type="folder",
        workspace_source={"path": "C:/projects/x"},
        scenario="scope_reconfirm",
        scope={
            "llm_provider": "anthropic",
            "llm_model": "claude-haiku-4.5",
            "outbound_endpoint": "api.anthropic.com",
        },
        sanitizer_rules_version="2026-04-20-1",
        user_ack=["raw_stays_local", "no_kb_persist", "outbound_to_anthropic"],
    )
    last_grant = {
        "event": "grant_issued",
        "user_ack": [
            "raw_stays_local",
            "no_kb_persist",
            "outbound_to_anthropic",
            "new_kind:email",
        ],
    }
    # MUST NOT raise
    validate_scenario_invariants(request, last_grant)


def test_validate_first_run_passes_with_no_prior() -> None:
    from codebus_agent.auth.service import (
        GrantRequest,
        validate_scenario_invariants,
    )

    request = GrantRequest(
        workspace_type="folder",
        workspace_source={"path": "C:/projects/x"},
        scenario="first_run",
        scope={
            "llm_provider": "anthropic",
            "llm_model": "claude-haiku-4.5",
            "outbound_endpoint": "api.anthropic.com",
        },
        sanitizer_rules_version="2026-04-20-1",
        user_ack=["raw_stays_local", "no_kb_persist", "outbound_to_anthropic"],
    )
    validate_scenario_invariants(request, last_grant=None)


def test_validate_scope_upgrade_with_diff_passes() -> None:
    from codebus_agent.auth.service import (
        GrantRequest,
        validate_scenario_invariants,
    )

    request = GrantRequest(
        workspace_type="folder",
        workspace_source={"path": "C:/projects/x"},
        scenario="scope_upgrade_new_kind",
        scope={
            "llm_provider": "anthropic",
            "llm_model": "claude-haiku-4.5",
            "outbound_endpoint": "api.anthropic.com",
        },
        sanitizer_rules_version="2026-04-20-1",
        user_ack=[
            "raw_stays_local",
            "no_kb_persist",
            "outbound_to_anthropic",
            "new_kind:secret",
            "new_kind:email",
        ],
    )
    last_grant = {
        "event": "grant_issued",
        "user_ack": [
            "raw_stays_local",
            "no_kb_persist",
            "outbound_to_anthropic",
            "new_kind:email",
        ],
    }
    validate_scenario_invariants(request, last_grant)
