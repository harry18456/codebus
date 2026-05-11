import { describe, expect, it, vi, beforeEach } from "vitest"
import { render } from "@testing-library/react"

import { useNewVaultShortcut } from "./useNewVaultShortcut"
import { useRouteStore } from "@/store/route"

function Harness({ onFire }: { onFire: () => void }) {
  useNewVaultShortcut(onFire)
  return null
}

function fireCmdN() {
  window.dispatchEvent(new KeyboardEvent("keydown", { key: "n", metaKey: true }))
}

function fireCtrlN() {
  window.dispatchEvent(new KeyboardEvent("keydown", { key: "n", ctrlKey: true }))
}

describe("useNewVaultShortcut", () => {
  beforeEach(() => {
    useRouteStore.setState({ route: { kind: "lobby" } })
  })

  it("fires Cmd+N when route is lobby", () => {
    const onFire = vi.fn()
    render(<Harness onFire={onFire} />)
    fireCmdN()
    expect(onFire).toHaveBeenCalledTimes(1)
  })

  it("fires Ctrl+N when route is lobby", () => {
    const onFire = vi.fn()
    render(<Harness onFire={onFire} />)
    fireCtrlN()
    expect(onFire).toHaveBeenCalledTimes(1)
  })

  it("does not fire when route is workspace-stub", () => {
    const onFire = vi.fn()
    render(<Harness onFire={onFire} />)
    useRouteStore.setState({
      route: {
        kind: "workspace-stub",
        vault: {
          path: "/v",
          display_name: "v",
          last_opened: "2026-05-11T00:00:00Z",
          is_missing: false,
        },
      },
    })
    fireCmdN()
    expect(onFire).not.toHaveBeenCalled()
  })

  it("does not fire on plain N without modifier", () => {
    const onFire = vi.fn()
    render(<Harness onFire={onFire} />)
    window.dispatchEvent(new KeyboardEvent("keydown", { key: "n" }))
    expect(onFire).not.toHaveBeenCalled()
  })
})
