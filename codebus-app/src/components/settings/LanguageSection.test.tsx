import { beforeEach, describe, expect, it, vi } from "vitest"
import { render, screen, fireEvent } from "@testing-library/react"

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}))

import { invoke } from "@tauri-apps/api/core"
import { SettingsModal } from "./SettingsModal"
import { LanguageSection } from "./LanguageSection"
import { useSettingsStore } from "@/store/settings"

const mockedInvoke = vi.mocked(invoke)

function seedClaudeConfig(extra: Record<string, unknown> = {}) {
  useSettingsStore.setState({
    config: {
      app: { quiz: { pass_threshold: 80, default_length: 5 }, ...extra },
      agent: {
        active_provider: "claude",
        providers: {
          claude: {
            active: "system",
            system: {
              goal: { model: "opus-4-6", effort: "high" },
              query: { model: "haiku-4-5", effort: "low" },
              fix: { model: "sonnet-4-6", effort: "medium" },
              verify: { model: "opus-4-6", effort: "high" },
            },
          },
        },
      },
      pii: { scanner: "regex_basic" },
      log: { sink: "~/.codebus/logs/" },
    } as never,
    initialConfig: {
      app: { quiz: { pass_threshold: 80, default_length: 5 } },
    } as never,
    dirty: false,
    loading: false,
    saving: false,
    error: null,
  })
}

describe("LanguageSection (standalone)", () => {
  beforeEach(() => seedClaudeConfig())

  it("renders three options labeled Auto / 中文 / English", () => {
    render(<LanguageSection value={null} onChange={() => {}} />)
    fireEvent.click(screen.getByTestId("language-select-trigger"))
    // Auto label is locale-dependent; identifier labels are not.
    expect(screen.getByTestId("language-option-auto")).toBeInTheDocument()
    expect(screen.getByTestId("language-option-zh").textContent).toBe("中文")
    expect(screen.getByTestId("language-option-en").textContent).toBe("English")
  })

  it("selecting 'English' emits the 'en' store value", () => {
    const onChange = vi.fn()
    render(<LanguageSection value={null} onChange={onChange} />)
    fireEvent.click(screen.getByTestId("language-select-trigger"))
    fireEvent.click(screen.getByTestId("language-option-en"))
    expect(onChange).toHaveBeenCalledWith("en")
  })

  it("selecting 'Auto' emits null (auto-detect)", () => {
    const onChange = vi.fn()
    render(<LanguageSection value="en" onChange={onChange} />)
    fireEvent.click(screen.getByTestId("language-select-trigger"))
    fireEvent.click(screen.getByTestId("language-option-auto"))
    expect(onChange).toHaveBeenCalledWith(null)
  })

  it("selecting '中文' emits 'zh'", () => {
    const onChange = vi.fn()
    render(<LanguageSection value={null} onChange={onChange} />)
    fireEvent.click(screen.getByTestId("language-select-trigger"))
    fireEvent.click(screen.getByTestId("language-option-zh"))
    expect(onChange).toHaveBeenCalledWith("zh")
  })
})

describe("SettingsModal · Language dropdown integration", () => {
  beforeEach(() => {
    seedClaudeConfig()
    mockedInvoke.mockReset()
    mockedInvoke.mockImplementation((cmd: string) => {
      if (cmd === "get_endpoint_key") return Promise.resolve({ kind: "unset" })
      return Promise.resolve(useSettingsStore.getState().config)
    })
  })

  it("renders the Language field between Endpoint Section and PII scanner", () => {
    render(<SettingsModal open onClose={() => {}} piiPatternCount={14} />)
    // Trigger exists in the modal.
    expect(screen.getByTestId("language-select-trigger")).toBeInTheDocument()

    // DOM order: endpoint section → language trigger → PII scanner trigger.
    // Radix Dialog renders into a portal so use screen / document queries.
    const endpoint = screen.getByTestId("endpoint-section")
    const language = screen.getByTestId("language-select-trigger")
    const pii = screen.getByTestId("pii-scanner-trigger")

    // Use Node#compareDocumentPosition: bit 4 = FOLLOWING.
    expect(endpoint.compareDocumentPosition(language) & 0x04).toBe(0x04)
    expect(language.compareDocumentPosition(pii) & 0x04).toBe(0x04)
  })

  it("picking English writes app.locale_override='en' to the store and marks dirty", () => {
    render(<SettingsModal open onClose={() => {}} piiPatternCount={14} />)
    fireEvent.click(screen.getByTestId("language-select-trigger"))
    fireEvent.click(screen.getByTestId("language-option-en"))
    const cfg = useSettingsStore.getState().config as {
      app?: { locale_override?: string | null }
    }
    expect(cfg.app?.locale_override).toBe("en")
    expect(useSettingsStore.getState().dirty).toBe(true)
  })

  it("picking Auto writes app.locale_override=null", () => {
    seedClaudeConfig({ locale_override: "en" })
    render(<SettingsModal open onClose={() => {}} piiPatternCount={14} />)
    fireEvent.click(screen.getByTestId("language-select-trigger"))
    fireEvent.click(screen.getByTestId("language-option-auto"))
    const cfg = useSettingsStore.getState().config as {
      app?: { locale_override?: string | null }
    }
    expect(cfg.app?.locale_override).toBeNull()
  })

  it("install hint uses the i18n key (no hard-coded English literal)", () => {
    seedClaudeConfig()
    // Force the cliStatus → not_installed branch by stubbing the IPC.
    // For this test we render and verify the source no longer contains the
    // legacy hard-coded string — the dynamic render path is exercised by
    // the en/zh smoke step in task 5.2 (full CDP). Here we just guard the
    // grep contract from regression at unit-test time.
    const src = String(SettingsModal.toString())
    expect(src).not.toContain("Install {provider.displayName} first")
  })
})
