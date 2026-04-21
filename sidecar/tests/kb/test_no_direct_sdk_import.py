"""Guard: runtime code MUST route Qdrant SDK usage through codebus_agent.kb.qdrant_client.

Backs openspec/changes/qdrant-lifecycle-bootstrap/specs/qdrant-client/spec.md
  Requirement: Qdrant client wrapper module
    Scenario: Runtime code does not import qdrant-client SDK directly

Only files under ``codebus_agent/kb/`` are permitted to import the
third-party ``qdrant_client`` package; everywhere else must go through
the wrapper so SDK API churn has one blast radius.
"""
from __future__ import annotations

import ast
from pathlib import Path

import codebus_agent

SRC_ROOT = Path(codebus_agent.__file__).resolve().parent
KB_DIR = SRC_ROOT / "kb"


def _runtime_py_files() -> list[Path]:
    return [
        p
        for p in SRC_ROOT.rglob("*.py")
        if KB_DIR not in p.parents and p != KB_DIR
    ]


def _imports_qdrant_client(path: Path) -> bool:
    tree = ast.parse(path.read_text(encoding="utf-8"), filename=str(path))
    for node in ast.walk(tree):
        if isinstance(node, ast.Import):
            for alias in node.names:
                if alias.name == "qdrant_client" or alias.name.startswith("qdrant_client."):
                    return True
        elif isinstance(node, ast.ImportFrom):
            module = node.module or ""
            if module == "qdrant_client" or module.startswith("qdrant_client."):
                return True
    return False


def test_no_direct_qdrant_client_import_outside_kb() -> None:
    offenders = [p for p in _runtime_py_files() if _imports_qdrant_client(p)]
    assert offenders == [], (
        "These runtime modules import qdrant_client directly — route them "
        "through codebus_agent.kb.qdrant_client instead: "
        + ", ".join(str(p.relative_to(SRC_ROOT)) for p in offenders)
    )


def test_guard_itself_would_catch_a_violation(tmp_path: Path) -> None:
    """Self-test: the AST walker must flag a synthetic violation.

    Prevents the guard from silently drifting into a no-op (e.g. if the
    walker ever stops matching ``ImportFrom``).
    """
    fake = tmp_path / "violator.py"
    fake.write_text("from qdrant_client import AsyncQdrantClient\n")
    assert _imports_qdrant_client(fake) is True
