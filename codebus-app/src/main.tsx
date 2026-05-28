import React from "react"
import ReactDOM from "react-dom/client"
import "./styles/globals.css"
import { App } from "./App"
import { useVaultsStore } from "./store/vaults"

// ---- Dev-only test hooks for CDP smoke drivers ----
//
// These globals are intentionally non-shipping (gated on
// `import.meta.env.DEV`). They let the CDP smoke driver at
// `codebus-app/scripts/.loading-overlay-smoke/driver.mjs` bypass the OS
// folder picker (which CDP cannot drive) and force-state the vaults
// store to capture the LoadingOverlay's failure / fallback / slow-phase
// UI without needing to manufacture real `run_init` failures.
//
// Production builds (`import.meta.env.PROD`) do NOT define these
// globals. Anything that runs in production MUST NOT read them.
if (import.meta.env.DEV) {
  // Re-export the store so the smoke driver can call
  // `__codebus_vaults_store__.getState()` / `.setState()`.
  ;(window as unknown as Record<string, unknown>).__codebus_vaults_store__ =
    useVaultsStore
  // Direct addVault dispatch (bypasses the OS folder picker that
  // `triggerNewVault` in App.tsx invokes).
  ;(
    window as unknown as Record<string, unknown>
  ).__codebus_test_add_vault__ = (path: string, mode: "detect" | "just_bind" | "re_init") =>
    useVaultsStore.getState().addVault(path, mode)
}

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
)
