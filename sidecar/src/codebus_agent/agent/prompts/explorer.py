"""Explorer system prompt + render helper.

Backs `docs/agent-core.md` §八. The P0 prompt is intentionally small —
golden-sample quality tuning lands in `explorer-golden-sample-p0`. What
we pin here:

- ``EXPLORER_SYSTEM`` describes the Agent's job (zh-TW), enumerates
  tool-call discipline (think in Chinese, call tools in JSON) and the
  stop condition (set ``stop=True`` when exploration converges).
- ``EXPLORER_PROMPT_VERSION`` = ``"v0-p0"`` so every
  ``reasoning_log.jsonl`` line captures the prompt revision in play.
  Future prompt changes MUST bump this constant (e.g., ``"v1-p0"``) so
  replays and golden samples can align.
"""
from __future__ import annotations

import textwrap
from typing import TYPE_CHECKING

if TYPE_CHECKING:
    from ..types import ExplorerState


__all__ = [
    "EXPLORER_PROMPT_VERSION",
    "EXPLORER_SYSTEM",
    "render_explorer_prompt",
]


EXPLORER_PROMPT_VERSION: str = "v0-p0"

EXPLORER_SYSTEM: str = textwrap.dedent(
    """
    你是探索 codebase 的 Agent。每一輪你會：
    1. 觀察目前狀態（任務、已訪問檔案、可用工具）
    2. 產出一段思考（thought）說明你接下來要看哪裡以及為什麼
    3. 決定要呼叫哪些工具（tool_calls），或在收斂時設 stop=true

    規則：
    - thought 必須具體（不是「我要看看程式碼」；要是「我要讀 KnowledgeBase.build
      看它怎麼寫進 Qdrant」）
    - tool_calls 每個是 {"id": "tc_N", "name": "<tool>", "arguments": {...}}；
      沒事做就留空 list
    - 當任務涵蓋度足夠或預算用盡時設 stop=true
    - 不要自行解釋工具規格、不要假設工具輸出；看到結果後再說
    """
).strip()


def render_explorer_prompt(
    state: "ExplorerState",
    tool_specs: list[dict],
) -> str:
    """Render the per-iteration user message for the Think step.

    `tool_specs` is a list of ``{"name", "description", "parameters"}``
    dicts (shape from `agent-core.md` §六 ``ToolSpec``). We keep it
    loose here — the concrete tool-spec type is the follow-up
    ``explorer-tools-p0`` change's job.
    """
    tool_lines = "\n".join(
        f"- {spec.get('name', '?')}: {spec.get('description', '')}"
        for spec in tool_specs
    ) or "- （尚無可用工具）"
    return textwrap.dedent(
        f"""
        任務：{state.task}
        已訪問檔案數：{len(state.visited_files)}
        目前路線站數：{len(state.stations)}
        pending 隊列長度：{len(state.pending_queue)}
        step（從 0 起）：{state.step_count}
        剩餘預算：steps={state.budget_steps_left} tokens={state.budget_tokens_left}

        可用工具：
        {tool_lines}

        回覆 JSON：{{"thought": "...", "tool_calls": [...], "stop": bool}}
        """
    ).strip()
