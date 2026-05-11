import "@testing-library/jest-dom/vitest"

// jsdom polyfills for Radix UI primitives that depend on browser APIs.
if (typeof globalThis.ResizeObserver === "undefined") {
  class ResizeObserverStub {
    observe() {}
    unobserve() {}
    disconnect() {}
  }
  Object.defineProperty(globalThis, "ResizeObserver", {
    writable: true,
    configurable: true,
    value: ResizeObserverStub,
  })
}

if (typeof window !== "undefined" && !window.HTMLElement.prototype.hasPointerCapture) {
  // Radix Select calls hasPointerCapture during open/close.
  window.HTMLElement.prototype.hasPointerCapture = () => false
  window.HTMLElement.prototype.releasePointerCapture = () => {}
  ;(window.HTMLElement.prototype as { scrollIntoView?: () => void }).scrollIntoView = () => {}
}

