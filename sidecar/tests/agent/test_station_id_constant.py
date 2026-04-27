"""Drift guard: stable station id regex is sourced from a single canonical leaf module.

Backs spec MODIFIED Scenarios in audit-path-unification-stage-2:
  qa-agent: ``Station id regex sourced from canonical leaf module`` (×2)
  kb-growth: ``Station id regex sourced from canonical leaf module``
  knowledge-base: ``Station id regex sourced from canonical leaf module``

Six callsites pre-validate stable station ids against
``^s\\d{2}-[a-z0-9-]{1,40}(-\\d+)?$`` before disk / Qdrant writes:
``agent.tools.add_to_kb`` / ``agent.tools.kb_search`` /
``kb.growth_logger`` / ``kb.knowledge_base`` / ``kb.payload`` /
``api.qa``. They MUST all reference the same ``re.Pattern`` object
exposed by ``codebus_agent.agent.station_id``. Identity (``is``) check
catches the drift even when six independent ``re.compile(r"...")``
calls happen to produce identical pattern strings.

Mirrors the ``test_rules_version_constant.py`` pattern that
``review-backlog-cleanup`` (2026-04-25 archive) established for the
``rules_version`` audit field.
"""
from __future__ import annotations

import inspect
import re
from pathlib import Path

from codebus_agent.agent.station_id import (
    STATION_ID_RE,
    _STATION_ID_RE,
    find_invalid_station_id,
    validate_station_id,
)
from codebus_agent.agent.tools.add_to_kb import _STATION_ID_RE as _add_to_kb_re
from codebus_agent.agent.tools.kb_search import _STATION_ID_RE as _kb_search_re
from codebus_agent.api.qa import _STATION_ID_RE as _api_qa_re
from codebus_agent.kb.growth_logger import _STATION_ID_RE as _growth_logger_re
from codebus_agent.kb.knowledge_base import _STATION_ID_RE as _knowledge_base_re
from codebus_agent.kb.payload import _STATION_ID_RE as _payload_re


def test_station_id_re_single_source() -> None:
    """Canonical pattern + 6 callsite import aliases MUST all be the same object."""
    # Canonical pattern shape — sanity check on the regex itself.
    assert isinstance(STATION_ID_RE, re.Pattern)
    assert STATION_ID_RE.pattern == r"^s\d{2}-[a-z0-9-]{1,40}(-\d+)?$"

    # Backward-compat private alias points at the same object.
    assert _STATION_ID_RE is STATION_ID_RE

    # Six production callsites import-alias the same `re.Pattern` object.
    # Identity (`is`) catches drift even if six independent `re.compile(...)`
    # produce structurally equal but distinct Pattern objects.
    assert _add_to_kb_re is STATION_ID_RE
    assert _kb_search_re is STATION_ID_RE
    assert _growth_logger_re is STATION_ID_RE
    assert _knowledge_base_re is STATION_ID_RE
    assert _payload_re is STATION_ID_RE
    assert _api_qa_re is STATION_ID_RE


def test_no_station_id_regex_compile_outside_canonical() -> None:
    """No module other than ``agent/station_id.py`` MAY hold its own
    ``re.compile(r"^s\\d{2}-...")`` literal. Source-level grep catches
    drift even if the duplicated regex never reaches an import.
    """
    package_root = Path(
        inspect.getsourcefile(__import__("codebus_agent")) or ""
    ).parent
    assert package_root.exists()

    # Match `re.compile(r"^s\d{2}-..."`-style literals — also accept the bare
    # raw-string opener `r"^s\d{2}-` so we catch the api/qa string variant.
    needle = re.compile(r"""r['\"]\^s\\d\{2\}-""")
    canonical = (package_root / "agent" / "station_id.py").resolve()

    offending: list[Path] = []
    for py_file in package_root.rglob("*.py"):
        resolved = py_file.resolve()
        if resolved == canonical:
            continue
        text = resolved.read_text(encoding="utf-8")
        if needle.search(text):
            offending.append(resolved.relative_to(package_root))

    assert offending == [], (
        "Found station-id regex literals outside the canonical leaf module: "
        f"{offending}. Import `_STATION_ID_RE` from "
        "`codebus_agent.agent.station_id` instead."
    )


def test_validate_station_id_helpers() -> None:
    """`validate_station_id` raises on bad input; `find_invalid_station_id`
    returns the first offender (or None for all-clean lists).
    """
    # Happy path — exact regex match shape.
    validate_station_id("s02-storage")  # MUST NOT raise

    # Sad path — single-digit segment violates the regex.
    try:
        validate_station_id("bad-id")
    except ValueError as exc:
        assert "bad-id" in str(exc)
    else:
        raise AssertionError("validate_station_id MUST raise on `bad-id`")

    # `find_invalid_station_id` reports the first offender; later good ids ignored.
    assert find_invalid_station_id(["s02-x", "bad", "s03-y"]) == "bad"

    # All-clean list returns None.
    assert find_invalid_station_id(["s02-x"]) is None
