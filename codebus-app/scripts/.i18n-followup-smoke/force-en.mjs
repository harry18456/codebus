// One-shot helper: install a navigator.language override into the running
// WebView2 page via Page.addScriptToEvaluateOnNewDocument, then reload so
// `useLocale` re-detects on the next render and resolves to `en`. Used by
// the i18n-sweep-phase-3a-followup en-locale CDP smoke.
import { chromium } from "playwright-core"

const CDP_URL = process.env.CDP_URL ?? "http://127.0.0.1:9222"

const browser = await chromium.connectOverCDP(CDP_URL)
try {
  const ctx = browser.contexts()[0]
  const page = ctx.pages()[0]
  if (!page) throw new Error("no page found")
  await page.addInitScript(() => {
    Object.defineProperty(navigator, "language", {
      get: () => "en-US",
      configurable: true,
    })
    Object.defineProperty(navigator, "languages", {
      get: () => ["en-US", "en"],
      configurable: true,
    })
  })
  await page.reload({ waitUntil: "load" })
  const lang = await page.evaluate(() => navigator.language)
  console.log("navigator.language now:", lang)
} finally {
  await browser.close()
}
