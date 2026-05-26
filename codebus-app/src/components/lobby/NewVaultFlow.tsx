import { useState } from "react"

import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogFooter,
} from "@/components/ui/dialog"
import type { AddVaultMode } from "@/lib/ipc"
import { useT } from "@/i18n/useT"

export interface DetectionDecision {
  mode: Exclude<AddVaultMode, "detect">
}

interface DetectionDialogProps {
  open: boolean
  path: string
  onCancel: () => void
  onDecide: (decision: DetectionDecision) => void
}

/**
 * `+ New Vault` detection dialog presented after the app detects a vault
 * folder already contains `.codebus/`. Spec: "Just bind it to Lobby
 * (recommended)" (default) plus "Re-initialize (destructive)" gated by a
 * typed `delete` confirmation.
 */
export function DetectionDialog({
  open,
  path,
  onCancel,
  onDecide,
}: DetectionDialogProps) {
  const t = useT()
  const [choice, setChoice] = useState<"just_bind" | "re_init">("just_bind")
  const [typed, setTyped] = useState("")
  const canReInit = typed.trim() === "delete"

  function close() {
    setChoice("just_bind")
    setTyped("")
    onCancel()
  }

  return (
    <Dialog open={open} onOpenChange={(o) => !o && close()}>
      <DialogContent data-testid="detection-dialog">
        <DialogHeader>
          <DialogTitle>{t("detection.title")}</DialogTitle>
        </DialogHeader>
        <div className="space-y-4 p-4">
          <p className="font-mono text-meta text-fg-tertiary truncate">
            {path}
          </p>
          <label className="flex items-start gap-3 cursor-pointer">
            <input
              type="radio"
              name="detection"
              value="just_bind"
              checked={choice === "just_bind"}
              onChange={() => setChoice("just_bind")}
              className="mt-1"
              data-testid="just-bind-radio"
            />
            <div>
              <div className="text-sm font-medium">
                {t("detection.justBind.label")}
              </div>
              <div className="text-xs text-fg-secondary">
                {t("detection.justBind.help")}
              </div>
            </div>
          </label>
          <label className="flex items-start gap-3 cursor-pointer">
            <input
              type="radio"
              name="detection"
              value="re_init"
              checked={choice === "re_init"}
              onChange={() => setChoice("re_init")}
              className="mt-1"
              data-testid="re-init-radio"
            />
            <div>
              <div className="text-sm font-medium text-error">
                {t("detection.reInit.label")}
              </div>
              <div className="text-xs text-fg-secondary">
                {t("detection.reInit.help")}
              </div>
            </div>
          </label>
          {choice === "re_init" && (
            <div className="space-y-1">
              <label className="text-xs text-fg-secondary">
                {t("detection.confirmInput.label", { keyword: "delete" })
                  .split("delete")
                  .map((chunk, i, arr) => (
                    <span key={i}>
                      {chunk}
                      {i < arr.length - 1 && (
                        <span className="font-mono text-error">delete</span>
                      )}
                    </span>
                  ))}
              </label>
              <Input
                value={typed}
                onChange={(e) => setTyped(e.target.value)}
                data-testid="re-init-confirm-input"
                aria-label={t("detection.confirmInput.aria")}
              />
            </div>
          )}
        </div>
        <DialogFooter>
          <Button variant="ghost" onClick={close} data-testid="detection-cancel">
            {t("common.cancel")}
          </Button>
          <Button
            variant="primary"
            data-testid="detection-confirm"
            disabled={choice === "re_init" && !canReInit}
            onClick={() => {
              if (choice === "just_bind") {
                onDecide({ mode: "just_bind" })
              } else if (canReInit) {
                onDecide({ mode: "re_init" })
              }
              setTyped("")
              setChoice("just_bind")
            }}
          >
            {choice === "re_init"
              ? t("detection.confirm.reInit")
              : t("detection.confirm.justBind")}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  )
}
