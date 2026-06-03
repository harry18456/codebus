import { fireEvent, render, screen, waitFor } from "@testing-library/react"
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest"

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
}))
vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }))
vi.mock("@/hooks/useLocale", () => ({
  useLocale: vi.fn(() => "en"),
}))

import { listen } from "@tauri-apps/api/event"
import type { RunLogSummary } from "@/lib/ipc"
import { useLocale } from "@/hooks/useLocale"
import { GoalsTab } from "./GoalsTab"
import { useGoalsStore } from "@/store/goals"

const mockedListen = vi.mocked(listen)
const mockedUseLocale = vi.mocked(useLocale)

function makeRun(id: string, startedAt: string): RunLogSummary {
  return {
    run_id: id,
    mode: "goal",
    goal: `goal ${id}`,
    started_at: startedAt,
    finished_at: startedAt,
    tokens: { input_tokens: 0, output_tokens: 0 },
    wiki_changed: false,
    lint_error_count: 0,
    lint_warn_count: 0,
    outcome: "succeeded",
  }
}

describe("GoalsTab", () => {
  beforeEach(() => {
    useGoalsStore.setState({ runs: [], activeRun: null })
    mockedUseLocale.mockReturnValue("en")
  })

  afterEach(() => {
    useGoalsStore.setState({ runs: [], activeRun: null })
  })

  it("GoalsTab_renders_runs_in_descending_started_at_order", () => {
    useGoalsStore.setState({
      runs: [
        makeRun("a", "2026-05-13T10:00:00Z"),
        makeRun("b", "2026-05-13T12:00:00Z"),
        makeRun("c", "2026-05-13T11:00:00Z"),
      ],
      activeRun: null,
    })
    render(<GoalsTab vaultPath="/v" onSelectRun={() => {}} />)
    const rows = screen.getAllByTestId(/^run-row-/)
    expect(rows.map((r) => r.dataset.testid)).toEqual([
      "run-row-b",
      "run-row-c",
      "run-row-a",
    ])
  })

  // Phase 4C content header row scenarios.

  it("GoalsTab_renders_content_header_row_with_cta_and_shortcut_chip", () => {
    useGoalsStore.setState({ runs: [], activeRun: null })
    render(<GoalsTab vaultPath="/v" onSelectRun={() => {}} />)
    const header = screen.getByTestId("tab-content-header-goals")
    expect(header).toBeInTheDocument()
    // h1 title from headerTitle key.
    expect(header.querySelector("h1")?.textContent).toBe("Goals")
    // Subtitle from headerSubtitle key.
    expect(header.querySelector("p")?.textContent).toContain(
      "List what you want to understand",
    )
    // CTA still present (now inside the header row).
    expect(screen.getByTestId("new-goal-button")).toHaveTextContent("+ New goal")
    // Shortcut chip with literal "N".
    const chip = header.querySelector("[data-tch-chip]")
    expect(chip).not.toBeNull()
    expect(chip?.textContent).toBe("N")
    // No legacy standalone right-aligned topbar (only the header row).
    expect(
      document.querySelectorAll("[data-tch-cta]").length,
    ).toBeGreaterThanOrEqual(1)
  })

  it("GoalsTab_populated_renders_RECENT_section_label_above_list", () => {
    useGoalsStore.setState({
      runs: [makeRun("a", "2026-05-13T10:00:00Z")],
      activeRun: null,
    })
    render(<GoalsTab vaultPath="/v" onSelectRun={() => {}} />)
    const recent = screen.getByText("RECENT")
    expect(recent).toBeInTheDocument()
    // Caps variant present on the SectionLabel root.
    expect(recent.classList.contains("section-label--caps")).toBe(true)
  })

  it("GoalsTab_populated_shows_persistent_quickstart_above_RECENT", () => {
    useGoalsStore.setState({
      runs: [makeRun("a", "2026-05-13T10:00:00Z")],
      activeRun: null,
    })
    render(<GoalsTab vaultPath="/v" onSelectRun={() => {}} />)

    // Quick-start region with its caps label + exactly four chips.
    const quickstart = screen.getByTestId("goals-quickstart")
    expect(quickstart).toBeInTheDocument()
    expect(quickstart).toHaveTextContent("Quick start")
    const chips = screen.getAllByTestId(/^goals-quickstart-chip-/)
    expect(chips).toHaveLength(4)
    expect(chips[0]).toHaveTextContent("describe what this project does")
    expect(chips[3]).toHaveTextContent("map the project structure")

    // Quick-start region renders above the RECENT section label.
    const recent = screen.getByText("RECENT")
    expect(
      quickstart.compareDocumentPosition(recent) &
        Node.DOCUMENT_POSITION_FOLLOWING,
    ).toBeTruthy()

    // Clicking a chip opens the modal pre-filled with that example.
    fireEvent.click(chips[0])
    expect(screen.getByTestId("new-goal-modal")).toBeInTheDocument()
    const textarea = screen.getByTestId(
      "new-goal-textarea",
    ) as HTMLTextAreaElement
    expect(textarea.value).toBe("describe what this project does")
  })

  it("GoalsTab_populated_zh_quickstart_chips_have_no_english_literal", () => {
    mockedUseLocale.mockReturnValue("zh")
    useGoalsStore.setState({
      runs: [makeRun("a", "2026-05-13T10:00:00Z")],
      activeRun: null,
    })
    render(<GoalsTab vaultPath="/v" onSelectRun={() => {}} />)
    const chips = screen.getAllByTestId(/^goals-quickstart-chip-/)
    expect(chips).toHaveLength(4)
    expect(chips[0]).toHaveTextContent("說明這個專案在做什麼")
    expect(chips[0].textContent).not.toContain(
      "describe what this project does",
    )
    // The quick-start label is localized too (not the English literal).
    expect(screen.getByTestId("goals-quickstart")).toHaveTextContent("快速開始")
  })

  it("GoalsTab_empty_state_shows_three_region_layout_with_i18n_prefill_examples", () => {
    useGoalsStore.setState({ runs: [], activeRun: null })
    render(<GoalsTab vaultPath="/v" onSelectRun={() => {}} />)

    // Region 1: content header row at the top.
    expect(screen.getByTestId("tab-content-header-goals")).toBeInTheDocument()

    // Region 2: hero region with heroTitle + heroSubtitle.
    const hero = screen.getByTestId("goals-empty-hero")
    expect(hero).toHaveTextContent("No goals yet")
    expect(hero).toHaveTextContent(
      "Start with one of the examples below, or write your own.",
    )

    // Region 3: exactly four pre-fill pills sourced from i18n keys (en values).
    expect(screen.getAllByTestId(/^goals-empty-prefill-/)).toHaveLength(4)
    const prefills = [
      screen.getByTestId("goals-empty-prefill-0"),
      screen.getByTestId("goals-empty-prefill-1"),
      screen.getByTestId("goals-empty-prefill-2"),
      screen.getByTestId("goals-empty-prefill-3"),
    ]
    expect(prefills[0]).toHaveTextContent("describe what this project does")
    expect(prefills[1]).toHaveTextContent(
      "list the key dependencies and frameworks",
    )
    expect(prefills[2]).toHaveTextContent("summarize the main features")
    expect(prefills[3]).toHaveTextContent("map the project structure")

    // Clicking opens the modal with that example pre-filled.
    fireEvent.click(prefills[0])
    expect(screen.getByTestId("new-goal-modal")).toBeInTheDocument()
    const textarea = screen.getByTestId(
      "new-goal-textarea",
    ) as HTMLTextAreaElement
    expect(textarea.value).toBe("describe what this project does")
  })

  it("GoalsTab_zh_locale_shows_no_english_prefill_literals", () => {
    mockedUseLocale.mockReturnValue("zh")
    useGoalsStore.setState({ runs: [], activeRun: null })
    render(<GoalsTab vaultPath="/v" onSelectRun={() => {}} />)
    const prefills = [
      screen.getByTestId("goals-empty-prefill-0"),
      screen.getByTestId("goals-empty-prefill-1"),
      screen.getByTestId("goals-empty-prefill-2"),
      screen.getByTestId("goals-empty-prefill-3"),
    ]
    expect(prefills[0]).toHaveTextContent("說明這個專案在做什麼")
    expect(prefills[1]).toHaveTextContent("列出主要依賴套件與框架")
    expect(prefills[2]).toHaveTextContent("整理主要功能")
    expect(prefills[3]).toHaveTextContent("畫出專案結構")
    // Crucially, none of the English literals leak through in zh locale.
    expect(prefills[0].textContent).not.toContain(
      "describe what this project does",
    )
  })

  it("filters non-goal modes out of the visible list", () => {
    useGoalsStore.setState({
      runs: [
        makeRun("g", "2026-05-13T10:00:00Z"),
        { ...makeRun("c", "2026-05-13T11:00:00Z"), mode: "chat" },
      ],
      activeRun: null,
    })
    render(<GoalsTab vaultPath="/v" onSelectRun={() => {}} />)
    expect(screen.queryByTestId("run-row-c")).toBeNull()
    expect(screen.getByTestId("run-row-g")).toBeInTheDocument()
  })

  it("terminal_spawn_appears_via_goals_changed", async () => {
    let capturedCallback: ((ev: { payload: unknown }) => void) | undefined
    mockedListen.mockImplementation(async (name, cb) => {
      if (name === "goals-changed") {
        capturedCallback = cb as (ev: { payload: unknown }) => void
      }
      return () => {}
    })
    const refreshRunsSpy = vi.fn(async () => {})
    useGoalsStore.setState({
      runs: [],
      activeRun: null,
      refreshRuns: refreshRunsSpy as never,
    })

    render(<GoalsTab vaultPath="/v" onSelectRun={() => {}} />)
    await waitFor(() => expect(capturedCallback).toBeTruthy())

    capturedCallback?.({ payload: null })
    await waitFor(() => expect(refreshRunsSpy).toHaveBeenCalledWith("/v"))
  })

  it.each([
    ["en", "+ New goal"],
    ["zh", "+ 新增 Goal"],
  ])(
    "GoalsTab_new_goal_button_label_in_%s_locale",
    (locale, expected) => {
      mockedUseLocale.mockReturnValue(locale as "en" | "zh")
      useGoalsStore.setState({ runs: [], activeRun: null })
      render(<GoalsTab vaultPath="/v" onSelectRun={() => {}} />)
      expect(screen.getByTestId("new-goal-button")).toHaveTextContent(expected)
    },
  )

  it("GoalsTab_pressing_bare_N_opens_new_goal_modal", () => {
    useGoalsStore.setState({ runs: [], activeRun: null })
    render(<GoalsTab vaultPath="/v" onSelectRun={() => {}} />)
    expect(screen.queryByTestId("new-goal-modal")).toBeNull()
    fireEvent.keyDown(window, { key: "n" })
    expect(screen.getByTestId("new-goal-modal")).toBeInTheDocument()
  })

  it("GoalsTab_N_shortcut_ignored_when_modal_already_open", () => {
    useGoalsStore.setState({ runs: [], activeRun: null })
    render(<GoalsTab vaultPath="/v" onSelectRun={() => {}} />)
    // Open modal by clicking the CTA first.
    fireEvent.click(screen.getByTestId("new-goal-button"))
    expect(screen.getByTestId("new-goal-modal")).toBeInTheDocument()
    const textarea = screen.getByTestId(
      "new-goal-textarea",
    ) as HTMLTextAreaElement
    // Pre-fill some text the user is typing.
    fireEvent.change(textarea, { target: { value: "describe " } })
    // Pressing N inside the textarea SHALL NOT re-fire the shortcut.
    fireEvent.keyDown(textarea, { key: "n", bubbles: true })
    expect(textarea.value).toBe("describe ")
  })
})
