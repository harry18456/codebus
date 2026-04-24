"""Content-contract tests for Coverage prompt module.

Backs SHALL clauses in
openspec/changes/coverage-gap-recurse/specs/agent-core/spec.md
  Requirement: LLMCoverageChecker produces one-shot CoverageResult
    Scenario: Prompt module exposes version constant

Section 2 pins:
- `render_coverage_prompt` is deterministic on sorted state.
- The visited-files window caps at 20 entries with a `... (N more)`
  footer when `visited_files` is larger (mirrors the Judge prompt
  window per design Decision 5).
- `COVERAGE_PROMPT_VERSION` follows date-version format
  `YYYY-MM-DD-N` so `reasoning_log.jsonl` replay can pin revisions.
"""
from __future__ import annotations

import re


def test_render_coverage_prompt_is_deterministic_on_sorted_state() -> None:
    """Spec scenario `render_coverage_prompt is deterministic`.

    Two consecutive renders of the same state MUST produce bit-identical
    strings so `reasoning_log.jsonl` replay + golden-sample drift guards
    are reliable.
    """
    from codebus_agent.agent.prompts.coverage import render_coverage_prompt
    from codebus_agent.agent.types import ExplorerState, Station

    state = ExplorerState(
        task="trace storage wiring",
        budget_steps_left=5,
        budget_tokens_left=1_000,
        visited_files={f"src/f{i:02d}.py" for i in range(10)},
        stations=[
            Station(
                path=f"src/st_{i}.py",
                role="entry",
                relevance=0.5,
                why="seed",
                depends_on=[],
            )
            for i in range(3)
        ],
    )

    rendered_a = render_coverage_prompt(state)
    rendered_b = render_coverage_prompt(state)

    assert rendered_a == rendered_b, (
        "render_coverage_prompt MUST be deterministic — two calls with the "
        "same state produced different strings"
    )


def test_render_coverage_prompt_windows_visited_at_20_with_more_footer() -> None:
    """Spec scenario `visited window caps at 20 with more footer`.

    30 visited files → first 20 sorted entries rendered, `... (10 more)`
    footer appended. Mirrors the Judge prompt window shape per design
    Decision 5 (bounded visited rendering).
    """
    from codebus_agent.agent.prompts.coverage import render_coverage_prompt
    from codebus_agent.agent.types import ExplorerState

    visited = {f"src/f{i:02d}.py" for i in range(30)}
    state = ExplorerState(
        task="t",
        budget_steps_left=5,
        budget_tokens_left=1_000,
        visited_files=visited,
    )
    rendered = render_coverage_prompt(state)
    sorted_visited = sorted(visited)

    for path in sorted_visited[:20]:
        assert path in rendered, (
            f"visited path {path!r} must appear in the first-20 window"
        )
    assert "... (10 more)" in rendered, (
        "visited window must include the `... (10 more)` footer when "
        "the set is larger than 20"
    )


def test_coverage_prompt_version_is_date_version_format() -> None:
    """Spec scenario `Prompt module exposes version constant`.

    `COVERAGE_PROMPT_VERSION` MUST match `^\\d{4}-\\d{2}-\\d{2}-\\d+$` so
    `reasoning_log.jsonl` replay can compare prompt revisions across
    runs. The pinned format matches `JUDGE_PROMPT_VERSION` so tooling
    that ingests both is format-symmetric.
    """
    from codebus_agent.agent.prompts.coverage import COVERAGE_PROMPT_VERSION

    assert re.match(r"^\d{4}-\d{2}-\d{2}-\d+$", COVERAGE_PROMPT_VERSION), (
        f"COVERAGE_PROMPT_VERSION={COVERAGE_PROMPT_VERSION!r} must match "
        f"the date-version regex ^\\d{{4}}-\\d{{2}}-\\d{{2}}-\\d+$"
    )
