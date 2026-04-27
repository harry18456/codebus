"""Drift guard: Q&A budget constants + dedup threshold are single-sourced in `agent.qa`.

Backs spec MODIFIED Scenarios in audit-path-unification-stage-2:
  qa-agent: ``All callsites import constants from agent.qa single source``
  knowledge-base: ``Dedup threshold sourced from canonical single source``

Five constants live in ``codebus_agent.agent.qa`` as the canonical
home (per spec ``Q&A budget constants are module-level``):
``_QA_MAX_STEPS`` / ``_QA_MAX_ADD_TO_KB_PER_SESSION`` /
``_QA_MAX_CHUNK_SIZE_CHARS`` / ``_QA_MAX_ADD_TO_KB_PER_QUESTION`` /
``_QA_DEDUP_THRESHOLD``. Two downstream callsites previously
redeclared their own copies (``agent.tools.add_to_kb`` for the three
pipeline budgets, ``kb.knowledge_base`` for the dedup threshold) —
this test pins them to the canonical objects via ``is`` identity so a
future bump cannot leave one behind.

Mirrors the ``test_rules_version_constant.py`` /
``test_station_id_constant.py`` patterns.
"""
from __future__ import annotations

import inspect
import re
from pathlib import Path

from codebus_agent.agent import qa as _qa_module
from codebus_agent.agent.qa import (
    _QA_DEDUP_THRESHOLD,
    _QA_MAX_ADD_TO_KB_PER_QUESTION,
    _QA_MAX_ADD_TO_KB_PER_SESSION,
    _QA_MAX_CHUNK_SIZE_CHARS,
    _QA_MAX_STEPS,
)
from codebus_agent.agent.tools.add_to_kb import (
    _QA_MAX_ADD_TO_KB_PER_QUESTION as _add_to_kb_per_question,
    _QA_MAX_ADD_TO_KB_PER_SESSION as _add_to_kb_per_session,
    _QA_MAX_CHUNK_SIZE_CHARS as _add_to_kb_chunk_size,
)
from codebus_agent.kb.knowledge_base import (
    _QA_DEDUP_THRESHOLD as _kb_dedup_threshold,
)


def test_qa_budget_constants_single_source() -> None:
    """`add_to_kb`'s three pipeline budgets MUST be the same object as
    `agent.qa`'s canonical constants. Identity (`is`) catches drift
    even if literal numeric values match.
    """
    assert _add_to_kb_chunk_size is _QA_MAX_CHUNK_SIZE_CHARS
    assert _add_to_kb_per_session is _QA_MAX_ADD_TO_KB_PER_SESSION
    assert _add_to_kb_per_question is _QA_MAX_ADD_TO_KB_PER_QUESTION


def test_no_qa_max_definition_outside_canonical() -> None:
    """No module other than ``agent/qa.py`` MAY hold a line-anchored
    ``_QA_(MAX|DEDUP)_...`` definition (assignment with `=` or `:`).
    Re-imports at module top via ``from codebus_agent.agent.qa import _QA_*``
    are explicitly allowed (they don't match the line-anchored pattern).
    """
    package_root = Path(
        inspect.getsourcefile(__import__("codebus_agent")) or ""
    ).parent
    assert package_root.exists()

    # Line-anchored definition: `_QA_MAX_FOO = ...` or `_QA_DEDUP_BAR: type = ...`.
    # Excludes `from ... import _QA_MAX_*` (which is `_QA_` mid-line, not line-start).
    needle = re.compile(r"^_QA_(?:MAX|DEDUP)_\w+\s*[:=]", re.MULTILINE)
    canonical = (package_root / "agent" / "qa.py").resolve()

    offending: list[Path] = []
    for py_file in package_root.rglob("*.py"):
        resolved = py_file.resolve()
        if resolved == canonical:
            continue
        text = resolved.read_text(encoding="utf-8")
        if needle.search(text):
            offending.append(resolved.relative_to(package_root))

    assert offending == [], (
        "Found `_QA_(MAX|DEDUP)_*` definitions outside the canonical "
        f"`codebus_agent/agent/qa.py`: {offending}. "
        "Import from `codebus_agent.agent.qa` instead of redeclaring."
    )


def test_dedup_threshold_single_source() -> None:
    """`kb.knowledge_base._QA_DEDUP_THRESHOLD` MUST be the same object as
    `agent.qa._QA_DEDUP_THRESHOLD`. Identity check via `is`.
    """
    assert _kb_dedup_threshold is _QA_DEDUP_THRESHOLD
    # Sanity: canonical object is the float ``0.95`` per spec.
    assert _qa_module._QA_DEDUP_THRESHOLD == 0.95
