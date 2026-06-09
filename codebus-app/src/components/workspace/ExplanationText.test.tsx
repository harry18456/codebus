import { fireEvent, render, screen } from "@testing-library/react"
import { describe, expect, it, vi } from "vitest"

import { InlineMarkdownText } from "./ExplanationText"

describe("InlineMarkdownText", () => {
  it("renders the supported inline markdown subset and resolvable wikilinks", () => {
    const onOpenWikiPage = vi.fn()
    render(
      <p data-testid="inline-text">
        <InlineMarkdownText
          text={
            "Use `codebus-core` with **Rust** and *workspace* from [[desktop-app-workspace]]."
          }
          pages={{
            "desktop-app-workspace": {
              slug: "desktop-app-workspace",
              path: "wiki/modules/desktop-app-workspace.md",
              title: "Desktop App Workspace",
            },
          }}
          onOpenWikiPage={onOpenWikiPage}
        />
      </p>,
    )

    expect(screen.getByText("codebus-core").tagName).toBe("CODE")
    expect(screen.getByText("Rust").tagName).toBe("STRONG")
    expect(screen.getByText("workspace").tagName).toBe("EM")
    const link = screen.getByTestId("wikilink-desktop-app-workspace")
    expect(link.tagName).toBe("A")
    expect(link).toHaveClass("cite-link")
    expect(link).not.toHaveClass("plain-wikilink")
    expect(link).toHaveTextContent("Desktop App Workspace")
    fireEvent.click(link)
    expect(onOpenWikiPage).toHaveBeenCalledWith("desktop-app-workspace")
  })

  it("renders unresolvable wikilinks as inactive dimmed text", () => {
    render(
      <InlineMarkdownText
        text={"See [[missing-page]]."}
        pages={{}}
        onOpenWikiPage={vi.fn()}
      />,
    )

    const link = screen.getByTestId("wikilink-missing-page")
    expect(link.tagName).toBe("SPAN")
    expect(link).toHaveAttribute("data-state", "unresolvable")
    expect(link).toHaveAttribute("title", "Page not found")
  })

  it("does not produce block DOM for block markdown-like input", () => {
    const { container } = render(
      <div data-testid="inline-text">
        <InlineMarkdownText
          text={"# Heading\n\n```rust\nfn main() {}\n```\n\n| A | B |\n| - | - |"}
          pages={{}}
        />
      </div>,
    )

    expect(
      container.querySelector("pre, table, h1, h2, h3, ul, ol, blockquote"),
    ).toBeNull()
    expect(screen.getByTestId("inline-text")).toHaveTextContent("```rust")
  })
})
