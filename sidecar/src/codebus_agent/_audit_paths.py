"""Workspace-level audit JSONL path constants — package-root leaf module.

`audit-path-unification` collected the workspace-level audit JSONL
filenames (sanitize_audit / tool_audit / token_usage / llm_calls /
reasoning_log) and the shared `.codebus/` subdirectory name into named
constants so callers do not sprinkle magic strings.
`module-5-generator-p0` added `generator_log.jsonl` (Module 5 per-Module
operational log; not a seventh audit chain layer — see
`docs/reviews/2026-04-25-stage-4.md`).

Hosted at the package root (rather than ``codebus_agent.api._audit_paths``
where it originally landed) so non-API callers — e.g.,
``codebus_agent.generator.runner`` — can import without triggering
``codebus_agent.api.__init__`` execution and creating an import cycle
with ``codebus_agent.api.generate``. ``codebus_agent.api._audit_paths``
re-exports the same names for backward compatibility with the
audit-path-unification archive's call sites.
"""
from __future__ import annotations


__all__ = [
    "_GENERATOR_LOG_FILENAME",
    "_LLM_CALLS_FILENAME",
    "_REASONING_LOG_FILENAME",
    "_SANITIZE_AUDIT_FILENAME",
    "_TOKEN_USAGE_FILENAME",
    "_TOOL_AUDIT_FILENAME",
    "_WORKSPACE_AUDIT_SUBDIR",
]


_WORKSPACE_AUDIT_SUBDIR = ".codebus"

# Six workspace-level operational / audit JSONL filenames. All resolve
# under `<workspace_root>/.codebus/`. App-level `authorization_audit.jsonl`
# (under `~/.codebus/`) lives in a future capability and is intentionally
# NOT listed here.
#
# `generator_log.jsonl` is per-Module operational log (parallel to
# `reasoning_log.jsonl`), not part of the seven-layer audit chain — see
# `module-5-generator-p0` design risks.
_SANITIZE_AUDIT_FILENAME = "sanitize_audit.jsonl"
_TOOL_AUDIT_FILENAME = "tool_audit.jsonl"
_TOKEN_USAGE_FILENAME = "token_usage.jsonl"
_LLM_CALLS_FILENAME = "llm_calls.jsonl"
_REASONING_LOG_FILENAME = "reasoning_log.jsonl"
_GENERATOR_LOG_FILENAME = "generator_log.jsonl"
