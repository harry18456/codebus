import { beforeEach, describe, expect, it } from "vitest"
import { render, act } from "@testing-library/react"

import { useSettingsStore } from "@/store/settings"
import { useT } from "./useT"
import { toLocalizedError } from "./errors"

/**
 * `LocalizedError` carries a message key + vars and is rendered through
 * `useT` at display time. Per design decision "LocalizedError 路徑 — 不需改",
 * switching the store override SHALL flip the rendered text without any
 * code change in `errors.ts`. This test exercises the chain end-to-end.
 */
function ErrorToast({ err }: { err: { key: string; vars?: Record<string, string | number> } }) {
  const t = useT()
  // The runtime type is `MessageKey`; for this integration test we accept
  // string and trust the bundle to have the key.
  return <span data-testid="msg">{t(err.key as never, err.vars)}</span>
}

describe("LocalizedError honors store locale_override at render time", () => {
  beforeEach(() => {
    useSettingsStore.setState({
      config: {},
      initialConfig: {},
      dirty: false,
      loading: false,
      saving: false,
      error: null,
    })
  })

  it("renders zh then flips to en when store override changes", () => {
    useSettingsStore.setState({
      config: { app: { locale_override: "zh" } } as never,
    })
    const err = toLocalizedError({
      kind: "vault_not_found",
      path: "/tmp/missing",
    })
    const { getByTestId } = render(<ErrorToast err={err} />)
    const zhText = getByTestId("msg").textContent ?? ""
    expect(zhText).toContain("/tmp/missing")
    // zh translation of errors.vaultNotFound carries Chinese characters.
    expect(zhText).toMatch(/[一-鿿]/)

    act(() => {
      useSettingsStore
        .getState()
        .update({ app: { locale_override: "en" } })
    })
    const enText = getByTestId("msg").textContent ?? ""
    expect(enText).toContain("/tmp/missing")
    expect(enText).not.toMatch(/[一-鿿]/)
  })

  it("does not require any imperative locale lookup in errors.ts (toLocalizedError is shape-only)", () => {
    const err1 = toLocalizedError({ kind: "internal", message: "boom" })
    const err2 = toLocalizedError({ kind: "internal", message: "boom" })
    expect(err1).toEqual(err2)
    expect(err1.key).toBe("errors.internal")
    expect(err1.vars).toEqual({ message: "boom" })
  })
})
