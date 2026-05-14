import { fireEvent, render, screen } from "@testing-library/react"
import { describe, expect, it, vi } from "vitest"

import type { WikiPageMeta } from "@/lib/ipc"
import { WikiTree } from "./WikiTree"

function p(slug: string, folder: string): WikiPageMeta {
  return {
    slug,
    path: `/v/.codebus/wiki/${folder}/${slug}.md`,
    title: slug,
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
})
