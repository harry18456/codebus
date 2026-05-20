/**
 * Per-vault watcher status, populated by the `vault-watcher-error`
 * Tauri event emitted by `codebus-app/src-tauri/src/watcher.rs` when
 * `notify::Watcher::new` fails (most commonly Linux ENOSPC or macOS
 * file-access denial). The frontend SHALL display a persistent
 * `auto-refresh disabled` indicator on every affected tab and SHALL
 * NOT auto-retry the watcher per spec `Watcher Error Surfaces
 * Auto-Refresh-Disabled State`.
 */
import { create } from "zustand"

interface VaultWatcherStatusState {
  /**
   * Map of absolute vault path → human-readable failure reason. A key
   * is present iff the per-vault watcher for that vault failed to
   * start in the current app session.
   */
  disabledVaults: Record<string, string>

  /** Record a watcher failure for `vaultPath`. */
  markDisabled: (vaultPath: string, reason: string) => void

  /**
   * Selector helper: returns the failure reason for `vaultPath`, or
   * `null` when the vault's watcher is healthy (no error recorded).
   */
  reasonFor: (vaultPath: string) => string | null
}

export const useVaultWatcherStatusStore = create<VaultWatcherStatusState>(
  (set, get) => ({
    disabledVaults: {},
    markDisabled(vaultPath, reason) {
      set((state) => ({
        disabledVaults: { ...state.disabledVaults, [vaultPath]: reason },
      }))
    },
    reasonFor(vaultPath) {
      return get().disabledVaults[vaultPath] ?? null
    },
  }),
)
