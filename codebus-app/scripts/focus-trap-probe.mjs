import { chromium } from "playwright-core"
const CDP_URL = "http://127.0.0.1:9222"
const b = await chromium.connectOverCDP(CDP_URL)
const ctx = b.contexts()[0]
const page = ctx.pages()[0]
async function snap(step) {
  return await page.evaluate((s) => {
    const el = document.activeElement
    function inDialog(e) { let p = e; while (p) { if (p.getAttribute && p.getAttribute('role') === 'dialog') return true; p = p.parentElement; } return false }
    const id = el?.getAttribute && el.getAttribute('data-testid')
    return { step: s, tag: el?.tagName, testid: id || null, role: el?.getAttribute?.('role') || null, inDialog: inDialog(el) }
  }, step)
}
const path = []
path.push(await snap('open'))
for (let i = 1; i <= 5; i++) { await page.keyboard.press('Tab'); path.push(await snap('Tab ' + i)) }
for (let i = 1; i <= 5; i++) { await page.keyboard.press('Shift+Tab'); path.push(await snap('Shift+Tab ' + i)) }
console.log(JSON.stringify(path, null, 2))
await b.close()
