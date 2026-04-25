"""Backward-compat shim — re-exports ``codebus_agent._audit_paths`` constants.

The constants moved to the package root in ``module-5-generator-p0``
to break an import cycle: ``generator/runner.py`` needs the audit
path constants, but importing them from ``codebus_agent.api._audit_paths``
forced ``codebus_agent.api.__init__`` to load early — which in turn
imports ``codebus_agent.api.generate``, which imports the still-loading
``codebus_agent.generator``. Hosting the constants at the package root
means non-API callers can import them without triggering the API
package's __init__.

This shim preserves every symbol exposed by the original
``codebus_agent.api._audit_paths`` so prior call sites
(``api/__init__.py`` / ``api/explore.py``) keep working unchanged.
"""
from __future__ import annotations

from codebus_agent._audit_paths import (
    _GENERATOR_LOG_FILENAME,
    _LLM_CALLS_FILENAME,
    _REASONING_LOG_FILENAME,
    _SANITIZE_AUDIT_FILENAME,
    _TOKEN_USAGE_FILENAME,
    _TOOL_AUDIT_FILENAME,
    _WORKSPACE_AUDIT_SUBDIR,
)


__all__ = [
    "_GENERATOR_LOG_FILENAME",
    "_LLM_CALLS_FILENAME",
    "_REASONING_LOG_FILENAME",
    "_SANITIZE_AUDIT_FILENAME",
    "_TOKEN_USAGE_FILENAME",
    "_TOOL_AUDIT_FILENAME",
    "_WORKSPACE_AUDIT_SUBDIR",
]
