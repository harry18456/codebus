<!--
codebus fixture vault for v3-app-quiz spike (2026-05-15).
Topic: simplified web auth. 5 wiki pages + 1 raw/code file.
This is a spike fixture; NOT a real vault. Used only by spike-quiz-* artifacts.
-->

# codebus Wiki Schema (fixture)

The vault under `.codebus/` is structured to document a simplified web auth
codebase for the purpose of testing the v3-app-quiz workflow.

## Workspace Layout

- READ-only: `raw/code/` (mirror), `wiki/` (existing pages)
- WRITE: `wiki/**/*.md` only
- DO NOT touch: `raw/`, `log/`, this schema file

## Wiki Structure

Type buckets: `concept` / `entity` / `module` / `process` / `synthesis`.

- `wiki/concepts/<slug>.md` — ideas, principles, mental models
- `wiki/entities/<slug>.md` — schemas, records
- `wiki/modules/<slug>.md` — code units, libraries, services
- `wiki/processes/<slug>.md` — workflows, lifecycles, algorithms
- `wiki/synthesis/<slug>.md` — cross-cutting summaries

Wikilinks `[[slug]]` resolve by filename across folders.

## Frontmatter Schema

```yaml
---
title: <Title>
type: concept | entity | module | process | synthesis
sources:
  - path: <repo-relative source path>
goals: []
related:
  - <slug of related page>
created: YYYY-MM-DD
updated: YYYY-MM-DD
stale: false
---
```
