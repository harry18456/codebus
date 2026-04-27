"""Drift guard: workspace-audit ``*.jsonl`` filename literals are single-sourced.

Backs spec MODIFIED Scenario in audit-path-unification-stage-2:
  sanitizer: ``Filename literal is single-sourced in canonical leaf module``

`audit-path-unification` (2026-04-25 archive) collected the seven
workspace-level audit filenames into named constants in
``codebus_agent._audit_paths``. This test pins the post-cleanup
invariant: no other module in the package may ship a ``"*.jsonl"``
quoted-string literal — every callsite imports the canonical constant
instead. Source-level grep is the chosen mechanism (D-3 in design.md);
``ast.parse`` would also catch docstring / comment mentions which are
not actually drift surfaces.

Whitelist: ``codebus_agent/_audit_paths.py`` (the canonical leaf
module — the seven literal strings live there). All other modules in
``sidecar/src/codebus_agent/`` MUST import via the constants.
"""
from __future__ import annotations

import inspect
import re
from pathlib import Path

# Quoted-string literal of the form `"...jsonl"` or `'...jsonl'`.
# The surrounding regex is intentionally tight: only word-chars / `_` / `-`
# in the basename, then literal ``.jsonl`` then closing quote. This avoids
# false positives on docstring / comment mentions of `.jsonl` (which are
# typically unquoted or appear inside markdown code fences).
_JSONL_LITERAL_RE = re.compile(r"""['"][\w_-]+\.jsonl['"]""")


def _package_root() -> Path:
    return Path(inspect.getsourcefile(__import__("codebus_agent")) or "").parent


def test_jsonl_literal_only_in_canonical_module() -> None:
    """Source-level scan of ``sidecar/src/codebus_agent/`` MUST find
    ``"...jsonl"`` quoted-string literals only inside ``_audit_paths.py``.
    """
    package_root = _package_root()
    assert package_root.exists()

    canonical = (package_root / "_audit_paths.py").resolve()

    offending: dict[str, list[str]] = {}
    for py_file in package_root.rglob("*.py"):
        resolved = py_file.resolve()
        if resolved == canonical:
            continue
        text = resolved.read_text(encoding="utf-8")
        hits = _JSONL_LITERAL_RE.findall(text)
        if hits:
            offending[str(resolved.relative_to(package_root))] = hits

    assert offending == {}, (
        "Found `*.jsonl` quoted-string literals outside the canonical leaf "
        f"module `codebus_agent/_audit_paths.py`: {offending}. "
        "Import the corresponding `_<NAME>_FILENAME` constant from "
        "`codebus_agent._audit_paths` (or the `codebus_agent.api._audit_paths` "
        "shim) instead of writing the literal string."
    )


def test_seven_audit_filenames_present_in_canonical() -> None:
    """The seven workspace-audit filenames MUST all live in
    ``_audit_paths.py`` source. Sanity check that the whitelist target
    actually carries every literal we expect to be single-sourced there.
    """
    package_root = _package_root()
    canonical = package_root / "_audit_paths.py"
    text = canonical.read_text(encoding="utf-8")

    expected = {
        "sanitize_audit.jsonl",
        "tool_audit.jsonl",
        "token_usage.jsonl",
        "llm_calls.jsonl",
        "reasoning_log.jsonl",
        "generator_log.jsonl",
        "kb_growth.jsonl",
    }
    missing = {name for name in expected if f'"{name}"' not in text}
    assert missing == set(), (
        f"`_audit_paths.py` is missing literal(s) for: {sorted(missing)}. "
        "All seven workspace audit filenames MUST be defined here."
    )
