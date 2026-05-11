import { isAppError } from "@/lib/ipc"

import type { MessageKey } from "./messages"

/** Localized-message-ready error payload. Stores hold this instead of a
 * pre-formatted string so the Toast component can render with the active
 * locale at display time. */
export interface LocalizedError {
  key: MessageKey
  vars?: Record<string, string | number>
}

export function toLocalizedError(err: unknown): LocalizedError {
  if (isAppError(err)) {
    switch (err.kind) {
      case "vault_already_exists":
        return { key: "errors.vaultAlreadyExists", vars: { path: err.path } }
      case "vault_not_found":
        return { key: "errors.vaultNotFound", vars: { path: err.path } }
      case "invalid":
        return {
          key: "errors.invalid",
          vars: { field: err.field, message: err.message },
        }
      case "io":
        return { key: "errors.io", vars: { message: err.message } }
      case "config_parse":
        return {
          key: "errors.configParse",
          vars: { message: err.message },
        }
      case "internal":
        return { key: "errors.internal", vars: { message: err.message } }
    }
  }
  if (err && typeof err === "object" && "message" in err) {
    const message = String((err as { message: unknown }).message ?? err)
    return { key: "errors.internal", vars: { message } }
  }
  return { key: "errors.generic" }
}
