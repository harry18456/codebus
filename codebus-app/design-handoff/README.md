# Handoff: codebus desktop app

## Overview

**codebus** is a Tauri + React + TypeScript desktop app for software
developers exploring unfamiliar codebases. The user points the app at a
code repo, runs LLM-driven "goals" to build a structured wiki about the
codebase, then takes auto-generated multiple-choice quizzes on wiki pages
to verify their own understanding. **Self-validation is the unique value
proposition.**

This handoff covers the 6 core screens of v1:

| #   | Screen                 | Purpose                                              |
| --- | ---------------------- | ---------------------------------------------------- |
| 01  | Vault Workspace        | "Home" inside a vault — Goals overview               |
| 02a | Goal · Running         | Live stream of an executing LLM goal                 |
| 02b | Goal · Completed       | Settled goal with wiki pages it produced             |
| 03a | Quiz · Pending         | A multiple-choice question, user picking             |
| 03b | Quiz · Reviewing       | Post-submit feedback with citation                   |
| 04a | Lobby · Populated      | Vault picker with recent vaults                      |
| 04b | Lobby · Empty          | First-run state, no vaults yet                       |
| 05a | Cmd+K · Streaming      | Spotlight overlay, answer mid-stream                 |
| 05b | Cmd+K · Answered       | Settled answer with cited wiki pages                 |
| 06  | Settings · Modal       | Global preferences modal                             |

Plus a **00 · Tokens** card on the canvas showing the full design system
(colors, type, control samples).

## About the Design Files

The files in `design_files/` are **design references created in HTML** —
prototypes showing intended look and behavior, **not production code to
copy directly**. The mocks use plain React 18 + Babel-standalone + raw
CSS so they run in a single static HTML file (`index.html`) for review.
**They are not built with the target stack.**

Your task is to **recreate these designs in the target stack**:

- **Tauri v2** (desktop window, no browser chrome)
- **React 19 + TypeScript + Vite**
- **shadcn/ui + Tailwind CSS v4**
- **Milkdown** for any markdown rendering inside the Wiki area
  (not yet shown in this set; the wiki page reader is a future screen).

The CSS class names (`.cb-*`) and structures in the mock are **for the
mock only**. In the real app, map them to Tailwind utility classes +
shadcn/ui components and reuse the design tokens listed below.

## Fidelity

**High-fidelity.** All colors, spacing, type, radii, and component
behaviors are intentional and locked. Recreate pixel-perfectly using
shadcn primitives styled to match.

## Design Tokens (canonical)

The mock declares these as CSS variables in `design_files/styles.css`
(`:root`). Translate into a Tailwind v4 theme.

### Color (dark mode only — v1 has no light mode)

```
--bg              #0a0a0a   /* page bg                           */
--bg-raised       #111111   /* cards, table rows, modals         */
--bg-hover        #161616   /* hovered rows/buttons              */
--bg-active       #1a1a1a   /* pressed/active surfaces           */
--bg-sunken       #070707   /* sidebar, modal footers, code      */

--border          #1f1f1f   /* default hairline                  */
--border-strong   #2a2a2a   /* emphasized hairline               */
--border-subtle   #161616   /* in-table row dividers             */

--fg              #e5e5e5   /* primary text                      */
--fg-secondary    #8a8a8a   /* secondary text                    */
--fg-tertiary     #5a5a5a   /* metadata, paths, hints            */
--fg-quaternary   #3a3a3a   /* extremely de-emphasized text      */

--accent          #f5a623   /* AMBER — the one accent            */
--accent-hover   #ffb13d
--accent-dim     #c9881c
--accent-fg      #0a0a0a    /* fg on accent backgrounds          */
--accent-tint    rgba(245,166,35,0.10)
--accent-ring    rgba(245,166,35,0.35)

--success         #4ade80   /* completed goal status, correct    */
--warn            #f5a623
--error           #f87171   /* failed goal, wrong answer         */
--info            #60a5fa
```

**Accent usage discipline:** the amber is the *only* color used to draw
the eye. It marks the active nav rail, primary buttons, the Cmd+K
prompt glyph, in-flight stream carets, the selected quiz radio, and
correct/cited links. **Do not introduce a second accent.**

### Typography

- **UI:** Inter — 400 / 500 / 600.
  `font-feature-settings: 'cv11', 'ss01'`, antialiased,
  `letter-spacing: -0.01em` body, `-0.015em` headings, `-0.02em` large.
- **Mono:** JetBrains Mono — 400 / 500.
  `font-feature-settings: 'zero', 'ss02'`, `letter-spacing: 0`.
  Used for: file paths, vault paths, timestamps, token counts,
  quiz question numbers, choice key labels (A/B/C/D), wiki links.

**Scale (Linear-tight density):**

| Token      | Size  | Weight | Where                                 |
| ---------- | ----- | ------ | ------------------------------------- |
| body       | 13px  | 400    | row text, nav items                   |
| body-lg    | 14px  | 400    | quiz choices                          |
| meta       | 11px  | 400    | timestamps, counts, paths             |
| micro      | 10px  | 500-600| SECTION LABELS (uppercase, tracked)   |
| h-row      | 18px  | 600    | screen titles (Goals, Recent Vaults)  |
| h-detail   | 20px  | 600    | goal title                            |
| h-quiz     | 22px  | 600    | quiz question                         |
| h-empty    | 24px  | 600    | empty-state hero                      |

Section labels use:
`font-size: 10px; font-weight: 600; color: --fg-tertiary;
text-transform: uppercase; letter-spacing: 0.12em`.

### Radii

- `--r-sm: 3px` — chips, kbd, mini-badges
- `--r-md: 4px` — buttons, nav items, choice rows
- `--r-lg: 6px` — cards, tables, vault rows
- `8px` — modal cards (Cmd+K, Settings) only

### Density

Linear-tight. **Standard row height is 28px** for nav items, buttons,
selects. Table rows are 30px (running goal row expands to ~52px to fit
its inline stream line). **Spacing scale is 4 / 8 / 12 / 14 / 18 / 24 /
28 / 36px.** Resist 16/20/32.

### Borders

**Always 1px, always hairline.** Use `--border` for default cards and
`--border-subtle` for in-table dividers (which are nearly invisible —
that's intentional, rows feel almost like a list). The accent's only
"glow" is amber `box-shadow: 0 0 4px var(--accent-ring)` on the running
status dot. **No multi-stop gradients anywhere. No glassmorphism.**

## Screens

### 01 · Vault Workspace (`components/vault-workspace.jsx`)

**Purpose:** the "home" of an open vault. Defaults to Goals overview.

**Layout:** 2-column.

- **Left sidebar — 200px fixed.** Top to bottom:
  - 30px back row `← Lobby` (font-size 12, `--fg-tertiary`)
  - 1px border
  - 56px Vault block: vault name (13/600), path (mono 11, ellipsized)
  - 1px border
  - Nav (flex column):
    - 10px uppercase "VAULT" section label
    - Three rows, each 28px tall: 🚏 Goals · 📂 Wiki · 🎓 Quiz, with
      mono 11px count on the right (12 / 38 / 12 in mock)
    - **Active row** has `--bg-active` fill, `--border-strong` border, and
      a 2px amber bar on the left (positioned at `left: -6px`)
  - Footer: settings + refresh icon buttons, `⌘K` kbd chip flush right
- **Right pane — fills remaining width.**
  - 28px topbar. **Empty** — this is the Tauri window-drag region
    (`-webkit-app-region: drag`). 1px bottom border.
  - Content padded `28px 40px 40px`:
    - Header row: `<h1>Goals</h1>` 18/600 + subtitle 12 secondary on
      the left, `[+ New Goal]` primary button with `N` shortcut chip
      on the right
    - "RECENT" section label + "6 of 12" mono count
    - Goal table — 6 rows in a `--bg-raised` card with `--border`:
      - Each row: `7px status dot · title · time-or-live · kebab`
      - **Running row** (always at most one in v1) is expanded: under
        the title it shows a mono 11px "stream tail" — the current
        action being narrated, with a 1px amber blinking caret at the
        end. The right column shows `streaming · 4,218 tok` in amber.
      - **Done row:** green dot, time ago in mono (`14m ago`).
      - **Failed row:** red dot, otherwise identical.
      - Kebab opacity 0 → 1 on row hover.

**Bus metaphor dose:** medium. Only 🚏 / 📂 / 🎓 emoji in nav and 🚌 in
lobby wordmark. **No illustrated buses or route lines.**

### 02a / 02b · Goal Detail (`components/goal-detail.jsx`)

**Purpose:** drill into a single LLM-driven goal. Two states.

**Running (02a):**
- Back link "← Goals" above the title.
- Title: full goal text at 20/600 (e.g. "搞懂 auth 模組怎麼運作").
- Right side: pulsing amber dot + amber "Running" + mono `23s` ·
  `8.2k tokens`, then a danger-toned `Cancel` button (red-tinted
  border, never red fill).
- Two collapsible **Timeline sections**:
  - "READING CODEBASE" with file rows (mono path, dim file icon, time)
  - "WRITING WIKI" with wiki path rows (amber file icon, `new` /
    `updated` mini-badge, time). Live row at the bottom shows a
    spinning circle + amber narration text (e.g. "analyzing token
    validation flow…").
  - Each section has a 1px-left-border guide with 8px tick marks
    pointing into each row (`::before` on `.cb-tl-row`).
- Bottom: collapsible "stream log" card — when open, a mono 11.5px
  log with `00:00.142` timestamps and color-coded `goal` / `plan` /
  `read` / `write` tags. Closed by default.

**Completed (02b):**
- Same header layout. Status line reads `Completed in 47s · 14.3k tokens`
  with a green `✓ Done` pill instead of Cancel.
- Replaces the timeline with a **"WIKI PAGES CHANGED"** card listing
  every wiki page the goal wrote to. Each row: amber pencil icon,
  mono path, `new` / `updated` badge, two action buttons on the right
  (`Open`, plus `Quiz me` in amber tint for pages with quizzes).
- Same collapsible stream log at the bottom.

### 03a / 03b · Quiz (`components/quiz.jsx`)

**Purpose:** the self-validation step.

**Layout:** centered 640px column on the right pane.

**Pending (03a):**
- Header strip: `Quiz: auth-middleware` (mono) on the left,
  `Q3 of 5` counter on the right. 1px bottom border, 36px margin.
- Question: `Q3.` mono num (18/500 dim) + question text (22/600).
- Four choice rows, 44px tall, each:
  `[A] (10/mono boxed key) · radio · label · optional tag`.
  Tapping or pressing the letter selects it.
  Selected row has amber border + inset amber ring +
  `--accent-ring` glow halo. The keyboard letter chip turns amber too.
- Bottom row: `⏎ to submit` hint on the left, amber `Submit`
  button on the right.

**Reviewing (03b):**
- Same header.
- Correct choice shows green border + green check radio + green
  `correct` tag. The user's wrong pick shows red border + ✕ radio +
  red `your answer` tag. Other choices fade to ~55% opacity.
- A **citation blockquote** appears below the choices: 2px amber
  left border, `--bg-raised` fill, an amber `"` opening glyph,
  the cited quote, and a dashed-underline mono wikilink in amber
  (e.g. `[[auth-flow#middleware]]`).
- Footer: `← Back to wiki page` on the left, amber `Next: Q4 →`
  on the right.

**Keybindings (both states):**
- Pending: `A` `B` `C` `D` to select, `⏎` to submit.
- Reviewing: `→` to advance.
- Both: `←` link is mouse-only.

### 04a / 04b · Lobby (`components/lobby.jsx`)

**Purpose:** the no-vault-open root state of the app.

**Topbar (44px):** 🚌 + "codebus" wordmark on the left. On populated,
`[+ New Vault]` primary button with `⌘N` shortcut on the right;
the empty state hides the topbar button (the CTA is in the hero).

**Populated (04a):**
- Centered 640px column.
- "RECENT VAULTS" section header + count.
- Vault cards — 1px border, `--bg-raised`, 12/14 padding,
  hover lifts border to `--border-strong`:
  - Row 1: vault name (14/600) flush left, vault path (mono 11.5,
    tertiary) flush right
  - Row 2: "last opened" + mono age (`2h ago`, `yesterday`, `3d ago`)
  - Hover-revealed kebab on the right
- Below the list, a dashed-top hint row:
  `tip · Drag a repo folder anywhere into this window to open it as a vault.`

**Empty (04b):**
- Centered 440px hero:
  - 🚌 56px emoji
  - "來搭第一台公車吧" — 24/600 heading
  - One-line subtitle (13 secondary)
  - Larger amber `+ Board a new bus` primary CTA (32px tall)
  - Below: a **Quickstart** card (1px border, raised bg, padding
    14/18) with "QUICKSTART" label + 3 numbered steps. Step 2
    includes an inline amber-tinted quote pill of the example goal
    text (`搞懂這 repo 的 X`).
- **Both states share a 32px lobby foot** at the very bottom:
  `[gear] Settings` link on the left, `v0.1.0` version mono on the right.

### 05a / 05b · Cmd+K Overlay (`components/cmdk-overlay.jsx`)

**Purpose:** spotlight-style Q&A against the wiki.

**Backdrop:** Vault Workspace screen behind, `filter: blur(6px)
saturate(0.85)` + `transform: scale(1.02)`, plus a
`rgba(0,0,0,0.62)` scrim with a light backdrop-blur.

**Card:** centered at 12% from top, `min(720px, 80%)` wide, max
70% tall, `--bg-raised`, 8px radius, heavy two-layer drop shadow.

- **Response area** (scrollable):
  - "YOU ASKED" mono tag in amber + the question text (1px dashed
    bottom border separator)
  - Answer body at 14.5px / 1.65 line-height. Inline `<code>` chips
    have border + raised bg. Inline status codes (`401`) shown in
    red-tinted mono kbd-ish chip.
- **Streaming state (05a):** answer truncates mid-sentence; a 7×14
  amber caret blinks at the cursor.
- **Answered state (05b):** full answer settled. Below the body,
  a **"CITED"** section with arrow caret + label + count, and
  **chip-shaped citation links** (`page icon + mono path`,
  pill radius, border, hover → amber border + tint).
- **Input bar (48px):** flush bottom of the card.
  `›` amber prompt · the question text · `⏎` glyph right-flush.
- **Footer hints** outside the card, centered ~28px from bottom of
  viewport: `↑↓ nav cited` · `⌘⏎ open citation` · `ESC to close`.
  Hint kbds are dim white.

### 06 · Settings Modal (`components/settings.jsx`)

**Purpose:** global preferences, opened from the lobby gear or any
in-vault sidebar gear.

**Backdrop:** the Vault Workspace, unblurred this time, with a
solid `rgba(0,0,0,0.55)` scrim.

**Card:** 560px wide, centered, max 86% tall, 8px radius.

- **Header (44px):** "Global Settings" title 13/600 + close × icon.
- **Body (scrollable):** form rows in a 2-column grid
  `168px | 1fr`, separated by 1px subtle dividers, 14px/22px
  padding. Each row: label (+ optional secondary help text in
  tertiary 11px) on the left, control(s) on the right.

  Rows in v1, exact:

  1. **AI Provider** — text `Claude CLI` + mono aux
     `only option for now`. No control.
  2. **Authentication** — green `✓ Connected` pill (mono, 11px,
     11px-radius pill, green tint border+bg) and a
     `Re-authenticate…` dashed-underline link button.
  3. **Default model** — help text `applies to all runs`. Three
     stacked sub-rows (`goal` / `query` / `fix` mono sub-labels at
     56px, then a select). Selects show `sonnet` / `haiku` / `sonnet`.
  4. **PII scanner** — single select showing
     `regex_basic · 14 patterns`. The pattern count must come from
     the real registry at runtime (the mock count is illustrative).
  5. **Log sink** — `~/.codebus/logs/` shown as a mono path chip
     (raised bg + border) + `Change folder…` link.
  6. **Quiz pass threshold** — help text `% correct to pass a quiz
     attempt`. Slider 50–100, default 80, value readout `80%`.
  7. **Default quiz length** — slider 3–10, default 5, value
     readout `5 questions`.

- **Footer (`--bg-sunken`):** mono note on the left —
  `Reads/writes ~/.codebus/config.yaml`. Two buttons right:
  `Cancel ESC` (default) + `Save ⌘S` (primary amber).

**Slider component (`Slider`):**
- 4px track (`--bg-active`), amber `.cb-slider-fill` from left,
  12px round thumb with 2px amber border and a 3px amber-ring halo.
- Value readout aligned right of the track (e.g. `80%`,
  `5 questions`).
- Range labels (`50–100%`, `3–10`) sit *under* the track in tertiary mono.
- An invisible native `<input type=range>` overlays the track for
  keyboard/mouse drag.

## Reusable Components

| Component         | Where defined                          | Used in screens         |
| ----------------- | -------------------------------------- | ----------------------- |
| `Sidebar`         | `components/sidebar.jsx`               | 01, 02a, 02b, 03a, 03b  |
| `IconPlus/Settings/Refresh/Filter/Kebab/Check` | `components/icons.jsx` | all                     |
| `Select`          | inline in `settings.jsx`               | 06                      |
| `Slider`          | inline in `settings.jsx`               | 06                      |
| `TimelineSection` | inline in `goal-detail.jsx`            | 02a                     |
| `StreamLog`       | inline in `goal-detail.jsx`            | 02a, 02b                |
| `cb-pill`         | class                                  | 02b, 06                 |
| `cb-mini-badge`   | class                                  | 02a, 02b                |
| `cb-back-link`    | class                                  | 02a, 02b, 03b           |
| `cb-cmdk-backdrop` + `cb-cmdk-scrim` | class               | 05, 06                  |

When recreating in shadcn:

- `cb-btn` / `cb-btn.primary` → shadcn `<Button>` with custom
  `variant="default"` and a `variant="primary"` styled to amber.
- `cb-pill` → small `<Badge>` variants.
- `cb-select` → `<Select>` with `<SelectTrigger>` styled flat.
- `cb-slider` → shadcn `<Slider>`; restyle thumb + add amber ring.
- `cb-modal-card` → `<Dialog>` with custom max-width.
- `cb-cmdk-card` → `<Command>` from cmdk + custom layout.
- Tooltips for shortcut chips → `<Tooltip>` if you want them
  interactive (the mock uses inline `.cb-kshort` and `.cb-kbd-inline`).

## Interactions & Behavior

### Quiz keyboard
- Pending (Q open):
  - `A` / `B` / `C` / `D` → set selected
  - `⏎` → submit (in mock flashes the Submit button via `cb-flash`
    animation; real app transitions to Reviewing)
- Reviewing:
  - `→` → advance (flashes the Next button)

### Cmd+K
- `⌘K` from anywhere opens the overlay.
- While streaming, the caret blinks (`cb-caret` keyframe, 1s steps(2)).
- `↑` `↓` navigate the cited chips (when present).
- `⌘⏎` opens the focused citation in the Wiki area.
- `ESC` dismisses; the backdrop click also dismisses.

### Goal rows
- Hover reveals the `⋮` kebab and surfaces `--bg-hover`.
- The running row's stream tail uses a `mask-image` gradient to
  fade the right edge so partial words don't read as clipped.

### Vault rows (Lobby)
- Whole card click → open vault. Hover-revealed kebab → secondary
  actions (rename, remove from recent, reveal in Finder).
- A repo folder dropped anywhere on the empty/populated lobby
  opens it as a new vault (mock hint copy says this — implement as
  a window-level drop target).

### Settings
- All controls are *editable* in v1; values persist to
  `~/.codebus/config.yaml`. The mock's values are seed defaults.
- The "Default model" sub-rows let the user pick which Claude model
  is used for which agent step (`goal` plans + executes, `query`
  answers Cmd+K, `fix` is reserved for repair runs). **This is the
  full per-step model choice — there is NO per-goal model override
  in v1.** That's why the row's help text reads "applies to all runs."
- The quiz pass threshold (50–100%) controls whether a quiz attempt
  is recorded as `passed`. v1 stores pass/fail **per attempt**, not
  per-page. No "learned" state, no streak, no spaced repetition.

### Animations & timing

| Animation        | Where                                  | Duration / curve              |
| ---------------- | -------------------------------------- | ----------------------------- |
| `cb-pulse`       | running status dot, goal Run pulse     | 1.4s ease-in-out infinite     |
| `cb-caret`       | stream caret + Cmd+K caret             | 1s steps(2) infinite          |
| `cb-spin`        | live-row spinner                       | 0.9s linear infinite          |
| `cb-flash`       | Submit/Next button press feedback      | 180ms ease-out                |
| transitions      | row hover bg, border, opacity          | 120ms (everywhere)            |
| transitions      | choice border + bg in quiz             | 120ms                         |

## State Management (v1)

Roughly what the React/TS app needs to hold:

- `vaults: Vault[]` (lobby)
- `currentVaultId: string | null`
- per-vault:
  - `goals: Goal[]` with one optional `runningGoalId`
  - `wikiPages: Page[]` (paths + metadata, contents loaded on demand)
  - `quizAttempts: Attempt[]` (per page, history)
- `commandPaletteOpen: boolean` + last query + last cited chip ids
- `settingsOpen: boolean` + form draft (saved to `config.yaml`)
- LLM stream state for the running goal:
  - `currentAction: string` (what's streamed under the row)
  - `tokensUsed: number`
  - `events: StreamEvent[]` for the collapsible log

There is **never** more than one running goal at a time in v1.

## Routing & Window Behavior

- The app is a Tauri v2 window — **no browser chrome**.
- The 28px topbar in `.cb-main` and the 44px lobby topbar must be
  set as the Tauri drag region (`data-tauri-drag-region` on the
  outer container of those bars, or `-webkit-app-region: drag` if
  using a webview drag handle).
- The lobby is its own route. Selecting a vault navigates to the
  workspace; the sidebar's `← Lobby` button returns.
- Inside a vault, Goals / Wiki / Quiz are tabs (no separate URLs
  needed in v1 — Tauri window, in-app state).
- The Cmd+K overlay and Settings modal are **layered above** the
  current screen; they do not unmount the underlying view (so the
  backdrop blur trick actually has something to blur).

## Filesystem Contract

The mock references these real paths; they must exist in the
implementation:

- `~/.codebus/config.yaml` — global settings persist here
- `~/.codebus/logs/` — log sink
- `<vault>/.codebus/` — per-vault state (wiki pages, quiz
  attempts, indexes). Existence of this folder is what makes a
  directory a "vault."

## Assets

**None used.** All glyphs are inline SVG (icons in
`components/icons.jsx`) or system emoji (🚌 🚏 📂 🎓). Replace
emoji with first-party iconography only if you bring in matching
amber-tinted variants — otherwise keep the emoji, they read fine
in Linear-tight dark mode.

There is **no logo file** to ship. The wordmark is plain text
("codebus") in Inter 13/600 next to the 🚌 emoji.

## Out of Scope (deferred to v2+, do NOT add UI for)

These features were considered and explicitly cut from v1.
Do not add controls or surfaces for them:

- Per-goal model override (the Settings row only sets defaults)
- Per-page "learned" state, streaks, spaced repetition
- Quiz history graph
- Goal queueing — v1 runs at most one goal at a time
- Multiple AI providers — Claude CLI is the only option
- Light theme
- Tags / folders / search inside the wiki tree (the Wiki tab is
  still TBD — separate handoff)
- Sharing / export

## Files in this Bundle

```
design_handoff_codebus/
├── README.md                           ← this file
└── design_files/
    ├── index.html                      ← canvas entry; opens all screens
    ├── styles.css                      ← all design tokens + every class
    ├── design-canvas.jsx               ← canvas wrapper (not part of app)
    └── components/
        ├── icons.jsx                   ← icon set used across screens
        ├── sidebar.jsx                 ← shared 200px sidebar
        ├── tokens-card.jsx             ← 00 · Tokens preview card
        ├── vault-workspace.jsx         ← 01
        ├── goal-detail.jsx             ← 02a + 02b
        ├── quiz.jsx                    ← 03a + 03b
        ├── lobby.jsx                   ← 04a + 04b
        ├── cmdk-overlay.jsx            ← 05a + 05b
        └── settings.jsx                ← 06
```

To preview the mock locally:

```sh
cd design_files
python3 -m http.server 8000
open http://localhost:8000
```

You'll see all 11 artboards laid out on a single pannable canvas
(scroll/drag to navigate, click any artboard's `↗` to focus).

## Cross-Screen Consistency Notes

Final consistency pass (May 2026) confirmed:

- Sidebar identical across screens 01, 02a/b, 03a/b. Nav counts
  match (Goals 12 / Wiki 38 / Quiz 12).
- 28px topbar is **drag-only**, no content — consistent everywhere
  the workspace appears.
- Back-link pattern (`← Goals`, `← Lobby`, `← Back to wiki page`)
  is the same component visually.
- Cmd+K overlay and Settings modal share the same backdrop
  technique; Cmd+K blurs, Settings darkens.
- No deferred-feature UI present — verified against the "Out of
  Scope" list above.
- Bus metaphor stays at the "medium" dose: emoji in nav + lobby
  wordmark + quickstart copy. No illustrated route lines or
  full-bleed bus imagery.

---

Questions about anything in here? The mock's class names map 1:1
to the descriptions above — when in doubt, inspect the rendered
artboard with browser devtools to see exact computed values.
