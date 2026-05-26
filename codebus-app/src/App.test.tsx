import { beforeEach, describe, expect, it, vi } from "vitest"
import { render, waitFor } from "@testing-library/react"

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
