"""Agent core Pydantic data structures.

Backs SHALL clauses in
openspec/changes/explorer-react-loop-p0/specs/agent-core/spec.md
  Requirement: Agent-core types are Pydantic BaseModels with stable JSON serialization

Every structure uses Pydantic v2 `BaseModel` so `model_dump_json` /
`model_validate_json` round-trip without data loss. That is the contract
`reasoning_log.jsonl` replay, golden-sample fixtures, and future
Generator-side consumers (Module 5) depend on.

`Step` is the unit of reasoning-log output; `ExplorerAction` is the
Instructor `response_model` for the Think step; `JudgeVerdict` is the
Instructor `response_model` for Judge one-shot calls.
"""
from __future__ import annotations

from datetime import datetime
from typing import Any, Literal

from pydantic import BaseModel, Field


__all__ = [
    "CoverageResult",
    "ExplorerAction",
    "ExplorerResult",
    "ExplorerState",
    "Gap",
    "JudgeVerdict",
    "KBCitation",
    "Message",
    "QAAction",
    "QAAnswer",
    "QAState",
    "Station",
    "Step",
    "ToolCall",
    "ToolResult",
]


class Message(BaseModel):
    """Chat message in agent-layer shape (distinct from provider-layer dataclass).

    Carries `tool_name` alongside `tool_call_id` so the reasoning log can
    pair tool observations back to the invocation without a second lookup
    — provider-layer `Message` (a dataclass in `codebus_agent.providers`)
    intentionally omits this context since it is irrelevant to wire
    payload. Conversion at the ``_think`` boundary drops `tool_name`
    before dispatch.
    """

    role: Literal["system", "user", "assistant", "tool"]
    content: str
    tool_call_id: str | None = None
    tool_name: str | None = None


class ToolCall(BaseModel):
    id: str
    name: str
    arguments: dict[str, Any]


class ToolResult(BaseModel):
    tool_call_id: str
    tool_name: str
    output: str
    raw: Any = None
    error: str | None = None


class JudgeVerdict(BaseModel):
    """Relevance Judge output — Instructor validates this at parse time."""

    relevance: float = Field(ge=0, le=1)
    should_follow_imports: bool
    should_add_station: bool
    reason: str


class Gap(BaseModel):
    """Coverage-gap descriptor — populated by follow-up `coverage-gap-recurse` change."""

    description: str
    suggested_target: str | None = None


class CoverageResult(BaseModel):
    """List of gaps returned by a `CoverageChecker` — schema stub for P0."""

    gaps: list[Gap] = Field(default_factory=list)


class Station(BaseModel):
    """One exploration waypoint — fed to Module 5 Generator downstream."""

    path: str
    role: str
    relevance: float = Field(ge=0, le=1)
    why: str
    depends_on: list[str] = Field(default_factory=list)


class ExplorerState(BaseModel):
    """ReAct session state — mutated by Explorer loop's Update step only."""

    task: str
    messages: list[Message] = Field(default_factory=list)
    visited_files: set[str] = Field(default_factory=set)
    pending_queue: list[str] = Field(default_factory=list)
    stations: list[Station] = Field(default_factory=list)
    budget_steps_left: int
    budget_tokens_left: int
    step_count: int = 0


class ExplorerAction(BaseModel):
    """Think-step output; Instructor populates this from `provider.chat`."""

    thought: str
    tool_calls: list[ToolCall] = Field(default_factory=list)
    stop: bool = False


class ExplorerResult(BaseModel):
    """Terminal output of `run_explorer`.

    `stopped_reason` enumerates the four convergence branches defined in
    spec Requirement `Explorer loop stops on budget exhaustion, empty
    queue, or cancel signal`. Kept as `Literal` so callers (HTTP layer,
    golden-sample replayer) can match-case on it safely. The
    `budget_tokens_exhausted` branch landed with
    `context-compression-token-budget` and fires only when a caller
    passes a non-None `TokenBudgetProbe` to `run_explorer`.
    """

    stations: list[Station] = Field(default_factory=list)
    log_path: str
    stopped_reason: Literal[
        "budget_exhausted",
        "queue_empty",
        "cancelled",
        "budget_tokens_exhausted",
    ]


class Step(BaseModel):
    """One ReAct iteration — written as a single line to `reasoning_log.jsonl`.

    `explorer_prompt_version` / `judge_prompt_version` are module-level
    constants captured at write time (`prompts.EXPLORER_PROMPT_VERSION`
    etc.) so golden-sample replays can pin prompt revisions — per spec
    Requirement `ReasoningLogger appends one JSONL line per Step to
    workspace path`.

    `qa_prompt_version` is the Q&A counterpart added by `module-8-qa-p0`;
    it is mutually exclusive with explorer / judge prompt versions —
    `ReasoningLogger` in `qa` mode strips the explorer/judge fields
    from the serialized line so the Q&A audit trail remains clearly
    attributed to the Q&A prompt revision in play.
    """

    step: int
    ts: datetime
    thought: str
    tool_calls: list[ToolCall] = Field(default_factory=list)
    tool_results: list[ToolResult] = Field(default_factory=list)
    judge_verdict: JudgeVerdict | None = None
    tokens_used: int = 0
    explorer_prompt_version: str = ""
    judge_prompt_version: str = ""
    qa_prompt_version: str = ""


class KBCitation(BaseModel):
    """Per-citation reference attached to a `QAAnswer`.

    Mirrors the SSE `qa_answer` event payload schema so the citation
    list can flow from `QAAnswer.citations` straight to the wire
    without per-field translation.
    """

    file_path: str
    line_start: int = Field(ge=0)
    line_end: int = Field(ge=0)
    related_stations: list[str] = Field(default_factory=list)


class QAAction(BaseModel):
    """Q&A Think-step output — Instructor `response_model` for `provider.chat`.

    Mirrors `ExplorerAction` shape (`thought`, `tool_calls`) so the
    same `_think`-style call seam can validate both Explorer and Q&A
    LLM responses without per-call branching.
    """

    thought: str
    tool_calls: list[ToolCall] = Field(default_factory=list)


class QAAnswer(BaseModel):
    """Terminal Q&A output — final synthesized answer + citations.

    Returned by `run_qa(...)` as the run's only success-path payload.
    The shape is also the wire schema for the `qa_answer` SSE event.
    """

    answer: str
    citations: list[KBCitation] = Field(default_factory=list)


class QAState(BaseModel):
    """Q&A session state — mirrors `ExplorerState` for `_should_stop` reuse.

    The minimum fields specified by the qa-agent capability spec are
    `question`, `originating_station_id`, `session_id`, `messages`,
    `step_count`, `add_to_kb_session_count`, `add_to_kb_question_count`.
    Additional ExplorerState-compatible fields (`budget_steps_left` /
    `budget_tokens_left` / `pending_queue` / `stations` / `visited_files`)
    carry defaults so the existing Explorer `_should_stop` helper can
    operate against `QAState` instances without per-state branching.
    The unused list / set fields stay empty during a Q&A run because
    Q&A has no station / coverage concept — they're present purely as
    structural compatibility seams.
    """

    question: str
    originating_station_id: str | None = None
    session_id: str
    messages: list[Message] = Field(default_factory=list)
    step_count: int = 0
    add_to_kb_session_count: int = 0
    add_to_kb_question_count: int = 0
    # Compatibility fields for explorer._should_stop reuse.
    budget_steps_left: int = 10
    budget_tokens_left: int = 0
    pending_queue: list[str] = Field(default_factory=list)
    stations: list[Station] = Field(default_factory=list)
    visited_files: set[str] = Field(default_factory=set)
