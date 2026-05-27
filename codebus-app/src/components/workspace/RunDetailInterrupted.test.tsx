import { fireEvent, render, screen } from "@testing-library/react"
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest"

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
}))
vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }))

import { invoke } from "@tauri-apps/api/core"
import { messages } from "@/i18n/messages"
import type { InterruptReason, RunDetail } from "@/lib/ipc"
import { useGoalsStore } from "@/store/goals"

import { RunDetailInterrupted } from "./RunDetailInterrupted"

const invokeMock = vi.mocked(invoke)
const en = messages.en

function makeDetail(
  over: Partial<RunDetail["summary"]> = {},
): RunDetail {
  return {
    summary: {
      run_id: "r1",
      mode: "goal",
      goal: "describe auth flow",
      started_at: "2026-05-27T10:00:00Z",
      finished_at: "2026-05-27T10:01:00Z",
      tokens: { input_tokens: 0, output_tokens: 0 },
      wiki_changed: false,
      lint_error_count: 0,
      lint_warn_count: 0,
      outcome: "cancelled",
      ...over,
    },
    events: [],
  }
}

describe("RunDetailInterrupted · banner state machine", () => {
  beforeEach(() => {
    invokeMock.mockReset()
    useGoalsStore.setState({ runs: [], activeRun: null })
  })

  afterEach(() => {
    useGoalsStore.setState({ runs: [], activeRun: null })
  })

  it("RunDetailInterrupted_cancelled_without_reason_renders_amber_fallback_subtitle", () => {
    render(
      <RunDetailInterrupted
        detail={makeDetail({ outcome: "cancelled" })}
        vaultPath="/v"
        onBack={() => {}}
      />,
    )
    expect(screen.getByTestId("interrupted-badge-amber")).toBeTruthy()
    expect(screen.getByTestId("interrupted-banner-fallback").textContent).toContain(
      en["workspace.runDetail.banner.interruptedSubtitle"],
    )
  })

  it("RunDetailInterrupted_cancelled_with_user_cancel_renders_amber_userCancel_subtitle", () => {
    render(
      <RunDetailInterrupted
        detail={makeDetail({
          outcome: "cancelled",
          interrupt_reason: "user-cancel" as InterruptReason,
        })}
        vaultPath="/v"
        onBack={() => {}}
      />,
    )
    expect(screen.getByTestId("interrupted-badge-amber")).toBeTruthy()
    expect(screen.getByTestId("interrupted-banner-userCancel").textContent).toContain(
      en["workspace.runDetail.banner.reason.userCancel"],
    )
  })

  it("RunDetailInterrupted_failed_renders_red_tier_with_failed_subtitle_and_ignores_reason", () => {
    render(
      <RunDetailInterrupted
        detail={makeDetail({
          outcome: "failed",
          // reason MUST be ignored on red tier — included here to prove
          // the failed banner does NOT read it.
          interrupt_reason: "user-cancel" as InterruptReason,
        })}
        vaultPath="/v"
        onBack={() => {}}
      />,
    )
    expect(screen.getByTestId("interrupted-badge-red")).toBeTruthy()
    const banner = screen.getByTestId("interrupted-banner-fallback")
    expect(banner.textContent).toContain(en["workspace.runDetail.banner.failedTitle"])
    expect(banner.textContent).toContain(en["workspace.runDetail.banner.failedSubtitle"])
    // Subtitle from the ignored reason MUST NOT appear in the banner.
    expect(banner.textContent).not.toContain(
      en["workspace.runDetail.banner.reason.userCancel"],
    )
  })

  it("RunDetailInterrupted_interrupted_with_app_close_renders_amber_appClose_subtitle", () => {
    render(
      <RunDetailInterrupted
        detail={makeDetail({
          outcome: "interrupted",
          interrupt_reason: "app-close" as InterruptReason,
        })}
        vaultPath="/v"
        onBack={() => {}}
      />,
    )
    expect(screen.getByTestId("interrupted-badge-amber")).toBeTruthy()
    expect(screen.getByTestId("interrupted-banner-appClose").textContent).toContain(
      en["workspace.runDetail.banner.reason.appClose"],
    )
  })

  it("RunDetailInterrupted_interrupted_with_network_drop_renders_amber_networkDrop_subtitle", () => {
    render(
      <RunDetailInterrupted
        detail={makeDetail({
          outcome: "interrupted",
          interrupt_reason: "network-drop" as InterruptReason,
        })}
        vaultPath="/v"
        onBack={() => {}}
      />,
    )
    expect(screen.getByTestId("interrupted-badge-amber")).toBeTruthy()
    expect(screen.getByTestId("interrupted-banner-networkDrop").textContent).toContain(
      en["workspace.runDetail.banner.reason.networkDrop"],
    )
  })

  it("RunDetailInterrupted_interrupted_with_other_reason_renders_amber_other_subtitle_and_hides_raw_string", () => {
    const { container } = render(
      <RunDetailInterrupted
        detail={makeDetail({
          outcome: "interrupted",
          interrupt_reason: { other: "agent-crash" } as InterruptReason,
        })}
        vaultPath="/v"
        onBack={() => {}}
      />,
    )
    expect(screen.getByTestId("interrupted-badge-amber")).toBeTruthy()
    expect(screen.getByTestId("interrupted-banner-other").textContent).toContain(
      en["workspace.runDetail.banner.reason.other"],
    )
    // Spec requirement: raw inner string from the { other: string } variant
    // SHALL NOT be rendered into the UI text (schema-internal token).
    expect(container.textContent ?? "").not.toContain("agent-crash")
  })
})

describe("RunDetailInterrupted · Retry behavior", () => {
  beforeEach(() => {
    invokeMock.mockReset()
    useGoalsStore.setState({ runs: [], activeRun: null })
  })

  afterEach(() => {
    useGoalsStore.setState({ runs: [], activeRun: null })
  })

  it.each([
    ["cancelled", undefined],
    ["failed", undefined],
    ["interrupted", "app-close" as InterruptReason],
  ] as const)(
    "RunDetailInterrupted_retry_prefills_modal_without_spawning_for_%s_outcome",
    (outcome, reason) => {
      render(
        <RunDetailInterrupted
          detail={makeDetail({
            outcome,
            goal: "describe auth flow",
            interrupt_reason: reason,
          })}
          vaultPath="/v"
          onBack={() => {}}
        />,
      )
      fireEvent.click(screen.getByTestId("retry-button"))
      const textarea = screen.getByTestId(
        "new-goal-textarea",
      ) as HTMLTextAreaElement
      expect(textarea.value).toBe("describe auth flow")
      // Spec requirement: Retry click alone SHALL NOT spawn a new goal.
      expect(invokeMock).not.toHaveBeenCalledWith("spawn_goal", expect.anything())
    },
  )
})
