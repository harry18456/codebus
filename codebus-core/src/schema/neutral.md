<!--
SPDX-License-Identifier: MIT
codebus built-in schema (vendor-neutral).
This file describes the structure rules for a codebus vault. The agent
working on the vault reads this file to know taxonomy, frontmatter, and
linking conventions. Verb-specific workflow lives in the corresponding
skill bundle (codebus-goal / codebus-query / codebus-fix), not here.
-->

# codebus Wiki Schema

The vault under `.codebus/` is structured to help engineers ramp up on
an unfamiliar codebase. This file documents the structure rules; the
codebus-goal / codebus-query / codebus-fix skill bundles describe the
workflows that produce and consume content under these rules.

## 1. Workspace Layout

- READ-only: `raw/code/` (PII-redacted mirror of source), `wiki/` (existing pages).
- WRITE: `wiki/**/*.md` only.
- DO NOT touch: `raw/` (read-only), `log/` (codebus internal),
  this schema file at the vault root (codebus owns it).

## 2. Wiki Structure

Two nav files at `wiki/` root:

- `wiki/index.md` — page catalog with summaries.
- `wiki/log.md` — chronological journal of goals; each entry covers
  goal text, covered pages by `[[wikilink]]`, suggested reading order,
  and key takeaways.

Knowledge pages live under **5 type buckets**:
`concept` / `entity` / `module` / `process` / `synthesis`.

Frontmatter `type` is the **authoritative** metadata. The same-named
folders under `wiki/` are an organizational hint for sidebar grouping
(codebus init pre-creates them so the sidebar is structured even when
empty), not a strict filing contract.

- `wiki/concepts/<slug>.md` — cross-cutting ideas, principles, mental
  models. WHAT something is or HOW it is organized at a static level.
- `wiki/entities/<slug>.md` — discrete data structures, records, schemas.
- `wiki/modules/<slug>.md` — code organization units, libraries, services.
- `wiki/processes/<slug>.md` — sequential workflows, state machines,
  lifecycles, AND **algorithms with ordered steps**. If the page
  describes things happening in a specific order, it is a process —
  not a concept.
- `wiki/synthesis/<slug>.md` — cross-cutting summaries that integrate
  multiple pages into one coherent view (architecture overview, main
  themes, how the modules fit together).

Wikilinks like `[[slug]]` resolve by filename regardless of folder, so
cross-folder linking just works.

**Concept vs process tiebreaker:** if you find yourself writing
"Step 1, Step 2, ..." or "first ... then ... finally ...", it is a
process. Concepts are statements of structure; processes are
sequences of action.

## 3. Page Conflict Rules

- Page does not exist → create with frontmatter + body in the right type folder.
- Page exists → add a new `## from goal: <X> (YYYY-MM-DD)` section at
  the end of body. Do not modify existing sections.
- Frontmatter array fields (sources, goals, related) → union, no duplicates.
- Locked fields: `title`, `type`, `created` — never change. Type is
  locked because it determines the folder; if you think the type is
  wrong, surface that thought rather than silently moving the file.
- Update `updated` to today.

## 4. Frontmatter Schema (per page)

```yaml
---
title: Payment Gateway
type: concept                     # concept | entity | module | process | synthesis
sources:
  - path: src/services/payment.py # source repo logical path, NO `raw/code/` prefix
goals:
  - "了解結帳流程"
created: '2026-05-04'              # UTC YYYY-MM-DD
updated: '2026-05-04'              # UTC YYYY-MM-DD
related:
  - "[[checkout-flow]]"
stale: false
---
```

> **Date convention:** `created` / `updated` are **UTC YYYY-MM-DD**.

## 5. Wikilinks Convention

- **Slug = file basename, NEVER the page title.** If the file is
  `concepts/project-purpose.md` the slug is `project-purpose`. Writing
  `[[專案目的]]` will NOT resolve — wikilinks match by filename, not by
  title. Title (frontmatter `title:`) is free-form and human-readable;
  slug is mechanical and ASCII.
- Link to other pages by slug: `[[payment-gateway]]` (NOT a path).
- Slug naming: lower-case kebab-case ASCII (`payment-gateway`,
  `checkout-flow`). Avoid CJK characters, spaces, and capitals.
- Slug uniqueness across folders matters: two pages with the same
  filename in different folders make `[[slug]]` ambiguous. Pick
  distinct slugs.
- In YAML lists you MUST quote each wikilink string:
  `related: ["[[a]]", "[[b]]"]` — do not write
  `related: [[a]], [[b]]` (that breaks YAML).
- In body text wikilinks need no quoting.
- Nav files at `wiki/` root are also valid wikilink targets:
  `[[index]]` and `[[log]]` resolve to the corresponding `wiki/<file>.md`.

## 6. Source Code References

- Frontmatter `sources` lists each raw file you read for this page.
- In body, cite source code with fenced code blocks and a path comment:

```python
# from src/services/payment.py
class PaymentGateway: ...
```

## 7. Stopping Criteria

- Step budget: aim for at most 30 reasoning-and-action steps per goal.
- Stay within scope of the goal text — do not explore tangential modules.
- When you have enough sources to write a coherent reading guide, stop
  exploring and start writing.

## 8. Out-of-Scope Detection

A goal is **out-of-scope** when the goal text references something
unrelated to any code in `raw/code/`:

- "查詢今天天氣" / "訂機票" / "查股價" (external real-time data)
- "今天午餐吃什麼" / "推薦一首歌" (irrelevant to any codebase)
- "翻譯這段文字" / "幫我寫履歷" (off-topic utility)

When out-of-scope, emit one short explanation (2-3 sentences) and
**create or modify zero files**. Out-of-scope goals leaving `wiki/`
unchanged is the correct outcome.

## 9. Failure Modes

- Read fails (file missing / encoding) → log it, skip, continue.
- Write fails (path outside `wiki/`) → log it, skip.
- Do not retry the same operation infinitely.
