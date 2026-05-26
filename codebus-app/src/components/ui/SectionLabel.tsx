import type { ReactNode } from "react"

export interface SectionLabelProps {
  variant?: "default" | "caps"
  count?: number | string
  className?: string
  children: ReactNode
}

export function SectionLabel({
  variant = "default",
  count,
  className,
  children,
}: SectionLabelProps) {
  const classes = ["section-label"]
  if (variant === "caps") classes.push("section-label--caps")
  if (className) classes.push(className)

  return (
    <span className={classes.join(" ")}>
      {children}
      {count !== undefined && (
        <span className="section-label__count">{count}</span>
      )}
    </span>
  )
}
