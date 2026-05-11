import { describe, expect, it } from "vitest"
import { renderHook } from "@testing-library/react"

import { tStatic, useT } from "./useT"

describe("useT / tStatic", () => {
  it("returns English string by default", () => {
    const { result } = renderHook(() => useT("en"))
    expect(result.current("common.save")).toBe("Save")
  })

  it("returns Chinese string for zh locale override", () => {
    const { result } = renderHook(() => useT("zh"))
    expect(result.current("common.save")).toBe("儲存")
  })

  it("interpolates {var} placeholders", () => {
    const { result } = renderHook(() => useT("en"))
    expect(
      result.current("errors.vaultAlreadyExists", { path: "/tmp/x" }),
    ).toBe("This vault is already in your list: /tmp/x")
  })

  it("leaves unknown placeholders as-is", () => {
    const { result } = renderHook(() => useT("en"))
    expect(
      result.current("errors.invalid", { field: "x" }),
    ).toBe("x: {message}")
  })

  it("tStatic resolves without React context", () => {
    expect(tStatic("common.cancel")).toMatch(/Cancel|取消/)
  })
})
