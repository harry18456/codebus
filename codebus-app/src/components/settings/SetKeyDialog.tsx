import { useEffect, useState } from "react"

import {
  Dialog,
  DialogContent,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog"
import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import { setEndpointKey } from "@/lib/ipc"

/**
 * Modal for entering the Azure API key. The key value is held only in
 * this component's `useState` and SHALL NOT propagate to any parent
 * store / context / DOM attribute. On `Confirm` the key flows to the
 * `set_endpoint_key` IPC and is dropped on unmount.
 */
export interface SetKeyDialogProps {
  open: boolean
  /** Keyring service to store the key under (provider-specific). */
  service: string
  onClose: () => void
  onSuccess: () => void
}

export function SetKeyDialog({ open, service, onClose, onSuccess }: SetKeyDialogProps) {
  const [key, setKey] = useState("")
  const [submitting, setSubmitting] = useState(false)
  const [error, setError] = useState<string | null>(null)

  // Reset state every time the dialog (re)opens.
  useEffect(() => {
    if (open) {
      setKey("")
      setSubmitting(false)
      setError(null)
    }
  }, [open])

  async function handleConfirm() {
    if (key.length === 0) {
      setError("API key cannot be empty")
      return
    }
    setSubmitting(true)
    setError(null)
    try {
      await setEndpointKey(service, key)
      onSuccess()
    } catch (err) {
      setError(formatError(err))
    } finally {
      setSubmitting(false)
    }
  }

  return (
    <Dialog open={open} onOpenChange={(o) => !o && onClose()}>
      <DialogContent data-testid="set-key-dialog">
        <DialogHeader>
          <DialogTitle>Set Azure API key</DialogTitle>
        </DialogHeader>
        <div className="flex flex-col gap-2 p-2">
          <label className="text-xs text-fg-secondary" htmlFor="set-key-input">
            Paste the API key — it will be stored in your OS keyring and never
            written to <code>~/.codebus/config.yaml</code>.
          </label>
          <Input
            id="set-key-input"
            data-testid="set-key-input"
            type="password"
            value={key}
            disabled={submitting}
            autoComplete="off"
            onChange={(e) => setKey(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter") void handleConfirm()
            }}
          />
          {error && (
            <span className="text-xs text-error" data-testid="set-key-error">
              {error}
            </span>
          )}
        </div>
        <DialogFooter>
          <Button
            type="button"
            variant="secondary"
            data-testid="set-key-cancel"
            onClick={onClose}
          >
            Cancel
          </Button>
          <Button
            type="button"
            data-testid="set-key-confirm"
            disabled={submitting}
            onClick={() => void handleConfirm()}
          >
            {submitting ? "Saving…" : "Confirm"}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  )
}

function formatError(err: unknown): string {
  if (typeof err === "object" && err && "message" in err) {
    const m = (err as { message?: unknown }).message
    if (typeof m === "string") return m
  }
  return String(err)
}
