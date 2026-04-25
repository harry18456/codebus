"""Token budget probes for the Explorer ReAct loop.

Backs SHALL clauses in
openspec/changes/context-compression-token-budget/design.md
  Decision 2: TokenBudgetProbe Protocol + AggregatedTokenProbe 具體 impl

`run_explorer` takes an optional `TokenBudgetProbe` — when set,
`_should_stop` queries `probe.total()` each iteration and fires a
`budget_tokens_exhausted` convergence when consumption reaches the
configured `state.budget_tokens_left`. The Protocol is intentionally
tiny (one method) so test doubles stay trivial.

The concrete `AggregatedTokenProbe` sums in-memory `session_total_tokens`
counters across reasoning / judge / coverage TrackedProvider instances
— the HTTP layer (`api/explore.py`) wires one per request after
materialising the three workspace-scoped providers. No file I/O, no
estimator — counters come straight from `TrackedProvider` memory (per
Decision 1 in design.md).
"""
from __future__ import annotations

from collections.abc import Sequence
from typing import Protocol, runtime_checkable

from codebus_agent.providers.tracked import TrackedProvider


__all__ = ["AggregatedTokenProbe", "TokenBudgetProbe"]


@runtime_checkable
class TokenBudgetProbe(Protocol):
    """Single-method surface: current session-cumulative token count.

    Implementations MUST return a non-negative integer representing the
    total tokens consumed across every tracked LLM call in the current
    Explorer run scope. `run_explorer` compares the returned value
    against `state.budget_tokens_left` to decide whether to stop.
    """

    def total(self) -> int: ...


class AggregatedTokenProbe:
    """Sum of `session_total_tokens` across a fixed set of TrackedProviders.

    Caller passes the reasoning + judge + coverage providers constructed
    for a single `POST /explore` request; each has its own in-memory
    counter advanced on successful `chat` / `embed`. Empty provider
    list is rejected at construction time to avoid silent under-budget
    (a zero probe would let the Explorer run forever token-wise —
    counter to the spec's intent).
    """

    def __init__(self, providers: Sequence[TrackedProvider]) -> None:
        providers = list(providers)
        if not providers:
            raise ValueError(
                "AggregatedTokenProbe requires at least one TrackedProvider; "
                "empty list would yield total()==0 and let the Explorer loop "
                "ignore the token budget entirely."
            )
        self._providers: tuple[TrackedProvider, ...] = tuple(providers)

    def total(self) -> int:
        return sum(p.session_total_tokens for p in self._providers)
