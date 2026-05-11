# codebus-app v1 UX Flow Design

> Status: Brainstorming output, awaiting user review
> Date: 2026-05-11
> Scope: codebus-app (Tauri desktop GUI) v1 user experience
> Next step: After review, hand off to `writing-plans` skill to break into implementation tasks
>
> This document captures all UX decisions reached in the 2026-05-11 brainstorming session. It is the
> source of truth for v1 scope. Visual prototyping (Claude Design / claude.ai/design) is a parallel
> track that consumes this doc but does not replace it.

---

## 0. TL;DR

codebus-app v1 is a **learner-centric desktop tool** that lets a developer explore an unfamiliar
codebase, build an LLM-maintained wiki, and self-validate understanding via auto-generated quizzes.
It is the GUI counterpart to the existing `codebus` CLI — same core (`codebus-core`), different
surface. v1 ships in 5–6 weeks with seven user-facing features. Tutorial generation, knowledge
graph, quest system, and multi-provider support are deferred to v1.5 / v2.

---

## 1. Product positioning

### 1.1 Primary user

A developer who is **about to onboard onto an unfamiliar codebase** — could be a new hire, a team
member rotating onto a project, an OSS contributor approaching a new repo, or the user themselves
revisiting their own side project after months away.

### 1.2 Core loop

```
explore → build wiki → self-validate (quiz) → iterate
```

The unique value proposition is **self-validation**: the user is not just consuming docs (like
GitBook / Docusaurus) and not just searching code (like Sourcegraph) — they are running an
agentic loop where the LLM helps them turn confusion into structured understanding, then tests
that understanding back.

### 1.3 Why not a "wiki for the team"

A team-wiki framing (Author writes wiki → Reader consumes) was considered and rejected. Reasons:

- Conflicts with README's existing "上車舞 / 公車 / 旅遊書" metaphor, which is first-person
  exploratory.
- "Author writes for others" is a small market (only leads / repo owners do this).
- "Author self-validates via quiz" is the differentiator vs llm_wiki and similar tools.
- Tutorial-for-others output is still possible (via export in v1.5), but the *primary* use case
  is the user themselves.

### 1.4 Non-goals (explicit)

The following are **NOT** in v1, even though they have been discussed:

- Author-mode optimized for writing tutorials others will consume → v1.5
- Multi-user collaboration / sync of `.codebus/` across machines → v2+ (and possibly never)
- Knowledge graph visualization → v2
- Multi-AI-provider switcher → v2
- Multi-PII-provider switcher → v2
- First-run wizard → v2
- Vault-specific config override → v2
- Quest / milestone goal system → v2
- Stations abstraction (above raw "goals run") → v2 (if needed)
- Theme / language settings → v2 (hard-coded dark + zh-tw + en fallback in v1)

---

## 2. Scope

### 2.1 v1 (this doc, 5–6 weeks)

Seven user-facing features:

1. **Lobby** — vault list, new vault, settings entry
2. **Vault Workspace** — sidebar + main shell, navigation
3. **Goal flow** — `+ New Goal` → input → stream visualization → completion
4. **Wiki preview** — markdown rendering, wikilink navigation
5. **Quiz flow** — single-page + 1-hop scope, 5 multi-choice questions, md-file storage
6. **Cmd+K query drawer** — spotlight-style overlay, single-shot
7. **Global Settings** — minimal config modal, reads/writes `~/.codebus/config.yaml`

### 2.2 v1.5 (after v1 ships)

- Tutorial md generation (LLM produces a structured walkthrough md)
- Slideshow / tutorial-reader mode with embedded checkpoints & quizzes
- Tutorial bundle export → standalone web bundle for sharing
- Short-answer questions + LLM grading (currently choice-only)
- Multi-page mixed quiz; user-selectable quiz scope
- "Open in Claude Code" handoff from quiz/wiki

### 2.3 v2

- Quest system (B + C hybrid: milestone count + opt-in user-defined quest with LLM-suggested seeds)
- Graph view (sigma.js + graphology, leverage llm_wiki reference)
- Multi-AI provider (Codex / Gemini / other agentic CLIs)
- Multi-PII provider (Presidio HTTP, AWS Comprehend)
- First-run setup wizard
- Vault-specific settings override
- Stations abstraction (if quest system shows need)
- Drift fallback for query (use raw code when wiki is stale)
- Query escalation flow (wiki insufficient → suggest running goal)
- Query filing-back (good answer → wiki page)

---

## 3. App architecture (VS Code-style two-state)

```
┌─────────────────────────────────────────────────────────────┐
│  codebus-app                                                │
│                                                             │
│  STATE 1: Lobby (no vault open)                             │
│  ┌─────────────────────────────────────────────┐           │
│  │ [header: codebus] [+ New Vault]             │           │
│  │ ─────────────────────────────────           │           │
│  │ RECENT VAULTS                               │           │
│  │   • uv                  /work/uv     2h ago │           │
│  │   • my-saas-backend     /side/saas   3d ago │           │
│  │   • tauri               /open/tauri  1w ago │           │
│  │ ─────────────────────────────────           │           │
│  │ [⚙ settings]                       [v0.x.x] │           │
│  └─────────────────────────────────────────────┘           │
│                                                             │
│  STATE 2: Vault Workspace (one vault open)                  │
│  ┌──────────┬──────────────────────────────────┐           │
│  │ ← lobby  │ Main area                        │           │
│  │ uv       │   (default: Goals overview)      │           │
│  │ ──────── │                                  │           │
│  │ 🚏 Goals │   [+ New Goal]                   │           │
│  │ 📂 Wiki  │                                  │           │
│  │ 🎓 Quiz  │   Recent goals & quiz events     │           │
│  │          │                                  │           │
│  │          │                                  │           │
│  │ ⚙        │                                  │           │
│  └──────────┴──────────────────────────────────┘           │
│                                                             │
│  Cmd+K (any state) → spotlight overlay (see §4.6)           │
└─────────────────────────────────────────────────────────────┘
```

### 3.1 State transition rules

- **Lobby → Workspace**: click on a vault card OR finish a `+ New Vault` flow.
- **Workspace → Lobby**: click `← lobby` in sidebar top.
- **Vault switching**: must go via Lobby (no in-place vault switching in v1). v2 may revisit.
- **Settings gear** is visible in both states, always at bottom-left.
- **Cmd+K** is available only in Workspace state (Lobby has no wiki to query against).

---

## 4. Detailed feature specs

### 4.1 Lobby

#### 4.1.1 Layout

```
┌─────────────────────────────────────────────────┐
│ 🚌 codebus                       [+ New Vault]  │  ← header
├─────────────────────────────────────────────────┤
│ RECENT VAULTS                                   │
│                                                 │
│ ┌─────────────────────────────────────────────┐ │
│ │ uv                            /work/uv      │ │
│ │ last opened 2h ago                          │ │
│ └─────────────────────────────────────────────┘ │
│                                                 │
│ ┌─────────────────────────────────────────────┐ │
│ │ my-saas-backend               /side/saas    │ │
│ │ last opened 3d ago                          │ │
│ └─────────────────────────────────────────────┘ │
│                                                 │
│ (more vaults...)                                │
│                                                 │
├─────────────────────────────────────────────────┤
│ ⚙ settings                            v0.1.0    │  ← footer
└─────────────────────────────────────────────────┘
```

#### 4.1.2 Vault card content (v1)

Each card displays only:

- Vault display name (defaults to repo folder name)
- Repo path (truncated if long)
- Last opened timestamp (relative: "2h ago" / "3d ago" / absolute date if > 30d)

**Not in v1 cards**: progress bars, station counts, quest banner, quiz pass-rate. v2 may add these
once the quest system is built.

#### 4.1.3 Empty state

Triggered when zero vaults exist (first-ever launch or all removed).

```
┌─────────────────────────────────────────────────┐
│                                                 │
│              🚌                                 │
│                                                 │
│       來搭第一台公車吧                          │
│                                                 │
│   codebus 把 LLM 探索程式碼的中間態            │
│   持久化成你的旅遊書。                          │
│                                                 │
│       [+ Board a new bus]                       │
│                                                 │
│   ── Quick start ─────────────────────          │
│   1. Pick a repo folder                         │
│   2. Run a goal: "搞懂這 repo 的 X"             │
│   3. Quiz yourself to verify                    │
│                                                 │
└─────────────────────────────────────────────────┘
```

#### 4.1.4 Interactions

- **Click vault card** → open that vault → transition to Workspace state.
- **`+ New Vault` button** → see §4.8.
- **Right-click vault card** → context menu:
  - "Open in file manager" (reveal repo path in OS)
  - "Remove from list" (does NOT delete `.codebus/` data, only unbinds from Lobby)
- **`⚙` footer** → open Global Settings modal (§4.7).

---

### 4.2 Vault Workspace

#### 4.2.1 Layout

```
┌────────────┬─────────────────────────────────────────────┐
│ ← lobby    │  Main area                                  │
│ uv         │                                             │
│ /work/uv   │  (default content = Goals overview, §4.2.3) │
│ ────────── │                                             │
│ 🚏 Goals   │                                             │
│ 📂 Wiki    │                                             │
│ 🎓 Quiz    │                                             │
│            │                                             │
│            │                                             │
│            │                                             │
│ ⚙          │                                             │
└────────────┴─────────────────────────────────────────────┘
```

#### 4.2.2 Sidebar

Fixed sidebar (~180px wide, not resizable in v1).

Top section:
- **`← lobby`** link (returns to Lobby state)
- Vault display name (bold)
- Repo path (muted, truncated)

Middle section — three nav items:
- **🚏 Goals** (default selected on enter)
- **📂 Wiki**
- **🎓 Quiz**

Bottom section:
- **⚙** gear (opens Global Settings)

#### 4.2.3 Main area — default content (Goals overview)

When user enters Workspace, main area shows:

```
┌─────────────────────────────────────────────────┐
│ Goals                          [+ New Goal]     │
├─────────────────────────────────────────────────┤
│                                                 │
│ RECENT  6 of 12                                 │
│                                                 │
│ ● "Map the request lifecycle from HTTP entry"   │
│    reading src/server/router/index.ts …▍       │
│                          streaming · 4,218 tok  │  ← running, expanded
│                                                 │
│ ● "How does the auth middleware compose..."     │
│                                       14m ago   │
│                                                 │
│ ● "Identify the public plugin API surface"      │
│                                        1h ago   │
│                                                 │
│ ✕ "Map the renderer/worker IPC protocol"        │
│                                        3h ago   │
│                                                 │
│ ● "Catalog the build/release pipeline"          │
│                                     yesterday   │
│                                                 │
└─────────────────────────────────────────────────┘
```

Goals are listed in reverse-chronological order. Two row states:

- **Default (30px, single line)**: status dot + goal text + relative timestamp on right.
  - Dot color encodes outcome: amber/yellow = running, green = succeeded, red = failed.
- **Running goal (~64px, expanded inline)**: same row but expanded vertically to show one
  line of live activity (`reading <file> …▍` or `writing <file> …▍` with blinking cursor)
  plus right-aligned `streaming · <tokens> tok` counter. Updates in place as codebus-core
  streams events.

Click any row → opens **Goal detail view** (§4.3.4) in main area for full timeline,
raw log, page list, and Cancel/Retry actions.

Design rationale: inline mini-stream keeps surrounding goal context visible while one runs
— mirrors GitHub Actions / Vercel deployments / Linear issue list patterns. The original
spec was "main area takeover on goal run"; revised based on Claude Design feedback (see
§9 decisions log).

#### 4.2.4 Sidebar nav switching

- **Click 🚏 Goals** → main area = Goals overview (default)
- **Click 📂 Wiki** → main area = Wiki tree + preview pane (§4.4)
- **Click 🎓 Quiz** → main area = Quiz history list (§4.5.6)

Active nav item visually highlighted (background fill).

---

### 4.3 Goal flow

#### 4.3.1 Trigger

`+ New Goal` button on Goals overview → opens modal.

#### 4.3.2 Input modal

```
┌─────────────────────────────────────────────────┐
│  + New Goal                              [X]    │
├─────────────────────────────────────────────────┤
│                                                 │
│  What do you want to understand?                │
│                                                 │
│  ┌───────────────────────────────────────────┐  │
│  │                                           │  │
│  │  e.g. "搞懂 auth 模組怎麼運作"            │  │
│  │                                           │  │
│  └───────────────────────────────────────────┘  │
│                                                 │
│                            [Cancel]  [Run goal] │
└─────────────────────────────────────────────────┘
```

- Textarea (multi-line allowed, ~3 lines visible)
- Placeholder text in user's language (zh-tw / en)
- `Run goal` button — enabled when input non-empty
- `ESC` or `Cancel` closes modal without action

#### 4.3.3 Submission → inline mini-stream

After `Run goal` click in the modal:

1. **Modal closes.**
2. **A new row appears at the top of `RECENT` in Goals overview**, expanded inline to
   show a one-line live mini-stream (see §4.2.3 running-state row visual).
3. User stays in Goals overview — they can watch the inline mini-stream tick, click into
   the row for full detail (§4.3.4), or navigate elsewhere (Wiki / Quiz / Cmd+K) and
   come back later.

This pattern (list-with-live-row) mirrors GitHub Actions, Vercel deployments, and Linear
issue lists. Rationale: keeps surrounding goal context visible during a run, avoids the
heavy mode-switch of taking over the main area.

**No main-area takeover happens automatically.** Full timeline / raw log / Cancel button
live in the detail view, surfaced by explicit click.

#### 4.3.4 Goal detail view (click into a row)

Clicking any goal row in `RECENT` opens the detail view in the main area. The view has
two sub-states depending on goal status:

**Sub-state A — goal currently running:**

```
┌─────────────────────────────────────────────────┐
│  ← back     "搞懂 auth 模組怎麼運作"     [Cancel]│
│             Running · 23s · 8.2k tokens         │
├─────────────────────────────────────────────────┤
│                                                 │
│  ▶ Reading codebase                             │
│    📄 src/auth/middleware.ts                    │
│    📄 src/auth/jwt.ts                           │
│    📄 src/auth/session.ts                       │
│                                                 │
│  ▶ Writing wiki                                 │
│    ✏ modules/auth-middleware.md (new)           │
│    ✏ concepts/jwt-token-lifecycle.md (new)      │
│    ✏ index.md (updated)                         │
│                                                 │
│  ⠋ analyzing token validation flow...           │
│                                                 │
│  ─── stream log (collapse ▼) ────────           │
│  (raw agent thought / tool calls, scrollable)   │
│                                                 │
└─────────────────────────────────────────────────┘
```

- Header: `← back` returns to Goals overview · goal text · live counter `Running · Ns · NK tokens` · `[Cancel]` (right).
- Body: structured event timeline parsed from codebus-core stream:
  - File reads (📄)
  - File writes (✏ with `new` / `updated` badge)
  - Current activity (⠋ spinner + status line)
- **Collapsed raw stream log** below — scrollable, holds full agent thought/tool stream
  for power users. Default collapsed.
- **Cancel** sends `SIGINT` to spawned agent → halts gracefully, partial wiki changes
  preserved (codebus-core auto-commits per goal).

**Sub-state B — goal completed (success or failure):**

```
┌─────────────────────────────────────────────────┐
│  ← back     "搞懂 auth 模組怎麼運作"   ✅ Done   │
│             Completed in 47s · 14.3k tokens     │
├─────────────────────────────────────────────────┤
│                                                 │
│  Wiki pages changed (3):                        │
│                                                 │
│  ✏ modules/auth-middleware.md (new)             │
│    [Open] [Quiz me]                             │
│                                                 │
│  ✏ concepts/jwt-token-lifecycle.md (new)        │
│    [Open] [Quiz me]                             │
│                                                 │
│  ✏ index.md (updated)                           │
│    [Open]                                       │
│                                                 │
│  ─── stream history (collapse ▼) ────           │
│  (timeline + raw log, collapsed by default)     │
│                                                 │
└─────────────────────────────────────────────────┘
```

- Header status pill: `✅ Done` (success) or `✕ Failed` (failure).
- For success: list of changed wiki pages with `[Open]` (jumps to Wiki preview) and
  `[Quiz me]` (jumps to Quiz flow §4.5 with that page as target) per page.
  - **`[Quiz me]` visibility rule**: only shown for content pages under `concepts/` /
    `entities/` / `modules/` / `processes/` / `synthesis/`. Catalog pages
    (`index.md`, `log.md`) get `[Open]` only — they are metadata, not content to quiz on.
- For failure: shows error reason and `[Retry with same goal]` button. Pages written
  before the failure are still listed (codebus-core auto-commit preserves them).
- **Stream history** at bottom — collapsed by default. Expand to review timeline + raw
  log of the completed run.

`← back` always returns to Goals overview (sub-state independent).

#### 4.3.5 Goal lifecycle summary

| Stage | Where it lives | User action |
|---|---|---|
| Input | Modal (§4.3.2) | Types goal, clicks Run goal |
| Submitted, running | Inline row in Goals overview (§4.2.3) with live mini-stream | Stays in list, or clicks into detail |
| Running detail | Detail view sub-state A (§4.3.4) | Watch timeline; Cancel if needed |
| Completed (success) | Row collapses to single line with green dot in Goals overview | Click into detail for page list (§4.3.4 sub-state B) |
| Completed (failure) | Row collapses to single line with red dot in Goals overview | Click into detail for error + Retry (§4.3.4 sub-state B) |

No "completion-takeover" auto-redirect. The user is never moved out of their current
context by a goal finishing — they see the row state change (or get a subtle inline
notification at the top of the list, deferred to v1.5).

---

### 4.4 Wiki preview

#### 4.4.1 Layout

When user clicks 📂 Wiki in sidebar, main area becomes a two-column wiki view:

```
┌───────────┬─────────────────────────────────────┐
│ Tree      │ Preview                             │
│ ┌───────┐ │ ┌─────────────────────────────────┐ │
│ │ 📂    │ │ │ modules/auth-middleware.md      │ │
│ │ concepts │ │ ▶ frontmatter (collapsed)      │ │
│ │  jwt..│ │ │                                 │ │
│ │ entities│ │ │ # Auth Middleware              │ │
│ │  user.│ │ │                                 │ │
│ │ modules │ │ │ The auth middleware runs...    │ │
│ │  auth.│ │ │ It uses [[jwt-token-lifecycle]] │ │
│ │  ...  │ │ │ to verify tokens.               │ │
│ │ processes│ │                                 │ │
│ │ synthesis│ │ ...                             │ │
│ │ index.md│ │                                 │ │
│ │ log.md │ │                                 │ │
│ └───────┘ │ │             [Quiz me on this]   │ │
│           │ └─────────────────────────────────┘ │
└───────────┴─────────────────────────────────────┘
```

#### 4.4.2 Wiki tree

Left sub-column (~200px), expandable folder tree.

Top-level entries (fixed order, mirrors `.codebus/wiki/` structure):
- `concepts/`
- `entities/`
- `modules/`
- `processes/`
- `synthesis/`
- `index.md` (top-level file)
- `log.md` (top-level file)

Folders expand to show their `.md` files. Click a file → preview loads in right column.

**Not in v1**: file search, pinned files, custom ordering.

#### 4.4.3 Preview pane

Right sub-column, renders the selected markdown file.

- **Frontmatter** rendered as a collapsible section at top. Default collapsed; small `▶` to expand.
- **Body** rendered with Milkdown (or chosen markdown renderer):
  - GFM tables
  - Code blocks with syntax highlighting (use language hints from fenced blocks)
  - Wikilinks `[[target]]` rendered as clickable links
- **Click wikilink** → navigate to target page in the same preview pane (NO back button — user
  re-selects via sidebar tree if they want to return).
- **Footer**: `[Quiz me on this]` button → triggers Quiz flow (§4.5) with this page as target.

#### 4.4.4 Wikilink resolution

- Match by slug (file name without `.md`).
- If `[[target]]` resolves to multiple files → pick the first in path order, show a small
  "ambiguous link" warning under the preview header.
- If `[[target]]` does not resolve → render as red/struck-through text, do nothing on click.
- **Anchor part is ignored in v1.** `[[page#heading]]` clicks navigate to the page (whole
  view, top of page). Scroll-to-heading is deferred to v1.5. This applies both in wiki
  preview body and inside quiz explanation blockquotes.

#### 4.4.5 Index.md / log.md special handling

- `index.md` and `log.md` are regular markdown pages — render the same as any other.
- Wikilinks in them work the same.
- No special UI affordance in v1.

---

### 4.5 Quiz flow

#### 4.5.1 Trigger points

Two entry points (same flow downstream):

- **From wiki preview**: `[Quiz me on this]` button at bottom of preview pane.
- **From sidebar Quiz tab**: `+ New quiz` button → user picks a wiki page from a list → continues.

#### 4.5.2 Scope (v1 locked decision)

Quiz scope is **target page + 1-hop wikilinked pages**.

- "Target page" = the page user explicitly picked.
- "1-hop wikilinks" = all pages this target page references via `[[...]]`.
- LLM context includes all of them; the prompt instructs LLM that **questions are about the target
  page, but may reference related pages for context**.

This solves the "single-page produces shallow questions" concern without exposing scope-picking UI
to the user.

**Not in v1**: cross-multi-page mix quiz, user-selectable scope (these are v1.5).

#### 4.5.3 Prep screen

```
┌─────────────────────────────────────────────────┐
│  Prep Quiz                                      │
├─────────────────────────────────────────────────┤
│                                                 │
│  Quizzing on:                                   │
│    📄 modules/auth-middleware.md                │
│                                                 │
│  Context will include 3 related pages:          │
│    📄 concepts/jwt-token-lifecycle.md           │
│    📄 entities/user-model.md                    │
│    📄 processes/login-flow.md                   │
│                                                 │
│  Question count: 5  (configured in settings)    │
│                                                 │
│                          [Cancel] [Generate]    │
└─────────────────────────────────────────────────┘
```

- Shows transparency on scope (target + 1-hop list).
- `[Generate]` → spawn LLM with quiz-generation prompt + context → produce quiz md file.
- During generation: show inline spinner "Generating questions…"

#### 4.5.4 Question UI (one question per screen)

```
┌─────────────────────────────────────────────────┐
│  Quiz: auth-middleware            Q3 of 5       │
├─────────────────────────────────────────────────┤
│                                                 │
│  Q3. Where does authentication start?           │
│                                                 │
│  ○ In the controller                            │
│  ● In the middleware                            │  ← selected
│  ○ In the database layer                        │
│  ○ In the frontend                              │
│                                                 │
│                                       [Submit]  │
└─────────────────────────────────────────────────┘
```

After clicking `[Submit]`:

```
┌─────────────────────────────────────────────────┐
│  Quiz: auth-middleware            Q3 of 5       │
├─────────────────────────────────────────────────┤
│                                                 │
│  Q3. Where does authentication start?           │
│                                                 │
│  ○ In the controller                            │
│  ✅ In the middleware ← your answer · correct   │
│  ○ In the database layer                        │
│  ○ In the frontend                              │
│                                                 │
│  Auth middleware runs before route handlers     │
│  per [[auth-flow#middleware]].                  │
│                                                 │
│                                  [Next: Q4 →]   │
└─────────────────────────────────────────────────┘
```

On wrong answer:

```
│  Q3. Where does authentication start?           │
│                                                 │
│  ❌ In the controller ← your answer             │
│  ✅ In the middleware (correct)                 │
│  ○ In the database layer                        │
│  ○ In the frontend                              │
│                                                 │
│  Auth middleware runs before route handlers...  │
│                                                 │
│  [← Back to wiki page]            [Next: Q4 →]  │
```

- Wrong answer surfaces `[← Back to wiki page]` link → opens target page in main area (closes
  quiz UI). Coarse-grained: jumps to whole page, NOT to specific paragraph.

Each choice row has a **letter badge (A / B / C / D)** on the left for visual anchoring and
keyboard binding. Selected state shows letter badge in accent (amber) + filled radio.

**Keyboard shortcuts (active during the quiz):**

| Key | Action |
|---|---|
| `A` / `B` / `C` / `D` | Select the matching choice |
| `Enter` (or `↵`) | Submit (when a choice is selected) |
| `→` (right arrow) | Advance to next question (after submit, in reveal state) |
| `Esc` | Exit quiz (with confirm if mid-quiz) — same as clicking out |

The submit hint "↵ to submit" is shown subtly near the Submit button. Letter-key affordances
are not explicitly labeled on screen (each row's letter badge IS the affordance).

#### 4.5.5 Summary screen (after Q5)

```
┌─────────────────────────────────────────────────┐
│  Quiz Complete                                  │
├─────────────────────────────────────────────────┤
│                                                 │
│           4 / 5 correct = 80%                   │
│           ✅ Passed                             │
│                                                 │
│  Question review:                               │
│   Q1 ✅  Q2 ✅  Q3 ❌  Q4 ✅  Q5 ✅              │
│                                                 │
│  Wrong: Q3 — review [[auth-flow]]               │
│                                                 │
│  [Retry with new questions] [Back to wiki page] │
└─────────────────────────────────────────────────┘
```

- Pass threshold = 80% (configurable in Global Settings).
- `[Retry with new questions]` → re-runs prep screen, generates fresh questions, new file.

#### 4.5.6 Quiz history (sidebar 🎓 Quiz tab)

```
┌─────────────────────────────────────────────────┐
│  Quiz History               [+ New quiz]        │
├─────────────────────────────────────────────────┤
│                                                 │
│  📄 auth-middleware                             │
│    ✅ 80% · 2h ago        ❌ 60% · yesterday    │
│                                                 │
│  📄 jwt-token-lifecycle                         │
│    ✅ 100% · 3h ago                             │
│                                                 │
│  📄 checkout-flow                               │
│    ❌ 40% · 2d ago                              │
│                                                 │
└─────────────────────────────────────────────────┘
```

Click any history row → open the result md file in preview pane (so user can see exact questions
asked and what they answered).

#### 4.5.7 Storage format

```
.codebus/quiz/
├─ auth-middleware/
│   ├─ 2026-05-11T14-30-00.md      ← quiz file (questions + result combined)
│   └─ 2026-05-11T16-45-22.md      ← retry, separate file
└─ jwt-token-lifecycle/
    └─ 2026-05-11T15-00-00.md
```

Single md file per quiz attempt. Format:

```markdown
---
quiz_id: 2026-05-11T14-30-00
target_page: modules/auth-middleware.md
context_pages:
  - concepts/jwt-token-lifecycle.md
  - entities/user-model.md
  - processes/login-flow.md
generated_at: 2026-05-11T14:30:00Z
question_count: 5
result:
  user_answers: [1, 0, 1, 2, 0]
  correct_answers: [1, 0, 1, 2, 0]
  score: 100
  passed: true
  finished_at: 2026-05-11T14:34:12Z
---

# Quiz: auth-middleware

## Q1: Where does authentication start?
- [ ] In the controller
- [x] In the middleware
- [ ] In the database layer
- [ ] In the frontend

> Auth middleware runs before route handlers per [[auth-flow#middleware]].

## Q2: What does the JWT lifecycle look like?
- [ ] ...
...
```

The `[x]` marker indicates the **correct** answer. User's actual answer lives in
`result.user_answers` (indexed). Explanation is the blockquote after the choices.

#### 4.5.8 Question generation prompt (sketch — final lives in codebus-core)

```
Given the target wiki page [content of target page]
and these related pages it links to [content of 1-hop pages],
generate exactly 5 multiple-choice questions (4 choices each)
that test the user's understanding of the target page.

Each question may reference relationships to the related pages
when relevant, but the primary subject is the target page.

Format: markdown sections (## Q1:, ## Q2:, ...) with:
- The question text
- Four bullet choices with [ ] / [x] checkbox syntax
- A blockquote explanation after the choices

Difficulty: factual + comprehension level. Not trick questions.
Language: same as the wiki content language.
```

---

### 4.6 Cmd+K Query drawer

#### 4.6.1 Trigger

- **Mac**: Cmd+K
- **Windows / Linux**: Ctrl+K
- Available only in Workspace state (Lobby has no wiki to query).

#### 4.6.2 Visual (spotlight-style overlay)

```
┌────────────────────────────────────────────────────────────┐
│ ░░░░░░░░ workspace background blurred ░░░░░░░░░░░░░░░░░ │
│ ░░░░                                                 ░░░░ │
│ ░░    ┌──────────────────────────────────────────┐   ░░  │
│ ░░    │  Response (clean dark card)              │   ░░  │
│ ░░    │                                          │   ░░  │
│ ░░    │  Auth middleware runs before route...    │   ░░  │
│ ░░    │  (scroll if long)                        │   ░░  │
│ ░░    │                                          │   ░░  │
│ ░░    │  ▶ Cited:                                │   ░░  │
│ ░░    │    📄 modules/auth-middleware.md         │   ░░  │
│ ░░    │    📄 concepts/jwt-token-lifecycle.md    │   ░░  │
│ ░░    ├──────────────────────────────────────────┤   ░░  │
│ ░░    │  > 你想知道什麼？               ⏎       │   ░░  │
│ ░░    └──────────────────────────────────────────┘   ░░  │
│ ░░░░                                                 ░░░░ │
│ ░░░░░░░░░░ ESC to close ░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░ │
└────────────────────────────────────────────────────────────┘
```

- **Card center-screen**, ~720px wide, ~70vh tall max.
- **Background**: blurred workspace behind (CSS backdrop-filter).
- **Card itself**: solid dark background, sharp text — readable.
- **Top of card**: response area, scrollable.
- **Bottom of card**: input bar with placeholder and submit-on-Enter.
- **Cited section**: appears below response when LLM returns citations. Each cited page is a clickable chip.

#### 4.6.3 Behavior

- **Open**: Cmd+K → overlay slides/fades in. Input auto-focused.
- **Type query + Enter** → spawn read-only agent via codebus-core (same `query` flow as CLI), stream response into card.
- **`YOU ASKED` header label** above response shows the current question, so it stays visible while reading a long answer.
- **Cited section** appears after streaming completes — `CITED N` count + chip-style links to wiki pages.
- **Click cited page chip** → **close overlay** + main area navigates to that wiki page in Wiki tab.
- **ESC** → close overlay, discard session.
- **Soft single-shot mode**: after an answer completes, the input bar accepts a new
  question. Submitting it discards the current answer and starts a fresh agent run
  (no conversation memory — each submit is independent). The just-asked question is
  echoed in the input as muted/grey text for context; typing replaces it.
- **Cmd+K while overlay open** → no-op (or focus input).

**Keyboard bindings inside the overlay:**

| Key | Action |
|---|---|
| `Enter` (`↵`) | Submit query / start new fresh query |
| `↑` / `↓` | Move focus between cited page chips (when cited section present) |
| `⌘↵` / `Ctrl+↵` | Open the focused cited page (closes overlay + navigates) |
| `Esc` | Close overlay, discard session |

Bottom-of-overlay shows three subtle keyboard hints outside the card:
`↕ nav cited` · `⌘↵ open citation` · `ESC to close`.

**Inline code styling**: technical tokens inside the response (function names, header
names, status codes, file paths, etc.) are rendered with monospace + subtle pill
background — distinguishes code references from prose.

#### 4.6.4 Cancellation / error

- During streaming, an `[X]` button replaces submit; click cancels.
- On error, response area shows the error inline; user can re-type and retry.

---

### 4.7 Global Settings

#### 4.7.1 Trigger

`⚙` gear at bottom-left of either Lobby or Workspace → opens **modal**.

#### 4.7.2 Modal layout

```
┌─────────────────────────────────────────────────┐
│  Global Settings                          [X]   │
├─────────────────────────────────────────────────┤
│                                                 │
│  AI Provider          Claude CLI (only option)  │
│                                                 │
│  Authentication       Connected ✅              │
│                       [Re-authenticate...]      │
│                                                 │
│  Default model                                  │
│    goal     [sonnet ▾]                          │
│    query    [haiku  ▾]                          │
│    fix      [sonnet ▾]                          │
│                                                 │
│  PII scanner          [regex_basic ▾]           │
│                                                 │
│  Log sink             ~/.codebus/logs/          │
│                       [Change folder...]        │
│                                                 │
│  Quiz pass threshold  80%  [slider]             │
│                                                 │
│  Default quiz length  5    [slider]             │
│                                                 │
├─────────────────────────────────────────────────┤
│  Reads/writes ~/.codebus/config.yaml            │
│                          [Cancel]    [Save]     │
└─────────────────────────────────────────────────┘
```

#### 4.7.3 Field details

| Field | Type | Default | Persisted to |
|---|---|---|---|
| AI Provider | read-only label | `Claude CLI` | n/a |
| Authentication | OAuth status + button | n/a | (managed by Claude CLI itself) |
| Default model — goal | dropdown | `sonnet` | `claude_code.goal.model` |
| Default model — query | dropdown | `haiku` | `claude_code.query.model` |
| Default model — fix | dropdown | `sonnet` | `claude_code.fix.model` |
| PII scanner | dropdown | `regex_basic` | `pii.scanner` |
| Log sink path | folder picker | `~/.codebus/logs/` | `log.sink.dir` |
| Quiz pass threshold | slider 50–100% | `80` | `app.quiz.pass_threshold` (new key) |
| Default quiz length | slider 3–10 | `5` | `app.quiz.default_length` (new key) |

The last two introduce a new `app.*` config namespace in `~/.codebus/config.yaml` for app-specific
settings the CLI doesn't use.

**Field labels and sub-labels (v1 wording lock):**

- `Default model` — no sub-label that promises a per-goal or per-vault override (those are
  v2 features; sub-label would be misleading). If a sub-label is needed for clarity,
  use neutral wording like "applies to all runs".
- `PII scanner` — show pattern count next to the scanner name (e.g. `regex_basic · 14 patterns`).
  Implementation reads pattern count **dynamically** from the scanner registry —
  do not hard-code the number.
- `Quiz pass threshold` — sub-label: `% correct to pass a quiz attempt`. Pass is
  **attempt-level boolean** in v1 (matches the spec's "passed" terminology). Do NOT use
  page-level vocabulary like "learned" / "mastered" / "graduated" — those imply per-page
  state we cut to v2.
- Slider value labels include the unit ("5 questions", "80%"), not just the bare number.

#### 4.7.4 Persistence

- All fields write back to `~/.codebus/config.yaml` on `[Save]`.
- App uses same loader as CLI (codebus-core) → CLI sees the same values immediately.
- `[Cancel]` discards changes.

#### 4.7.5 Not in v1

- Theme (hard-coded dark)
- Language (hard-coded with light auto-detection: zh-tw if system locale starts with `zh`, otherwise en)
- Vault-specific override (entire concept deferred to v2)
- Direct YAML editing pane (power users edit `~/.codebus/config.yaml` directly)

---

### 4.8 New Vault flow

#### 4.8.1 Trigger

Three equivalent ways to start the New Vault flow:

- **`+ New Vault` button** at Lobby top-right (populated state) or **`+ Board a new bus`**
  centered button (empty state).
- **Keyboard shortcut `Cmd+N` / `Ctrl+N`** — works in Lobby state.
- **Drag-and-drop a folder** anywhere into the Lobby window — skips the picker step and
  goes directly to detection (§4.8.2 step 2 onward). The drag-drop affordance is hinted
  with a subtle line at the bottom of the vault list: "tip · Drag a repo folder anywhere
  into this window to open it as a vault." The hint is only shown in Lobby (Workspace
  doesn't accept folder drops).

#### 4.8.2 Steps

1. **Folder picker** opens (Tauri native dialog) → user selects a repo folder.
2. **Detection step**: app checks for existing `<picked-folder>/.codebus/`.
3. **Branch**:

   **Branch A: no `.codebus/`** → silently run `codebus init` equivalent on that folder. On
   success, add to Lobby list and transition to Workspace state.

   **Branch B: existing `.codebus/`** → show choice dialog:

   ```
   ┌────────────────────────────────────────────┐
   │  Folder already initialized                │
   ├────────────────────────────────────────────┤
   │                                            │
   │  This folder contains a .codebus/ vault.   │
   │                                            │
   │  ○ Just bind it to Lobby (recommended)     │
   │  ○ Re-initialize (destructive — deletes    │
   │     existing wiki and starts fresh)        │
   │                                            │
   │                  [Cancel]   [Continue]     │
   └────────────────────────────────────────────┘
   ```

   - "Just bind" → add to Lobby, transition to Workspace. No vault data touched.
   - "Re-initialize" → require explicit confirmation (typed phrase like "delete" or second
     dialog), then wipe + re-init. Heavy destructive action — fully disclosed.

#### 4.8.3 Error handling

- Folder not readable → error toast, no Lobby change.
- `codebus init` fails (e.g., git not installed) → error toast with message, no Lobby change.
- Picker cancelled → no-op.

---

## 5. Data model and storage

### 5.1 Per-vault data (`<repo>/.codebus/`)

```
.codebus/
├─ CLAUDE.md                     ← agent system prompt schema (existing)
├─ manifest.yaml                 ← source file manifest (existing)
├─ raw/                          ← source mirror (existing)
├─ wiki/                         ← wiki content (existing)
│   ├─ concepts/
│   ├─ entities/
│   ├─ modules/
│   ├─ processes/
│   ├─ synthesis/
│   ├─ index.md
│   └─ log.md
├─ logs/                         ← RunLog (existing)
├─ quiz/                         ← NEW in v1
│   └─ <page-slug>/
│       └─ <ISO-timestamp>.md
└─ .git/                         ← nested git (existing)
```

### 5.2 Global app state (`~/.codebus/`)

```
~/.codebus/
├─ config.yaml                   ← existing global config (CLI + app share)
└─ app-state.json                ← NEW in v1, app-only
    {
      "vault_list": [
        { "path": "/work/uv",        "display_name": "uv",          "last_opened": "2026-05-11T14:30:00Z" },
        { "path": "/side/saas",      "display_name": "my-saas",     "last_opened": "2026-05-09T10:00:00Z" }
      ],
      "schema_version": 1
    }
```

Vault list is **stored in app-state.json**, not derived from filesystem scan. This means:

- "Remove from list" simply removes the entry — `.codebus/` data untouched.
- If user moves a vault folder, the path in app-state becomes stale → app shows "missing" badge
  on next launch, offers to remove from list.
- App-state.json is **app-private** — CLI doesn't read it.

### 5.3 What goes into `~/.codebus/config.yaml`

Existing keys (CLI + app share):
- `lint.fix.*`
- `pii.*`
- `claude_code.*` (per-verb model / effort)
- `log.*`

New `app.*` namespace (app reads, CLI ignores):
- `app.quiz.pass_threshold` — int 50-100 (default 80)
- `app.quiz.default_length` — int 3-10 (default 5)

---

## 6. Technical decisions

### 6.1 Stack

| Layer | Choice | Reason |
|---|---|---|
| Desktop shell | Tauri v2 (Rust backend = codebus-core) | Reuse existing core; native binary; small footprint |
| Frontend framework | React 19 + TypeScript + Vite | Industry standard; matches llm_wiki reference |
| UI library | shadcn/ui + Tailwind CSS v4 | Composable; quick polish; consistent with llm_wiki |
| Markdown rendering | Milkdown | ProseMirror-based; wikilink plugin available |
| Frontend state | Zustand | Light; sufficient for this scope |
| Streaming | codebus-core stream parser → Tauri IPC → frontend | Reuse existing parser; no new abstraction |
| LLM | All agent calls go through codebus-core → spawns Claude CLI | No direct LLM calls from frontend |

### 6.2 Explicitly NOT chosen (and why)

- **sigma.js / graphology** — graph view is v2, no need in v1.
- **LanceDB / vector search** — semantic search is v2+.
- **Direct API call to Anthropic from frontend** — would bypass codebus-core sandbox & PII guarantees. Always go through core.
- **Electron** — Tauri already chosen, smaller and Rust-native.

### 6.3 Tauri ↔ codebus-core interface

(Detailed Rust commands belong in the implementation plan, not this design doc. Open question
listed in §8.)

High-level expected commands:
- `list_vaults() -> Vec<VaultEntry>`
- `add_vault(path) -> Result<VaultEntry>`
- `remove_vault(path) -> Result<()>`
- `read_wiki_tree(vault_path) -> WikiTree`
- `read_wiki_page(vault_path, page_path) -> WikiPage`
- `run_goal(vault_path, goal_text, on_event)` — streaming
- `run_query(vault_path, query_text, on_event)` — streaming
- `generate_quiz(vault_path, target_page) -> QuizFile`
- `submit_quiz_result(quiz_file, user_answers) -> QuizResult`
- `load_global_config() -> GlobalConfig`
- `save_global_config(config) -> Result<()>`

---

## 7. Timeline estimate

Part-time / solo developer working incrementally. Order is suggested by dependency:

| Stage | Duration | Deliverable |
|---|---|---|
| Tauri shell scaffold + codebus-core IPC bridge | 1 week | Empty app runs, can call codebus-core |
| Lobby + New Vault flow + Global Settings modal | 1 week | Can open vaults, see list, change settings |
| Vault Workspace shell + Wiki preview | 1 week | Can browse wiki content |
| Goal flow (input → stream → completion) | 1 week | Can run goals via GUI |
| Quiz flow (prep → questions → result → history) | 1.5 weeks | Can self-validate via quiz |
| Cmd+K query drawer | 1 week | Spotlight overlay works |
| Polish + cross-platform testing (mac/win/linux) | 0.5 week | Ship-ready |
| **Total v1** | **~7 weeks** | (5–6 weeks if no surprises) |

---

## 8. Open questions / deferred decisions

These are intentionally NOT pinned in v1 — will be decided during implementation or in a later
spec.

1. **codebus-core API surface for app** — exact Rust function signatures, error types, streaming
   event schema. Belongs in implementation plan, not UX doc.
2. **`agent-state.json` migration strategy** — first-launch creates v1 schema; future versions
   will need migration.
3. **Tauri auto-update channel** — out of scope for v1 (manual install).
4. **Telemetry / analytics** — none in v1. May be added later, but only opt-in.
5. **Crash reporting** — none in v1. Standard `RUST_BACKTRACE=1` logs to stderr.
6. **Conflict resolution if user has open the same vault in CLI and app simultaneously** — codebus
   already uses `manifest.yaml` + nested git for state. Most operations should compose, but
   simultaneous writes to wiki/ from both surfaces could race. v1 assumes single active surface.

---

## 9. Decisions log (lock-in)

Every decision in this doc was made in the 2026-05-11 brainstorming session. Key reversals from
earlier framings:

| Decision | Earlier framing | Final |
|---|---|---|
| Primary user | Author writing tutorials for others | **Learner doing self-study** |
| Progress model | Quest with milestones (B+C hybrid) | **No quest in v1; v2 may add** |
| Stations abstraction | Stations object with graduation state | **Just "goals" — no separate Station abstraction in v1** |
| Quiz scope | Single page only | **Single page + 1-hop wikilinks** |
| Quiz item type | Multi-choice + short-answer | **Multi-choice only; short-answer is v1.5** |
| Quiz storage | JSON / DB | **Markdown md file per attempt** |
| Cmd+K visual | Right-side drawer | **Spotlight overlay, center card** |
| Cmd+K mode | Chat with memory | **Single-shot, ESC discards** |
| Wiki nav back button | Yes | **No (use sidebar tree)** |
| Settings layers | Global + Vault override | **Global only in v1; Vault override is v2** |
| Vault list display | Cards with quest banner | **Plain list, name + path + last-opened only** |
| Multi-AI provider | Possibly in v1 | **Deferred to v2 (no real user demand yet)** |
| Goal stream UI | Main-area takeover after submit | **Inline mini-stream in goal list row; click into row for detail (revised 2026-05-11 after Claude Design Screen 1 review)** |

### 9.1 Amendment — 2026-05-11 (post-Screen-1 review)

After reviewing Claude Design's first iteration of Screen 1 (Vault Workspace), the
"main-area takeover when goal runs" pattern in original §4.3.3 was revised to an inline
mini-stream pattern. The new model:

- Goal submit → modal closes → row appears at top of `RECENT` in Goals overview, expanded
  inline to ~64px with one-line live activity + token counter.
- Other goal rows remain visible — surrounding context preserved during a run.
- Click into running row → detail view (§4.3.4 sub-state A) shows full timeline + raw
  log + Cancel.
- Completed rows collapse back to 30px single-line with green/red status dot.

Rationale: matches GitHub Actions / Vercel / Linear list-with-live-row pattern; lower
mode-switch cost than full takeover; preserves goal-list context during a run. The
revised flow is what §4.2.3, §4.3.3, §4.3.4, §4.3.5 now describe — original takeover
framing has been removed.

### 9.2 Amendment — 2026-05-11 (post-Screen-2/3 review)

Small additions surfaced while reviewing Claude Design's Screen 2 (Goal detail) and
Screen 3 (Quiz question) outputs:

- **§4.3.4** — Added `[Quiz me]` visibility rule: only shown for content pages
  (`concepts/` / `entities/` / `modules/` / `processes/` / `synthesis/`). Catalog
  pages (`index.md`, `log.md`) get `[Open]` only.
- **§4.4.4** — Added wikilink anchor handling rule: `[[page#heading]]` clicks
  navigate to the page (whole view, top); scroll-to-heading is v1.5.
- **§4.5.4** — Added letter badges (A/B/C/D) per choice row and a keyboard-shortcut
  table (A-D to select, Enter to submit, → to advance, Esc to exit). The `↵ to submit`
  hint near the Submit button is the only explicit on-screen label; letter affordances
  are implicit via the badges.

These were all inferences Claude Design made that turned out to be correct UX choices
worth promoting to spec.

### 9.3 Amendment — 2026-05-11 (post-Screen-4 review)

After Claude Design's Lobby screens (04a populated + 04b empty), one feature added to v1:

- **§4.8.1** — Drag-and-drop folder as third New Vault entry, alongside the button and
  `Cmd+N` shortcut. Drag-drop hint line shown only in Lobby. Tauri natively supports
  file-drop events (~half-day implementation, high-ROI for desktop-app feel).

The keyboard hint pattern (`⌘N` on button, `↵` on Submit) is now expected on all
primary actions across screens. No spec change needed — applied as a global convention
to the design system.

### 9.4 Amendment — 2026-05-11 (post-Screen-5 review)

After Claude Design's Cmd+K overlay (05a streaming + 05b answered), three additions
to §4.6:

- **Soft single-shot mode** (was: strict single-shot). After an answer completes the
  input bar accepts a new question; submitting starts a fresh agent run with no
  conversation memory carried over. The just-asked question is shown as muted echo
  in the input bar; typing replaces it. Net effect: user can ask successive questions
  without ESC + Cmd+K each time, while the "no memory" invariant is preserved.
- **Keyboard navigation for cited chips** — `↑` / `↓` to move focus between chips,
  `⌘↵` to open focused citation (closes overlay + navigates). Click still works too.
- **`YOU ASKED` header label** above response — keeps question visible during long
  scrollable answers.
- **Inline code styling** inside responses — monospace pill background for technical
  tokens (function names, status codes, paths). Promoted as a global content-rendering
  convention for any LLM-generated text shown in the app.

### 9.5 Amendment — 2026-05-11 (post-Screen-6 review)

Claude Design's Settings modal pass was clean except for two pieces of vocabulary that
sneaked in v2 features. Spec §4.7.3 now has explicit wording locks:

- "Default model" must NOT carry a sub-label suggesting per-goal or per-vault override
  (no such UI in v1). Removed the suggested "used unless overridden per goal" sub-label.
- "Quiz pass threshold" sub-label uses **passed (attempt-level)** vocabulary, not
  "learned" / "mastered" / "graduated" (those imply page-level state, deferred to v2).
- PII scanner pattern count badge (`regex_basic · 14 patterns`) — kept as UX detail,
  with implementation note that count must be read **dynamically** from the scanner
  registry, not hard-coded.

Three additions kept from Screen 6 (no spec change needed, just observed conventions):

- `✓ Connected` green pill for Authentication status
- Slider value with unit labels ("5 questions", "80%")
- `⌘S` save / `ESC` cancel keyboard hints on footer buttons (consistent with global
  keyboard hint convention from §9.3)

This completes the post-Screen review pass. All 6 screens reviewed; spec locked.

---

## 10. Hand-off

### 10.1 To `writing-plans` skill

After user approves this doc, hand off to `writing-plans` to break the 7 stages of §7 into
discrete tasks with explicit dependencies, file paths, and acceptance criteria.

### 10.2 To Claude Design (parallel track)

User may take this doc + codebus README to **claude.ai/design** to produce a polished interactive
prototype, which can then export a "handoff bundle for Claude Code" to feed back into
implementation. This is optional and parallel — does not block plan creation.

Recommended Claude Design prompt:

> 我要做一個叫 codebus 的 desktop app（Tauri + React + shadcn/ui + Tailwind v4 + Milkdown）。
> 產品定位：給工程師「自我探索陌生 codebase + 自我測驗」用的工具。
> 氣質：dark mode、IDE-like、極簡但有「公車隱喻」的小溫度。
>
> 附上：
> 1. 完整 UX 決定 doc（貼這份 design doc 內容）
> 2. 產品 README（貼 codebus README）
>
> 視覺參考座標：VS Code（兩態 layout）、Raycast（Cmd+K overlay）、Linear（dark 極簡）。
>
> 請產出 polished interactive prototype，重點：Lobby / Vault Workspace / Quiz flow / Cmd+K overlay。
