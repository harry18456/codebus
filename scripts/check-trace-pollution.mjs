#!/usr/bin/env node
// scripts/check-trace-pollution.mjs
//
// Detects @trace pollution in OpenSpec spec files caused by `spectra archive`.
//
// Background (TOOL-1 in docs/internal/BACKLOG.md): every `spectra archive`
// flattens a requirement's @trace `code:` list into the whole dirty working
// tree — pulling in unrelated docs/backlog notes, lockfiles and version
// manifests — and cross-contaminates requirements. It also deletes a MODIFIED
// requirement's trace without regenerating it (leaving an empty block). Both
// otherwise require manual grep + restore after every single archive.
//
// This is the DETECTION (MVP) half of the fix: it scans the spec files, parses
// every `<!-- @trace ... -->` block, and reports blocks whose `code:`/`tests:`
// lists contain entries that have no business in a spec trace, plus blocks that
// lost their `code:` entirely. Restoration stays manual, but is now driven by a
// precise, line-numbered list instead of eyeballing thousands of lines.
//
// A spec trace `code:`/`tests:` entry should point at IMPLEMENTATION source or
// a test file. It should NEVER point at:
//   - docs/**            (documentation / dated backlog notes)
//   - lockfiles          (Cargo.lock, package-lock.json, pnpm-lock.yaml, ...)
//   - version manifests  (Cargo.toml, package.json, tauri.conf.json) [suspect]
// Those only appear because archive flattened the dirty tree into the trace.
//
// Usage:
//   node scripts/check-trace-pollution.mjs            # report, always exit 0
//   node scripts/check-trace-pollution.mjs --strict   # exit 1 if any pollution
//   node scripts/check-trace-pollution.mjs --dir PATH # scan a different root
//
// Pure node: builtins, no third-party deps — runs anywhere without install.

import { readFileSync, readdirSync, statSync } from "node:fs"
import { join, dirname, relative } from "node:path"
import { fileURLToPath } from "node:url"

const HERE = dirname(fileURLToPath(import.meta.url))
const REPO_ROOT = join(HERE, "..")

const argv = process.argv.slice(2)
const STRICT = argv.includes("--strict")
const dirFlagIdx = argv.indexOf("--dir")
const SCAN_DIR =
  dirFlagIdx >= 0 && argv[dirFlagIdx + 1]
    ? argv[dirFlagIdx + 1]
    : join(REPO_ROOT, "openspec", "specs")

// --- file discovery ---------------------------------------------------------

/** Recursively collect every `spec.md` under `root`. */
function findSpecFiles(root) {
  const out = []
  let entries
  try {
    entries = readdirSync(root, { withFileTypes: true })
  } catch {
    return out
  }
  for (const e of entries) {
    const full = join(root, e.name)
    if (e.isDirectory()) {
      out.push(...findSpecFiles(full))
    } else if (e.isFile() && e.name === "spec.md") {
      out.push(full)
    }
  }
  return out
}

// --- @trace parsing ---------------------------------------------------------

/**
 * Parse all `<!-- @trace ... -->` blocks out of a spec file's text.
 * Returns [{ startLine, source, updated, code: [], tests: [] }].
 */
function parseTraceBlocks(text) {
  const lines = text.split(/\r?\n/)
  const blocks = []
  let cur = null
  let section = null // "code" | "tests" | null

  for (let i = 0; i < lines.length; i++) {
    const line = lines[i]
    const trimmed = line.trim()

    if (trimmed === "<!-- @trace") {
      cur = { startLine: i + 1, source: "", updated: "", code: [], tests: [] }
      section = null
      continue
    }
    if (!cur) continue

    if (trimmed === "-->") {
      blocks.push(cur)
      cur = null
      section = null
      continue
    }

    const header = line.match(/^(source|updated|code|tests):\s*(.*)$/)
    if (header) {
      const key = header[1]
      const inline = header[2].trim()
      if (key === "source") cur.source = inline
      else if (key === "updated") cur.updated = inline
      else {
        section = key // "code" | "tests"
        if (inline) cur[key].push(inline) // rare inline single value
      }
      continue
    }

    const item = line.match(/^\s*-\s+(.*\S)\s*$/)
    if (item && section) cur[section].push(item[1].trim())
  }

  return blocks
}

// --- pollution classification ----------------------------------------------

const LOCKFILES = new Set([
  "Cargo.lock",
  "package-lock.json",
  "pnpm-lock.yaml",
  "yarn.lock",
])
const MANIFESTS = new Set(["Cargo.toml", "package.json", "tauri.conf.json"])

/** Classify a single code:/tests: entry. "ok" means it looks legitimate. */
function classifyEntry(path) {
  const p = path.replace(/\\/g, "/")
  const base = p.split("/").pop() ?? p
  if (p.startsWith("docs/")) return "docs" // high confidence
  if (LOCKFILES.has(base)) return "lockfile" // high confidence
  if (MANIFESTS.has(base)) return "manifest" // suspect (version bump)
  return "ok"
}

// --- run ---------------------------------------------------------------------

const specFiles = findSpecFiles(SCAN_DIR)

let affectedFiles = 0
let pollutedBlocks = 0 // high-confidence: docs/ or lockfile in the trace
let suspectBlocks = 0 // manifest-only: version bump vs real dependency change
let emptyBlocks = 0
let pollutedRefs = 0
let suspectRefs = 0

const out = []
out.push("== @trace pollution report ==")
out.push(`scanned ${specFiles.length} spec file(s) under ${relative(REPO_ROOT, SCAN_DIR) || SCAN_DIR}`)
out.push("")

for (const file of specFiles) {
  const rel = relative(REPO_ROOT, file).replace(/\\/g, "/")
  const blocks = parseTraceBlocks(readFileSync(file, "utf8"))

  const fileReports = []
  for (const b of blocks) {
    const hits = { docs: [], lockfile: [], manifest: [] }
    for (const entry of [...b.code, ...b.tests]) {
      const cls = classifyEntry(entry)
      if (cls !== "ok") hits[cls].push(entry)
    }
    const highCount = hits.docs.length + hits.lockfile.length
    const suspectCount = hits.manifest.length
    const isEmpty = b.code.length === 0

    if (highCount > 0 || suspectCount > 0) {
      const tag = highCount > 0 ? "POLLUTION" : "SUSPECT"
      if (highCount > 0) {
        pollutedBlocks++
        pollutedRefs += highCount
      } else {
        suspectBlocks++
      }
      suspectRefs += suspectCount
      const detail = [`  L${b.startLine}  [${tag}] source: ${b.source || "(none)"}`]
      for (const kind of ["docs", "lockfile", "manifest"]) {
        if (hits[kind].length) {
          const note =
            kind === "manifest" ? " — verify: version bump vs real dependency change" : ""
          detail.push(`    ${kind} (${hits[kind].length})${note}:`)
          for (const h of hits[kind]) detail.push(`      - ${h}`)
        }
      }
      fileReports.push(detail.join("\n"))
    } else if (isEmpty) {
      emptyBlocks++
      fileReports.push(
        `  L${b.startLine}  [EMPTY] source: ${b.source || "(none)"} — code: missing (delete-without-regenerate)`,
      )
    }
  }

  if (fileReports.length) {
    affectedFiles++
    out.push(rel)
    out.push(...fileReports)
    out.push("")
  }
}

out.push("== summary ==")
out.push(`  spec files affected:          ${affectedFiles} / ${specFiles.length}`)
out.push(`  polluted blocks (docs/lock):  ${pollutedBlocks}   refs: ${pollutedRefs}`)
out.push(`  suspect blocks (manifest):    ${suspectBlocks}   refs: ${suspectRefs}`)
out.push(`  empty trace blocks:           ${emptyBlocks}`)

// Only high-confidence pollution (docs/lockfile) and empty traces are
// actionable failures. A manifest hit is flagged for human review but cannot be
// auto-condemned: a real dependency or installer-config change legitimately
// lists a manifest (e.g. windows-installer touching tauri.conf.json).
const clean = pollutedBlocks === 0 && emptyBlocks === 0
out.push("")
out.push(
  clean
    ? suspectBlocks > 0
      ? "OK — no high-confidence pollution; review the SUSPECT manifest hits above."
      : "OK — no @trace pollution detected."
    : "FAIL — @trace pollution detected (see POLLUTION/EMPTY above).",
)

console.log(out.join("\n"))

if (STRICT && !clean) process.exit(1)
