import { fireEvent, render, screen } from "@testing-library/react"
import { describe, expect, it } from "vitest"

import { ActivityCluster } from "./ActivityCluster"
import type { VerbEvent } from "@/lib/ipc"

function toolUse(name: string): VerbEvent {
  return { kind: "stream", data: { kind: "tool_use", name, input: {} } }
}

function withLocale(lang: string, run: () => void) {
  const orig = navigator.language
  Object.defineProperty(navigator, "language", {
    value: lang,
    configurable: true,
  })
  try {
    run()
  } finally {
    Object.defineProperty(navigator, "language", {
      value: orig,
      configurable: true,
    })
  }
}

describe("ActivityCluster · headings + a11y", () => {
  it("render_reading_phase_with_mono_icon_prefix (en)", () => {
    withLocale("en", () => {
      render(
        <ActivityCluster
          phase="reading_codebase"
          events={[toolUse("Read"), toolUse("Glob")]}
          count={2}
          terminal={false}
        >
          <div data-testid="child">stub</div>
        </ActivityCluster>,
      )
      const heading = screen.getByTestId("activity-cluster-heading")
      expect(heading.textContent).toContain("Reading codebase")
      // Spec: heading SHALL contain at least one of the mono icons.
      const hasMonoIcon = /📄|🗂|🔍|\$_|\$\?/.test(heading.textContent ?? "")
      expect(hasMonoIcon).toBe(true)
      // Spec: heading SHALL NOT contain generic 🛠️ emoji.
      expect(heading.textContent ?? "").not.toContain("🛠️")
    })
  })

  it("default_open_during_running renders children", () => {
    render(
      <ActivityCluster
        phase="reading_codebase"
        events={[toolUse("Read")]}
        count={1}
        terminal={false}
      >
        <div data-testid="child">visible</div>
      </ActivityCluster>,
    )
    const heading = screen.getByTestId("activity-cluster-heading")
    expect(heading.getAttribute("aria-expanded")).toBe("true")
    expect(screen.getByTestId("child")).toBeTruthy()
  })

  it("default_closed_when_done hides children", () => {
    render(
      <ActivityCluster
        phase="reading_codebase"
        events={[toolUse("Read")]}
        count={1}
        terminal={true}
      >
        <div data-testid="child">hidden</div>
      </ActivityCluster>,
    )
    const heading = screen.getByTestId("activity-cluster-heading")
    expect(heading.getAttribute("aria-expanded")).toBe("false")
    // Children always mount (a11y + test DOM presence); `hidden` is the
    // only visibility gate.
    const container = screen.getByTestId("activity-cluster-children")
    expect(container.hasAttribute("hidden")).toBe(true)
  })

  it("user_toggles_via_heading_button_flips_aria_expanded", () => {
    render(
      <ActivityCluster
        phase="writing_wiki"
        events={[toolUse("Write")]}
        count={1}
        terminal={true}
      >
        <div data-testid="child">x</div>
      </ActivityCluster>,
    )
    const heading = screen.getByTestId("activity-cluster-heading")
    const container = screen.getByTestId("activity-cluster-children")
    expect(heading.getAttribute("aria-expanded")).toBe("false")
    expect(container.hasAttribute("hidden")).toBe(true)
    fireEvent.click(heading)
    expect(heading.getAttribute("aria-expanded")).toBe("true")
    expect(container.hasAttribute("hidden")).toBe(false)
    fireEvent.click(heading)
    expect(heading.getAttribute("aria-expanded")).toBe("false")
    expect(container.hasAttribute("hidden")).toBe(true)
  })

  it("count_rendered_in_heading", () => {
    render(
      <ActivityCluster
        phase="reading_codebase"
        events={[toolUse("Read"), toolUse("Read"), toolUse("Read")]}
        count={3}
        terminal={false}
      >
        <></>
      </ActivityCluster>,
    )
    expect(screen.getByTestId("activity-cluster-count").textContent).toBe("(3)")
  })
})

describe("ActivityCluster · collapsed summary (terminal only)", () => {
  // From design v1.5 § 02b summary examples:
  //   Reading codebase · 12 reads · 195 shell · 6.2s
  //   讀檔案 12 次 · shell 195 次 · 6.2 秒
  //   Writing wiki · 3 new · 2 updated · 4.5s
  //   新增 3 · 更新 2 · 4.5 秒
  function readingEvents(reads: number, shell: number): VerbEvent[] {
    const out: VerbEvent[] = []
    for (let i = 0; i < reads; i++) out.push(toolUse("Read"))
    for (let i = 0; i < shell; i++) out.push(toolUse("Bash"))
    return out
  }
  function writingEvents(created: number, updated: number): VerbEvent[] {
    const out: VerbEvent[] = []
    for (let i = 0; i < created; i++) out.push(toolUse("Write"))
    for (let i = 0; i < updated; i++) out.push(toolUse("Edit"))
    return out
  }

  it("summary_renders_localized_en (reading)", () => {
    withLocale("en", () => {
      render(
        <ActivityCluster
          phase="reading_codebase"
          events={readingEvents(12, 195)}
          count={207}
          terminal={true}
          elapsedMs={6200}
        >
          <></>
        </ActivityCluster>,
      )
      const summary = screen.getByTestId("activity-cluster-summary")
      expect(summary.textContent).toBe(
        "Reading codebase · 12 reads · 195 shell · 6.2s",
      )
    })
  })

  it("summary_renders_localized_zh (reading)", () => {
    withLocale("zh-TW", () => {
      render(
        <ActivityCluster
          phase="reading_codebase"
          events={readingEvents(12, 195)}
          count={207}
          terminal={true}
          elapsedMs={6200}
        >
          <></>
        </ActivityCluster>,
      )
      const summary = screen.getByTestId("activity-cluster-summary")
      expect(summary.textContent).toBe("讀檔案 12 次 · shell 195 次 · 6.2 秒")
    })
  })

  it("summary_renders_localized_en (writing)", () => {
    withLocale("en", () => {
      render(
        <ActivityCluster
          phase="writing_wiki"
          events={writingEvents(3, 2)}
          count={5}
          terminal={true}
          elapsedMs={4500}
        >
          <></>
        </ActivityCluster>,
      )
      const summary = screen.getByTestId("activity-cluster-summary")
      expect(summary.textContent).toBe(
        "Writing wiki · 3 new · 2 updated · 4.5s",
      )
    })
  })

  it("summary_renders_localized_zh (writing)", () => {
    withLocale("zh-TW", () => {
      render(
        <ActivityCluster
          phase="writing_wiki"
          events={writingEvents(3, 2)}
          count={5}
          terminal={true}
          elapsedMs={4500}
        >
          <></>
        </ActivityCluster>,
      )
      const summary = screen.getByTestId("activity-cluster-summary")
      expect(summary.textContent).toBe("新增 3 · 更新 2 · 4.5 秒")
    })
  })

  it("summary is not rendered while running (terminal=false)", () => {
    render(
      <ActivityCluster
        phase="reading_codebase"
        events={readingEvents(2, 2)}
        count={4}
        terminal={false}
        elapsedMs={1000}
      >
        <></>
      </ActivityCluster>,
    )
    expect(screen.queryByTestId("activity-cluster-summary")).toBeNull()
  })
})

describe("ActivityCluster · a11y attributes", () => {
  it("heading_is_button_with_aria_attributes", () => {
    render(
      <ActivityCluster
        phase="reading_codebase"
        events={[toolUse("Read")]}
        count={1}
        terminal={false}
      >
        <></>
      </ActivityCluster>,
    )
    const heading = screen.getByTestId("activity-cluster-heading")
    expect(heading.tagName).toBe("BUTTON")
    expect(heading.getAttribute("type")).toBe("button")
    expect(heading.getAttribute("aria-expanded")).not.toBeNull()
    expect(heading.getAttribute("aria-controls")).not.toBeNull()
  })

  it("aria_controls_targets_visible_child_container", () => {
    render(
      <ActivityCluster
        phase="reading_codebase"
        events={[toolUse("Read")]}
        count={1}
        terminal={false}
      >
        <div data-testid="child">visible</div>
      </ActivityCluster>,
    )
    const heading = screen.getByTestId("activity-cluster-heading")
    const childContainer = screen.getByTestId("activity-cluster-children")
    expect(heading.getAttribute("aria-controls")).toBe(
      childContainer.getAttribute("id"),
    )
    expect(childContainer.hasAttribute("hidden")).toBe(false)
  })
})
