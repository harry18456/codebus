/**
 * Single source of truth for the `ActionEntry` shape consumed by both
 * `useExplorerStream` (Module 4 Explorer console — `stepBuckets[].actions[]`)
 * and `useQaSession` (Q&A drawer overlay — `reactSteps[].actions[]`).
 *
 * Spec references:
 *   - `openspec/specs/qa-overlay/spec.md` line 33 (`actions: ActionEntry[]`)
 *     and line 55 (cross-reference to the Explorer console's ActionEntry).
 *   - `openspec/specs/agent-console/spec.md` line 45 (`stepBuckets[event.step]
 *     .actions[]` inline shape `{ tool, observation, tokens_used, isError }`).
 *
 * Landed by change `fix-action-entry-import-collision` to break the Nuxt
 * auto-import duplicate-export warning that fired when both composables
 * declared their own `export interface ActionEntry`.
 */
export interface ActionEntry {
  tool: string
  observation: string
  tokens_used: number
  isError: boolean
}
