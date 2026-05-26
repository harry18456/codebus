import { useEffect, useRef, useState } from "react"

import { Button } from "@/components/ui/button"
import {
  Dialog,
  DialogContent,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog"
import { useGoalsStore } from "@/store/goals"

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
  const activeRun = useGoalsStore((s) => s.activeRun)
  const spawnGoal = useGoalsStore((s) => s.spawnGoal)
  const [text, setText] = useState(initialText)
  const [submitting, setSubmitting] = useState(false)
  const textareaRef = useRef<HTMLTextAreaElement | null>(null)

  // Reset textarea content when the modal re-opens (so a previous
  // submission's trailing text does not linger).
  useEffect(() => {
    if (open) {
      setText(initialText)
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
    try {
      const runId = await spawnGoal(vaultPath, trimmed)
      onSpawned?.(runId)
      onClose()
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
          <DialogTitle>New goal</DialogTitle>
        </DialogHeader>
        <div className="flex flex-col gap-2 p-4">
          <textarea
            ref={textareaRef}
            data-testid="new-goal-textarea"
            placeholder="What should codebus document?"
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
              Wait for current run to finish or cancel it before starting a new one.
            </p>
          )}
        </div>
        <DialogFooter>
          <Button
            variant="ghost"
            data-testid="new-goal-cancel"
            onClick={onClose}
          >
            Cancel
          </Button>
          <Button
            data-testid="new-goal-run"
            disabled={runDisabled}
            onClick={() => void handleRun()}
          >
            Run
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  )
}
