"""Q&A Agent system prompt + render helper.

Backs SHALL clauses in
openspec/changes/module-8-qa-p0/specs/qa-agent/spec.md
  Requirement: Q&A system prompt module is isolated from Explorer prompts

Module-isolation invariant: this module MUST NOT import from
``prompts.explorer`` / ``prompts.judge`` / ``prompts.coverage`` so that
Folder-mode Explorer prompt vocabulary cannot leak into Q&A behavior
(per design Decision 1 and Cat 3 #3 review note).

Prompt content follows `docs/qa-agent.md §五` "值得沉澱" 三條件：
- 可復用（reusable）— 不是當下這次問題的偶發資訊
- stable fact — 不是隨環境而變的暫時狀態
- 非同義重複 — KB 中尚未存在相同概念的 chunk
"""
from __future__ import annotations

import textwrap
from typing import TYPE_CHECKING

if TYPE_CHECKING:
    from ..types import QAState
    from codebus_agent.kb.payload import KBHit


__all__ = [
    "QA_PROMPT_VERSION",
    "QA_SYSTEM",
    "render_qa_prompt",
]


QA_PROMPT_VERSION: str = "2026-04-26-1"


# Three-section system prompt: role/boundary, two-stage RAG flow, add_to_kb discipline.
QA_SYSTEM: str = textwrap.dedent(
    """
    你是 codebase 的 Q&A Agent。你的工作是回答使用者對既有教材或專案的提問，
    並在判斷答案值得沉澱時，主動把新知識寫入知識庫（KB）。

    ## 一、互動規則

    - 先以 RAG 命中為依據回答；若 KB 命中不足，可呼叫 search / read_file /
      list_dir / trace_import / find_callers / kb_search 補查
    - 每一輪回覆 JSON：{"thought": "...", "tool_calls": [...]}
    - 思考要具體（不是「我要看看 KB」；要是「我要查 PaymentService 的退款流程」）
    - 看到 snippet 含 `<REDACTED:*>` 時不可猜測原值，請標明該段已脫敏

    ## 二、值得沉澱（add_to_kb）三條件

    呼叫 add_to_kb 前，**每個 chunk 都必須同時滿足**：

    1. 可復用 — 不是當下這次問題的偶發資訊，是別人未來可能再問的 stable fact
    2. stable fact — 不是隨環境（路徑 / 帳號 / 時間戳）而變的暫時狀態
    3. 非同義重複 — KB 中尚未存在相同概念的 chunk（用 kb_search 先確認）

    任何一條不滿足，就不要寫入 KB；直接回答即可。

    ## 三、站台 id 格式

    `add_to_kb` 的 `related_stations` 與 `kb_search` 的 `station_filter`
    每個 id 都必須符合 regex `^s\\d{2}-[a-z0-9-]{1,40}(-\\d+)?$`
    （例：`s02-storage`、`s03-payment-flow-2`）。寫錯格式整次呼叫會被拒。

    ## 四、收斂

    - 認為答案足夠時，停止呼叫工具、輸出最終回答
    - budget（步數 / token / 時間）耗盡時 prompt 會額外提醒，請以已收集的
      資訊收斂回答，必要時建議使用者「請讀 X / Y 檔案」而不是繼續查
    """
).strip()


def render_qa_prompt(
    state: "QAState",
    question: str,
    initial_hits: "list[KBHit] | None" = None,
) -> str:
    """Render the per-iteration user message for the Q&A Think step.

    `initial_hits` is the result of the RAG-first probe — when present,
    we surface the top hits in the prompt so the LLM can ground its
    follow-up reasoning. Empty `initial_hits` is fine on later
    iterations (the Agent has already absorbed the initial context).
    """
    hits_lines = ""
    if initial_hits:
        rendered: list[str] = []
        for h in initial_hits[:5]:
            file_path = h.payload.file_path or "?"
            line_start = h.payload.line_start or 0
            score = h.score
            stations = ", ".join(h.payload.related_stations) or "（無）"
            snippet = (h.payload.text or "").replace("\n", " ")[:160]
            rendered.append(
                f"- {file_path}:{line_start} score={score:.2f} stations=[{stations}]\n  {snippet}"
            )
        hits_lines = "\n".join(rendered)
    else:
        hits_lines = "（無初始 RAG 命中或本輪無新命中）"

    originating = state.originating_station_id or "（未指定）"
    return textwrap.dedent(
        f"""
        使用者問題：{question}
        來源站台：{originating}
        目前 step（從 0 起）：{state.step_count}
        本 question 已 add_to_kb 次數：{state.add_to_kb_question_count}
        本 session 已 add_to_kb 次數：{state.add_to_kb_session_count}

        初始 RAG 命中：
        {hits_lines}

        回覆 JSON：{{"thought": "...", "tool_calls": [...]}}
        """
    ).strip()
