"""Drift guard: `rules_version` is sourced from a single module-level constant.

Backs spec MODIFIED Requirement
``Rules version is recorded on every audit line`` Scenario
``Single source of truth for rules_version constant`` (review-backlog-cleanup).

Invariant 9 (`CLAUDE.md`): Sanitizer rule changes MUST bump version. The
audit-chain integrity depends on every writer stamping the same string.
Three independent literals across `sanitizer/config.py` /
`api/__init__.py` / `api/scan.py` opened a drift surface — bump one,
forget the others, and `sanitize_audit.jsonl` rows would mix versions.

This test pins the post-cleanup invariant: every callsite resolves the
same Python string object (identity check via ``is``), so renaming /
bumping the canonical constant cannot silently leave a callsite behind.
"""
from __future__ import annotations

from codebus_agent import sanitizer as _sanitizer_pkg
from codebus_agent.agent.tools.folder_tools import (
    _SANITIZE_RULES_VERSION as _folder_tools_rules_version,
)
from codebus_agent.api import _RULES_VERSION as _api_init_rules_version
from codebus_agent.api.scan import _RULES_VERSION as _api_scan_rules_version
from codebus_agent.generator.runner import (
    _DEFAULT_RULES_VERSION as _generator_rules_version,
)
from codebus_agent.sanitizer import RULES_VERSION
from codebus_agent.sanitizer.config import (
    _BUILTIN_RULES_VERSION,
    RULES_VERSION as _config_rules_version,
)


def test_rules_version_constant_has_single_source() -> None:
    # Canonical constant exists at the package boundary.
    assert isinstance(RULES_VERSION, str)
    assert RULES_VERSION  # non-empty

    # Re-export and config-module symbol resolve to the same object.
    assert _sanitizer_pkg.RULES_VERSION is RULES_VERSION
    assert _config_rules_version is RULES_VERSION

    # Backward-compat alias still in place and pointing at the same object.
    assert _BUILTIN_RULES_VERSION is RULES_VERSION

    # Every production callsite import-aliases the same object — not separate
    # string literals. Identity (`is`) catches drift even if literal values
    # match. Adding new callsites? Pin them here so future bumps cannot leave
    # one behind.
    assert _api_init_rules_version is RULES_VERSION
    assert _api_scan_rules_version is RULES_VERSION
    assert _generator_rules_version is RULES_VERSION
    assert _folder_tools_rules_version is RULES_VERSION
