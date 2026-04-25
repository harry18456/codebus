"""Tests for per-station retry + degraded-fallback pipeline (Section 7).

Backs Requirements
``Generator entrypoint orchestrates per-station markdown pipeline``
(per-station path) and
``Degraded fallback writes per-station stub after retry exhaustion``.
"""
from __future__ import annotations

import json
from datetime import datetime, timezone
from pathlib import Path

import pytest

from codebus_agent.agent.types import Station
from codebus_agent.generator.log import GeneratorLogger
from codebus_agent.generator.station import (
    StationContext,
    StationOutcome,
    generate_station,
)
from codebus_agent.generator.types import StationMarkdown
from codebus_agent.providers.mock import MockScript
from codebus_agent.providers.tracked import TrackedProvider
from codebus_agent.sanitizer import SanitizerAuditLogger, SanitizerEngine


_TS = datetime(2026, 4, 25, 10, 30, 0, tzinfo=timezone.utc)


def _ctx(
    *,
    workspace_dir: Path,
    provider: TrackedProvider,
    station_index: int,
    station_id: str,
    title: str,
) -> StationContext:
    audit_dir = workspace_dir / ".codebus"
    audit_dir.mkdir(parents=True, exist_ok=True)
    out_dir = workspace_dir / "codebus-tutorials" / "generate_test1234" / "stations"
    return StationContext(
        workspace_root=workspace_dir,
        output_path=out_dir / f"{station_id}.md",
        provider=provider,
        sanitizer=SanitizerEngine(),
        sanitizer_audit=SanitizerAuditLogger(audit_dir / "sanitize_audit.jsonl"),
        rules_version="2026-04-20-1",
        log=GeneratorLogger(audit_dir / "generator_log.jsonl"),
        station_index=station_index,
        station_id=station_id,
        station_title=title,
        task="add gdrive adapter",
        repo_name=workspace_dir.name,
        workspace_type="folder",
        generated_at=_TS,
        duration_minutes=15,
        related_files=[],
        related_stations=[],
        mode="interactive",
        target_persona="experienced engineer",
        previous_stations_summary="",
        related_files_excerpt="",
        kb_hits_excerpt="",
        max_retries=3,
    )


def _good_body(idx: int, title: str) -> str:
    return (
        f"# {title}\n\n"
        f"這站的核心是 {title} 的職責切片，先看 input 邊界再看 output 邊界。\n\n"
        f"<Checkpoint id=\"station-{idx}-check\">\n- [ ] 對齊 {title} 的核心職責\n</Checkpoint>\n"
    )


def _bad_body() -> str:
    # No <Checkpoint> → validator returns missing_checkpoint
    return "# Title\n\nplain prose without any custom elements\n"


@pytest.mark.asyncio
async def test_three_retries_with_persistent_issues_produces_degraded_stub(
    tmp_path: Path,
    mock_script_generate: MockScript,
    mock_generate_provider: TrackedProvider,
) -> None:
    # Push 3 bad responses — validator will reject each one.
    for _ in range(3):
        mock_script_generate.push(
            StationMarkdown(thought="bad attempt", body=_bad_body())
        )
    # Push a 4th (good) response that MUST NOT be consumed.
    canary = StationMarkdown(thought="canary", body=_good_body(2, "Storage"))
    mock_script_generate.push(canary)

    ctx = _ctx(
        workspace_dir=tmp_path,
        provider=mock_generate_provider,
        station_index=2,
        station_id="s02-storage",
        title="Storage",
    )

    outcome = await generate_station(
        station=Station(
            path="src/storage.ts", role="interface", relevance=0.8, why="..."
        ),
        ctx=ctx,
    )
    assert isinstance(outcome, StationOutcome)
    assert outcome.degraded is True
    assert outcome.station_path.exists()
    text = outcome.station_path.read_text(encoding="utf-8")
    assert "degraded: true" in text
    # Canary MUST still be in script (4th attempt must NOT have happened).
    leftover = mock_script_generate.pop()
    assert leftover is canary, "_generate_station MUST NOT consume the 4th response"


@pytest.mark.asyncio
async def test_per_station_degradation_does_not_affect_subsequent_stations(
    tmp_path: Path,
    mock_script_generate: MockScript,
    mock_generate_provider: TrackedProvider,
) -> None:
    # Station 1: one good response
    mock_script_generate.push(
        StationMarkdown(thought="ok", body=_good_body(1, "Overview"))
    )
    # Station 2: 3 bad responses → degraded
    for _ in range(3):
        mock_script_generate.push(
            StationMarkdown(thought="bad", body=_bad_body())
        )
    # Station 3: one good response
    mock_script_generate.push(
        StationMarkdown(thought="ok", body=_good_body(3, "Adapter"))
    )

    outcomes: list[StationOutcome] = []
    cases = [
        (1, "s01-overview", "Overview"),
        (2, "s02-storage", "Storage"),
        (3, "s03-adapter", "Adapter"),
    ]
    for idx, sid, title in cases:
        ctx = _ctx(
            workspace_dir=tmp_path,
            provider=mock_generate_provider,
            station_index=idx,
            station_id=sid,
            title=title,
        )
        outcomes.append(
            await generate_station(
                station=Station(
                    path=f"src/x{idx}.ts", role="interface", relevance=0.5, why="."
                ),
                ctx=ctx,
            )
        )

    assert outcomes[0].degraded is False
    assert outcomes[1].degraded is True
    assert outcomes[2].degraded is False
    for o in outcomes:
        assert o.station_path.exists()


@pytest.mark.asyncio
async def test_disk_write_failure_does_not_retry_indefinitely(
    tmp_path: Path,
    mock_script_generate: MockScript,
    mock_generate_provider: TrackedProvider,
    monkeypatch,
) -> None:
    mock_script_generate.push(
        StationMarkdown(thought="ok", body=_good_body(1, "Overview"))
    )
    ctx = _ctx(
        workspace_dir=tmp_path,
        provider=mock_generate_provider,
        station_index=1,
        station_id="s01-overview",
        title="Overview",
    )

    write_calls: list[Path] = []
    real_write = Path.write_text

    def _fail_write(self: Path, *args, **kwargs):
        write_calls.append(self)
        raise OSError("disk full")

    monkeypatch.setattr(Path, "write_text", _fail_write)

    outcome = await generate_station(
        station=Station(
            path="src/x.ts", role="interface", relevance=0.5, why="."
        ),
        ctx=ctx,
    )
    assert outcome.degraded is True
    assert outcome.error == "write_failed"

    # Restore so we can inspect the log file.
    monkeypatch.setattr(Path, "write_text", real_write)
    log_lines = [
        json.loads(line)
        for line in (tmp_path / ".codebus" / "generator_log.jsonl")
        .read_text(encoding="utf-8")
        .splitlines()
    ]
    assert any(e.get("event") == "write_failed" for e in log_lines)
    # Exactly one write attempt MUST have occurred.
    target_writes = [p for p in write_calls if p.name == "s01-overview.md"]
    assert len(target_writes) == 1, (
        f"expected exactly 1 write attempt for the station file, got {target_writes!r}"
    )


@pytest.mark.asyncio
async def test_validator_issues_feed_into_next_attempt_prompt_as_correction_hint(
    tmp_path: Path,
    mock_script_generate: MockScript,
    mock_generate_provider: TrackedProvider,
) -> None:
    # 1st response: bad (missing_checkpoint) — validator rejects.
    mock_script_generate.push(
        StationMarkdown(thought="bad", body=_bad_body())
    )
    # 2nd response: good — validator accepts.
    mock_script_generate.push(
        StationMarkdown(thought="ok", body=_good_body(2, "Storage"))
    )

    ctx = _ctx(
        workspace_dir=tmp_path,
        provider=mock_generate_provider,
        station_index=2,
        station_id="s02-storage",
        title="Storage",
    )

    captured_user_prompts: list[str] = []
    real_chat = mock_generate_provider._inner.chat

    async def _spy_chat(messages, *, response_model):
        captured_user_prompts.append(
            "\n".join(m.content for m in messages if m.role == "user")
        )
        return await real_chat(messages, response_model=response_model)

    mock_generate_provider._inner.chat = _spy_chat  # type: ignore[assignment]

    outcome = await generate_station(
        station=Station(
            path="src/storage.ts", role="interface", relevance=0.8, why="."
        ),
        ctx=ctx,
    )
    assert outcome.degraded is False
    assert len(captured_user_prompts) == 2
    # Second attempt's user prompt MUST carry the prior validator issues.
    assert "missing_checkpoint" in captured_user_prompts[1], (
        f"correction_hint must surface previous validator issues; "
        f"got: {captured_user_prompts[1]!r}"
    )
