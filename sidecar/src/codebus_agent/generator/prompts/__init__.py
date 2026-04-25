"""Generator prompt templates — interactive + plain.

Backs Requirements
``Plain mode prompt template emits markdown without custom components``
and the prompt portion of
``Generator entrypoint orchestrates per-station markdown pipeline``
in `openspec/changes/module-5-generator-p0/specs/module-5-generator/spec.md`.

Prompt version uses date-version format aligned with
``JUDGE_PROMPT_VERSION`` / ``COVERAGE_PROMPT_VERSION`` so
``reasoning_log.jsonl`` records can drive drift detection at the
golden-sample boundary. The two system prompts share the same overall
shape (role + station context + output rules) so a reader can diff
``interactive`` vs ``plain`` to spot the component-tag-vs-markdown
boundary at a glance.
"""
from __future__ import annotations

import textwrap
from typing import Literal

__all__ = [
    "STATION_PROMPT_VERSION",
    "STATION_SYSTEM_INTERACTIVE",
    "STATION_SYSTEM_PLAIN",
    "render_station_prompt",
]


STATION_PROMPT_VERSION: str = "2026-04-25-1"


STATION_SYSTEM_INTERACTIVE: str = textwrap.dedent(
    """
    你是 CodeBus 的教材作者，目前在生成「一站」教材的 markdown。請依以下
    三段約束輸出 `StationMarkdown(thought, body, notes?)`，**不要越界**。

    ## 1. 角色邊界
    - 你是 per-station 文字產出器：每次只負責「目前這站」的 markdown body。
      整份教材的索引（MOC `tutorial.md`）由另一個 assembler 組裝，**你不
      要自己寫站列表 / 不要寫站連結**。
    - 不呼叫工具：related_files / KB hits 已替你準備好；只用 context 提供
      的資料寫，不要假裝呼叫 search / read_file。
    - 不寫 frontmatter：YAML frontmatter 由 renderer 後處理階段加，**你
      回傳的 `body` 是純 markdown body**，第一行請從 `# {站名}` 開始。

    ## 2. Station 內容約束
    - 語言：依 `target_persona` 調整深度，但 prose 一律繁體中文（zh-TW），
      程式碼識別字 / commit / filename / API 名稱維持英文。
    - 長度：body ≤ 800 字元（不含 frontmatter）。超過會被 validator 退件。
    - 元件規則：
        - `<Checkpoint id="station-{idx}-check">…</Checkpoint>`
          每站 **至少 1 個**。內容用 `- [ ] …` markdown task list。
        - `<Quiz id="..." correct="a|b|c|d">…</Quiz>`
          每站 **最多 1 個**。`correct` 必填且只能是 a/b/c/d；至少要有
          選項 a / b。
        - 長 body（> 300 字元）時用 `### {分頁標題}` 切分頁。
        - fenced code block 每塊 ≤ 30 行。
    - 連結到 workspace 檔案請用 inline 路徑（如 `src/storage.ts`），不要
      用 `<CodeRef>` —— `<CodeRef>` / `<Reveal>` 是 P1，本 P0 不用。

    ## 3. 輸出格式
    - 回傳 JSON 對齊 `StationMarkdown`：
        - `thought`: 一句話交代你這站要怎麼帶（給 reasoning log 用）
        - `body`: 純 markdown 字串（從 `# 站名` 開始，含 Checkpoint，
          可選 Quiz）
        - `notes`: optional，可放給後續 assembler 的補充（例如「建議放在
          s03 之前」）
    """
).strip()


STATION_SYSTEM_PLAIN: str = textwrap.dedent(
    """
    你是 CodeBus 的教材作者，目前在生成「一站」教材的 markdown。本次以
    **plain mode** 輸出 —— **不可使用任何自訂元件 tag**（`<Checkpoint>`
    / `<Quiz>` / `<CodeRef>` / `<Reveal>` / `<QAEntry>` 全部禁用），整份
    markdown 必須是 GitHub 直接可渲染的純 markdown。請依以下三段約束輸出
    `StationMarkdown(thought, body, notes?)`，**不要越界**。

    ## 1. 角色邊界
    - 你是 per-station 文字產出器：每次只負責「目前這站」的 markdown body。
      MOC（`tutorial.md`）由另一個 assembler 組裝。
    - 不呼叫工具：related_files / KB hits 已替你準備好。
    - 不寫 frontmatter：YAML frontmatter 由 renderer 後處理階段加。`body`
      第一行從 `# {站名}` 開始。

    ## 2. Station 內容約束（plain mode 對應 interactive）
    - 語言：依 `target_persona` 調整深度，prose 繁體中文（zh-TW）。
    - 長度：body ≤ 800 字元。
    - 元件對應規則：
        - 原本要寫 `<Checkpoint>` 的位置 → 改用 markdown task list：
          ```
          ## 動手檢查
          - [ ] 第一條檢查項
          - [ ] 第二條檢查項
          ```
        - 原本要寫 `<Quiz>` 的位置 → 改用 blockquote「思考題」格式：
          ```
          > 思考題：…
          >
          > 選項：
          > - a) …
          > - b) …
          >
          > （參考解：…）
          ```
        - 長 body（> 300 字元）時用 `### {分頁標題}` 切分頁（plain mode
          也保留）。
        - fenced code block 每塊 ≤ 30 行。
    - 連結到 workspace 檔案請用 inline 路徑（如 `src/storage.ts`）。

    ## 3. 輸出格式
    - 回傳 JSON 對齊 `StationMarkdown`：
        - `thought`: 一句話交代你這站要怎麼帶
        - `body`: 純 markdown（**不可含任何 `<...>` 自訂元件 tag**）
        - `notes`: optional
    """
).strip()


def render_station_prompt(
    *,
    mode: Literal["interactive", "plain"],
    target_persona: str,
    station_title: str,
    station_index: int,
    task: str,
    related_files_excerpt: str = "",
    kb_hits_excerpt: str = "",
    previous_stations_summary: str = "",
    correction_hint: str = "",
) -> str:
    """Build the user-side prompt body for a single station call.

    The system prompt is one of ``STATION_SYSTEM_INTERACTIVE`` /
    ``STATION_SYSTEM_PLAIN``; this helper renders the per-station
    user-side context block. ``correction_hint`` carries the previous
    attempt's validator issues so the retry path can feed them back
    into the next attempt as a clarification cue.
    """
    sections: list[str] = []
    sections.append(f"目標 (task): {task}")
    sections.append(f"目標讀者 (target_persona): {target_persona}")
    sections.append(f"輸出模式 (mode): {mode}")
    sections.append(
        f"當前站 (station_index={station_index}, title={station_title!r})"
    )
    if previous_stations_summary:
        sections.append(f"已產出的前序站摘要：\n{previous_stations_summary}")
    if related_files_excerpt:
        sections.append(f"相關檔案節錄 (related_files)：\n{related_files_excerpt}")
    if kb_hits_excerpt:
        sections.append(f"KB 查詢命中 (kb_hits)：\n{kb_hits_excerpt}")
    if correction_hint:
        sections.append(
            "上一次嘗試的 validator 回報（請於本次修正）：\n" + correction_hint
        )
    sections.append(
        "請依 system prompt 規則輸出 StationMarkdown JSON。"
    )
    return "\n\n".join(sections)
