import { fireEvent, render, screen } from "@testing-library/react"
import { describe, expect, it, vi } from "vitest"

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }))

import { invoke } from "@tauri-apps/api/core"
import type { WikiPageMeta } from "@/lib/ipc"
import {
  WikilinkLink,
  transformBodyWikilinks,
} from "./milkdown-wikilink"

const invokeMock = vi.mocked(invoke)

function makePages(...slugs: string[]): Record<string, WikiPageMeta> {
  const out: Record<string, WikiPageMeta> = {}
  for (const slug of slugs) {
    out[slug] = {
      slug,
      path: `/v/.codebus/wiki/modules/${slug}.md`,
      title: slug,
      goals: [],
      updated: "",
    }
  }
  return out
}

describe("WikilinkLink", () => {
  it("wikilink_plugin_renders_resolvable_link_clickable", () => {
    const onResolve = vi.fn()
    render(
      <WikilinkLink
        slug="uv-lib"
        pages={makePages("uv-lib")}
        onResolve={onResolve}
      />,
    )
    const anchor = screen.getByTestId("wikilink-uv-lib")
    expect(anchor.tagName).toBe("A")
    expect(anchor.getAttribute("data-state")).toBe("resolvable")
    // WP11 design v1.1: resolvable body wikilinks carry plain-wikilink.
    expect(anchor.className).toMatch(/plain-wikilink/)
    fireEvent.click(anchor)
    expect(onResolve).toHaveBeenCalledWith("uv-lib")
  })

  it("wikilink_plugin_unresolvable_does_not_carry_plain_wikilink_class", () => {
    const onResolve = vi.fn()
    render(
      <WikilinkLink
        slug="missing"
        pages={makePages("uv-lib")}
        onResolve={onResolve}
      />,
    )
    const el = screen.getByTestId("wikilink-missing")
    // Unresolvable wikilinks keep the dimmed disabled look; the
    // plain-wikilink visual variant only applies to resolvable links.
    expect(el.className).not.toMatch(/plain-wikilink/)
  })

  it("wikilink_plugin_renders_unresolvable_disabled", () => {
    const onResolve = vi.fn()
    render(
      <WikilinkLink
        slug="missing"
        pages={makePages("uv-lib")}
        onResolve={onResolve}
      />,
    )
    const el = screen.getByTestId("wikilink-missing")
    expect(el.getAttribute("data-state")).toBe("unresolvable")
    expect(el.getAttribute("title")).toBe("Page not found")
    fireEvent.click(el)
    expect(onResolve).not.toHaveBeenCalled()
  })

  it("wikilink_click_does_not_invoke_list_wiki_pages_ipc", () => {
    render(
      <WikilinkLink
        slug="uv-lib"
        pages={makePages("uv-lib")}
        onResolve={() => {}}
      />,
    )
    fireEvent.click(screen.getByTestId("wikilink-uv-lib"))
    expect(invokeMock).not.toHaveBeenCalled()
  })
})

describe("transformBodyWikilinks", () => {
  it("replaces [[slug]] with markdown anchors using codebus:// scheme", () => {
    const { transformed, slugs } = transformBodyWikilinks(
      "see [[uv-lib]] and [[uv-child]] for details",
    )
    expect(slugs).toEqual(["uv-lib", "uv-child"])
    expect(transformed).toContain("[uv-lib](codebus://wiki/uv-lib)")
    expect(transformed).toContain("[uv-child](codebus://wiki/uv-child)")
  })

  it("encodes special characters in slugs", () => {
    const { transformed } = transformBodyWikilinks("[[a b]]")
    expect(transformed).toBe("[a b](codebus://wiki/a%20b)")
  })

  it("returns input unchanged when no wikilinks present", () => {
    const { transformed, slugs } = transformBodyWikilinks("# heading\nbody")
    expect(transformed).toBe("# heading\nbody")
    expect(slugs).toEqual([])
  })
})
