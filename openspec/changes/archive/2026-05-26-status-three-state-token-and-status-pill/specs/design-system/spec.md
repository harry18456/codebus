## ADDED Requirements

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

---

### Requirement: StatusPill pulse animation respects prefers-reduced-motion

The pulse animation used by `StatusPill` running state SHALL be gated behind `@media not (prefers-reduced-motion: reduce)` in `codebus-app/src/styles/globals.css`. The `@keyframes` definition and the animation declaration MUST sit inside the media query block. The static box-shadow ring (no animation) MUST remain outside the media query so that the ring color is preserved when motion is reduced.

#### Scenario: Reduced-motion user sees static ring

- **WHEN** the operating system reports `prefers-reduced-motion: reduce` and the app renders `<StatusPill status="running" variant="pill" />`
- **THEN** the dot element shows a static 4px box-shadow ring using `--color-accent-tint`, with no animated frame change over time

#### Scenario: Default-motion user sees animated ring

- **WHEN** the operating system reports `prefers-reduced-motion: no-preference` and the app renders `<StatusPill status="running" variant="pill" />`
- **THEN** the dot element pulses between a 4px and 6px box-shadow ring on a 1.4-second loop

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
