"""RED tests for LLMCallLogger SSE emit (agent-sse-wiring §8).

Backs openspec/changes/agent-sse-wiring/specs/explorer-sse/spec.md
  Requirement: LLMCallLogger emits llm_call event carrying preview
"""
from __future__ import annotations

import json
from pathlib import Path

from codebus_agent.providers.llm_call_logger import LLMCallLogger
from codebus_agent.providers.protocol import ProviderRole


class _SpyEmitter:
    def __init__(self) -> None:
        self.events: list[dict] = []

    def emit(self, event: dict) -> None:
        self.events.append(event)


def _read_lines(path: Path) -> list[dict]:
    return [
        json.loads(line) for line in path.read_text(encoding="utf-8").splitlines()
    ]


def _log_kwargs(**overrides: object) -> dict:
    base: dict = {
        "role": ProviderRole.JUDGE,
        "provider_id": "mock",
        "model": "mock-judge-v1",
        "prompt_tokens": 4,
        "completion_tokens": 8,
    }
    base.update(overrides)
    return base


def test_successful_call_emits_llm_call_event(tmp_path: Path) -> None:
    """Spec scenario `Successful call emits llm_call event`."""
    spy = _SpyEmitter()
    jsonl = tmp_path / "llm_calls.jsonl"
    logger = LLMCallLogger(jsonl, emitter=spy)

    request = {
        "messages": [
            {"role": "system", "content": "you are helpful"},
            {"role": "user", "content": "x" * 400},
        ]
    }
    response = {"content": "hello"}
    logger.log(
        request=request,
        response=response,
        module="judge",
        request_id="req_abc",
        **_log_kwargs(),
    )

    assert len(spy.events) == 1
    evt = spy.events[0]
    assert evt["type"] == "llm_call"
    # `preview` MUST be ≤ 200 chars and drawn from the first user message.
    assert len(evt["preview"]) <= 200
    assert evt["preview"].startswith("xxxx")
    # request_id / module are present on every record.
    assert evt["request_id"] == "req_abc"
    assert evt["module"] == "judge"

    # wire-log parity — the jsonl line is still produced.
    lines = _read_lines(jsonl)
    assert len(lines) == 1
    assert lines[0]["response"] == response


def test_failed_call_still_emits_llm_call_event(tmp_path: Path) -> None:
    """Spec scenario `Failed call still emits llm_call event`."""
    spy = _SpyEmitter()
    jsonl = tmp_path / "llm_calls.jsonl"
    logger = LLMCallLogger(jsonl, emitter=spy)

    try:
        raise RuntimeError("upstream-500")
    except RuntimeError as exc:
        logger.log_failure(
            request={"messages": [{"role": "user", "content": "hi"}]},
            exception=exc,
            module="reasoning",
            request_id="req_failure",
            **_log_kwargs(role=ProviderRole.REASONING),
        )

    assert len(spy.events) == 1
    evt = spy.events[0]
    assert evt["type"] == "llm_call"
    # request_id / module / model MUST be present on failure records.
    assert evt["request_id"] == "req_failure"
    assert evt["module"] == "reasoning"
    assert evt["model"] == "mock-judge-v1"


def test_omitted_emitter_preserves_file_only_behavior(tmp_path: Path) -> None:
    """Spec scenario `Omitted emitter preserves file-only behavior`."""
    jsonl = tmp_path / "llm_calls.jsonl"
    logger = LLMCallLogger(jsonl)  # no emitter

    logger.log(request={"messages": []}, response={"x": 1}, **_log_kwargs())

    # No crash, jsonl written as before.
    lines = _read_lines(jsonl)
    assert len(lines) == 1
    assert lines[0]["response"] == {"x": 1}
