import { render, screen } from "@testing-library/react"
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest"

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
}))
vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }))

// Milkdown mocks are needed because the WikiTab path (even when not
// active) is imported through Workspace -> WikiTab -> WikiPreview.
vi.mock("@milkdown/core", () => ({
  Editor: { make: vi.fn(() => ({
    config: vi.fn(function (this: object) { return this }),
    use: vi.fn(function (this: object) { return this }),
    create: vi.fn(() => Promise.resolve({ destroy: vi.fn() })),
  })) },
  rootCtx: "rootCtx",
  defaultValueCtx: "defaultValueCtx",
  editorViewOptionsCtx: "editorViewOptionsCtx",
}))
vi.mock("@milkdown/preset-commonmark", () => ({ commonmark: () => ({}) }))

import { invoke } from "@tauri-apps/api/core"
import type { VaultEntry } from "@/lib/ipc"
import { Workspace } from "./Workspace"
import { useGoalsStore } from "@/store/goals"
import { useWikiStore } from "@/store/wiki"

const invokeMock = vi.mocked(invoke)

const VAULT: VaultEntry = {
  path: "/v",
  display_name: "vault",
  last_opened: "2026-05-13T00:00:00Z",
  is_missing: false,
}

describe("Workspace", () => {
  beforeEach(() => {
    invokeMock.mockReset()
    invokeMock.mockResolvedValue([])
    useGoalsStore.setState({ runs: [], activeRun: null })
    useWikiStore.setState({
      pages: {},
      currentPath: null,
      body: null,
      _bodyCache: {},
    })
  })

  afterEach(() => {
    useGoalsStore.setState({ runs: [], activeRun: null })
    useWikiStore.setState({
      pages: {},
      currentPath: null,
      body: null,
      _bodyCache: {},
    })
  })

  it("Workspace_mounts_with_goals_tab_default", () => {
    render(<Workspace vault={VAULT} />)
    const goalsTabBtn = screen.getByTestId("workspace-tab-goals")
    expect(goalsTabBtn.getAttribute("data-active")).toBe("true")
    const main = screen.getByTestId("workspace-main")
    expect(main).toContainElement(screen.getByTestId("goals-tab"))
  })

  it("renders the vault display name and path in the sidebar", () => {
    render(<Workspace vault={VAULT} />)
    expect(screen.getByTestId("workspace-vault-name")).toHaveTextContent(
      "vault",
    )
    expect(screen.getByTestId("workspace-vault-path")).toHaveTextContent("/v")
  })
})
