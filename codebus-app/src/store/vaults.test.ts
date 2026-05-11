import { beforeEach, describe, expect, it, vi } from "vitest"

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}))

import { invoke } from "@tauri-apps/api/core"
import { useVaultsStore } from "./vaults"

const mockedInvoke = vi.mocked(invoke)

function makeEntry(path: string) {
  return {
    path,
    display_name: path,
    last_opened: "2026-05-11T00:00:00Z",
    is_missing: false,
  }
}

describe("vaults store", () => {
  beforeEach(() => {
    useVaultsStore.setState({ vaults: [], loading: false, error: null })
    mockedInvoke.mockReset()
  })

  it("loads vaults from IPC", async () => {
    mockedInvoke.mockResolvedValueOnce([makeEntry("/a"), makeEntry("/b")])
    await useVaultsStore.getState().loadVaults()
    expect(useVaultsStore.getState().vaults).toHaveLength(2)
    expect(useVaultsStore.getState().loading).toBe(false)
  })

  it("appends a new vault on add", async () => {
    mockedInvoke.mockResolvedValueOnce(makeEntry("/new"))
    await useVaultsStore.getState().addVault("/new")
    expect(useVaultsStore.getState().vaults).toHaveLength(1)
    expect(useVaultsStore.getState().vaults[0].path).toBe("/new")
  })

  it("decrements the list on remove", async () => {
    useVaultsStore.setState({ vaults: [makeEntry("/a"), makeEntry("/b")] })
    mockedInvoke.mockResolvedValueOnce(undefined)
    await useVaultsStore.getState().removeVault("/a")
    expect(useVaultsStore.getState().vaults).toHaveLength(1)
    expect(useVaultsStore.getState().vaults[0].path).toBe("/b")
  })

  it("rolls back removal on IPC failure", async () => {
    useVaultsStore.setState({ vaults: [makeEntry("/a"), makeEntry("/b")] })
    mockedInvoke.mockRejectedValueOnce({ kind: "io", message: "fs" })
    await expect(
      useVaultsStore.getState().removeVault("/a"),
    ).rejects.toBeTruthy()
    expect(useVaultsStore.getState().vaults).toHaveLength(2)
    expect(useVaultsStore.getState().error).toBeTruthy()
  })

  it("marks an entry as missing without IPC", () => {
    useVaultsStore.setState({ vaults: [makeEntry("/a")] })
    useVaultsStore.getState().markMissing("/a")
    expect(useVaultsStore.getState().vaults[0].is_missing).toBe(true)
  })
})
