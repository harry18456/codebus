# Changelog

Notable changes to codebus. Adheres to [Semantic Versioning](https://semver.org/).

Detailed per-change history lives in the archived proposals under
`openspec/changes/archive/` and in the git log; this file is the high-level summary.

## [Unreleased]

- Repo prepared for public release: pruned the v2 reference implementation and
  internal PoC scripts, reorganized `docs/` into an outward-facing top level plus
  `docs/internal/` working archive, fixed documentation drift.

## [3.0.x]

- v3 line. Rust workspace (CLI + Tauri desktop app) with a provider-neutral agent
  seam driving Claude Code or OpenAI Codex, an Obsidian-compatible per-repo vault,
  PII-redacted source mirror, and the `init` / `goal` / `query` / `fix` / `chat` /
  `quiz` / `lint` verbs. Codex + Azure OpenAI support and the desktop app shipped
  within this line.

(For granular history of 3.0.0 → 3.0.2, see `git log` and `openspec/changes/archive/`.)
