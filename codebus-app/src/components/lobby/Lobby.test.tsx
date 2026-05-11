import { describe, expect, it, vi, beforeEach } from "vitest"
import { render, screen, fireEvent } from "@testing-library/react"

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}))

import { invoke } from "@tauri-apps/api/core"
import { Lobby } from "./Lobby"
import { useVaultsStore } from "@/store/vaults"
import { useRouteStore } from "@/store/route"

const mockedInvoke = vi.mocked(invoke)

function seed(vaults: Parameters<typeof useVaultsStore.setState>[0] extends infer S ? Extract<S, { vaults: unknown }> extends never ? never : never : never) {
  void vaults
}

function entry(path: string, isMissing = false) {
  return {
    path,
    display_name: path.split("/").pop() ?? path,
    last_opened: "2026-05-11T00:00:00Z",
    is_missing: isMissing,
  }
}

describe("Lobby", () => {
  beforeEach(() => {
    seed
    mockedInvoke.mockReset()
    mockedInvoke.mockResolvedValue([])
    useVaultsStore.setState({ vaults: [], loading: false, error: null })
    useRouteStore.setState({ route: { kind: "lobby" } })
  })

  it("renders the empty state when vault list is empty", async () => {
    render(
      <Lobby onNewVault={() => {}} onRevealInFiles={() => {}} />,
    )
    expect(await screen.findByTestId("lobby-empty")).toBeInTheDocument()
    expect(screen.getByTestId("lobby")).toHaveAttribute("data-state", "empty")
  })

  it("renders vault cards when populated, including missing badge", () => {
    useVaultsStore.setState({
      vaults: [entry("/alpha"), entry("/beta", true)],
    })
    render(<Lobby onNewVault={() => {}} onRevealInFiles={() => {}} />)
    expect(screen.getByTestId("lobby")).toHaveAttribute("data-state", "populated")
    expect(screen.getByTestId("vault-card-/alpha")).toBeInTheDocument()
    expect(screen.getByTestId("vault-card-/beta")).toBeInTheDocument()
    expect(screen.getByTestId("missing-badge")).toBeInTheDocument()
  })

  it("invokes onNewVault when populated New Vault button is clicked", () => {
    useVaultsStore.setState({ vaults: [entry("/alpha")] })
    const onNewVault = vi.fn()
    render(<Lobby onNewVault={onNewVault} onRevealInFiles={() => {}} />)
    fireEvent.click(screen.getByTestId("new-vault-button"))
    expect(onNewVault).toHaveBeenCalledTimes(1)
  })

  it("opens the workspace route when a vault card is clicked", () => {
    useVaultsStore.setState({ vaults: [entry("/alpha")] })
    render(<Lobby onNewVault={() => {}} onRevealInFiles={() => {}} />)
    fireEvent.click(screen.getByTestId("vault-card-/alpha"))
    const route = useRouteStore.getState().route
    expect(route.kind).toBe("workspace-stub")
    if (route.kind === "workspace-stub") {
      expect(route.vault.path).toBe("/alpha")
    }
  })
})
