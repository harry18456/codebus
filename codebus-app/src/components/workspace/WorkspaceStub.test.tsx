import { describe, expect, it, vi, beforeEach } from "vitest"
import { render, screen, fireEvent } from "@testing-library/react"

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}))

import { invoke } from "@tauri-apps/api/core"
import { WorkspaceStub } from "./WorkspaceStub"
import { useRouteStore } from "@/store/route"
import { useVaultsStore } from "@/store/vaults"

const mockedInvoke = vi.mocked(invoke)

const VAULT = {
  path: "/path/to/repo",
  display_name: "repo",
  last_opened: "2026-05-11T00:00:00Z",
  is_missing: false,
}

describe("WorkspaceStub", () => {
  beforeEach(() => {
    mockedInvoke.mockReset()
    mockedInvoke.mockResolvedValue([])
    useRouteStore.setState({ route: { kind: "workspace-stub", vault: VAULT } })
    useVaultsStore.setState({ vaults: [VAULT], loading: false, error: null })
  })

  it("renders the three required elements", () => {
    render(<WorkspaceStub vault={VAULT} />)
    expect(screen.getByTestId("workspace-vault-name")).toHaveTextContent("repo")
    expect(screen.getByTestId("workspace-vault-path")).toHaveTextContent(
      "/path/to/repo",
    )
    expect(screen.getByTestId("workspace-coming-soon")).toHaveTextContent(
      "Workspace coming in v3-app-workspace-goal",
    )
    expect(screen.getByTestId("workspace-back")).toBeInTheDocument()
  })

  it("MUST NOT render any deferred-Workspace element", () => {
    render(<WorkspaceStub vault={VAULT} />)
    const stub = screen.getByTestId("workspace-stub")
    const text = stub.textContent ?? ""
    // Wiki tree
    expect(text.toLowerCase()).not.toContain("wiki tree")
    expect(text.toLowerCase()).not.toContain("📂 wiki")
    // Goal list / goal flow surface
    expect(text.toLowerCase()).not.toContain("goal list")
    expect(text.toLowerCase()).not.toContain("running goal")
    // Quiz
    expect(text.toLowerCase()).not.toContain("quiz")
    // Cmd+K UI
    expect(text.toLowerCase()).not.toContain("⌘k")
    expect(text.toLowerCase()).not.toContain("cmd+k")
  })

  it("Back to Lobby returns to lobby route and reloads vault list", () => {
    render(<WorkspaceStub vault={VAULT} />)
    fireEvent.click(screen.getByTestId("workspace-back"))
    expect(useRouteStore.getState().route.kind).toBe("lobby")
    expect(mockedInvoke).toHaveBeenCalledWith("list_vaults", undefined)
  })
})
