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

  it("renders a backtick-wrapped lone citation `[[slug]]` as a clickable wikilink, not literal code", () => {
    const onOpenWikiPage = vi.fn()
    render(
      <InlineMarkdownText
        text={"See `[[vault-model]]` for the vault layout."}
        pages={{
          "vault-model": {
            slug: "vault-model",
            path: "wiki/concepts/vault-model.md",
            title: "Vault Model",
          },
        }}
        onOpenWikiPage={onOpenWikiPage}
      />,
    )

    const link = screen.getByTestId("wikilink-vault-model")
    expect(link.tagName).toBe("A")
    expect(link).toHaveClass("cite-link")
    expect(link).toHaveTextContent("Vault Model")
    expect(link.closest("code")).toBeNull()
    fireEvent.click(link)
    expect(onOpenWikiPage).toHaveBeenCalledWith("vault-model")
  })

  it("unwraps a backtick-wrapped citation even when unresolvable (dimmed span, not code)", () => {
    render(<InlineMarkdownText text={"`[[missing-page]]`"} pages={{}} />)

    const link = screen.getByTestId("wikilink-missing-page")
    expect(link.tagName).toBe("SPAN")
    expect(link).toHaveAttribute("data-state", "unresolvable")
    expect(link.closest("code")).toBeNull()
  })

  it("leaves genuine inline code untouched and does not unwrap a wikilink mixed with other text", () => {
    render(
      <InlineMarkdownText
        text={"Call `foo()`; `see [[x]]` stays code."}
        pages={{ x: { slug: "x", path: "wiki/x.md", title: "X" } }}
      />,
    )

    expect(screen.getByText("foo()").tagName).toBe("CODE")
    expect(screen.queryByTestId("wikilink-x")).toBeNull()
  })
})
