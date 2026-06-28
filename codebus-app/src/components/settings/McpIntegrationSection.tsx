import { useEffect, useState } from "react"

import {
  type McpClientStatus,
  mcpClientInstall,
  mcpClientRemove,
  mcpClientStatus,
} from "@/lib/ipc"
import { PROVIDERS, type ProviderId } from "@/lib/providers"
import { useT } from "@/i18n/useT"

/**
 * Settings · MCP integration. Renders ONE INDEPENDENT row per supported client
 * (claude, codex) — NOT a single control that follows the active provider
 * (spec `mcp-client-install`: "independent control for EACH supported client").
 * Each row probes its own client and offers a connect/disconnect toggle that
 * shells out to that client's native CLI via the backend; an absent client
 * disables only its own row.
 */
export function McpIntegrationSection() {
  const t = useT()
  return (
    <div
      data-testid="mcp-integration-section"
      className="col-span-2 flex flex-col gap-2 rounded border border-border bg-bg-secondary/40 p-3"
    >
      <div className="text-fg-secondary text-meta">{t("settings.mcp.label")}</div>
      <div className="text-meta text-fg-tertiary">
        {t("settings.mcp.description")}
      </div>
      {Object.values(PROVIDERS).map((p) => (
        <McpClientRow
          key={p.id}
          providerId={p.id}
          cliBinaryId={p.cliBinaryId}
          displayName={p.displayName}
        />
      ))}
    </div>
  )
}

type RowState = { kind: "loading" } | { kind: "ready"; status: McpClientStatus }

function McpClientRow({
  providerId,
  cliBinaryId,
  displayName,
}: {
  providerId: ProviderId
  cliBinaryId: string
  displayName: string
}) {
  const t = useT()
  const [state, setState] = useState<RowState>({ kind: "loading" })
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState<string | null>(null)

  // Probe THIS client's registration state on mount. Each row is independent —
  // a failure or missing client here never affects the other row.
  useEffect(() => {
    let live = true
    setState({ kind: "loading" })
    mcpClientStatus(cliBinaryId as never)
      .then((status) => live && setState({ kind: "ready", status }))
      .catch(
        () =>
          live && setState({ kind: "ready", status: { kind: "client_missing" } }),
      )
    return () => {
      live = false
    }
  }, [cliBinaryId])

  const status = state.kind === "ready" ? state.status : null
  const missing = status?.kind === "client_missing"
  const connected = status?.kind === "installed"
  const disabled = busy || state.kind === "loading" || missing

  async function toggle() {
    setBusy(true)
    setError(null)
    try {
      if (connected) {
        await mcpClientRemove(cliBinaryId as never)
      } else {
        await mcpClientInstall(cliBinaryId as never)
      }
      const next = await mcpClientStatus(cliBinaryId as never)
      setState({ kind: "ready", status: next })
    } catch (e) {
      setError((e as { message?: string })?.message ?? String(e))
    } finally {
      setBusy(false)
    }
  }

  const statusText =
    state.kind === "loading"
      ? t("settings.mcp.status.checking")
      : missing
        ? t("settings.mcp.status.missing", { name: displayName })
        : connected
          ? t("settings.mcp.status.connected")
          : t("settings.mcp.status.notConnected")

  return (
    <div
      data-testid={`mcp-row-${providerId}`}
      className="flex flex-wrap items-center justify-between gap-2"
    >
      <label className="flex items-center gap-2 text-xs">
        <input
          type="checkbox"
          data-testid={`mcp-toggle-${providerId}`}
          checked={connected}
          disabled={disabled}
          onChange={toggle}
        />
        <span>{displayName}</span>
      </label>
      <span
        data-testid={`mcp-status-${providerId}`}
        className="text-meta text-fg-tertiary"
      >
        {statusText}
      </span>
      {error && (
        <span
          data-testid={`mcp-error-${providerId}`}
          className="text-meta text-error"
        >
          {t("settings.mcp.error", { name: displayName, message: error })}
        </span>
      )}
    </div>
  )
}
