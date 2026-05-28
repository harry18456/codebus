import { useCallback, useEffect, useState } from "react"

import { BottomStrip } from "@/components/BottomStrip"
import { DropTargetOverlay } from "@/components/DropTargetOverlay"
import { LoadingOverlay } from "@/components/LoadingOverlay"
import { Toast } from "@/components/Toast"
import { WindowControls } from "@/components/WindowControls"
import { Lobby } from "@/components/lobby/Lobby"
import { DetectionDialog } from "@/components/lobby/NewVaultFlow"
import { SettingsModal } from "@/components/settings/SettingsModal"
import { Workspace } from "@/components/workspace/Workspace"
import { useNewVaultShortcut } from "@/hooks/useNewVaultShortcut"
import { useLobbyDragDrop } from "@/hooks/useLobbyDragDrop"
import {
  type AddVaultMode,
  type VaultEntry,
  isAppError,
} from "@/lib/ipc"
import { useRouteStore } from "@/store/route"
import { useSettingsStore } from "@/store/settings"
import { useVaultsStore } from "@/store/vaults"
import { PrimitiveShowcase } from "@/sandbox/PrimitiveShowcase"

const APP_VERSION = "v3.0.0"
const PII_PATTERN_COUNT = 14

export function App() {
  if (
    typeof window !== "undefined" &&
    new URLSearchParams(window.location.search).get("sandbox") === "1"
  ) {
    return <PrimitiveShowcase />
  }

  return <AppShell />
}

function AppShell() {
  const route = useRouteStore((s) => s.route)
  const openRoute = useRouteStore((s) => s.open)
  const addVault = useVaultsStore((s) => s.addVault)

  const [settingsOpen, setSettingsOpen] = useState(false)
  const [pendingDetection, setPendingDetection] = useState<string | null>(null)

  // Preload settings at app start so `app.locale_override` is honored from
  // the very first render — without this, the Lobby renders in the system
  // locale until something else (Workspace mount, Settings modal open) loads
  // the store, which would visibly flash the wrong language for users who
  // chose an override. Backs spec scenario "Locale override survives
  // application restart".
  const settingsLoad = useSettingsStore((s) => s.load)
  useEffect(() => {
    void settingsLoad().catch(() => {})
  }, [settingsLoad])

  const triggerNewVault = useCallback(async () => {
    try {
      const mod = await import("@tauri-apps/plugin-dialog")
      const picked = await mod.open({ directory: true, multiple: false })
      if (typeof picked === "string") {
        await handlePath(picked)
      }
    } catch (err) {
      console.error("file picker failed", err)
    }
  }, [])

  const handlePath = useCallback(
    async (path: string) => {
      try {
        const entry = await addVault(path, "detect")
        openRoute(entry)
      } catch (err) {
        if (
          isAppError(err) &&
          err.kind === "invalid" &&
          err.field === "mode"
        ) {
          setPendingDetection(path)
        } else {
          console.error("add vault failed", err)
        }
      }
    },
    [addVault, openRoute],
  )

  useNewVaultShortcut(triggerNewVault)
  const { isDragOver } = useLobbyDragDrop(handlePath)

  async function handleDetection(mode: AddVaultMode) {
    if (!pendingDetection) return
    const path = pendingDetection
    setPendingDetection(null)
    try {
      const entry = await addVault(path, mode)
      openRoute(entry)
    } catch (err) {
      console.error("add vault (post-detection) failed", err)
    }
  }

  return (
    <div className="flex h-screen w-screen flex-col bg-bg text-fg">
      <div className="flex flex-1 overflow-hidden">
        {route.kind === "lobby" ? (
          <Lobby
            onNewVault={triggerNewVault}
            onRevealInFiles={(v) => void revealInFiles(v)}
          />
        ) : (
          <Workspace
            vault={route.vault}
            onOpenSettings={() => setSettingsOpen(true)}
          />
        )}
      </div>
      {route.kind === "lobby" && (
        <BottomStrip
          version={APP_VERSION}
          onOpenSettings={() => setSettingsOpen(true)}
        />
      )}
      <WindowControls />
      <Toast />
      {isDragOver && <DropTargetOverlay />}
      <LoadingOverlay />

      <DetectionDialog
        open={pendingDetection !== null}
        path={pendingDetection ?? ""}
        onCancel={() => setPendingDetection(null)}
        onDecide={({ mode }) => void handleDetection(mode)}
      />
      <SettingsModal
        open={settingsOpen}
        onClose={() => setSettingsOpen(false)}
        piiPatternCount={PII_PATTERN_COUNT}
      />
    </div>
  )
}

async function revealInFiles(vault: VaultEntry) {
  try {
    const { revealItemInDir } = await import("@tauri-apps/plugin-opener")
    await revealItemInDir(vault.path)
  } catch (err) {
    console.error("reveal failed", err)
  }
}

