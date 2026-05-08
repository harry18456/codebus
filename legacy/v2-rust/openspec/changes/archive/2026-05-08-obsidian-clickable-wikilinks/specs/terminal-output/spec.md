## ADDED Requirements

### Requirement: Apply markdown styling to thought text when use_color is enabled

The system SHALL apply lightweight markdown styling to `thought` event text when `use_color` is true, covering three marker forms: `**bold**` SHALL render as ANSI bold, `` `inline code` `` SHALL render in cyan, and `[[wikilink]]` SHALL render in cyan with underline. When `use_color` is false, the text SHALL be emitted verbatim without any escape codes so byte-equal fixture comparisons and CI / `NO_COLOR` runs remain unaffected.

The styling SHALL apply to thought text only — `tool_use`, `tool_result`, and lifecycle banner text are unaffected by this requirement.

#### Scenario: Bold marker renders with ANSI bold escape

- **WHEN** the system renders a thought event whose text contains `**important**` and `use_color` is true
- **THEN** the output contains the ANSI bold escape sequence wrapping `important`, with the surrounding `**` removed

#### Scenario: Inline code renders cyan

- **WHEN** the system renders a thought event whose text contains `` `slug` `` and `use_color` is true
- **THEN** the output contains a cyan ANSI color escape wrapping `slug`, with the surrounding backticks removed

#### Scenario: Wikilink renders cyan with underline

- **WHEN** the system renders a thought event whose text contains `[[some-slug]]` and `use_color` is true
- **THEN** the output contains both cyan and underline ANSI escapes wrapping `[[some-slug]]` (the brackets are preserved as visible text)

#### Scenario: use_color false produces no styling

- **WHEN** the system renders a thought event whose text contains `**bold**`, `` `code` ``, and `[[wikilink]]` and `use_color` is false
- **THEN** the output contains the raw text byte-for-byte with no ANSI escape codes

#### Scenario: Tool events are not styled

- **WHEN** the system renders a `tool_use` event whose `input` JSON contains `**` or backticks
- **THEN** the rendered tool args are emitted verbatim without markdown-style transformations

### Requirement: Wrap wikilinks with OSC 8 hyperlinks when terminal supports them

The system SHALL wrap `[[<slug>]]` markers in `thought` text with an OSC 8 hyperlink escape sequence when both conditions hold: (a) `use_color` is true, and (b) terminal hyperlink support is detected via the `supports-hyperlinks` crate. The hyperlink target URI SHALL be `obsidian://open?vault=<vault-id>&file=<type>/<slug>` where `<vault-id>` is the resolved effective vault id (from `RenderOptions::vault_id`) and `<type>/<slug>` is resolved from the slug index. When the slug is not present in the index, the system SHALL render the wikilink with markdown styling but without OSC 8 wrapping.

When terminal hyperlink support is not detected, the system SHALL render the wikilink with markdown styling only (no OSC 8 escape) so unsupported terminals do not display garbage characters.

The OSC 8 sequence SHALL use the form `\x1b]8;;<URI>\x1b\\<text>\x1b]8;;\x1b\\` (ESC-backslash terminator), with `<text>` carrying the styled `[[slug]]` payload.

#### Scenario: Supported terminal with resolvable slug emits OSC 8 hyperlink

- **WHEN** `use_color` is true, hyperlinks are supported, `vault_id` is `a38bcac8afd70c5e`, slug index resolves `buddy-cli-commands` to `(Concept, concepts/buddy-cli-commands)`, and the system renders thought text containing `[[buddy-cli-commands]]`
- **THEN** the output contains an OSC 8 hyperlink whose URI is exactly `obsidian://open?vault=a38bcac8afd70c5e&file=concepts/buddy-cli-commands` wrapping the styled `[[buddy-cli-commands]]` text

#### Scenario: Unsupported terminal renders styling only

- **WHEN** `use_color` is true, hyperlinks are not supported (e.g., dumb terminal), and the system renders text containing `[[buddy-cli-commands]]`
- **THEN** the output contains cyan + underline ANSI escapes around `[[buddy-cli-commands]]` but no OSC 8 escape sequence

#### Scenario: Slug not in index falls back to styling only

- **WHEN** `use_color` is true, hyperlinks are supported, but the slug index does not contain `unknown-slug`
- **THEN** the system renders `[[unknown-slug]]` with markdown styling but no OSC 8 wrapping

#### Scenario: use_color false suppresses both styling and hyperlink

- **WHEN** `use_color` is false and the system renders text containing `[[buddy-cli-commands]]`
- **THEN** the output contains the raw `[[buddy-cli-commands]]` bytes with no ANSI or OSC escape codes

### Requirement: RenderOptions carries vault context for hyperlink emission

The system SHALL extend `RenderOptions` (the renderer-specific options struct on `TerminalRenderer`) with three new optional fields used to construct OSC 8 hyperlinks:

- `vault_id: Option<String>` — the effective Obsidian vault id used in the OSC 8 URI's `vault=` parameter. `None` SHALL disable hyperlink emission entirely (treat as if hyperlink support were not detected).
- `slug_index: Option<Arc<SlugIndex>>` — a slug-to-(PageType, relative-path) mapping built once per run. `None` SHALL disable hyperlink emission.
- `hyperlinks: bool` — explicit override that, when false, SHALL disable hyperlink emission regardless of detection. Default true. Allows test fixtures and `--no-hyperlinks` opt-out.

These fields SHALL be populated by the goal / query / fix flow at run start (after vault registration and slug index build) and threaded into the renderer instance.

#### Scenario: vault_id None disables hyperlink even when supported

- **WHEN** `use_color` is true, hyperlinks are supported, `slug_index` is populated, but `vault_id` is `None`
- **THEN** wikilink rendering produces markdown styling only, no OSC 8 escape

#### Scenario: hyperlinks false overrides terminal detection

- **WHEN** `use_color` is true, terminal supports hyperlinks, `vault_id` and `slug_index` are populated, but `hyperlinks` is false
- **THEN** wikilink rendering produces markdown styling only, no OSC 8 escape
