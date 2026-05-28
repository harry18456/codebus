import { fireEvent, render, screen } from "@testing-library/react"
import { describe, expect, it, vi } from "vitest"

import type { WikiPageMeta } from "@/lib/ipc"
import { WikiTree } from "./WikiTree"

function p(slug: string, folder: string): WikiPageMeta {
  return {
    slug,
    path: `/v/.codebus/wiki/${folder}/${slug}.md`,
    title: slug,
    goals: [],
    updated: "",
  }
}

describe("WikiTree", () => {
  it("WikiTree_groups_pages_by_taxonomy_folder", () => {
    const pages: Record<string, WikiPageMeta> = {
      "auth": p("auth", "concepts"),
      "user": p("user", "entities"),
      "uv-lib": p("uv-lib", "modules"),
      "ingest": p("ingest", "processes"),
      "summary": p("summary", "synthesis"),
    }
    render(
      <WikiTree pages={pages} currentSlug={null} onSelectSlug={() => {}} />,
    )
    expect(screen.getByTestId("wiki-tree-group-concepts")).toBeInTheDocument()
    expect(screen.getByTestId("wiki-tree-group-entities")).toBeInTheDocument()
    expect(screen.getByTestId("wiki-tree-group-modules")).toBeInTheDocument()
    expect(screen.getByTestId("wiki-tree-group-processes")).toBeInTheDocument()
    expect(screen.getByTestId("wiki-tree-group-synthesis")).toBeInTheDocument()
  })

  it("renders rows that map to pages and trigger onSelectSlug", () => {
    const pages: Record<string, WikiPageMeta> = {
      a: p("a", "concepts"),
      b: p("b", "modules"),
    }
    const onSelect = vi.fn()
    render(
      <WikiTree pages={pages} currentSlug={null} onSelectSlug={onSelect} />,
    )
    fireEvent.click(screen.getByTestId("wiki-tree-row-a"))
    expect(onSelect).toHaveBeenCalledWith("a")
  })

  // ---- wiki-page-reader-v1.1: WK2 / WP-tree-footer ----

  it("WikiTree_renders_Wiki_Index_at_top_when_present", () => {
    const indexPage: WikiPageMeta = {
      slug: "index",
      path: "/v/.codebus/wiki/index.md",
      title: "Wiki Index",
      goals: [],
      updated: "",
    }
    const pages: Record<string, WikiPageMeta> = {
      index: indexPage,
      auth: p("auth", "concepts"),
    }
    const { container } = render(
      <WikiTree pages={pages} currentSlug={null} onSelectSlug={() => {}} />,
    )
    const indexRow = screen.getByTestId("wiki-tree-row-index")
    const conceptsHeader = screen.getByTestId("wiki-tree-group-concepts")
    // Index row's DOM position is earlier than any taxonomy bucket
    // header — confirms top-of-tree placement.
    const nav = container.querySelector('[data-testid="wiki-tree"]')!
    const indexIdx = Array.from(nav.querySelectorAll("*")).indexOf(indexRow)
    const conceptsIdx = Array.from(nav.querySelectorAll("*")).indexOf(
      conceptsHeader,
    )
    expect(indexIdx).toBeGreaterThanOrEqual(0)
    expect(conceptsIdx).toBeGreaterThan(indexIdx)
  })

  it("WikiTree_renders_travel_log_in_footer_slot_and_invokes_callback", () => {
    const logPage: WikiPageMeta = {
      slug: "log",
      path: "/v/.codebus/wiki/log.md",
      title: "Travel log",
      goals: [],
      updated: "",
    }
    const pages: Record<string, WikiPageMeta> = {
      log: logPage,
      auth: p("auth", "concepts"),
    }
    const onSelect = vi.fn()
    render(
      <WikiTree pages={pages} currentSlug={null} onSelectSlug={onSelect} />,
    )
    const footer = screen.getByTestId("wiki-tree-footer-slot")
    expect(footer).toBeInTheDocument()
    expect(footer.textContent).toMatch(/Travel log|旅行日誌/)
    fireEvent.click(screen.getByTestId("wiki-tree-row-log"))
    expect(onSelect).toHaveBeenCalledWith("log")
  })

  it("WikiTree_does_not_render_an_OTHER_bucket", () => {
    // Pages outside the five-bucket taxonomy stay reachable via the
    // page index (loadPage), but the tree SHALL NOT render an OTHER
    // bucket header for them.
    const pages: Record<string, WikiPageMeta> = {
      stray: {
        slug: "stray",
        path: "/v/.codebus/wiki/stray.md",
        title: "Stray Page",
        goals: [],
        updated: "",
      },
      auth: p("auth", "concepts"),
    }
    render(
      <WikiTree pages={pages} currentSlug={null} onSelectSlug={() => {}} />,
    )
    expect(screen.queryByTestId("wiki-tree-group-other")).toBeNull()
    // Concepts bucket still renders for the real-taxonomy page.
    expect(screen.getByTestId("wiki-tree-group-concepts")).toBeInTheDocument()
  })
})
