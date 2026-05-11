import { describe, expect, it, vi, beforeEach } from "vitest"
import { render } from "@testing-library/react"

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}))

import { invoke } from "@tauri-apps/api/core"
import { Lobby } from "@/components/lobby/Lobby"
import { SettingsModal } from "@/components/settings/SettingsModal"
import { WorkspaceStub } from "@/components/workspace/WorkspaceStub"
import { useVaultsStore } from "@/store/vaults"
import { useRouteStore } from "@/store/route"
import { useSettingsStore } from "@/store/settings"

const mockedInvoke = vi.mocked(invoke)

const VAULT = {
  path: "/path",
  display_name: "repo",
  last_opened: "2026-05-11T00:00:00Z",
  is_missing: false,
}

/**
 * Spec `Forbidden Behaviors in v1`: each phrase below MUST NOT appear in
 * the three root component renderings. The exact list comes from the
 * proposal §"What Changes" non-goals and the design Non-Goals.
 */
const FORBIDDEN_PHRASES: Array<{ label: string; regex: RegExp }> = [
  { label: "theme toggle", regex: /theme\s*(toggle|switch)/i },
  { label: "light mode", regex: /light\s*mode/i },
  { label: "language switcher", regex: /language\s*(switch|picker|selector)/i },
  { label: "quest banner", regex: /quest\s*(banner|bar)?/i },
  { label: "Recent Pages panel", regex: /recent\s+pages/i },
  { label: "Graph view entry", regex: /graph\s*view/i },
  { label: "Cmd\\+K UI", regex: /(⌘k|cmd\+k|command\s*palette)/i },
  { label: "Tutorial slideshow", regex: /tutorial\s*(slideshow|carousel)/i },
  { label: "learned/mastered/graduated", regex: /(learned|mastered|graduated)/i },
]

function assertNoForbidden(label: string, text: string) {
  for (const { label: phraseLabel, regex } of FORBIDDEN_PHRASES) {
    expect(regex.test(text), `${label}: forbidden phrase \"${phraseLabel}\" matched in render`).toBe(
      false,
    )
  }
}

describe("Forbidden Behaviors in v1", () => {
  beforeEach(() => {
    mockedInvoke.mockReset()
    mockedInvoke.mockResolvedValue([])
    useVaultsStore.setState({ vaults: [], loading: false, error: null })
    useRouteStore.setState({ route: { kind: "lobby" } })
    useSettingsStore.setState({
      config: { app: { quiz: { pass_threshold: 80, default_length: 5 } } },
      initialConfig: {
        app: { quiz: { pass_threshold: 80, default_length: 5 } },
      },
      dirty: false,
      loading: false,
      saving: false,
      error: null,
    })
  })

  it("Lobby empty state has no forbidden phrases", () => {
    const { container } = render(
      <Lobby onNewVault={() => {}} onRevealInFiles={() => {}} />,
    )
    assertNoForbidden("Lobby/empty", container.textContent ?? "")
  })

  it("Lobby populated state has no forbidden phrases", () => {
    useVaultsStore.setState({ vaults: [VAULT] })
    const { container } = render(
      <Lobby onNewVault={() => {}} onRevealInFiles={() => {}} />,
    )
    assertNoForbidden("Lobby/populated", container.textContent ?? "")
  })

  it("Settings modal has no forbidden phrases", () => {
    const { container } = render(
      <SettingsModal open onClose={() => {}} piiPatternCount={14} />,
    )
    assertNoForbidden("Settings modal", container.textContent ?? "")
  })

  it("Workspace stub has no forbidden phrases", () => {
    useRouteStore.setState({ route: { kind: "workspace-stub", vault: VAULT } })
    const { container } = render(<WorkspaceStub vault={VAULT} />)
    assertNoForbidden("Workspace stub", container.textContent ?? "")
  })
})
