import { useEffect } from "react"

import type { VaultEntry } from "@/lib/ipc"
import { useVaultsStore } from "@/store/vaults"
import { useRouteStore } from "@/store/route"
import { useT } from "@/i18n/useT"
import { useWatcherEvent } from "@/hooks/useWatcherEvent"

import { EmptyState } from "./EmptyState"
import { VaultCard } from "./VaultCard"
import { Button } from "@/components/ui/button"
import { SectionLabel } from "@/components/ui/SectionLabel"

interface LobbyProps {
  onNewVault: () => void
  onRevealInFiles: (v: VaultEntry) => void
}

export function Lobby({ onNewVault, onRevealInFiles }: LobbyProps) {
  const vaults = useVaultsStore((s) => s.vaults)
  const loadVaults = useVaultsStore((s) => s.loadVaults)
  const removeVault = useVaultsStore((s) => s.removeVault)
  const openRoute = useRouteStore((s) => s.open)

  useEffect(() => {
    loadVaults()
  }, [loadVaults])

  // Subscribe to the Lobby watcher so external edits to
  // `~/.codebus/app-state.json` (e.g. another instance, or a future
  // CLI hook that registers vaults) refresh the Lobby card list
  // without requiring the user to re-mount. Cleanup unsubscribes on
  // unmount per spec `Lobby Subscribes To Vault List Watcher`.
  useEffect(
    () => useWatcherEvent("vault-list-changed", () => {
      void loadVaults()
    }),
    [loadVaults],
  )

  const empty = vaults.length === 0

  return (
    <main
      data-testid="lobby"
      data-state={empty ? "empty" : "populated"}
      className="flex h-full w-full flex-col items-center"
    >
      <Topbar empty={empty} onNewVault={onNewVault} />
      <div className="flex flex-1 w-full flex-col items-center px-6 pt-12 pb-8">
        {empty ? (
          <EmptyState onBoard={onNewVault} />
        ) : (
          <PopulatedList
            vaults={[...vaults].sort(byLastOpenedDesc)}
            onOpen={openRoute}
            onRemove={(v) => void removeVault(v.path)}
            onRevealInFiles={onRevealInFiles}
          />
        )}
      </div>
    </main>
  )
}

function Topbar({ empty, onNewVault }: { empty: boolean; onNewVault: () => void }) {
  const t = useT()
  return (
    <header
      data-tauri-drag-region
      className="flex h-11 w-full items-center justify-between border-b border-border px-4"
    >
      <div className="flex items-center gap-2 text-sm font-semibold">
        <span aria-hidden="true">🚌</span>
        {t("common.appName")}
      </div>
      <div className="flex items-center gap-2">
        {!empty && (
          <Button
            variant="primary"
            onClick={onNewVault}
            data-testid="new-vault-button"
            aria-keyshortcuts="Mod+N"
          >
            {t("lobby.topbar.newVaultButton")}
            <kbd className="ml-2 hidden font-mono text-micro text-accent-fg/70 sm:inline">
              {t("lobby.topbar.newVaultShortcutHint")}
            </kbd>
          </Button>
        )}
        {/* leave room for the fixed WindowControls (3 × 46px = 138px). */}
        <div className="w-[140px]" aria-hidden="true" />
      </div>
    </header>
  )
}

interface PopulatedListProps {
  vaults: VaultEntry[]
  onOpen: (v: VaultEntry) => void
  onRemove: (v: VaultEntry) => void
  onRevealInFiles: (v: VaultEntry) => void
}

function PopulatedList({ vaults, onOpen, onRemove, onRevealInFiles }: PopulatedListProps) {
  const t = useT()
  return (
    <div className="flex w-full max-w-[640px] flex-col gap-3">
      <SectionLabel count={vaults.length} className="w-full">
        {t("lobby.populated.sectionLabel")}
      </SectionLabel>
      <div className="flex flex-col gap-2">
        {vaults.map((v) => (
          <VaultCard
            key={v.path}
            vault={v}
            onOpen={onOpen}
            onRemove={onRemove}
            onRevealInFiles={onRevealInFiles}
          />
        ))}
      </div>
      <p className="mt-2 border-t border-dashed border-border pt-2 text-meta text-fg-tertiary text-center">
        {t("lobby.populated.dragTip")}
      </p>
    </div>
  )
}

function byLastOpenedDesc(a: VaultEntry, b: VaultEntry): number {
  return b.last_opened.localeCompare(a.last_opened)
}
