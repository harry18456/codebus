"""Judge system prompt + render helper.

Backs `docs/agent-core.md` §七. The Judge is a one-shot relevance
verdict producer (``Judge evaluation runs as one-shot call per
iteration``). Kept intentionally narrow in P0: the Judge only scores
the most recent iteration's tool results; longer-horizon scoring lands
in a future change.
"""
from __future__ import annotations

import textwrap
from typing import TYPE_CHECKING

if TYPE_CHECKING:
    from ..types import ToolResult


__all__ = [
    "JUDGE_PROMPT_VERSION",
    "JUDGE_SYSTEM",
    "render_judge_prompt",
]


JUDGE_PROMPT_VERSION: str = "v0-p0"

JUDGE_SYSTEM: str = textwrap.dedent(
    """
    你是 codebase 探索結果的 Relevance Judge。給你任務描述與這一輪
    Explorer 剛看到的工具結果，你要回：

    - relevance (0..1)：這批結果對任務多有用
    - should_follow_imports：是否值得追這些檔案的 import / references
    - should_add_station：是否值得把它加進探索路線
    - reason：一句話解釋理由

    規則：
    - relevance 嚴格 0..1，超出 Instructor 會拒收
    - reason 要具體（「這是 KnowledgeBase 主類」比「很有用」好）
    - 不要重複敘述任務，只給判斷
    """
).strip()


def render_judge_prompt(task: str, results: list["ToolResult"]) -> str:
    """Render the per-iteration user message for the Judge."""
    if not results:
        result_block = "（Explorer 這輪沒有產生工具結果）"
    else:
        lines = []
        for r in results:
            status = "ERROR" if r.error else "OK"
            preview = r.output[:400].replace("\n", " ")
            lines.append(f"- [{status}] {r.tool_name}: {preview}")
        result_block = "\n".join(lines)
    return textwrap.dedent(
        f"""
        任務：{task}

        本輪工具結果：
        {result_block}

        回覆 JSON：{{"relevance": float, "should_follow_imports": bool,
        "should_add_station": bool, "reason": "..."}}
        """
    ).strip()
