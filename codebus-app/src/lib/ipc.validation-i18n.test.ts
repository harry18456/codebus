import { describe, expect, it } from "vitest"

import { messages, type MessageKey } from "@/i18n/messages"

import {
  type ClaudeCodeBlock,
  type ClaudeCodeValidationError,
  type CodexBlock,
  validateClaudeCodeBlock,
  validateCodexBlock,
} from "./ipc"

const VALIDATION_KEYS: readonly MessageKey[] = [
  "settings.endpoint.validation.azureProfileRequired",
  "settings.endpoint.validation.baseUrlRequired",
  "settings.endpoint.validation.apiVersionRequired",
  "settings.endpoint.validation.keyringServiceRequired",
  "settings.endpoint.validation.deploymentNameRequired",
  "settings.endpoint.validation.effortInvalid",
  "settings.endpoint.validation.systemModelRequired",
] as const

function emptyClaudeAzureBlock(): ClaudeCodeBlock {
  return {
    active: "azure",
    system: {
      goal: { model: "opus", effort: "high" },
      query: { model: "haiku", effort: "low" },
      fix: { model: "sonnet", effort: "medium" },
      verify: { model: "opus", effort: "high" },
    },
    azure: {
      base_url: "",
      keyring_service: "",
      goal: { model: "", effort: "high" },
      query: { model: "dep", effort: "low" },
      fix: { model: "dep", effort: "medium" },
      verify: { model: "dep", effort: "high" },
    },
  }
}

function emptyCodexAzureBlock(): CodexBlock {
  return {
    active: "azure",
    system: {
      goal: { model: "gpt", effort: "high" },
      query: { model: "gpt", effort: "low" },
      fix: { model: "gpt", effort: "medium" },
      verify: { model: "gpt", effort: "high" },
    },
    azure: {
      base_url: "",
      api_version: "",
      keyring_service: "",
      goal: { model: "", effort: "high" },
      query: { model: "dep", effort: "low" },
      fix: { model: "dep", effort: "medium" },
      verify: { model: "dep", effort: "high" },
    },
  }
}

describe("ipc.ts validation i18n bundle coverage (app-shell policy)", () => {
  it("declares all 7 validation keys in en bundle", () => {
    for (const key of VALIDATION_KEYS) {
      expect(messages.en[key], `en bundle missing ${key}`).toBeTruthy()
    }
  })

  it("declares all 7 validation keys in zh bundle", () => {
    for (const key of VALIDATION_KEYS) {
      expect(messages.zh[key], `zh bundle missing ${key}`).toBeTruthy()
    }
  })

  it("en and zh values differ for every validation key (real translation, not stub)", () => {
    for (const key of VALIDATION_KEYS) {
      expect(
        messages.en[key],
        `${key}: en and zh should not be identical (would mean missing translation)`,
      ).not.toBe(messages.zh[key])
    }
  })

  it("preserves placeholders verbatim across en and zh", () => {
    const extractPlaceholders = (template: string): string[] =>
      Array.from(template.matchAll(/\{(\w+)\}/g))
        .map((m) => m[1])
        .sort()
    for (const key of VALIDATION_KEYS) {
      const enPlaceholders = extractPlaceholders(messages.en[key])
      const zhPlaceholders = extractPlaceholders(messages.zh[key])
      expect(zhPlaceholders, `${key}: placeholders drifted`).toEqual(
        enPlaceholders,
      )
    }
  })
})

describe("validateClaudeCodeBlock returns LocalizedError-shaped entries", () => {
  it("returns key (not message) for empty azure profile", () => {
    const block: ClaudeCodeBlock = {
      active: "azure",
      system: emptyClaudeAzureBlock().system,
      azure: null,
    }
    const errors = validateClaudeCodeBlock(block)
    expect(errors).toContainEqual<ClaudeCodeValidationError>({
      field: "claude_code.azure",
      key: "settings.endpoint.validation.azureProfileRequired",
    })
    expect(
      errors.every((e) => !("message" in e)),
      "no entry should carry a legacy `message` field",
    ).toBe(true)
  })

  it("returns baseUrlRequired + keyringServiceRequired + deploymentNameRequired keys for empty azure fields", () => {
    const block = emptyClaudeAzureBlock()
    const errors = validateClaudeCodeBlock(block)
    const byField = new Map(errors.map((e) => [e.field, e]))
    expect(byField.get("claude_code.azure.base_url")?.key).toBe(
      "settings.endpoint.validation.baseUrlRequired",
    )
    expect(byField.get("claude_code.azure.keyring_service")?.key).toBe(
      "settings.endpoint.validation.keyringServiceRequired",
    )
    const deployment = byField.get("claude_code.azure.goal.model")
    expect(deployment?.key).toBe(
      "settings.endpoint.validation.deploymentNameRequired",
    )
    expect(deployment?.vars).toEqual({ verb: "goal" })
  })

  it("returns effortInvalid key with verb + allowed vars for non-enum effort", () => {
    const block = emptyClaudeAzureBlock()
    block.system.query.effort = "super-high"
    const errors = validateClaudeCodeBlock(block)
    const effortErr = errors.find(
      (e) => e.field === "claude_code.system.query.effort",
    )
    expect(effortErr?.key).toBe("settings.endpoint.validation.effortInvalid")
    expect(effortErr?.vars).toEqual({
      verb: "query",
      allowed: expect.stringContaining("high"),
    })
  })
})

describe("validateCodexBlock returns LocalizedError-shaped entries", () => {
  it("returns key for empty azure profile (codex)", () => {
    const block: CodexBlock = {
      active: "azure",
      system: emptyCodexAzureBlock().system,
      azure: null,
    }
    const errors = validateCodexBlock(block)
    expect(errors).toContainEqual<ClaudeCodeValidationError>({
      field: "codex.azure",
      key: "settings.endpoint.validation.azureProfileRequired",
    })
  })

  it("returns apiVersionRequired key (codex-specific) for empty api_version", () => {
    const block = emptyCodexAzureBlock()
    const errors = validateCodexBlock(block)
    const apiVersionErr = errors.find(
      (e) => e.field === "codex.azure.api_version",
    )
    expect(apiVersionErr?.key).toBe(
      "settings.endpoint.validation.apiVersionRequired",
    )
  })

  it("returns systemModelRequired key with verb var for empty system model (codex)", () => {
    const block = emptyCodexAzureBlock()
    block.active = "system"
    block.system.fix.model = ""
    const errors = validateCodexBlock(block)
    const modelErr = errors.find((e) => e.field === "codex.system.fix.model")
    expect(modelErr?.key).toBe(
      "settings.endpoint.validation.systemModelRequired",
    )
    expect(modelErr?.vars).toEqual({ verb: "fix" })
  })

  it("returns deploymentNameRequired key with verb var for empty codex azure deployment", () => {
    const block = emptyCodexAzureBlock()
    const errors = validateCodexBlock(block)
    const depErr = errors.find((e) => e.field === "codex.azure.goal.model")
    expect(depErr?.key).toBe(
      "settings.endpoint.validation.deploymentNameRequired",
    )
    expect(depErr?.vars).toEqual({ verb: "goal" })
  })
})
