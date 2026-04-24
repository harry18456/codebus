"""Coverage Checker system prompt + render helper.

Backs SHALL clauses in
openspec/changes/coverage-gap-recurse/specs/agent-core/spec.md
  Requirement: LLMCoverageChecker produces one-shot CoverageResult

Coverage Checker is the companion one-shot evaluator to Judge: it runs
exactly once after the Explorer main while loop converges (budget
exhausted / queue empty / cancel), inspects the accumulated state, and
returns a list of `Gap` entries the Agent should follow up on in a
recursive round. The prompt is three-section (role bounds / Gap
criteria / output format) to mirror `JUDGE_SYSTEM`'s shape so a reader
can pattern-match between the two evaluators.

Rendering mirrors `render_judge_prompt`:
- visited-files window caps at 20 entries + `... (N more)` footer so the
  prompt stays bounded on large repos (design Risk mitigation 2 —
  `render_coverage_prompt ... bounded visited rendering`).
- stations rendered in full so Coverage can reason about what's already
  covered vs. what's still open.
"""
from __future__ import annotations

import textwrap
from typing import TYPE_CHECKING

if TYPE_CHECKING:
    from ..types import ExplorerState


__all__ = [
    "COVERAGE_PROMPT_VERSION",
    "COVERAGE_SYSTEM",
    "render_coverage_prompt",
]


COVERAGE_PROMPT_VERSION: str = "2026-04-26-1"

_VISITED_WINDOW: int = 20

COVERAGE_SYSTEM: str = textwrap.dedent(
    """
    你是 codebase 探索結果的 Coverage Checker。你在 Explorer ReAct 主
    迴圈**收斂後**跑一次，看一眼 stations + visited_files 是否遺漏任
    務關鍵路徑，回一組 Gap 讓 Agent 決定是否補查。下面三段是你的唯一
    角色定義與判準，**不要越界**。

    ## 1. 角色邊界（role bounds）
    - 你是 one-shot 補查評估器：每次 Explorer run 結束前只用一次。
    - 不進 ReAct 子迴圈：你不會有第二輪輸入；這一次回答就是全部。
    - 不呼叫工具：Explorer 的工具（search / read_file / list_dir /
      mark_station / trace_import / find_callers）與你無關。
    - 不改 state：visited_files / stations / pending_queue 由
      Explorer 的 Update 步驟 / 遞迴層寫入；你的職責是「指出 gap」，
      不是「變更」。

    ## 2. Gap 判準（何時回一個 Gap）
    - **要回**：stations 收斂點明顯遺漏任務關鍵路徑（例如主路由尚
      未追進去、跨模組 Adapter 介面未收錄、核心資料流缺一段）。
    - **不要回**：stations 已覆蓋任務骨架、剩下只是雜訊或已 visited
      的檔案、或 stations 明顯已足以給下游 Generator 產章節骨架。
    - 寧缺勿濫：**空 gaps 是合法輸出**；寧可 0 gap 收斂乾淨，也不要
      為了湊 gap 把無關檔案推下去，會浪費 Agent 剩餘 budget。

    ## 3. 輸出格式
    回覆 JSON `{"gaps": [{"description": str, "suggested_target":
    str | null}, ...]}`。
    - `description`：一句話描述缺了什麼（具體，例：「Adapter 介面
      未被追蹤，影響 Storage 路徑閉合」勝過「還有些地方沒看」）。
    - `suggested_target`：若你能指出明確的檔案路徑或 symbol，填
      之；不確定就填 null，Agent 會用 description 前 80 字當查詢
      鍵。
    - gaps 陣列可為空；代表「查無 gap，乾淨收斂」。
    """
).strip()


def render_coverage_prompt(state: "ExplorerState") -> str:
    """Render the post-convergence user message for the Coverage Checker.

    Accepts the full `ExplorerState` (mirroring `render_judge_prompt`):
    the rendered prompt shows the task, current stations summary, and a
    bounded visited window so the Checker sees what Explorer converged
    on. Determinism matters — visited files sort lexicographically so
    re-runs produce bit-identical output (this is what
    `test_render_coverage_prompt_is_deterministic_on_sorted_state`
    pins).
    """
    visited_block = _render_visited(state)
    stations_block = _render_stations(state)
    return textwrap.dedent(
        f"""
        任務：{state.task}

        當前 stations：
        {stations_block}

        當前 visited 摘要：
        {visited_block}

        依 COVERAGE_SYSTEM 的三段判準回 JSON（`{{"gaps": [...]}}`）。
        若無 gap，回 `{{"gaps": []}}`。
        """
    ).strip()


def _render_visited(state: "ExplorerState") -> str:
    visited = sorted(state.visited_files)
    total = len(visited)
    if total == 0:
        return "visited 檔案數=0（尚未探索）"
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
        return "stations 數=0（Explorer 未收斂出任何站點）"
    lines = [f"stations 數={total}"]
    for st in stations:
        lines.append(f"- role={st.role} path={st.path}")
    return "\n".join(lines)
