import { describe, expect, it } from "vitest"

import { type CodexBlock, validateCodexBlock } from "./ipc"

function azureBlock(over: Partial<CodexBlock["azure"]> = {}): CodexBlock {
  return {
    active: "azure",
    system: {
      goal: { model: "gpt-5.5", effort: "high" },
      query: { model: "gpt-5.5", effort: "low" },
      fix: { model: "gpt-5.5", effort: "medium" },
      verify: { model: "gpt-5.5", effort: "high" },
    },
    azure: {
      base_url: "https://x.cognitiveservices.azure.com/openai",
      api_version: "2025-04-01-preview",
      keyring_service: "codebus-azure",
      goal: { model: "gpt-5.4", effort: "high" },
      query: { model: "gpt-5.4", effort: "low" },
      fix: { model: "gpt-5.4", effort: "medium" },
      verify: { model: "gpt-5.4", effort: "high" },
      ...over,
    },
  }
}

describe("validateCodexBlock", () => {
  it("accepts a fully populated azure block", () => {
    expect(validateCodexBlock(azureBlock())).toEqual([])
  })

  it("flags missing api_version when active=azure", () => {
    const errs = validateCodexBlock(azureBlock({ api_version: "" }))
    expect(errs.map((e) => e.field)).toContain("codex.azure.api_version")
  })

  it("flags missing base_url and keyring_service when active=azure", () => {
    const errs = validateCodexBlock(azureBlock({ base_url: "  ", keyring_service: "" }))
    const fields = errs.map((e) => e.field)
    expect(fields).toContain("codex.azure.base_url")
    expect(fields).toContain("codex.azure.keyring_service")
  })

  it("flags an empty azure verb deployment model", () => {
    const errs = validateCodexBlock(azureBlock({ verify: { model: "", effort: "high" } }))
    expect(errs.map((e) => e.field)).toContain("codex.azure.verify.model")
  })

  it("requires non-empty system verb models when active=system", () => {
    const block: CodexBlock = {
      active: "system",
      system: {
        goal: { model: "gpt-5.5", effort: "high" },
        query: { model: "", effort: "low" },
        fix: { model: "gpt-5.5", effort: "medium" },
        verify: { model: "gpt-5.5", effort: "high" },
      },
      azure: null,
    }
    expect(validateCodexBlock(block).map((e) => e.field)).toContain("codex.system.query.model")
  })

  it("accepts arbitrary (non-enum) system model strings", () => {
    const block: CodexBlock = {
      active: "system",
      system: {
        goal: { model: "gpt-5.5", effort: "max" },
        query: { model: "o4-mini", effort: "low" },
        fix: { model: "gpt-5.5-codex", effort: "medium" },
        verify: { model: "anything-goes", effort: "high" },
      },
      azure: null,
    }
    // No closed-enum rejection, no effort-enum rejection.
    expect(validateCodexBlock(block)).toEqual([])
  })
})
