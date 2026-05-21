import { beforeEach, describe, expect, it, vi } from "vitest"

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}))

import { invoke } from "@tauri-apps/api/core"
import { readHooksConfig, useSettingsStore } from "./settings"

const mockedInvoke = vi.mocked(invoke)

describe("settings store", () => {
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

  it("marks dirty when a field is updated", () => {
    useSettingsStore.getState().update({ app: { quiz: { pass_threshold: 70 } } })
    expect(useSettingsStore.getState().dirty).toBe(true)
    expect(useSettingsStore.getState().config.app?.quiz?.pass_threshold).toBe(70)
  })

  it("clears dirty after a successful save", async () => {
    mockedInvoke.mockResolvedValueOnce(undefined)
    useSettingsStore.setState({
      config: { app: { quiz: { pass_threshold: 70, default_length: 5 } } },
      initialConfig: {},
      dirty: true,
    })
    await useSettingsStore.getState().save()
    expect(useSettingsStore.getState().dirty).toBe(false)
    expect(mockedInvoke).toHaveBeenCalledWith("save_global_config", {
      config: { app: { quiz: { pass_threshold: 70, default_length: 5 } } },
    })
  })

  it("resets to initial config", () => {
    useSettingsStore.setState({
      config: { app: { quiz: { pass_threshold: 70 } } },
      initialConfig: { app: { quiz: { pass_threshold: 80 } } },
      dirty: true,
    })
    useSettingsStore.getState().reset()
    expect(useSettingsStore.getState().config.app?.quiz?.pass_threshold).toBe(80)
    expect(useSettingsStore.getState().dirty).toBe(false)
  })

  it("getPassThreshold defaults to 80 when app.quiz.pass_threshold is absent", () => {
    useSettingsStore.setState({ config: {} })
    expect(useSettingsStore.getState().getPassThreshold()).toBe(80)
  })

  it("getPassThreshold reads app.quiz.pass_threshold verbatim when present", () => {
    useSettingsStore.setState({
      config: { app: { quiz: { pass_threshold: 90 } } },
    })
    expect(useSettingsStore.getState().getPassThreshold()).toBe(90)
    useSettingsStore.setState({
      config: { app: { quiz: { pass_threshold: 55 } } },
    })
    expect(useSettingsStore.getState().getPassThreshold()).toBe(55)
  })

  it("getDefaultLength prefers shared quiz.default_length over legacy app.quiz.default_length", () => {
    useSettingsStore.setState({
      config: {
        quiz: { default_length: 7 },
        app: { quiz: { default_length: 4 } },
      },
    })
    expect(useSettingsStore.getState().getDefaultLength()).toBe(7)
  })

  it("getDefaultLength falls back to legacy app.quiz.default_length when shared is absent", () => {
    useSettingsStore.setState({
      config: { app: { quiz: { default_length: 8 } } },
    })
    expect(useSettingsStore.getState().getDefaultLength()).toBe(8)
  })

  it("getDefaultLength defaults to 5 when no length configured", () => {
    useSettingsStore.setState({ config: {} })
    expect(useSettingsStore.getState().getDefaultLength()).toBe(5)
  })

  it("getDefaultLength clamps to the inclusive 3..10 range", () => {
    useSettingsStore.setState({ config: { quiz: { default_length: 2 } } })
    expect(useSettingsStore.getState().getDefaultLength()).toBe(3)
    useSettingsStore.setState({ config: { quiz: { default_length: 99 } } })
    expect(useSettingsStore.getState().getDefaultLength()).toBe(10)
    useSettingsStore.setState({ config: { quiz: { default_length: 10 } } })
    expect(useSettingsStore.getState().getDefaultLength()).toBe(10)
    useSettingsStore.setState({ config: { quiz: { default_length: 3 } } })
    expect(useSettingsStore.getState().getDefaultLength()).toBe(3)
  })

  it("keeps dirty=false on a failed save and surfaces error", async () => {
    mockedInvoke.mockRejectedValueOnce({ kind: "io", message: "fs" })
    useSettingsStore.setState({ config: { app: { quiz: { pass_threshold: 70, default_length: 5 } } }, dirty: true })
    await expect(useSettingsStore.getState().save()).rejects.toBeTruthy()
    expect(useSettingsStore.getState().error).toBeTruthy()
    // dirty remains true so user can retry.
    expect(useSettingsStore.getState().dirty).toBe(true)
  })

  // --- pretooluse-image-block-toggle task 4.2 ---
  // `readHooksConfig` must mirror the Rust HooksConfig contract:
  // absent / non-object input → default (block on); explicit boolean
  // wins; non-boolean → default (fail-safe to block).

  describe("readHooksConfig (hooks namespace)", () => {
    it("defaults to read_image_block=true when config has no hooks section", () => {
      expect(readHooksConfig({})).toEqual({ read_image_block: true })
    })

    it("defaults to read_image_block=true when config is null", () => {
      expect(readHooksConfig(null)).toEqual({ read_image_block: true })
    })

    it("defaults to read_image_block=true when hooks is not an object", () => {
      expect(readHooksConfig({ hooks: "yes" } as unknown as Parameters<typeof readHooksConfig>[0])).toEqual({
        read_image_block: true,
      })
    })

    it("reads an explicit true", () => {
      expect(
        readHooksConfig({
          hooks: { read_image_block: true },
        } as unknown as Parameters<typeof readHooksConfig>[0]),
      ).toEqual({ read_image_block: true })
    })

    it("reads an explicit false", () => {
      expect(
        readHooksConfig({
          hooks: { read_image_block: false },
        } as unknown as Parameters<typeof readHooksConfig>[0]),
      ).toEqual({ read_image_block: false })
    })

    it("defaults to true when read_image_block is non-boolean (fail-safe)", () => {
      expect(
        readHooksConfig({
          hooks: { read_image_block: "false" },
        } as unknown as Parameters<typeof readHooksConfig>[0]),
      ).toEqual({ read_image_block: true })
    })
  })
})
