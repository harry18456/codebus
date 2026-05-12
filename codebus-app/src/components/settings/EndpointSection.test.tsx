import { useState } from "react"
import { describe, expect, it, vi, beforeEach } from "vitest"
import { render, screen, fireEvent, waitFor } from "@testing-library/react"

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}))

import { invoke } from "@tauri-apps/api/core"
import { EndpointSection } from "./EndpointSection"
import type { ClaudeCodeBlock } from "@/lib/ipc"

const mockedInvoke = vi.mocked(invoke)

function defaultBlock(): ClaudeCodeBlock {
  return {
    active: "system",
    system: {
      goal: { model: "opus-4-6", effort: "high" },
      query: { model: "haiku-4-5", effort: "low" },
      fix: { model: "sonnet-4-6", effort: "medium" },
    },
    azure: null,
  }
}

function setupInvokeForKeyStatus(status: "set" | "unset") {
  mockedInvoke.mockImplementation((cmd: string) => {
    if (cmd === "get_endpoint_key") {
      return Promise.resolve({ kind: status })
    }
    return Promise.resolve(undefined)
  })
}

describe("EndpointSection", () => {
  beforeEach(() => {
    mockedInvoke.mockReset()
    setupInvokeForKeyStatus("unset")
  })

  it("renders both profile sub-sections in the DOM (accordion)", () => {
    render(<EndpointSection claudeCode={defaultBlock()} onChange={() => {}} />)
    expect(screen.getByTestId("system-profile")).toBeInTheDocument()
    expect(screen.getByTestId("azure-profile")).toBeInTheDocument()
  })

  it("active radio defaults to system and shows azure as inactive + collapsed", () => {
    render(<EndpointSection claudeCode={defaultBlock()} onChange={() => {}} />)
    expect(screen.getByTestId("active-system")).toBeChecked()
    expect(screen.getByTestId("active-azure")).not.toBeChecked()
    // Inactive label on the non-active header.
    expect(screen.getByTestId("azure-profile-inactive-label")).toBeInTheDocument()
    expect(
      screen.queryByTestId("system-profile-inactive-label"),
    ).not.toBeInTheDocument()
    // Active SHALL be expanded, inactive SHALL be collapsed (initial render).
    expect(screen.getByTestId("system-profile")).toHaveAttribute(
      "data-expanded",
      "true",
    )
    expect(screen.getByTestId("azure-profile")).toHaveAttribute(
      "data-expanded",
      "false",
    )
    // Body of inactive SHALL be hidden via the `hidden` attribute but
    // remain in the DOM (input values persist across collapse).
    expect(screen.getByTestId("azure-profile-body")).toHaveAttribute("hidden")
    expect(screen.getByTestId("system-profile-body")).not.toHaveAttribute(
      "hidden",
    )
  })

  it("clicking inactive header expands it without collapsing the active one", () => {
    render(<EndpointSection claudeCode={defaultBlock()} onChange={() => {}} />)
    fireEvent.click(screen.getByTestId("azure-profile-header"))
    expect(screen.getByTestId("azure-profile")).toHaveAttribute(
      "data-expanded",
      "true",
    )
    // System (active) SHALL remain expanded — user clicks on inactive
    // does NOT collapse active.
    expect(screen.getByTestId("system-profile")).toHaveAttribute(
      "data-expanded",
      "true",
    )
  })

  it("toggling active auto-folds: new active expands, old active collapses", async () => {
    const onChange = vi.fn<(b: ClaudeCodeBlock) => void>()
    // Use a stateful wrapper so the EndpointSection re-renders with the
    // new active prop after onChange fires (mirrors store behavior).
    function Wrapper() {
      const [block, setBlock] = useState<ClaudeCodeBlock>(defaultBlock())
      return (
        <EndpointSection
          claudeCode={block}
          onChange={(next) => {
            onChange(next)
            setBlock(next)
          }}
        />
      )
    }
    render(<Wrapper />)
    // Initial: system expanded, azure collapsed.
    expect(screen.getByTestId("system-profile")).toHaveAttribute(
      "data-expanded",
      "true",
    )
    expect(screen.getByTestId("azure-profile")).toHaveAttribute(
      "data-expanded",
      "false",
    )
    // Toggle to azure.
    fireEvent.click(screen.getByTestId("active-azure"))
    await waitFor(() =>
      expect(screen.getByTestId("azure-profile")).toHaveAttribute(
        "data-expanded",
        "true",
      ),
    )
    expect(screen.getByTestId("system-profile")).toHaveAttribute(
      "data-expanded",
      "false",
    )
  })

  it("toggling active radio mutates only the active field; profile bodies are preserved", () => {
    const onChange = vi.fn<(b: ClaudeCodeBlock) => void>()
    const initial = defaultBlock()
    render(<EndpointSection claudeCode={initial} onChange={onChange} />)
    fireEvent.click(screen.getByTestId("active-azure"))
    expect(onChange).toHaveBeenCalledTimes(1)
    const next = onChange.mock.calls[0][0]
    expect(next.active).toBe("azure")
    // System profile body is unchanged (preservation contract per spec
    // `Active radio switch preserves non-active profile inputs`).
    expect(next.system).toEqual(initial.system)
  })

  it("system model dropdowns expose exactly four versioned options per verb", async () => {
    render(<EndpointSection claudeCode={defaultBlock()} onChange={() => {}} />)
    // Open the goal verb's dropdown.
    fireEvent.click(screen.getByTestId("system-model-goal"))
    await waitFor(() => {
      expect(screen.getByRole("option", { name: "opus-4-7" })).toBeInTheDocument()
    })
    for (const opt of ["opus-4-7", "opus-4-6", "haiku-4-5", "sonnet-4-6"]) {
      expect(screen.getByRole("option", { name: opt })).toBeInTheDocument()
    }
    // Forbidden unversioned aliases SHALL NOT appear.
    for (const forbidden of ["opus", "haiku", "sonnet"]) {
      expect(
        screen.queryByRole("option", { name: forbidden }),
      ).not.toBeInTheDocument()
    }
  })

  it("azure keyring_service input pre-fills with codebus-azure when azure block is null", () => {
    const block = defaultBlock()
    block.azure = null
    render(<EndpointSection claudeCode={block} onChange={() => {}} />)
    expect(screen.getByTestId("azure-keyring-service")).toHaveValue(
      "codebus-azure",
    )
  })

  it("Set new... button opens the SetKeyDialog modal", async () => {
    render(<EndpointSection claudeCode={defaultBlock()} onChange={() => {}} />)
    fireEvent.click(screen.getByTestId("azure-key-set"))
    await waitFor(() =>
      expect(screen.getByTestId("set-key-dialog")).toBeInTheDocument(),
    )
    expect(screen.getByTestId("set-key-input")).toHaveAttribute("type", "password")
  })

  it("Confirming the modal invokes set_endpoint_key and updates status to Set", async () => {
    mockedInvoke.mockImplementation((cmd: string) => {
      if (cmd === "get_endpoint_key") return Promise.resolve({ kind: "unset" })
      if (cmd === "set_endpoint_key") return Promise.resolve(undefined)
      return Promise.resolve(undefined)
    })

    render(<EndpointSection claudeCode={defaultBlock()} onChange={() => {}} />)
    await waitFor(() =>
      expect(screen.getByTestId("azure-key-status")).toHaveTextContent("Unset"),
    )

    fireEvent.click(screen.getByTestId("azure-key-set"))
    fireEvent.change(screen.getByTestId("set-key-input"), {
      target: { value: "sk-modal-test" },
    })
    fireEvent.click(screen.getByTestId("set-key-confirm"))

    await waitFor(() =>
      expect(screen.getByTestId("azure-key-status")).toHaveTextContent("Set"),
    )

    expect(mockedInvoke).toHaveBeenCalledWith(
      "set_endpoint_key",
      { profile: "azure", key: "sk-modal-test" },
    )
    // The key value MUST NOT persist in any visible DOM after confirm.
    expect(document.body.textContent).not.toContain("sk-modal-test")
  })

  it("Delete button invokes delete_endpoint_key and updates status to Unset", async () => {
    setupInvokeForKeyStatus("set")
    mockedInvoke.mockImplementation((cmd: string) => {
      if (cmd === "get_endpoint_key") return Promise.resolve({ kind: "set" })
      if (cmd === "delete_endpoint_key") return Promise.resolve(undefined)
      return Promise.resolve(undefined)
    })

    render(<EndpointSection claudeCode={defaultBlock()} onChange={() => {}} />)
    await waitFor(() =>
      expect(screen.getByTestId("azure-key-status")).toHaveTextContent("Set"),
    )

    fireEvent.click(screen.getByTestId("azure-key-delete"))
    await waitFor(() =>
      expect(screen.getByTestId("azure-key-status")).toHaveTextContent("Unset"),
    )
    expect(mockedInvoke).toHaveBeenCalledWith("delete_endpoint_key", {
      profile: "azure",
    })
  })

  it("renders validation summary + aria-invalid when errors prop is non-empty", () => {
    render(
      <EndpointSection
        claudeCode={defaultBlock()}
        onChange={() => {}}
        errors={[
          {
            field: "claude_code.azure.base_url",
            message: "base_url is required when active=azure",
          },
          {
            field: "claude_code.azure.goal.model",
            message: "goal deployment name is required when active=azure",
          },
        ]}
      />,
    )
    // Summary block surfaces both error messages.
    const summary = screen.getByTestId("endpoint-validation-summary")
    expect(summary).toBeInTheDocument()
    expect(summary).toHaveTextContent("base_url is required")
    expect(summary).toHaveTextContent("goal deployment name is required")
    // aria-invalid set on the offending fields, not on others.
    expect(screen.getByTestId("azure-base-url")).toHaveAttribute(
      "aria-invalid",
      "true",
    )
    expect(screen.getByTestId("azure-deployment-goal")).toHaveAttribute(
      "aria-invalid",
      "true",
    )
    expect(screen.getByTestId("azure-deployment-query")).not.toHaveAttribute(
      "aria-invalid",
    )
  })

  it("does NOT render validation summary when errors prop is empty (default)", () => {
    render(<EndpointSection claudeCode={defaultBlock()} onChange={() => {}} />)
    expect(
      screen.queryByTestId("endpoint-validation-summary"),
    ).not.toBeInTheDocument()
  })

  it("cancelling the set-key modal does NOT invoke set_endpoint_key", async () => {
    render(<EndpointSection claudeCode={defaultBlock()} onChange={() => {}} />)
    fireEvent.click(screen.getByTestId("azure-key-set"))
    fireEvent.change(screen.getByTestId("set-key-input"), {
      target: { value: "sk-cancel" },
    })
    fireEvent.click(screen.getByTestId("set-key-cancel"))

    // Only the initial get_endpoint_key call SHALL have happened; no set.
    const calls = mockedInvoke.mock.calls.map((c) => c[0])
    expect(calls).not.toContain("set_endpoint_key")
  })
})
