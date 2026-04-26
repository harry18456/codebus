"""Tests for `_KB_GROWTH_FILENAME` path constant + factory wiring.

Backs SHALL clauses in
openspec/changes/module-8-qa-p0/specs/kb-growth/spec.md
  Requirement: kb_growth.jsonl path constant lives alongside other audit filenames
  Requirement: kb_growth_logger_factory wired into app.state
"""
from __future__ import annotations

from pathlib import Path

import pytest


def test_kb_growth_filename_constant_exists() -> None:
    """Constant importable from leaf module with expected value."""
    from codebus_agent.api._audit_paths import _KB_GROWTH_FILENAME

    assert _KB_GROWTH_FILENAME == "kb_growth.jsonl"


def test_kb_growth_filename_constant_exists_root_module() -> None:
    """Same constant importable from package-root leaf module."""
    from codebus_agent._audit_paths import _KB_GROWTH_FILENAME as root_const

    assert root_const == "kb_growth.jsonl"


def test_no_literal_kb_growth_jsonl_outside_leaf() -> None:
    """Grep `sidecar/src/codebus_agent/` for literal `"kb_growth.jsonl"`.

    The only writing-site mention MUST live in `_audit_paths.py`.
    Any other source file mentioning the literal indicates path-string
    duplication that breaks the single-source-of-truth Requirement.
    """
    src_root = Path(__file__).resolve().parents[2] / "src" / "codebus_agent"
    assert src_root.is_dir(), f"unexpected layout: {src_root}"
    target = '"kb_growth.jsonl"'
    offenders: list[str] = []
    for py in src_root.rglob("*.py"):
        if "_audit_paths.py" in py.as_posix().split("/"):
            continue
        text = py.read_text(encoding="utf-8")
        if target in text:
            offenders.append(str(py.relative_to(src_root)))
    assert offenders == [], (
        f"literal {target} appears outside _audit_paths.py: {offenders}"
    )


def test_factory_kb_growth_logger_lands_under_codebus(tmp_path: Path) -> None:
    """`wire_kb_dependencies` factory MUST resolve to `<ws>/.codebus/kb_growth.jsonl`."""
    from fastapi import FastAPI

    from codebus_agent.api import wire_kb_dependencies

    app = FastAPI()
    wire_kb_dependencies(app, openai_api_key="sk-fake-key", qdrant_url=None)

    factory = getattr(app.state, "kb_growth_logger_factory", None)
    assert factory is not None and callable(factory)

    workspace = tmp_path / "ws"
    workspace.mkdir()
    logger = factory(workspace)
    expected = workspace / ".codebus" / "kb_growth.jsonl"
    assert getattr(logger, "path", None) == expected


def test_factory_kb_growth_logger_none_without_openai() -> None:
    """No openai key → factory MUST be None (mirrors other KB factories)."""
    from fastapi import FastAPI

    from codebus_agent.api import wire_kb_dependencies

    app = FastAPI()
    wire_kb_dependencies(app, openai_api_key=None, qdrant_url=None)

    assert app.state.kb_growth_logger_factory is None
