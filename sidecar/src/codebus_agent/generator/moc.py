"""MOC (Map-of-Content) ``tutorial.md`` assembler.

Backs Requirement
``MOC assembler writes pure-index tutorial.md with standard markdown links``
in `openspec/changes/module-5-generator-p0/specs/module-5-generator/spec.md`.

The MOC is a pure index:
  - H1 task heading + locale suffix
  - 目標 / 預估時長 / 產出時間 / Repo 區塊
  - 路線總覽 (numbered station list, standard markdown links — never wikilinks)
  - 🎯 下車（完成）heading
  - interactive mode: ``<QAEntry>`` element for Module 8 entry
  - plain mode: literal Q&A sentence (no custom tags — D-029 §六)

The MOC MUST NOT duplicate station body content (D-029 §十六.3).
"""
from __future__ import annotations

from datetime import datetime
from pathlib import Path
from typing import Literal

from .types import StationSummary

__all__ = ["assemble_moc"]


_LOCALE_SUFFIX = " — CodeBus 學習教材"
_PLAIN_QA_SENTENCE = "本專案有 Q&A 功能可對話式繼續學習。"


def assemble_moc(
    *,
    task: str,
    total_minutes: int,
    generated_at: datetime,
    workspace_name: str,
    station_summaries: list[StationSummary],
    mode: Literal["interactive", "plain"],
    output_path: Path,
) -> None:
    """Render and write the MOC ``tutorial.md`` file.

    ``output_path`` is the absolute path
    ``<workspace_root>/codebus-tutorials/{task_id}/tutorial.md``;
    parent dirs are created with ``parents=True, exist_ok=True``.
    """
    output_path.parent.mkdir(parents=True, exist_ok=True)

    lines: list[str] = []
    lines.append(f"# {task}{_LOCALE_SUFFIX}")
    lines.append("")
    lines.append(f"> 目標：{task}")
    lines.append(f">")
    lines.append(f"> 預估時長：{total_minutes} 分鐘")
    lines.append(f">")
    lines.append(f"> 產出時間：{generated_at.isoformat()}")
    lines.append(f">")
    lines.append(f"> Repo：{workspace_name}")
    lines.append("")
    lines.append("## 🚌 路線總覽")
    lines.append("")
    for idx, summary in enumerate(station_summaries, start=1):
        link = f"./stations/{summary.station_id}.md"
        lines.append(
            f"{idx}. 🚏 [{summary.title}]({link})（{summary.duration} min）"
        )
    lines.append("")
    lines.append("---")
    lines.append("")
    lines.append("## 🎯 下車（完成）")
    lines.append("")
    if mode == "interactive":
        lines.append(
            "<QAEntry prompt=\"整條路線我最想再追一下的是：\">"
            "繼續問 Q&A Agent</QAEntry>"
        )
    else:
        lines.append(_PLAIN_QA_SENTENCE)
    lines.append("")

    output_path.write_text("\n".join(lines), encoding="utf-8")
