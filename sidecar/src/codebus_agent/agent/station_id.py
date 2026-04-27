"""Canonical single-source for the stable station id regex + validators.

Backs SHALL clauses in
openspec/changes/audit-path-unification-stage-2/specs/qa-agent/spec.md
  Requirement: add_to_kb pipeline runs sanitize, validate, upsert, growth-log in order
    Scenario: Station id regex sourced from canonical leaf module
openspec/changes/audit-path-unification-stage-2/specs/kb-growth/spec.md
  Requirement: Required fields on every kb_growth.jsonl line
    Scenario: Station id regex sourced from canonical leaf module
openspec/changes/audit-path-unification-stage-2/specs/knowledge-base/spec.md
  Requirement: KnowledgeBase query and find_similar API
    Scenario: Station id regex sourced from canonical leaf module

This is a leaf module — only `re` from stdlib — so the six station-id
validating callsites (`agent.tools.add_to_kb`, `agent.tools.kb_search`,
`kb.growth_logger`, `kb.knowledge_base`, `kb.payload`, `api.qa`) can
import it without triggering import cycles. Mirrors the
`codebus_agent.sanitizer.RULES_VERSION` single-constant pattern that
`review-backlog-cleanup` (2026-04-25 archive) established for the
`rules_version` audit field.

The defensive identity-check test
``sidecar/tests/agent/test_station_id_constant.py`` rejects any drift
where a callsite redeclares its own `re.compile(...)` instead of
importing `STATION_ID_RE` from this module.
"""
from __future__ import annotations

import re

__all__ = ["STATION_ID_RE", "validate_station_id", "find_invalid_station_id"]


STATION_ID_RE: re.Pattern[str] = re.compile(r"^s\d{2}-[a-z0-9-]{1,40}(-\d+)?$")
_STATION_ID_RE = STATION_ID_RE  # backward-compat alias for in-package callsites


def validate_station_id(sid: str) -> None:
    """Raise ``ValueError`` when ``sid`` violates the stable station id regex."""
    if not isinstance(sid, str) or not STATION_ID_RE.fullmatch(sid):
        raise ValueError(f"invalid station_id: {sid}")


def find_invalid_station_id(ids: list[str]) -> str | None:
    """Return the first id failing the stable station id regex, or ``None``."""
    for sid in ids:
        if not isinstance(sid, str) or not STATION_ID_RE.fullmatch(sid):
            return sid
    return None
