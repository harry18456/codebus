import { describe, expect, it } from "vitest"

import type { ToolKind, VerbBanner, VerbEvent } from "@/lib/ipc"
import type { TimelineItem } from "@/components/workspace/ActivityStreamItem"

import {
  classifyToolPhase,
  projectClusters,
  type ClusterItem,
  type ClusterPhase,
} from "./clusterTimeline"

function toolUse(name: string, tool_kind?: ToolKind): VerbEvent {
  return {
    kind: "stream",
    data: { kind: "tool_use", name, input: {}, ...(tool_kind ? { tool_kind } : {}) },
  }
}

function thoughtEvent(text: string): VerbEvent {
  return { kind: "stream", data: { kind: "thought", text } }
}

function bannerEvent(banner: VerbBanner): VerbEvent {
  return { kind: "banner", data: banner }
}

function eventItem(event: VerbEvent): TimelineItem {
  return { kind: "event", event }
}

function thoughtBlock(text: string): TimelineItem {
  return { kind: "thought_block", text }
}

function expectCluster(
  item: ClusterItem,
  phase: ClusterPhase,
  count: number,
): void {
  expect(item.kind).toBe("cluster")
  if (item.kind !== "cluster") return
  expect(item.phase).toBe(phase)
  expect(item.count).toBe(count)
  expect(item.events.length).toBe(count)
}

describe("classifyToolPhase · tool name dispatch", () => {
  it("Read / Glob / Grep all classify as reading_codebase", () => {
    expect(classifyToolPhase(toolUse("Read"))).toBe("reading_codebase")
    expect(classifyToolPhase(toolUse("Glob"))).toBe("reading_codebase")
    expect(classifyToolPhase(toolUse("Grep"))).toBe("reading_codebase")
  })

  it("Write / Edit classify as writing_wiki", () => {
    expect(classifyToolPhase(toolUse("Write"))).toBe("writing_wiki")
    expect(classifyToolPhase(toolUse("Edit"))).toBe("writing_wiki")
  })

  it("Bash with tool_kind=read groups as reading_codebase", () => {
    expect(classifyToolPhase(toolUse("Bash", "read"))).toBe("reading_codebase")
  })

  it("Bash with tool_kind=inspect groups as reading_codebase", () => {
    expect(classifyToolPhase(toolUse("Bash", "inspect"))).toBe("reading_codebase")
  })

  it("Bash with tool_kind=mutation groups as writing_wiki", () => {
    expect(classifyToolPhase(toolUse("Bash", "mutation"))).toBe("writing_wiki")
  })

  it("Bash with tool_kind=other_read groups as reading_codebase", () => {
    expect(classifyToolPhase(toolUse("Bash", "other_read"))).toBe(
      "reading_codebase",
    )
  })

  it("Bash with tool_kind=other_write groups as writing_wiki", () => {
    expect(classifyToolPhase(toolUse("Bash", "other_write"))).toBe(
      "writing_wiki",
    )
  })

  it("Shell (codex) without tool_kind falls back to reading_codebase (Inspect)", () => {
    expect(classifyToolPhase(toolUse("Shell"))).toBe("reading_codebase")
  })

  // Per app-workspace § "Missing tool_kind defaults to Inspect" scenario.
  it("classifyToolPhase_legacy_event_no_tool_kind_groups_reading", () => {
    const legacyBash: VerbEvent = {
      kind: "stream",
      data: { kind: "tool_use", name: "Bash", input: { command: "git status" } },
    }
    expect(classifyToolPhase(legacyBash)).toBe("reading_codebase")
  })

  it("non-tool events classify as null", () => {
    expect(classifyToolPhase(thoughtEvent("hi"))).toBeNull()
    expect(
      classifyToolPhase(bannerEvent({ kind: "goal", goal_text: "x" })),
    ).toBeNull()
  })
})

describe("projectClusters · fold scenarios (Activity Stream Two-Phase Cluster Rendering)", () => {
  it("three consecutive Read calls fold into one count=3 reading cluster", () => {
    const items: TimelineItem[] = [
      eventItem(toolUse("Read")),
      eventItem(toolUse("Read")),
      eventItem(toolUse("Read")),
    ]
    const out = projectClusters(items)
    expect(out.length).toBe(1)
    expectCluster(out[0], "reading_codebase", 3)
  })

  it("Read followed by Write opens two distinct clusters", () => {
    const items: TimelineItem[] = [
      eventItem(toolUse("Read")),
      eventItem(toolUse("Write")),
      eventItem(toolUse("Edit")),
    ]
    const out = projectClusters(items)
    expect(out.length).toBe(2)
    expectCluster(out[0], "reading_codebase", 1)
    expectCluster(out[1], "writing_wiki", 2)
  })

  it("banner inside cluster ends the cluster and is rendered flat", () => {
    const items: TimelineItem[] = [
      eventItem(toolUse("Read")),
      eventItem(bannerEvent({ kind: "commit_done", sha7: "abc1234" })),
      eventItem(toolUse("Read")),
    ]
    const out = projectClusters(items)
    expect(out.length).toBe(3)
    expectCluster(out[0], "reading_codebase", 1)
    expect(out[1].kind).toBe("event")
    expectCluster(out[2], "reading_codebase", 1)
  })

  it("thought_block ends the cluster and is rendered flat", () => {
    const items: TimelineItem[] = [
      eventItem(toolUse("Read")),
      thoughtBlock("checking..."),
      eventItem(toolUse("Read")),
    ]
    const out = projectClusters(items)
    expect(out.length).toBe(3)
    expectCluster(out[0], "reading_codebase", 1)
    expect(out[1].kind).toBe("thought_block")
    expectCluster(out[2], "reading_codebase", 1)
  })

  it("clusters MAY repeat across timeline (read → thought → read → thought → write)", () => {
    const items: TimelineItem[] = [
      eventItem(toolUse("Read")),
      thoughtBlock("a"),
      eventItem(toolUse("Read")),
      thoughtBlock("b"),
      eventItem(toolUse("Write")),
    ]
    const out = projectClusters(items)
    // 5 entries: cluster, thought, cluster, thought, cluster
    expect(out.map((i) => i.kind)).toEqual([
      "cluster",
      "thought_block",
      "cluster",
      "thought_block",
      "cluster",
    ])
    expectCluster(out[0], "reading_codebase", 1)
    expectCluster(out[2], "reading_codebase", 1)
    expectCluster(out[4], "writing_wiki", 1)
  })

  it("cluster count excludes thought blocks (only tool_use rows counted)", () => {
    // thought_block between two Read events MUST split into two clusters;
    // neither cluster's count includes the thought.
    const items: TimelineItem[] = [
      eventItem(toolUse("Read")),
      thoughtBlock("inline thought"),
      eventItem(toolUse("Read")),
      eventItem(toolUse("Read")),
    ]
    const out = projectClusters(items)
    expect(out.length).toBe(3)
    expectCluster(out[0], "reading_codebase", 1)
    expect(out[1].kind).toBe("thought_block")
    expectCluster(out[2], "reading_codebase", 2)
  })

  it("does not mutate input array", () => {
    const items: TimelineItem[] = [
      eventItem(toolUse("Read")),
      eventItem(toolUse("Read")),
    ]
    const snapshot = JSON.parse(JSON.stringify(items))
    projectClusters(items)
    expect(items).toEqual(snapshot)
  })
})
