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
