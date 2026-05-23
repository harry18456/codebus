## MODIFIED Requirements

### Requirement: Chat Assistant Message Markdown Rendering and Wiki Citation Links

The Chat Widget SHALL render each assistant message's text content through a Markdown renderer (`react-markdown`) rather than as plain text. The renderer SHALL be configured with the `remark-gfm` plugin so GitHub-flavored Markdown tables, strikethrough, AND task lists render as their corresponding HTML elements (`<table>` / `<del>` / task-list items) instead of leaking through as raw markdown syntax.

Before passing assistant text to `react-markdown`, the renderer SHALL pre-process the text by replacing every `[[slug]]` occurrence with a standard markdown link of the form `[<slug>](codebus://wiki/<percent-encoded-slug>)` (reusing the existing `transformBodyWikilinks` helper shared with the wiki preview surface). The renderer SHALL pass an `urlTransform` to `react-markdown` that returns each URL unchanged so the synthetic `codebus://wiki/...` scheme survives the renderer's default safelist (which would otherwise strip non-http(s)/mailto schemes).

The custom `a` element override SHALL classify each rendered link by `href` shape AND route the click accordingly:

- **Wikilink (codebus scheme)**: when `href` starts with `codebus://wiki/`, the renderer SHALL extract the slug by stripping that prefix AND percent-decoding the remainder. The renderer SHALL consult `useWikiStore.pages` (the client-side page index loaded at Workspace mount time) to classify the slug:
  - **Resolvable** (slug exists in `pages`): rendered as a `<button>`-like clickable element whose visible text is `pages[slug].title` when present, otherwise the raw slug. Clicking SHALL invoke `onWikiLinkClick(slug)` (passing the **decoded slug**, NOT the raw href) AND SHALL transition the Chat Widget to `collapsed` via `useChatStore.toggleExpanded()` (the existing collapse helper already short-circuits when already collapsed).
  - **Unresolvable** (slug missing from `pages`): rendered as a dimmed `<span>` with a `title` tooltip reading "Page not found". Clicking SHALL be a no-op (no `onWikiLinkClick` invocation, no widget transition).
- **Legacy wiki markdown link**: when `href` matches the regex `^wiki\/(.+)\.md$` (used by older agent outputs that embedded markdown links of the form `[label](wiki/<path>.md)`), the renderer SHALL extract the slug from the capture group (the path between `wiki/` AND the trailing `.md`) AND route through the SAME resolvable / unresolvable flow as the codebus-scheme branch. The capture group's value SHALL be the slug passed to `onWikiLinkClick`; the raw href SHALL NOT be passed.
- **External link**: when `href` starts with `http://` or `https://`, the renderer SHALL invoke the existing Tauri opener plugin with the URL. The Workspace active tab SHALL NOT change AND the Chat Widget SHALL remain in its current state.
- **Other**: any other `href` shape (for example source code paths like `src/auth/jwt.rs`) SHALL render as an inert `<span>` with no click handler AND no `<a>` tag carrying a non-empty href.

The `onWikiLinkClick` callback on `ChatTranscript` AND its descendants SHALL accept a **slug string**, NOT a raw href. Callers (notably `Workspace.onSelectPage(slug)`) SHALL receive the post-extraction slug regardless of whether the source markdown used `[[slug]]` syntax or the legacy `[label](wiki/<path>.md)` form. This contract change corrects a prior type-lie where the callback was documented AND typed as receiving an href but the only production consumer (`Workspace.onSelectPage`) treated the argument as a slug AND fed it to `useWikiStore.loadPage(vault, slug)` — leading to a `wiki/wiki/<path>.md.md` lookup miss if the chat had ever actually emitted a clickable wiki markdown link.

Plain-text mentions of wiki paths within an assistant message (for example `"see wiki/modules/auth.md"` without markdown link syntax AND without `[[...]]` syntax) SHALL NOT be auto-detected or made clickable; only markdown link syntax OR `[[slug]]` syntax SHALL produce clickable elements.

#### Scenario: GFM table renders as table element

- **WHEN** an assistant message contains the GFM markdown text below (column separators, header divider, two rows of data)

  ```
  | Tool | Replaces |
  |------|----------|
  | uv   | pip      |
  | ruff | flake8   |
  ```

- **THEN** the rendered DOM SHALL contain a `<table>` element with at least one `<th>` element bearing the text `Tool` AND at least one `<td>` element bearing the text `uv` AND SHALL NOT contain raw `|---|` text in the rendered prose

#### Scenario: Wikilink markdown syntax renders as clickable resolvable link

- **WHEN** an assistant message contains the plain text `[[modules/auth]]` AND `useWikiStore.pages["modules/auth"]` exists AND the user clicks the rendered link
- **THEN** the rendered link's visible text SHALL be `pages["modules/auth"].title` (falling back to `modules/auth` when the title is empty) AND the click SHALL invoke `onWikiLinkClick("modules/auth")` (the decoded slug, NOT a raw href) AND the Chat Widget SHALL transition to `collapsed`

#### Scenario: Wikilink to nonexistent page renders dimmed and is inert

- **WHEN** an assistant message contains `[[nonexistent-page]]` AND `useWikiStore.pages["nonexistent-page"]` does NOT exist AND the user clicks the rendered text
- **THEN** the rendered element SHALL be a `<span>` (NOT a `<button>` or `<a>` with click handler) AND its `title` attribute SHALL equal "Page not found" AND `onWikiLinkClick` SHALL NOT be invoked AND the Chat Widget SHALL NOT transition

#### Scenario: Legacy wiki markdown link click passes slug not href

- **WHEN** an assistant message contains the markdown text `[auth](wiki/modules/auth.md)` AND `useWikiStore.pages["modules/auth"]` exists AND the user clicks the rendered link
- **THEN** the Workspace active tab SHALL become `wiki` AND `onWikiLinkClick` SHALL be invoked with the slug `"modules/auth"` (the regex capture group between `wiki/` AND `.md`, NOT the raw href `"wiki/modules/auth.md"`) AND the Chat Widget SHALL transition to `collapsed`

#### Scenario: External https link opens in browser

- **WHEN** an assistant message contains `[docs](https://example.com/foo)` AND the user clicks the link
- **THEN** the Tauri opener plugin SHALL be invoked with the URL `https://example.com/foo` AND the Workspace active tab SHALL NOT change AND the Chat Widget SHALL remain in its current state

#### Scenario: Source code path renders as inert text

- **WHEN** an assistant message contains the markdown text `[jwt.rs](src/auth/jwt.rs)` AND the user clicks the rendered text
- **THEN** no navigation or IPC call SHALL occur AND the rendered element SHALL NOT have an `<a>` tag with a non-empty href OR equivalent click handler

#### Scenario: Plain text wiki mention without markdown or wikilink syntax is not clickable

- **WHEN** an assistant message contains the plain text `"see wiki/modules/auth.md for details"` (no markdown link syntax, no `[[...]]` wrapping)
- **THEN** the rendered text `"wiki/modules/auth.md"` SHALL NOT have a click handler attached AND SHALL render as inert prose
