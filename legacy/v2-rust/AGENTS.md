# AGENTS.md

> Cross-tool instructions for AI coding agents working in this repo. Aligns with the [AGENTS.md](https://agents.md/) standard (Linux Foundation Agentic AI Foundation, 2025).

## Project: codebus v3

This is the **v3 rewrite** of codebus, a strategic pivot from "spawning `claude -p` to build wiki" to "**being a codebase memory layer for any agentic CLI**".

- **v2 frozen** at `v2-archive` branch (full git history) and `legacy/v2-rust/` (working-tree reference)
- **v1 frozen** at `legacy/ts-src/` (TypeScript prototype)
- v3 is **fresh start** — `main` branch has no carried git history before the v3 root commit

## Read these first

1. [`docs/strategy/2026-05-08-skill-vs-binary-pivot.md`](docs/strategy/2026-05-08-skill-vs-binary-pivot.md) — **strategic context**. Read §11.7 for v3 day-1 architectural intent (multi-vendor / MCP / skill-generator pre-wiring) and §11.6 short/mid/long-term roadmap.
2. [`docs/superpowers/REVIEW_LESSONS.md`](docs/superpowers/REVIEW_LESSONS.md) — cross-phase lessons carried from v1/v2 review iterations. Especially relevant: lessons #1, #8, #9, #10 (spike rigor, plan/spec convergence, sandbox testing).
3. `legacy/v2-rust/` — v2 Rust workspace. Useful for porting lint rules / schema definitions / token tracking semantics into v3 form when it's time.

## Development workflow

This repo uses **Spectra** for spec-driven development (config: `.spectra.yaml`, workspace: `openspec/`). Use these slash commands when invoked by user:

| Command | Use |
|---|---|
| `/spectra-discuss` | Unstructured exploration that needs framing |
| `/spectra-propose` | Create a change proposal with all required artifacts |
| `/spectra-apply` | Execute a parked change |
| `/spectra-ingest` | Update an in-progress change with new context |
| `/spectra-archive` | Archive a completed change |
| `/spectra-ask` | Query specs/documents to answer questions |

`tdd: true` and `audit: true` and `parallel_tasks: true` are all enabled in `.spectra.yaml`.

## Locale & code conventions

- User communicates in **zh-tw**. Reply in zh-tw; technical terms / code identifiers stay in original form.
- **Code comments and commit messages in en-us**.
- Dates in frontmatter / logs are **UTC `YYYY-MM-DD`** (no local-tz drift).
- Windows host environment; user shell is PowerShell. POSIX shell available via Bash tool.

## v3 day-1 architectural commitments

When designing v3 modules, get these right from day 1 (incrementally fill in implementations later):

- `AgenticProvider` trait (replaces v2's `LlmProvider`) — abstracts agentic CLIs, not bare LLM APIs. **No leaking of Claude-specific assumptions** (no `--tools` flag, no stream-json wrappers, no `anthropic-beta` headers in the trait surface).
- `schema/AGENTS.md` (neutral) + `schema/CLAUDE.md` / `schema/GEMINI.md` (per-vendor adapter) — split, not glued. v3 first release only needs Claude adapter implemented but **the split must exist on day 1**.
- `codebus mcp` subcommand position reserved — even if implementation deferred, the entry point in CLI dispatch and the internal query API shapes (`get_page` / `search_wiki` / `get_backlinks` / `list_pages_by_type`) must be designed.
- Cross-vendor extensibility — Cargo feature gates, `~/.codebus/config.yaml` schema, trait method signatures all assume "another vendor will be added later".

If you find yourself baking in Claude-specific assumption to save short-term effort, **pause and check the strategy memo §11.7 day-1 checklist**.

## Things that should NOT happen

- Don't carry v2 code into v3 implementation paths without questioning whether v3's abstraction layer differs (e.g. don't blindly translate v2's `ClaudeCliProvider::build_argv` into v3 — v3's `AgenticProvider` interface should be cleaner).
- Don't reintroduce `LlmProvider` naming or `tool_runtime` ambitions — those were v2 hypotheses, retired in §11 of the strategy memo.
- Don't recreate v2 Rust workspace structure 1-to-1 — v3 may stay Rust, may not. The pivot leaves implementation language open until first major design decision.

## Status snapshot

As of v3 day-1 (May 2026): the working tree only contains documentation, legacy reference, and an empty `openspec/` skeleton. **No code yet**. v3 implementation begins when the first `/spectra-propose` lands.
