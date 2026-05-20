import { describe, expect, it, beforeEach } from "vitest"

import { useVaultWatcherStatusStore } from "./vault-watcher-status"

describe("useVaultWatcherStatusStore", () => {
  beforeEach(() => {
    useVaultWatcherStatusStore.setState({ disabledVaults: {} })
  })

  it("reasonFor returns null for a healthy vault", () => {
    expect(useVaultWatcherStatusStore.getState().reasonFor("/v")).toBeNull()
  })

  it("markDisabled records the failure reason keyed by vault path", () => {
    const { markDisabled, reasonFor } = useVaultWatcherStatusStore.getState()
    markDisabled("/v", "ENOSPC: inotify watch limit exhausted")
    expect(reasonFor("/v")).toBe("ENOSPC: inotify watch limit exhausted")
    expect(reasonFor("/other")).toBeNull()
  })

  it("disabledVaults retain entries across distinct vault paths", () => {
    const { markDisabled } = useVaultWatcherStatusStore.getState()
    markDisabled("/v1", "reason 1")
    markDisabled("/v2", "reason 2")
    const map = useVaultWatcherStatusStore.getState().disabledVaults
    expect(map).toEqual({ "/v1": "reason 1", "/v2": "reason 2" })
  })
})
