import { afterEach, beforeEach, describe, expect, it } from "vitest"
import { renderHook, act } from "@testing-library/react"

import { useSettingsStore } from "@/store/settings"
import { useLocale } from "./useLocale"

const originalNavigator = globalThis.navigator

function setNavigatorLanguage(lang: string | undefined): void {
  if (lang === undefined) {
    // Simulate non-browser env: remove navigator entirely.
    Object.defineProperty(globalThis, "navigator", {
      configurable: true,
      value: undefined,
    })
    return
  }
  Object.defineProperty(globalThis, "navigator", {
    configurable: true,
    value: { ...originalNavigator, language: lang } as Navigator,
  })
}

/**
 * Precedence resolution for `useLocale`, mirroring spec ADDED requirement
 * *Settings Language Override* Example table:
 *
 *   hook arg > store locale_override > navigator.language
 */
describe("useLocale · precedence", () => {
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

  afterEach(() => {
    Object.defineProperty(globalThis, "navigator", {
      configurable: true,
      value: originalNavigator,
    })
  })

  // Example table row 1: hook arg "zh" beats store "en" + navigator "en-US"
  it("hook arg outranks store and navigator", () => {
    useSettingsStore.setState({
      config: { app: { locale_override: "en" } } as never,
    })
    setNavigatorLanguage("en-US")
    const { result } = renderHook(() => useLocale("zh"))
    expect(result.current).toBe("zh")
  })

  // Row 2: store "en" beats navigator "zh-TW" when hook arg absent
  it("store override outranks navigator.language", () => {
    useSettingsStore.setState({
      config: { app: { locale_override: "en" } } as never,
    })
    setNavigatorLanguage("zh-TW")
    const { result } = renderHook(() => useLocale())
    expect(result.current).toBe("en")
  })

  // Row 3: store null + navigator zh-TW → "zh"
  it("store null falls through to navigator zh-TW → \"zh\"", () => {
    useSettingsStore.setState({
      config: { app: { locale_override: null } } as never,
    })
    setNavigatorLanguage("zh-TW")
    const { result } = renderHook(() => useLocale())
    expect(result.current).toBe("zh")
  })

  // Row 4: store null + navigator en-US → "en"
  it("store null falls through to navigator en-US → \"en\"", () => {
    useSettingsStore.setState({
      config: { app: { locale_override: null } } as never,
    })
    setNavigatorLanguage("en-US")
    const { result } = renderHook(() => useLocale())
    expect(result.current).toBe("en")
  })

  // Row 5: store null + navigator fr-FR → "en"
  it("non-zh navigator language defaults to \"en\"", () => {
    useSettingsStore.setState({
      config: { app: { locale_override: null } } as never,
    })
    setNavigatorLanguage("fr-FR")
    const { result } = renderHook(() => useLocale())
    expect(result.current).toBe("en")
  })

  // Row 6: store null + navigator undefined → "en"
  it("missing navigator defaults to \"en\"", () => {
    useSettingsStore.setState({
      config: { app: { locale_override: null } } as never,
    })
    setNavigatorLanguage(undefined)
    const { result } = renderHook(() => useLocale())
    expect(result.current).toBe("en")
  })

  // Reactive subscription: store mutation triggers re-render
  it("re-renders when store locale_override changes", () => {
    useSettingsStore.setState({
      config: { app: { locale_override: null } } as never,
    })
    setNavigatorLanguage("zh-TW")
    const { result } = renderHook(() => useLocale())
    expect(result.current).toBe("zh")
    act(() => {
      useSettingsStore
        .getState()
        .update({ app: { locale_override: "en" } })
    })
    expect(result.current).toBe("en")
  })

  // Absent app section behaves the same as locale_override: null
  it("treats absent app.locale_override as auto", () => {
    useSettingsStore.setState({ config: {} as never })
    setNavigatorLanguage("zh-TW")
    const { result } = renderHook(() => useLocale())
    expect(result.current).toBe("zh")
  })

  // Hook arg null/undefined does NOT override store
  it("hook arg undefined yields to store override", () => {
    useSettingsStore.setState({
      config: { app: { locale_override: "zh" } } as never,
    })
    setNavigatorLanguage("en-US")
    const { result } = renderHook(() => useLocale(undefined))
    expect(result.current).toBe("zh")
  })
})
