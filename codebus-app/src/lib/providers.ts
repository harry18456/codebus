import {
  type ClaudeCodeBlock,
  type ClaudeCodeValidationError,
  type CodexBlock,
  validateClaudeCodeBlock,
  validateCodexBlock,
} from "./ipc"

/**
 * Provider id — the value stored at `agent.active_provider` and the key under
 * `agent.providers.<id>`. Adding a future provider extends this union plus a
 * registry entry below (and its own editor component); no cross-provider glue
 * changes.
 */
export type ProviderId = "claude" | "codex"

/**
 * One provider's settings-layer descriptor. The registry abstracts ONLY the
 * cross-provider glue (selection, config read/write by id, CLI status probe by
 * `cliBinaryId`, validation dispatch). Provider-specific form rendering lives
 * in each entry's editor component (added by the SettingsModal wiring) — there
 * is no generic schema-to-form engine. `profiles` is declared per provider and
 * is NOT assumed to include `azure`.
 */
export interface ProviderDescriptor {
  id: ProviderId
  displayName: string
  /** Argument passed to `check_cli_installed`. */
  cliBinaryId: string
  /** Endpoint profiles this provider supports, declared per provider. */
  profiles: string[]
  /** Client-side validation; rules mirror the provider's codebus-core parser. */
  validate: (block: unknown) => ClaudeCodeValidationError[]
}

export const PROVIDERS: Record<ProviderId, ProviderDescriptor> = {
  claude: {
    id: "claude",
    displayName: "Claude Code",
    cliBinaryId: "claude_code",
    profiles: ["system", "azure"],
    validate: (block) => validateClaudeCodeBlock(block as ClaudeCodeBlock),
  },
  codex: {
    id: "codex",
    displayName: "OpenAI Codex",
    cliBinaryId: "codex",
    profiles: ["system", "azure"],
    validate: (block) => validateCodexBlock(block as CodexBlock),
  },
}
