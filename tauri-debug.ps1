# Launch codebus Tauri dev with the WebView2 remote-debugging port open.
#
# This lets Claude attach to the REAL WebView2 frontend over CDP (IPC works,
# real data renders) via: node codebus-app/scripts/cdp.mjs <shot|text|eval|click|html>
#
# Usage (from anywhere):  .\tauri-debug.ps1
# Then ask Claude to verify/screenshot/click the Tauri UI.

Set-Location $PSScriptRoot
$env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS = "--remote-debugging-port=9222"
cargo tauri dev
