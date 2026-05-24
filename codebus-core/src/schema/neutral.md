<!--
SPDX-License-Identifier: MIT
codebus built-in schema injected at vault init as the per-vault agent rules file.
-->

# codebus Wiki Schema

The vault under `.codebus/` is structured to help engineers ramp up on
an unfamiliar codebase. This file documents the structure rules; the
codebus-goal / codebus-query / codebus-fix skill bundles describe the
workflows that produce and consume content under these rules.

## 0. Language Policy

- The natural language of agent output (page bodies, stdout summary
  lines, answer text) SHALL follow the prompt context language — i.e.
  the language of the user's goal / query / chat text — not the
  language of any existing wiki page or raw source content read along
  the way.
- Structural tokens and YAML keys are ALWAYS literal English
  (`type:`, `sources:`, `created:`, `updated:`, marker lines like
  `[CODEBUS_*]`).
- Frontmatter free-text values (`title:`, `goals:`) follow the prompt
  context language; the field names themselves stay English.
- This schema document is an "internal surface" — the agent does NOT
  mirror this file's language into output. The agent reads this file
  to know the rules; the output language is decided by the prompt
  context, not by this file.

## 1. Workspace Layout

- READ: `raw/code/` (PII-redacted mirror of source), `wiki/` (existing pages for context).
- WRITE: `wiki/**/*.md` only — do NOT write to `raw/`, `log/` (codebus internal), or this schema file at the vault root (codebus owns it).
- NO ACCESS: any path outside `<vault>/`.

## 2. Wiki Structure

Two nav files at `wiki/` root:

- `wiki/index.md` — page catalog with summaries.
- `wiki/log.md` — chronological journal of goals; each entry covers
  goal text, covered pages by `[[wikilink]]`, suggested reading order,
  and key takeaways.

Knowledge pages live under five type buckets. Frontmatter `type` is
the **authoritative** metadata. The same-named folders under `wiki/`
are an organizational hint for sidebar grouping (codebus init
pre-creates them so the sidebar is structured even when empty), not a
strict filing contract. Folder names are lowercase plural
(`concepts/`, not `Concepts/` or `concept/`).

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

### 2a. Type tiebreakers

When the same content could plausibly fit two types, use these
tiebreakers:

- **concept vs process** — if you find yourself writing "Step 1,
  Step 2, ..." or "first ... then ... finally ...", it is a process.
  Concepts are statements of structure; processes are sequences of
  action.
- **concept vs entity** — a discrete data structure with named fields
  (schema, record, message shape) → entity. A category of ideas, a
  design pattern, or a principle → concept.
- **module vs synthesis** — one code unit (one library / one service /
  one runtime component) → module. An integration view that explains
  how multiple modules fit together → synthesis.

Wikilinks like `[[slug]]` resolve by filename regardless of folder, so
cross-folder linking just works.

## 3. Page Conflict Rules

- Page does not exist → create with frontmatter + body in the right type folder.
- Page exists (normal ingest mode) → add a new
  `## from goal: <X> (YYYY-MM-DD)` section at the end of body. Do not
  modify existing sections.
  - `<X>` is a 5-15 word abridged form of the goal text in the goal's
    own language (NOT the full goal text). Keep it short enough to fit
    on one heading line.
  - `YYYY-MM-DD` is the `updated` date from this page's frontmatter
    after this ingest run.
- Page exists (repair mode — goal verify/repair) → Edit existing
  sections per the `CONTENT DEFECTS:` list provided in the prompt.
  The "do not modify" rule above is for ingest only.
- Page exists (fix mode — `codebus fix`) → Edit per the lint rule
  semantics named in the prompt (frontmatter shape, wikilink format).
  Structure-only changes, not content rewrites.
- Frontmatter array fields (`sources`, `goals`, `related`) → union, no duplicates.
- Locked fields: `title`, `type`, `created` — never change. Type is
  locked because it determines the folder; if you think the type is
  wrong, surface that thought rather than silently moving the file.
- Update `updated` to today.

## 4. Frontmatter Schema (per page)

```yaml
---
title: Payment Gateway              # required, free-text human-readable
type: concept                       # required: concept | entity | module | process | synthesis
sources:                            # required (may be empty []), each item is { path: <source-repo-logical-path> }
  - path: src/services/payment.py   # source repo logical path, NO `raw/code/` prefix
goals:                              # required (may be empty [])
  - "了解結帳流程"
created: '2026-05-04'                # required, UTC YYYY-MM-DD
updated: '2026-05-04'                # required, UTC YYYY-MM-DD
related:                            # required (may be empty []), each item is a quoted "[[slug]]" string
  - "[[checkout-flow]]"
stale: false                        # required, boolean
---
```

**All fields above are required**; `codebus lint` enforces presence via
the `frontmatter-parse` rule. Array fields (`sources`, `goals`,
`related`) MAY be empty (`[]`) when the page legitimately has no
items.

> **Date convention:** `created` / `updated` are **UTC YYYY-MM-DD**.
> If the agent has no reliable signal for today's date (the prompt did
> not name a date, no `manifest.yaml` timestamp available, etc.), it
> SHALL leave `updated` unchanged rather than guess — a stale
> `updated` is recoverable; a hallucinated future date is not.

> **`stale` lifecycle:** `stale: false` at create time. Goal verify
> mode SHALL set `stale: true` when the page is flagged
> `unfaithful` / `off-goal` / `taxonomy-misplaced` and has not yet
> been repaired. Normal ingest mode SHALL NOT flip `stale`.

## 5. Wikilinks Convention

- **Slug = file basename, NEVER the page title.** If the file is
  `concepts/project-purpose.md` the slug is `project-purpose`. Writing
  `[[專案目的]]` will NOT resolve — wikilinks match by filename, not by
  title. Title (frontmatter `title:`) is free-form and human-readable;
  slug is mechanical and ASCII.
- Link to other pages by slug: `[[payment-gateway]]` (NOT a path).
- **Whole-page only** — `[[slug#heading]]` anchor syntax is NOT
  supported; wikilinks resolve at filename level only. The lint will
  not catch `[[slug#heading]]` (it is a syntactically valid wikilink
  shape), but anchor navigation will silently fail.
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
- `raw/code/` content has been PII-redacted upstream by codebus; you
  may quote raw/ snippets into wiki/ pages without further
  sanitization.
- In body, cite source code with a fenced code block and a path
  comment using the file's **native single-line comment syntax**:

```python
# from src/services/payment.py
class PaymentGateway: ...
```

```typescript
// from src/components/checkout-form.tsx
export function CheckoutForm() { ... }
```

```rust
// from src/agent/spawn_spec.rs
pub struct SpawnSpec { ... }
```

For languages with no single-line comment (e.g. JSON, plain XML), put
the path reference in a fenced quote block immediately above the code
block instead:

> from `data/schema.json`

```json
{ "type": "concept" }
```

## 7. Stopping Criteria

- Be deliberate about exploration depth — read entry points and
  high-level structure (folder layout, top-level module names, public
  exports) before drilling into specifics. Prefer breadth over depth
  in the first few steps so you can plan which sources to read in
  detail.
- Soft step budget: ~30 reasoning-and-action steps per goal as a
  self-check. This is a soft hint — the codebus binary does not
  enforce it. Complex goals legitimately exceed 30; an excessively
  large step count usually signals the goal needs to be split rather
  than the budget needs to be raised.
- Stay within scope of the goal text — do not explore tangential
  modules.
- When you have enough sources to write a coherent reading guide,
  stop exploring and start writing.

## 8. Out-of-Scope Detection

A goal is **out-of-scope** when the goal text references something
unrelated to any code in `raw/code/`:

- "查詢今天天氣" / "what is the weather today" / "book a flight" (external real-time data)
- "今天午餐吃什麼" / "recommend me a song" (irrelevant to any codebase)
- "翻譯這段文字" / "help me write a resume" (off-topic utility)

When out-of-scope, emit one short explanation (2-3 sentences) and
**create or modify zero files**. Out-of-scope goals leaving `wiki/`
unchanged is the correct outcome.

## 9. Failure Modes

- Read fails (file missing / encoding error) → mention the path
  briefly in the closing stdout summary, skip that source, and
  continue. Do NOT write the error message into any `wiki/` page; do
  NOT fail the entire run.
- Write fails (path outside `wiki/`, permission error) → mention the
  attempted path briefly in the closing stdout summary, skip that
  write, and continue. The unit being skipped is a single page write
  — other pages in the same run still attempt their writes
  independently.
- Do not retry the same operation infinitely; one retry is the
  practical cap for transient errors.

### Complete page example

Below is the end-to-end shape of one `wiki/` page produced by ingest,
for agents that have not yet seen the schema in concrete form:

````markdown
---
title: Payment Gateway
type: concept
sources:
  - path: src/services/payment.py
goals:
  - "了解結帳流程"
created: '2026-05-04'
updated: '2026-05-04'
related:
  - "[[checkout-flow]]"
stale: false
---

Payment Gateway 是結帳流程的金流抽象層；上游 [[checkout-flow]] 透過
它把訂單轉成第三方支付的請求。

## Mechanism

收到結帳請求後，PaymentGateway 依 `provider` 欄位 dispatch 到對應
的 adapter（Stripe / LinePay / ECPay），把訂單金額換成各 provider 的
charge payload，等待 webhook 回呼後更新訂單狀態。

```python
# from src/services/payment.py
class PaymentGateway:
    def charge(self, order, provider): ...
```
````
