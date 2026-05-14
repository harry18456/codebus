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

  it("transitions to workspace on open with vault context", () => {
    const vault = makeEntry("/v")
    useRouteStore.getState().open(vault)
    const route = useRouteStore.getState().route
    expect(route.kind).toBe("workspace")
    if (route.kind === "workspace") {
      expect(route.vault.path).toBe("/v")
    }
  })

  // Spec: app-shell § Workspace Stub Transition (modified) — the
  // open(vault) path transitions to the real Workspace state, not the
  // historical stub kind. This test pins the discriminator string so a
  // future rename surfaces as a compile + test failure rather than
  // silently breaking the App.tsx routing branch.
  it("route_open_transitions_to_workspace", () => {
    const vault = makeEntry("/v2")
    useRouteStore.getState().open(vault)
    expect(useRouteStore.getState().route.kind).toBe("workspace")
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
