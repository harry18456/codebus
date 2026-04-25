"""Chat-cost lookup table — model id → (input_per_1m, output_per_1m).

Backs spec MODIFIED Requirement
``UsageTracker writes token_usage.jsonl`` (`usage-tracking` capability)
Scenarios ``Known chat model writes non-zero cost_usd`` and
``Unknown chat model logs warning and writes zero cost_usd``
(review-backlog-cleanup), per D-021 (token / cost ledger) and D-022
(wire payload). The pricing table closes the Stage 4 review backlog
finding that chat ``cost_usd`` was hard-coded to ``0.0`` while embed
already had a working cost path through ``Usage.cost_usd``.

Scope intentionally narrow:
- ``chat`` operations only — embed already records ``Usage.cost_usd``
  from the inner provider.
- The pricing table is hand-maintained — there is no auto-sync against
  OpenAI's published prices. When a model's pricing changes upstream,
  bump the entry here in a follow-up change.
- Unknown models record ``cost_usd=0.0`` rather than raising so the
  audit chain never breaks because of a missing price; a WARNING log
  flags the gap so operators can extend the table.
- Zero-token calls (healthz probes, empty prompts) return ``0.0`` with
  NO warning — emitting one for arithmetic-zero output would flood logs.

Key shape:
The keys mirror what ``codebus_agent.providers.tracked._chat_model_id``
reports — i.e., ``"<inner-model>-chat-v1"``. For ``OpenAIChatProvider``
the inner model is the OpenAI model id (e.g., ``gpt-4o-mini``); for
``MockProvider`` the inner is the literal name ``mock``. The
``-chat-v1`` suffix lets the same table key shape evolve if Module 4 /
Module 8 introduce a chat-v2 wire shape later.

Pricing source (2026-04 OpenAI public pricing for ``gpt-4o-mini``):
- input:  $0.15 per 1M tokens
- output: $0.60 per 1M tokens
"""
from __future__ import annotations

import logging

logger = logging.getLogger(__name__)


_CHAT_PRICING: dict[str, tuple[float, float]] = {
    # Real production model — gpt-4o-mini default for all chat-ish roles
    # (REASONING / JUDGE / CHAT / coverage / generate) per
    # `wire_kb_dependencies` factory. Values are USD per 1M tokens.
    "gpt-4o-mini-chat-v1": (0.15, 0.60),
    # Mock provider placeholder. Listed so MockProvider chat calls don't
    # spam WARNING logs in the existing test suite (28+ tests run mock
    # chat). Cost stays $0 because mock traffic is not real OpenAI usage.
    "mock-chat-v1": (0.0, 0.0),
}


def estimate_chat_cost_usd(
    model: str,
    *,
    prompt_tokens: int,
    completion_tokens: int,
) -> float:
    """Return the USD cost for a chat call given the inner model id.

    When ``model`` is present in ``_CHAT_PRICING`` the cost is the sum of
    ``prompt_tokens × input_per_1m / 1_000_000`` and
    ``completion_tokens × output_per_1m / 1_000_000``.

    When ``model`` is absent the function returns ``0.0`` so the audit
    chain never breaks. A WARNING is emitted *only* when the call did
    real work (i.e., ``prompt_tokens + completion_tokens > 0``); a
    zero-token unknown-model call (e.g., a healthz probe wired through
    a future code path) returns silently to keep operator logs clean.
    """
    pricing = _CHAT_PRICING.get(model)
    total_tokens = prompt_tokens + completion_tokens

    if pricing is None:
        if total_tokens > 0:
            logger.warning(
                "unknown chat pricing for model %s — cost recorded as 0.0; "
                "extend codebus_agent.providers.pricing._CHAT_PRICING to fix",
                model,
            )
        return 0.0

    input_per_1m, output_per_1m = pricing
    return (
        prompt_tokens * input_per_1m / 1_000_000
        + completion_tokens * output_per_1m / 1_000_000
    )


__all__ = ["estimate_chat_cost_usd"]
