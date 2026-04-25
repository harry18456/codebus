"""Workspace-level audit JSONL path constants — leaf module to avoid circular imports.

`audit-path-unification` collected the five workspace-level audit JSONL
filenames (sanitize_audit / tool_audit / token_usage / llm_calls /
reasoning_log) and the shared `.codebus/` subdirectory name into named
constants so callers do not sprinkle magic strings.

This module is imported by both `api/__init__.py` (which builds the
factory wiring for KB / chat-ish providers) and `api/explore.py` (which
constructs ReasoningLogger directly), so it MUST stay a leaf — it MUST
NOT import from `api/__init__.py` or anywhere downstream.
"""
from __future__ import annotations


__all__ = [
    "_LLM_CALLS_FILENAME",
    "_REASONING_LOG_FILENAME",
    "_SANITIZE_AUDIT_FILENAME",
    "_TOKEN_USAGE_FILENAME",
    "_TOOL_AUDIT_FILENAME",
    "_WORKSPACE_AUDIT_SUBDIR",
]


_WORKSPACE_AUDIT_SUBDIR = ".codebus"

# Five workspace-level audit JSONL filenames. All resolve under
# `<workspace_root>/.codebus/`. App-level `authorization_audit.jsonl`
# (under `~/.codebus/`) lives in a future capability and is intentionally
# NOT listed here.
_SANITIZE_AUDIT_FILENAME = "sanitize_audit.jsonl"
_TOOL_AUDIT_FILENAME = "tool_audit.jsonl"
_TOKEN_USAGE_FILENAME = "token_usage.jsonl"
_LLM_CALLS_FILENAME = "llm_calls.jsonl"
_REASONING_LOG_FILENAME = "reasoning_log.jsonl"
