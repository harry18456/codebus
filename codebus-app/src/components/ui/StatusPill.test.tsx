import { afterEach, beforeEach, describe, expect, it, vi } from "vitest"
import { render, screen } from "@testing-library/react"

import { StatusPill } from "./StatusPill"

describe("StatusPill", () => {
  describe("dot variant", () => {
    it("renders a 7px dot with bg-status-done utility for done status", () => {
      const { container } = render(<StatusPill status="done" variant="dot" />)
      const root = container.firstElementChild
      expect(root).not.toBeNull()
      expect(root?.classList.contains("bg-status-done")).toBe(true)
      // No descendant text content for dot variant.
      expect(root?.textContent).toBe("")
    })

    it("renders bg-status-interrupted for interrupted status", () => {
      const { container } = render(
        <StatusPill status="interrupted" variant="dot" />,
      )
      const root = container.firstElementChild
      expect(root?.classList.contains("bg-status-interrupted")).toBe(true)
    })

    it("renders bg-status-failed for failed status", () => {
      const { container } = render(<StatusPill status="failed" variant="dot" />)
      const root = container.firstElementChild
      expect(root?.classList.contains("bg-status-failed")).toBe(true)
    })
  })

  describe("pill variant", () => {
    it("renders dot plus localized label for done status (en locale default)", () => {
      const { container } = render(<StatusPill status="done" variant="pill" />)
      expect(screen.getByText("Done")).toBeInTheDocument()
      const dot = container.querySelector(".status-pill__dot")
      expect(dot).not.toBeNull()
      expect(dot?.classList.contains("bg-status-done")).toBe(true)
    })

    it("renders dot plus localized label for interrupted status", () => {
      const { container } = render(
        <StatusPill status="interrupted" variant="pill" />,
      )
      expect(screen.getByText("Interrupted")).toBeInTheDocument()
      const dot = container.querySelector(".status-pill__dot")
      expect(dot?.classList.contains("bg-status-interrupted")).toBe(true)
    })

    it("renders dot plus localized label for failed status", () => {
      const { container } = render(
        <StatusPill status="failed" variant="pill" />,
      )
      expect(screen.getByText("Failed")).toBeInTheDocument()
      const dot = container.querySelector(".status-pill__dot")
      expect(dot?.classList.contains("bg-status-failed")).toBe(true)
    })

    it("renders running pill with both ring and animated classes plus label and optional caret", () => {
      const { container } = render(
        <StatusPill
          status="running"
          variant="pill"
          caret={<span className="font-mono">analyzing…|</span>}
        />,
      )
      expect(screen.getByText("Running")).toBeInTheDocument()
      const dot = container.querySelector(".status-pill__dot")
      expect(dot).not.toBeNull()
      expect(dot?.classList.contains("bg-status-running")).toBe(true)
      // Both classes always present; CSS @media gates the animation rule.
      expect(dot?.classList.contains("status-pill__dot--running-ring")).toBe(
        true,
      )
      expect(
        dot?.classList.contains("status-pill__dot--running-animated"),
      ).toBe(true)
      // Caret rendered.
      expect(screen.getByText("analyzing…|")).toBeInTheDocument()
    })

    it("omits caret slot when caret prop is not provided", () => {
      const { container } = render(
        <StatusPill status="running" variant="pill" />,
      )
      const caretNodes = container.querySelectorAll(".status-pill__caret")
      expect(caretNodes.length).toBe(0)
    })

    it("renders running pill without caret content but keeps ring classes", () => {
      const { container } = render(
        <StatusPill status="running" variant="pill" />,
      )
      const dot = container.querySelector(".status-pill__dot")
      expect(dot?.classList.contains("status-pill__dot--running-ring")).toBe(
        true,
      )
      expect(
        dot?.classList.contains("status-pill__dot--running-animated"),
      ).toBe(true)
    })
  })

  describe("dot + running invariant", () => {
    let warnSpy: ReturnType<typeof vi.spyOn>

    beforeEach(() => {
      // Vitest runs with import.meta.env.PROD === false (MODE = "test"),
      // matching the dev-build gate the component checks.
      warnSpy = vi.spyOn(console, "warn").mockImplementation(() => {})
    })

    afterEach(() => {
      warnSpy.mockRestore()
    })

    it("warns in development when dot + running combination is used and does not throw", () => {
      expect(() =>
        render(<StatusPill status="running" variant="dot" />),
      ).not.toThrow()
      expect(warnSpy).toHaveBeenCalled()
      const warnArgs = warnSpy.mock.calls.flat().join(" ")
      expect(warnArgs.toLowerCase()).toContain("statuspill")
    })

    it("does not render pulse ring classes when dot + running invariant is hit", () => {
      const { container } = render(
        <StatusPill status="running" variant="dot" />,
      )
      const root = container.firstElementChild
      expect(
        root?.classList.contains("status-pill__dot--running-ring"),
      ).toBe(false)
      expect(
        root?.classList.contains("status-pill__dot--running-animated"),
      ).toBe(false)
    })
  })

  describe("className merge", () => {
    it("merges caller className with internal status-pill classes for pill variant", () => {
      const { container } = render(
        <StatusPill status="done" variant="pill" className="ml-2" />,
      )
      const root = container.firstElementChild
      expect(root?.classList.contains("status-pill")).toBe(true)
      expect(root?.classList.contains("ml-2")).toBe(true)
    })

    it("merges caller className with internal classes for dot variant", () => {
      const { container } = render(
        <StatusPill status="done" variant="dot" className="opacity-50" />,
      )
      const root = container.firstElementChild
      expect(root?.classList.contains("opacity-50")).toBe(true)
    })
  })
})
