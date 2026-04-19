"""Backs SHALL clauses in
openspec/changes/m1-power-on/specs/usage-tracking/spec.md
  Requirement: LLMCallLogger writes llm_calls.jsonl
    Scenario: Request and response captured
    Scenario: Sanitizer-ready field reserved
    Scenario: Failure still logged
"""
from __future__ import annotations

import json
from pathlib import Path

from codebus_agent.providers.llm_call_logger import LLMCallLogger


def _read_lines(path: Path) -> list[dict]:
    return [json.loads(line) for line in path.read_text(encoding="utf-8").splitlines()]


def test_log_captures_request_and_response(tmp_path: Path) -> None:
    """Scenario: Request and response captured."""
    jsonl = tmp_path / "llm_calls.jsonl"
    logger = LLMCallLogger(jsonl)

    request = {"messages": [{"role": "user", "content": "hi"}]}
    response = {"content": "hello"}
    logger.log(request=request, response=response)

    entry = _read_lines(jsonl)[0]
    assert entry["request"] == request
    assert entry["response"] == response


def test_sanitizer_pass2_applied_defaults_false(tmp_path: Path) -> None:
    """Scenario: Sanitizer-ready field reserved (M1 MUST be false)."""
    jsonl = tmp_path / "llm_calls.jsonl"
    logger = LLMCallLogger(jsonl)

    logger.log(request={}, response={})

    entry = _read_lines(jsonl)[0]
    assert "sanitizer_pass2_applied" in entry
    assert entry["sanitizer_pass2_applied"] is False


def test_failure_writes_null_response_and_error(tmp_path: Path) -> None:
    """Scenario: Failure still logged."""
    jsonl = tmp_path / "llm_calls.jsonl"
    logger = LLMCallLogger(jsonl)

    try:
        raise RuntimeError("upstream 500")
    except RuntimeError as exc:
        logger.log_failure(request={"messages": []}, exception=exc)

    entry = _read_lines(jsonl)[0]
    assert entry["response"] is None
    assert entry["error"]["class"] == "RuntimeError"
    assert entry["error"]["message"] == "upstream 500"


def test_timestamp_field_present(tmp_path: Path) -> None:
    jsonl = tmp_path / "llm_calls.jsonl"
    logger = LLMCallLogger(jsonl)

    logger.log(request={}, response={})

    entry = _read_lines(jsonl)[0]
    assert isinstance(entry["timestamp"], str) and entry["timestamp"]


def test_multiple_calls_appended(tmp_path: Path) -> None:
    jsonl = tmp_path / "llm_calls.jsonl"
    logger = LLMCallLogger(jsonl)

    logger.log(request={"a": 1}, response={"b": 2})
    logger.log(request={"a": 3}, response={"b": 4})

    lines = _read_lines(jsonl)
    assert len(lines) == 2
    assert lines[0]["request"]["a"] == 1
    assert lines[1]["request"]["a"] == 3
