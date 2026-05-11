# Claude Design — codebus app v1 prompts

> Companion to `docs/2026-05-11-app-ux-flow-design.md`.
> Copy-paste sections directly into [claude.ai/design](https://claude.ai/design).
> Treat the design doc as source of truth — these prompts are the execution layer.

---

## How to use this file

1. Open [claude.ai/design](https://claude.ai/design)
2. Start a new design session
3. Work through the **6 screens below in order** (each one informs the next)
4. After all screens are locked, export "handoff bundle for Claude Code"
5. Bring the bundle back to this repo / Claude Code session → implementation phase

**Why this order matters:** screen #1 sets the visual language. Screens #2-6 build on it via "same visual language as before". Don't skip ahead.

---

## General principles (read once before starting)

- **One screen at a time.** Don't dump the whole spec — get one screen right, then build on it.
- **Use anti-pattern lists.** Each prompt has an `AVOID:` section. Underspecification is the #1 cause of generic AI design output.
- **Reference real apps, not adjectives.** "Like Linear" beats "minimal and clean".
- **One fix per inline comment.** Don't pile up corrections — it over-corrects.
- **Use design vocabulary, not vibes.** "Tighter line-height" beats "feels weird".
- **Export early.** After screens #1 and #2, do one test export to see what the handoff bundle looks like. Adjust later screens to match the export format.
- **Recalibrate the vibe at the start.** If screen #1 vibe is off, regenerate — don't try to patch with comments.

---

## Pre-flight: paste this context first

Before any screen, open the design session and paste this **context block** as the opening message. It primes the model with constraints.

```
I'm designing a desktop app called "codebus" — a Tauri+React+TypeScript
desktop app for software developers exploring unfamiliar codebases.

Core product idea: developer points the app at a code repo, runs LLM-driven
"goals" to build a structured wiki about the codebase, then takes
auto-generated multiple-choice quizzes on wiki pages to verify their own
understanding. Self-validation is the unique value proposition.

Tech stack constraints (must work within these):
- Tauri v2 (so it's a desktop window, no browser chrome)
- React 19 + TypeScript + Vite
- shadcn/ui + Tailwind CSS v4
- Milkdown for markdown rendering

Visual references — aim for this *family* of products:
- VS Code (two-state: welcome / open-folder; sidebar layout)
- Linear (dark minimal, tight density)
- Raycast (Cmd+K spotlight overlay)
- Zed editor (developer-first dark IDE aesthetic)

NOT this family:
- Notion (too marketing/airy)
- Slack (too colorful)
- Any landing-page hero / gradient / glassmorphism aesthetic

Personality: dark mode IDE-like, tight type, monospace where appropriate
(file paths, code), just enough warmth from a "bus / station" metaphor
(🚌 🚏 emoji acceptable; no full whimsical illustration treatment).

I'll show you screens in sequence — please keep visual language consistent
across them.
```

---

## Screen 1 · Vault Workspace main view

> Most important screen — sets visual language for everything else.

```
Design the main view of codebus.

Context: the user has just opened a vault called "uv" (a Rust-based Python
package manager). The vault is at /work/uv.

Layout: 2-column.
- LEFT sidebar: ~180px wide, fixed (not resizable in v1)
- RIGHT main area: fills the rest of the window

Sidebar contents (top to bottom):
1. "← back to lobby" small link at very top
2. Vault display name "uv" (bold)
3. Repo path "/work/uv" (smaller, muted, truncated if needed)
4. Divider
5. Three nav items (only one active at a time):
   - 🚏 Goals      ← active state shown here
   - 📂 Wiki
   - 🎓 Quiz
6. (vertical space filler)
7. ⚙ gear icon at very bottom (Settings entry)

Main area shows "Goals overview" (default content):
- Top bar: "Goals" title on left, [+ New Goal] primary button on right
- Section label: "RECENT" (small caps, muted)
- 4 goal rows, each containing:
  - 🚏 emoji prefix
  - Goal text in primary color, e.g. "搞懂 cache invalidation"
  - Below: muted small text "12 minutes ago · 3 wiki pages changed"
- After the 4 recent rows: muted divider "── earlier ──"
- 2 more older goal rows

Style requirements:
- True dark mode (#0a0a0a or similar background, not blue-tinted)
- IDE density: tight line-height, ~13-14px body text, paths in monospace
- shadcn-style buttons (subtle, not flashy)
- Active sidebar item: subtle background fill, not a heavy accent stripe

AVOID:
- Hero treatment, gradients, glassmorphism, drop shadows
- Pastel or candy colors
- Oversized whitespace / breathing room
- Mascot illustrations
- Marketing-page aesthetic anywhere
```

**Lock screen 1 before moving on.** If vibe is off, regenerate with tighter constraints.

---

## Screen 2 · Goal detail view (running + completed)

```
Same visual language as the previous screen.

Now design the Goal detail view — the screen shown when the user clicks
into a goal row (running or completed) from the Goals list (Screen 1).

IMPORTANT: this is NOT a "main-area takeover when a goal runs". The list
in Screen 1 already shows the inline mini-stream for a running goal. This
detail view is the *drill-down* opened by explicit click — surfaces the
full timeline, raw log, and Cancel/Retry actions.

Show TWO sub-states side-by-side (or stacked as artboards):

SUB-STATE A — goal currently running:

Header strip:
- "← back" link on left (returns to Goals list)
- Goal text "搞懂 auth 模組怎麼運作" (large, primary)
- Right side: live counter "Running · 23s · 8.2k tokens"
- [Cancel] button on far right (subtle, danger-tinted but not red-screaming)

Body — structured event timeline (vertical, scrollable):
- Section "▶ Reading codebase":
   - 📄 src/auth/middleware.ts
   - 📄 src/auth/jwt.ts
   - 📄 src/auth/session.ts
- Section "▶ Writing wiki":
   - ✏ modules/auth-middleware.md  (badge: "new")
   - ✏ concepts/jwt-token-lifecycle.md  (badge: "new")
   - ✏ index.md  (badge: "updated")
- Current activity line at the bottom of the visible activity:
   - ⠋ animated spinner + "analyzing token validation flow…"

Below the timeline:
- A collapsed section header: "▼ stream log"
- (When expanded — show the affordance — raw agent output in monospace ~12px)

SUB-STATE B — goal completed (success case):

Header strip:
- "← back" link on left
- Goal text "搞懂 auth 模組怎麼運作"
- Right side: muted summary "Completed in 47s · 14.3k tokens"
- Status pill far right: "✅ Done" (small, green-tinted)

Body:
- Section heading: "Wiki pages changed (3)"
- List of changed pages, each row:
   ✏ modules/auth-middleware.md   (badge: "new")
   [Open] [Quiz me]               ← inline buttons per row
   ✏ concepts/jwt-token-lifecycle.md   (badge: "new")
   [Open] [Quiz me]
   ✏ index.md                     (badge: "updated")
   [Open]
- At the bottom: collapsed "▼ stream history" section (timeline + raw log
  from the run, collapsed by default; expand to review).

(Optional 3rd state — goal failed: same as B but status pill is "✕ Failed",
error reason shown, and [Retry with same goal] button replaces the
per-page actions. Skip if it complicates the canvas.)

Vibe: feels like clicking into a CI run on GitHub Actions or a build
detail on Vercel — informative and calm. NO progress bars (we don't
know %).

AVOID:
- Loading dots / "AI thinking" animations
- Progress percentages
- Cute mascots / wait-screen illustrations
- Pulsing whole-screen treatments
- Auto-redirect or auto-celebrate when goal finishes (user explicitly
  returns to list via "← back")
```

---

## Screen 3 · Quiz question screen

> USP screen — must feel right.

```
Same visual language as previous screens.

Design the quiz-taking screen. The user is mid-quiz on the wiki page
"auth-middleware.md".

Show TWO states side-by-side or as a labeled before/after:

STATE A — question pending:
- Header bar (small): "Quiz: auth-middleware" on left, "Q3 of 5" on right
- Big question text: "Q3. Where does authentication start?"
- Four choices, radio-style:
   ○ In the controller
   ● In the middleware           ← selected
   ○ In the database layer
   ○ In the frontend
- [Submit] button at bottom-right

STATE B — after submit (showing wrong answer case):
- Same header and question
- Choices now revealed:
   ❌ "In the controller" — labeled "your answer"  (this is wrong;
     in real flow user picked B in state A — but show wrong case here
     for illustration)
   ✅ "In the middleware" — labeled "correct"
   ○ "In the database layer"
   ○ "In the frontend"
- Blockquote below choices: "Auth middleware runs before route handlers
  per [[auth-flow#middleware]]."
- Bottom row: [← Back to wiki page] link on left, [Next: Q4 →] button on right

Vibe: focused single-task moment. Exam-like clarity but warmer than
a Google Form. Tight, calm. No celebration animations, no "great job!"
toasts.

AVOID:
- Quiz-show aesthetics (Kahoot-style colorful tiles)
- Confetti / celebration moments
- Big "CORRECT!" / "WRONG!" banners
- Progress bars on the question (the Q# counter is enough)
```

---

## Screen 4 · Lobby + Empty state

```
Same visual language as before.

Design the Lobby (the first state of the app, before any vault is open).

Show TWO states:

STATE A — populated lobby (user has 3 vaults already):

Layout: single column, centered or left-aligned.
- Top bar: "🚌 codebus" title on left, [+ New Vault] button on right
- Section label: "RECENT VAULTS"
- Three vault cards stacked, each containing:
   - Vault display name (bold) + path (muted, right-aligned)
   - Below: "last opened 2h ago" (muted small text)
- Bottom strip: ⚙ "settings" link on left, "v0.1.0" version label on right

STATE B — empty state (first-ever launch, zero vaults):

Layout: centered content.
- Large 🚌 emoji (not illustrated — actual emoji, generous size)
- Title: "來搭第一台公車吧" (or "Board your first bus" in English)
- Subtitle (smaller, muted): "codebus 把 LLM 探索程式碼的中間態持久化成你的旅遊書。"
- Primary button: [+ Board a new bus]
- Below the button, a small "Quick start" card:
   1. Pick a repo folder
   2. Run a goal: "搞懂這 repo 的 X"
   3. Quiz yourself to verify

Both states share the same bottom strip with settings link.

AVOID:
- Onboarding carousel
- Animated illustrations
- Tutorial popovers
- Customer testimonials
- Anything that says "Welcome!" in a marketing voice
```

---

## Screen 5 · Cmd+K query overlay

```
Same visual language. This is an overlay that appears on top of the
Workspace view.

Design a spotlight-style overlay triggered by Cmd+K (Mac) / Ctrl+K (Win).

Layout: full-window overlay over a blurred workspace background.
- Center the content card: ~720px wide, max ~70% viewport height
- Background outside card: backdrop-filter blur of the workspace
- Card itself: solid dark, NOT translucent (readability priority)

Card contents, top to bottom:
1. Response area (scrollable):
   - Streaming LLM answer text (well-typed prose, slightly larger than body)
   - Below the answer, a "▶ Cited:" section with chip-style links:
     📄 modules/auth-middleware.md
     📄 concepts/jwt-token-lifecycle.md
2. Divider
3. Input bar at bottom:
   - Prompt indicator ">"
   - Input placeholder: "你想知道什麼？" or "Ask anything…"
   - Enter glyph "⏎" on the right

Outside the card, very subtle text at the bottom: "ESC to close"

Reference: macOS Spotlight, Raycast root search, Linear command menu.

Treat this as a focused over-everything moment — the user wants to ask
one question and get one answer, then close.

AVOID:
- Tabs / multiple panels inside the card
- "History" or sidebar within the overlay
- Auto-suggestions / typeahead dropdown
- Chat-history scrollback (this is single-shot, not conversation)
```

---

## Screen 6 · Settings modal

```
Same visual language. This is a modal triggered by the ⚙ gear icon.

Design the Global Settings modal.

Layout: modal centered over a slightly dimmed workspace. Modal width ~560px.

Modal contents:
- Header: "Global Settings" title on left, [X] close on right
- Body: two-column form layout (label on left, control on right)

Fields in order:
1. AI Provider          | Plain text: "Claude CLI (only option for now)"
2. Authentication       | "Connected ✅" + small [Re-authenticate...] link button
3. Default model        | Three rows of dropdowns:
                        |   goal   [sonnet ▾]
                        |   query  [haiku ▾]
                        |   fix    [sonnet ▾]
4. PII scanner          | Dropdown: [regex_basic ▾]
5. Log sink             | Path display "~/.codebus/logs/" + [Change folder...] link
6. Quiz pass threshold  | Slider 50%-100%, value "80%"
7. Default quiz length  | Slider 3-10, value "5"

Footer:
- Muted small text on left: "Reads/writes ~/.codebus/config.yaml"
- Right side: [Cancel] secondary button + [Save] primary button

AVOID:
- Tabs / sidebars within the modal
- Section headers between fields (the field labels are enough)
- Help-tooltip icons everywhere (assume user knows what they're doing)
- Save-on-change autopilot (use explicit Save button)
```

---

## After all 6 screens are locked

### Cross-check against the design doc

Before exporting, verify the design output doesn't include any deferred features:

- [ ] No quest banner or quest-related UI on any screen
- [ ] No graph view entry in sidebar
- [ ] No theme / language switcher in Settings
- [ ] No "vault-specific settings" section in any Settings UI
- [ ] No back button in wiki preview (verify if you have a wiki preview design)
- [ ] No multi-vault sidebar inside Workspace (vault list is Lobby-only)
- [ ] No chat-mode for Cmd+K (single-shot only)
- [ ] No tutorial / slideshow UI (that's v1.5)

If any of these snuck in: add a comment "remove this — deferred to v1.5/v2 per design doc §1.4" and regenerate.

### Export the handoff bundle

Use Claude Design's export → **Handoff bundle for Claude Code**.

The bundle should give you:
- React component code (shadcn/ui patterns expected)
- Tailwind class usage
- Probably some design tokens / theme config
- Interaction sketches (hover, active, etc.)

### Bring it back

Place the bundle somewhere in the repo, e.g.:

```
codebus-app/design-handoff/
  ├─ screens/
  │   ├─ 01-vault-workspace.tsx
  │   ├─ 02-goal-detail.tsx
  │   ├─ 03-quiz-question.tsx
  │   ├─ 04-lobby.tsx
  │   ├─ 05-cmdk-overlay.tsx
  │   └─ 06-settings.tsx
  ├─ tokens/ (design tokens, if any)
  └─ README.md (Claude Design export note)
```

Then open a fresh Claude Code session (or continue this one) and say:

> "I've exported a Claude Design handoff bundle to `codebus-app/design-handoff/`.
> Please review it against `docs/2026-05-11-app-ux-flow-design.md`,
> flag any divergences from the spec, then invoke `writing-plans` skill to
> create the implementation plan with the design-handoff as a visual reference."

---

## Reminder

Claude Design output is **a visual reference, not a contract**. The design doc remains source of truth. If Claude Design produces something prettier-but-different from the spec, **spec wins**, unless the design surfaces a real UX improvement worth amending the spec for (in which case: discuss, update the spec, then proceed).
