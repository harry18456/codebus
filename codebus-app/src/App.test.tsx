import { beforeEach, describe, expect, it, vi } from "vitest"
import { fireEvent, render, screen, waitFor } from "@testing-library/react"

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}))
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
}))
vi.mock("@tauri-apps/plugin-dialog", () => ({
  open: vi.fn(),
}))
vi.mock("@milkdown/core", () => ({
  Editor: {
    make: vi.fn(() => ({
      config: vi.fn(function (this: object) {
        return this
      }),
      use: vi.fn(function (this: object) {
        return this
      }),
      create: vi.fn(() => Promise.resolve({ destroy: vi.fn() })),
    })),
  },
  rootCtx: "rootCtx",
  defaultValueCtx: "defaultValueCtx",
  editorViewOptionsCtx: "editorViewOptionsCtx",
}))
vi.mock("@milkdown/preset-commonmark", () => ({ commonmark: () => ({}) }))

import { invoke } from "@tauri-apps/api/core"
import { App } from "./App"
import { useSettingsStore } from "@/store/settings"
import { useVaultsStore } from "@/store/vaults"
import { useRouteStore } from "@/store/route"

const mockedInvoke = vi.mocked(invoke)

/**
 * The app SHALL preload the global settings at startup so
 * `app.locale_override` is honored from the very first render (no language
 * flash to the system locale before Workspace / SettingsModal triggers a
 * load). Backs spec scenario "Locale override survives application restart".
 */
describe("App preloads settings on mount", () => {
  beforeEach(() => {
    mockedInvoke.mockReset()
    useSettingsStore.setState({
      config: {},
      initialConfig: {},
      dirty: false,
      loading: false,
      saving: false,
      error: null,
    })
    useVaultsStore.setState({ vaults: [], loading: false, error: null })
    useRouteStore.setState({ route: { kind: "lobby" } })
  })

  it("invokes load_global_config once at mount", async () => {
    mockedInvoke.mockImplementation((cmd: string) => {
      if (cmd === "load_global_config")
        return Promise.resolve({ app: { locale_override: "en" } })
      // Other IPC commands (list_vaults, etc.) — return safe defaults.
      if (cmd === "list_vaults") return Promise.resolve([])
      return Promise.resolve(null)
    })
    render(<App />)
    await waitFor(
      () => {
        const calls = mockedInvoke.mock.calls.map((c) => c[0])
        expect(calls).toContain("load_global_config")
      },
      { timeout: 2000 },
    )
    await waitFor(() => {
      const cfg = useSettingsStore.getState().config as {
        app?: { locale_override?: string | null }
      }
      expect(cfg.app?.locale_override).toBe("en")
    })
  })

  it("does not fail when load_global_config rejects", async () => {
    mockedInvoke.mockImplementation((cmd: string) => {
      if (cmd === "load_global_config")
        return Promise.reject({ kind: "io", message: "test" })
      if (cmd === "list_vaults") return Promise.resolve([])
      return Promise.resolve(null)
    })
    // Should not throw.
    expect(() => render(<App />)).not.toThrow()
    await waitFor(() => {
      const calls = mockedInvoke.mock.calls.map((c) => c[0])
      expect(calls).toContain("load_global_config")
    })
  })
})

/**
 * BottomStrip lives at the application shell level and SHALL render ONLY
 * when the route is Lobby (spec: `app-shell` § Lobby Two-State Rendering).
 * Workspace SHALL invoke Settings via its sidebar footer (spec: `app-shell` §
 * Settings Modal Invocation From Workspace Sidebar Footer) and the modal
 * instance SHALL be the same one Lobby uses.
 */
describe("BottomStrip Lobby-only render + Workspace settings invocation", () => {
  beforeEach(() => {
    mockedInvoke.mockReset()
    mockedInvoke.mockImplementation((cmd: string) => {
      if (cmd === "load_global_config") return Promise.resolve({})
      if (cmd === "list_vaults") return Promise.resolve([])
      return Promise.resolve([])
    })
    useSettingsStore.setState({
      config: {},
      initialConfig: {},
      dirty: false,
      loading: false,
      saving: false,
      error: null,
    })
    useVaultsStore.setState({ vaults: [], loading: false, error: null })
    useRouteStore.setState({ route: { kind: "lobby" } })
  })

  it("renders BottomStrip when route is Lobby", async () => {
    render(<App />)
    await waitFor(() => {
      expect(screen.getByTestId("bottom-strip")).toBeInTheDocument()
    })
  })

  it("does not render BottomStrip when route is Workspace", async () => {
    useRouteStore.setState({
      route: {
        kind: "workspace",
        vault: {
          path: "/v",
          display_name: "v",
          last_opened: "2026-05-27T00:00:00Z",
          is_missing: false,
        },
      },
    })
    render(<App />)
    await waitFor(() => {
      expect(screen.getByTestId("workspace")).toBeInTheDocument()
    })
    expect(screen.queryByTestId("bottom-strip")).not.toBeInTheDocument()
  })

  it("Workspace sidebar settings button opens the same SettingsModal as the Lobby gear", async () => {
    useRouteStore.setState({
      route: {
        kind: "workspace",
        vault: {
          path: "/v",
          display_name: "v",
          last_opened: "2026-05-27T00:00:00Z",
          is_missing: false,
        },
      },
    })
    render(<App />)
    await waitFor(() => {
      expect(screen.getByTestId("workspace")).toBeInTheDocument()
    })
    // Sidebar Settings button SHALL be present and SHALL open the same
    // single SettingsModal instance owned by the app shell.
    const sidebarSettings = screen.getByTestId("workspace-sidebar-settings")
    fireEvent.click(sidebarSettings)
    // Settings modal renders into a portal; the Radix mount is non-atomic, so
    // findBy (retries until present) is used over waitFor+getBy, with a raised
    // timeout for slow/loaded CI environments.
    await screen.findByTestId("settings-modal", undefined, { timeout: 4000 })
    // Only one modal instance in the DOM.
    expect(screen.getAllByTestId("settings-modal")).toHaveLength(1)
  })
})
