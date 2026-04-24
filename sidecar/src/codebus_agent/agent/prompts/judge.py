"""Judge system prompt + render helper.

Backs `docs/agent-core.md` §七 + `openspec/changes/explorer-judge-golden/
specs/explorer-golden/spec.md`. The Judge is a one-shot relevance
verdict producer (``Judge evaluation runs as one-shot call per
iteration``). `explorer-judge-golden` upgraded the prompt from the
P0 placeholder into a three-section contract (role bounds / station
decision / follow-imports + relevance anchoring) and bumped the
version into date-version format so `reasoning_log.jsonl` records
can drive golden-sample drift detection.
"""
from __future__ import annotations

import textwrap
from typing import TYPE_CHECKING

if TYPE_CHECKING:
    from ..types import ExplorerState, ToolResult


__all__ = [
    "JUDGE_PROMPT_VERSION",
    "JUDGE_SYSTEM",
    "render_judge_prompt",
]


JUDGE_PROMPT_VERSION: str = "2026-04-25-1"

_VISITED_WINDOW: int = 20
_STATIONS_TAIL: int = 3
_TOOL_OUTPUT_TRUNCATE: int = 800
_TOOL_OUTPUT_MARKER: str = "…"

JUDGE_SYSTEM: str = textwrap.dedent(
    """
    你是 codebase 探索結果的 Relevance Judge。下面三段是你在 Explorer
    ReAct 迴圈裡唯一的角色定義與判準，**不要越界**。

    ## 1. 角色邊界（role bounds）
    - 你是 one-shot 評估器：每輪 Explorer loop 結束前用一次，只看這一
      輪的 ToolResult，回一個 JudgeVerdict。
    - 不進 ReAct 子迴圈：你不會有第二輪輸入；這一次回答就是全部。
    - 不呼叫工具：Explorer 的工具（search / read_file / list_dir /
      mark_station）與你無關，你拿到的就是它們的產出，別試圖模擬或
      要求再跑。
    - 不改 state：visited_files / stations / pending_queue 由 Explorer
      的 Update 步驟寫入；你的職責是「判斷」，不是「變更」。

    ## 2. Station 判準（should_add_station）
    - **true**：這輪 ToolResult 揭露了**新的架構切片 / entrypoint /
      協議邊界**，且與任務明確相關（例如主路由入口、跨模組的
      Provider/Adapter 介面、核心資料流的序列化邊界）。
    - **false**：純 import chain 或雜訊（只是把依賴鏈展開、沒有新的
      架構訊號）；或檔案已 visited（visited_files 已收錄）。
    - 站點要精簡：寧缺勿濫，每站都要能當成後續 Generator 的章節骨架。

    ## 3. Follow-imports + relevance anchoring
    - **should_follow_imports=true**：ToolResult 揭露了新的未探訪符號
      或檔案（尚未在 visited_files 中），且看起來跟任務相關，值得
      Explorer 下一輪追進去。
    - **should_follow_imports=false**：已 visited 或明顯不相關（工具
      腳本、產出檔、第三方範例程式碼）。
    - **relevance** 嚴格落在 `[0.0, 1.0]`，五檔錨（Instructor 會用這
      個 float 拒收範圍外值）：
        - `0.0` 無關 — 完全與任務無關
        - `0.3` 邊緣 — 有點關係但不是核心路徑
        - `0.5` 相關 — 中性佐證檔
        - `0.8` 核心 — 任務主幹上的關鍵切片
        - `1.0` entrypoint — 系統入口 / 最上層啟動面

    回覆 JSON：{"relevance": float, "should_follow_imports": bool,
    "should_add_station": bool, "reason": "..."}。reason 要具體（例如
    「這是 KnowledgeBase 主類，對應任務 KB 建構流程」勝過「很有用」）。
    """
).strip()


def render_judge_prompt(
    state: "ExplorerState", results: list["ToolResult"]
) -> str:
    """Render the per-iteration user message for the Judge.

    Accepts the full ``state`` (was ``task: str`` in P0) so the visited
    summary + stations tail land inside the Judge's context without
    extra argument plumbing. The rendered prompt stays deterministic:
    visited files sort lexicographically so re-runs produce stable
    diffs, and every ``ToolResult`` block renders either
    ``output=<≤800 chars>`` or ``error=<msg>`` but never both.
    """
    visited_block = _render_visited(state)
    stations_block = _render_stations(state)
    results_block = _render_results(results)
    return textwrap.dedent(
        f"""
        任務：{state.task}

        當前 visited 摘要：
        {visited_block}

        當前 stations 摘要：
        {stations_block}

        本輪工具結果：
        {results_block}

        依 JUDGE_SYSTEM 的三段判準回 JSON。
        """
    ).strip()


def _render_visited(state: "ExplorerState") -> str:
    visited = sorted(state.visited_files)
    total = len(visited)
    if total == 0:
        return f"visited 檔案數=0（尚未探索）"
    window = visited[:_VISITED_WINDOW]
    lines = [f"visited 檔案數={total}"]
    lines.extend(f"- {p}" for p in window)
    if total > _VISITED_WINDOW:
        lines.append(f"... ({total - _VISITED_WINDOW} more)")
    return "\n".join(lines)


def _render_stations(state: "ExplorerState") -> str:
    stations = state.stations
    total = len(stations)
    if total == 0:
        return "stations 數=0（尚未收斂出站點）"
    tail = stations[-_STATIONS_TAIL:]
    lines = [f"stations 數={total}（最近 {len(tail)} 條）"]
    for st in tail:
        lines.append(f"- role={st.role} path={st.path}")
    return "\n".join(lines)


def _render_results(results: list["ToolResult"]) -> str:
    if not results:
        return "（Explorer 這輪沒有產生工具結果）"
    lines: list[str] = []
    for r in results:
        if r.error:
            lines.append(f"- [{r.tool_name}] error={r.error}")
        else:
            body = r.output
            if len(body) > _TOOL_OUTPUT_TRUNCATE:
                body = body[:_TOOL_OUTPUT_TRUNCATE] + _TOOL_OUTPUT_MARKER
            lines.append(f"- [{r.tool_name}] output={body}")
    return "\n".join(lines)
