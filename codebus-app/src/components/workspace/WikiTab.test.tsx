import { fireEvent, render, screen } from "@testing-library/react"
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

import { WikiTab } from "./WikiTab"
import { useWikiStore } from "@/store/wiki"

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

  it("WikiTab_empty_state_shows_hint", () => {
    useWikiStore.setState({ pages: {} })
    render(<WikiTab vaultPath="/v" />)
    expect(screen.getByTestId("wiki-empty")).toHaveTextContent(
      "No wiki pages yet — run a goal to start documenting",
    )
  })
})
