import { describe, expect, it, beforeEach } from "vitest"

import { PROVIDERS, type ProviderId } from "./providers"
import { useSettingsStore } from "@/store/settings"

describe("provider registry", () => {
  it("contains claude and codex entries with required fields", () => {
    const ids = Object.keys(PROVIDERS) as ProviderId[]
    expect(ids.sort()).toEqual(["claude", "codex"])
    for (const id of ids) {
      const d = PROVIDERS[id]
      expect(d.id).toBe(id)
      expect(typeof d.displayName).toBe("string")
      expect(typeof d.cliBinaryId).toBe("string")
      expect(Array.isArray(d.profiles)).toBe(true)
      expect(typeof d.validate).toBe("function")
    }
  })

  it("declares profiles per provider (claude and codex both system+azure)", () => {
    expect(PROVIDERS.claude.profiles).toEqual(["system", "azure"])
    expect(PROVIDERS.codex.profiles).toEqual(["system", "azure"])
  })

  it("maps each provider to its CLI binary id", () => {
    expect(PROVIDERS.claude.cliBinaryId).toBe("claude_code")
    expect(PROVIDERS.codex.cliBinaryId).toBe("codex")
  })
})

describe("settings store provider-keyed accessors", () => {
  beforeEach(() => {
    useSettingsStore.setState({ config: {}, dirty: false })
  })

  it("updateProviderBlock writes agent.providers.<id> and active_provider", () => {
    const block = { active: "system", system: {}, azure: null } as never
    useSettingsStore.getState().updateProviderBlock("codex", block)
    const cfg = useSettingsStore.getState().config as {
      agent?: { active_provider?: string; providers?: Record<string, unknown> }
    }
    expect(cfg.agent?.active_provider).toBe("codex")
    expect(cfg.agent?.providers?.codex).toEqual(block)
    expect(useSettingsStore.getState().dirty).toBe(true)
  })

  it("updateProviderBlock preserves sibling provider blocks", () => {
    useSettingsStore.setState({
      config: { agent: { active_provider: "claude", providers: { claude: { x: 1 } } } } as never,
    })
    useSettingsStore.getState().updateProviderBlock("codex", { y: 2 } as never)
    const providers = (useSettingsStore.getState().config as {
      agent?: { providers?: Record<string, unknown> }
    }).agent?.providers
    expect(providers?.claude).toEqual({ x: 1 })
    expect(providers?.codex).toEqual({ y: 2 })
  })

  it("getProviderBlock('codex') reads back what was written", () => {
    const block = { active: "azure" } as never
    useSettingsStore.getState().updateProviderBlock("codex", block)
    expect(useSettingsStore.getState().getProviderBlock("codex")).toEqual(block)
  })
})
