## ADDED Requirements

### Requirement: Open Wiki Page In Obsidian

The system SHALL let the user open the currently-previewed wiki page in Obsidian directly from the codebus-app Wiki tab, leveraging the Obsidian vault registration that `codebus init` already performs (`codebus_core::vault::obsidian_register`). This requirement defines two Tauri IPC commands (a visibility probe and an open action) and the WikiPreview button that drives them. These commands are defined separately from the `Tauri IPC Commands for Goal Lifecycle and Wiki Read` requirement, following the same precedent by which the chat-turn lifecycle commands live in their own requirement.

#### IPC command: get_obsidian_vault_id

`get_obsidian_vault_id(vault_path: String) -> Result<Option<String>, AppError>` SHALL resolve the Obsidian vault id for the vault's wiki directory by calling `codebus_core::vault::obsidian_register::lookup_vault_id(<vault_path>/.codebus/wiki)`. The result mapping SHALL be:

- `Ok(Some(id))` from the core helper → `Ok(Some(id))` (the 16-char SHA-256 prefix Obsidian uses as the vault key).
- `Ok(None)` (no `obsidian.json`, Obsidian config dir absent, or no entry matches the wiki path) → `Ok(None)`.
- `Err(io_error)` (the `obsidian.json` exists but cannot be read or parsed) → `Err(AppError)` — a fail-soft signal the frontend treats identically to `None` (button hidden), never a hard crash.

#### IPC command: open_wiki_in_obsidian

`open_wiki_in_obsidian(vault_path: String, slug: String) -> Result<(), AppError>` SHALL perform the following steps in order:

1. Resolve the vault id via `lookup_vault_id`. When it resolves to `None`, the command SHALL return `AppError::Invalid { field: "obsidian", message: <vault-not-registered message> }` and SHALL NOT attempt to open anything.
2. Locate the wiki file whose filename stem (without `.md`) equals `slug`, by scanning `<vault_path>/.codebus/wiki/**/*.md`. When no file matches, the command SHALL return `AppError::Invalid { field: "slug", message: <no-such-page message> }`.
3. Compute the file path relative to `<vault_path>/.codebus/wiki/`, normalize path separators to forward slashes, and percent-encode each path segment.
4. Construct the URL `obsidian://open?vault=<id>&file=<rel>` where `<id>` is the resolved vault id and `<rel>` is the encoded relative path including the `.md` extension.
5. Open the URL via the tauri-plugin-opener Rust API. When the opener call fails, the command SHALL return `AppError`.

The relative-path + URL construction SHALL be a pure, separately-unit-testable function so the URL string can be asserted without spawning Obsidian. The command SHALL re-resolve the vault id on every invocation rather than accepting a caller-supplied id, so a vault that becomes unregistered while the app is open is detected at action time.

#### WikiPreview button

The Wiki preview footer action area (the same area that hosts `[Quiz me on this]`) SHALL render an `[Open in Obsidian]` button when, and only when, the wiki store's cached Obsidian vault id is non-null. The store SHALL fetch the vault id once via `get_obsidian_vault_id` when a vault's wiki is loaded and clear it on reset. The button SHALL render for both content pages and nav pages (`index.md` / `log.md`) — unlike `[Quiz me on this]` which renders only for content pages. Clicking the button SHALL invoke `open_wiki_in_obsidian(vault_path, current_slug)` exactly once with the currently-previewed page's slug.

When the cached vault id is null (vault not registered, or the probe returned an error), the button SHALL NOT be present in the DOM at all (hidden, not disabled).

#### Scenario: get_obsidian_vault_id returns Some for a registered vault

- **WHEN** the frontend calls `invoke("get_obsidian_vault_id", { vault_path })` AND the user's `obsidian.json` contains an entry whose path matches `<vault_path>/.codebus/wiki`
- **THEN** the command SHALL return `Ok(Some(<id>))` where `<id>` is the 16-char vault key

#### Scenario: get_obsidian_vault_id returns None when Obsidian not registered

- **WHEN** the frontend calls `invoke("get_obsidian_vault_id", { vault_path })` AND no `obsidian.json` exists OR no entry matches the wiki path
- **THEN** the command SHALL return `Ok(None)`

#### Scenario: get_obsidian_vault_id maps a parse failure to AppError (fail-soft)

- **WHEN** `obsidian.json` exists but cannot be parsed as JSON AND the frontend calls `invoke("get_obsidian_vault_id", { vault_path })`
- **THEN** the command SHALL return `Err(AppError)` AND the frontend SHALL treat this identically to `None` (the Open in Obsidian button SHALL NOT render)

#### Scenario: open_wiki_in_obsidian builds the id-based URL for a sub-folder page

- **WHEN** the frontend calls `invoke("open_wiki_in_obsidian", { vault_path, slug: "uv-lib" })` AND the page lives at `<vault_path>/.codebus/wiki/modules/uv-lib.md` AND the vault id resolves to `abc123def456abcd`
- **THEN** the command SHALL open the URL `obsidian://open?vault=abc123def456abcd&file=modules/uv-lib.md`

##### Example: relative path + encoding cases

| slug | abs wiki path (under `<vault>/.codebus/wiki/`) | `file=` value |
| --- | --- | --- |
| `uv-lib` | `modules/uv-lib.md` | `modules/uv-lib.md` |
| `project-purpose` | `concepts/project-purpose.md` | `concepts/project-purpose.md` |
| `index` | `index.md` | `index.md` |
| `授權流程` | `processes/授權流程.md` | `processes/%E6%8E%88%E6%AC%8A%E6%B5%81%E7%A8%8B.md` |

#### Scenario: open_wiki_in_obsidian rejects an unregistered vault

- **WHEN** the frontend calls `invoke("open_wiki_in_obsidian", { vault_path, slug })` AND `lookup_vault_id` resolves to `None`
- **THEN** the command SHALL return `AppError::Invalid { field: "obsidian", .. }` AND SHALL NOT attempt to open any URL

#### Scenario: open_wiki_in_obsidian rejects an unknown slug

- **WHEN** the frontend calls `invoke("open_wiki_in_obsidian", { vault_path, slug: "no-such-page" })` AND no wiki file has that filename stem
- **THEN** the command SHALL return `AppError::Invalid { field: "slug", .. }`

#### Scenario: Button renders for both content and nav pages when vault id is present

- **WHEN** the wiki store's cached Obsidian vault id is non-null AND the preview shows a content page OR a nav page (`index.md` / `log.md`)
- **THEN** the `[Open in Obsidian]` button SHALL be present in the footer action area in all of those cases (whereas `[Quiz me on this]` renders only on content pages)

#### Scenario: Button hidden when vault id is null

- **WHEN** the wiki store's cached Obsidian vault id is null
- **THEN** the `[Open in Obsidian]` button SHALL NOT be present in the DOM (hidden, not merely disabled)

#### Scenario: Clicking the button invokes the open command with the current slug

- **WHEN** the preview shows the page with slug `uv-lib` AND the vault id is non-null AND the user clicks `[Open in Obsidian]`
- **THEN** the frontend SHALL call `invoke("open_wiki_in_obsidian", { vault_path, slug: "uv-lib" })` exactly once
