import { fireEvent, render, screen, waitFor } from "@testing-library/react"
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest"

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }))
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
}))

import { invoke } from "@tauri-apps/api/core"
import { listen } from "@tauri-apps/api/event"
import { WikiPreview } from "./WikiPreview"
import { useWikiStore } from "@/store/wiki"

const invokeMock = vi.mocked(invoke)
const mockedListen = vi.mocked(listen)

describe("WikiPreview", () => {
  beforeEach(() => {
    invokeMock.mockReset()
    useWikiStore.setState({
      pages: {},
      currentPath: null,
      body: null,
      obsidianVaultId: null,
      _bodyCache: {},
    })
  })

  afterEach(() => {
    useWikiStore.setState({
      pages: {},
      currentPath: null,
      body: null,
      obsidianVaultId: null,
      _bodyCache: {},
    })
  })

  it("WikiPreview_renders_markdown_with_visual_hierarchy", () => {
    render(
      <WikiPreview
        vaultPath="/v"
        body={"# Title\n\nsome **bold** body\n\n- item one\n- item two"}
      />,
    )
    // Heading + list + bold all surface in the DOM.
    expect(screen.getByText("Title").tagName).toBe("H1")
    expect(screen.getByText("bold").tagName).toBe("STRONG")
    expect(screen.getByText("item one").tagName).toBe("LI")
  })

  it("WikiPreview_renders_nothing_when_body_is_null", () => {
    render(<WikiPreview vaultPath="/v" body={null} />)
    // Container still mounts, but no markdown content.
    const container = screen.getByTestId("wiki-preview")
    expect(container.textContent ?? "").toBe("")
  })

  it("WikiPreview_renders_wikilinks_as_clickable_anchors_with_title_text", async () => {
    // Seed the page index so the wikilink resolves and the anchor
    // displays the page title (not the raw slug).
    useWikiStore.setState({
      pages: {
        "uv-lib": {
          slug: "uv-lib",
          path: "/v/.codebus/wiki/modules/uv-lib.md",
          title: "UV Library Entry",
        },
      },
      currentPath: null,
      body: null,
      _bodyCache: {},
    })
    invokeMock.mockResolvedValueOnce("body for uv-lib")
    render(
      <WikiPreview
        vaultPath="/v"
        body={"see [[uv-lib]] for details"}
      />,
    )
    // Anchor displays the title, not the slug.
    const link = screen.getByText("UV Library Entry")
    expect(link.tagName).toBe("A")
    expect(link.getAttribute("data-wikilink")).toBe("uv-lib")
    expect(link.getAttribute("data-state")).toBe("resolvable")
    fireEvent.click(link)
    expect(invokeMock).toHaveBeenCalledWith("read_wiki_page", {
      vaultPath: "/v",
      pageSlug: "uv-lib",
    })
  })

  it("WikiPreview_renders_unresolvable_wikilink_as_dimmed_span_with_slug", () => {
    useWikiStore.setState({
      pages: {},
      currentPath: null,
      body: null,
      _bodyCache: {},
    })
    render(
      <WikiPreview
        vaultPath="/v"
        body={"see [[missing-page]] for details"}
      />,
    )
    const el = screen.getByText("missing-page")
    expect(el.tagName).toBe("SPAN")
    expect(el.getAttribute("data-state")).toBe("unresolvable")
    expect(el.getAttribute("title")).toBe("Page not found")
  })

  it("WikiPreview_renders_code_blocks_and_inline_code", () => {
    render(
      <WikiPreview
        vaultPath="/v"
        body={"use `inline` and:\n\n```\nblock\n```\n"}
      />,
    )
    expect(screen.getByText("inline").tagName).toBe("CODE")
    expect(screen.getByText("block").tagName).toBe("CODE")
  })

  // --- task 5.3: [Quiz me on this] trigger ---
  // Spec: app-workspace § Quiz Tab Plan-Confirm-Generate Flow —
  // "Quiz-me-on-this" appears on content pages only (index.md / log.md
  // excluded) and starts the Page flow with the current page path.

  it("shows [Quiz me on this] on a content page and reports its path", () => {
    useWikiStore.setState({
      currentPath: "/v/.codebus/wiki/modules/auth-middleware.md",
    })
    const onQuiz = vi.fn()
    render(
      <WikiPreview
        vaultPath="/v"
        body={"# Auth\n\nbody"}
        onQuizMeOnThis={onQuiz}
      />,
    )
    const btn = screen.getByTestId("quiz-me-on-this")
    expect(btn).toBeInTheDocument()
    fireEvent.click(btn)
    expect(onQuiz).toHaveBeenCalledWith(
      "/v/.codebus/wiki/modules/auth-middleware.md",
    )
  })

  it("hides [Quiz me on this] on index.md nav page", () => {
    useWikiStore.setState({
      currentPath: "/v/.codebus/wiki/index.md",
    })
    render(<WikiPreview vaultPath="/v" body={"# Index"} />)
    expect(screen.queryByTestId("quiz-me-on-this")).not.toBeInTheDocument()
  })

  it("hides [Quiz me on this] on log.md nav page", () => {
    useWikiStore.setState({
      currentPath: "/v/.codebus/wiki/log.md",
    })
    render(<WikiPreview vaultPath="/v" body={"# Log"} />)
    expect(screen.queryByTestId("quiz-me-on-this")).not.toBeInTheDocument()
  })

  // --- task 4.1: [Open in Obsidian] button ---
  // Spec: app-workspace § Open Wiki Page In Obsidian — the button renders
  // iff the store's cached `obsidianVaultId` is non-null, on BOTH content
  // and nav pages (unlike Quiz which is content-only), and clicking it
  // invokes `open_wiki_in_obsidian` once with the current page's slug.

  it("shows [Open in Obsidian] alongside Quiz on a content page when vault id is present", () => {
    useWikiStore.setState({
      currentPath: "uv-lib",
      obsidianVaultId: "abc123def456abcd",
    })
    render(<WikiPreview vaultPath="/v" body={"# uv-lib\n\nbody"} />)
    expect(screen.getByTestId("open-in-obsidian")).toBeInTheDocument()
    expect(screen.getByTestId("quiz-me-on-this")).toBeInTheDocument()
  })

  it("shows [Open in Obsidian] (but not Quiz) on a nav page when vault id is present", () => {
    useWikiStore.setState({
      currentPath: "/v/.codebus/wiki/index.md",
      obsidianVaultId: "abc123def456abcd",
    })
    render(<WikiPreview vaultPath="/v" body={"# Index"} />)
    expect(screen.getByTestId("open-in-obsidian")).toBeInTheDocument()
    expect(screen.queryByTestId("quiz-me-on-this")).not.toBeInTheDocument()
  })

  it("hides [Open in Obsidian] entirely when vault id is null", () => {
    useWikiStore.setState({
      currentPath: "uv-lib",
      obsidianVaultId: null,
    })
    render(<WikiPreview vaultPath="/v" body={"# uv-lib"} />)
    expect(screen.queryByTestId("open-in-obsidian")).not.toBeInTheDocument()
  })

  it("clicking [Open in Obsidian] invokes open_wiki_in_obsidian once with the current slug", () => {
    invokeMock.mockResolvedValue(undefined)
    useWikiStore.setState({
      currentPath: "uv-lib",
      obsidianVaultId: "abc123def456abcd",
    })
    render(<WikiPreview vaultPath="/v" body={"# uv-lib"} />)
    fireEvent.click(screen.getByTestId("open-in-obsidian"))
    expect(invokeMock).toHaveBeenCalledTimes(1)
    expect(invokeMock).toHaveBeenCalledWith("open_wiki_in_obsidian", {
      vaultPath: "/v",
      slug: "uv-lib",
    })
  })

  // ---- Watcher integration (codebus-fs-watcher) ----

  it("external_edit_open_page_refetches", async () => {
    let capturedCallback:
      | ((ev: { payload: { path: string } }) => void)
      | undefined
    mockedListen.mockImplementation(async (name, cb) => {
      if (name === "wiki-page-changed") {
        capturedCallback = cb as (ev: { payload: { path: string } }) => void
      }
      return () => {}
    })
    const loadPageSpy = vi.fn(async () => {})
    // `slug` is `path.file_stem()` (the basename without `.md`), NOT
    // the relative path under wiki/. See
    // `codebus-app/src-tauri/src/ipc/wiki.rs::list_wiki_pages_impl`.
    useWikiStore.setState({
      currentPath: "foo",
      body: "old",
      loadPage: loadPageSpy as never,
    })

    render(<WikiPreview vaultPath="/v" body={"old"} />)
    await waitFor(() => expect(capturedCallback).toBeTruthy())

    capturedCallback?.({
      payload: { path: "/v/.codebus/wiki/concepts/foo.md" },
    })
    await waitFor(() => expect(loadPageSpy).toHaveBeenCalledWith("/v", "foo"))
  })

  it("external_edit_other_page_is_ignored", async () => {
    let capturedCallback:
      | ((ev: { payload: { path: string } }) => void)
      | undefined
    mockedListen.mockImplementation(async (name, cb) => {
      if (name === "wiki-page-changed") {
        capturedCallback = cb as (ev: { payload: { path: string } }) => void
      }
      return () => {}
    })
    const loadPageSpy = vi.fn(async () => {})
    useWikiStore.setState({
      currentPath: "foo",
      body: "stable",
      loadPage: loadPageSpy as never,
    })

    render(<WikiPreview vaultPath="/v" body={"stable"} />)
    await waitFor(() => expect(capturedCallback).toBeTruthy())

    capturedCallback?.({
      payload: { path: "/v/.codebus/wiki/concepts/other.md" },
    })
    // Allow one microtask + a small delay; loadPage SHALL NOT fire.
    await new Promise((r) => setTimeout(r, 50))
    expect(loadPageSpy).not.toHaveBeenCalled()
  })
})
