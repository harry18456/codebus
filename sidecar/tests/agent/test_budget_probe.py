"""RED tests for TokenBudgetProbe Protocol + AggregatedTokenProbe.

Backs SHALL clauses in
openspec/changes/context-compression-token-budget/design.md
  Decision 2: TokenBudgetProbe Protocol + AggregatedTokenProbe 具體 impl

Section 4 pins the Protocol shape (runtime_checkable single method
`total() -> int`) and the aggregation contract (sum across providers,
reject empty, zero when all providers are fresh).
"""
from __future__ import annotations

from collections.abc import Callable
from pathlib import Path

import pytest
from pydantic import BaseModel

from codebus_agent.providers.mock import MockScript
from codebus_agent.providers.protocol import Message
from codebus_agent.providers.tracked import TrackedProvider


class _Plan(BaseModel):
    title: str = ""


def test_token_budget_probe_is_runtime_checkable_protocol() -> None:
    """Spec Decision 2 shape: Protocol accepts structural conformers."""
    from codebus_agent.agent.budget import TokenBudgetProbe

    class _DuckProbe:
        def total(self) -> int:
            return 42

    assert isinstance(_DuckProbe(), TokenBudgetProbe)

    # Missing `total` fails the isinstance check.
    class _NotAProbe:
        pass

    assert not isinstance(_NotAProbe(), TokenBudgetProbe)


@pytest.mark.asyncio
async def test_aggregated_probe_sums_across_providers(
    mock_reasoning_provider_factory: Callable[[Path], TrackedProvider],
    mock_judge_provider_factory: Callable[[Path], TrackedProvider],
    mock_coverage_provider_factory: Callable[[Path], TrackedProvider],
    workspace_dir: Path,
    mock_script_reasoning: MockScript,
    mock_script_judge: MockScript,
    mock_script_coverage: MockScript,
) -> None:
    """Spec Decision 2: AggregatedTokenProbe sums every provider's running total."""
    from codebus_agent.agent.budget import AggregatedTokenProbe

    # Distinct workspace roots so each provider writes its own
    # token_usage.jsonl and counters stay independent.
    ws_r = workspace_dir / "r"
    ws_j = workspace_dir / "j"
    ws_c = workspace_dir / "c"
    for d in (ws_r, ws_j, ws_c):
        d.mkdir()

    p_r = mock_reasoning_provider_factory(ws_r)
    p_j = mock_judge_provider_factory(ws_j)
    p_c = mock_coverage_provider_factory(ws_c)

    mock_script_reasoning.push(_Plan(title="r"))
    mock_script_judge.push(_Plan(title="j"))
    mock_script_coverage.push(_Plan(title="c"))

    await p_r.chat([Message(role="user", content="r")], response_model=_Plan)
    await p_j.chat([Message(role="user", content="j")], response_model=_Plan)
    await p_c.chat([Message(role="user", content="c")], response_model=_Plan)

    probe = AggregatedTokenProbe([p_r, p_j, p_c])
    expected = (
        p_r.session_total_tokens + p_j.session_total_tokens + p_c.session_total_tokens
    )
    assert probe.total() == expected
    assert expected > 0


def test_aggregated_probe_requires_at_least_one_provider() -> None:
    """Spec Decision 2 Risk mitigation: empty list raises to avoid silent 0 budget."""
    from codebus_agent.agent.budget import AggregatedTokenProbe

    with pytest.raises(ValueError):
        AggregatedTokenProbe([])


def test_aggregated_probe_is_zero_for_fresh_providers(
    mock_reasoning_provider_factory: Callable[[Path], TrackedProvider],
    mock_judge_provider_factory: Callable[[Path], TrackedProvider],
    workspace_dir: Path,
) -> None:
    """Spec scenario `Counters start at zero` lifted to aggregate."""
    from codebus_agent.agent.budget import AggregatedTokenProbe

    ws_r = workspace_dir / "r"
    ws_j = workspace_dir / "j"
    ws_r.mkdir()
    ws_j.mkdir()
    p_r = mock_reasoning_provider_factory(ws_r)
    p_j = mock_judge_provider_factory(ws_j)

    assert AggregatedTokenProbe([p_r, p_j]).total() == 0
