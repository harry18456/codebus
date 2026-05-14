import { fireEvent, render, screen } from "@testing-library/react"
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest"

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
}))
vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }))

import type { RunDetail } from "@/lib/ipc"
import { useWikiStore } from "@/store/wiki"
import { RunDetailDone } from "./RunDetailDone"

function makeDetail(over: Partial<RunDetail> = {}): RunDetail {
  return {
    summary: {
      run_id: "r1",
      mode: "goal",
      goal: "describe X",
      started_at: "2026-05-13T10:00:00Z",
      finished_at: "2026-05-13T10:02:30Z",
      tokens: { input_tokens: 100, output_tokens: 50 },
      wiki_changed: true,
      lint_error_count: 0,
      lint_warn_count: 1,
      outcome: "succeeded",
    },
    events: [],
    ...over,
  }
}

describe("RunDetailDone", () => {
  beforeEach(() => {
    useWikiStore.setState({
      pages: {},
      currentPath: null,
      body: null,
      _bodyCache: {},
    })
  })

  afterEach(() => {
    useWikiStore.setState({
      pages: {},
      currentPath: null,
      body: null,
      _bodyCache: {},
    })
  })

  it("RunDetailDone_covered_page_renders_title_when_known", () => {
    useWikiStore.setState({
      pages: {
        auth: {
          slug: "auth",
          path: "/v/.codebus/wiki/modules/auth.md",
          title: "Authentication Module",
        },
      },
      currentPath: null,
      body: null,
      _bodyCache: {},
    })
    const detail = makeDetail({
      events: [
        spawnStartEvent("goal"),
        toolUseEvent("Write", { file_path: "wiki/modules/auth.md" }),
        spawnEndEvent("goal"),
      ],
    })
    render(
      <RunDetailDone
        detail={detail}
        onBack={() => {}}
        onSelectPage={() => {}}
      />,
    )
    const btn = screen.getByTestId("covered-page-auth")
    expect(btn).toHaveTextContent("Authentication Module")
    expect(btn.textContent).not.toContain("[[")
    expect(btn.getAttribute("data-slug")).toBe("auth")
  })

  it("RunDetailDone_covered_page_falls_back_to_slug_when_title_unknown", () => {
    // pages map empty — wiki not yet refreshed for this slug.
    const detail = makeDetail({
      events: [
        spawnStartEvent("goal"),
        toolUseEvent("Write", { file_path: "wiki/modules/auth.md" }),
        spawnEndEvent("goal"),
      ],
    })
    render(
      <RunDetailDone
        detail={detail}
        onBack={() => {}}
        onSelectPage={() => {}}
      />,
    )
    expect(screen.getByTestId("covered-page-auth")).toHaveTextContent("auth")
  })

  it("RunDetailDone_lists_covered_pages_from_events", () => {
    const detail = makeDetail({
      events: [
        spawnStartEvent("goal"),
        toolUseEvent("Write", { file_path: "wiki/modules/auth.md" }),
        toolUseEvent("Edit", { file_path: "wiki/index.md" }),
        spawnEndEvent("goal"),
      ],
    })
    render(
      <RunDetailDone
        detail={detail}
        onBack={() => {}}
        onSelectPage={() => {}}
      />,
    )
    expect(screen.getByTestId("covered-page-auth")).toBeInTheDocument()
    expect(screen.getByTestId("covered-page-index")).toBeInTheDocument()
  })

  it("RunDetailDone_covered_page_click_switches_to_wiki_tab", () => {
    const onSelectPage = vi.fn()
    const detail = makeDetail({
      events: [
        spawnStartEvent("goal"),
        toolUseEvent("Write", { file_path: "wiki/modules/auth.md" }),
        spawnEndEvent("goal"),
      ],
    })
    render(
      <RunDetailDone
        detail={detail}
        onBack={() => {}}
        onSelectPage={onSelectPage}
      />,
    )
    fireEvent.click(screen.getByTestId("covered-page-auth"))
    expect(onSelectPage).toHaveBeenCalledWith("auth")
  })

  it("renders empty hint when no wiki Write/Edit events", () => {
    render(
      <RunDetailDone
        detail={makeDetail()}
        onBack={() => {}}
        onSelectPage={() => {}}
      />,
    )
    expect(screen.getByText("No wiki pages changed")).toBeInTheDocument()
  })

  it("activity_summary_groups_tool_counts_by_verb_phase", () => {
    // Spec scenario: goal phase has 12 Read; fix phase has 2 Bash + 2 Write.
    const events = [
      spawnStartEvent("goal"),
      ...Array.from({ length: 12 }, () => toolUseEvent("Read")),
      spawnEndEvent("goal"),
      spawnStartEvent("fix"),
      ...Array.from({ length: 2 }, () => toolUseEvent("Bash", { command: "codebus lint" })),
      ...Array.from({ length: 2 }, () => toolUseEvent("Write", { file_path: "wiki/x.md" })),
      spawnEndEvent("fix"),
    ]
    render(
      <RunDetailDone
        detail={makeDetail({ events })}
        onBack={() => {}}
        onSelectPage={() => {}}
      />,
    )
    // goal phase heading present + 12 Read row only.
    expect(screen.getByTestId("activity-phase-goal")).toBeInTheDocument()
    expect(screen.getByTestId("activity-summary-goal-Read")).toHaveTextContent("12 Read")
    expect(screen.queryByTestId("activity-summary-goal-Write")).toBeNull()
    // fix phase heading present + 2 Bash + 2 Write rows.
    expect(screen.getByTestId("activity-phase-fix")).toBeInTheDocument()
    expect(screen.getByTestId("activity-summary-fix-Bash")).toHaveTextContent("2 Bash")
    expect(screen.getByTestId("activity-summary-fix-Write")).toHaveTextContent("2 Write")
    expect(screen.queryByTestId("activity-summary-fix-Read")).toBeNull()
  })

  it("activity_summary_phase_with_zero_tool_uses_renders_empty_hint", () => {
    // goal agent ran but invoked no tools (e.g., judged goal out-of-scope).
    const events = [spawnStartEvent("goal"), spawnEndEvent("goal")]
    render(
      <RunDetailDone
        detail={makeDetail({ events })}
        onBack={() => {}}
        onSelectPage={() => {}}
      />,
    )
    expect(screen.getByTestId("activity-phase-goal")).toBeInTheDocument()
    // Empty-hint text from i18n key (en bundle).
    expect(screen.getByTestId("activity-summary")).toHaveTextContent(
      "(no tools used)",
    )
  })

  it("covered_pages_groups_slugs_by_writing_phase", () => {
    const events = [
      spawnStartEvent("goal"),
      toolUseEvent("Write", { file_path: "wiki/modules/auth.md" }),
      spawnEndEvent("goal"),
      spawnStartEvent("fix"),
      toolUseEvent("Write", { file_path: "wiki/index.md" }),
      toolUseEvent("Write", { file_path: "wiki/log.md" }),
      spawnEndEvent("fix"),
    ]
    render(
      <RunDetailDone
        detail={makeDetail({ events })}
        onBack={() => {}}
        onSelectPage={() => {}}
      />,
    )
    expect(screen.getByTestId("covered-phase-goal")).toBeInTheDocument()
    expect(screen.getByTestId("covered-phase-fix")).toBeInTheDocument()
    // [[auth]] under goal phase, [[index]] and [[log]] under fix phase.
    expect(screen.getByTestId("covered-page-auth")).toBeInTheDocument()
    expect(screen.getByTestId("covered-page-index")).toBeInTheDocument()
    expect(screen.getByTestId("covered-page-log")).toBeInTheDocument()
  })

  it("run_details_block_collapsed_by_default_then_expands_full_replay_with_inline_thoughts", () => {
    const events = [
      spawnStartEvent("goal"),
      toolUseEvent("Read"),
      toolUseEvent("Glob", { pattern: "*.md" }),
      thoughtEvent("hmm..."),
      spawnEndEvent("goal"),
    ]
    render(
      <RunDetailDone
        detail={makeDetail({ events })}
        onBack={() => {}}
        onSelectPage={() => {}}
      />,
    )
    expect(screen.queryByTestId("run-details-block")).toBeNull()
    fireEvent.click(screen.getByTestId("run-details-toggle"))
    const block = screen.getByTestId("run-details-block")
    const toolUseItems = block.querySelectorAll('[data-testid="stream-tool-use"]')
    expect(toolUseItems.length).toBe(2)
    const thoughtItems = block.querySelectorAll('[data-testid="thought-item"]')
    expect(thoughtItems.length).toBe(1)
    expect(thoughtItems[0].textContent).toContain("🤔")
    expect(thoughtItems[0].textContent).toContain("hmm...")
  })
})

function spawnStartEvent(verb: string): {
  ts: string
  event: { kind: "lifecycle"; data: { kind: "spawn_start"; verb: string } }
} {
  return {
    ts: "2026-05-13T10:00:00Z",
    event: { kind: "lifecycle", data: { kind: "spawn_start", verb } },
  }
}

function spawnEndEvent(verb: string): {
  ts: string
  event: {
    kind: "lifecycle"
    data: { kind: "spawn_end"; verb: string; exit_code: number | null }
  }
} {
  return {
    ts: "2026-05-13T10:00:00Z",
    event: {
      kind: "lifecycle",
      data: { kind: "spawn_end", verb, exit_code: 0 },
    },
  }
}

function toolUseEvent(
  name: string,
  input: Record<string, unknown> = {},
): { ts: string; event: { kind: "stream"; data: { kind: "tool_use"; name: string; input: unknown } } } {
  return {
    ts: "2026-05-13T10:00:00Z",
    event: { kind: "stream", data: { kind: "tool_use", name, input } },
  }
}

function thoughtEvent(text: string): {
  ts: string
  event: { kind: "stream"; data: { kind: "thought"; text: string } }
} {
  return {
    ts: "2026-05-13T10:00:00Z",
    event: { kind: "stream", data: { kind: "thought", text } },
  }
}
