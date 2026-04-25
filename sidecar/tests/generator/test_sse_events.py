"""Tests for SSE generating events (Section 14).

Backs Requirement
``SSE generating events stream per-station progress``
in `openspec/changes/module-5-generator-p0/specs/module-5-generator/spec.md`.
"""
from __future__ import annotations

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


def _state() -> ExplorerState:
    return ExplorerState(
        task="t",
        budget_steps_left=0,
        budget_tokens_left=0,
        stations=_three_stations(),
    )


def _filter(events: list[dict], **fields) -> list[dict]:
    out = []
    for e in events:
        if all(e.get(k) == v for k, v in fields.items()):
            out.append(e)
    return out


@pytest.mark.asyncio
async def test_three_station_run_emits_all_phases_for_each_station_plus_assembling_moc_twice(
    tmp_path: Path,
    mock_script_generate: MockScript,
    mock_generate_provider_factory: Callable[[Path], TrackedProvider],
    spy_emitter,
) -> None:
    for idx, title in enumerate(["A", "B", "C"], start=1):
        mock_script_generate.push(StationMarkdown(thought="ok", body=_good_body(idx, title)))

    await run_generator(
        state=_state(),
        workspace_root=tmp_path,
        task_id=_TASK_ID,
        llm_chat_provider=mock_generate_provider_factory,
        options=GeneratorOptions(mode="interactive"),
        emitter=spy_emitter,
    )

    progress_events = [e for e in spy_emitter.events if e.get("type") == "progress"]
    assert len(_filter(progress_events, status="generating")) == 3
    assert len(_filter(progress_events, status="validating")) == 3
    station_writes = _filter(progress_events, status="writing_file", phase="generating")
    assert len(station_writes) == 3, station_writes
    moc_writes = _filter(progress_events, phase="assembling_moc")
    assert len(moc_writes) == 2, moc_writes
    # Every event MUST carry total_stations=3.
    assert all(e.get("total_stations") == 3 for e in progress_events)


@pytest.mark.asyncio
async def test_retry_attempt_emits_retry_status(
    tmp_path: Path,
    mock_script_generate: MockScript,
    mock_generate_provider_factory: Callable[[Path], TrackedProvider],
    spy_emitter,
) -> None:
    # Station 1: ok. Station 2: 1 bad → 1 ok. Station 3: ok.
    mock_script_generate.push(StationMarkdown(thought="ok", body=_good_body(1, "A")))
    mock_script_generate.push(StationMarkdown(thought="bad", body=_bad_body()))
    mock_script_generate.push(StationMarkdown(thought="ok", body=_good_body(2, "B")))
    mock_script_generate.push(StationMarkdown(thought="ok", body=_good_body(3, "C")))

    await run_generator(
        state=_state(),
        workspace_root=tmp_path,
        task_id=_TASK_ID,
        llm_chat_provider=mock_generate_provider_factory,
        options=GeneratorOptions(mode="interactive"),
        emitter=spy_emitter,
    )

    retry_events = [e for e in spy_emitter.events if e.get("status") == "retry"]
    assert len(retry_events) == 1, retry_events
    assert retry_events[0]["current_station"] == 2
    assert retry_events[0]["attempt"] == 2


@pytest.mark.asyncio
async def test_missing_emitter_suppresses_all_generating_progress_events(
    tmp_path: Path,
    mock_script_generate: MockScript,
    mock_generate_provider_factory: Callable[[Path], TrackedProvider],
) -> None:
    for idx, title in enumerate(["A", "B", "C"], start=1):
        mock_script_generate.push(StationMarkdown(thought="ok", body=_good_body(idx, title)))

    # No emitter passed — behaviour must remain identical, no side effects.
    result = await run_generator(
        state=_state(),
        workspace_root=tmp_path,
        task_id=_TASK_ID,
        llm_chat_provider=mock_generate_provider_factory,
        options=GeneratorOptions(mode="interactive"),
    )
    assert len(result.station_paths) == 3
    assert result.degraded_count == 0
