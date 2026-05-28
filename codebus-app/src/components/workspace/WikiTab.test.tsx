import { fireEvent, render, screen, waitFor } from "@testing-library/react"
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest"

const setMock = vi.fn()
const editorChain = {
  config: vi.fn(function (this: typeof editorChain, cb: (ctx: { set: typeof setMock }) => void) {
    cb({ set: setMock })
    return this
  }),
  use: vi.fn(function (this: typeof editorChain) {
    return this
  }),
  create: vi.fn(() => Promise.resolve({ destroy: vi.fn() })),
}

vi.mock("@milkdown/core", () => ({
  Editor: { make: vi.fn(() => editorChain) },
  rootCtx: "rootCtx",
  defaultValueCtx: "defaultValueCtx",
  editorViewOptionsCtx: "editorViewOptionsCtx",
}))
vi.mock("@milkdown/preset-commonmark", () => ({ commonmark: () => ({}) }))
vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }))
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
}))

import { listen } from "@tauri-apps/api/event"
import { WikiTab } from "./WikiTab"
import { useWikiStore } from "@/store/wiki"
import { useVaultWatcherStatusStore } from "@/store/vault-watcher-status"

const mockedListen = vi.mocked(listen)

describe("WikiTab", () => {
  beforeEach(() => {
    useWikiStore.setState({
      pages: {},
      currentPath: null,
      body: null,
      _bodyCache: {},
    })
  })

  afterEach(() => {
    useWikiStore.setState({
      pages: {},
      currentPath: null,
      body: null,
      _bodyCache: {},
    })
  })

  it("WikiTab_mounts_with_tree_expanded", () => {
    useWikiStore.setState({
      pages: {
        a: {
          slug: "a",
          path: "/v/.codebus/wiki/modules/a.md",
          title: "A",
        },
      },
    })
    render(<WikiTab vaultPath="/v" />)
    // Tree IS rendered by default — Wiki tab opens with tree expanded
    // (matches usability research: most users come to Wiki to browse,
    // not to read a single open page).
    expect(screen.getByTestId("wiki-tree")).toBeInTheDocument()
    expect(screen.getByTestId("wiki-tree-toggle")).toHaveAttribute(
      "aria-pressed",
      "true",
    )
  })

  it("WikiTab_toggle_button_collapses_tree", () => {
    useWikiStore.setState({
      pages: {
        a: {
          slug: "a",
          path: "/v/.codebus/wiki/modules/a.md",
          title: "A",
        },
      },
    })
    render(<WikiTab vaultPath="/v" />)
    // From the default expanded state, clicking the toggle hides the tree.
    fireEvent.click(screen.getByTestId("wiki-tree-toggle"))
    expect(screen.queryByTestId("wiki-tree")).toBeNull()
  })

  it("WikiTab_empty_state_renders_hero_with_icon_title_subtitle_and_amber_cta", () => {
    useWikiStore.setState({ pages: {} })
    render(<WikiTab vaultPath="/v" />)
    const hero = screen.getByTestId("wiki-empty-hero")
    expect(hero).toBeInTheDocument()
    expect(hero.textContent).toMatch(/No wiki pages yet|還沒有任何 wiki page/)
    expect(hero.textContent).toMatch(
      /Run a goal — codebus|跑一個 goal，codebus/,
    )
    const cta = screen.getByTestId("wiki-empty-cta")
    expect(cta).toBeInTheDocument()
    expect(cta.textContent).toMatch(/Run a goal to start|跑一個 goal 開始/)
    // CTA uses amber primary variant.
    expect(cta.className).toMatch(/bg-accent/)
    // The deprecated single-line hint SHALL NOT render alongside the hero.
    expect(screen.queryByTestId("wiki-empty")).toBeNull()
  })

  it("WikiTab_empty_cta_invokes_onWikiEmptyCta", () => {
    useWikiStore.setState({ pages: {} })
    const onWikiEmptyCta = vi.fn()
    render(<WikiTab vaultPath="/v" onWikiEmptyCta={onWikiEmptyCta} />)
    fireEvent.click(screen.getByTestId("wiki-empty-cta"))
    expect(onWikiEmptyCta).toHaveBeenCalledTimes(1)
  })

  // ---- Watcher integration (codebus-fs-watcher) ----

  it("error_event_disables_autorefresh_and_shows_indicator", () => {
    useVaultWatcherStatusStore.setState({
      disabledVaults: { "/v": "ENOSPC: inotify watch limit exhausted" },
    })
    useWikiStore.setState({
      pages: { a: { slug: "a", path: "/v/.codebus/wiki/a.md", title: "A" } },
    })
    render(<WikiTab vaultPath="/v" />)
    const banner = screen.getByTestId("watcher-status-banner")
    expect(banner).toHaveTextContent(/auto-refresh disabled/i)
    expect(banner).toHaveTextContent("ENOSPC")
    // Reset to keep test isolation tight.
    useVaultWatcherStatusStore.setState({ disabledVaults: {} })
  })

  it("external_add_refreshes_tree", async () => {
    let capturedCallback: ((ev: { payload: unknown }) => void) | undefined
    mockedListen.mockImplementation(async (name, cb) => {
      if (name === "wiki-list-changed") {
        capturedCallback = cb as (ev: { payload: unknown }) => void
      }
      return () => {}
    })
    const listPagesSpy = vi.fn(async () => {})
    useWikiStore.setState({
      pages: { a: { slug: "a", path: "/v/.codebus/wiki/a.md", title: "A" } },
      listPages: listPagesSpy as never,
    })

    render(<WikiTab vaultPath="/v" />)
    await waitFor(() => expect(capturedCallback).toBeTruthy())

    capturedCallback?.({ payload: null })
    await waitFor(() => expect(listPagesSpy).toHaveBeenCalledWith("/v"))
  })
})
