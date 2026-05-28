import { useEffect, useRef, useState } from "react"

import { Button } from "@/components/ui/button"
import {
  Dialog,
  DialogContent,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog"
import { useT } from "@/i18n/useT"
import { useGoalsStore } from "@/store/goals"

/** Best-effort string extraction from a Tauri-rejected IPC promise.
 *  Backend rejects with `AppError` serialized as
 *  `{ kind, field?, message }`; SDK occasionally wraps in `Error`. */
function extractAppErrorMessage(err: unknown): string {
  if (err && typeof err === "object" && "message" in err) {
    const m = (err as { message?: unknown }).message
    if (typeof m === "string") return m
  }
  if (err instanceof Error) return err.message
  return typeof err === "string" ? err : String(err)
}

interface NewGoalModalProps {
  open: boolean
  vaultPath: string
  /** Optional pre-fill (used by empty-state examples and Retry). */
  initialText?: string
  onClose: () => void
  /** Called after a successful spawn so callers can navigate. */
  onSpawned?: (runId: string) => void
}

/**
 * Spec: app-workspace § New Goal Modal Flow.
 *
 * - centered dialog, textarea placeholder "What should codebus document?"
 * - Cancel + Run buttons
 * - Run disabled when (text whitespace-only) OR (activeRun != null)
 * - When disabled by active run, render the hint
 *   "Wait for current run to finish or cancel it before starting a new one."
 * - Esc / overlay-click / Cancel / successful Run all close the modal
 */
export function NewGoalModal({
  open,
  vaultPath,
  initialText = "",
  onClose,
  onSpawned,
}: NewGoalModalProps) {
  const t = useT()
  const activeRun = useGoalsStore((s) => s.activeRun)
  const spawnGoal = useGoalsStore((s) => s.spawnGoal)
  const [text, setText] = useState(initialText)
  const [submitting, setSubmitting] = useState(false)
  // vault-switch-goal-regression Decision 7: spawnGoal IPC reject SHALL
  // surface inline; previously the catch swallowed the error and the
  // modal looked unresponsive ("Run 沒反應").
  const [spawnError, setSpawnError] = useState<string | null>(null)
  const textareaRef = useRef<HTMLTextAreaElement | null>(null)

  // Reset textarea content when the modal re-opens (so a previous
  // submission's trailing text does not linger).
  useEffect(() => {
    if (open) {
      setText(initialText)
      setSpawnError(null)
      // Focus the textarea on next paint; Radix focus trap handles
      // subsequent Tab navigation.
      requestAnimationFrame(() => textareaRef.current?.focus())
    }
  }, [open, initialText])

  const trimmed = text.trim()
  const blockedByActiveRun = activeRun !== null
  const runDisabled =
    trimmed.length === 0 || blockedByActiveRun || submitting

  async function handleRun() {
    if (runDisabled) return
    setSubmitting(true)
    setSpawnError(null)
    try {
      const runId = await spawnGoal(vaultPath, trimmed)
      onSpawned?.(runId)
      onClose()
    } catch (err) {
      // Backend rejects with `AppError::Invalid { message: "another goal
      // run is already active" }` when active_runs still holds the prior
      // entry (e.g. user navigated away, came back, list_runs may not
      // yet have updated activeRun via the running-detection path). Map
      // it to a user-friendly hint; fall through to a generic message
      // for unknown errors so the modal never silently swallows again.
      const raw = extractAppErrorMessage(err)
      if (raw.includes("already active")) {
        setSpawnError(t("workspace.newGoalModal.errorAlreadyActive"))
      } else {
        setSpawnError(t("workspace.newGoalModal.errorSpawnFailed", { message: raw }))
      }
    } finally {
      setSubmitting(false)
    }
  }

  return (
    <Dialog open={open} onOpenChange={(o) => !o && onClose()}>
      <DialogContent
        data-testid="new-goal-modal"
        onKeyDown={(e) => {
          // Cmd/Ctrl+Enter shortcut (matches the existing settings modal
          // pattern). Esc is handled by Radix automatically.
          if ((e.metaKey || e.ctrlKey) && e.key === "Enter") {
            e.preventDefault()
            void handleRun()
          }
        }}
      >
        <DialogHeader>
          <DialogTitle>{t("workspace.newGoalModal.title")}</DialogTitle>
        </DialogHeader>
        <div className="flex flex-col gap-2 p-4">
          <textarea
            ref={textareaRef}
            data-testid="new-goal-textarea"
            placeholder={t("workspace.newGoalModal.placeholder")}
            value={text}
            onChange={(e) => setText(e.target.value)}
            rows={5}
            className="w-full resize-none rounded-md border border-border bg-bg p-2 text-sm focus:outline-none focus:ring-2 focus:ring-accent-ring"
          />
          {blockedByActiveRun && (
            <p
              data-testid="new-goal-blocked-hint"
              className="text-meta text-fg-tertiary"
            >
              {t("workspace.newGoalModal.blockedHint")}
            </p>
          )}
          {spawnError && (
            <p
              data-testid="new-goal-spawn-error"
              role="alert"
              className="text-meta text-status-error"
            >
              {spawnError}
            </p>
          )}
        </div>
        <DialogFooter>
          <Button
            variant="ghost"
            data-testid="new-goal-cancel"
            onClick={onClose}
          >
            {t("workspace.newGoalModal.cancel")}
          </Button>
          <Button
            data-testid="new-goal-run"
            disabled={runDisabled}
            onClick={() => void handleRun()}
          >
            {t("workspace.newGoalModal.run")}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  )
}
