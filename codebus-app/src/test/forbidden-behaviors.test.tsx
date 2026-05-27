import { describe, expect, it, vi, beforeEach } from "vitest"
import { render } from "@testing-library/react"

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}))
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
}))
vi.mock("@milkdown/core", () => ({
  Editor: { make: vi.fn(() => ({
    config: vi.fn(function (this: object) { return this }),
    use: vi.fn(function (this: object) { return this }),
    create: vi.fn(() => Promise.resolve({ destroy: vi.fn() })),
  })) },
  rootCtx: "rootCtx",
  defaultValueCtx: "defaultValueCtx",
  editorViewOptionsCtx: "editorViewOptionsCtx",
}))
vi.mock("@milkdown/preset-commonmark", () => ({ commonmark: () => ({}) }))

import { invoke } from "@tauri-apps/api/core"
import { Lobby } from "@/components/lobby/Lobby"
import { SettingsModal } from "@/components/settings/SettingsModal"
import { Workspace } from "@/components/workspace/Workspace"
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
  // The language switcher entry was lifted from v1 forbidden list by the
  // `settings-language-switcher` change — Settings now exposes a Language
  // dropdown by design.
  { label: "quest banner", regex: /quest\s*(banner|bar)?/i },
  { label: "Recent Pages panel", regex: /recent\s+pages/i },
  { label: "Graph view entry", regex: /graph\s*view/i },
  // Cmd+K UI: forbids the overlay / command-palette pattern (spec
  // `Forbidden Behaviors in v1`). The Workspace sidebar footer is
  // permitted to render a passive `⌘K` kbd chip that labels the
  // `useChatShortcut` toggle (per `app-shell` carve-out + `app-workspace`
  // § Workspace Sidebar Footer), so the regex matches only the
  // overlay/palette variants — not the chip text itself.
  { label: "Cmd+K palette UI", regex: /(cmd\+k|⌘\+k).*(palette|overlay|spotlight)|command\s*palette/i },
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

  it("workspace_has_no_forbidden_surfaces", () => {
    useRouteStore.setState({ route: { kind: "workspace", vault: VAULT } })
    const { container } = render(<Workspace vault={VAULT} />)
    assertNoForbidden("Workspace", container.textContent ?? "")
  })

  // Spec: app-workspace § One Active Goal Run At A Time — frontend
  // layer. When `activeRun` is non-null, the New Goal modal's Run
  // button SHALL be disabled (the backend rejection scenario is
  // covered by codebus-app-tauri's
  // `spawn_goal_rejects_when_another_run_is_active` Rust test).
  it("cannot_spawn_second_goal_run_while_active", async () => {
    const { NewGoalModal } = await import(
      "@/components/workspace/NewGoalModal"
    )
    const { useGoalsStore } = await import("@/store/goals")
    useGoalsStore.setState({
      activeRun: {
        runId: "r-active",
        goal: "first",
        startedAt: "2026-05-13T00:00:00Z",
        events: [],
        cancelling: false,
      },
      runs: [],
    })
    const { screen } = await import("@testing-library/react")
    render(
      <NewGoalModal open vaultPath="/v" initialText="second" onClose={() => {}} />,
    )
    const runBtn = screen.getByTestId("new-goal-run") as HTMLButtonElement
    expect(runBtn.disabled).toBe(true)
    useGoalsStore.setState({ runs: [], activeRun: null })
  })
})
