import { useEffect, useState } from "react"
import { X, AlertCircle } from "lucide-react"

import { useT } from "@/i18n/useT"
import { useVaultsStore } from "@/store/vaults"

const AUTO_DISMISS_MS = 5000

/**
 * Minimal error toast surfacing `vaultsStore.error`. Auto-dismisses after
 * 5s; user can also click ✕ to clear immediately. Reads the locale-aware
 * message at render time so the toast respects the active locale.
 * z-55: above modal overlay (z-50), below WindowControls (z-60).
 */
export function Toast() {
  const t = useT()
  const error = useVaultsStore((s) => s.error)
  const clearError = useVaultsStore((s) => s.clearError)
  const [visible, setVisible] = useState(false)

  useEffect(() => {
    if (!error) {
      setVisible(false)
      return
    }
    setVisible(true)
    const id = setTimeout(() => {
      setVisible(false)
      clearError()
    }, AUTO_DISMISS_MS)
    return () => clearTimeout(id)
  }, [error, clearError])

  if (!visible || !error) return null

  return (
    <div
      role="alert"
      data-testid="toast-error"
      className="fixed top-14 right-4 z-[55] flex max-w-[420px] items-start gap-2 rounded-md border border-error/40 bg-bg-raised px-3 py-2 text-meta text-fg shadow-lg"
    >
      <AlertCircle className="mt-0.5 h-3.5 w-3.5 shrink-0 text-error" />
      <div className="flex-1 break-words">{t(error.key, error.vars)}</div>
      <button
        aria-label={t("common.dismiss")}
        onClick={() => {
          setVisible(false)
          clearError()
        }}
        className="ml-1 rounded-sm text-fg-tertiary hover:text-fg focus:outline-none focus:ring-2 focus:ring-accent-ring"
      >
        <X className="h-3 w-3" />
      </button>
    </div>
  )
}
