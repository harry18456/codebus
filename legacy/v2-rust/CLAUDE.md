# CLAUDE.md

> Claude Code reads this on project entry. For cross-tool / vendor-neutral instructions see [`AGENTS.md`](AGENTS.md). This file is the **Claude-specific adapter slot** — only put here what's truly Claude / Anthropic-specific.

## Project context

See [`AGENTS.md`](AGENTS.md) — that's the source of truth for project state, workflow, locale, and v3 day-1 architectural intent.

## Claude-specific notes

(Empty for now — v3 day-1. As Claude-specific quirks accumulate during development they go here, not in `AGENTS.md`.)

Examples of what would belong here when they arise:

- Claude Code-specific tool naming / sandbox flag references
- `anthropic-beta` header handling guidance
- Claude session vs `-p` mode distinctions when relevant

## Spectra

The project uses Spectra for spec-driven development. See [`AGENTS.md`](AGENTS.md) § "Development workflow" for the shared command list — same commands apply to Claude Code as any other tool.
