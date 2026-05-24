// Headless-Chromium screenshot driver for codebus-app's vite dev server.
//
// Connects to local Chromium via CDP, injects a stub
// `window.__TAURI_INTERNALS__` so IPC-backed views render with seed
// data, navigates to a small library of states, and writes PNGs to
// /tmp/codebus-shots/.

import { chromium } from "playwright-core"
import { mkdirSync } from "node:fs"

const OUT_DIR = "/tmp/codebus-shots"
mkdirSync(OUT_DIR, { recursive: true })

const VITE_URL = "http://127.0.0.1:1420"

// ---- IPC mock library --------------------------------------------------

const VAULTS_POPULATED = [
  {
    path: "/Users/harry/code/codebus",
    display_name: "codebus",
    last_opened: new Date(Date.now() - 1000 * 60 * 60 * 2).toISOString(),
    is_missing: false,
  },
  {
    path: "/Users/harry/code/next-auth",
    display_name: "next-auth",
    last_opened: new Date(Date.now() - 1000 * 60 * 60 * 24).toISOString(),
    is_missing: false,
  },
  {
    path: "/Users/harry/code/zustand",
    display_name: "zustand",
    last_opened: new Date(Date.now() - 1000 * 60 * 60 * 24 * 3).toISOString(),
    is_missing: false,
  },
]

const DEFAULT_CONFIG = {
  agent: {
    active_provider: "claude_code",
    providers: {
      claude: {
        active: "system",
        system: {
          goal: { model: "opus-4-7", effort: "high" },
          query: { model: "sonnet-4-6", effort: "medium" },
          fix: { model: "sonnet-4-6", effort: "medium" },
          verify: { model: "opus-4-6", effort: "high" },
        },
        azure: null,
      },
    },
  },
  pii: { scanner: "regex_basic" },
  log: { dir: "~/.codebus/logs/" },
  quiz: { default_length: 5 },
  app: { quiz: { pass_threshold: 80, default_length: 5 } },
}

const RUNS_SAMPLE = [
  {
    run_id: "run-001",
    kind: "goal",
    title: "搞懂 auth 模組怎麼運作",
    state: "running",
    started_at: new Date(Date.now() - 23_000).toISOString(),
    finished_at: null,
    tokens: 8200,
  },
  {
    run_id: "run-002",
    kind: "goal",
    title: "弄清楚 route store 的狀態流",
    state: "done",
    started_at: new Date(Date.now() - 14 * 60_000).toISOString(),
    finished_at: new Date(Date.now() - 13 * 60_000).toISOString(),
    tokens: 14300,
  },
  {
    run_id: "run-003",
    kind: "goal",
    title: "找出 vault watcher 怎麼 debounce",
    state: "done",
    started_at: new Date(Date.now() - 60 * 60_000).toISOString(),
    finished_at: new Date(Date.now() - 58 * 60_000).toISOString(),
    tokens: 9100,
  },
  {
    run_id: "run-004",
    kind: "goal",
    title: "看看 quiz parse 怎麼處理 markdown",
    state: "failed",
    started_at: new Date(Date.now() - 3 * 60 * 60_000).toISOString(),
    finished_at: new Date(Date.now() - 3 * 60 * 60_000 + 40_000).toISOString(),
    tokens: 2400,
  },
]

const WIKI_PAGES = [
  { path: "auth/middleware.md", last_modified: new Date().toISOString(), bytes: 4096 },
  { path: "auth/session.md", last_modified: new Date().toISOString(), bytes: 2048 },
  { path: "auth/tokens.md", last_modified: new Date().toISOString(), bytes: 6100 },
  { path: "route/store.md", last_modified: new Date().toISOString(), bytes: 1800 },
  { path: "quiz/parse.md", last_modified: new Date().toISOString(), bytes: 3200 },
]

function buildInitScript({ vaults }) {
  // Stringified runtime: runs in the page before our app boots.
  return `
    (() => {
      const vaults = ${JSON.stringify(vaults)};
      const config = ${JSON.stringify(DEFAULT_CONFIG)};
      const runs = ${JSON.stringify(RUNS_SAMPLE)};
      const wikiPages = ${JSON.stringify(WIKI_PAGES)};

      const handlers = {
        list_vaults: () => vaults,
        add_vault: ({ path, options }) => ({
          path, display_name: path.split("/").pop() || path,
          last_opened: new Date().toISOString(), is_missing: false,
        }),
        remove_vault: () => undefined,
        load_global_config: () => config,
        save_global_config: () => undefined,
        get_endpoint_key: () => ({ kind: "unset" }),
        set_endpoint_key: () => undefined,
        delete_endpoint_key: () => undefined,
        check_cli_installed: () => ({ installed: true, version: "1.0.123" }),
        list_runs: () => runs,
        get_run_detail: ({ runId }) => ({
          run_id: runId, kind: "goal", state: "done",
          title: "弄清楚 route store 的狀態流",
          started_at: new Date(Date.now() - 14 * 60000).toISOString(),
          finished_at: new Date(Date.now() - 13 * 60000).toISOString(),
          tokens: 14300, events: [],
        }),
        list_wiki_pages: () => wikiPages,
        read_wiki_page: () => "# Sample\\n\\nThis page is loaded from the stub.",
        get_obsidian_vault_id: () => null,
        list_quiz_attempts: () => [],
        read_quiz_attempt: () => "",
        read_quiz_events: () => [],
        read_quiz_progress: () => ({ correct: 0, total: 0 }),
        write_quiz_progress: () => undefined,
      };

      const noop = () => undefined;
      const fakeListen = () => Promise.resolve(() => undefined);

      window.__TAURI_INTERNALS__ = {
        invoke: (cmd, args) => {
          const fn = handlers[cmd];
          if (!fn) {
            console.warn("[stub] no handler for", cmd, args);
            return Promise.resolve(null);
          }
          try {
            return Promise.resolve(fn(args ?? {}));
          } catch (e) {
            return Promise.reject(e);
          }
        },
        transformCallback: (cb) => {
          const id = Math.floor(Math.random() * 1e9);
          window["_" + id] = cb;
          return id;
        },
        unregisterCallback: noop,
        convertFileSrc: (p) => p,
        // event API stub used by listen()
        ipc: (cmd) => Promise.resolve(),
      };

      // Tauri v2 event listen() uses internal channel; provide a noop
      window.__TAURI_EVENT_PLUGIN_INTERNALS__ = {
        unregisterListener: noop,
      };
    })();
  `
}

async function shoot({
  name,
  url = VITE_URL,
  viewport = { width: 1280, height: 800 },
  vaults = VAULTS_POPULATED,
  prep,
  fullPage = false,
}) {
  const browser = await chromium.launch({
    executablePath: "/opt/pw-browsers/chromium-1194/chrome-linux/chrome",
    headless: true,
    args: ["--no-sandbox", "--disable-dev-shm-usage", "--remote-debugging-port=0"],
  })
  try {
    const ctx = await browser.newContext({ viewport, deviceScaleFactor: 2 })
    await ctx.addInitScript(buildInitScript({ vaults }))
    const page = await ctx.newPage()
    page.on("pageerror", (e) => console.error("[pageerror]", name, e.message))
    page.on("console", (m) => {
      if (m.type() === "error") console.warn("[console.error]", name, m.text())
    })
    await page.goto(url, { waitUntil: "networkidle" })
    // small settle delay for any post-mount IPC fetches
    await page.waitForTimeout(500)
    if (prep) await prep(page)
    await page.waitForTimeout(200)
    const path = `${OUT_DIR}/${name}.png`
    await page.screenshot({ path, fullPage })
    console.log("wrote", path)
  } finally {
    await browser.close()
  }
}

// ---- the shoot list ----------------------------------------------------

const SHOTS = [
  { name: "01-lobby-empty", vaults: [] },
  { name: "02-lobby-populated", vaults: VAULTS_POPULATED },
  {
    name: "03-settings-modal",
    vaults: VAULTS_POPULATED,
    prep: async (page) => {
      // BottomStrip has a Settings button — look for testid or aria
      const candidates = [
        '[data-testid="settings-button"]',
        'button[aria-label*="ettings"]',
        'button:has-text("Settings")',
      ]
      for (const sel of candidates) {
        const el = await page.$(sel)
        if (el) {
          await el.click()
          return
        }
      }
      // fallback: click the gear icon in the bottom strip — last button
      const buttons = await page.$$("footer button, [data-testid='bottom-strip'] button")
      if (buttons.length) await buttons[0].click()
    },
  },
  {
    name: "04-workspace-goals",
    vaults: VAULTS_POPULATED,
    prep: async (page) => {
      // Click the first vault card to enter workspace
      const card = await page.$('[data-testid="vault-card"]')
      if (card) await card.click()
      else {
        const first = await page.$("main button, main a, main [role=button]")
        if (first) await first.click()
      }
      await page.waitForTimeout(600)
    },
  },
  {
    name: "05-workspace-wiki",
    vaults: VAULTS_POPULATED,
    prep: async (page) => {
      const card = await page.$('[data-testid="vault-card"]')
      if (card) await card.click()
      await page.waitForTimeout(400)
      // click Wiki tab
      const wikiTab = await page.$('button:has-text("Wiki"), [data-testid*="wiki"]')
      if (wikiTab) await wikiTab.click()
      await page.waitForTimeout(400)
    },
  },
  {
    name: "06-workspace-quiz",
    vaults: VAULTS_POPULATED,
    prep: async (page) => {
      const card = await page.$('[data-testid="vault-card"]')
      if (card) await card.click()
      await page.waitForTimeout(400)
      const quizTab = await page.$('button:has-text("Quiz"), [data-testid*="quiz"]')
      if (quizTab) await quizTab.click()
      await page.waitForTimeout(400)
    },
  },
  {
    name: "07-primitive-sandbox",
    url: `${VITE_URL}/?sandbox=1`,
    fullPage: true,
    vaults: [],
  },
]

async function main() {
  const filter = process.argv[2]
  const list = filter ? SHOTS.filter((s) => s.name.includes(filter)) : SHOTS
  for (const shot of list) {
    try {
      await shoot(shot)
    } catch (e) {
      console.error("FAILED", shot.name, e.message)
    }
  }
}

main().catch((e) => {
  console.error(e)
  process.exit(1)
})
