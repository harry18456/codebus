"""Tests for `agent.prompts.qa` and `agent/qa.py` import-graph isolation.

Backs SHALL clauses in
openspec/changes/module-8-qa-p0/specs/qa-agent/spec.md
  Requirement: Q&A system prompt module is isolated from Explorer prompts
"""
from __future__ import annotations

import ast
import re
from pathlib import Path

import pytest


def test_qa_prompt_module_exposes_required_symbols() -> None:
    from codebus_agent.agent.prompts import qa as qa_prompts

    assert hasattr(qa_prompts, "QA_SYSTEM")
    assert hasattr(qa_prompts, "render_qa_prompt")
    assert hasattr(qa_prompts, "QA_PROMPT_VERSION")
    assert re.match(r"^\d{4}-\d{2}-\d{2}-\d+$", qa_prompts.QA_PROMPT_VERSION)


def _imports_in(path: Path) -> list[str]:
    tree = ast.parse(path.read_text(encoding="utf-8"))
    out: list[str] = []
    for node in ast.walk(tree):
        if isinstance(node, ast.ImportFrom) and node.module:
            out.append(node.module)
        elif isinstance(node, ast.Import):
            out.extend(alias.name for alias in node.names)
    return out


def _qa_prompt_path() -> Path:
    return (
        Path(__file__).resolve().parents[2]
        / "src"
        / "codebus_agent"
        / "agent"
        / "prompts"
        / "qa.py"
    )


def _qa_module_path() -> Path:
    return (
        Path(__file__).resolve().parents[2]
        / "src"
        / "codebus_agent"
        / "agent"
        / "qa.py"
    )


def test_qa_prompt_module_does_not_import_explorer_or_judge_or_coverage() -> None:
    forbidden = {
        "codebus_agent.agent.prompts.explorer",
        "codebus_agent.agent.prompts.judge",
        "codebus_agent.agent.prompts.coverage",
    }
    imports = _imports_in(_qa_prompt_path())
    leaks = [m for m in imports if m in forbidden]
    assert leaks == [], f"prompts/qa.py leaks imports: {leaks}"


def test_qa_module_does_not_import_explorer_or_judge_or_coverage_prompts() -> None:
    qa_path = _qa_module_path()
    if not qa_path.exists():
        pytest.skip("agent/qa.py is implemented in section 8 GREEN")
    forbidden = {
        "codebus_agent.agent.prompts.explorer",
        "codebus_agent.agent.prompts.judge",
        "codebus_agent.agent.prompts.coverage",
    }
    imports = _imports_in(qa_path)
    leaks = [m for m in imports if m in forbidden]
    assert leaks == [], f"agent/qa.py leaks imports: {leaks}"


def test_system_prompt_contains_three_worth_persisting_rules() -> None:
    from codebus_agent.agent.prompts.qa import QA_SYSTEM

    # Three-condition substring (zh-TW per docs/qa-agent.md §五):
    assert "可復用" in QA_SYSTEM
    assert "stable fact" in QA_SYSTEM
    assert "非同義重複" in QA_SYSTEM
    # Station id format hint substring.
    assert r"^s\d{2}-" in QA_SYSTEM
