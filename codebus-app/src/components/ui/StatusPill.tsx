import type { ReactNode } from "react"

import { useT } from "@/i18n/useT"

export type StatusPillStatus = "done" | "interrupted" | "failed" | "running"
export type StatusPillVariant = "dot" | "pill"

export interface StatusPillProps {
  status: StatusPillStatus
  variant: StatusPillVariant
  caret?: ReactNode
  className?: string
}

const STATUS_BG: Record<StatusPillStatus, string> = {
  done: "bg-status-done",
  interrupted: "bg-status-interrupted",
  failed: "bg-status-failed",
  running: "bg-status-running",
}

const STATUS_BORDER: Record<StatusPillStatus, string> = {
  done: "border-status-done/30",
  interrupted: "border-status-interrupted/35",
  failed: "border-status-failed/30",
  running: "border-status-running/35",
}

const STATUS_TEXT: Record<StatusPillStatus, string> = {
  done: "text-status-done",
  interrupted: "text-status-interrupted",
  failed: "text-status-failed",
  running: "text-status-running",
}

const STATUS_KEY = {
  done: "workspace.status.done",
  interrupted: "workspace.status.interrupted",
  failed: "workspace.status.failed",
  running: "workspace.status.running",
} as const

export function StatusPill({
  status,
  variant,
  caret,
  className,
}: StatusPillProps) {
  const t = useT()
  const invalidCombo = variant === "dot" && status === "running"

  if (invalidCombo && !import.meta.env.PROD) {
    console.warn(
      "StatusPill: 'dot' variant does not accept status=\"running\" (running uses pulse ring which would overflow a 7px dot). Use variant=\"pill\" for running state.",
    )
  }

  if (variant === "dot") {
    const dotClasses = [
      "inline-block",
      "w-[7px]",
      "h-[7px]",
      "rounded-full",
      STATUS_BG[status],
    ]
    if (className) dotClasses.push(className)
    return (
      <span
        className={dotClasses.join(" ")}
        aria-label={t(STATUS_KEY[status])}
      />
    )
  }

  const pillClasses = [
    "status-pill",
    "inline-flex",
    "items-center",
    "gap-[6px]",
    "px-[9px]",
    "py-[3px]",
    "rounded-[3px]",
    "border",
    "text-[12px]",
    "font-medium",
    STATUS_BORDER[status],
    STATUS_TEXT[status],
  ]
  if (className) pillClasses.push(className)

  const dotClasses = [
    "status-pill__dot",
    "inline-block",
    "w-[7px]",
    "h-[7px]",
    "rounded-full",
    STATUS_BG[status],
  ]
  if (status === "running") {
    dotClasses.push("status-pill__dot--running-ring")
    dotClasses.push("status-pill__dot--running-animated")
  }

  const showCaret = status === "running" && caret !== undefined && caret !== null

  return (
    <span className={pillClasses.join(" ")}>
      <span className={dotClasses.join(" ")} />
      <span>{t(STATUS_KEY[status])}</span>
      {showCaret ? (
        <span className="status-pill__caret">{caret}</span>
      ) : null}
    </span>
  )
}
