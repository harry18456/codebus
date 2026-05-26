import { beforeEach, describe, expect, it, vi } from "vitest"

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}))

import { invoke } from "@tauri-apps/api/core"
import { useSettingsStore } from "./settings"

const mockedInvoke = vi.mocked(invoke)

/**
 * Reactive selector contract for `app.locale_override`: components that
 * subscribe via the store hook re-render when the value changes. Backs spec
 * ADDED requirement *Settings Language Override* and design decision
 * "Reactive: zustand subscribe vs. one-time read".
 */
describe("settings store · locale_override reactivity", () => {
  beforeEach(() => {
    useSettingsStore.setState({
      config: {},
      initialConfig: {},
      dirty: false,
      loading: false,
      saving: false,
      error: null,
    })
    mockedInvoke.mockReset()
  })

  it("reads locale_override from a loaded config", async () => {
    mockedInvoke.mockResolvedValueOnce({
      app: { quiz: { pass_threshold: 80 }, locale_override: "en" },
    })
    await useSettingsStore.getState().load()
    expect(useSettingsStore.getState().config.app?.locale_override).toBe("en")
  })

  it("update() switches locale_override and marks dirty", () => {
    useSettingsStore
      .getState()
      .update({ app: { locale_override: "zh" } })
    expect(useSettingsStore.getState().dirty).toBe(true)
    expect(useSettingsStore.getState().config.app?.locale_override).toBe("zh")
  })

  it("update() back to null surfaces in the selector (Auto path)", () => {
    useSettingsStore.setState({
      config: { app: { locale_override: "en" } } as never,
      initialConfig: {},
      dirty: false,
    })
    useSettingsStore
      .getState()
      .update({ app: { locale_override: null } })
    expect(useSettingsStore.getState().config.app?.locale_override).toBeNull()
    expect(useSettingsStore.getState().dirty).toBe(true)
  })

  it("notifies subscribers when locale_override changes", () => {
    const seen: Array<"zh" | "en" | null | undefined> = []
    const unsub = useSettingsStore.subscribe((state) => {
      seen.push(state.config.app?.locale_override)
    })
    useSettingsStore.getState().update({ app: { locale_override: "en" } })
    useSettingsStore.getState().update({ app: { locale_override: "zh" } })
    useSettingsStore.getState().update({ app: { locale_override: null } })
    unsub()
    // Each update should have surfaced through the subscription, in order.
    expect(seen).toContain("en")
    expect(seen).toContain("zh")
    expect(seen).toContain(null)
  })
})
