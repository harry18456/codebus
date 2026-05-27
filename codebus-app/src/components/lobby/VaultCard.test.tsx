import { describe, expect, it, vi } from "vitest"
import { fireEvent, render, screen } from "@testing-library/react"

import { VaultCard } from "./VaultCard"
import type { VaultEntry } from "@/lib/ipc"

function entry(path = "/alpha", isMissing = false): VaultEntry {
  return {
    path,
    display_name: path.split("/").pop() ?? path,
    last_opened: "2026-05-11T00:00:00Z",
    is_missing: isMissing,
  }
}

describe("VaultCard kebab affordance", () => {
  it("renders a kebab button anchored at the card's right edge", () => {
    render(
      <VaultCard
        vault={entry("/alpha")}
        onOpen={() => {}}
        onRemove={() => {}}
        onRevealInFiles={() => {}}
      />,
    )
    const kebab = screen.getByTestId("vault-card-kebab-/alpha")
    expect(kebab).toBeInTheDocument()
    // The kebab MUST start invisible so it does not add static visual noise;
    // hover and keyboard focus reveal it via group-hover / focus-visible.
    expect(kebab.className).toContain("opacity-0")
    expect(kebab.className).toContain("group-hover:opacity-100")
    expect(kebab.className).toContain("focus-visible:opacity-100")
  })

  it("opens the action menu when the kebab is clicked", () => {
    render(
      <VaultCard
        vault={entry("/alpha")}
        onOpen={() => {}}
        onRemove={() => {}}
        onRevealInFiles={() => {}}
      />,
    )
    expect(screen.queryByRole("menu")).toBeNull()
    fireEvent.click(screen.getByTestId("vault-card-kebab-/alpha"))
    expect(screen.getByRole("menu")).toBeInTheDocument()
    // Both menu items are present and i18n-routed.
    const revealText = screen
      .getAllByRole("menuitem")
      .map((el) => el.textContent ?? "")
    expect(revealText.length).toBe(2)
  })

  it("kebab click does NOT trigger the card's onOpen handler", () => {
    const onOpen = vi.fn()
    render(
      <VaultCard
        vault={entry("/alpha")}
        onOpen={onOpen}
        onRemove={() => {}}
        onRevealInFiles={() => {}}
      />,
    )
    fireEvent.click(screen.getByTestId("vault-card-kebab-/alpha"))
    expect(onOpen).not.toHaveBeenCalled()
  })

  it("right-click on the card still opens the action menu (shortcut path)", () => {
    render(
      <VaultCard
        vault={entry("/alpha")}
        onOpen={() => {}}
        onRemove={() => {}}
        onRevealInFiles={() => {}}
      />,
    )
    expect(screen.queryByRole("menu")).toBeNull()
    fireEvent.contextMenu(screen.getByTestId("vault-card-/alpha"))
    expect(screen.getByRole("menu")).toBeInTheDocument()
  })

  it("menu items wire to onRevealInFiles and onRemove", () => {
    const onReveal = vi.fn()
    const onRemove = vi.fn()
    render(
      <VaultCard
        vault={entry("/alpha")}
        onOpen={() => {}}
        onRemove={onRemove}
        onRevealInFiles={onReveal}
      />,
    )
    fireEvent.click(screen.getByTestId("vault-card-kebab-/alpha"))
    const items = screen.getAllByRole("menuitem")
    items[0]?.click()
    expect(onReveal).toHaveBeenCalledWith(expect.objectContaining({ path: "/alpha" }))

    fireEvent.click(screen.getByTestId("vault-card-kebab-/alpha"))
    const itemsAgain = screen.getAllByRole("menuitem")
    itemsAgain[1]?.click()
    expect(onRemove).toHaveBeenCalledWith(expect.objectContaining({ path: "/alpha" }))
  })
})
