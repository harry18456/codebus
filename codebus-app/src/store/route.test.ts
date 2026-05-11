import { beforeEach, describe, expect, it } from "vitest"

import { useRouteStore } from "./route"

function makeEntry(path: string) {
  return {
    path,
    display_name: path,
    last_opened: "2026-05-11T00:00:00Z",
    is_missing: false,
  }
}

describe("route store", () => {
  beforeEach(() => {
    useRouteStore.setState({ route: { kind: "lobby" } })
  })

  it("transitions to workspace-stub on open with vault context", () => {
    const vault = makeEntry("/v")
    useRouteStore.getState().open(vault)
    const route = useRouteStore.getState().route
    expect(route.kind).toBe("workspace-stub")
    if (route.kind === "workspace-stub") {
      expect(route.vault.path).toBe("/v")
    }
  })

  it("returns to lobby on back", () => {
    useRouteStore.getState().open(makeEntry("/v"))
    useRouteStore.getState().back()
    expect(useRouteStore.getState().route.kind).toBe("lobby")
  })

  it("starts at the lobby state", () => {
    expect(useRouteStore.getState().route.kind).toBe("lobby")
  })
})
