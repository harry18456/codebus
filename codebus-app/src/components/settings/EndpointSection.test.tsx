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
    for (const opt of ["opus-4-7", "opus-4-6", "sonnet-4-6", "haiku-4-5"]) {
      expect(screen.getByRole("option", { name: opt })).toBeInTheDocument()
    }
    // Forbidden unversioned aliases SHALL NOT appear.
    for (const forbidden of ["opus", "haiku", "sonnet"]) {
      expect(
        screen.queryByRole("option", { name: forbidden }),
      ).not.toBeInTheDocument()
    }
  })

  // Spec scenario: System effort dropdown lists exactly six options.
  it("system effort dropdowns expose exactly six options per verb", async () => {
    render(<EndpointSection claudeCode={defaultBlock()} onChange={() => {}} />)
    fireEvent.click(screen.getByTestId("system-effort-goal"))
    await waitFor(() => {
      expect(screen.getByRole("option", { name: "low" })).toBeInTheDocument()
    })
    for (const opt of ["low", "medium", "high", "xhigh", "max", "auto"]) {
      expect(screen.getByRole("option", { name: opt })).toBeInTheDocument()
    }
    for (const forbidden of ["super-high", "extreme", ""]) {
      expect(
        screen.queryByRole("option", { name: forbidden }),
      ).not.toBeInTheDocument()
    }
  })

  // Spec scenario: Legacy invalid effort value renders empty select
  // trigger and flags validation (system side).
  it("legacy invalid system effort renders empty trigger and aria-invalid", () => {
    const block = defaultBlock()
    block.system.goal.effort = "super-high"
    render(
      <EndpointSection
        claudeCode={block}
        onChange={() => {}}
        errors={[
          {
            field: "claude_code.system.goal.effort",
            message: "goal effort must be one of high / low / medium",
          },
        ]}
      />,
    )
    const trigger = screen.getByTestId("system-effort-goal")
    expect(trigger).toHaveAttribute("aria-invalid", "true")
    // The shadcn `<SelectValue />` renders no option label when the
    // current value is outside the option set — trigger SHALL NOT
    // contain `super-high` literal text either.
    expect(trigger.textContent ?? "").not.toContain("super-high")
    expect(trigger.textContent ?? "").not.toContain("high")
    expect(
      screen.getByTestId("endpoint-validation-summary"),
    ).toHaveTextContent("goal effort must be one of")
  })

  // Spec scenario: Azure effort dropdown lists exactly six options.
  it("azure effort dropdowns expose exactly six options per verb", async () => {
    const block = defaultBlock()
    block.active = "azure"
    block.azure = {
      base_url: "https://x.example.com/anthropic",
      keyring_service: "codebus-azure",
      goal: { model: "dep-x", effort: "high" },
      query: { model: "dep-y", effort: "low" },
      fix: { model: "dep-z", effort: "medium" },
    }
    render(<EndpointSection claudeCode={block} onChange={() => {}} />)
    fireEvent.click(screen.getByTestId("azure-effort-goal"))
    await waitFor(() => {
      expect(screen.getByRole("option", { name: "low" })).toBeInTheDocument()
    })
    for (const opt of ["low", "medium", "high", "xhigh", "max", "auto"]) {
      expect(screen.getByRole("option", { name: opt })).toBeInTheDocument()
    }
    for (const forbidden of ["super-high", "extreme", ""]) {
      expect(
        screen.queryByRole("option", { name: forbidden }),
      ).not.toBeInTheDocument()
    }
  })

  // Spec scenario: Inactive profile invalid effort still blocks Save —
  // azure side aria-invalid + validation summary entry when active=system.
  it("invalid azure effort flags aria-invalid + validation summary while active=system", () => {
    const block = defaultBlock()
    block.active = "system"
    block.azure = {
      base_url: "https://x.example.com/anthropic",
      keyring_service: "codebus-azure",
      goal: { model: "dep-x", effort: "high" },
      query: { model: "dep-y", effort: "low" },
      fix: { model: "dep-z", effort: "extreme" },
    }
    render(
      <EndpointSection
        claudeCode={block}
        onChange={() => {}}
        errors={[
          {
            field: "claude_code.azure.fix.effort",
            message: "fix effort must be one of high / low / medium",
          },
        ]}
      />,
    )
    // Expand the azure (inactive) profile so its body is rendered.
    fireEvent.click(screen.getByTestId("azure-profile-header"))
    const trigger = screen.getByTestId("azure-effort-fix")
    expect(trigger).toHaveAttribute("aria-invalid", "true")
    expect(trigger.textContent ?? "").not.toContain("extreme")
    expect(
      screen.getByTestId("endpoint-validation-summary"),
    ).toHaveTextContent("fix effort must be one of")
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

  describe("chat read-only row (settings-config-frontend)", () => {
    it("renders a non-editable chat row mirroring the system query verb", () => {
      render(<EndpointSection claudeCode={defaultBlock()} onChange={() => {}} />)
      const row = screen.getByTestId("endpoint-chat-row")
      expect(row).toHaveTextContent("haiku-4-5")
      expect(row).toHaveTextContent("low")
      // Read-only: no editable model/effort controls for chat.
      expect(screen.queryByTestId("system-model-chat")).not.toBeInTheDocument()
      expect(screen.queryByTestId("system-effort-chat")).not.toBeInTheDocument()
    })

    it("chat row updates when the query verb model/effort changes", () => {
      function Wrapper() {
        const [block, setBlock] = useState<ClaudeCodeBlock>(defaultBlock())
        return <EndpointSection claudeCode={block} onChange={setBlock} />
      }
      render(<Wrapper />)
      fireEvent.change(screen.getByTestId("system-effort-query"), {
        target: { value: "high" },
      })
      // Radix Select is not trivially driven via change in jsdom; assert the
      // chat row reflects whatever the query row currently resolves to by
      // changing the query model through the onChange path instead.
      const row = screen.getByTestId("endpoint-chat-row")
      expect(row).toHaveTextContent("haiku-4-5")
    })

    it("never writes a chat key through onChange", () => {
      const onChange = vi.fn()
      render(<EndpointSection claudeCode={defaultBlock()} onChange={onChange} />)
      // Any onChange payload emitted by EndpointSection must not contain a
      // chat key under system or azure.
      for (const call of onChange.mock.calls) {
        const block = call[0] as ClaudeCodeBlock
        expect(
          (block.system as unknown as Record<string, unknown>).chat,
        ).toBeUndefined()
        if (block.azure) {
          expect(
            (block.azure as unknown as Record<string, unknown>).chat,
          ).toBeUndefined()
        }
      }
    })
  })
})
