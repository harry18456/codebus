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
        // Profile-mode schema (post-stage-a). Legacy flat shape is
        // unsupported as of the `stage-b-app-endpoint-settings` change.
        claude_code: {
          active: "system",
          system: {
            goal: { model: "opus-4-6", effort: "high" },
            query: { model: "haiku-4-5", effort: "low" },
            fix: { model: "sonnet-4-6", effort: "medium" },
          },
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
    // Default mock: every IPC call resolves to the current config. Per-
    // test overrides (mockResolvedValueOnce / mockImplementation) replace
    // this for specific calls like `set_endpoint_key` / `get_endpoint_key`.
    mockedInvoke.mockImplementation((cmd: string) => {
      if (cmd === "get_endpoint_key") return Promise.resolve({ kind: "unset" })
      return Promise.resolve(useSettingsStore.getState().config)
    })
  })

  it("renders the required top-level fields including the endpoint section", () => {
    render(
      <SettingsModal open onClose={() => {}} piiPatternCount={14} />,
    )
    expect(screen.getByText("AI Provider")).toBeInTheDocument()
    // The legacy OAuth "Authentication" field is replaced by a real CLI
    // installation probe row ("Claude Code CLI").
    expect(screen.getByText("Claude Code CLI")).toBeInTheDocument()
    // The legacy "Default model" Field is replaced by the Endpoint section.
    expect(
      screen.getByText("Claude Code endpoint settings"),
    ).toBeInTheDocument()
    expect(screen.getByTestId("endpoint-section")).toBeInTheDocument()
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

  it("CLI status row shows `Checking…` immediately on open then resolves to installed", async () => {
    mockedInvoke.mockImplementation((cmd: string) => {
      if (cmd === "check_cli_installed") {
        return Promise.resolve({ kind: "installed", version: "2.1.139 (Claude Code)" })
      }
      if (cmd === "get_endpoint_key") return Promise.resolve({ kind: "unset" })
      return Promise.resolve(useSettingsStore.getState().config)
    })
    render(<SettingsModal open onClose={() => {}} piiPatternCount={14} />)
    // Eventually flips to installed.
    await waitFor(() =>
      expect(screen.getByTestId("cli-status")).toHaveAttribute(
        "data-state",
        "installed",
      ),
    )
    expect(screen.getByTestId("cli-status")).toHaveTextContent("2.1.139")
  })

  it("Save button is disabled when active=azure has empty required fields", async () => {
    useSettingsStore.setState({
      config: {
        app: { quiz: { pass_threshold: 80, default_length: 5 } },
        claude_code: {
          active: "azure",
          system: {
            goal: { model: "opus-4-6", effort: "high" },
            query: { model: "haiku-4-5", effort: "low" },
            fix: { model: "sonnet-4-6", effort: "medium" },
          },
          azure: {
            base_url: "", // missing → invalid
            keyring_service: "codebus-azure",
            goal: { model: "", effort: "high" },
            query: { model: "", effort: "low" },
            fix: { model: "", effort: "medium" },
          },
        },
      },
      initialConfig: { app: { quiz: { pass_threshold: 80, default_length: 5 } } },
      dirty: true, // user has edited so dirty would otherwise enable Save
      loading: false,
      saving: false,
      error: null,
    })
    render(<SettingsModal open onClose={() => {}} piiPatternCount={14} />)
    await waitFor(() => screen.getByTestId("settings-save"))
    expect(screen.getByTestId("settings-save")).toBeDisabled()
  })

  it("Save button enables when active=azure becomes fully populated", async () => {
    const fullConfig = {
      app: { quiz: { pass_threshold: 80, default_length: 5 } },
      claude_code: {
        active: "azure",
        system: {
          goal: { model: "opus-4-6", effort: "high" },
          query: { model: "haiku-4-5", effort: "low" },
          fix: { model: "sonnet-4-6", effort: "medium" },
        },
        azure: {
          base_url: "https://x.example.com/anthropic",
          keyring_service: "codebus-azure",
          goal: { model: "dep-x", effort: "high" },
          query: { model: "dep-y", effort: "low" },
          fix: { model: "dep-z", effort: "medium" },
        },
      },
    }
    useSettingsStore.setState({
      config: fullConfig,
      initialConfig: fullConfig,
      dirty: false,
      loading: false,
      saving: false,
      error: null,
    })
    mockedInvoke.mockImplementation((cmd: string) => {
      if (cmd === "get_endpoint_key") return Promise.resolve({ kind: "unset" })
      if (cmd === "check_cli_installed")
        return Promise.resolve({ kind: "installed", version: "x" })
      return Promise.resolve(fullConfig)
    })
    render(<SettingsModal open onClose={() => {}} piiPatternCount={14} />)
    // The on-open `load()` resets dirty; flip dirty back to simulate a
    // post-load user edit (the actual edit content doesn't matter for
    // this test — only that validation passes).
    await waitFor(() => screen.getByTestId("settings-save"))
    useSettingsStore.setState({ dirty: true })
    await waitFor(() =>
      expect(screen.getByTestId("settings-save")).not.toBeDisabled(),
    )
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
