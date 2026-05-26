import { describe, expect, it } from "vitest"

import {
  SYSTEM_EFFORTS,
  validateClaudeCodeBlock,
  type ClaudeCodeBlock,
  type SystemEffort,
} from "./ipc"

/**
 * Spec: app-shell § Settings UI Endpoint Section.
 *
 * `SYSTEM_EFFORTS` is the closed enum of valid `effort` values surfaced
 * by the Settings UI dropdown. The Rust side keeps `effort: String` for
 * yaml backward compatibility, so the enum is only enforced at the UI
 * layer via `validateClaudeCodeBlock`.
 */
describe("SYSTEM_EFFORTS constant", () => {
  it("contains the six Claude Code effort levels in fixed order", () => {
    expect(SYSTEM_EFFORTS).toEqual([
      "low",
      "medium",
      "high",
      "xhigh",
      "max",
      "auto",
    ])
  })

  it("type narrows literal union members", () => {
    const low: SystemEffort = "low"
    const medium: SystemEffort = "medium"
    const high: SystemEffort = "high"
    const xhigh: SystemEffort = "xhigh"
    const max: SystemEffort = "max"
    const auto: SystemEffort = "auto"
    expect([low, medium, high, xhigh, max, auto]).toEqual([
      "low",
      "medium",
      "high",
      "xhigh",
      "max",
      "auto",
    ])
  })
})

describe("validateClaudeCodeBlock — effort enum enforcement", () => {
  function baseBlock(): ClaudeCodeBlock {
    return {
      active: "system",
      system: {
        goal: { model: "opus-4-6", effort: "high" },
        query: { model: "haiku-4-5", effort: "low" },
        fix: { model: "sonnet-4-6", effort: "medium" },
        verify: { model: "opus-4-6", effort: "high" },
      },
      azure: null,
    }
  }

  it("returns no errors when every effort is in SYSTEM_EFFORTS", () => {
    const errors = validateClaudeCodeBlock(baseBlock())
    expect(errors).toEqual([])
  })

  // Scenario: Legacy invalid effort value renders empty select trigger
  // and flags validation (system side).
  it("flags an invalid system effort value", () => {
    const block = baseBlock()
    block.system.goal.effort = "super-high"
    const errors = validateClaudeCodeBlock(block)
    expect(errors).toContainEqual({
      field: "claude_code.system.goal.effort",
      key: "settings.endpoint.validation.effortInvalid",
      vars: {
        verb: expect.any(String),
        allowed: expect.stringContaining("high"),
      },
    })
  })

  // Scenario: Inactive profile invalid effort still blocks Save.
  it("flags an invalid azure effort even when active=system", () => {
    const block = baseBlock()
    block.azure = {
      base_url: "https://x.example.com/anthropic",
      keyring_service: "codebus-azure",
      goal: { model: "dep-x", effort: "high" },
      query: { model: "dep-y", effort: "low" },
      fix: { model: "dep-z", effort: "extreme" },
      verify: { model: "dep-x", effort: "high" },
    }
    const errors = validateClaudeCodeBlock(block)
    expect(errors).toContainEqual({
      field: "claude_code.azure.fix.effort",
      key: "settings.endpoint.validation.effortInvalid",
      vars: {
        verb: expect.any(String),
        allowed: expect.stringContaining("high"),
      },
    })
  })

  // Scenario: Save button enables when active=azure becomes fully populated
  // — verifies the validator's "no errors" path under active=azure.
  it("returns no errors when active=azure and every field is valid", () => {
    const block: ClaudeCodeBlock = {
      active: "azure",
      system: {
        goal: { model: "opus-4-6", effort: "high" },
        query: { model: "haiku-4-5", effort: "low" },
        fix: { model: "sonnet-4-6", effort: "medium" },
        verify: { model: "opus-4-6", effort: "high" },
      },
      azure: {
        base_url: "https://x.example.com/anthropic",
        keyring_service: "codebus-azure",
        goal: { model: "dep-x", effort: "high" },
        query: { model: "dep-y", effort: "low" },
        fix: { model: "dep-z", effort: "medium" },
        verify: { model: "dep-x", effort: "high" },
      },
    }
    expect(validateClaudeCodeBlock(block)).toEqual([])
  })

  it("flags the empty string as invalid effort", () => {
    const block = baseBlock()
    block.system.query.effort = ""
    const errors = validateClaudeCodeBlock(block)
    expect(errors).toContainEqual({
      field: "claude_code.system.query.effort",
      key: "settings.endpoint.validation.effortInvalid",
      vars: {
        verb: expect.any(String),
        allowed: expect.stringContaining("high"),
      },
    })
  })
})
