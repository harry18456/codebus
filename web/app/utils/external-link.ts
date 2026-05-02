// Open an external URL via Tauri's opener plugin (system default browser).
//
// Tauri 2 webview blocks `<a target="_blank">` navigation to external URLs
// by default for security; the only sanctioned escape hatch is the opener
// plugin's IPC. In non-Tauri (vitest / SSR) environments we fall back to
// `window.open` so component tests don't need a Tauri shim.

export async function openExternal(url: string | null | undefined): Promise<void> {
  if (!url) return
  // Tauri injects `__TAURI_INTERNALS__` into the global at boot. SSR /
  // vitest environments leave it undefined, so we skip the plugin import
  // and use the browser fallback.
  const isTauri =
    typeof window !== 'undefined' &&
    Object.prototype.hasOwnProperty.call(window, '__TAURI_INTERNALS__')
  if (isTauri) {
    const { openUrl } = await import('@tauri-apps/plugin-opener')
    await openUrl(url)
    return
  }
  if (typeof window !== 'undefined') {
    window.open(url, '_blank', 'noopener,noreferrer')
  }
}
