import { beforeEach, describe, expect, it, vi } from "vitest"

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}))

import { invoke } from "@tauri-apps/api/core"
import { useSettingsStore } from "./settings"

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

  it("keeps dirty=false on a failed save and surfaces error", async () => {
    mockedInvoke.mockRejectedValueOnce({ kind: "io", message: "fs" })
    useSettingsStore.setState({ config: { app: { quiz: { pass_threshold: 70, default_length: 5 } } }, dirty: true })
    await expect(useSettingsStore.getState().save()).rejects.toBeTruthy()
    expect(useSettingsStore.getState().error).toBeTruthy()
    // dirty remains true so user can retry.
    expect(useSettingsStore.getState().dirty).toBe(true)
  })
})
