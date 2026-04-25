"""Tests for ``run_generator`` orchestrator (Section 13).

Backs Requirement
``Generator entrypoint orchestrates per-station markdown pipeline``
(orchestrator path).
"""
from __future__ import annotations

import json
from collections.abc import Callable
from pathlib import Path

import pytest

from codebus_agent.agent.types import ExplorerState, Station
from codebus_agent.generator.runner import run_generator
from codebus_agent.generator.types import GeneratorOptions, StationMarkdown
from codebus_agent.providers.mock import MockScript
from codebus_agent.providers.tracked import TrackedProvider


_TASK_ID = "generate_abcd1234"


def _good_body(idx: int, title: str) -> str:
    return (
        f"# {title}\n\n"
        f"Body for {title}.\n\n"
        f"<Checkpoint id=\"station-{idx}-check\">\n- [ ] {title} 對齊\n</Checkpoint>\n"
    )


def _bad_body() -> str:
    return "# Bad\n\nplain prose without checkpoint\n"


def _three_stations() -> list[Station]:
    return [
        Station(path=f"src/{c}.ts", role="interface", relevance=0.5, why=".")
        for c in "abc"
    ]


@pytest.mark.asyncio
async def test_run_generator_over_three_scripted_stations_writes_three_station_files_plus_moc_plus_route(
    tmp_path: Path,
    mock_script_generate: MockScript,
    mock_generate_provider_factory: Callable[[Path], TrackedProvider],
) -> None:
    for idx, title in enumerate(["Alpha", "Beta", "Gamma"], start=1):
        mock_script_generate.push(
            StationMarkdown(thought="ok", body=_good_body(idx, title))
        )

    state = ExplorerState(
        task="add gdrive adapter",
        budget_steps_left=0,
        budget_tokens_left=0,
        stations=_three_stations(),
    )
    result = await run_generator(
        state=state,
        workspace_root=tmp_path,
        task_id=_TASK_ID,
        llm_chat_provider=mock_generate_provider_factory,
        options=GeneratorOptions(mode="interactive"),
    )
    assert len(result.station_paths) == 3
    for p in result.station_paths:
        assert p.exists()
        text = p.read_text(encoding="utf-8")
        assert text.startswith("---\n"), f"missing frontmatter for {p}"
    assert result.tutorial_path.exists()
    assert result.route_path.exists()
    assert result.degraded_count == 0


@pytest.mark.asyncio
async def test_per_station_failure_does_not_abort_the_run(
    tmp_path: Path,
    mock_script_generate: MockScript,
    mock_generate_provider_factory: Callable[[Path], TrackedProvider],
) -> None:
    # Station 1: ok, Station 2: 3 bad → degraded, Station 3: ok
    mock_script_generate.push(
        StationMarkdown(thought="ok", body=_good_body(1, "Alpha"))
    )
    for _ in range(3):
        mock_script_generate.push(StationMarkdown(thought="bad", body=_bad_body()))
    mock_script_generate.push(
        StationMarkdown(thought="ok", body=_good_body(3, "Gamma"))
    )

    state = ExplorerState(
        task="t",
        budget_steps_left=0,
        budget_tokens_left=0,
        stations=_three_stations(),
    )
    result = await run_generator(
        state=state,
        workspace_root=tmp_path,
        task_id=_TASK_ID,
        llm_chat_provider=mock_generate_provider_factory,
        options=GeneratorOptions(mode="interactive"),
    )
    assert len(result.station_paths) == 3
    assert result.degraded_count == 1

    payload = json.loads(result.route_path.read_text(encoding="utf-8"))
    assert len(payload["stations"]) == 3
    degraded_flags = [s["degraded"] for s in payload["stations"]]
    assert degraded_flags == [False, True, False]


@pytest.mark.asyncio
async def test_generator_uses_tracked_provider_through_llm_chat_provider_factory(
    tmp_path: Path,
    mock_script_generate: MockScript,
    mock_generate_provider_factory: Callable[[Path], TrackedProvider],
) -> None:
    for idx, title in enumerate(["Alpha", "Beta", "Gamma"], start=1):
        mock_script_generate.push(
            StationMarkdown(thought="ok", body=_good_body(idx, title))
        )

    state = ExplorerState(
        task="t",
        budget_steps_left=0,
        budget_tokens_left=0,
        stations=_three_stations(),
    )
    await run_generator(
        state=state,
        workspace_root=tmp_path,
        task_id=_TASK_ID,
        llm_chat_provider=mock_generate_provider_factory,
        options=GeneratorOptions(mode="interactive"),
    )

    token_lines = [
        json.loads(line)
        for line in (tmp_path / ".codebus" / "token_usage.jsonl")
        .read_text(encoding="utf-8")
        .splitlines()
    ]
    assert token_lines, "TrackedProvider MUST write token_usage.jsonl"
    assert all(line.get("module") == "generate" for line in token_lines), (
        f"every token_usage line MUST carry module=generate; "
        f"got modules={[line.get('module') for line in token_lines]!r}"
    )

    llm_lines = (
        (tmp_path / ".codebus" / "llm_calls.jsonl")
        .read_text(encoding="utf-8")
        .splitlines()
    )
    assert llm_lines, "TrackedProvider MUST write llm_calls.jsonl"
    # At least one entry must carry a request payload (proxy for "wire payload landed").
    parsed = [json.loads(line) for line in llm_lines]
    assert any("request" in p for p in parsed), parsed
