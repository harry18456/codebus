# design-system Specification

## Purpose

TBD - created by archiving change 'design-foundation-tokens-and-section-label'. Update Purpose after archive.

## Requirements

### Requirement: Typography scale tokens

The `codebus-app` design system SHALL expose a typography scale via Tailwind v4 `@theme` tokens in `codebus-app/src/styles/tokens.css`. Each token name MUST follow the `--text-<role>` convention so that Tailwind v4 auto-generates the matching `text-<role>` utility class.

The scale MUST define the following roles and pixel values:

| Token | Pixel value | Role |
| ----- | ----------- | ---- |
| `--text-body` | 14px | Default row text, nav items, paragraph copy |
| `--text-body-lg` | 15px | Quiz choices and other emphasized body text |
| `--text-meta` | 12px | Timestamps, counts, file paths, secondary metadata |
| `--text-micro` | 11px | Section labels (uppercase/tracked), small chips |
| `--text-h-row` | 20px | Screen titles (e.g. "Goals") |
| `--text-h-detail` | 22px | Goal detail titles |
| `--text-h-quiz` | 24px | Quiz question text |
| `--text-h-empty` | 28px | Empty-state hero headings |

#### Scenario: Tailwind utility generation

- **WHEN** a developer writes `<p className="text-body">…</p>` in a `.tsx` file under `codebus-app/src/`
- **THEN** the rendered element has `font-size: 14px` applied via Tailwind v4 auto-generated utility, with no additional Tailwind configuration

#### Scenario: Hard-coded font-size sweep target

- **WHEN** a developer runs `grep -rn "text-\[" codebus-app/src --include="*.tsx"` after the sweep lands
- **THEN** the count of matches is less than 30, and every remaining match is either a large emoji glyph (≥ 56px) or carries an inline comment justifying the deviation from the scale

---
### Requirement: Border color tokens

The design system SHALL expose three semantic border color tokens in `codebus-app/src/styles/tokens.css`:

- `--color-border`: default visible hairline, value `#2a2a2a`
- `--color-border-subtle`: weakest separator, value `#161616` (unchanged from prior baseline)
- `--color-border-hairline`: explicit "almost invisible" separator reserved for in-card row separation, value `#1f1f1f`

The design system SHALL continue to define `--color-border-strong` with value `#2a2a2a` (identical to `--color-border`) for backward compatibility, so callers using either token render identically. Removing `--color-border-strong` SHALL be deferred to a later change.

#### Scenario: Hairline visibility on Windows ClearType

- **WHEN** the `codebus-app` dev build runs on a Windows machine at 1920×1080 resolution with 100% display scaling
- **THEN** every DOM element using `border-border` renders a 1px line visible to a viewer with normal eyesight from typical desktop viewing distance

#### Scenario: Explicit hairline opt-in

- **WHEN** a developer wants a near-invisible row separator inside an existing card
- **THEN** they use `border-border-hairline` explicitly, and the rendered color is `#1f1f1f` rather than the new default `#2a2a2a`

---
### Requirement: SectionLabel component

The design system SHALL provide a `SectionLabel` React component at `codebus-app/src/components/ui/SectionLabel.tsx` with the following public API:

```tsx
export interface SectionLabelProps {
  variant?: "default" | "caps";
  count?: number | string;
  className?: string;
  children: React.ReactNode;
}

export function SectionLabel(props: SectionLabelProps): JSX.Element;
```

The component MUST render its visual amber bar via a CSS pseudo-element (`::before`) so the bar does not appear in the React tree or in the accessibility tree.

The `default` variant MUST render text at 12px / weight 500 / `var(--fg-secondary)` with no uppercase transform and no letter-spacing tracking, preceded by a 2px wide × 12px tall amber bar using `var(--accent)`.

The `caps` variant MUST render text at 11px / `var(--fg-tertiary)` with `text-transform: uppercase` and `letter-spacing: 0.08em`, while keeping the same 2px amber bar from the default variant.

When `count` is provided, the component MUST render the count as a right-aligned mono-font 11px element styled with `var(--fg-tertiary)`, placed via `margin-left: auto` so it consumes remaining row width.

When `className` is provided, the component MUST merge the caller's classes with its own internal class names without overwriting them.

#### Scenario: Default variant render

- **WHEN** a developer renders `<SectionLabel>最近</SectionLabel>`
- **THEN** the DOM contains a single inline-flex `<span>` with the text "最近", and the rendered output shows an amber 2px bar to the left of the text

#### Scenario: Caps variant render

- **WHEN** a developer renders `<SectionLabel variant="caps">Modules</SectionLabel>`
- **THEN** the rendered text is uppercase with `letter-spacing: 0.08em` and font size 11px

#### Scenario: Count rendering

- **WHEN** a developer renders `<SectionLabel count={3}>最近</SectionLabel>`
- **THEN** the rendered output contains the label "最近" on the left and a mono-font "3" on the right of the same row, with no extra wrapping element exposed to screen readers between them

#### Scenario: Accessibility — amber bar is decorative

- **WHEN** a screen reader traverses a rendered `<SectionLabel>` element
- **THEN** the screen reader announces only the children text (and the count value if provided), and never announces the amber bar as content

---
### Requirement: Hard-coded font-size sweep convention

When a developer writes new code in `codebus-app/src/`, they SHALL use a typography token utility (`text-body`, `text-meta`, etc.) instead of an inline `text-[Npx]` Tailwind arbitrary value, unless the size falls into one of the following exemptions:

1. Large decorative glyphs (≥ 56px), typically emoji used as hero visuals
2. Sizes that intentionally deviate from the scale for a documented design reason

For each exemption, the code MUST carry an inline comment stating the reason (e.g. `// large glyph, intentionally outside type scale`).

#### Scenario: Reviewer catches scale violation

- **WHEN** a reviewer reads a diff that introduces `text-[13px]` for body copy
- **THEN** the reviewer flags it because the diff does not include a justification comment, and the scale already covers body copy via `text-body`

#### Scenario: Reviewer accepts documented exemption

- **WHEN** a reviewer reads a diff that introduces `text-[64px]` for a hero emoji together with a comment `// large glyph, intentionally outside type scale`
- **THEN** the reviewer accepts the deviation because the exemption is recorded inline

---
### Requirement: Design tokens are the single source for color, typography, and border

All `codebus-app/src/` components SHALL consume color, typography, and border values via Tailwind utilities backed by `tokens.css` `@theme` tokens. Components MUST NOT define equivalent hex color literals, font-size pixel literals, or border-color hex literals inline when a matching token exists.

#### Scenario: Token override drives global change

- **WHEN** a maintainer changes `--text-body` from 14px to 15px in `tokens.css`
- **THEN** every component using `text-body` re-renders at the new size on the next build, with no per-component edit required

---
### Requirement: Status semantic color tokens

The design system SHALL expose four semantic status color tokens in `codebus-app/src/styles/tokens.css` inside the `@theme` block. Each token name MUST follow the `--color-status-<state>` convention so that Tailwind v4 auto-generates the matching `text-status-<state>`, `bg-status-<state>`, and `border-status-<state>` utility classes.

The tokens MUST alias the existing color tokens as follows:

| Token                          | Aliases             | Semantic state                                                      |
| ------------------------------ | ------------------- | ------------------------------------------------------------------- |
| `--color-status-done`          | `--color-success`   | Goal completed successfully; quiz passed                            |
| `--color-status-interrupted`   | `--color-warn`      | Goal interrupted by user cancel / app close / network drop          |
| `--color-status-failed`        | `--color-error`     | Goal failed due to unrecoverable error; quiz failed                 |
| `--color-status-running`       | `--color-warn`      | Goal currently executing; same hue as interrupted by design intent  |

The design system SHALL NOT introduce a fourth hue for the `running` state. Visual differentiation between `running` and `interrupted` MUST come from motion (pulse ring) and an optional caret affordance, not color.

#### Scenario: Tailwind utility generation for status tokens

- **WHEN** a developer writes `<span className="bg-status-done">…</span>` in a `.tsx` file under `codebus-app/src/`
- **THEN** the rendered element has `background-color: var(--color-success)` applied via Tailwind v4 auto-generated utility, with no additional Tailwind configuration

#### Scenario: CSS variable inject sanity check

- **WHEN** a developer evaluates `getComputedStyle(document.documentElement).getPropertyValue('--color-status-done')` in the running app
- **THEN** the return value is the non-empty string `#4ade80` (matching `--color-success`)

#### Scenario: Running and interrupted share the same hue

- **WHEN** a developer reads the rendered values of `--color-status-running` and `--color-status-interrupted`
- **THEN** both resolve to `#f5a623` (the existing `--color-warn` value)


<!-- @trace
source: status-three-state-token-and-status-pill
updated: 2026-05-26
code:
  - codebus-app/src/i18n/messages.ts
  - codebus-app/src/components/workspace/QuizAnswering.tsx
  - codebus-app/src/components/workspace/QuizReview.tsx
  - codebus-app/src/components/workspace/RunDetailDone.tsx
  - codebus-app/src/components/workspace/RunListItem.tsx
  - codebus-app/src/components/ui/StatusPill.tsx
  - codebus-app/src/components/workspace/RunDetailRunning.tsx
  - codebus-app/src/styles/globals.css
  - codebus-app/src/components/workspace/RunDetailCancelled.tsx
  - codebus-app/src/vite-env.d.ts
  - codebus-app/src/styles/tokens.css
tests:
  - codebus-app/src/components/ui/StatusPill.test.tsx
  - codebus-app/src/components/workspace/RunListItem.test.tsx
-->

---
### Requirement: StatusPill component

The design system SHALL provide a `StatusPill` React component at `codebus-app/src/components/ui/StatusPill.tsx` with the following public API:

```tsx
export type StatusPillStatus = "done" | "interrupted" | "failed" | "running";
export type StatusPillVariant = "dot" | "pill";

export interface StatusPillProps {
  status: StatusPillStatus;
  variant: StatusPillVariant;
  caret?: React.ReactNode;
  className?: string;
}

export function StatusPill(props: StatusPillProps): JSX.Element;
```

The `dot` variant MUST render a 7px circular element using `bg-status-<status>` and MUST NOT render any text label or caret slot.

The `pill` variant MUST render a 7px dot followed by a localized text label in a single inline-flex container, with a 1px border and a tinted background derived from the status color. The label text MUST come from the i18n bundle key `workspace.status.<status>` (one of `done`, `interrupted`, `failed`, `running`).

When `status === "running"` and `variant === "pill"`, the dot element MUST carry an outer pulse ring via `box-shadow` (4px ring at rest, animated to 6px during pulse) and MUST render the `caret` slot to the right of the label if `caret` is provided.

When `variant === "dot"`, the component MUST NOT accept `status === "running"`. The component SHALL issue a `console.warn` in development builds when this combination is detected at runtime, but MUST NOT throw, to avoid breaking production rendering.

When `className` is provided, the component MUST merge the caller's classes with its own internal class names without overwriting them.

#### Scenario: Dot variant renders only a colored circle

- **WHEN** a developer renders `<StatusPill status="done" variant="dot" />`
- **THEN** the rendered DOM contains a single 7px circular element with `background-color: var(--color-status-done)` and no descendant text node

#### Scenario: Pill variant renders dot plus localized label

- **WHEN** a developer renders `<StatusPill status="failed" variant="pill" />` with the active locale set to zh-tw
- **THEN** the rendered DOM contains a 7px dot using `--color-status-failed`, followed by the text "失敗", inside a single inline-flex container with a 1px border derived from the failed color

#### Scenario: Running pill renders pulse ring and optional caret

- **WHEN** a developer renders `<StatusPill status="running" variant="pill" caret={<span className="mono">analyzing…|</span>} />`
- **THEN** the dot element carries a `box-shadow` outer ring using `--color-accent-tint`, the localized label "執行中" appears next to the dot, and the caret content appears immediately after the label

#### Scenario: Running plus dot variant triggers development warning

- **WHEN** a developer renders `<StatusPill status="running" variant="dot" />` in a development build
- **THEN** the component emits a `console.warn` describing the invalid combination, renders without throwing, and omits any pulse ring effect

#### Scenario: Custom className merges without overwriting

- **WHEN** a developer renders `<StatusPill status="done" variant="pill" className="ml-2" />`
- **THEN** the rendered element retains the internal status pill classes and additionally carries the `ml-2` utility


<!-- @trace
source: status-three-state-token-and-status-pill
updated: 2026-05-26
code:
  - codebus-app/src/i18n/messages.ts
  - codebus-app/src/components/workspace/QuizAnswering.tsx
  - codebus-app/src/components/workspace/QuizReview.tsx
  - codebus-app/src/components/workspace/RunDetailDone.tsx
  - codebus-app/src/components/workspace/RunListItem.tsx
  - codebus-app/src/components/ui/StatusPill.tsx
  - codebus-app/src/components/workspace/RunDetailRunning.tsx
  - codebus-app/src/styles/globals.css
  - codebus-app/src/components/workspace/RunDetailCancelled.tsx
  - codebus-app/src/vite-env.d.ts
  - codebus-app/src/styles/tokens.css
tests:
  - codebus-app/src/components/ui/StatusPill.test.tsx
  - codebus-app/src/components/workspace/RunListItem.test.tsx
-->

---
### Requirement: StatusPill pulse animation respects prefers-reduced-motion

The pulse animation used by `StatusPill` running state SHALL be gated behind `@media not (prefers-reduced-motion: reduce)` in `codebus-app/src/styles/globals.css`. The `@keyframes` definition and the animation declaration MUST sit inside the media query block. The static box-shadow ring (no animation) MUST remain outside the media query so that the ring color is preserved when motion is reduced.

#### Scenario: Reduced-motion user sees static ring

- **WHEN** the operating system reports `prefers-reduced-motion: reduce` and the app renders `<StatusPill status="running" variant="pill" />`
- **THEN** the dot element shows a static 4px box-shadow ring using `--color-accent-tint`, with no animated frame change over time

#### Scenario: Default-motion user sees animated ring

- **WHEN** the operating system reports `prefers-reduced-motion: no-preference` and the app renders `<StatusPill status="running" variant="pill" />`
- **THEN** the dot element pulses between a 4px and 6px box-shadow ring on a 1.4-second loop


<!-- @trace
source: status-three-state-token-and-status-pill
updated: 2026-05-26
code:
  - codebus-app/src/i18n/messages.ts
  - codebus-app/src/components/workspace/QuizAnswering.tsx
  - codebus-app/src/components/workspace/QuizReview.tsx
  - codebus-app/src/components/workspace/RunDetailDone.tsx
  - codebus-app/src/components/workspace/RunListItem.tsx
  - codebus-app/src/components/ui/StatusPill.tsx
  - codebus-app/src/components/workspace/RunDetailRunning.tsx
  - codebus-app/src/styles/globals.css
  - codebus-app/src/components/workspace/RunDetailCancelled.tsx
  - codebus-app/src/vite-env.d.ts
  - codebus-app/src/styles/tokens.css
tests:
  - codebus-app/src/components/ui/StatusPill.test.tsx
  - codebus-app/src/components/workspace/RunListItem.test.tsx
-->

---
### Requirement: Status visuals consume StatusPill instead of inline color literals

All `codebus-app/src/components/` consumers that present goal status, quiz pass/fail result, or any of the four canonical status states (done / interrupted / failed / running) SHALL render the status visual through `<StatusPill>`. Components MUST NOT use Tailwind palette utilities (`text-amber-*`, `bg-amber-*`, `text-green-*`, `bg-green-*`, `text-red-*`, `bg-red-*`, and similar) to express status semantics inline.

Components MAY still use the underlying palette utilities for non-status uses (for example accent decoration, illustrative tints, or design-handoff hero icons such as quiz completion). The Tailwind palette is not banned globally; it is banned as a substitute for the status semantic layer.

#### Scenario: Goals list row uses StatusPill dot

- **WHEN** a developer inspects a rendered Goals list row representing a completed goal
- **THEN** the row contains a `<StatusPill status="done" variant="dot" />` element and no inline `text-green-*` or `bg-green-*` Tailwind class on the status indicator

#### Scenario: Goal Detail header uses StatusPill pill

- **WHEN** a developer inspects the Goal Detail header for an interrupted goal
- **THEN** the header right side contains a `<StatusPill status="interrupted" variant="pill" />` element and no inline `text-amber-*` or `bg-amber-*` Tailwind class on the status indicator

#### Scenario: Quiz review fail tag uses StatusPill pill

- **WHEN** a developer inspects a rendered Quiz review screen for a failed attempt
- **THEN** the fail indicator is a `<StatusPill status="failed" variant="pill" />` element and no inline `text-red-*` or `bg-red-*` Tailwind class on the result tag

<!-- @trace
source: status-three-state-token-and-status-pill
updated: 2026-05-26
code:
  - codebus-app/src/i18n/messages.ts
  - codebus-app/src/components/workspace/QuizAnswering.tsx
  - codebus-app/src/components/workspace/QuizReview.tsx
  - codebus-app/src/components/workspace/RunDetailDone.tsx
  - codebus-app/src/components/workspace/RunListItem.tsx
  - codebus-app/src/components/ui/StatusPill.tsx
  - codebus-app/src/components/workspace/RunDetailRunning.tsx
  - codebus-app/src/styles/globals.css
  - codebus-app/src/components/workspace/RunDetailCancelled.tsx
  - codebus-app/src/vite-env.d.ts
  - codebus-app/src/styles/tokens.css
tests:
  - codebus-app/src/components/ui/StatusPill.test.tsx
  - codebus-app/src/components/workspace/RunListItem.test.tsx
-->