## ADDED Requirements

### Requirement: TabContentHeader component

The `codebus-app` design system SHALL expose a shared React component `TabContentHeader` at `codebus-app/src/components/ui/TabContentHeader.tsx`. This component SHALL be the single rendering site for the "content header row" pattern at the top of a Workspace tab's main content area (Goals tab, Quiz tab, and any future tab adopting the same pattern). Direct inline duplication of this pattern across multiple consumers SHALL NOT be permitted; consumers MUST consume this component.

The component SHALL accept the following props:

| Prop | Type | Required | Description |
| ---- | ---- | -------- | ----------- |
| `title` | `string` | yes | The h1 heading text. Caller MUST pass an already-translated string (e.g. the result of `t(key)`); the component MUST NOT call `useT()` or any i18n hook itself. |
| `subtitle` | `string` | no | Optional descriptive subtitle rendered below the h1. Caller MUST pass an already-translated string when present. |
| `cta` | `React.ReactNode` | no | Optional right-aligned call-to-action node (typically a `<Button>`). Caller controls click behavior. |
| `shortcutChipText` | `string` | no | Optional single- or short-character identifier (e.g. `"N"`, `"⌘K"`) rendered as a visual chip next to the CTA. This text is an identifier and MUST NOT be sourced from i18n. The chip SHALL render only when both `cta` and `shortcutChipText` are present. |
| `testId` | `string` | no | Optional `data-testid` value applied to the root row element so consumers can anchor unit and end-to-end tests. |

The component SHALL render a single horizontal row at the top of its consumer's main content area with the following invariants:

- The root row SHALL include the attribute `data-tauri-drag-region` so the row participates in the Tauri window-drag region (consistent with prior Workspace tab header behavior).
- The row SHALL reserve right-edge padding equivalent to the existing WindowControls allowance (`pr-[160px]`) so the CTA and shortcut chip do not collide with the platform window controls.
- The h1 SHALL use the `text-h-row` typography token (20px, per the Typography scale tokens requirement) and the primary foreground color token.
- The subtitle (when present) SHALL render below the h1 using a meta-scale typography token and a secondary foreground color token.
- The CTA and shortcut chip (when present) SHALL render on the right side of the row, vertically centered with the h1 group.
- The shortcut chip SHALL apply `aria-hidden="true"` since it is a visual reminder of an existing keyboard binding, not an interactive element.

The component SHALL NOT depend on any consumer-specific business logic, state stores, or IPC modules; its only inputs are its props.

#### Scenario: Component renders title-only

- **WHEN** a consumer renders `<TabContentHeader title="Goals" />`
- **THEN** the rendered output contains an h1 with text `Goals` AND no subtitle, CTA, or shortcut chip elements are present

#### Scenario: Component renders title plus subtitle

- **WHEN** a consumer renders `<TabContentHeader title="Goals" subtitle="List what you want to understand" />`
- **THEN** the rendered output contains an h1 with text `Goals` AND a subtitle element with text `List what you want to understand`

#### Scenario: Component renders CTA without shortcut chip

- **WHEN** a consumer renders `<TabContentHeader title="Quiz" cta={<button>+ New quiz</button>} />`
- **THEN** the rendered output contains the supplied CTA node on the right side AND no shortcut chip element is present

#### Scenario: Component renders CTA with shortcut chip

- **WHEN** a consumer renders `<TabContentHeader title="Goals" cta={<button>+ New goal</button>} shortcutChipText="N" />`
- **THEN** the rendered output contains the CTA node AND a shortcut chip element whose visible text is `N` AND the chip has `aria-hidden="true"`

#### Scenario: Shortcut chip is suppressed when CTA is absent

- **WHEN** a consumer renders `<TabContentHeader title="Goals" shortcutChipText="N" />` (no `cta` prop)
- **THEN** the rendered output SHALL NOT contain a shortcut chip element, because the chip is only meaningful adjacent to the CTA it annotates

#### Scenario: testId anchors the root row

- **WHEN** a consumer renders `<TabContentHeader title="Goals" testId="tab-content-header-goals" />`
- **THEN** the root row element carries the attribute `data-testid="tab-content-header-goals"`
