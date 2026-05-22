// CDP bridge to the running codebus Tauri WebView2.
//
// Prereq: launch the app with the WebView2 debug port open, e.g. (PowerShell):
//   $env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS = "--remote-debugging-port=9222"
//   cargo tauri dev
//
// Usage:
//   node scripts/cdp.mjs shot [outPath]   capture screenshot (default: scripts/.cdp-shot.png)
//   node scripts/cdp.mjs text             dump visible innerText of <body>
//   node scripts/cdp.mjs eval "<js>"      evaluate JS in the page, print JSON result
//   node scripts/cdp.mjs click "<sel>"    click first element matching a CSS selector
//   node scripts/cdp.mjs html "<sel>"     print outerHTML of first match (default: body)
//
// Connects to the EXISTING WebView2 page — does not spawn a browser. Because it
// attaches to the real Tauri webview, window.__TAURI_INTERNALS__ is present and
// IPC-backed views render for real.

import { chromium } from "playwright-core"
import { fileURLToPath } from "node:url"
import { dirname, resolve } from "node:path"

// Use 127.0.0.1 (not "localhost"): Node may resolve localhost to IPv6 ::1,
// but the WebView2 debug server only listens on IPv4 → ECONNREFUSED ::1:9222.
const CDP_URL = process.env.CDP_URL ?? "http://127.0.0.1:9222"
const here = dirname(fileURLToPath(import.meta.url))

const [cmd, arg] = process.argv.slice(2)

async function getPage(browser) {
  const ctx = browser.contexts()[0]
  if (!ctx) throw new Error("no browser context — is the app running with the debug port open?")
  const page = ctx.pages()[0]
  if (!page) throw new Error("no page found in the WebView2 target")
  return page
}

async function main() {
  let browser
  try {
    browser = await chromium.connectOverCDP(CDP_URL)
  } catch (error) {
    console.error(`failed to connect to ${CDP_URL}: ${error.message}`)
    console.error("Is the app running with --remote-debugging-port=9222?")
    process.exit(2)
  }

  try {
    const page = await getPage(browser)

    switch (cmd) {
      case "shot": {
        const out = resolve(arg ?? resolve(here, ".cdp-shot.png"))
        await page.screenshot({ path: out, fullPage: false })
        console.log(out)
        break
      }
      case "text": {
        const text = await page.evaluate(() => document.body.innerText)
        console.log(text)
        break
      }
      case "eval": {
        if (!arg) throw new Error('eval needs a JS expression argument')
        const result = await page.evaluate((js) => {
          // eslint-disable-next-line no-eval
          return eval(js)
        }, arg)
        console.log(JSON.stringify(result, null, 2))
        break
      }
      case "click": {
        if (!arg) throw new Error("click needs a CSS selector argument")
        await page.click(arg, { timeout: 5000 })
        console.log(`clicked: ${arg}`)
        break
      }
      case "html": {
        const sel = arg ?? "body"
        const html = await page.evaluate((s) => {
          const el = document.querySelector(s)
          return el ? el.outerHTML : null
        }, sel)
        console.log(html ?? `no element matched: ${sel}`)
        break
      }
      default:
        console.error("unknown command. use: shot | text | eval | click | html")
        process.exit(1)
    }
  } finally {
    // Detach without closing the user's app window.
    await browser.close()
  }
}

main().catch((error) => {
  console.error(error.message)
  process.exit(1)
})
