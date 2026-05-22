import { describe, expect, it, vi, beforeEach } from "vitest"
import { render, screen, fireEvent, waitFor, act } from "@testing-library/react"

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }))

import { invoke } from "@tauri-apps/api/core"
import { SettingsModal } from "./SettingsModal"
import { useSettingsStore } from "@/store/settings"

const mockedInvoke = vi.mocked(invoke)

const codexConfig = {
  app: { quiz: { pass_threshold: 80, default_length: 5 } },
  agent: {
    active_provider: "codex",
    providers: {
      codex: {
        active: "system",
        system: {
          goal: { model: "gpt-5.5", effort: "high" },
          query: { model: "gpt-5.5", effort: "low" },
          fix: { model: "gpt-5.5", effort: "medium" },
          verify: { model: "gpt-5.5", effort: "high" },
        },
      },
    },
  },
  pii: { scanner: "regex_basic" },
  log: { sink: "~/.codebus/logs/" },
}

describe("SettingsModal — codex provider", () => {
  beforeEach(() => {
    useSettingsStore.setState({
      config: codexConfig as never,
      initialConfig: codexConfig as never,
      dirty: false,
      loading: false,
      saving: false,
      error: null,
    })
    mockedInvoke.mockReset()
    mockedInvoke.mockImplementation((cmd: string) => {
      if (cmd === "get_endpoint_key") return Promise.resolve({ kind: "unset" })
      if (cmd === "check_cli_installed") return Promise.resolve({ kind: "not_installed" })
      return Promise.resolve(useSettingsStore.getState().config)
    })
  })

  it("renders the codex editor (not the claude one) when codex is active", () => {
    render(<SettingsModal open onClose={() => {}} piiPatternCount={14} />)
    expect(screen.getByTestId("codex-endpoint-section")).toBeInTheDocument()
    expect(screen.getByText("OpenAI Codex endpoint settings")).toBeInTheDocument()
    // Claude editor must NOT be mounted simultaneously.
    expect(screen.queryByTestId("endpoint-section")).toBeNull()
  })

  it("labels the CLI status row with the codex provider", () => {
    render(<SettingsModal open onClose={() => {}} piiPatternCount={14} />)
    expect(screen.getByText("OpenAI Codex CLI")).toBeInTheDocument()
  })

  it("offers both providers in the AI Provider selector", () => {
    render(<SettingsModal open onClose={() => {}} piiPatternCount={14} />)
    expect(screen.getByTestId("ai-provider-select")).toBeInTheDocument()
  })

  it("Save sends a config carrying the codex provider block to the IPC", async () => {
    useSettingsStore.setState({ dirty: true })
    const onClose = vi.fn()
    render(<SettingsModal open onClose={onClose} piiPatternCount={14} />)
    fireEvent.click(screen.getByTestId("settings-save"))
    await waitFor(() => expect(onClose).toHaveBeenCalled(), { timeout: 1000 })
    const call = mockedInvoke.mock.calls.find((c) => c[0] === "save_global_config")
    expect(call).toBeTruthy()
    const cfg = (call![1] as { config: { agent: { active_provider: string; providers: Record<string, { system: { goal: { model: string } } }> } } }).config
    expect(cfg.agent.active_provider).toBe("codex")
    expect(cfg.agent.providers.codex.system.goal.model).toBe("gpt-5.5")
  })
})

describe("SettingsModal — switching provider must not revert", () => {
  const claudeDiskConfig = {
    app: { quiz: { pass_threshold: 80, default_length: 5 } },
    agent: {
      active_provider: "claude",
      providers: {
        claude: {
          active: "system",
          system: {
            goal: { model: "opus-4-6", effort: "high" },
            query: { model: "haiku-4-5", effort: "low" },
            fix: { model: "sonnet-4-6", effort: "medium" },
            verify: { model: "opus-4-6", effort: "high" },
          },
        },
      },
    },
    pii: { scanner: "regex_basic" },
    log: { sink: "~/.codebus/logs/" },
  }

  beforeEach(() => {
    useSettingsStore.setState({
      config: claudeDiskConfig as never,
      initialConfig: claudeDiskConfig as never,
      dirty: false,
      loading: false,
      saving: false,
      error: null,
    })
    mockedInvoke.mockReset()
    // `load_global_config` mimics DISK: always returns the claude config.
    // This is the trap — if switching provider re-fires load(), the
    // in-memory codex switch is overwritten back to claude.
    mockedInvoke.mockImplementation((cmd: string) => {
      if (cmd === "get_endpoint_key") return Promise.resolve({ kind: "unset" })
      if (cmd === "check_cli_installed") return Promise.resolve({ kind: "not_installed" })
      if (cmd === "load_global_config") return Promise.resolve(claudeDiskConfig)
      return Promise.resolve(useSettingsStore.getState().config)
    })
  })

  it("stays on codex after selecting it (does not reload disk config and revert)", async () => {
    render(<SettingsModal open onClose={() => {}} piiPatternCount={14} />)
    // Initial load resolves the claude (disk) config → claude editor.
    await waitFor(() => expect(screen.getByTestId("endpoint-section")).toBeInTheDocument())

    // Simulate the provider selector switching to codex.
    await act(async () => {
      useSettingsStore.getState().setActiveProvider("codex")
    })

    // Give any (buggy) reload effect a chance to fire and revert.
    await waitFor(() => {
      expect(screen.getByTestId("codex-endpoint-section")).toBeInTheDocument()
    })
    // Must NOT have reverted to the claude editor.
    expect(screen.queryByTestId("endpoint-section")).toBeNull()
    expect(
      (useSettingsStore.getState().config as { agent?: { active_provider?: string } })
        .agent?.active_provider,
    ).toBe("codex")
  })
})

describe("settings store setActiveProvider", () => {
  it("switches active_provider without dropping provider blocks", () => {
    useSettingsStore.setState({
      config: { agent: { active_provider: "claude", providers: { claude: { a: 1 }, codex: { b: 2 } } } } as never,
      dirty: false,
    })
    useSettingsStore.getState().setActiveProvider("codex")
    const cfg = useSettingsStore.getState().config as {
      agent?: { active_provider?: string; providers?: Record<string, unknown> }
    }
    expect(cfg.agent?.active_provider).toBe("codex")
    expect(cfg.agent?.providers?.claude).toEqual({ a: 1 })
    expect(cfg.agent?.providers?.codex).toEqual({ b: 2 })
    expect(useSettingsStore.getState().dirty).toBe(true)
  })
})
