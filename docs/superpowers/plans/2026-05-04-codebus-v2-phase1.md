# CodeBus v2 Phase 1 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the codebus CLI binary that takes a user repo + goal (or query), spawns `claude -p` as agent runtime, incrementally builds / queries an LLM wiki under `.codebus/`, with hybrid emoji/symbol terminal rendering.

**Architecture:** Node.js + TypeScript CLI (`npm install -g codebus`), hexagonal pattern — `core/` pure domain (wiki + vault), `infra/` side-effect adapters (fs + git + llm), `ui/` rendering, `commands/` thin orchestration. Phase 1 has single LLM adapter (claude-cli subprocess); phase 2/3 swap in anthropic-sdk / openai-sdk without touching core.

**Tech Stack:** Node.js ≥ 20, TypeScript 5+, `commander`, `chalk`, `ora`, `gray-matter`, `simple-git`, `vitest`, `tsx`, `tsc`. License MIT. Distribution via `npm publish`.

**Reference spec:** `docs/superpowers/specs/2026-05-04-codebus-v2-phase1-design.md`

---

## File Structure

```
codebus/
├── package.json
├── tsconfig.json
├── vitest.config.ts
├── .gitignore
├── README.md
├── LICENSE                              ← MIT
├── NOTICE                               ← (only if Apache/BSD deps require)
├── src/
│   ├── cli.ts                           ← entry, commander setup
│   ├── commands/                        ← thin orchestration
│   │   ├── init.ts
│   │   ├── goal.ts                      ← --goal (ingest)
│   │   └── query.ts                     ← --query (read-only)
│   ├── core/                            ← pure domain (no IO)
│   │   ├── wiki/
│   │   │   ├── types.ts                 ← Page / Frontmatter / SourceRef
│   │   │   ├── frontmatter.ts           ← parse / serialize
│   │   │   ├── frontmatter-repair.ts    ← wikilink list YAML repair
│   │   │   ├── page-merge.ts            ← append-merge dispatcher
│   │   │   └── stale-detect.ts          ← sha256 比對純函式
│   │   └── vault/
│   │       ├── layout.ts                ← .codebus/ paths constants
│   │       └── lock.ts                  ← file-based mutex (acquire/release)
│   ├── infra/                           ← side-effect adapters
│   │   ├── fs/
│   │   │   ├── file-ops.ts              ← read/write/hash 包裝
│   │   │   └── raw-sync.ts              ← copy repo → raw/code/ + gitignore filter
│   │   ├── git/
│   │   │   ├── source-version.ts        ← git rev-parse / git status
│   │   │   └── nested-repo.ts           ← .codebus/.git init / add / commit
│   │   ├── llm/
│   │   │   ├── types.ts                 ← LLMProvider interface + StreamEvent
│   │   │   └── claude-cli.ts            ← spawn `claude -p` 唯一 phase 1 adapter
│   │   └── global-config.ts             ← read ~/.codebus/config.yaml
│   ├── ui/
│   │   ├── stream-parser.ts             ← claude-cli stream-json → StreamEvent
│   │   ├── render.ts                    ← StreamEvent → terminal (emoji/symbol)
│   │   └── emoji-mode.ts                ← auto/on/off detection
│   └── schema/
│       └── claude-md.ts                 ← built-in CLAUDE.md template content
└── tests/
    ├── core/wiki/
    ├── core/vault/
    ├── infra/fs/
    ├── infra/git/
    ├── infra/llm/
    ├── ui/
    ├── commands/
    └── e2e/
```

---

## Task 1: Repo init (package.json / tsconfig / vitest / LICENSE)

**Files:**
- Create: `package.json`, `tsconfig.json`, `vitest.config.ts`, `.gitignore`, `LICENSE`, `README.md`

- [ ] **Step 1: Create `package.json`**

```json
{
  "name": "codebus",
  "version": "0.1.0",
  "description": "Build an LLM wiki for any codebase via claude -p",
  "license": "MIT",
  "type": "module",
  "bin": {
    "codebus": "dist/cli.js"
  },
  "main": "dist/cli.js",
  "files": ["dist", "LICENSE", "README.md"],
  "engines": { "node": ">=20" },
  "scripts": {
    "build": "tsc",
    "dev": "tsx src/cli.ts",
    "test": "vitest run",
    "test:watch": "vitest",
    "prepublishOnly": "npm run test && npm run build"
  },
  "dependencies": {
    "commander": "^12.0.0",
    "chalk": "^5.3.0",
    "ora": "^8.0.0",
    "gray-matter": "^4.0.3",
    "simple-git": "^3.22.0",
    "js-yaml": "^4.1.0"
  },
  "devDependencies": {
    "typescript": "^5.4.0",
    "tsx": "^4.7.0",
    "vitest": "^1.4.0",
    "@types/node": "^20.11.0",
    "@types/js-yaml": "^4.0.9"
  }
}
```

- [ ] **Step 2: Create `tsconfig.json`**

```json
{
  "compilerOptions": {
    "target": "ES2022",
    "module": "ES2022",
    "moduleResolution": "Bundler",
    "outDir": "dist",
    "rootDir": "src",
    "strict": true,
    "esModuleInterop": true,
    "skipLibCheck": true,
    "forceConsistentCasingInFileNames": true,
    "declaration": true,
    "sourceMap": true
  },
  "include": ["src/**/*"],
  "exclude": ["node_modules", "dist", "tests"]
}
```

- [ ] **Step 3: Create `vitest.config.ts`**

```typescript
import { defineConfig } from 'vitest/config'

export default defineConfig({
  test: {
    include: ['tests/**/*.test.ts'],
    coverage: {
      provider: 'v8',
      reporter: ['text', 'html'],
      include: ['src/**/*.ts'],
      thresholds: { lines: 80, functions: 80, branches: 75, statements: 80 }
    }
  }
})
```

- [ ] **Step 4: Create `.gitignore`**

```
node_modules
dist
coverage
.DS_Store
*.log
```

- [ ] **Step 5: Create `LICENSE` (MIT)**

```
MIT License

Copyright (c) 2026 Harry (codebus contributors)

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
```

- [ ] **Step 6: Skeleton `README.md`**

```markdown
# 🚌 CodeBus

> Build an LLM wiki for any codebase via `claude -p`.

## Install

\`\`\`bash
npm install -g codebus
\`\`\`

## Usage

\`\`\`bash
codebus --repo <path>                          # init only
codebus --repo <path> --goal "<goal-text>"     # build wiki
codebus --repo <path> --query "<question>"     # ask wiki
\`\`\`

Open `.codebus/` in Obsidian to browse the wiki.

## License

MIT — see [LICENSE](LICENSE).
```

- [ ] **Step 7: Install deps + verify**

Run: `npm install`
Run: `npx tsc --noEmit` → expect no errors (no src/ yet)
Run: `npx vitest run` → expect "no test files found"

- [ ] **Step 8: Commit**

```bash
git add package.json tsconfig.json vitest.config.ts .gitignore LICENSE README.md
git commit -m "chore: scaffold codebus repo (package.json + tsconfig + vitest + LICENSE)"
```

---

## Task 2: core/vault (layout + lock)

**Files:**
- Create: `src/core/vault/layout.ts`, `src/core/vault/lock.ts`
- Create: `tests/core/vault/layout.test.ts`, `tests/core/vault/lock.test.ts`

- [ ] **Step 1: Write failing test for `layout.ts`**

`tests/core/vault/layout.test.ts`:
```typescript
import { describe, it, expect } from 'vitest'
import { vaultPaths } from '../../../src/core/vault/layout.js'

describe('vaultPaths', () => {
  it('returns all .codebus/ paths under given repo root', () => {
    const p = vaultPaths('/tmp/myrepo')
    expect(p.root).toBe('/tmp/myrepo/.codebus')
    expect(p.git).toBe('/tmp/myrepo/.codebus/.git')
    expect(p.gitignore).toBe('/tmp/myrepo/.codebus/.gitignore')
    expect(p.goalsJsonl).toBe('/tmp/myrepo/.codebus/goals.jsonl')
    expect(p.schemaMd).toBe('/tmp/myrepo/.codebus/CLAUDE.md')
    expect(p.raw).toBe('/tmp/myrepo/.codebus/raw')
    expect(p.rawCode).toBe('/tmp/myrepo/.codebus/raw/code')
    expect(p.wiki).toBe('/tmp/myrepo/.codebus/wiki')
    expect(p.wikiOverview).toBe('/tmp/myrepo/.codebus/wiki/overview.md')
    expect(p.wikiIndex).toBe('/tmp/myrepo/.codebus/wiki/index.md')
    expect(p.wikiLog).toBe('/tmp/myrepo/.codebus/wiki/log.md')
    expect(p.wikiPages).toBe('/tmp/myrepo/.codebus/wiki/pages')
    expect(p.wikiGoals).toBe('/tmp/myrepo/.codebus/wiki/goals')
    expect(p.output).toBe('/tmp/myrepo/.codebus/output')
    expect(p.lock).toBe('/tmp/myrepo/.codebus/.lock')
  })
})
```

- [ ] **Step 2: Run test → expect FAIL**

Run: `npx vitest run tests/core/vault/layout.test.ts`
Expected: FAIL — module not found.

- [ ] **Step 3: Implement `src/core/vault/layout.ts`**

```typescript
import { join } from 'node:path'

export interface VaultPaths {
  root: string
  git: string
  gitignore: string
  goalsJsonl: string
  schemaMd: string
  raw: string                        // raw/ 父 folder（容納多種 source type）
  rawCode: string                    // raw/code/ — codebase 落點
  wiki: string
  wikiOverview: string
  wikiIndex: string
  wikiLog: string
  wikiPages: string
  wikiGoals: string
  output: string
  lock: string
}

export function vaultPaths(repoRoot: string): VaultPaths {
  const root = join(repoRoot, '.codebus')
  const wiki = join(root, 'wiki')
  const raw = join(root, 'raw')
  return {
    root,
    git: join(root, '.git'),
    gitignore: join(root, '.gitignore'),
    goalsJsonl: join(root, 'goals.jsonl'),
    schemaMd: join(root, 'CLAUDE.md'),
    raw,
    rawCode: join(raw, 'code'),
    wiki,
    wikiOverview: join(wiki, 'overview.md'),
    wikiIndex: join(wiki, 'index.md'),
    wikiLog: join(wiki, 'log.md'),
    wikiPages: join(wiki, 'pages'),
    wikiGoals: join(wiki, 'goals'),
    output: join(root, 'output'),
    lock: join(root, '.lock')
  }
}
```

- [ ] **Step 4: Run test → expect PASS**

Run: `npx vitest run tests/core/vault/layout.test.ts`
Expected: PASS.

- [ ] **Step 5: Write failing test for `lock.ts`**

`tests/core/vault/lock.test.ts`:
```typescript
import { describe, it, expect, beforeEach, afterEach } from 'vitest'
import { mkdtempSync, rmSync, existsSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { acquireLock, releaseLock } from '../../../src/core/vault/lock.js'

describe('lock', () => {
  let dir: string
  beforeEach(() => { dir = mkdtempSync(join(tmpdir(), 'codebus-lock-')) })
  afterEach(() => { rmSync(dir, { recursive: true, force: true }) })

  it('acquires lock by writing pid file', async () => {
    const lockPath = join(dir, '.lock')
    const handle = await acquireLock(lockPath)
    expect(existsSync(lockPath)).toBe(true)
    await releaseLock(handle)
    expect(existsSync(lockPath)).toBe(false)
  })

  it('throws when lock already held', async () => {
    const lockPath = join(dir, '.lock')
    const h1 = await acquireLock(lockPath)
    await expect(acquireLock(lockPath)).rejects.toThrow(/already held/)
    await releaseLock(h1)
  })

  it('release is idempotent', async () => {
    const lockPath = join(dir, '.lock')
    const h = await acquireLock(lockPath)
    await releaseLock(h)
    await releaseLock(h) // no throw
  })
})
```

- [ ] **Step 6: Run test → expect FAIL**

Run: `npx vitest run tests/core/vault/lock.test.ts`
Expected: FAIL — module not found.

- [ ] **Step 7: Implement `src/core/vault/lock.ts`**

```typescript
import { writeFile, unlink } from 'node:fs/promises'
import { existsSync } from 'node:fs'

export interface LockHandle {
  path: string
  released: boolean
}

export async function acquireLock(lockPath: string): Promise<LockHandle> {
  if (existsSync(lockPath)) {
    throw new Error(`Lock already held at ${lockPath}`)
  }
  await writeFile(lockPath, String(process.pid), { flag: 'wx' })
  return { path: lockPath, released: false }
}

export async function releaseLock(handle: LockHandle): Promise<void> {
  if (handle.released) return
  if (existsSync(handle.path)) {
    await unlink(handle.path)
  }
  handle.released = true
}
```

- [ ] **Step 8: Run test → expect PASS**

Run: `npx vitest run tests/core/vault/lock.test.ts`
Expected: PASS (3 tests).

- [ ] **Step 9: Commit**

```bash
git add src/core/vault tests/core/vault
git commit -m "feat(core): add vault layout constants and file-based lock"
```

---

## Task 3: core/wiki/types + frontmatter (parse / serialize)

**Files:**
- Create: `src/core/wiki/types.ts`, `src/core/wiki/frontmatter.ts`
- Create: `tests/core/wiki/frontmatter.test.ts`

- [ ] **Step 1: Write `src/core/wiki/types.ts`**

```typescript
export interface SourceRef {
  path: string
  sha256: string
  at_commit: string
}

export type PageType = 'concept' | 'module' | 'process' | 'entity'

export interface PageFrontmatter {
  title: string
  type: PageType
  sources: SourceRef[]
  goals: string[]
  created: string                   // YYYY-MM-DD
  updated: string                   // YYYY-MM-DD
  related: string[]                 // ["[[slug-a]]", "[[slug-b]]"]
  stale: boolean
}

export interface ParsedPage {
  frontmatter: PageFrontmatter
  body: string
}
```

- [ ] **Step 2: Write failing test for `frontmatter.ts`**

`tests/core/wiki/frontmatter.test.ts`:
```typescript
import { describe, it, expect } from 'vitest'
import { parsePage, serializePage } from '../../../src/core/wiki/frontmatter.js'

const samplePage = `---
title: Payment Gateway
type: concept
sources:
  - path: src/services/payment.py
    sha256: abc123
    at_commit: deadbeef
goals:
  - "了解結帳流程"
created: '2026-05-04'
updated: '2026-05-04'
related:
  - "[[checkout-flow]]"
stale: false
---
# Payment Gateway

Body content here.
`

describe('parsePage', () => {
  it('parses frontmatter and body', () => {
    const { frontmatter, body } = parsePage(samplePage)
    expect(frontmatter.title).toBe('Payment Gateway')
    expect(frontmatter.type).toBe('concept')
    expect(frontmatter.sources).toEqual([
      { path: 'src/services/payment.py', sha256: 'abc123', at_commit: 'deadbeef' }
    ])
    expect(frontmatter.goals).toEqual(['了解結帳流程'])
    expect(frontmatter.related).toEqual(['[[checkout-flow]]'])
    expect(frontmatter.stale).toBe(false)
    expect(body.trim().startsWith('# Payment Gateway')).toBe(true)
  })

  it('throws on missing required field', () => {
    const bad = `---\ntitle: X\n---\nbody`
    expect(() => parsePage(bad)).toThrow(/required field/)
  })
})

describe('serializePage', () => {
  it('round-trips parse → serialize → parse', () => {
    const { frontmatter, body } = parsePage(samplePage)
    const serialized = serializePage(frontmatter, body)
    const reparsed = parsePage(serialized)
    expect(reparsed.frontmatter).toEqual(frontmatter)
  })
})
```

- [ ] **Step 3: Run test → expect FAIL**

Run: `npx vitest run tests/core/wiki/frontmatter.test.ts`
Expected: FAIL — module not found.

- [ ] **Step 4: Implement `src/core/wiki/frontmatter.ts`**

```typescript
import matter from 'gray-matter'
import type { PageFrontmatter, ParsedPage } from './types.js'

const REQUIRED_FIELDS: (keyof PageFrontmatter)[] = [
  'title', 'type', 'sources', 'goals', 'created', 'updated', 'related', 'stale'
]

export function parsePage(content: string): ParsedPage {
  const { data, content: body } = matter(content)

  for (const field of REQUIRED_FIELDS) {
    if (!(field in data)) {
      throw new Error(`Missing required field in frontmatter: ${field}`)
    }
  }

  return {
    frontmatter: {
      title: String(data.title),
      type: data.type,
      sources: Array.isArray(data.sources) ? data.sources : [],
      goals: Array.isArray(data.goals) ? data.goals.map(String) : [],
      created: String(data.created),
      updated: String(data.updated),
      related: Array.isArray(data.related) ? data.related.map(String) : [],
      stale: Boolean(data.stale)
    },
    body
  }
}

export function serializePage(frontmatter: PageFrontmatter, body: string): string {
  return matter.stringify(body, frontmatter as Record<string, unknown>)
}
```

- [ ] **Step 5: Run test → expect PASS**

Run: `npx vitest run tests/core/wiki/frontmatter.test.ts`
Expected: PASS (3 tests).

- [ ] **Step 6: Commit**

```bash
git add src/core/wiki/types.ts src/core/wiki/frontmatter.ts tests/core/wiki/frontmatter.test.ts
git commit -m "feat(core): add wiki types and frontmatter parse/serialize"
```

---

## Task 4: core/wiki/frontmatter-repair (wikilink list YAML)

**Files:**
- Create: `src/core/wiki/frontmatter-repair.ts`
- Create: `tests/core/wiki/frontmatter-repair.test.ts`

- [ ] **Step 1: Write failing test**

`tests/core/wiki/frontmatter-repair.test.ts`:
```typescript
import { describe, it, expect } from 'vitest'
import { repairWikilinkList } from '../../../src/core/wiki/frontmatter-repair.js'

describe('repairWikilinkList', () => {
  it('quotes wikilink list values', () => {
    const input = `related: [[a]], [[b]], [[c]]`
    const output = repairWikilinkList(input)
    expect(output).toBe(`related: ["[[a]]", "[[b]]", "[[c]]"]`)
  })

  it('handles single wikilink with no comma', () => {
    const input = `related: [[only-one]]`
    const output = repairWikilinkList(input)
    expect(output).toBe(`related: ["[[only-one]]"]`)
  })

  it('leaves already-quoted wikilink list untouched', () => {
    const input = `related: ["[[a]]", "[[b]]"]`
    expect(repairWikilinkList(input)).toBe(input)
  })

  it('only repairs wikilink-shaped lines, not other arrays', () => {
    const input = `tags: [foo, bar]`
    expect(repairWikilinkList(input)).toBe(input)
  })

  it('repairs each line independently in multi-line input', () => {
    const input = `related: [[a]], [[b]]\nsee_also: [[x]], [[y]]`
    const expected = `related: ["[[a]]", "[[b]]"]\nsee_also: ["[[x]]", "[[y]]"]`
    expect(repairWikilinkList(input)).toBe(expected)
  })
})
```

- [ ] **Step 2: Run test → expect FAIL**

Run: `npx vitest run tests/core/wiki/frontmatter-repair.test.ts`
Expected: FAIL — module not found.

- [ ] **Step 3: Implement `src/core/wiki/frontmatter-repair.ts`**

```typescript
const WIKILINK_LIST_LINE = /^(\s*[A-Za-z_][\w-]*\s*:\s*)(\[\[[^\]]+\]\](?:\s*,\s*\[\[[^\]]+\]\])*)\s*$/

export function repairWikilinkList(text: string): string {
  return text
    .split('\n')
    .map((line) => {
      const m = line.match(WIKILINK_LIST_LINE)
      if (!m) return line
      const prefix = m[1]
      const items = m[2]
        .split(',')
        .map((s) => s.trim())
        .filter(Boolean)
        .map((s) => `"${s}"`)
        .join(', ')
      return `${prefix}[${items}]`
    })
    .join('\n')
}
```

- [ ] **Step 4: Run test → expect PASS**

Run: `npx vitest run tests/core/wiki/frontmatter-repair.test.ts`
Expected: PASS (5 tests).

- [ ] **Step 5: Commit**

```bash
git add src/core/wiki/frontmatter-repair.ts tests/core/wiki/frontmatter-repair.test.ts
git commit -m "feat(core): add wikilink list YAML repair util"
```

---

## Task 5: core/wiki/page-merge (append-merge)

**Files:**
- Create: `src/core/wiki/page-merge.ts`
- Create: `tests/core/wiki/page-merge.test.ts`

- [ ] **Step 1: Write failing test**

`tests/core/wiki/page-merge.test.ts`:
```typescript
import { describe, it, expect } from 'vitest'
import { mergePage } from '../../../src/core/wiki/page-merge.js'
import type { ParsedPage } from '../../../src/core/wiki/types.js'

const existing: ParsedPage = {
  frontmatter: {
    title: 'Payment Gateway',
    type: 'concept',
    sources: [{ path: 'src/payment.py', sha256: 'abc', at_commit: 'c1' }],
    goals: ['結帳流程'],
    created: '2026-05-01',
    updated: '2026-05-01',
    related: ['[[checkout-flow]]'],
    stale: false
  },
  body: '# Payment Gateway\n\nOriginal body.\n'
}

const incoming: ParsedPage = {
  frontmatter: {
    title: 'Payment Gateway',
    type: 'concept',
    sources: [{ path: 'src/refund.py', sha256: 'def', at_commit: 'c2' }],
    goals: ['退款處理'],
    created: '2026-05-04',           // ignored — locked
    updated: '2026-05-04',
    related: ['[[refund-flow]]'],
    stale: true                      // ignored — incoming doesn't override
  },
  body: 'Refund-perspective content.\n'
}

describe('mergePage', () => {
  it('unions sources / goals / related arrays', () => {
    const merged = mergePage(existing, incoming, '退款處理', '2026-05-04')
    expect(merged.frontmatter.sources).toEqual([
      { path: 'src/payment.py', sha256: 'abc', at_commit: 'c1' },
      { path: 'src/refund.py', sha256: 'def', at_commit: 'c2' }
    ])
    expect(merged.frontmatter.goals).toEqual(['結帳流程', '退款處理'])
    expect(merged.frontmatter.related).toEqual(['[[checkout-flow]]', '[[refund-flow]]'])
  })

  it('locks title / type / created from existing', () => {
    const merged = mergePage(existing, incoming, '退款處理', '2026-05-04')
    expect(merged.frontmatter.title).toBe('Payment Gateway')
    expect(merged.frontmatter.type).toBe('concept')
    expect(merged.frontmatter.created).toBe('2026-05-01')
  })

  it('updates `updated` to today', () => {
    const merged = mergePage(existing, incoming, '退款處理', '2026-05-04')
    expect(merged.frontmatter.updated).toBe('2026-05-04')
  })

  it('appends ## from goal section to body', () => {
    const merged = mergePage(existing, incoming, '退款處理', '2026-05-04')
    expect(merged.body).toContain('Original body.')
    expect(merged.body).toContain('## from goal: 退款處理 (2026-05-04)')
    expect(merged.body).toContain('Refund-perspective content.')
  })

  it('does not duplicate goal in goals array if already present', () => {
    const merged = mergePage(existing, incoming, '結帳流程', '2026-05-04')
    expect(merged.frontmatter.goals).toEqual(['結帳流程', '退款處理'])
  })
})
```

- [ ] **Step 2: Run test → expect FAIL**

Run: `npx vitest run tests/core/wiki/page-merge.test.ts`
Expected: FAIL — module not found.

- [ ] **Step 3: Implement `src/core/wiki/page-merge.ts`**

```typescript
import type { ParsedPage, SourceRef } from './types.js'

function uniqueSources(a: SourceRef[], b: SourceRef[]): SourceRef[] {
  const seen = new Set<string>()
  const out: SourceRef[] = []
  for (const s of [...a, ...b]) {
    if (!seen.has(s.path)) {
      seen.add(s.path)
      out.push(s)
    }
  }
  return out
}

function uniqueStrings(a: string[], b: string[]): string[] {
  const seen = new Set<string>()
  const out: string[] = []
  for (const s of [...a, ...b]) {
    if (!seen.has(s)) {
      seen.add(s)
      out.push(s)
    }
  }
  return out
}

export function mergePage(
  existing: ParsedPage,
  incoming: ParsedPage,
  goalText: string,
  today: string
): ParsedPage {
  const sources = uniqueSources(existing.frontmatter.sources, incoming.frontmatter.sources)
  const goals = uniqueStrings(existing.frontmatter.goals, [goalText])
  const related = uniqueStrings(existing.frontmatter.related, incoming.frontmatter.related)

  const sectionHeader = `## from goal: ${goalText} (${today})`
  const body = `${existing.body.trimEnd()}\n\n${sectionHeader}\n\n${incoming.body.trim()}\n`

  return {
    frontmatter: {
      // Locked fields from existing
      title: existing.frontmatter.title,
      type: existing.frontmatter.type,
      created: existing.frontmatter.created,
      // Merged
      sources,
      goals,
      related,
      // Updated
      updated: today,
      // Stale flag managed by stale-detect, not page-merge
      stale: existing.frontmatter.stale
    },
    body
  }
}
```

- [ ] **Step 4: Run test → expect PASS**

Run: `npx vitest run tests/core/wiki/page-merge.test.ts`
Expected: PASS (5 tests).

- [ ] **Step 5: Commit**

```bash
git add src/core/wiki/page-merge.ts tests/core/wiki/page-merge.test.ts
git commit -m "feat(core): add append-merge page conflict resolver"
```

---

## Task 6: core/wiki/stale-detect (sha256 比對)

**Files:**
- Create: `src/core/wiki/stale-detect.ts`
- Create: `tests/core/wiki/stale-detect.test.ts`

- [ ] **Step 1: Write failing test**

`tests/core/wiki/stale-detect.test.ts`:
```typescript
import { describe, it, expect } from 'vitest'
import { detectStaleSources, type StaleResult } from '../../../src/core/wiki/stale-detect.js'
import type { PageFrontmatter } from '../../../src/core/wiki/types.js'

const fm: PageFrontmatter = {
  title: 'X', type: 'concept',
  sources: [
    { path: 'src/a.py', sha256: 'aaa', at_commit: 'c1' },
    { path: 'src/b.py', sha256: 'bbb', at_commit: 'c1' }
  ],
  goals: [], created: '2026-05-04', updated: '2026-05-04', related: [], stale: false
}

describe('detectStaleSources', () => {
  it('returns clean when all source hashes match current', () => {
    const current = new Map([['src/a.py', 'aaa'], ['src/b.py', 'bbb']])
    const result: StaleResult = detectStaleSources(fm, current)
    expect(result.isStale).toBe(false)
    expect(result.changedSources).toEqual([])
  })

  it('returns stale when any source hash differs', () => {
    const current = new Map([['src/a.py', 'aaa'], ['src/b.py', 'bbb-NEW']])
    const result = detectStaleSources(fm, current)
    expect(result.isStale).toBe(true)
    expect(result.changedSources).toEqual(['src/b.py'])
  })

  it('returns stale when source missing from current', () => {
    const current = new Map([['src/a.py', 'aaa']])
    const result = detectStaleSources(fm, current)
    expect(result.isStale).toBe(true)
    expect(result.changedSources).toEqual(['src/b.py'])
  })

  it('handles empty sources gracefully', () => {
    const empty: PageFrontmatter = { ...fm, sources: [] }
    const result = detectStaleSources(empty, new Map())
    expect(result.isStale).toBe(false)
    expect(result.changedSources).toEqual([])
  })
})
```

- [ ] **Step 2: Run test → expect FAIL**

Run: `npx vitest run tests/core/wiki/stale-detect.test.ts`
Expected: FAIL — module not found.

- [ ] **Step 3: Implement `src/core/wiki/stale-detect.ts`**

```typescript
import type { PageFrontmatter } from './types.js'

export interface StaleResult {
  isStale: boolean
  changedSources: string[]    // path of sources whose hash changed or missing
}

export function detectStaleSources(
  fm: PageFrontmatter,
  currentHashes: Map<string, string>
): StaleResult {
  const changed: string[] = []
  for (const src of fm.sources) {
    const current = currentHashes.get(src.path)
    if (current === undefined || current !== src.sha256) {
      changed.push(src.path)
    }
  }
  return {
    isStale: changed.length > 0,
    changedSources: changed
  }
}
```

- [ ] **Step 4: Run test → expect PASS**

Run: `npx vitest run tests/core/wiki/stale-detect.test.ts`
Expected: PASS (4 tests).

- [ ] **Step 5: Commit**

```bash
git add src/core/wiki/stale-detect.ts tests/core/wiki/stale-detect.test.ts
git commit -m "feat(core): add stale source detection (pure sha256 compare)"
```

---

## Task 7: infra/fs (file-ops + raw-sync with gitignore filter)

**Files:**
- Create: `src/infra/fs/file-ops.ts`, `src/infra/fs/raw-sync.ts`
- Create: `tests/infra/fs/file-ops.test.ts`, `tests/infra/fs/raw-sync.test.ts`

- [ ] **Step 1: Write failing test for `file-ops.ts`**

`tests/infra/fs/file-ops.test.ts`:
```typescript
import { describe, it, expect, beforeEach, afterEach } from 'vitest'
import { mkdtempSync, rmSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { sha256File, listFilesRecursive } from '../../../src/infra/fs/file-ops.js'

describe('sha256File', () => {
  let dir: string
  beforeEach(() => { dir = mkdtempSync(join(tmpdir(), 'codebus-fs-')) })
  afterEach(() => { rmSync(dir, { recursive: true, force: true }) })

  it('computes sha256 of file content', async () => {
    const f = join(dir, 'a.txt')
    writeFileSync(f, 'hello')
    const hash = await sha256File(f)
    expect(hash).toBe('2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824')
  })
})

describe('listFilesRecursive', () => {
  let dir: string
  beforeEach(() => {
    dir = mkdtempSync(join(tmpdir(), 'codebus-fs-'))
    writeFileSync(join(dir, 'a.txt'), '')
    const sub = join(dir, 'sub')
    require('node:fs').mkdirSync(sub)
    writeFileSync(join(sub, 'b.txt'), '')
  })
  afterEach(() => { rmSync(dir, { recursive: true, force: true }) })

  it('lists all files recursively (paths relative to root)', async () => {
    const files = await listFilesRecursive(dir)
    expect(files.sort()).toEqual(['a.txt', 'sub/b.txt'])
  })
})
```

- [ ] **Step 2: Run test → expect FAIL**

Run: `npx vitest run tests/infra/fs/file-ops.test.ts`
Expected: FAIL — module not found.

- [ ] **Step 3: Implement `src/infra/fs/file-ops.ts`**

```typescript
import { createHash } from 'node:crypto'
import { createReadStream } from 'node:fs'
import { readdir, stat } from 'node:fs/promises'
import { join, relative, sep } from 'node:path'

export async function sha256File(path: string): Promise<string> {
  const hash = createHash('sha256')
  const stream = createReadStream(path)
  for await (const chunk of stream) hash.update(chunk)
  return hash.digest('hex')
}

export async function listFilesRecursive(root: string): Promise<string[]> {
  const out: string[] = []
  async function walk(dir: string) {
    const entries = await readdir(dir, { withFileTypes: true })
    for (const e of entries) {
      const full = join(dir, e.name)
      if (e.isDirectory()) await walk(full)
      else if (e.isFile()) out.push(relative(root, full).split(sep).join('/'))
    }
  }
  await walk(root)
  return out
}
```

- [ ] **Step 4: Run test → expect PASS**

Run: `npx vitest run tests/infra/fs/file-ops.test.ts`
Expected: PASS (2 tests).

- [ ] **Step 5: Write failing test for `raw-sync.ts`**

`tests/infra/fs/raw-sync.test.ts`:
```typescript
import { describe, it, expect, beforeEach, afterEach } from 'vitest'
import { mkdtempSync, rmSync, mkdirSync, writeFileSync, existsSync, readFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { syncRepoToRaw } from '../../../src/infra/fs/raw-sync.js'

describe('syncRepoToRaw', () => {
  let repo: string, rawCode: string
  beforeEach(() => {
    repo = mkdtempSync(join(tmpdir(), 'codebus-repo-'))
    rawCode = join(repo, '.codebus', 'raw', 'code')
    mkdirSync(join(repo, 'src'), { recursive: true })
    writeFileSync(join(repo, 'src', 'app.ts'), 'console.log("hi")')
    mkdirSync(join(repo, 'node_modules', 'lodash'), { recursive: true })
    writeFileSync(join(repo, 'node_modules', 'lodash', 'index.js'), '// big')
    mkdirSync(join(repo, '.git'))
    writeFileSync(join(repo, '.git', 'HEAD'), 'ref: refs/heads/main')
    mkdirSync(join(repo, '.codebus'))
    writeFileSync(join(repo, '.codebus', 'goals.jsonl'), '{}')
    writeFileSync(join(repo, '.gitignore'), 'node_modules\n')
    writeFileSync(join(repo, '.env'), 'SECRET=xxx')
  })
  afterEach(() => { rmSync(repo, { recursive: true, force: true }) })

  it('copies repo content into raw/code, excluding .codebus/, .git/, .env, and gitignored', async () => {
    await syncRepoToRaw(repo, rawCode)
    expect(existsSync(join(rawCode, 'src', 'app.ts'))).toBe(true)
    expect(readFileSync(join(rawCode, 'src', 'app.ts'), 'utf8')).toBe('console.log("hi")')
    expect(existsSync(join(rawCode, 'node_modules'))).toBe(false)
    expect(existsSync(join(rawCode, '.git'))).toBe(false)
    expect(existsSync(join(rawCode, '.codebus'))).toBe(false)
    expect(existsSync(join(rawCode, '.env'))).toBe(false)
  })

  it('clears existing raw/code before re-syncing (does not touch raw/ siblings)', async () => {
    mkdirSync(rawCode, { recursive: true })
    writeFileSync(join(rawCode, 'stale.txt'), 'old')
    // Create a sibling raw/docs/ — must survive
    mkdirSync(join(repo, '.codebus', 'raw', 'docs'), { recursive: true })
    writeFileSync(join(repo, '.codebus', 'raw', 'docs', 'spec.md'), 'user-managed')
    await syncRepoToRaw(repo, rawCode)
    expect(existsSync(join(rawCode, 'stale.txt'))).toBe(false)
    expect(existsSync(join(repo, '.codebus', 'raw', 'docs', 'spec.md'))).toBe(true)
  })
})
```

- [ ] **Step 6: Run test → expect FAIL**

Run: `npx vitest run tests/infra/fs/raw-sync.test.ts`
Expected: FAIL — module not found.

- [ ] **Step 7: Implement `src/infra/fs/raw-sync.ts`**

```typescript
import { readdir, mkdir, copyFile, rm, readFile } from 'node:fs/promises'
import { existsSync } from 'node:fs'
import { join, relative, sep } from 'node:path'

const ALWAYS_SKIP = new Set(['.codebus', '.git', '.env'])

interface IgnoreMatcher {
  match(relPath: string): boolean
}

async function loadGitignore(repoRoot: string): Promise<IgnoreMatcher> {
  const gi = join(repoRoot, '.gitignore')
  if (!existsSync(gi)) return { match: () => false }
  const text = await readFile(gi, 'utf8')
  const patterns = text.split('\n').map(s => s.trim()).filter(s => s && !s.startsWith('#'))
  return {
    match(rel: string): boolean {
      const segments = rel.split('/')
      return patterns.some(pat => {
        if (pat.endsWith('/')) pat = pat.slice(0, -1)
        return segments.includes(pat) || rel === pat || rel.startsWith(pat + '/')
      })
    }
  }
}

const MAX_FILE_BYTES = 1024 * 1024 * 5  // 5 MiB skip threshold

export async function syncRepoToRaw(repoRoot: string, rawDir: string): Promise<void> {
  if (existsSync(rawDir)) await rm(rawDir, { recursive: true, force: true })
  await mkdir(rawDir, { recursive: true })

  const ignore = await loadGitignore(repoRoot)

  async function walk(srcDir: string, dstDir: string): Promise<void> {
    const entries = await readdir(srcDir, { withFileTypes: true })
    for (const e of entries) {
      const srcPath = join(srcDir, e.name)
      const rel = relative(repoRoot, srcPath).split(sep).join('/')
      if (ALWAYS_SKIP.has(e.name) && srcDir === repoRoot) continue
      if (ignore.match(rel)) continue
      const dstPath = join(dstDir, e.name)
      if (e.isDirectory()) {
        await mkdir(dstPath, { recursive: true })
        await walk(srcPath, dstPath)
      } else if (e.isFile()) {
        const { size } = await import('node:fs').then(m => m.statSync(srcPath))
        if (size > MAX_FILE_BYTES) continue
        await copyFile(srcPath, dstPath)
      }
    }
  }

  await walk(repoRoot, rawDir)
}
```

- [ ] **Step 8: Run test → expect PASS**

Run: `npx vitest run tests/infra/fs/raw-sync.test.ts`
Expected: PASS (2 tests).

- [ ] **Step 9: Commit**

```bash
git add src/infra/fs tests/infra/fs
git commit -m "feat(infra): add fs file-ops (sha256/list) and raw-sync with gitignore filter"
```

---

## Task 8: infra/git (source-version + nested-repo)

**Files:**
- Create: `src/infra/git/source-version.ts`, `src/infra/git/nested-repo.ts`
- Create: `tests/infra/git/source-version.test.ts`, `tests/infra/git/nested-repo.test.ts`

- [ ] **Step 1: Write failing test for `source-version.ts`**

`tests/infra/git/source-version.test.ts`:
```typescript
import { describe, it, expect, beforeEach, afterEach } from 'vitest'
import { mkdtempSync, rmSync, writeFileSync, mkdirSync } from 'node:fs'
import { execSync } from 'node:child_process'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { getSourceVersion } from '../../../src/infra/git/source-version.js'

describe('getSourceVersion', () => {
  let dir: string
  beforeEach(() => {
    dir = mkdtempSync(join(tmpdir(), 'codebus-srcver-'))
    execSync('git init -q -b main', { cwd: dir })
    execSync('git config user.email "t@t.com" && git config user.name "T"', { cwd: dir })
    writeFileSync(join(dir, 'a.txt'), 'hello')
    execSync('git add . && git commit -q -m init', { cwd: dir })
  })
  afterEach(() => { rmSync(dir, { recursive: true, force: true }) })

  it('returns commit hash and clean=true on clean repo', async () => {
    const v = await getSourceVersion(dir)
    expect(v.commit).toMatch(/^[0-9a-f]{40}$/)
    expect(v.uncommitted).toBe(false)
  })

  it('returns uncommitted=true when working tree has changes', async () => {
    writeFileSync(join(dir, 'a.txt'), 'changed')
    const v = await getSourceVersion(dir)
    expect(v.uncommitted).toBe(true)
  })

  it('returns commit=null when path is not a git repo', async () => {
    const nonGit = mkdtempSync(join(tmpdir(), 'codebus-nongit-'))
    const v = await getSourceVersion(nonGit)
    expect(v.commit).toBe(null)
    expect(v.uncommitted).toBe(false)
    rmSync(nonGit, { recursive: true, force: true })
  })
})
```

- [ ] **Step 2: Run test → expect FAIL**

Run: `npx vitest run tests/infra/git/source-version.test.ts`
Expected: FAIL — module not found.

- [ ] **Step 3: Implement `src/infra/git/source-version.ts`**

```typescript
import { existsSync } from 'node:fs'
import { join } from 'node:path'
import { simpleGit } from 'simple-git'

export interface SourceVersion {
  commit: string | null         // null = not a git repo
  uncommitted: boolean
}

export async function getSourceVersion(repoRoot: string): Promise<SourceVersion> {
  if (!existsSync(join(repoRoot, '.git'))) {
    return { commit: null, uncommitted: false }
  }
  const git = simpleGit(repoRoot)
  const commit = (await git.revparse(['HEAD'])).trim()
  const status = await git.status()
  return {
    commit,
    uncommitted: !status.isClean()
  }
}
```

- [ ] **Step 4: Run test → expect PASS**

Run: `npx vitest run tests/infra/git/source-version.test.ts`
Expected: PASS (3 tests).

- [ ] **Step 5: Write failing test for `nested-repo.ts`**

`tests/infra/git/nested-repo.test.ts`:
```typescript
import { describe, it, expect, beforeEach, afterEach } from 'vitest'
import { mkdtempSync, rmSync, mkdirSync, writeFileSync, existsSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { initNestedRepo, autoCommit } from '../../../src/infra/git/nested-repo.js'

describe('nested-repo', () => {
  let dir: string
  beforeEach(() => {
    dir = mkdtempSync(join(tmpdir(), 'codebus-nested-'))
    mkdirSync(join(dir, '.codebus'))
  })
  afterEach(() => { rmSync(dir, { recursive: true, force: true }) })

  it('initializes nested git repo at .codebus/.git', async () => {
    await initNestedRepo(join(dir, '.codebus'))
    expect(existsSync(join(dir, '.codebus', '.git'))).toBe(true)
  })

  it('autoCommit stages all files and commits with given message', async () => {
    await initNestedRepo(join(dir, '.codebus'))
    writeFileSync(join(dir, '.codebus', 'README.md'), 'hi')
    const sha = await autoCommit(join(dir, '.codebus'), 'wiki: test')
    expect(sha).toMatch(/^[0-9a-f]{40}$/)
  })
})
```

- [ ] **Step 6: Run test → expect FAIL**

Run: `npx vitest run tests/infra/git/nested-repo.test.ts`
Expected: FAIL — module not found.

- [ ] **Step 7: Implement `src/infra/git/nested-repo.ts`**

```typescript
import { existsSync } from 'node:fs'
import { join } from 'node:path'
import { simpleGit } from 'simple-git'

export async function initNestedRepo(vaultRoot: string): Promise<void> {
  if (existsSync(join(vaultRoot, '.git'))) return
  const git = simpleGit(vaultRoot)
  await git.init(['-b', 'main'])
  await git.addConfig('user.email', 'codebus@local')
  await git.addConfig('user.name', 'codebus')
}

export async function autoCommit(vaultRoot: string, message: string): Promise<string> {
  const git = simpleGit(vaultRoot)
  await git.add('-A')
  const status = await git.status()
  if (status.isClean()) {
    return (await git.revparse(['HEAD'])).trim()
  }
  const result = await git.commit(message)
  return result.commit
}
```

- [ ] **Step 8: Run test → expect PASS**

Run: `npx vitest run tests/infra/git/nested-repo.test.ts`
Expected: PASS (2 tests).

- [ ] **Step 9: Commit**

```bash
git add src/infra/git tests/infra/git
git commit -m "feat(infra): add git source-version detection and nested repo helpers"
```

---

## Task 9: infra/llm/types (LLMProvider interface + StreamEvent)

**Files:**
- Create: `src/infra/llm/types.ts`

- [ ] **Step 1: Write `src/infra/llm/types.ts`**

```typescript
export type StreamEvent =
  | { kind: 'thought'; text: string }
  | { kind: 'tool_use'; name: string; input: unknown }
  | { kind: 'tool_result'; output: string; isError: boolean }
  | { kind: 'done' }

export type LLMMode = 'ingest' | 'query'

export interface InvokeOptions {
  systemPrompt: string
  userMessage: string
  mode: LLMMode
  cwd: string
  vaultRoot: string                  // for --add-dir
}

export interface LLMProvider {
  invoke(opts: InvokeOptions): AsyncIterable<StreamEvent>
  cancel(): void
}
```

- [ ] **Step 2: Verify types compile**

Run: `npx tsc --noEmit`
Expected: no errors.

- [ ] **Step 3: Commit**

```bash
git add src/infra/llm/types.ts
git commit -m "feat(infra): add LLMProvider interface and StreamEvent schema"
```

---

## Task 10: infra/llm/claude-cli (spawn + stream)

**Files:**
- Create: `src/infra/llm/claude-cli.ts`
- Create: `tests/infra/llm/claude-cli.test.ts`

- [ ] **Step 1: Write failing test (using mock subprocess)**

`tests/infra/llm/claude-cli.test.ts`:
```typescript
import { describe, it, expect, vi } from 'vitest'
import { ClaudeCliProvider } from '../../../src/infra/llm/claude-cli.js'

describe('ClaudeCliProvider', () => {
  it('builds correct argv for ingest mode', () => {
    const p = new ClaudeCliProvider({ binary: 'claude' })
    const argv = p.buildArgv({ mode: 'ingest', vaultRoot: '/tmp/.codebus' })
    expect(argv).toEqual([
      '-p',
      '--output-format', 'stream-json',
      '--input-format', 'stream-json',
      '--verbose',
      '--add-dir', '/tmp/.codebus',
      '--disallowedTools', 'Bash,WebFetch,WebSearch'
    ])
  })

  it('builds correct argv for query mode (Write/Edit also disallowed)', () => {
    const p = new ClaudeCliProvider({ binary: 'claude' })
    const argv = p.buildArgv({ mode: 'query', vaultRoot: '/tmp/.codebus' })
    expect(argv).toContain('--disallowedTools')
    const idx = argv.indexOf('--disallowedTools')
    expect(argv[idx + 1]).toBe('Bash,WebFetch,WebSearch,Write,Edit')
  })
})
```

- [ ] **Step 2: Run test → expect FAIL**

Run: `npx vitest run tests/infra/llm/claude-cli.test.ts`
Expected: FAIL — module not found.

- [ ] **Step 3: Implement `src/infra/llm/claude-cli.ts`**

```typescript
import { spawn, ChildProcess } from 'node:child_process'
import { createInterface } from 'node:readline'
import type { LLMProvider, InvokeOptions, StreamEvent, LLMMode } from './types.js'

export interface ClaudeCliConfig {
  binary?: string                    // default 'claude'
  timeoutMs?: number                 // default 30 minutes
}

export class ClaudeCliProvider implements LLMProvider {
  private child: ChildProcess | null = null
  private cfg: Required<ClaudeCliConfig>

  constructor(cfg: ClaudeCliConfig = {}) {
    this.cfg = {
      binary: cfg.binary ?? 'claude',
      timeoutMs: cfg.timeoutMs ?? 30 * 60 * 1000
    }
  }

  buildArgv(opts: { mode: LLMMode; vaultRoot: string }): string[] {
    const disallowed = ['Bash', 'WebFetch', 'WebSearch']
    if (opts.mode === 'query') disallowed.push('Write', 'Edit')
    return [
      '-p',
      '--output-format', 'stream-json',
      '--input-format', 'stream-json',
      '--verbose',
      '--add-dir', opts.vaultRoot,
      '--disallowedTools', disallowed.join(',')
    ]
  }

  async *invoke(opts: InvokeOptions): AsyncIterable<StreamEvent> {
    const argv = this.buildArgv({ mode: opts.mode, vaultRoot: opts.vaultRoot })
    this.child = spawn(this.cfg.binary, argv, { cwd: opts.cwd })

    const timer = setTimeout(() => this.cancel(), this.cfg.timeoutMs)

    // send a single user-turn message via stream-json input
    const inputMsg = {
      type: 'user',
      message: { role: 'user', content: `${opts.systemPrompt}\n\n${opts.userMessage}` }
    }
    this.child.stdin?.write(JSON.stringify(inputMsg) + '\n')
    this.child.stdin?.end()

    const stdout = this.child.stdout
    if (!stdout) throw new Error('claude -p produced no stdout')
    const rl = createInterface({ input: stdout })

    try {
      for await (const line of rl) {
        if (!line.trim()) continue
        const parsed = JSON.parse(line)
        const event = mapClaudeStreamLine(parsed)
        if (event) yield event
      }
      yield { kind: 'done' }
    } finally {
      clearTimeout(timer)
    }
  }

  cancel(): void {
    if (this.child && !this.child.killed) {
      this.child.kill('SIGTERM')
    }
  }
}

// Internal: claude -p stream-json line → unified StreamEvent
function mapClaudeStreamLine(parsed: any): StreamEvent | null {
  if (parsed.type === 'stream_event' && parsed.event?.type === 'content_block_delta') {
    const text = parsed.event.delta?.text ?? ''
    return text ? { kind: 'thought', text } : null
  }
  if (parsed.type === 'stream_event' && parsed.event?.type === 'tool_use') {
    return { kind: 'tool_use', name: parsed.event.name, input: parsed.event.input }
  }
  if (parsed.type === 'stream_event' && parsed.event?.type === 'tool_result') {
    const content = Array.isArray(parsed.event.content)
      ? parsed.event.content.map((c: any) => c.text ?? '').join('')
      : String(parsed.event.content ?? '')
    return { kind: 'tool_result', output: content, isError: Boolean(parsed.event.is_error) }
  }
  // session_init / result_summary / assistant fallback / unknown → skip
  return null
}
```

- [ ] **Step 4: Run test → expect PASS**

Run: `npx vitest run tests/infra/llm/claude-cli.test.ts`
Expected: PASS (2 tests).

- [ ] **Step 5: Commit**

```bash
git add src/infra/llm/claude-cli.ts tests/infra/llm/claude-cli.test.ts
git commit -m "feat(infra): add claude-cli LLM provider (spawn + stream-json parse)"
```

---

## Task 11: ui/emoji-mode + render

**Files:**
- Create: `src/ui/emoji-mode.ts`, `src/ui/render.ts`
- Create: `tests/ui/emoji-mode.test.ts`, `tests/ui/render.test.ts`

- [ ] **Step 1: Write failing test for `emoji-mode.ts`**

`tests/ui/emoji-mode.test.ts`:
```typescript
import { describe, it, expect } from 'vitest'
import { resolveEmojiMode } from '../../src/ui/emoji-mode.js'

describe('resolveEmojiMode', () => {
  it('returns true when flag=on regardless of env', () => {
    expect(resolveEmojiMode('on', { isTTY: false, env: { CI: '1' } })).toBe(true)
  })

  it('returns false when flag=off', () => {
    expect(resolveEmojiMode('off', { isTTY: true, env: {} })).toBe(false)
  })

  it('auto: returns true when tty + no CI + no NO_EMOJI + TERM != dumb', () => {
    expect(resolveEmojiMode('auto', { isTTY: true, env: {} })).toBe(true)
  })

  it('auto: returns false when in CI', () => {
    expect(resolveEmojiMode('auto', { isTTY: true, env: { CI: '1' } })).toBe(false)
  })

  it('auto: returns false when NO_EMOJI is set', () => {
    expect(resolveEmojiMode('auto', { isTTY: true, env: { NO_EMOJI: '1' } })).toBe(false)
  })

  it('auto: returns false when not TTY', () => {
    expect(resolveEmojiMode('auto', { isTTY: false, env: {} })).toBe(false)
  })

  it('auto: returns false when TERM=dumb', () => {
    expect(resolveEmojiMode('auto', { isTTY: true, env: { TERM: 'dumb' } })).toBe(false)
  })
})
```

- [ ] **Step 2: Run test → expect FAIL**

Run: `npx vitest run tests/ui/emoji-mode.test.ts`
Expected: FAIL — module not found.

- [ ] **Step 3: Implement `src/ui/emoji-mode.ts`**

```typescript
export type EmojiMode = 'auto' | 'on' | 'off'

export interface EmojiEnv {
  isTTY: boolean
  env: Record<string, string | undefined>
}

export function resolveEmojiMode(flag: EmojiMode, runtime: EmojiEnv): boolean {
  if (flag === 'on') return true
  if (flag === 'off') return false
  return runtime.isTTY
      && !runtime.env.CI
      && !runtime.env.NO_EMOJI
      && runtime.env.TERM !== 'dumb'
}

export function detectRuntime(): EmojiEnv {
  return {
    isTTY: Boolean(process.stdout.isTTY),
    env: process.env
  }
}
```

- [ ] **Step 4: Run test → expect PASS**

Run: `npx vitest run tests/ui/emoji-mode.test.ts`
Expected: PASS (7 tests).

- [ ] **Step 5: Write failing test for `render.ts`**

`tests/ui/render.test.ts`:
```typescript
import { describe, it, expect } from 'vitest'
import { renderEvent, renderBanner } from '../../src/ui/render.js'

describe('renderEvent', () => {
  it('renders thought with emoji', () => {
    const out = renderEvent({ kind: 'thought', text: 'thinking' }, { useEmoji: true, useColor: false })
    expect(out).toContain('🤔')
    expect(out).toContain('thinking')
  })

  it('renders thought with symbol when no emoji', () => {
    const out = renderEvent({ kind: 'thought', text: 'thinking' }, { useEmoji: false, useColor: false })
    expect(out).toContain('◆')
    expect(out).not.toContain('🤔')
  })

  it('renders tool_use Write with ✍️ + green', () => {
    const out = renderEvent({ kind: 'tool_use', name: 'Write', input: { file_path: 'a.md' } }, { useEmoji: true, useColor: false })
    expect(out).toContain('✍️')
    expect(out).toContain('a.md')
  })

  it('renders tool_use Read with 🛠️', () => {
    const out = renderEvent({ kind: 'tool_use', name: 'Read', input: { path: 'src/x.py' } }, { useEmoji: true, useColor: false })
    expect(out).toContain('🛠️')
    expect(out).toContain('Read')
  })

  it('renders error tool_result with 👀 (color marks error, not emoji)', () => {
    const out = renderEvent({ kind: 'tool_result', output: 'fail', isError: true }, { useEmoji: true, useColor: false })
    expect(out).toContain('👀')
    expect(out).toContain('fail')
  })
})

describe('renderBanner', () => {
  it('start banner with emoji', () => {
    const out = renderBanner('start', { path: '/tmp/r' }, { useEmoji: true, useColor: false })
    expect(out).toContain('🚌')
    expect(out).toContain('/tmp/r')
  })

  it('done banner with symbol fallback', () => {
    const out = renderBanner('done', { wikiPath: '.codebus/wiki' }, { useEmoji: false, useColor: false })
    expect(out).toContain('✓')
    expect(out).not.toContain('🎉')
  })
})
```

- [ ] **Step 6: Run test → expect FAIL**

Run: `npx vitest run tests/ui/render.test.ts`
Expected: FAIL — module not found.

- [ ] **Step 7: Implement `src/ui/render.ts`**

```typescript
import chalk from 'chalk'
import type { StreamEvent } from '../infra/llm/types.js'

export interface RenderOptions {
  useEmoji: boolean
  useColor: boolean
}

const EMOJI = {
  thought: '🤔',
  tool: '🛠️ ',
  write: '✍️ ',
  result: '👀',
  start: '🚌',
  goal: '🎯',
  done: '🎉',
  hint: '💡'
}

const SYMBOL = {
  thought: '◆',
  tool: '→',
  write: '+',
  result: '←',
  start: '▶',
  goal: '◎',
  done: '✓',
  hint: 'i'
}

function lead(key: keyof typeof EMOJI, useEmoji: boolean): string {
  return useEmoji ? EMOJI[key] : SYMBOL[key]
}

function colored(text: string, color: 'cyan' | 'green' | 'dim' | 'red', useColor: boolean): string {
  if (!useColor) return text
  return chalk[color](text)
}

export function renderEvent(event: StreamEvent, opts: RenderOptions): string {
  switch (event.kind) {
    case 'thought':
      return `${lead('thought', opts.useEmoji)} ${colored('[Agent 思考]', 'dim', opts.useColor)} ${event.text}`
    case 'tool_use': {
      if (event.name === 'Write' || event.name === 'Edit') {
        const fp = (event.input as any)?.file_path ?? '(unknown)'
        return `${lead('write', opts.useEmoji)} ${colored('[正在生成]', 'green', opts.useColor)} ${fp}`
      }
      return `${lead('tool', opts.useEmoji)} ${colored('[呼叫工具]', 'cyan', opts.useColor)} ${event.name}(${JSON.stringify(event.input).slice(0, 80)})`
    }
    case 'tool_result': {
      const color = event.isError ? 'red' : 'dim'
      return `${lead('result', opts.useEmoji)} ${colored('[觀察結果]', color, opts.useColor)} ${event.output.slice(0, 200)}`
    }
    case 'done':
      return ''
  }
}

type BannerKind = 'start' | 'goal' | 'done' | 'hint'
type BannerData = Record<string, string>

export function renderBanner(kind: BannerKind, data: BannerData, opts: RenderOptions): string {
  const sym = lead(kind, opts.useEmoji)
  switch (kind) {
    case 'start': return `${sym} CodeBus 啟動！正在駛入 ${data.path} ...`
    case 'goal': return `${sym} 任務目標：${data.goal}`
    case 'done': return `${sym} 完成。wiki 已生成於 ${data.wikiPath}`
    case 'hint': return `${sym} 請用 Obsidian 開 ${data.path}`
  }
}
```

- [ ] **Step 8: Run test → expect PASS**

Run: `npx vitest run tests/ui/render.test.ts`
Expected: PASS (7 tests).

- [ ] **Step 9: Commit**

```bash
git add src/ui tests/ui
git commit -m "feat(ui): add emoji-mode detection and event/banner renderers"
```

---

## Task 11.5: infra/global-config (load ~/.codebus/config.yaml)

**Files:**
- Create: `src/infra/global-config.ts`
- Create: `tests/infra/global-config.test.ts`

- [ ] **Step 1: Write failing test**

`tests/infra/global-config.test.ts`:
```typescript
import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest'
import { mkdtempSync, rmSync, mkdirSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { loadGlobalConfig } from '../../../src/infra/global-config.js'

describe('loadGlobalConfig', () => {
  let home: string
  beforeEach(() => {
    home = mkdtempSync(join(tmpdir(), 'codebus-home-'))
    vi.stubEnv('HOME', home)
    vi.stubEnv('USERPROFILE', home)  // Windows
  })
  afterEach(() => {
    vi.unstubAllEnvs()
    rmSync(home, { recursive: true, force: true })
  })

  it('returns empty config when ~/.codebus/config.yaml does not exist', async () => {
    const cfg = await loadGlobalConfig()
    expect(cfg).toEqual({})
  })

  it('parses valid emoji setting', async () => {
    mkdirSync(join(home, '.codebus'))
    writeFileSync(join(home, '.codebus', 'config.yaml'), 'emoji: off\n')
    const cfg = await loadGlobalConfig()
    expect(cfg.emoji).toBe('off')
  })

  it('returns empty + warns on invalid yaml', async () => {
    const warn = vi.spyOn(console, 'warn').mockImplementation(() => {})
    mkdirSync(join(home, '.codebus'))
    writeFileSync(join(home, '.codebus', 'config.yaml'), '{{{ broken yaml')
    const cfg = await loadGlobalConfig()
    expect(cfg).toEqual({})
    expect(warn).toHaveBeenCalled()
    warn.mockRestore()
  })

  it('silently ignores unknown fields (forward-compat for phase 2)', async () => {
    mkdirSync(join(home, '.codebus'))
    writeFileSync(
      join(home, '.codebus', 'config.yaml'),
      'emoji: on\ndefault_provider: anthropic-sdk\napi_keys:\n  anthropic: sk-xxx\n'
    )
    const cfg = await loadGlobalConfig()
    expect(cfg.emoji).toBe('on')
    expect((cfg as any).default_provider).toBeUndefined()
  })

  it('rejects unknown emoji value with warning', async () => {
    mkdirSync(join(home, '.codebus'))
    writeFileSync(join(home, '.codebus', 'config.yaml'), 'emoji: maybe\n')
    const warn = vi.spyOn(console, 'warn').mockImplementation(() => {})
    const cfg = await loadGlobalConfig()
    expect(cfg.emoji).toBeUndefined()
    expect(warn).toHaveBeenCalled()
    warn.mockRestore()
  })
})
```

- [ ] **Step 2: Run test → expect FAIL**

Run: `npx vitest run tests/infra/global-config.test.ts`
Expected: FAIL — module not found.

- [ ] **Step 3: Implement `src/infra/global-config.ts`**

```typescript
import { existsSync } from 'node:fs'
import { readFile } from 'node:fs/promises'
import { homedir } from 'node:os'
import { join } from 'node:path'
import yaml from 'js-yaml'

export interface GlobalConfig {
  emoji?: 'auto' | 'on' | 'off'
}

const VALID_EMOJI = ['auto', 'on', 'off'] as const

function pickKnownFields(parsed: unknown): GlobalConfig {
  const out: GlobalConfig = {}
  if (!parsed || typeof parsed !== 'object') return out
  const data = parsed as Record<string, unknown>
  if ('emoji' in data) {
    const v = data.emoji
    if (typeof v === 'string' && (VALID_EMOJI as readonly string[]).includes(v)) {
      out.emoji = v as GlobalConfig['emoji']
    } else {
      console.warn(
        `codebus: ignoring invalid emoji value '${String(v)}' in ~/.codebus/config.yaml ` +
        `(must be auto|on|off)`
      )
    }
  }
  // Phase 2 fields (default_provider / api_keys / token_usage_log) are
  // silently ignored here — forward-compat so user can pre-fill them.
  return out
}

export async function loadGlobalConfig(): Promise<GlobalConfig> {
  const path = join(homedir(), '.codebus', 'config.yaml')
  if (!existsSync(path)) return {}
  let raw: string
  try {
    raw = await readFile(path, 'utf8')
  } catch {
    return {}
  }
  let parsed: unknown
  try {
    parsed = yaml.load(raw)
  } catch (err) {
    console.warn(
      `codebus: failed to parse ~/.codebus/config.yaml — using defaults ` +
      `(${(err as Error).message})`
    )
    return {}
  }
  return pickKnownFields(parsed)
}
```

- [ ] **Step 4: Run test → expect PASS**

Run: `npx vitest run tests/infra/global-config.test.ts`
Expected: PASS (5 tests).

- [ ] **Step 5: Commit**

```bash
git add src/infra/global-config.ts tests/infra/global-config.test.ts
git commit -m "feat(infra): add global config loader (~/.codebus/config.yaml)"
```

---

## Task 12: ui/stream-parser (claude-cli stream-json → StreamEvent)

**Files:** (note: actual parsing happens inside `claude-cli.ts` Task 10. This task extracts the line-by-line parser as a pure module so it's independently testable.)

This is already covered inline in Task 10's `mapClaudeStreamLine`. Refactor to extract:

- Modify: `src/infra/llm/claude-cli.ts` — extract `mapClaudeStreamLine` to a new file
- Create: `src/ui/stream-parser.ts`
- Create: `tests/ui/stream-parser.test.ts`

- [ ] **Step 1: Write failing test for the extracted parser**

`tests/ui/stream-parser.test.ts`:
```typescript
import { describe, it, expect } from 'vitest'
import { parseClaudeStreamLine } from '../../src/ui/stream-parser.js'

describe('parseClaudeStreamLine', () => {
  it('parses content_block_delta as thought', () => {
    const line = JSON.stringify({ type: 'stream_event', event: { type: 'content_block_delta', delta: { text: 'hello' } } })
    expect(parseClaudeStreamLine(line)).toEqual({ kind: 'thought', text: 'hello' })
  })

  it('parses tool_use', () => {
    const line = JSON.stringify({ type: 'stream_event', event: { type: 'tool_use', name: 'Read', input: { path: 'a' } } })
    expect(parseClaudeStreamLine(line)).toEqual({ kind: 'tool_use', name: 'Read', input: { path: 'a' } })
  })

  it('parses tool_result success', () => {
    const line = JSON.stringify({ type: 'stream_event', event: { type: 'tool_result', content: [{ text: 'ok' }] } })
    expect(parseClaudeStreamLine(line)).toEqual({ kind: 'tool_result', output: 'ok', isError: false })
  })

  it('parses tool_result error', () => {
    const line = JSON.stringify({ type: 'stream_event', event: { type: 'tool_result', content: 'fail', is_error: true } })
    expect(parseClaudeStreamLine(line)).toEqual({ kind: 'tool_result', output: 'fail', isError: true })
  })

  it('returns null for session_init / unknown types', () => {
    expect(parseClaudeStreamLine(JSON.stringify({ type: 'session_init' }))).toBe(null)
    expect(parseClaudeStreamLine(JSON.stringify({ type: 'result_summary' }))).toBe(null)
  })

  it('returns null for empty content_block_delta', () => {
    const line = JSON.stringify({ type: 'stream_event', event: { type: 'content_block_delta', delta: { text: '' } } })
    expect(parseClaudeStreamLine(line)).toBe(null)
  })
})
```

- [ ] **Step 2: Run test → expect FAIL**

Run: `npx vitest run tests/ui/stream-parser.test.ts`
Expected: FAIL — module not found.

- [ ] **Step 3: Create `src/ui/stream-parser.ts` (move logic from claude-cli.ts)**

```typescript
import type { StreamEvent } from '../infra/llm/types.js'

export function parseClaudeStreamLine(rawLine: string): StreamEvent | null {
  let parsed: any
  try { parsed = JSON.parse(rawLine) } catch { return null }
  if (parsed.type === 'stream_event' && parsed.event?.type === 'content_block_delta') {
    const text = parsed.event.delta?.text ?? ''
    return text ? { kind: 'thought', text } : null
  }
  if (parsed.type === 'stream_event' && parsed.event?.type === 'tool_use') {
    return { kind: 'tool_use', name: parsed.event.name, input: parsed.event.input }
  }
  if (parsed.type === 'stream_event' && parsed.event?.type === 'tool_result') {
    const content = Array.isArray(parsed.event.content)
      ? parsed.event.content.map((c: any) => c.text ?? '').join('')
      : String(parsed.event.content ?? '')
    return { kind: 'tool_result', output: content, isError: Boolean(parsed.event.is_error) }
  }
  return null
}
```

- [ ] **Step 4: Refactor `src/infra/llm/claude-cli.ts` to import the parser**

Replace inline `mapClaudeStreamLine` with import + delete the local copy:
```typescript
import { parseClaudeStreamLine } from '../../ui/stream-parser.js'
// ... in the for-await loop, replace `mapClaudeStreamLine(parsed)` with:
//   const event = parseClaudeStreamLine(line)
// (and remove JSON.parse — parser handles it)
```

- [ ] **Step 5: Run all tests → expect PASS**

Run: `npx vitest run`
Expected: all tests PASS (Task 10 still green after refactor).

- [ ] **Step 6: Commit**

```bash
git add src/ui/stream-parser.ts src/infra/llm/claude-cli.ts tests/ui/stream-parser.test.ts
git commit -m "refactor(ui): extract stream-parser from claude-cli for independent testing"
```

---

## Task 13: schema/claude-md (built-in template content)

**Files:**
- Create: `src/schema/claude-md.ts`
- Create: `tests/schema/claude-md.test.ts`

- [ ] **Step 1: Write failing test**

`tests/schema/claude-md.test.ts`:
```typescript
import { describe, it, expect } from 'vitest'
import { CODEBUS_SCHEMA_MARKDOWN } from '../../src/schema/claude-md.js'

describe('CODEBUS_SCHEMA_MARKDOWN', () => {
  it('contains SPDX license header', () => {
    expect(CODEBUS_SCHEMA_MARKDOWN).toContain('SPDX-License-Identifier: MIT')
  })

  it('contains all 12 schema sections', () => {
    const sections = [
      'Your Role', 'Workspace Layout', 'Wiki Structure',
      'Workflow per Goal', 'Page Conflict', 'Frontmatter Schema',
      'WikiLinks', 'Source', 'Stopping Criteria',
      'Failure Modes', 'Output Format', 'Workflow per Query'
    ]
    for (const s of sections) {
      expect(CODEBUS_SCHEMA_MARKDOWN).toContain(s)
    }
  })

  it('warns LLM about wikilink YAML quoting requirement', () => {
    expect(CODEBUS_SCHEMA_MARKDOWN).toContain('"[[')
    expect(CODEBUS_SCHEMA_MARKDOWN).toMatch(/quote|引號/)
  })
})
```

- [ ] **Step 2: Run test → expect FAIL**

Run: `npx vitest run tests/schema/claude-md.test.ts`
Expected: FAIL — module not found.

- [ ] **Step 3: Implement `src/schema/claude-md.ts` (the built-in schema content)**

```typescript
export const CODEBUS_SCHEMA_MARKDOWN = `<!--
SPDX-License-Identifier: MIT
Codebus built-in schema (CLAUDE.md). Generated by codebus init.
This file teaches the LLM how to maintain the wiki under this folder.
-->

# CodeBus Wiki Schema

You are codebus's wiki maintainer. Your job is to read the user's codebase
(under \`raw/code/\`) and incrementally build a structured markdown wiki
under \`wiki/\` that helps engineers ramp up on the codebase.

## 1. Your Role

- Goals: build / update wiki pages, maintain index.md / log.md / overview.md,
  produce per-goal reading guide in goals/.
- Non-goals: do NOT modify source code, do NOT write outside wiki/, do NOT
  invoke shell commands or web fetch (Bash/WebFetch/WebSearch are disallowed).

## 2. Workspace Layout

- You can READ: \`raw/code/\` (codebase snapshot), \`wiki/\` (existing wiki).
- You can WRITE: \`wiki/**/*.md\` only.
- You must NOT touch: \`raw/\` (read-only), \`output/\` (phase 1 unused),
  \`goals.jsonl\` (codebus internal), \`.git/\` (auto-managed).
- Note: \`raw/\` may contain sibling folders (\`docs/\`, \`clips/\` etc.)
  added by the user — those are also read-only references but phase 1
  workflow focuses on \`raw/code/\`.

## 3. Wiki Structure

Four special files:
- \`wiki/overview.md\` — repo-level overview, cross-goal, rewrite each run
- \`wiki/index.md\` — page catalog with summaries, rewrite each run
- \`wiki/log.md\` — chronological append: \`## [YYYY-MM-DD] goal: "X" → covers [[A]], [[B]]\`
- \`wiki/goals/<slug>.md\` — per-goal reading guide

Plus:
- \`wiki/pages/<slug>.md\` — knowledge units (cross-goal assets)

## 4. Workflow per Goal (Ingest)

1. **Discover**: grep wiki/pages/*.md frontmatter \`sources:\` to see what
   raw files are already indexed. Read wiki/index.md for the catalog.
2. **Plan**: list pages to update vs new pages to create.
3. **Explore**: use Read/Grep/Glob on raw/code/ for source files not yet covered.
4. **Write**: create or update wiki/pages/<slug>.md with frontmatter (§6).
5. **Index**: rewrite wiki/index.md catalog.
6. **Log**: append a line to wiki/log.md.
7. **Guide**: write wiki/goals/<slug>.md as the reading guide for this goal.

## 5. Page Conflict

- Page does not exist → create with frontmatter + body.
- Page exists → add a new \`## from goal: <X> (YYYY-MM-DD)\` section at the
  end of body. Do not modify existing sections.
- Frontmatter array fields (sources, goals, related) → union, no duplicates.
- Locked fields: \`title\`, \`type\`, \`created\` — never change.
- Update \`updated\` to today.

## 6. Frontmatter Schema (per page)

\`\`\`yaml
---
title: Payment Gateway
type: concept                    # concept | module | process | entity
sources:
  # path = source repo logical path, do NOT include raw/code/ prefix.
  # Read via: raw/code/<path> (codebus prepends automatically for stale check).
  - path: src/services/payment.py
    sha256: <40-hex>
    at_commit: <git-sha-or-empty>
goals:
  - "了解結帳流程"
created: '2026-05-04'
updated: '2026-05-04'
related:
  - "[[checkout-flow]]"
stale: false
---
\`\`\`

## 7. WikiLinks Convention

- Link to other pages by slug: \`[[payment-gateway]]\` (NOT a path).
- In YAML lists you MUST quote each wikilink string:
  \`related: ["[[a]]", "[[b]]"]\` — do not write \`related: [[a]], [[b]]\`
  (that breaks YAML).
- In body text wikilinks need no quoting.

## 8. Source Code References

- Frontmatter \`sources\` list each raw file you read for this page.
- In body, cite source code with fenced code blocks and a path comment:

\`\`\`python
# from src/services/payment.py
class PaymentGateway: ...
\`\`\`

## 9. Stopping Criteria

- Step budget: aim for ≤ 30 ReAct steps per goal.
- Stay within scope of the goal text — don't explore tangential modules.
- When you have enough sources to write a coherent reading guide, stop
  exploring and start writing.

## 10. Failure Modes

- Read fails (file missing / encoding) → log it, skip, continue.
- Write fails (path outside wiki/) → log it, skip.
- Do not retry the same operation infinitely.

## 11. Output Format

Your thoughts, tool calls, and tool results stream back to the user via
stream-json events. Codebus renders them as emoji-prefixed terminal output
("🤔 [Agent 思考]" / "🛠️ [呼叫工具]" / "✍️ [正在生成]" / "👀 [觀察結果]").
Write thoughts in clear user-facing language.

## 12. Workflow per Query

1. **Read index**: read wiki/index.md to see what pages exist.
2. **Identify pages**: pick the relevant wiki/pages/*.md for the question.
3. **Read pages**: Read those page files.
4. **Answer + cite**: produce a final answer that cites pages by
   \`[[wikilink]]\`. **Do NOT write any files** in query mode.
`
```

- [ ] **Step 4: Run test → expect PASS**

Run: `npx vitest run tests/schema/claude-md.test.ts`
Expected: PASS (3 tests).

- [ ] **Step 5: Commit**

```bash
git add src/schema/claude-md.ts tests/schema/claude-md.test.ts
git commit -m "feat(schema): add built-in CLAUDE.md schema template (12 sections)"
```

---

## Task 14: commands/init

**Files:**
- Create: `src/commands/init.ts`
- Create: `tests/commands/init.test.ts`

- [ ] **Step 1: Write failing test**

`tests/commands/init.test.ts`:
```typescript
import { describe, it, expect, beforeEach, afterEach } from 'vitest'
import { mkdtempSync, rmSync, existsSync, readFileSync, writeFileSync } from 'node:fs'
import { execSync } from 'node:child_process'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { runInit } from '../../src/commands/init.js'

describe('runInit', () => {
  let dir: string
  beforeEach(() => {
    dir = mkdtempSync(join(tmpdir(), 'codebus-init-'))
    execSync('git init -q -b main', { cwd: dir })
    execSync('git config user.email "t@t.com" && git config user.name "T"', { cwd: dir })
    writeFileSync(join(dir, 'README.md'), 'hi')
    execSync('git add . && git commit -q -m init', { cwd: dir })
  })
  afterEach(() => { rmSync(dir, { recursive: true, force: true }) })

  it('creates .codebus/ with all subdirs and files', async () => {
    await runInit(dir)
    expect(existsSync(join(dir, '.codebus'))).toBe(true)
    expect(existsSync(join(dir, '.codebus', '.git'))).toBe(true)
    expect(existsSync(join(dir, '.codebus', 'CLAUDE.md'))).toBe(true)
    expect(existsSync(join(dir, '.codebus', 'goals.jsonl'))).toBe(true)
    expect(existsSync(join(dir, '.codebus', 'wiki', 'pages'))).toBe(true)
    expect(existsSync(join(dir, '.codebus', 'wiki', 'goals'))).toBe(true)
    expect(existsSync(join(dir, '.codebus', 'raw'))).toBe(true)
    expect(existsSync(join(dir, '.codebus', 'raw', 'code'))).toBe(true)
    expect(existsSync(join(dir, '.codebus', 'output'))).toBe(true)
  })

  it('adds .codebus to source repo .gitignore (creating it if missing)', async () => {
    await runInit(dir)
    const gi = readFileSync(join(dir, '.gitignore'), 'utf8')
    expect(gi).toContain('.codebus')
  })

  it('does not duplicate .codebus entry if already in .gitignore', async () => {
    writeFileSync(join(dir, '.gitignore'), 'node_modules\n.codebus\n')
    await runInit(dir)
    const gi = readFileSync(join(dir, '.gitignore'), 'utf8')
    const matches = gi.match(/^\.codebus$/gm) ?? []
    expect(matches.length).toBe(1)
  })

  it('is idempotent — running twice does not error', async () => {
    await runInit(dir)
    await runInit(dir)
    expect(existsSync(join(dir, '.codebus'))).toBe(true)
  })

  it('skips .gitignore mutation when source repo is not git', async () => {
    const nonGit = mkdtempSync(join(tmpdir(), 'codebus-nongit-'))
    await runInit(nonGit)
    expect(existsSync(join(nonGit, '.gitignore'))).toBe(false)
    expect(existsSync(join(nonGit, '.codebus'))).toBe(true)
    rmSync(nonGit, { recursive: true, force: true })
  })
})
```

- [ ] **Step 2: Run test → expect FAIL**

Run: `npx vitest run tests/commands/init.test.ts`
Expected: FAIL — module not found.

- [ ] **Step 3: Implement `src/commands/init.ts`**

```typescript
import { existsSync } from 'node:fs'
import { mkdir, writeFile, readFile, appendFile } from 'node:fs/promises'
import { join } from 'node:path'
import { vaultPaths } from '../core/vault/layout.js'
import { initNestedRepo, autoCommit } from '../infra/git/nested-repo.js'
import { CODEBUS_SCHEMA_MARKDOWN } from '../schema/claude-md.js'

export async function runInit(repoRoot: string): Promise<void> {
  const p = vaultPaths(repoRoot)

  // Create folder structure
  await mkdir(p.root, { recursive: true })
  await mkdir(p.raw, { recursive: true })
  await mkdir(p.rawCode, { recursive: true })
  await mkdir(p.wiki, { recursive: true })
  await mkdir(p.wikiPages, { recursive: true })
  await mkdir(p.wikiGoals, { recursive: true })
  await mkdir(p.output, { recursive: true })

  // Write CLAUDE.md schema (only if missing — never overwrite user customizations)
  if (!existsSync(p.schemaMd)) {
    await writeFile(p.schemaMd, CODEBUS_SCHEMA_MARKDOWN)
  }

  // Touch goals.jsonl
  if (!existsSync(p.goalsJsonl)) {
    await writeFile(p.goalsJsonl, '')
  }

  // Internal .gitignore inside .codebus/
  if (!existsSync(p.gitignore)) {
    await writeFile(p.gitignore, '.lock\nraw/code/\n')
  }

  // Init nested git repo
  await initNestedRepo(p.root)

  // Add .codebus to source repo .gitignore (only if source is a git repo)
  if (existsSync(join(repoRoot, '.git'))) {
    const giPath = join(repoRoot, '.gitignore')
    let content = ''
    if (existsSync(giPath)) content = await readFile(giPath, 'utf8')
    const lines = content.split('\n').map(l => l.trim())
    if (!lines.includes('.codebus')) {
      const ensureNl = content.length && !content.endsWith('\n') ? '\n' : ''
      await appendFile(giPath, `${ensureNl}.codebus\n`)
    }
  }

  // Initial commit if nothing committed yet
  await autoCommit(p.root, 'init: codebus vault')
}
```

- [ ] **Step 4: Run test → expect PASS**

Run: `npx vitest run tests/commands/init.test.ts`
Expected: PASS (5 tests).

- [ ] **Step 5: Commit**

```bash
git add src/commands/init.ts tests/commands/init.test.ts
git commit -m "feat(commands): add init command (vault scaffold + nested git + .gitignore mutation)"
```

---

## Task 15: commands/goal (full ingest sequence)

**Files:**
- Create: `src/commands/goal.ts`
- Create: `tests/commands/goal.test.ts` (uses fake LLMProvider)

- [ ] **Step 1: Write failing test (with mocked LLMProvider)**

`tests/commands/goal.test.ts`:
```typescript
import { describe, it, expect, beforeEach, afterEach } from 'vitest'
import { mkdtempSync, rmSync, writeFileSync, readFileSync, existsSync } from 'node:fs'
import { execSync } from 'node:child_process'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { runGoal } from '../../src/commands/goal.js'
import type { LLMProvider, InvokeOptions, StreamEvent } from '../../src/infra/llm/types.js'

class FakeProvider implements LLMProvider {
  async *invoke(_opts: InvokeOptions): AsyncIterable<StreamEvent> {
    yield { kind: 'thought', text: 'analyzing...' }
    yield { kind: 'done' }
  }
  cancel(): void {}
}

describe('runGoal', () => {
  let dir: string
  beforeEach(() => {
    dir = mkdtempSync(join(tmpdir(), 'codebus-goal-'))
    execSync('git init -q -b main', { cwd: dir })
    execSync('git config user.email "t@t.com" && git config user.name "T"', { cwd: dir })
    writeFileSync(join(dir, 'app.ts'), 'console.log("hi")')
    execSync('git add . && git commit -q -m init', { cwd: dir })
  })
  afterEach(() => { rmSync(dir, { recursive: true, force: true }) })

  it('runs init if needed, syncs raw, records goal, invokes provider, commits', async () => {
    const provider = new FakeProvider()
    const events: StreamEvent[] = []
    await runGoal({
      repoRoot: dir,
      goal: '了解 app.ts',
      provider,
      onEvent: (e) => events.push(e)
    })

    expect(existsSync(join(dir, '.codebus'))).toBe(true)
    expect(existsSync(join(dir, '.codebus', 'raw', 'code', 'app.ts'))).toBe(true)
    const goalsJsonl = readFileSync(join(dir, '.codebus', 'goals.jsonl'), 'utf8')
    expect(goalsJsonl).toContain('了解 app.ts')
    expect(goalsJsonl).toContain('"uncommitted":false')
    expect(events.length).toBeGreaterThan(0)
  })

  it('records uncommitted=true when working tree has changes', async () => {
    writeFileSync(join(dir, 'app.ts'), 'changed')
    await runGoal({ repoRoot: dir, goal: 'g', provider: new FakeProvider() })
    const goalsJsonl = readFileSync(join(dir, '.codebus', 'goals.jsonl'), 'utf8')
    expect(goalsJsonl).toContain('"uncommitted":true')
  })
})
```

- [ ] **Step 2: Run test → expect FAIL**

Run: `npx vitest run tests/commands/goal.test.ts`
Expected: FAIL — module not found.

- [ ] **Step 3: Implement `src/commands/goal.ts`**

```typescript
import { appendFile, readdir, readFile } from 'node:fs/promises'
import { join } from 'node:path'
import { vaultPaths } from '../core/vault/layout.js'
import { acquireLock, releaseLock } from '../core/vault/lock.js'
import { syncRepoToRaw } from '../infra/fs/raw-sync.js'
import { sha256File } from '../infra/fs/file-ops.js'
import { getSourceVersion } from '../infra/git/source-version.js'
import { autoCommit } from '../infra/git/nested-repo.js'
import { parsePage } from '../core/wiki/frontmatter.js'
import { detectStaleSources } from '../core/wiki/stale-detect.js'
import { runInit } from './init.js'
import type { LLMProvider, StreamEvent } from '../infra/llm/types.js'
import { existsSync } from 'node:fs'

export interface RunGoalOptions {
  repoRoot: string
  goal: string
  provider: LLMProvider
  onEvent?: (e: StreamEvent) => void
}

export async function runGoal(opts: RunGoalOptions): Promise<void> {
  const p = vaultPaths(opts.repoRoot)

  if (!existsSync(p.root)) await runInit(opts.repoRoot)

  const lock = await acquireLock(p.lock)
  try {
    // Sync raw
    await syncRepoToRaw(opts.repoRoot, p.rawCode)

    // Record source version
    const ver = await getSourceVersion(opts.repoRoot)
    const goalEntry = {
      goal: opts.goal,
      source_commit: ver.commit,
      uncommitted: ver.uncommitted,
      timestamp: new Date().toISOString()
    }
    await appendFile(p.goalsJsonl, JSON.stringify(goalEntry) + '\n')

    // Compose system prompt
    const schema = await readFile(p.schemaMd, 'utf8')
    const indexMd = existsSync(p.wikiIndex) ? await readFile(p.wikiIndex, 'utf8') : '(empty)'
    const systemPrompt = `${schema}\n\n# Current wiki index\n\n${indexMd}\n\n# Goal\n\n${opts.goal}`

    // Invoke LLM
    for await (const event of opts.provider.invoke({
      systemPrompt,
      userMessage: `Build/update the wiki for this goal: ${opts.goal}`,
      mode: 'ingest',
      cwd: opts.repoRoot,
      vaultRoot: p.root
    })) {
      opts.onEvent?.(event)
    }

    // Stale detect (post-LLM)
    await flagStalePages(p.wikiPages, p.rawCode)

    // Auto-commit nested git
    await autoCommit(p.root, `wiki: ${opts.goal}`)
  } finally {
    await releaseLock(lock)
  }
}

async function flagStalePages(pagesDir: string, rawCodeDir: string): Promise<void> {
  if (!existsSync(pagesDir)) return
  const files = await readdir(pagesDir)
  for (const f of files) {
    if (!f.endsWith('.md')) continue
    const fullPath = join(pagesDir, f)
    const content = await readFile(fullPath, 'utf8')
    let parsed
    try { parsed = parsePage(content) } catch { continue }
    const currentHashes = new Map<string, string>()
    for (const src of parsed.frontmatter.sources) {
      const rawPath = join(rawCodeDir, src.path)
      if (existsSync(rawPath)) {
        currentHashes.set(src.path, await sha256File(rawPath))
      }
    }
    const result = detectStaleSources(parsed.frontmatter, currentHashes)
    if (result.isStale !== parsed.frontmatter.stale) {
      const { serializePage } = await import('../core/wiki/frontmatter.js')
      const updated = serializePage(
        { ...parsed.frontmatter, stale: result.isStale },
        parsed.body
      )
      const { writeFile } = await import('node:fs/promises')
      await writeFile(fullPath, updated)
    }
  }
}
```

- [ ] **Step 4: Run test → expect PASS**

Run: `npx vitest run tests/commands/goal.test.ts`
Expected: PASS (2 tests).

- [ ] **Step 5: Commit**

```bash
git add src/commands/goal.ts tests/commands/goal.test.ts
git commit -m "feat(commands): add goal command (full ingest sequence)"
```

---

## Task 16: commands/query (read-only mode)

**Files:**
- Create: `src/commands/query.ts`
- Create: `tests/commands/query.test.ts`

- [ ] **Step 1: Write failing test**

`tests/commands/query.test.ts`:
```typescript
import { describe, it, expect, beforeEach, afterEach } from 'vitest'
import { mkdtempSync, rmSync, mkdirSync, writeFileSync } from 'node:fs'
import { execSync } from 'node:child_process'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { runQuery } from '../../src/commands/query.js'
import type { LLMProvider, InvokeOptions, StreamEvent } from '../../src/infra/llm/types.js'

class FakeProvider implements LLMProvider {
  receivedMode: string | null = null
  async *invoke(opts: InvokeOptions): AsyncIterable<StreamEvent> {
    this.receivedMode = opts.mode
    yield { kind: 'thought', text: 'searching wiki...' }
    yield { kind: 'done' }
  }
  cancel(): void {}
}

describe('runQuery', () => {
  let dir: string
  beforeEach(() => {
    dir = mkdtempSync(join(tmpdir(), 'codebus-query-'))
    execSync('git init -q -b main', { cwd: dir })
    execSync('git config user.email "t@t.com" && git config user.name "T"', { cwd: dir })
    writeFileSync(join(dir, 'a.txt'), 'x')
    execSync('git add . && git commit -q -m init', { cwd: dir })
  })
  afterEach(() => { rmSync(dir, { recursive: true, force: true }) })

  it('throws if .codebus/wiki/pages/ is empty (need --goal first)', async () => {
    await expect(
      runQuery({ repoRoot: dir, query: 'q', provider: new FakeProvider() })
    ).rejects.toThrow(/請先用 --goal/)
  })

  it('invokes provider with mode=query when wiki has pages', async () => {
    mkdirSync(join(dir, '.codebus', 'wiki', 'pages'), { recursive: true })
    writeFileSync(join(dir, '.codebus', 'wiki', 'pages', 'a.md'), '# a')
    writeFileSync(join(dir, '.codebus', 'wiki', 'index.md'), '- [[a]]')
    writeFileSync(join(dir, '.codebus', 'CLAUDE.md'), 'schema')
    const provider = new FakeProvider()
    await runQuery({ repoRoot: dir, query: '結帳怎麼跑', provider })
    expect(provider.receivedMode).toBe('query')
  })
})
```

- [ ] **Step 2: Run test → expect FAIL**

Run: `npx vitest run tests/commands/query.test.ts`
Expected: FAIL — module not found.

- [ ] **Step 3: Implement `src/commands/query.ts`**

```typescript
import { existsSync } from 'node:fs'
import { readdir, readFile } from 'node:fs/promises'
import { vaultPaths } from '../core/vault/layout.js'
import type { LLMProvider, StreamEvent } from '../infra/llm/types.js'

export interface RunQueryOptions {
  repoRoot: string
  query: string
  provider: LLMProvider
  onEvent?: (e: StreamEvent) => void
}

export async function runQuery(opts: RunQueryOptions): Promise<void> {
  const p = vaultPaths(opts.repoRoot)

  if (!existsSync(p.wikiPages)) {
    throw new Error('請先用 --goal 建 wiki (.codebus/wiki/pages/ 不存在)')
  }
  const files = await readdir(p.wikiPages)
  if (files.filter(f => f.endsWith('.md')).length === 0) {
    throw new Error('請先用 --goal 建 wiki (.codebus/wiki/pages/ 為空)')
  }

  const schema = existsSync(p.schemaMd) ? await readFile(p.schemaMd, 'utf8') : ''
  const indexMd = existsSync(p.wikiIndex) ? await readFile(p.wikiIndex, 'utf8') : '(empty)'
  const systemPrompt = `${schema}\n\n# Current wiki index\n\n${indexMd}\n\n# Mode: Query\n\nAnswer the user's question by reading wiki/pages/*.md. Cite pages using [[wikilink]]. Do NOT write any files.`

  for await (const event of opts.provider.invoke({
    systemPrompt,
    userMessage: opts.query,
    mode: 'query',
    cwd: opts.repoRoot,
    vaultRoot: p.root
  })) {
    opts.onEvent?.(event)
  }
}
```

- [ ] **Step 4: Run test → expect PASS**

Run: `npx vitest run tests/commands/query.test.ts`
Expected: PASS (2 tests).

- [ ] **Step 5: Commit**

```bash
git add src/commands/query.ts tests/commands/query.test.ts
git commit -m "feat(commands): add query command (read-only wiki Q&A)"
```

---

## Task 17: cli.ts (commander entry + dispatch)

**Files:**
- Create: `src/cli.ts`
- Create: `tests/cli.test.ts` (smoke — run with --help / --version)

- [ ] **Step 1: Implement `src/cli.ts`**

```typescript
#!/usr/bin/env node
import { Command } from 'commander'
import { runInit } from './commands/init.js'
import { runGoal } from './commands/goal.js'
import { runQuery } from './commands/query.js'
import { ClaudeCliProvider } from './infra/llm/claude-cli.js'
import { loadGlobalConfig } from './infra/global-config.js'
import { resolveEmojiMode, detectRuntime, type EmojiMode } from './ui/emoji-mode.js'
import { renderEvent, renderBanner } from './ui/render.js'

const program = new Command()
program
  .name('codebus')
  .description('Build an LLM wiki for any codebase via claude -p')
  .version('0.1.0')
  .option('--repo <path>', 'repo path (default: cwd)', process.cwd())
  .option('--goal <text>', 'build wiki for this goal')
  .option('--query <text>', 'ask the wiki a question')
  .option('--debug', 'verbose stream-json output')
  .option('--no-emoji', 'force symbol mode (disable emoji)')

program.parse()
const opts = program.opts()

async function main() {
  // Settings priority for emoji mode (per spec §17.3):
  //   1. CLI --no-emoji    2. NO_EMOJI env    3. ~/.codebus/config.yaml    4. 'auto'
  const globalCfg = await loadGlobalConfig()
  const emojiFlag: EmojiMode =
    opts.emoji === false ? 'off' :
    process.env.NO_EMOJI ? 'off' :
    (globalCfg.emoji ?? 'auto')
  const useEmoji = resolveEmojiMode(emojiFlag, detectRuntime())
  const useColor = process.stdout.isTTY && !process.env.NO_COLOR
  const renderOpts = { useEmoji, useColor }

  const repo = opts.repo

  if (!opts.goal && !opts.query) {
    console.log(renderBanner('start', { path: repo }, renderOpts))
    await runInit(repo)
    console.log(renderBanner('done', { wikiPath: `${repo}/.codebus/wiki` }, renderOpts))
    console.log(renderBanner('hint', { path: `${repo}/.codebus` }, renderOpts))
    return
  }

  const provider = new ClaudeCliProvider()
  const onEvent = (e: any) => {
    const line = renderEvent(e, renderOpts)
    if (line) console.log(line)
  }

  if (opts.goal) {
    console.log(renderBanner('start', { path: repo }, renderOpts))
    console.log(renderBanner('goal', { goal: opts.goal }, renderOpts))
    await runGoal({ repoRoot: repo, goal: opts.goal, provider, onEvent })
    console.log(renderBanner('done', { wikiPath: `${repo}/.codebus/wiki` }, renderOpts))
    console.log(renderBanner('hint', { path: `${repo}/.codebus` }, renderOpts))
  } else if (opts.query) {
    console.log(renderBanner('start', { path: repo }, renderOpts))
    await runQuery({ repoRoot: repo, query: opts.query, provider, onEvent })
  }
}

main().catch((err) => {
  console.error(`error: ${err.message}`)
  process.exit(1)
})
```

- [ ] **Step 2: Write smoke test for CLI argv parsing**

`tests/cli.test.ts`:
```typescript
import { describe, it, expect } from 'vitest'
import { execSync } from 'node:child_process'

describe('cli', () => {
  it('--version prints version', () => {
    const out = execSync('npx tsx src/cli.ts --version').toString()
    expect(out).toContain('0.1.0')
  })

  it('--help mentions all 3 main flags', () => {
    const out = execSync('npx tsx src/cli.ts --help').toString()
    expect(out).toContain('--repo')
    expect(out).toContain('--goal')
    expect(out).toContain('--query')
  })
})
```

- [ ] **Step 3: Run test → expect PASS**

Run: `npx vitest run tests/cli.test.ts`
Expected: PASS (2 tests).

- [ ] **Step 4: Run full test suite to confirm nothing broke**

Run: `npx vitest run`
Expected: all tests PASS.

- [ ] **Step 5: Commit**

```bash
git add src/cli.ts tests/cli.test.ts
git commit -m "feat(cli): add commander entry + dispatch + emoji/banner integration"
```

---

## Task 18: E2E smoke test (init only — no real LLM call)

**Files:**
- Create: `tests/e2e/init-smoke.test.ts`

- [ ] **Step 1: Write E2E init smoke**

`tests/e2e/init-smoke.test.ts`:
```typescript
import { describe, it, expect, beforeEach, afterEach } from 'vitest'
import { mkdtempSync, rmSync, existsSync, readFileSync } from 'node:fs'
import { execSync } from 'node:child_process'
import { tmpdir } from 'node:os'
import { join } from 'node:path'

describe('e2e: init', () => {
  let dir: string
  beforeEach(() => {
    dir = mkdtempSync(join(tmpdir(), 'codebus-e2e-'))
    execSync('git init -q -b main', { cwd: dir })
    execSync('git config user.email "t@t.com" && git config user.name "T"', { cwd: dir })
    require('node:fs').writeFileSync(join(dir, 'README.md'), 'hi')
    execSync('git add . && git commit -q -m init', { cwd: dir })
  })
  afterEach(() => { rmSync(dir, { recursive: true, force: true }) })

  it('runs `codebus --repo <dir>` end-to-end and creates .codebus vault', () => {
    execSync(`npx tsx src/cli.ts --repo "${dir}"`, { stdio: 'pipe' })
    expect(existsSync(join(dir, '.codebus'))).toBe(true)
    expect(existsSync(join(dir, '.codebus', '.git'))).toBe(true)
    expect(existsSync(join(dir, '.codebus', 'CLAUDE.md'))).toBe(true)
    const gi = readFileSync(join(dir, '.gitignore'), 'utf8')
    expect(gi).toContain('.codebus')
  })
})
```

- [ ] **Step 2: Run test → expect PASS**

Run: `npx vitest run tests/e2e/init-smoke.test.ts`
Expected: PASS (1 test).

- [ ] **Step 3: Run full suite**

Run: `npx vitest run`
Expected: all tests PASS, coverage ≥ 80%.

- [ ] **Step 4: Commit**

```bash
git add tests/e2e/init-smoke.test.ts
git commit -m "test(e2e): add init smoke test"
```

---

## Task 19: README finalize + npm publish prep

**Files:**
- Modify: `README.md`
- Verify: `package.json`, `LICENSE`, build output

- [ ] **Step 1: Expand `README.md`**

```markdown
# 🚌 CodeBus

> Build an LLM wiki for any codebase via `claude -p`. Browse with Obsidian.

[![npm version](https://img.shields.io/npm/v/codebus.svg)](https://www.npmjs.com/package/codebus)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)

## What it does

Point codebus at any codebase + give it a goal. It spawns `claude -p` to
explore the code and incrementally builds a structured markdown wiki under
`.codebus/wiki/`. Open `.codebus/` in Obsidian to browse with backlinks /
graph view / Dataview queries.

## Install

\`\`\`bash
# Prerequisite: install Anthropic Claude Code CLI first
npm install -g @anthropic-ai/claude-code

# Then install codebus
npm install -g codebus
\`\`\`

## Usage

\`\`\`bash
# 1. Initialize vault (creates .codebus/ in your repo, adds it to .gitignore)
codebus --repo /path/to/your/repo

# 2. Build wiki for a goal
codebus --repo /path/to/your/repo --goal "了解購物車結帳流程"

# 3. Ask the wiki a question (read-only)
codebus --repo /path/to/your/repo --query "PaymentGateway 怎麼處理失敗?"
\`\`\`

Open `<repo>/.codebus/` in Obsidian to browse the generated wiki.

## Flags

- \`--repo <path>\` — repo path (default: cwd)
- \`--goal <text>\` — build wiki for this goal
- \`--query <text>\` — ask the wiki (read-only)
- \`--debug\` — verbose stream-json output
- \`--no-emoji\` — symbol fallback for CI / log files (also \`NO_EMOJI=1\`)

## License

MIT — see [LICENSE](LICENSE).
```

- [ ] **Step 2: Verify build output**

Run: `npm run build`
Expected: `dist/` populated with .js + .d.ts files. No tsc errors.

Run: `node dist/cli.js --version`
Expected: prints `0.1.0`.

- [ ] **Step 3: Verify package contents (dry run)**

Run: `npm pack --dry-run`
Expected: lists `dist/`, `LICENSE`, `README.md`, `package.json` only (no src/, tests/, node_modules/).

- [ ] **Step 4: Verify all tests still pass**

Run: `npm run test`
Expected: all tests PASS, coverage ≥ 80%.

- [ ] **Step 5: Commit**

```bash
git add README.md
git commit -m "docs: finalize README with badges, install/usage, flags, license"
```

- [ ] **Step 6: (Manual, not in plan) Publish to npm when ready**

```bash
npm login                            # interactive
npm publish --access public          # only when v0.1.0 ready to ship
```

---

## Self-Review

**Spec coverage check:**

- ✅ §3.1 Architecture (codebus thin wrapper + claude -p + load global config) → Task 10 (claude-cli.ts) + Task 11.5 (global-config) + Task 17 (cli.ts dispatch)
- ✅ §3.5 Module Architecture (core/infra/ui hexagonal) → Tasks 2-13 reflect the layer separation; global-config in infra/ (Task 11.5)
- ✅ §4 Disk Layout (`.codebus/` vault with raw/code/) → Task 2 (vault layout, +rawCode field) + Task 14 (init mkdir raw/code/)
- ✅ §4.1 raw/code/ rationale → Task 7 (sync target = raw/code/)
- ✅ §5 CLI Surface (init / goal / query + --no-emoji + --debug + settings priority) → Task 17 (4-level emoji resolution chain)
- ✅ §6 Schema 12 sections + sources path note → Task 13 (CODEBUS_SCHEMA_MARKDOWN with raw/code/ refs)
- ✅ §7 Goal sequence (12 steps; sync to raw/code/) → Task 15 (runGoal with p.rawCode)
- ✅ §7.5 Query sequence → Task 16 (runQuery)
- ✅ §8 Stream-json → Terminal rendering (4+4 emoji + symbol fallback) → Tasks 11-12
- ✅ §9 Page conflict (append-merge + locked fields) → Task 5 (mergePage)
- ✅ §10 Sync (重 copy raw/code/ + commit hash + sha256 + stale flag) → Tasks 7 (raw-sync to rawCode), 8 (source-version), 6 (stale-detect), 15 (orchestrate)
- ✅ §11 License MIT + checklist → Task 1 (LICENSE) + Task 13 (SPDX header) + Task 19 (README badge)
- ✅ §12 LLM Wiki 借鑑 (clean-room ideas) → frontmatter-repair / page-merge / stale-detect / lock all reimplemented
- ✅ §13 Toolkit (+js-yaml for config parse) → Task 1 deps
- ✅ §14 Repo structure (+ infra/global-config.ts) → Task 1 + Task 2 onwards + Task 11.5
- ✅ §17 Global Settings (~/.codebus/config.yaml) → Task 11.5 (loadGlobalConfig) + Task 17 (priority resolution)
- ⚠️ §15 Open Questions are **deferred to implementation iteration** by design — not all need tasks (failure modes / chalk color tuning / demo repo selection / per-repo config phase 2)

**Placeholder scan:** Searched plan for "TBD" / "TODO" / "implement later" / "fill in" — none found. All steps have actual code.

**Type consistency check:**
- `LLMProvider.invoke` signature consistent across Tasks 9 / 10 / 15 / 16 / 17 (all use `InvokeOptions`)
- `StreamEvent` shape consistent: 4 kinds (`thought` / `tool_use` / `tool_result` / `done`)
- `ParsedPage` / `PageFrontmatter` consistent across Tasks 3 / 5 / 6 / 15
- `vaultPaths` keys consistent across Tasks 2 / 14 / 15 / 16 (raw + new rawCode field both present)
- `GlobalConfig.emoji` type matches `EmojiMode` ('auto' | 'on' | 'off') across Tasks 11 / 11.5 / 17

No issues found.

---

## End of Plan
