import { describe, expect, it, vi, beforeEach } from "vitest"
import { render, screen, fireEvent, waitFor } from "@testing-library/react"

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}))

import { invoke } from "@tauri-apps/api/core"
import { McpIntegrationSection } from "./McpIntegrationSection"

const mockedInvoke = vi.mocked(invoke)

type Kind = "installed" | "not_registered" | "client_missing"

/**
 * Drive the three IPC commands off a mutable per-provider status map so
 * install/remove flip the status and the component's re-probe observes it.
 * Keyed by `cliBinaryId` (`claude_code` / `codex`).
 */
function mockClients(statuses: Record<string, Kind>) {
  mockedInvoke.mockImplementation((cmd: string, args?: unknown) => {
    const provider = (args as { provider?: string } | undefined)?.provider ?? ""
    if (cmd === "mcp_client_status") {
      return Promise.resolve({ kind: statuses[provider] ?? "client_missing" })
    }
    if (cmd === "mcp_client_install") {
      statuses[provider] = "installed"
      return Promise.resolve(undefined)
    }
    if (cmd === "mcp_client_remove") {
      statuses[provider] = "not_registered"
      return Promise.resolve(undefined)
    }
    return Promise.resolve(undefined)
  })
}

describe("McpIntegrationSection", () => {
  beforeEach(() => {
    mockedInvoke.mockReset()
  })

  it("renders an enabled toggle per detected client reflecting registration", async () => {
    mockClients({ claude_code: "not_registered", codex: "installed" })
    render(<McpIntegrationSection />)

    // Both client rows exist (independent of the active provider).
    expect(await screen.findByTestId("mcp-row-claude")).toBeInTheDocument()
    expect(screen.getByTestId("mcp-row-codex")).toBeInTheDocument()

    const claudeStatus = await screen.findByTestId("mcp-status-claude")
    await waitFor(() => expect(claudeStatus).toHaveTextContent("Not connected"))
    const claudeToggle = screen.getByTestId("mcp-toggle-claude")
    expect(claudeToggle).not.toBeDisabled()
    expect(claudeToggle).not.toBeChecked()

    const codexToggle = screen.getByTestId("mcp-toggle-codex")
    await waitFor(() => expect(codexToggle).toBeChecked())
    expect(screen.getByTestId("mcp-status-codex")).toHaveTextContent("Connected")
  })

  it("disables only the absent client's row, leaving the other independent", async () => {
    mockClients({ claude_code: "client_missing", codex: "not_registered" })
    render(<McpIntegrationSection />)

    const claudeToggle = await screen.findByTestId("mcp-toggle-claude")
    await waitFor(() => expect(claudeToggle).toBeDisabled())
    expect(screen.getByTestId("mcp-status-claude")).toHaveTextContent(
      "Claude Code not installed",
    )

    // codex is unaffected by claude being absent.
    const codexToggle = screen.getByTestId("mcp-toggle-codex")
    await waitFor(() => expect(codexToggle).not.toBeDisabled())
    expect(screen.getByTestId("mcp-status-codex")).toHaveTextContent("Not connected")
  })

  it("connecting an unregistered client installs then re-probes to connected", async () => {
    mockClients({ claude_code: "not_registered", codex: "client_missing" })
    render(<McpIntegrationSection />)

    const claudeToggle = await screen.findByTestId("mcp-toggle-claude")
    await waitFor(() => expect(claudeToggle).not.toBeDisabled())

    fireEvent.click(claudeToggle)

    await waitFor(() =>
      expect(
        mockedInvoke,
      ).toHaveBeenCalledWith("mcp_client_install", { provider: "claude_code" }),
    )
    await waitFor(() =>
      expect(screen.getByTestId("mcp-status-claude")).toHaveTextContent("Connected"),
    )
  })
})
