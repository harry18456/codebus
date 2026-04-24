"""Content-contract tests for Judge prompt.

Backs SHALL clauses in
openspec/changes/explorer-judge-golden/specs/explorer-golden/spec.md
  Requirement: Judge prompt produces station and follow-imports signals
  Requirement: JUDGE_PROMPT_VERSION uses date-version format and bumps with content changes

Section 2 pins the three-section shape of `JUDGE_SYSTEM` and the
rendering contract of `render_judge_prompt` (visited / stations /
ToolResult truncation). Section 4 pins the version-string format and
`EXPLORER_PROMPT_VERSION` freeze.

These are **content** assertions (strings in a prompt), so they
accept either the exact Chinese wording from the spec or a small set
of documented synonyms — that way a future prompt refactor can
reword without every assertion flipping red spuriously.
"""
from __future__ import annotations

import re

from codebus_agent.agent.prompts.judge import (
    JUDGE_PROMPT_VERSION,
    JUDGE_SYSTEM,
    render_judge_prompt,
)
from codebus_agent.agent.prompts.explorer import EXPLORER_PROMPT_VERSION
from codebus_agent.agent.types import (
    ExplorerState,
    Station,
    ToolResult,
)


# ------------------------- Section 2 RED ------------------------------------


def test_judge_system_carries_role_bounds_section() -> None:
    """JUDGE_SYSTEM MUST describe the one-shot role boundaries.

    Spec Requirement `Judge prompt produces station and follow-imports signals`
    §1 says the Judge runs one-shot, MUST NOT enter a ReAct sub-loop, MUST
    NOT invoke tools, MUST NOT mutate state. We assert the zh-TW phrasing
    carries each of those four constraints. Accepted synonyms capture the
    small set of rewordings we've already approved so a future prompt
    polish doesn't flip this red spuriously.
    """
    role_synonyms = {
        "one_shot": ["one-shot", "一次性", "單次"],
        "no_react_subloop": ["不進 ReAct", "不進入 ReAct", "不展開 ReAct"],
        "no_tools": ["不呼叫工具", "不呼叫 tool", "不使用工具"],
        "no_state_mutation": ["不改 state", "不改變 state", "不修改 state"],
    }
    for label, candidates in role_synonyms.items():
        assert any(c in JUDGE_SYSTEM for c in candidates), (
            f"JUDGE_SYSTEM missing role-bounds constraint {label!r}; "
            f"expected one of {candidates!r} to appear in the prompt"
        )


def test_judge_system_carries_station_decision_section() -> None:
    """JUDGE_SYSTEM MUST give at least one positive and one negative station criterion.

    Spec §2 enumerates positive criteria (new architectural slice / entrypoint /
    protocol boundary) and negative criteria (pure import chain / already
    visited). Any single synonym per direction is enough to pass.
    """
    positive_candidates = ["架構切片", "entrypoint", "協議邊界"]
    negative_candidates = ["純 import", "已 visited", "已訪問"]
    assert any(c in JUDGE_SYSTEM for c in positive_candidates), (
        f"JUDGE_SYSTEM missing a positive station criterion "
        f"(expected one of {positive_candidates!r})"
    )
    assert any(c in JUDGE_SYSTEM for c in negative_candidates), (
        f"JUDGE_SYSTEM missing a negative station criterion "
        f"(expected one of {negative_candidates!r})"
    )


def test_judge_system_carries_follow_imports_and_relevance_anchor() -> None:
    """JUDGE_SYSTEM MUST carry the five-point relevance anchor.

    Spec §3 pins a fixed five-point scale (0.0 / 0.3 / 0.5 / 0.8 / 1.0).
    Each anchor value MUST appear literally so the LLM has a concrete
    scaffold to land on.
    """
    for anchor in ("0.0", "0.3", "0.5", "0.8", "1.0"):
        assert anchor in JUDGE_SYSTEM, (
            f"JUDGE_SYSTEM missing relevance anchor {anchor!r}; the five-point "
            f"scale must include 0.0 / 0.3 / 0.5 / 0.8 / 1.0"
        )


def test_render_judge_prompt_includes_visited_and_stations() -> None:
    """The user prompt MUST summarise visited files and stations so Judge sees convergence context.

    Spec scenario `render_judge_prompt includes visited and stations context`:
    - 25 visited files → first 20 rendered, marker `... (5 more)` appended.
    - 4 stations → count "4" rendered, last 3 stations' role/path rendered.
    """
    visited = {f"src/f{i:02d}.py" for i in range(25)}
    stations = [
        Station(
            path=f"src/station_{i}.py",
            role=f"role_{i}",
            relevance=0.5,
            why="fixture",
            depends_on=[],
        )
        for i in range(4)
    ]
    state = ExplorerState(
        task="explore the knowledge base",
        budget_steps_left=5,
        budget_tokens_left=1000,
        visited_files=visited,
        stations=stations,
    )

    rendered = render_judge_prompt(state, [])

    assert "visited" in rendered, (
        "rendered prompt must reference 'visited' so the Judge knows which "
        "files to skip for should_add_station"
    )
    # All 20 retained paths must appear; the 5 dropped must not (we can't
    # know the exact set because it depends on sort order, but we can assert
    # the first 20 sorted entries are present).
    sorted_visited = sorted(visited)
    for path in sorted_visited[:20]:
        assert path in rendered, (
            f"visited path {path!r} should appear in the first-20 window"
        )
    assert "... (5 more)" in rendered, (
        "visited-files truncation marker `... (5 more)` missing when "
        "25 > 20 entries"
    )

    # Stations count + last 3 summary
    assert "4" in rendered, "stations count (4) must render"
    for station in stations[-3:]:
        assert station.path in rendered, (
            f"last-3 stations window missing path {station.path!r}"
        )
        assert station.role in rendered, (
            f"last-3 stations window missing role {station.role!r}"
        )
    # The first station (oldest) MUST NOT appear in the last-3 summary.
    # Guarded via the station_0 path (distinctive enough to avoid false
    # negatives from other sections of the prompt).
    assert "src/station_0.py" not in rendered, (
        "last-3 summary leaked station_0 (the dropped entry)"
    )


def test_render_judge_prompt_truncates_tool_output_at_800_chars() -> None:
    """ToolResult rendering MUST cap output at 800 chars and surface errors distinctly.

    Spec scenario `ToolResult output is truncated at 800 chars`:
    - `output = "x" * 10_000` → rendered slice length ≤ 810 (800 + short marker).
    - `error != None` → prompt renders `error=<msg>` not `output=<...>`.
    """
    huge = ToolResult(
        tool_call_id="tc_huge",
        tool_name="read_file",
        output="x" * 10_000,
        raw=None,
        error=None,
    )
    failed = ToolResult(
        tool_call_id="tc_err",
        tool_name="search",
        output="",
        raw=None,
        error="boom: disk exploded",
    )
    state = ExplorerState(
        task="t", budget_steps_left=1, budget_tokens_left=1
    )

    rendered_huge = render_judge_prompt(state, [huge])
    rendered_err = render_judge_prompt(state, [failed])

    # Find the contiguous run of 'x' in the rendered prompt; it must be ≤ 810.
    runs = re.findall(r"x+", rendered_huge)
    longest_run = max((len(r) for r in runs), default=0)
    assert longest_run <= 810, (
        f"tool output rendered at {longest_run} chars; must be ≤ 810 "
        f"(800 body + short truncation marker)"
    )
    assert longest_run >= 700, (
        f"tool output rendered at {longest_run} chars; expected ~800 — "
        f"truncation floor too aggressive"
    )

    assert "error=" in rendered_err, (
        "failed ToolResult must render with `error=<msg>` prefix so the "
        "Judge can distinguish failure from empty success"
    )
    assert "boom: disk exploded" in rendered_err, (
        "error message body missing from rendered prompt"
    )
    assert "output=" not in rendered_err, (
        "failed ToolResult must NOT carry `output=` prefix — it would "
        "suggest the tool succeeded"
    )


# ------------------------- Section 4 RED ------------------------------------


def test_judge_prompt_version_matches_date_format() -> None:
    """JUDGE_PROMPT_VERSION MUST follow the date-version format `YYYY-MM-DD-N`.

    Spec scenario `JUDGE_PROMPT_VERSION matches the required regex`: the
    constant MUST match `^\\d{4}-\\d{2}-\\d{2}-\\d+$`.
    """
    assert re.match(r"^\d{4}-\d{2}-\d{2}-\d+$", JUDGE_PROMPT_VERSION), (
        f"JUDGE_PROMPT_VERSION={JUDGE_PROMPT_VERSION!r} must match "
        f"the date-version regex ^\\d{{4}}-\\d{{2}}-\\d{{2}}-\\d+$"
    )


def test_explorer_prompt_version_unchanged_by_this_change() -> None:
    """EXPLORER_PROMPT_VERSION MUST stay at its pre-change value.

    Spec Requirement `JUDGE_PROMPT_VERSION uses date-version format` §
    `EXPLORER_PROMPT_VERSION stays frozen across Judge-only changes`:
    this change is scoped to Judge prompt work, so Explorer's constant
    MUST remain at its pre-change pinned value.
    """
    # Hard pin — if a future Explorer prompt change lands, this test must
    # flip red and be updated in the SAME commit that bumps the constant.
    assert EXPLORER_PROMPT_VERSION == "v0-p0", (
        f"EXPLORER_PROMPT_VERSION drifted to {EXPLORER_PROMPT_VERSION!r}; "
        f"this change is Judge-only and must leave the Explorer version at "
        f"'v0-p0'. If an Explorer prompt change is landing, bump this pin "
        f"in the SAME commit."
    )
