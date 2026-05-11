import { describe, expect, it, vi } from "vitest"
import { render, screen } from "@testing-library/react"

import { BottomStrip } from "./BottomStrip"

describe("BottomStrip", () => {
  it("renders gear on the left and version on the right", () => {
    render(<BottomStrip version="v3.0.0" onOpenSettings={() => {}} />)
    expect(screen.getByTestId("settings-gear")).toBeInTheDocument()
    expect(screen.getByTestId("version-label")).toHaveTextContent("v3.0.0")
  })

  it("invokes onOpenSettings when the gear is clicked", () => {
    const onOpen = vi.fn()
    render(<BottomStrip version="v3.0.0" onOpenSettings={onOpen} />)
    screen.getByTestId("settings-gear").click()
    expect(onOpen).toHaveBeenCalledTimes(1)
  })
})
