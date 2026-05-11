import { useEffect } from "react"

import type { VaultEntry } from "@/lib/ipc"
import { useRouteStore } from "@/store/route"
import { useVaultsStore } from "@/store/vaults"
import { useT } from "@/i18n/useT"

interface WorkspaceStubProps {
  vault: VaultEntry
}

export function WorkspaceStub({ vault }: WorkspaceStubProps) {
  const t = useT()
  const back = useRouteStore((s) => s.back)
  const loadVaults = useVaultsStore((s) => s.loadVaults)

  function handleBack() {
    back()
    loadVaults()
  }

  // The drag-drop scope test relies on this component NOT registering a
  // listener; the `useLobbyDragDrop` hook reads the route from the store
  // and returns early when kind !== "lobby". We intentionally do NOT mount
  // it here.

  useEffect(() => {
    // Defensive: if a stale missing-marked vault is opened, the Lobby
    // already marked it; nothing for the stub to do beyond render.
  }, [])

  return (
    <main
      data-testid="workspace-stub"
      className="flex h-full w-full"
    >
      <aside
        data-testid="workspace-sidebar"
        className="flex w-[200px] flex-col gap-2 border-r border-border bg-bg-sunken p-4"
      >
        <button
          onClick={handleBack}
          data-testid="workspace-back"
          className="text-[12px] text-fg-tertiary hover:text-fg focus:outline-none focus:ring-2 focus:ring-accent-ring rounded-sm text-left"
        >
          {t("workspace.backToLobby")}
        </button>
        <div className="mt-2 border-t border-border pt-2">
          <div
            data-testid="workspace-vault-name"
            className="text-sm font-semibold"
          >
            {vault.display_name}
          </div>
          <div
            data-testid="workspace-vault-path"
            className="font-mono text-[11px] text-fg-tertiary truncate"
            title={vault.path}
          >
            {vault.path}
          </div>
        </div>
      </aside>
      <section
        data-testid="workspace-main"
        data-tauri-drag-region
        className="flex flex-1 items-center justify-center px-8 text-center"
      >
        <div className="flex flex-col items-center gap-3">
          <div className="text-[40px]" aria-hidden="true">
            🚏
          </div>
          <h1
            data-testid="workspace-coming-soon"
            className="text-[20px] font-semibold tracking-tight"
          >
            {t("workspace.coming.title")}
          </h1>
          <p className="text-[13px] text-fg-secondary">
            {t("workspace.coming.subtitle")}
          </p>
        </div>
      </section>
    </main>
  )
}
