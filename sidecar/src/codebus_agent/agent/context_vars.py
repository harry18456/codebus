"""Agent-scope context variables for SSE event enrichment.

Backs SHALL clauses in
openspec/changes/agent-sse-wiring/specs/explorer-sse/spec.md
  Requirement: TrackedProvider emits usage_delta on every completed call
    (phase / step / session fields come from these ContextVars.)

The Explorer loop sets `current_phase_var` / `current_step_var` once per
iteration so every downstream LLM call `TrackedProvider` makes picks up
the scope automatically, without threading the values through every call
site. When the vars are unset (e.g. KB build path), event payload carries
JSON `null` for the corresponding fields.

Three `ContextVar`s ship here (with module-level getter helpers that
dereference `None`-default into `None` for call sites):

- `current_phase_var` — the pipeline phase (`"explore"`, `"kb_build"`,
  `"kb_query"`, `"qa"`, ...). Kept as a loose string so new phases don't
  need a schema change.
- `current_step_var` — the iteration number inside the current phase
  (Explorer loop sets this to `state.step_count` at iteration start).
- `current_session_var` — an opaque per-session id (reserved for
  cross-cut audit correlation; Q&A / multi-session plumbing will set it).
"""
from __future__ import annotations

from contextvars import ContextVar


__all__ = [
    "current_phase_var",
    "current_step_var",
    "current_session_var",
    "current_phase",
    "current_step",
    "current_session",
]


current_phase_var: ContextVar[str | None] = ContextVar("current_phase", default=None)
current_step_var: ContextVar[int | None] = ContextVar("current_step", default=None)
current_session_var: ContextVar[str | None] = ContextVar(
    "current_session", default=None
)


def current_phase() -> str | None:
    """Return the active phase label, or `None` when unset."""
    return current_phase_var.get()


def current_step() -> int | None:
    """Return the active iteration step, or `None` when unset."""
    return current_step_var.get()


def current_session() -> str | None:
    """Return the active session id, or `None` when unset."""
    return current_session_var.get()
