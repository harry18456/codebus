## MODIFIED Requirements

### Requirement: Lobby Two-State Rendering

The Lobby SHALL render in exactly one of two states determined by `vault_list` length.

The populated state SHALL display vault cards with `display_name`, `path`, and human-readable relative `last_opened` (absolute date after 30 days), plus a top-right primary action button whose label SHALL NOT contain the literal word "Vault" or "vault" in any rendered locale. The populated state SHALL render a section label above the card list using the shared `SectionLabel` component (default variant, no uppercase tracking), and the label text SHALL NOT contain the literal word "Vault" or "vault" in any rendered locale. The populated state SHALL render a drag-tip caption below the card list whose text SHALL NOT contain the literal word "Vault" or "vault" in any rendered locale.

Each vault card SHALL expose a visible kebab (`⋮`) button on hover and on keyboard focus that opens the per-vault action menu (Reveal in files / Remove); right-click (context menu) on the card SHALL continue to open the same menu as a shortcut. The kebab button SHALL be hidden (zero opacity) when neither hovered nor focused so it does not add static visual noise.

The empty state SHALL display a hero with a large 🚌 emoji, a title, a subtitle, a primary `+ Board a new bus` CTA (or its localized equivalent), and a Quickstart 3-step orientation card. The Quickstart card SHALL render each step number as a monospace digit without a trailing period, in `text-fg-tertiary` color. The Quickstart step 2 SHALL render its example fragment inside an amber-tinted monospace pill (background `accent-tint`, foreground `accent`, 1px amber-tinted border, `rounded-sm`, monospace font); the example fragment SHALL be sourced from a dedicated i18n key separate from the step prefix so that pill styling and step wording can evolve independently.

The Lobby `<main>` content SHALL flow from the top using a vertical flex column; it SHALL NOT vertically center its content within the viewport in either state. The bottom strip (Settings gear left, version label right) SHALL remain a sibling of the Lobby `<main>` rendered at the application shell level, naturally occupying the bottom of the viewport.

#### Scenario: Empty list renders empty state

- **WHEN** the Lobby loads and `vault_list` is empty
- **THEN** the empty-state hero (🚌 emoji, title, subtitle, Board-a-new-bus CTA, Quickstart card) is rendered and no vault cards are shown

#### Scenario: Non-empty list renders cards

- **WHEN** the Lobby loads and `vault_list` contains one or more entries
- **THEN** vault cards are rendered in reverse-chronological order by `last_opened`, the top-right primary add-action button is shown, and a non-uppercase-tracked section label is rendered above the card list

#### Scenario: Populated state UI text contains no "Vault" literal

- **WHEN** the Lobby loads in populated state in any supported locale (zh, en)
- **THEN** the topbar add-action button label, the section label above the card list, and the drag-tip caption below the card list each contain no occurrence of the literal string "Vault" or "vault"

##### Example: locale string audit

| Element             | zh literal forbidden | en literal forbidden |
| ------------------- | -------------------- | -------------------- |
| Topbar add-action   | "Vault" / "vault"    | "Vault" / "vault"    |
| Populated section   | "Vault" / "vault"    | "Vault" / "vault"    |
| Drag-tip caption    | "Vault" / "vault"    | "Vault" / "vault"    |

#### Scenario: Vault card kebab visible on hover and focus

- **WHEN** a user moves the pointer over a vault card or focuses the card via keyboard navigation
- **THEN** a visible kebab (`⋮`) button appears at the card's right edge, and activating it opens the action menu anchored to the button

#### Scenario: Vault card kebab hidden when idle

- **WHEN** a vault card is neither pointer-hovered nor keyboard-focused
- **THEN** the kebab button is rendered at zero opacity so it does not contribute static visual noise to the list

#### Scenario: Vault card right-click still opens menu

- **WHEN** a user right-clicks (context menu) anywhere on a vault card
- **THEN** the same action menu opens, positioned at the cursor

#### Scenario: Quickstart step number uses monospace digits without period

- **WHEN** the Lobby renders the empty state Quickstart card
- **THEN** each step is prefixed by a monospace digit (1, 2, 3) with no trailing period and rendered in `text-fg-tertiary` color

#### Scenario: Quickstart step 2 example renders in amber pill

- **WHEN** the Lobby renders the empty state Quickstart card
- **THEN** the example fragment of step 2 is wrapped in an inline element styled as an amber-tinted monospace pill (background `accent-tint`, foreground `accent`, 1px amber-tinted border, `rounded-sm` corners) distinct from the surrounding step text

#### Scenario: Lobby content flows from the top

- **WHEN** the Lobby is rendered in either empty or populated state at common desktop viewports (e.g., 1920×1080 at 100% scaling)
- **THEN** the Lobby `<main>` content (hero / cards) is aligned to the top of the available area, not vertically centered, and the application-shell bottom strip occupies the bottom of the viewport naturally

#### Scenario: Section labels use the shared SectionLabel component

- **WHEN** the Lobby renders the populated state's recent-cards label or the empty state's Quickstart label
- **THEN** each label is rendered by the shared `SectionLabel` component in its default (non-uppercase-tracked) variant, so the visual treatment is identical between Latin and CJK label text

## ADDED Requirements

### Requirement: Lobby Empty State Idle Motion

The empty-state hero 🚌 emoji SHALL render a subtle idle micro-motion: a vertical translation of approximately 2 pixels and a horizontal translation of approximately 1 pixel, looping continuously with the vertical and horizontal axes desynchronized (different loop durations) and without any rotation or opacity change. The motion SHALL be implemented as pure CSS keyframes; no JavaScript animation library SHALL be introduced for this effect.

When the user agent advertises `prefers-reduced-motion: reduce`, the idle motion SHALL be suppressed and the emoji SHALL remain completely static; no transform animation SHALL be applied.

The idle motion SHALL be confined to the empty-state hero only; the topbar 🚌 wordmark glyph SHALL remain static, and no other Lobby element SHALL animate as a consequence of this requirement.

#### Scenario: Hero 🚌 animates with idle micro-motion by default

- **WHEN** a user opens the Lobby in empty state on a system that does not advertise `prefers-reduced-motion: reduce`
- **THEN** the hero 🚌 emoji visibly translates within a 2px vertical / 1px horizontal range on a continuous loop, with the two axes desynchronized, and without any rotation or opacity change

#### Scenario: Reduced-motion preference disables idle motion

- **WHEN** the user agent advertises `prefers-reduced-motion: reduce`
- **THEN** the hero 🚌 emoji renders with no transform animation applied; computed `animation-name` is `none` (or equivalent)

#### Scenario: Idle motion is scoped to empty-state hero

- **WHEN** the idle motion is active in the empty state
- **THEN** the topbar 🚌 wordmark glyph and every other Lobby element render without any motion attributable to this requirement
