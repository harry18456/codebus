"""Tests for output directory layout (Section 12).

Backs Requirement
``Output root directory is workspace/codebus-tutorials per task``
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
        f"This station covers {title}.\n\n"
        f"<Checkpoint id=\"station-{idx}-check\">\n- [ ] {title} 對齊\n</Checkpoint>\n"
    )


def _push_three(script: MockScript) -> None:
    for idx, title in enumerate(["A", "B", "C"], start=1):
        script.push(StationMarkdown(thought="ok", body=_good_body(idx, title)))


@pytest.mark.asyncio
async def test_first_write_creates_codebus_tutorials_directory_tree(
    tmp_path: Path,
    mock_script_generate: MockScript,
    mock_generate_provider_factory: Callable[[Path], TrackedProvider],
) -> None:
    _push_three(mock_script_generate)
    state = ExplorerState(
        task="t",
        budget_steps_left=0,
        budget_tokens_left=0,
        stations=[
            Station(path=f"src/{c}.ts", role="x", relevance=0.5, why=".")
            for c in "abc"
        ],
    )
    await run_generator(
        state=state,
        workspace_root=tmp_path,
        task_id=_TASK_ID,
        llm_chat_provider=mock_generate_provider_factory,
        options=GeneratorOptions(mode="interactive"),
    )
    stations_dir = tmp_path / "codebus-tutorials" / _TASK_ID / "stations"
    assert stations_dir.exists() and stations_dir.is_dir()


@pytest.mark.asyncio
async def test_generator_does_not_write_to_codebus_subdir_except_generator_log_jsonl(
    tmp_path: Path,
    mock_script_generate: MockScript,
    mock_generate_provider_factory: Callable[[Path], TrackedProvider],
) -> None:
    _push_three(mock_script_generate)
    state = ExplorerState(
        task="t",
        budget_steps_left=0,
        budget_tokens_left=0,
        stations=[
            Station(path=f"src/{c}.ts", role="x", relevance=0.5, why=".")
            for c in "abc"
        ],
    )
    await run_generator(
        state=state,
        workspace_root=tmp_path,
        task_id=_TASK_ID,
        llm_chat_provider=mock_generate_provider_factory,
        options=GeneratorOptions(mode="interactive"),
    )
    # No station / tutorial / route under <ws>/.codebus/
    codebus_dir = tmp_path / ".codebus"
    forbidden = list(codebus_dir.glob("**/tutorial.md")) + list(
        codebus_dir.glob("**/route.json")
    ) + list(codebus_dir.glob("**/s*-*.md"))
    assert forbidden == [], f"forbidden artifacts under .codebus: {forbidden!r}"

    # The generator_log.jsonl is the only operational log allowed there.
    # (`token_usage.jsonl` / `llm_calls.jsonl` / `sanitize_audit.jsonl`
    # are written by TrackedProvider — also expected; they're audit
    # chain, not generator product.)
    assert (codebus_dir / "generator_log.jsonl").exists()
