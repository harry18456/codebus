# `docs/superpowers/` — Phase 1 brainstorming archive

This directory holds **design-intent snapshots** from CodeBus phase 1 brainstorming and planning. It is a frozen, append-only archive — content here is **not maintained, not updated, and not authoritative** for current behavior.

## Role

| Folder | Purpose |
|---|---|
| `specs/` | Phase 1 design documents written before implementation began |
| `plans/` | Phase 1 implementation plans broken into tracked steps |
| `REVIEW_LESSONS.md` | Cross-phase lessons learned during phase 1 reviews — still useful as memory before a new spec/plan cycle |

The `specs/` and `plans/` files capture the brainstorming state at a single point in time. Implementation iterations (iter-1 through iter-9 of phase 1) refined the actual behavior beyond what these documents describe; that drift was an explicit, accepted outcome, not an oversight.

Each spec/plan file in `specs/` and `plans/` carries a starting-spec banner at the top:

> ⚠️ **Starting spec — design-intent snapshot from phase 1 brainstorming.** Source of truth for shipped behavior is `openspec/specs/`. Drift from current code is expected; do not implement from this file.

The banner is the load-bearing marker. Do not strip it.

## Source of truth

For shipped behavior, read **`openspec/specs/<capability>/spec.md`**. Capabilities currently specified there:

- `vault-init` — `.codebus/` layout, gitignore handling, nested git, schema seeding
- `wiki-ingest` — goal-driven agentic ingest pipeline, sandbox, source enrichment, stale detection, auto-commit
- `wiki-query` — read-only `--query` against an existing vault
- `wiki-lint` — auto-lint after ingest (soft mode) + standalone `--check` + 5-folder rules + nav-file body wikilink scan
- `terminal-output` — stream rendering, emoji-mode priority, lint report formatting

Cross-reference: `CLAUDE.md` §"Specs and review lessons" describes how this directory relates to `openspec/`.

## How to make a capability change

**All new capability work goes through the Spectra `/spectra-propose` workflow** — not by editing files in this directory:

1. `/spectra-propose <change-name>` — generates a change proposal, design (if needed), specs delta, and tasks under `openspec/changes/<name>/`
2. `/spectra-apply <change-name>` — implements the tasks, marking checkboxes in `tasks.md`
3. `/spectra-archive <change-name>` — applies the spec delta into `openspec/specs/` and moves the change folder under `openspec/changes/archive/`

`docs/superpowers/` does **not** receive updates from this loop. If a phase 1 spec line is contradicted by current behavior, the resolution is a new openspec change, not an inline edit here.

## Why retain the directory at all?

- **Provenance**: the design rationale for early decisions (sandbox model, stream-parser shape, schema taxonomy) lives nowhere else with the same depth.
- **`REVIEW_LESSONS.md` is cross-phase**: the iteration lessons (especially #10 on the `--tools` sandbox gate) actively inform new work and should be read before starting a new propose/apply cycle.
- **Audit trail**: the gap between phase 1 design intent and shipped behavior is itself documentation — visible drift is a feature, not a bug to hide.

## What NOT to do

- ❌ Edit `specs/` or `plans/` content to "fix" drift — fixing drift here erases the archival value and re-creates the maintenance burden the banner exists to prevent.
- ❌ Implement a feature from a `specs/` requirement without first checking `openspec/specs/` for the current spec.
- ❌ Strip the starting-spec banner.
- ❌ Add new spec/plan files here for a current capability change — use `/spectra-propose` instead.
