"""Sanitizer package — three-pass redaction pipeline.

Backs SHALL clauses in
openspec/changes/sanitizer-safety-chain/specs/sanitizer/spec.md

Pass 1: Scanner → KB
Pass 2: LLMProvider dispatch pre-flight (TrackedProvider)
Pass 3: Q&A add_to_kb (slot reserved; wired in a later change)

Placeholder format `<REDACTED:kind#index>` is one-way — no reverse
mapping is retained, per D-015.
"""
from __future__ import annotations

from .audit import SanitizerAuditLogger
from .config import PatternAllowlistEntry, SanitizerConfig
from .engine import (
    AuditEntry,
    FileSource,
    MessageSource,
    SanitizedResult,
    SanitizeSource,
    SanitizerEngine,
    SanitizerError,
)

__all__ = [
    "AuditEntry",
    "FileSource",
    "MessageSource",
    "PatternAllowlistEntry",
    "SanitizedResult",
    "SanitizeSource",
    "SanitizerAuditLogger",
    "SanitizerConfig",
    "SanitizerEngine",
    "SanitizerError",
]
