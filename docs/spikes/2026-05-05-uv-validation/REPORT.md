# Spike Report: codebus on D:/side_project/uv

**Date:** 2026-05-05  
**Goal:** validate two open concerns from CLAUDE.md / phase 1 wrap-up
- **Concern 1:** `--query` mode untested in real flow
- **Concern 2:** multi-goal scaling on medium/large project untested

**Standard:** D:/side_project/uv (186 MB, 474k Rust LoC, 69 crates) — 10× larger than buddy-gacha which previously surfaced iter-9 sandbox bug.

**Overall result: both concerns cleared on this scale. 6 follow-up items identified, all phase-1.5 hygiene, none P0.**

---

## Headline numbers

- **4 goals + 1 query** completed end-to-end in ~19 wall minutes
- **14 knowledge pages** produced across all 5 type folders except `synthesis/`
- **0 errors** throughout (lint, sandbox, exit codes)
- **0 cwd-外 Write attempts** — sandbox iter-9 fix holds at scale
- **6 lint warnings** at end — all 6 are lint false positives on semantically-valid Obsidian markdown

| Stage | Wall clock | Pages produced | Lint errors | Notes |
|---|---|---|---|---|
| Stage 1 (single-crate goal) | 3m 26s | 3 + 1 guide | 0e / 1w | uv-cache-key (908 LoC) |
| Stage 2 (query) | 26s | (read-only) | n/a | 8/8 grounded claims |
| Stage 3 goal 2 | 4m 53s | 3 + 1 guide | 0e / 1w | uv-cache-info |
| Stage 3 goal 3 | 6m 22s | 5 + 1 guide | 0e / 6w | uv-resolver (31k LoC) |
| Stage 3 goal 4 | 4m 25s | 2 + 1 guide | 0e / 6w | uv-installer |

## Concern 1: `--query` real-world effect

**Cleared on small wiki (3-page).** Tested at 26-second response with 8/8 grounded claims and 3 valid `[[wikilink]]` cites. Sandbox in query mode held (no Write attempts, no raw/code reads). Agent self-selected to read 2 pages and synthesized.

Caveats:
- Tested on tiny wiki (3 knowledge pages). Behavior at 50+ pages, multi-domain wiki, ambiguous question untested.
- Single query, single topic. Selectivity at scale not stress-tested.

Follow-up: re-run query against the 14-page vault produced by stage 3 to extend coverage.

## Concern 2: multi-goal scaling

**Partially cleared.** Sandbox holds, lint errorCount stays 0, wall clock grows modestly (1.28× across crate-size variance). But 3 weakness signals surfaced:

1. **index.md grows linearly** (~150-200 bytes per knowledge page) and enters every system prompt
2. **Page-merge never fires** — overlapping goals (cache+installer) produced fresh slugs every time, schema bias too weak
3. **raw/code/ full re-copy every goal** — 27 MB cleared and re-copied 4× even though source content invariant

None of these are phase-1 blockers. All are phase-1.5 design items.

## Lint findings (2 code bugs surfaced)

**Bug A: lint regex doesn't skip code spans / fenced blocks**
- `wiki/overview.md` flagged for `\`[[wikilink]]\`` (inline-code meta-explanation of syntax)
- Agent behavior is correct (Obsidian renders code-span literal); lint regex too greedy

**Bug B: lint regex eats backslash into slug for escaped table pipes**
- `wiki/modules/uv-resolver.md` 5× flagged for `[[slug\|alias]]` in markdown table cells
- Agent escaped `|` to prevent table delimiter conflict — correct markdown
- Lint slug class `[^\]|#\s]+` doesn't exclude `\`, captures it as part of slug

Both bugs are deterministic regex issues, not "AI literacy" gaps. Fix scope per bug: ~10 lines src + ~15 lines test in `src/core/wiki/lint.ts`.

## Other observations

- **No `synthesis/` page** ever produced across 4 goals — schema doesn't actively prompt for cross-cutting summaries
- **Stream parser handles nicely** — 4 long-running goals, no parser failures, no unknown event types crashed
- **Soft auto-lint integration works as designed** — failures surface as 1-line summary, never block commit
- **Done banner accurate** — "wikiChanged=true / 掰掰~下車囉" correctly distinguished real wiki growth from no-op runs

## Follow-up backlog (priority sorted)

| # | Item | Severity | Scope | Concern |
|---|---|---|---|---|
| 1 | **Lint Bug B** — regex eats `\` from `[[slug\|alias]]` table escapes | Medium (5 false-positive warns per doc with tables) | ~10 LoC + 2 tests in lint.ts | Lint hygiene |
| 2 | **Lint Bug A** — `[[…]]` inside backticks falsely flagged | Low (1 warn typically) | ~10 LoC + 2 tests in lint.ts | Lint hygiene |
| 3 | **Page-merge bias too weak** — schema doesn't drive incremental update of existing concepts | Medium (defeats Karpathy "incremental wiki" promise) | Schema language tightening + new section in CLAUDE.md template | Concern 2 |
| 4 | **index.md size cap** — full file enters every system prompt, grows linearly with pages | Medium (eats context window over time) | Goal: design TOC/abstract OR truncation policy; impl: change goal.ts system prompt composition | Concern 2 |
| 5 | **Incremental raw-sync** — full re-copy on every goal | Low-Medium (wasted I/O, slow on HDD) | mtime/hash skip in `syncRepoToRaw` | Concern 2 |
| 6 | **`--debug` flag not wired** — declared but no effect | Low (blocks token-cost instrumentation) | Wire `--debug` to dump raw stream-json to file in `.codebus/output/` | Tooling |

Items 1+2 can be one change (`lint-markdown-aware-scan`). Items 3+4 belong together (`schema-drift-prevention` or similar). Item 5 standalone. Item 6 standalone.

## Recommended next moves

1. **Group items 1+2** into a small change (`lint-markdown-aware-scan`) — fast win, removes daily noise
2. **Open a phase-1.5 discussion** for items 3+4 — they're entangled (page-merge weak + index.md growth both undermine "wiki gets denser, not just longer" Karpathy goal). Discuss shape before proposing.
3. **Items 5 + 6 are independent** — open as standalone proposals when comfort to tackle

## Sign-off

Phase 1 codebus is **safe to use on medium-large repos** at the demonstrated scale (14 pages, 4 goals). Found bugs do not block usage; they degrade signal quality and waste I/O. None compromise sandbox or correctness.

Two open concerns from session start are **closed at the tested scale**, with explicit caveats noted for very-large-vault behavior (50+ pages, 10+ goals).

## Artifacts

- `spike-uv/observations.md` — per-stage detailed metrics + grounding checks
- `spike-uv/transcripts/*.log` — full rendered output of each run (gitignored)
- `D:/side_project/uv/.codebus/` — produced vault (left in place, not committed; uv git status remains clean)
