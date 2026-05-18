import { render, screen, waitFor } from "@testing-library/react"
import { beforeEach, describe, expect, it, vi } from "vitest"

import type { EventEnvelope } from "@/lib/ipc"

// Mock the IPC module so the component's events load is deterministic.
vi.mock("@/lib/ipc", () => ({
  readQuizEvents: vi.fn(),
}))

import { readQuizEvents } from "@/lib/ipc"
import { QuizGenerationLog } from "./QuizGenerationLog"

const mockedReadQuizEvents = vi.mocked(readQuizEvents)

const ENVELOPES: EventEnvelope[] = [
  {
    ts: "2026-05-17T00:00:00Z",
    event: { kind: "stream", data: { kind: "thought", text: "planning the quiz" } },
  },
  {
    ts: "2026-05-17T00:00:01Z",
    event: {
      kind: "stream",
      data: { kind: "tool_use", name: "Read", input: { file_path: "wiki/a.md" } },
    },
  },
]

describe("QuizGenerationLog", () => {
  beforeEach(() => {
    mockedReadQuizEvents.mockReset()
    mockedReadQuizEvents.mockResolvedValue(ENVELOPES)
  })

  // Spec: app-workspace § Quiz History List —
  // "View-generation-log opens the events timeline" +
  // "View-generation-log is not a bare path".

  it("renders the attempt events through the existing agent stream rendering", async () => {
    render(
      <QuizGenerationLog
        vaultPath="/v"
        eventsLog="/v/.codebus/log/events-q.jsonl"
      />,
    )
    await waitFor(() =>
      expect(mockedReadQuizEvents).toHaveBeenCalledWith(
        "/v",
        "/v/.codebus/log/events-q.jsonl",
      ),
    )
    // Stream-rendered items must appear (not just the path string).
    await waitFor(() =>
      expect(screen.getByTestId("thought-item")).toBeInTheDocument(),
    )
    expect(screen.getByTestId("stream-tool-use")).toBeInTheDocument()
  })

  it("does not reduce the log to a bare events.jsonl path string", async () => {
    render(
      <QuizGenerationLog
        vaultPath="/v"
        eventsLog="/v/.codebus/log/events-q.jsonl"
      />,
    )
    await waitFor(() =>
      expect(screen.getByTestId("thought-item")).toBeInTheDocument(),
    )
    // The render is the timeline, not a single path label.
    expect(
      screen.queryByTestId("quiz-view-log-path"),
    ).not.toBeInTheDocument()
  })
})
