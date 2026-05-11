import { describe, expect, it, vi, beforeEach } from "vitest"
import { render, screen, fireEvent, waitFor } from "@testing-library/react"

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}))

import { invoke } from "@tauri-apps/api/core"
import { SettingsModal } from "./SettingsModal"
import { useSettingsStore } from "@/store/settings"

const mockedInvoke = vi.mocked(invoke)

describe("SettingsModal", () => {
  beforeEach(() => {
    useSettingsStore.setState({
      config: {
        app: { quiz: { pass_threshold: 80, default_length: 5 } },
        claude_code: {
          goal: { model: "opus" },
          query: { model: "haiku" },
          fix: { model: "sonnet" },
        },
        pii: { scanner: "regex_basic" },
        log: { sink: "~/.codebus/logs/" },
      },
      initialConfig: {
        app: { quiz: { pass_threshold: 80, default_length: 5 } },
      },
      dirty: false,
      loading: false,
      saving: false,
      error: null,
    })
    mockedInvoke.mockReset()
    mockedInvoke.mockResolvedValue(
      useSettingsStore.getState().config,
    )
  })

  it("renders exactly the seven required fields", () => {
    render(
      <SettingsModal open onClose={() => {}} piiPatternCount={14} />,
    )
    expect(screen.getByText("AI Provider")).toBeInTheDocument()
    expect(screen.getByText("Authentication")).toBeInTheDocument()
    expect(screen.getByText("Default model")).toBeInTheDocument()
    expect(screen.getByText("PII scanner")).toBeInTheDocument()
    expect(screen.getByText("Log sink")).toBeInTheDocument()
    expect(screen.getByText("Quiz pass threshold")).toBeInTheDocument()
    expect(screen.getByText("Default quiz length")).toBeInTheDocument()

    // Forbidden controls (Forbidden Behaviors in v1).
    expect(screen.queryByText(/theme/i)).toBeNull()
    expect(screen.queryByText(/language/i)).toBeNull()
    expect(screen.queryByText(/vault-specific/i)).toBeNull()
  })

  it("renders the runtime PII pattern count, not a hard-coded number", () => {
    render(
      <SettingsModal open onClose={() => {}} piiPatternCount={42} />,
    )
    expect(screen.getByTestId("pii-pattern-count-display")).toHaveTextContent(
      "regex_basic · 42 patterns",
    )
  })

  it("sub-labels avoid the forbidden vocabulary", () => {
    render(
      <SettingsModal open onClose={() => {}} piiPatternCount={14} />,
    )
    const modal = screen.getByTestId("settings-modal")
    const text = modal.textContent ?? ""
    expect(text).not.toMatch(/override/i)
    expect(text).not.toMatch(/learned/i)
    expect(text).not.toMatch(/mastered/i)
    expect(text).not.toMatch(/graduated/i)
  })

  it("calls save_global_config on Save and closes after success", async () => {
    mockedInvoke.mockResolvedValueOnce(undefined)
    const onClose = vi.fn()
    useSettingsStore.setState({ dirty: true })
    render(
      <SettingsModal open onClose={onClose} piiPatternCount={14} />,
    )
    fireEvent.click(screen.getByTestId("settings-save"))
    await waitFor(() => expect(onClose).toHaveBeenCalled(), { timeout: 1000 })
    expect(mockedInvoke).toHaveBeenCalledWith(
      "save_global_config",
      expect.objectContaining({ config: expect.any(Object) }),
    )
  })

  it("keeps modal open and shows inline error on save failure", async () => {
    mockedInvoke.mockRejectedValueOnce({ kind: "io", message: "disk full" })
    const onClose = vi.fn()
    useSettingsStore.setState({ dirty: true })
    render(
      <SettingsModal open onClose={onClose} piiPatternCount={14} />,
    )
    fireEvent.click(screen.getByTestId("settings-save"))
    await waitFor(() =>
      expect(screen.getByTestId("settings-error")).toBeInTheDocument(),
    )
    expect(onClose).not.toHaveBeenCalled()
  })

  it("threshold slider value renders with % unit, length renders with `questions` unit", () => {
    render(
      <SettingsModal open onClose={() => {}} piiPatternCount={14} />,
    )
    expect(screen.getByTestId("quiz-threshold-value")).toHaveTextContent("80%")
    expect(screen.getByTestId("quiz-length-value")).toHaveTextContent(
      "5 questions",
    )
  })
})
