"""Q&A Agent main loop — RAG-first two-stage flow + ReAct fallback.

Backs SHALL clauses in
openspec/changes/module-8-qa-p0/specs/qa-agent/spec.md
  Requirement: Q&A loop entry point with two-stage RAG-first flow
  Requirement: `_hits_confident` declares three threshold conditions
  Requirement: Q&A budget constants are module-level

Per Decision 1: this module MUST NOT import `LLMJudge` /
`LLMCoverageChecker` / `Judge` / `CoverageChecker` — Q&A's only stop
conditions are budget exhaustion and explicit cancellation.

Per Decision 2: Stage 1 (RAG-first probe) skips the ReAct loop when
`_hits_confident` returns True; Stage 2 ReAct loop is reused from
`agent.explorer` (`_execute_tools` / `_should_stop` semantics) but
with QA-specific prompts and stop conditions.
"""
from __future__ import annotations

import asyncio
import re
from datetime import datetime, timezone
from typing import Any

from codebus_agent.agent.emitter import NullEmitter, SSEEmitter
from codebus_agent.agent.explorer import (
    _append_observations,
    _execute_tools,
    _to_provider_messages,
)
from codebus_agent.agent.prompts.qa import (
    QA_PROMPT_VERSION,
    QA_SYSTEM,
    render_qa_prompt,
)
from codebus_agent.agent.reasoning_logger import ReasoningLogger
from codebus_agent.agent.types import (
    KBCitation,
    Message,
    QAAction,
    QAAnswer,
    QAState,
    Step,
    ToolCall,
    ToolResult,
)
from codebus_agent.providers.protocol import Message as ProviderMessage


__all__ = [
    "_QA_DEDUP_THRESHOLD",
    "_QA_MAX_ADD_TO_KB_PER_QUESTION",
    "_QA_MAX_ADD_TO_KB_PER_SESSION",
    "_QA_MAX_CHUNK_SIZE_CHARS",
    "_QA_MAX_STEPS",
    "_answer_from_hits",
    "_hits_confident",
    "_significant_tokens",
    "_synthesize_answer",
    "run_qa",
]


# Budget constants — single source of truth for Q&A safety guards.
_QA_MAX_STEPS: int = 10
_QA_MAX_ADD_TO_KB_PER_SESSION: int = 20
_QA_MAX_CHUNK_SIZE_CHARS: int = 2000
_QA_MAX_ADD_TO_KB_PER_QUESTION: int = 5
_QA_DEDUP_THRESHOLD: float = 0.95

# RAG-first probe thresholds (Decision 2; baseline values from
# `docs/qa-agent.md §四`). Tunable on golden replay calibration.
_HITS_CONFIDENT_TOP1: float = 0.75
_HITS_CONFIDENT_TOP3_MEAN: float = 0.65
_HITS_CONFIDENT_MIN_HITS: int = 3
_INITIAL_PROBE_TOP_K: int = 8
_MESSAGE_ROLLING_WINDOW: int = 16

# Stop-word set used by `_significant_tokens` so common particles do not
# false-trigger entity-coverage. zh-TW + en-US small overlap is fine —
# the goal is reducing noise, not covering every language.
_STOPWORDS: frozenset[str] = frozenset(
    {
        # English
        "a", "an", "the", "is", "are", "was", "were", "be", "been", "being",
        "of", "in", "on", "at", "to", "for", "with", "by", "from", "as",
        "and", "or", "but", "if", "then", "this", "that", "these", "those",
        "it", "its", "i", "you", "he", "she", "we", "they",
        "what", "when", "where", "why", "how", "do", "does", "did",
        # zh-TW common particles
        "的", "是", "在", "和", "了", "嗎", "呢", "吧", "啊",
    }
)

_TOKEN_SPLIT_RE = re.compile(r"[^\w]+")


def _significant_tokens(text: str) -> set[str]:
    """Lowercase + split-on-non-alphanumeric → drop short / stop-word tokens."""
    if not text:
        return set()
    raw = _TOKEN_SPLIT_RE.split(text.lower())
    return {t for t in raw if len(t) >= 2 and t not in _STOPWORDS}


def _hits_confident(question: str, hits: list[Any]) -> bool:
    """Three-condition gate for the RAG-first cheap path.

    True iff:
      1. `hits[0].score > 0.75`
      2. mean of top-3 scores > 0.65
      3. at least one significant token from the question appears in
         the union of top-5 hit texts
    """
    if len(hits) < _HITS_CONFIDENT_MIN_HITS:
        return False
    if hits[0].score <= _HITS_CONFIDENT_TOP1:
        return False
    top3 = hits[:3]
    mean = sum(h.score for h in top3) / len(top3)
    if mean <= _HITS_CONFIDENT_TOP3_MEAN:
        return False
    question_tokens = _significant_tokens(question)
    if not question_tokens:
        # No discriminative token in the question → entity-coverage cannot
        # confirm; force the loop path so the Agent can clarify via tools.
        return False
    union: set[str] = set()
    for h in hits[:5]:
        union |= _significant_tokens(h.payload.text or "")
    return bool(question_tokens & union)


def _hits_to_citations(hits: list[Any]) -> list[KBCitation]:
    citations: list[KBCitation] = []
    for h in hits:
        payload = h.payload
        if not payload.file_path:
            continue
        citations.append(
            KBCitation(
                file_path=payload.file_path,
                line_start=payload.line_start or 0,
                line_end=payload.line_end or 0,
                related_stations=list(payload.related_stations),
            )
        )
    return citations


async def _answer_from_hits(
    question: str, hits: list[Any], provider: Any
) -> QAAnswer:
    """Cheap-path answer synthesis — single chat call grounded on RAG hits."""
    bullets = []
    for h in hits[:5]:
        payload = h.payload
        snippet = (payload.text or "").replace("\n", " ")[:200]
        bullets.append(f"- {payload.file_path}:{payload.line_start}: {snippet}")
    grounding = "\n".join(bullets) or "（無命中）"
    user_prompt = (
        f"使用者問題：{question}\n\n"
        f"以下是 KB 檢索到的相關片段：\n{grounding}\n\n"
        f"請以這些片段為依據，產出簡潔回答。"
    )
    messages = [
        ProviderMessage(role="system", content=QA_SYSTEM),
        ProviderMessage(role="user", content=user_prompt),
    ]
    answer_model = await provider.chat(messages, response_model=QAAnswer)
    if isinstance(answer_model, QAAnswer):
        # Override citations with the actual hits to ensure they're grounded.
        return QAAnswer(answer=answer_model.answer, citations=_hits_to_citations(hits))
    # Defensive fallback — should never trigger because Instructor validates.
    return QAAnswer(answer=str(answer_model), citations=_hits_to_citations(hits))


async def _qa_think(
    state: QAState,
    provider: Any,
    user_prompt: str,
) -> tuple[str, list[ToolCall]]:
    """Single chat call producing the next Q&A action.

    Mirrors `explorer._think`'s shape but with QA prompts and the
    `QAAction` response model.
    """
    windowed = state.messages[-_MESSAGE_ROLLING_WINDOW:]
    messages = _to_provider_messages(windowed) + [
        ProviderMessage(role="system", content=QA_SYSTEM),
        ProviderMessage(role="user", content=user_prompt),
    ]
    action = await provider.chat(messages, response_model=QAAction)
    if isinstance(action, QAAction):
        return action.thought, action.tool_calls
    return str(action), []


def _qa_should_stop(state: QAState, cancel_event: asyncio.Event | None) -> tuple[bool, str | None]:
    """Q&A stop condition — cancel > step limit > pending-queue empty.

    Note: Q&A pending_queue is always empty (Q&A doesn't enqueue
    follow-up exploration targets), so the "queue empty" branch fires
    immediately when no pending tool calls are emitted. This is the
    natural convergence path: the Agent stops calling tools when it
    has enough information.
    """
    if cancel_event is not None and cancel_event.is_set():
        return True, "cancelled"
    if state.step_count >= _QA_MAX_STEPS:
        return True, "budget_exhausted"
    return False, None


async def _synthesize_answer(
    state: QAState,
    initial_hits: list[Any],
    provider: Any,
    question: str,
) -> QAAnswer:
    """Final answer synthesis — runs after ReAct loop convergence."""
    # Compose what the Agent has gathered (`messages` carries thoughts +
    # tool observations) and ask the provider for a final answer.
    gathered: list[str] = []
    for msg in state.messages[-_MESSAGE_ROLLING_WINDOW:]:
        if msg.role == "tool":
            gathered.append(f"[{msg.tool_name}] {msg.content[:200]}")
        elif msg.role == "assistant":
            gathered.append(f"[think] {msg.content[:200]}")
    summary = "\n".join(gathered) or "（無收集到的觀察）"
    user_prompt = (
        f"使用者問題：{question}\n\n"
        f"已收集的觀察：\n{summary}\n\n"
        f"請整合上述觀察，產出最終回答。"
        f"若資訊不足，請明確說明哪些檔案 / 位置可以補充。"
    )
    messages = [
        ProviderMessage(role="system", content=QA_SYSTEM),
        ProviderMessage(role="user", content=user_prompt),
    ]
    answer_model = await provider.chat(messages, response_model=QAAnswer)
    if isinstance(answer_model, QAAnswer):
        if not answer_model.citations and initial_hits:
            return QAAnswer(
                answer=answer_model.answer,
                citations=_hits_to_citations(initial_hits),
            )
        return answer_model
    return QAAnswer(answer=str(answer_model), citations=_hits_to_citations(initial_hits))


async def run_qa(
    *,
    question: str,
    state: QAState,
    kb: Any,
    tools: Any,
    provider: Any,
    logger: ReasoningLogger | None = None,
    emitter: SSEEmitter | None = None,
    cancel_event: asyncio.Event | None = None,
) -> QAAnswer:
    """Drive the three-stage Q&A flow.

    Stage 1 — RAG-first probe: query KB once, evaluate `_hits_confident`.
    Stage 2 — Optional ReAct loop: only when probe returned False.
    Stage 3 — Synthesize final answer + citations.

    `logger`, `emitter`, `cancel_event` are optional; absent values
    use null defaults so unit tests can drive the loop without
    workspace path / SSE wiring.
    """
    _emitter: SSEEmitter = emitter or NullEmitter()

    # Stage 1: RAG-first probe.
    initial_hits = await kb.query(question, top_k=_INITIAL_PROBE_TOP_K)
    _emitter.emit(
        {
            "type": "rag_hits",
            "hits": [
                {
                    "score": float(h.score),
                    "file_path": h.payload.file_path or "",
                    "line_start": h.payload.line_start or 0,
                    "line_end": h.payload.line_end or 0,
                    "snippet": (h.payload.text or "")[:200],
                    "related_stations": list(h.payload.related_stations),
                }
                for h in initial_hits[:_INITIAL_PROBE_TOP_K]
            ],
        }
    )

    if _hits_confident(question, initial_hits):
        # Cheap path — single chat call, no Step writes, return early.
        answer = await _answer_from_hits(question, initial_hits, provider)
        _emitter.emit(
            {
                "type": "qa_answer",
                "answer": answer.answer,
                "citations": [c.model_dump(mode="json") for c in answer.citations],
            }
        )
        return answer

    # Stage 2: ReAct loop. Seed `state.messages` with the rendered prompt.
    state.messages.append(
        Message(
            role="user",
            content=render_qa_prompt(state, question, initial_hits),
        )
    )

    while True:
        should_stop, _stopped = _qa_should_stop(state, cancel_event)
        if should_stop:
            break

        iter_step = state.step_count
        user_prompt = render_qa_prompt(state, question, initial_hits=None)
        thought, tool_calls = await _qa_think(state, provider, user_prompt)
        _emitter.emit(
            {
                "type": "agent_thought",
                "step": iter_step,
                "thought": thought,
                "action": [c.model_dump() for c in tool_calls],
            }
        )

        # Empty tool_calls signals "Agent has enough info" — break out.
        if not tool_calls:
            if logger is not None:
                logger.write(
                    Step(
                        step=iter_step,
                        ts=datetime.now(timezone.utc),
                        thought=thought,
                        tool_calls=[],
                        tool_results=[],
                        judge_verdict=None,
                        tokens_used=0,
                    )
                )
            state.step_count += 1
            break

        results: list[ToolResult] = await _execute_tools(tool_calls, tools)
        for r in results:
            _emitter.emit(
                {
                    "type": "agent_action_result",
                    "step": iter_step,
                    "tool": r.tool_name,
                    "observation": (r.output or "")[:500],
                    "tokens_used": 0,
                }
            )

        _append_observations(state, tool_calls, results)

        if logger is not None:
            logger.write(
                Step(
                    step=iter_step,
                    ts=datetime.now(timezone.utc),
                    thought=thought,
                    tool_calls=list(tool_calls),
                    tool_results=list(results),
                    judge_verdict=None,
                    tokens_used=0,
                )
            )

        state.step_count += 1

    # Stage 3: synthesize final answer.
    answer = await _synthesize_answer(state, initial_hits, provider, question)
    _emitter.emit(
        {
            "type": "qa_answer",
            "answer": answer.answer,
            "citations": [c.model_dump(mode="json") for c in answer.citations],
        }
    )
    return answer
