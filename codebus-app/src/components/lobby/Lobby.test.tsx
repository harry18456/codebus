import { describe, expect, it, vi, beforeEach } from "vitest"
import { render, screen, fireEvent, waitFor } from "@testing-library/react"

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}))

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(),
}))

import { invoke } from "@tauri-apps/api/core"
import { listen } from "@tauri-apps/api/event"
import { Lobby } from "./Lobby"
import { useVaultsStore } from "@/store/vaults"
import { useRouteStore } from "@/store/route"

const mockedInvoke = vi.mocked(invoke)
const mockedListen = vi.mocked(listen)

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
    mockedListen.mockReset()
    // Default: listen subscribes but never fires. Per-test overrides
    // capture the callback when verifying event-driven reload.
    mockedListen.mockResolvedValue(() => {})
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
    expect(route.kind).toBe("workspace")
    if (route.kind === "workspace") {
      expect(route.vault.path).toBe("/alpha")
    }
  })

  // ---- Lobby watcher integration (settings-config / codebus-fs-watcher) ----

  it("external_vault_add_refreshes_lobby", async () => {
    // Capture the vault-list-changed callback registered by the hook.
    let capturedCallback: ((ev: { payload: unknown }) => void) | undefined
    const unlistenSpy = vi.fn()
    mockedListen.mockImplementation(async (name, cb) => {
      if (name === "vault-list-changed") {
        capturedCallback = cb as (ev: { payload: unknown }) => void
      }
      return unlistenSpy
    })

    // First load returns one vault; second (post-event) returns two.
    mockedInvoke
      .mockResolvedValueOnce([entry("/alpha")])
      .mockResolvedValueOnce([entry("/alpha"), entry("/bravo")])

    render(<Lobby onNewVault={() => {}} onRevealInFiles={() => {}} />)
    await waitFor(() =>
      expect(screen.getByTestId("vault-card-/alpha")).toBeInTheDocument(),
    )
    expect(screen.queryByTestId("vault-card-/bravo")).not.toBeInTheDocument()

    // Wait for hook listen() to resolve and stash the callback.
    await waitFor(() => expect(capturedCallback).toBeTruthy())

    // Fire the Tauri event — Lobby SHALL invoke loadVaults() again.
    capturedCallback?.({ payload: null })
    await waitFor(() =>
      expect(screen.getByTestId("vault-card-/bravo")).toBeInTheDocument(),
    )
  })

  it("unmount_stops_subscription", async () => {
    const unlistenSpy = vi.fn()
    mockedListen.mockResolvedValue(unlistenSpy)

    const { unmount } = render(
      <Lobby onNewVault={() => {}} onRevealInFiles={() => {}} />,
    )
    // Wait for listen() to resolve and the cleanup to capture the
    // unlisten handle.
    await waitFor(() => expect(mockedListen).toHaveBeenCalled())
    await Promise.resolve()

    unmount()
    await waitFor(() => expect(unlistenSpy).toHaveBeenCalled())
  })
})
