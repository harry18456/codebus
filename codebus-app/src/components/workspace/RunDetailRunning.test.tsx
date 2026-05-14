import { fireEvent, render, screen } from "@testing-library/react"
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest"

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
}))
vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }))

import { invoke } from "@tauri-apps/api/core"
import { RunDetailRunning } from "./RunDetailRunning"
import { useGoalsStore } from "@/store/goals"

const invokeMock = vi.mocked(invoke)

function seedActiveRun(patch: Partial<ReturnType<typeof useGoalsStore.getState>>) {
  useGoalsStore.setState(patch as Parameters<typeof useGoalsStore.setState>[0])
}

describe("RunDetailRunning", () => {
  beforeEach(() => {
    invokeMock.mockReset()
    useGoalsStore.setState({ runs: [], activeRun: null })
  })

  afterEach(() => {
    useGoalsStore.setState({ runs: [], activeRun: null })
  })

  it("RunDetailRunning_renders_two_tool_use_one_liners_with_emoji", () => {
    seedActiveRun({
      activeRun: {
        runId: "r1",
        goal: "describe X",
        startedAt: "2026-05-13T10:00:00Z",
        events: [
          {
            kind: "stream",
            data: {
              kind: "tool_use",
              name: "Read",
              input: { file_path: "raw/code/auth.rs" },
            },
          },
          {
            kind: "stream",
            data: {
              kind: "tool_use",
              name: "Glob",
              input: { pattern: "wiki/**/*.md" },
            },
          },
        ],
        cancelling: false,
      },
      runs: [],
    })
    render(<RunDetailRunning onBack={() => {}} />)
    const items = screen.getAllByTestId("stream-tool-use")
    expect(items).toHaveLength(2)
    expect(items[0].textContent).toContain("🛠️")
    expect(items[0]).toHaveTextContent("Read")
    expect(items[0]).toHaveTextContent("auth.rs")
    expect(items[1].textContent).toContain("🛠️")
    expect(items[1]).toHaveTextContent("Glob")
    expect(items[1]).toHaveTextContent("wiki/**/*.md")
  })

  it("RunDetailRunning_Write_tool_renders_path_only_with_writing_emoji", () => {
    seedActiveRun({
      activeRun: {
        runId: "r-write",
        goal: "write",
        startedAt: "2026-05-13T10:00:00Z",
        events: [
          {
            kind: "stream",
            data: {
              kind: "tool_use",
              name: "Write",
              input: { file_path: "wiki/modules/auth.md" },
            },
          },
        ],
        cancelling: false,
      },
      runs: [],
    })
    render(<RunDetailRunning onBack={() => {}} />)
    const item = screen.getByTestId("stream-tool-use")
    expect(item.textContent).toContain("✍️")
    expect(item).toHaveTextContent("wiki/modules/auth.md")
    // Tool name `Write` SHALL NOT leak — emoji conveys it.
    expect(item.textContent).not.toContain("Write")
    expect(item.textContent).not.toContain("file_path")
  })

  it("RunDetailRunning_folds_consecutive_thought_chunks_into_one_inline_item", () => {
    seedActiveRun({
      activeRun: {
        runId: "r2",
        goal: "thinking",
        startedAt: "2026-05-13T10:00:00Z",
        events: [
          { kind: "stream", data: { kind: "thought", text: "Analyzing " } },
          { kind: "stream", data: { kind: "thought", text: "the auth " } },
          {
            kind: "stream",
            data: { kind: "thought", text: "middleware..." },
          },
        ],
        cancelling: false,
      },
      runs: [],
    })
    render(<RunDetailRunning onBack={() => {}} />)
    const items = screen.getAllByTestId("thought-item")
    expect(items).toHaveLength(1)
    expect(items[0]).toHaveTextContent(
      "🤔 Analyzing the auth middleware...",
    )
  })

  it("RunDetailRunning_breaks_thought_fold_on_intermediate_tool_use", () => {
    seedActiveRun({
      activeRun: {
        runId: "r3",
        goal: "fold-break",
        startedAt: "2026-05-13T10:00:00Z",
        events: [
          { kind: "stream", data: { kind: "thought", text: "first thought" } },
          {
            kind: "stream",
            data: {
              kind: "tool_use",
              name: "Read",
              input: { file_path: "x.rs" },
            },
          },
          { kind: "stream", data: { kind: "thought", text: "second thought" } },
        ],
        cancelling: false,
      },
      runs: [],
    })
    render(<RunDetailRunning onBack={() => {}} />)
    const thoughts = screen.getAllByTestId("thought-item")
    expect(thoughts).toHaveLength(2)
    expect(thoughts[0]).toHaveTextContent("first thought")
    expect(thoughts[1]).toHaveTextContent("second thought")
  })

  it("RunDetailRunning_cancel_button_disables_after_click", async () => {
    seedActiveRun({
      activeRun: {
        runId: "r3",
        goal: "x",
        startedAt: "2026-05-13T10:00:00Z",
        events: [],
        cancelling: false,
      },
      runs: [],
    })
    invokeMock.mockResolvedValue(undefined)
    render(<RunDetailRunning onBack={() => {}} />)
    const btn = screen.getByTestId("cancel-button")
    expect(btn).not.toBeDisabled()
    fireEvent.click(btn)
    expect(useGoalsStore.getState().activeRun?.cancelling).toBe(true)
    expect(screen.getByTestId("cancel-button")).toBeDisabled()
  })

  it("returns nothing when activeRun is null", () => {
    useGoalsStore.setState({ activeRun: null, runs: [] })
    const { container } = render(<RunDetailRunning onBack={() => {}} />)
    expect(container).toBeEmptyDOMElement()
  })
})
