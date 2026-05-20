/**
 * Persistent inline banner shown at the top of Wiki / Goals / Quiz tabs
 * whenever the per-vault watcher for the open vault has failed to start.
 * Reads its state from `useVaultWatcherStatusStore`, populated by the
 * Workspace's subscription to the `vault-watcher-error` Tauri event.
 *
 * Spec: `Watcher Error Surfaces Auto-Refresh-Disabled State`. The banner
 * SHALL NOT auto-retry the watcher — the frontend treats the disabled
 * state as session-scoped per design D6.
 */
import { useVaultWatcherStatusStore } from "@/store/vault-watcher-status"

interface WatcherStatusBannerProps {
  vaultPath: string
}

export function WatcherStatusBanner({ vaultPath }: WatcherStatusBannerProps) {
  const reason = useVaultWatcherStatusStore((s) => s.reasonFor(vaultPath))
  if (!reason) return null
  return (
    <div
      data-testid="watcher-status-banner"
      role="alert"
      className="border-b border-warn/40 bg-warn/10 px-3 py-1 text-[11px] text-warn"
    >
      Auto-refresh disabled: {reason}
    </div>
  )
}
