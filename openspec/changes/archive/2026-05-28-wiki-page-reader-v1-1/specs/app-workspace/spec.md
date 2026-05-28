## ADDED Requirements

### Requirement: Wiki Page Metadata Bar

The Wiki page reader SHALL render a single-line metadata bar at the top of the preview (immediately above the markdown body) on every wiki page view. The bar SHALL be composed of up to three segments rendered in this order, separated by a middle-dot `·`:

1. **Last authoring goal** — Text `Last updated by <goal>` where `<goal>` is the last element of `PageFrontmatter.goals`. When `goals` is empty, this segment SHALL NOT be rendered. The `<goal>` value SHALL be rendered as a clickable element that, when activated, navigates the user to the Goal Detail view for that goal.
2. **Time since update** — A relative time-ago string derived from `PageFrontmatter.updated`. The time-ago value SHALL reuse the existing `common.minutesAgo` / `common.hoursAgo` / `common.daysAgo` localized strings. When `PageFrontmatter.updated` cannot be parsed into a valid date, this segment SHALL NOT be rendered.
3. **Wikilink count** — Text `<N> sources` (or localized equivalent) where `<N>` is the count of `[[wikilink]]` occurrences in the page body, computed via the same wikilink-extraction routine the renderer uses. When `<N>` is less than 1, this segment SHALL NOT be rendered.

The metadata bar SHALL NOT render tags, word counts, view counts, author lists, or any other field beyond the three segments specified above. When all three segments are suppressed, the metadata bar component SHALL render nothing (no empty bar).

#### Scenario: All three metadata segments render

- **WHEN** a wiki page with `goals: [g1, g2]`, valid `updated: 2026-05-27T12:00:00Z`, and a body containing three `[[wikilink]]` references is rendered
- **THEN** the metadata bar renders three segments separated by `·`: `Last updated by g2`, a localized time-ago string, and `3 sources`

#### Scenario: Authoring goal name is clickable

- **WHEN** the user clicks the `<goal>` token in the metadata bar
- **THEN** the workspace navigates to the Goal Detail view for that goal AND no IPC call is required to compute the click target

#### Scenario: Empty goals list suppresses first segment

- **WHEN** a wiki page with `goals: []` is rendered AND the body has two wikilinks AND `updated` is valid
- **THEN** the metadata bar renders only `<time-ago> · 2 sources` AND the `Last updated by` segment is omitted

#### Scenario: Zero wikilinks suppresses sources segment

- **WHEN** a wiki page body contains no `[[wikilink]]` references
- **THEN** the metadata bar omits the `<N> sources` segment entirely (including the leading `·` separator)

#### Scenario: All segments suppressed renders nothing

- **WHEN** a wiki page has `goals: []` AND `updated` is unparseable AND the body has no wikilinks
- **THEN** the metadata bar component renders no DOM output

##### Example: segment suppression matrix

| goals          | updated parses | wikilink count | Rendered segments                                  |
| -------------- | -------------- | -------------- | -------------------------------------------------- |
| `[g1, g2]`     | yes            | 3              | `Last updated by g2 · <time-ago> · 3 sources`      |
| `[]`           | yes            | 2              | `<time-ago> · 2 sources`                           |
| `[g1]`         | no             | 1              | `Last updated by g1 · 1 sources`                   |
| `[]`           | no             | 0              | (no DOM output)                                    |
| `[g1]`         | yes            | 0              | `Last updated by g1 · <time-ago>`                  |

### Requirement: Wiki Page Edit Hint Footer

The Wiki page reader SHALL render an edit-hint footer at the bottom of every wiki page view (rendered after the markdown body and after the existing bottom action button row). The hint SHALL be a single line of text styled in a tertiary foreground color and SHALL contain a clickable element labelled `Run a goal` (or its localized equivalent). The full hint text SHALL communicate that the user can edit the page by starting a new goal that asks codebus to change it.

Activating the clickable element SHALL open the existing New Goal Modal with a pre-filled goal description of the form `修改 wiki/<page-relative-path> — ` (Chinese form) or `Edit wiki/<page-relative-path> — ` (English form), where `<page-relative-path>` is the wiki page's path relative to the vault `.codebus/wiki/` directory including the type-folder prefix and the `.md` extension. The pre-fill SHALL end with an em-dash and a trailing space so the user can append their own description.

The edit hint footer SHALL NOT be rendered when no wiki page is currently selected.

#### Scenario: Edit hint footer renders for content pages

- **WHEN** the user opens a wiki content page (any page that is not `index.md` or `log.md`)
- **THEN** the edit hint footer is rendered below the markdown body AND below the existing action button row

#### Scenario: Activating Run a goal opens prefilled modal

- **WHEN** the user clicks the `Run a goal` element in the edit hint footer for a page whose relative path is `modules/auth-middleware.md`
- **THEN** the existing New Goal Modal opens AND its goal description is pre-filled with `修改 wiki/modules/auth-middleware.md — ` (in Chinese locale) or the English equivalent prefix, including the trailing em-dash and space

### Requirement: Wiki Page Reader Quiz Button Visual Emphasis

The `Quiz me on this` button rendered at the bottom of a wiki content page SHALL use the amber primary button variant. The `Open in Obsidian` button (when present) SHALL retain the secondary button variant. The localized label of the Quiz button SHALL match the standardized wording for the action (`Quiz this page` in English, `Quiz 這頁` in Traditional Chinese; the `Quiz` jargon term SHALL be preserved verbatim in the Chinese label).

#### Scenario: Quiz button uses amber primary variant

- **WHEN** a wiki content page renders its bottom action row with both buttons present
- **THEN** the `Quiz me on this` button uses the amber primary variant AND the `Open in Obsidian` button uses the secondary variant

#### Scenario: Chinese label preserves Quiz jargon

- **WHEN** the locale is `zh-tw`
- **THEN** the Quiz button label is `Quiz 這頁` AND the substring `Quiz` is rendered verbatim without translation

### Requirement: Wikilink Plain and Citation Style Variants

The wikilink renderer SHALL apply two distinct visual variants depending on rendering context:

- **Plain wikilink variant** — Used for resolvable wikilinks inside a wiki body. The element SHALL carry the literal CSS class name `plain-wikilink` and SHALL render with the default foreground color, an underline whose decoration color matches the strong-border token, and a 3px underline offset. On hover the element SHALL transition the text color and underline color to the accent token. Under `prefers-reduced-motion: reduce`, the hover color change SHALL be applied instantly without a CSS transition.
- **Citation wikilink variant** — Used inside citation blocks (such as quiz citation blockquotes and chat bubble citations). The element SHALL carry the literal CSS class name `cite-link` and SHALL render in a monospace font with the accent foreground color and a dashed underline at a 3px offset.

Unresolvable wikilinks (slugs that are not present in the wiki page index) SHALL continue to render as a dimmed `cursor-not-allowed` element with the `Page not found` tooltip; no new state SHALL be introduced beyond the existing resolvable / unresolvable distinction. The wikilink renderer SHALL NOT track or visualize a `visited` state.

#### Scenario: Resolvable body wikilink uses plain-wikilink class

- **WHEN** a resolvable `[[slug]]` is rendered inside a wiki body
- **THEN** the rendered anchor element has the literal CSS class `plain-wikilink` in its class list AND the anchor uses the foreground / strong-border / accent token colors specified above

#### Scenario: Citation wikilink uses cite-link class

- **WHEN** a wikilink is rendered inside a quiz citation block or a chat-bubble citation block
- **THEN** the rendered anchor element has the literal CSS class `cite-link` in its class list AND uses the monospace / accent / dashed-underline styling

#### Scenario: Reduced motion suppresses hover transition

- **WHEN** the user agent advertises `prefers-reduced-motion: reduce`
- **THEN** hovering a `plain-wikilink` element changes the color and underline color instantly without a CSS transition

#### Scenario: No visited state is rendered

- **WHEN** the user navigates to a wikilink target and later returns to a page that links to that target
- **THEN** the wikilink renders with the same `plain-wikilink` (or `cite-link`) styling as a never-clicked link AND no visited-state styling is applied

### Requirement: Wiki Tree Travel Log Footer Slot

The Wiki Tree component SHALL render a footer slot immediately below the last bucket. The footer slot SHALL contain exactly one entry labelled `Travel log` (or its localized equivalent) representing the system-generated `log.md` page. The footer slot entry SHALL be visually separated from the buckets above by a hairline top border and an 18px gap above the border. The entry SHALL render in a tertiary foreground color to distinguish it from the bucket entries.

Activating the footer slot entry SHALL invoke the same page-selection callback that bucket entries use, with the slug `log`.

When the Wiki Tree is rendered, the Wiki Index entry (slug `index`) SHALL appear at the top of the tree as the first entry, above any bucket. The previous catch-all `OTHER` bucket SHALL no longer be rendered as a bucket; the `log.md` system page that previously appeared inside `OTHER` SHALL appear only in the footer slot. Pages whose `PageFrontmatter.page_type` does not match any of the five known buckets SHALL still be reachable through the page index but SHALL NOT cause an `OTHER` bucket header to appear.

#### Scenario: Travel log footer renders below buckets

- **WHEN** the Wiki Tree mounts with at least one wiki page in the vault
- **THEN** the bottom of the tree renders an entry labelled `Travel log` (or the active locale equivalent) AND the entry is separated from the bucket list above by a hairline top border with an 18px gap

#### Scenario: Travel log entry selects log page

- **WHEN** the user clicks the `Travel log` entry in the footer slot
- **THEN** the page-selection callback fires with slug `log` AND the Wiki page reader loads the `log.md` system page

#### Scenario: Wiki Index appears at the top of the tree

- **WHEN** the Wiki Tree renders
- **THEN** the first entry in the tree is the Wiki Index (slug `index`) AND it appears above any bucket entry

#### Scenario: OTHER bucket is no longer rendered

- **WHEN** the Wiki Tree renders with pages of all five known `page_type` values
- **THEN** the tree renders the five known bucket headers AND no bucket header labelled `OTHER` is rendered

### Requirement: Wiki Page Reader Unselected Hint Card

When the Wiki tab is mounted with at least one wiki page in the vault but no page is currently selected, the Wiki page reader region SHALL render a hint card containing exactly three elements: a 36px folder emoji icon (`📂`) in a quaternary foreground color, a primary line of localized text inviting the user to pick a page, and a secondary line of localized text directing the user to the Travel log entry in the tree footer. The hint card SHALL NOT render the metadata bar, the markdown body, the action button row, or the edit hint footer.

#### Scenario: Unselected hint card renders when no page is selected

- **WHEN** the Wiki tab is mounted AND the vault contains at least one wiki page AND no page is currently selected
- **THEN** the Wiki page reader region renders a hint card with the `📂` icon, the `Pick a page to start reading.` primary line (or active locale equivalent), and the `Or click the travel log below to see what codebus has been up to.` secondary line (or active locale equivalent)

#### Scenario: Hint card does not render reader chrome

- **WHEN** the unselected hint card is showing
- **THEN** no metadata bar is rendered AND no markdown body is rendered AND no edit hint footer is rendered AND no action button row is rendered

### Requirement: Wiki Tab Empty Hero When Vault Has No Pages

When the Wiki tab is mounted and the vault contains zero wiki pages, the Wiki tab content area SHALL render a single empty hero. The empty hero SHALL contain exactly four elements stacked vertically and centered: a 56px lucide `Folder` icon, a localized hero title (`No wiki pages yet` in English or its locale equivalent), a localized subtitle inviting the user to run a goal so codebus can build the wiki, and a single amber primary call-to-action button (`→ Run a goal to start` in English or its locale equivalent). Activating the call-to-action button SHALL switch the active Workspace tab to the Goals tab AND SHALL open the existing New Goal Modal (without pre-filling the goal description).

The empty hero SHALL replace the previous single-line empty hint entirely; the previous `workspace.wiki.empty` localized hint SHALL NOT be rendered alongside the hero.

#### Scenario: Empty hero renders when vault has no wiki pages

- **WHEN** the Wiki tab mounts AND the vault contains zero wiki pages
- **THEN** the Wiki tab content area renders the 56px Folder icon AND the localized hero title AND the localized subtitle AND the amber primary call-to-action button AND no single-line hint is rendered alongside

#### Scenario: CTA switches to Goals tab and opens New Goal Modal

- **WHEN** the user clicks the empty hero call-to-action button
- **THEN** the Workspace active tab switches to `Goals` AND the existing New Goal Modal opens AND the modal's goal description field is empty (no pre-fill)
