import { render, screen, fireEvent } from "@testing-library/react"
import { describe, expect, it, vi } from "vitest"

import { CodexEndpointSection } from "./CodexEndpointSection"
import type { CodexBlock } from "@/lib/ipc"

vi.mock("@/lib/ipc", async (orig) => {
  const actual = await orig<typeof import("@/lib/ipc")>()
  return {
    ...actual,
    getEndpointKey: vi.fn().mockResolvedValue({ kind: "unset" }),
    deleteEndpointKey: vi.fn().mockResolvedValue(undefined),
  }
})

function systemBlock(): CodexBlock {
  return {
    active: "system",
    system: {
      goal: { model: "gpt-5.5", effort: "high" },
      query: { model: "gpt-5.5", effort: "low" },
      fix: { model: "gpt-5.5", effort: "medium" },
      verify: { model: "gpt-5.5", effort: "high" },
    },
    azure: null,
  }
}

describe("CodexEndpointSection", () => {
  it("renders system verb models as free-text inputs (not dropdowns)", () => {
    render(<CodexEndpointSection block={systemBlock()} onChange={() => {}} />)
    const goal = screen.getByTestId("codex-system-model-goal")
    expect(goal.tagName).toBe("INPUT")
    expect((goal as HTMLInputElement).value).toBe("gpt-5.5")
  })

  it("editing a system model calls onChange with the new free-text value", () => {
    const onChange = vi.fn()
    render(<CodexEndpointSection block={systemBlock()} onChange={onChange} />)
    fireEvent.change(screen.getByTestId("codex-system-model-goal"), {
      target: { value: "o4-mini" },
    })
    expect(onChange).toHaveBeenCalled()
    const next = onChange.mock.calls.at(-1)![0] as CodexBlock
    expect(next.system.goal.model).toBe("o4-mini")
  })

  it("renders an api_version field in the azure profile", () => {
    const block: CodexBlock = {
      active: "azure",
      system: systemBlock().system,
      azure: {
        base_url: "https://x.cognitiveservices.azure.com/openai",
        api_version: "2025-04-01-preview",
        keyring_service: "codebus-azure",
        goal: { model: "gpt-5.4", effort: "high" },
        query: { model: "gpt-5.4", effort: "low" },
        fix: { model: "gpt-5.4", effort: "medium" },
        verify: { model: "gpt-5.4", effort: "high" },
      },
    }
    render(<CodexEndpointSection block={block} onChange={() => {}} />)
    const apiVersion = screen.getByTestId("codex-azure-api-version") as HTMLInputElement
    expect(apiVersion.value).toBe("2025-04-01-preview")
  })

  it("links model/effort inputs to suggestion datalists (combobox, still free-text)", () => {
    const { container } = render(<CodexEndpointSection block={systemBlock()} onChange={() => {}} />)
    expect(screen.getByTestId("codex-system-model-goal").getAttribute("list")).toBe("codex-model-suggestions")
    expect(screen.getByTestId("codex-system-effort-goal").getAttribute("list")).toBe("codex-effort-suggestions")
    const modelOpts = Array.from(
      container.querySelectorAll("#codex-model-suggestions option"),
    ).map((o) => o.getAttribute("value"))
    expect(modelOpts).toContain("gpt-5.5")
    expect(modelOpts).toContain("gpt-5.3-codex")
    const effortOpts = Array.from(
      container.querySelectorAll("#codex-effort-suggestions option"),
    ).map((o) => o.getAttribute("value"))
    expect(effortOpts).toEqual(["low", "medium", "high", "xhigh"])
  })

  it("marks an invalid field with aria-invalid from errors prop", () => {
    const block: CodexBlock = {
      active: "azure",
      system: systemBlock().system,
      azure: {
        base_url: "https://x/openai",
        api_version: "",
        keyring_service: "codebus-azure",
        goal: { model: "d", effort: "high" },
        query: { model: "d", effort: "low" },
        fix: { model: "d", effort: "medium" },
        verify: { model: "d", effort: "high" },
      },
    }
    render(
      <CodexEndpointSection
        block={block}
        onChange={() => {}}
        errors={[
          {
            field: "codex.azure.api_version",
            key: "settings.endpoint.validation.apiVersionRequired",
          },
        ]}
      />,
    )
    expect(
      screen.getByTestId("codex-azure-api-version").getAttribute("aria-invalid"),
    ).toBe("true")
  })

  // Mirrors EndpointSection's `endpoint-chat-row`: chat reuses query's
  // model/effort by design (Verb::Chat → query in codex.rs:98), so each
  // profile section shows a read-only hint reflecting that inheritance.
  it("renders a read-only chat hint row in each codex profile reflecting query model/effort", () => {
    const block: CodexBlock = {
      active: "system",
      system: systemBlock().system,
      azure: {
        base_url: "https://x/openai",
        api_version: "2025-04-01-preview",
        keyring_service: "codebus-codex-azure",
        goal: { model: "azure-deploy-x", effort: "high" },
        query: { model: "azure-deploy-x", effort: "low" },
        fix: { model: "azure-deploy-x", effort: "medium" },
        verify: { model: "azure-deploy-x", effort: "high" },
      },
    }
    render(<CodexEndpointSection block={block} onChange={() => {}} />)
    const systemRow = screen.getByTestId("codex-endpoint-chat-row")
    expect(systemRow.textContent ?? "").toContain("gpt-5.5")
    expect(systemRow.textContent ?? "").toContain("low")
    const azureRow = screen.getByTestId("codex-azure-endpoint-chat-row")
    expect(azureRow.textContent ?? "").toContain("azure-deploy-x")
    expect(azureRow.textContent ?? "").toContain("low")
  })
})
