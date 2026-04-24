"""RED tests for ToolContext's new optional kb / usage_tracker fields.

Backs SHALL clauses in
openspec/changes/explorer-tools-p0/specs/tool-sandbox/spec.md
  Requirement: ToolContext carries workspace type discriminator (modified —
    adds optional kb / usage_tracker fields)

The additive nature is load-bearing: every existing sandbox / red-team
fixture constructs ``ToolContext`` without these fields and MUST keep
compiling unchanged.
"""
from __future__ import annotations

from pathlib import Path

import pytest
from pydantic import ValidationError


def test_kb_and_usage_tracker_default_to_none(tmp_path: Path) -> None:
    """Constructing ToolContext without new deps MUST leave them None."""
    from codebus_agent.sandbox import ToolContext

    ctx = ToolContext(workspace_root=tmp_path, workspace_type="folder")

    assert ctx.kb is None
    assert ctx.usage_tracker is None
    # Existing fields keep their defaults
    assert ctx.workspace_id == ""
    assert ctx.session_id == ""
    assert ctx.sanitizer is None


def test_kb_and_usage_tracker_accept_typed_instances(tmp_path: Path) -> None:
    """Wiring a KnowledgeBase / UsageTracker MUST validate AND stay frozen."""
    from codebus_agent.kb.knowledge_base import KnowledgeBase
    from codebus_agent.providers.usage_tracker import UsageTracker
    from codebus_agent.sandbox import ToolContext

    # Minimal kwargs — the integration wiring isn't under test here, just
    # that the dep slots accept typed instances without ValidationError.
    # We use object() stand-ins typed at the annotation level to prove the
    # schema's not secretly validating internals; `arbitrary_types_allowed`
    # lets us do this.
    fake_kb = object.__new__(KnowledgeBase)  # bypass __init__ side effects
    fake_tracker = UsageTracker(tmp_path / "token_usage.jsonl")

    ctx = ToolContext(
        workspace_root=tmp_path,
        workspace_type="folder",
        kb=fake_kb,
        usage_tracker=fake_tracker,
    )
    assert ctx.kb is fake_kb
    assert ctx.usage_tracker is fake_tracker

    # frozen=True still enforced — mutating must fail
    with pytest.raises(ValidationError):
        ctx.kb = None  # type: ignore[misc]
