"""Prompt module re-exports — populated in Section 7 GREEN tasks."""
from __future__ import annotations

from .coverage import (
    COVERAGE_PROMPT_VERSION,
    COVERAGE_SYSTEM,
    render_coverage_prompt,
)
from .explorer import EXPLORER_PROMPT_VERSION, EXPLORER_SYSTEM, render_explorer_prompt
from .judge import JUDGE_PROMPT_VERSION, JUDGE_SYSTEM, render_judge_prompt
from .qa import QA_PROMPT_VERSION, QA_SYSTEM, render_qa_prompt

__all__ = [
    "COVERAGE_PROMPT_VERSION",
    "COVERAGE_SYSTEM",
    "EXPLORER_PROMPT_VERSION",
    "EXPLORER_SYSTEM",
    "JUDGE_PROMPT_VERSION",
    "JUDGE_SYSTEM",
    "QA_PROMPT_VERSION",
    "QA_SYSTEM",
    "render_coverage_prompt",
    "render_explorer_prompt",
    "render_judge_prompt",
    "render_qa_prompt",
]
