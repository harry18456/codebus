# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

<!-- SPECTRA:START v1.0.2 -->

# Spectra Instructions

This project uses Spectra for Spec-Driven Development(SDD). Specs live in `openspec/specs/`, change proposals in `openspec/changes/`.

## Use `/spectra-*` skills when:

- A discussion needs structure before coding → `/spectra-discuss`
- User wants to plan, propose, or design a change → `/spectra-propose`
- Tasks are ready to implement → `/spectra-apply`
- There's an in-progress change to continue → `/spectra-ingest`
- User asks about specs or how something works → `/spectra-ask`
- Implementation is done → `/spectra-archive`
- Commit only files related to a specific change → `/spectra-commit`

## Workflow

discuss? → propose → apply ⇄ ingest → archive

- `discuss` is optional — skip if requirements are clear
- Requirements change mid-work? Plan mode → `ingest` → resume `apply`

## Parked Changes

Changes can be parked（暫存）— temporarily moved out of `openspec/changes/`. Parked changes won't appear in `spectra list` but can be found with `spectra list --parked`. To restore: `spectra unpark <name>`. The `/spectra-apply` and `/spectra-ingest` skills handle parked changes automatically.

<!-- SPECTRA:END -->

## What this project is

CodeBus uses agentic AI to help engineers ramp up on an unfamiliar codebase, by incrementally building a structured markdown wiki under `.codebus/wiki/` that captures concepts, modules, processes, entities, and synthesis as the user explores the repo with goal-driven prompts. The wiki design follows Karpathy's "LLM Wiki" pattern (`https://gist.github.com/karpathy/3ef7345f9192fe96d11a25fb1c40b35c`) adapted for code: 5 typed folders + cross-page wikilinks + goal-driven incremental growth, instead of one-shot RAG. The vault is **Obsidian-compatible by default** — open `.codebus/wiki/` in Obsidian for backlinks, graph view, and Dataview queries.

**Phase 1** delivers this via `claude -p` subprocess (Anthropic Claude Code CLI) — codebus orchestrates the run, parses stream-json, sandboxes the agent's toolset, and post-processes the output. **Phase 2 design intent** is to abstract over multiple LLM providers (direct Anthropic API, OpenAI, local models, etc.) so the agentic capability is not coupled to a single CLI. The provider boundary lives at `src/infra/llm/types.ts` (`LLMProvider` interface); current `ClaudeCliProvider` is the first implementation, future providers slot in alongside without touching `commands/`, `core/`, or `ui/`.

**Long-term goal — interactive tutorial mode:** Wiki + Obsidian is the read-only baseline. The eventual vision (prototyped in the `v1-archive` branch — see `docs/interactive-tutorial.md` there for the full UI spec) is an **interactive guided walkthrough**: a frontend parses the generated markdown into "stations" (slide-deck pages), with embedded `<Checkpoint>` / `<Quiz>` mdc components, a Q&A drawer (Cmd+K), and two view modes (投影片 vs 文件). The markdown stays plain-readable in any renderer — interactivity degrades gracefully when components aren't mounted. The Karpathy 5-folder pages produced today are the content layer that this future UI will consume; each `concepts/`, `processes/`, `synthesis/` page is candidate station content. Bridging from "wiki for Obsidian" → "tutorial app that teaches you the codebase" is what graduates codebus from a wiki generator to an actual onboarding tool.

## Common commands

```bash
npm run build          # tsc → dist/
npm run dev            # tsx src/cli.ts (skip build during iteration)
npm test               # vitest run, all 152 tests
npm test -- <pattern>  # filter, e.g. npm test -- tests/core/wiki/lint.test.ts
npm run test:coverage  # 80%+ target
codebus --repo X --check   # standalone read-only lint of an existing vault
```

A live test against another repo (e.g. `D:/side_project/buddy-gacha`) is the cheapest sanity check after touching ingest, sandbox, or schema — many phase-1 bugs only surfaced via real `claude -p` runs, not unit tests.

## Architecture

Hexagonal split — `core/` is pure, `infra/` owns side effects, `ui/` is rendering only, `commands/` orchestrates. Tests should respect this: `core/` tests use temp dirs only; `infra/llm/claude-cli.ts` accepts a `spawn` injection for fakes.

```
src/
├─ cli.ts                  Commander entry; SIGINT handler MUST come AFTER `const repo = opts.repo` (iter-8 TDZ trap)
├─ commands/               Orchestration: init, goal, query, check
├─ core/
│  ├─ vault/               layout (single source of truth for paths), lock, sanity-check (rejects --repo .codebus/etc.)
│  └─ wiki/                types, frontmatter (gray-matter wrapper), page-merge, lint, stale-detect
├─ infra/
│  ├─ fs/                  raw-sync (gitignore-aware copy), file-ops (sha256)
│  ├─ git/                 nested-repo (.codebus/.git auto-commit), source-version
│  ├─ llm/claude-cli.ts    claude subprocess provider; argv builder is the sandbox gate
│  ├─ cli-detect.ts        prerequisite check: claude CLI installed
│  └─ global-config.ts     ~/.codebus/config.yaml loader (tolerant)
├─ ui/                     stream-parser, render, emoji-mode (5-level priority), lint-report
└─ schema/claude-md.ts     The built-in CLAUDE.md written into each user vault — this IS the agent's system prompt
```

`src/core/vault/layout.ts` is the single source of truth for vault paths. Code that touches `.codebus/<sub>` must use `vaultPaths(repo)` not string concatenation. `wikiPageFolders` and `wikiTypeFolderMap` give iteration order / type→path lookup over the 5 folders below.

## Wiki structure (Karpathy 5-folder)

Knowledge pages live in 5 typed buckets, NOT a flat `wiki/pages/`. Folder name maps 1:1 to frontmatter `type`:

- `wiki/concepts/`   — cross-cutting ideas, principles
- `wiki/entities/`   — data structures, schemas
- `wiki/modules/`    — code organisation units
- `wiki/processes/`  — sequential workflows, lifecycles, ordered algorithms
- `wiki/synthesis/`  — cross-cutting summaries that integrate multiple pages

Plus 3 special files at root (`overview.md`, `index.md`, `log.md`) and `wiki/goals/<slug>.md` per-goal reading guides. All 4 categories (5 folders + 3 specials + goals/) are valid `[[wikilink]]` targets — lint catalogues all of them. The flat `wiki/pages/` scheme was removed in commit `6323971`; do not reintroduce it.

## Sandbox: `--tools` is the gate, NOT `--allowedTools` (iter-9 lesson)

`src/infra/llm/claude-cli.ts:buildArgv()` MUST emit BOTH:

- `--tools <list>`        — the actual toolset whitelist; tools not listed are not visible to the agent at all
- `--allowedTools <list>` — auto-approval list (mirrors `--tools` as a redundant safety net so future Claude Code permission-mode behaviour can't hang on a prompt with no terminal)

Phase 1 iter-1 ~ iter-8 wrongly believed `--allowedTools` alone was a sandbox; live test exposed Bash leaking through. `--allowedTools` only auto-approves; in `-p` mode + `acceptEdits`, unlisted tools are silently auto-approved too because there is no terminal to deny them. Full context in `docs/superpowers/specs/2026-05-04-codebus-v2-phase1-design.md` §3.2.4 and `docs/superpowers/REVIEW_LESSONS.md` lesson #10.

Other sandbox invariants worth knowing before refactoring `claude-cli.ts` or `commands/goal.ts`:

- `spawn cwd = .codebus/` is system-level isolation from the user's source repo (cwd-external Writes are denied even under `acceptEdits`). Do not change cwd or add `--add-dir` (it widens, not narrows).
- `goals.jsonl` is appended by codebus, not the agent — never source it from agent output.
- `enrichSourceMetadata()` only fills missing `sha256`/`at_commit`. Unconditionally rewriting them silently breaks `flagStalePages` (compares same-hash-vs-same-hash). Iter-8 broke this once.

## The built-in schema is code

`src/schema/claude-md.ts` exports `CODEBUS_SCHEMA_MARKDOWN`, a template-literal constant. `runInit()` writes it to `.codebus/CLAUDE.md` only when missing (preserves user customization). Editing this file changes the agent's system prompt — concept/process/synthesis taxonomy boundaries, slug discipline, out-of-scope detection, and refusal behaviour all live here. Tests in `tests/schema/claude-md.test.ts` lock in critical phrases so future edits don't accidentally drop them.

Existing user vaults won't pick up schema changes automatically (init won't overwrite). For local testing against another repo (e.g. `buddy-gacha`), manually overwrite the vault's `CLAUDE.md` after a schema edit, or `rm -rf .codebus/` for a clean restart.

## Stream / render contract

`src/ui/stream-parser.ts` parses Claude CLI stream-json events into a normalized `StreamEvent` discriminated union. The schema was rewritten in iter-8 after a live spike — earlier commits assumed a fictional `{type: "stream_event"}` wrapper that never exists. Real schema: `{type: "assistant"|"user"|"system"|"result", message: {...}}`. Parser returns `StreamEvent[]` because `assistant.content[]` can hold multiple items per line. Forward-compat: unknown event types are silently dropped (don't add throws).

## Specs and review lessons

`openspec/specs/` holds main capability specs (4 capabilities seeded from the codebus-v2-phase1 archive). Active changes go in `openspec/changes/<name>/`; archived ones in `openspec/changes/archive/YYYY-MM-DD-<name>/`. Use `/spectra-*` skills (see Spectra block above), not raw `spectra` CLI invocations.

`docs/superpowers/REVIEW_LESSONS.md` is cross-phase memory — read it before starting a new spec/plan/implementation cycle. Notable lessons:

- #1: Spike summaries must quote transcript lines, not paraphrase
- #8: Spec convergence ≠ plan convergence — plan code review iteration is mandatory
- #9: Late-stage reviewers misread diffs; re-read final file state for structural changes
- #10: Sandbox spike must measure "unwanted tools blocked", not just "wanted tools work"

## Date convention

Frontmatter `created` / `updated` are UTC `YYYY-MM-DD`. Use `utcTodayISO()` from `src/core/wiki/date.ts`, never `new Date().toLocaleDateString()` — cross-timezone drift breaks `flagStalePages` comparisons.
