## RENAMED Requirements

- FROM: `### Requirement: Single-vault stdio MCP server lifecycle`
- TO: `### Requirement: Stdio MCP server lifecycle and startup modes`

- FROM: `### Requirement: Tools-only query surface without path parameters`
- TO: `### Requirement: Tools-only query surface with registry-scoped vault selection`

## MODIFIED Requirements

### Requirement: Single-vault stdio MCP server lifecycle

The system SHALL provide a `codebus mcp` subcommand that starts a Model Context Protocol server over stdio transport in one of two startup modes determined by the presence of `--vault`. When invoked WITHOUT `--vault`, the server SHALL start in **registry mode**, resolving vaults from the codebus app-state registry (`~/.codebus/app-state.json`) so a single process can serve any registered vault. When invoked WITH `--vault <path>`, the server SHALL start in **pinned mode**, bound to exactly that one vault (compatible with the v1 single-vault behavior). In both modes the server SHALL advertise the `tools` capability and SHALL NOT advertise resources or prompts. The server SHALL keep serving until stdin closes or the process is terminated.

#### Scenario: Registry mode starts without --vault

- **WHEN** `codebus mcp` is invoked without `--vault`
- **THEN** the process SHALL start an MCP server in registry mode that exchanges JSON-RPC over stdin/stdout, advertises only the `tools` capability, and SHALL keep serving until stdin closes or the process is terminated, regardless of how many vaults are currently registered (including zero)

#### Scenario: Pinned mode starts for a valid vault

- **WHEN** `codebus mcp --vault <path>` is invoked AND `<path>/.codebus/wiki/` exists and is a directory
- **THEN** the process SHALL start an MCP server in pinned mode bound to that one vault, advertising only the `tools` capability, with the same query semantics as the v1 single-vault behavior

#### Scenario: Pinned mode missing wiki directory aborts startup

- **WHEN** `codebus mcp --vault <path>` is invoked AND `<path>/.codebus/wiki/` does not exist or is not a directory
- **THEN** the process SHALL exit with a non-zero status and write an explanatory message to stderr, and SHALL NOT start the server

#### Scenario: Diagnostics never pollute the protocol channel

- **WHEN** the server emits any log, warning, or diagnostic output in either mode
- **THEN** it SHALL write that output to stderr only, leaving stdout exclusively for JSON-RPC frames

### Requirement: Tools-only query surface without path parameters

The server SHALL expose exactly four query-only tools — `vault_list`, `wiki_list`, `wiki_read`, and `wiki_search`. No tool SHALL mutate the vault and no write tool SHALL be exposed. The three wiki tools (`wiki_list` / `wiki_read` / `wiki_search`) SHALL accept an OPTIONAL `vault` argument that selects which registered vault to query; the `vault` value is a registry-member identifier — the normalized absolute path returned by `vault_list` (or carried on a `wiki_list` / `wiki_search` result) — and SHALL NOT be treated as an arbitrary filesystem path. A `vault` outside the registry SHALL be rejected per the `Vault selection across startup modes` requirement. `vault_list` SHALL NOT accept any argument. The per-tool query behavior — page index, character pagination, keyword search — is defined by the `wiki_list returns the page index`, `wiki_read returns the paginated page body`, and `wiki_search performs keyword substring search` requirements; in registry mode `wiki_list` and `wiki_search` aggregate across present vaults when `vault` is omitted and tag each result with its source vault, while `wiki_read` requires an unambiguous vault, all per the `Vault selection across startup modes` requirement.

#### Scenario: tools/list enumerates the four query tools

- **WHEN** a client sends a `tools/list` request
- **THEN** the response SHALL contain `vault_list`, `wiki_list`, `wiki_read`, and `wiki_search`, each with an auto-generated input schema, and SHALL NOT contain any write tool

#### Scenario: Wiki tools expose an optional vault selector

- **WHEN** a client inspects the input schema of `wiki_list`, `wiki_read`, and `wiki_search`
- **THEN** each schema SHALL contain an OPTIONAL `vault` string property AND the `vault_list` schema SHALL contain no `vault` or filesystem-path property

#### Scenario: No tool mutates the vault

- **WHEN** a client enumerates and invokes any exposed tool
- **THEN** no tool SHALL create, modify, or delete any file under any vault

### Requirement: wiki_list returns the page index

`wiki_list(vault?)` SHALL return the slug and title of every Markdown page under the resolved vault's `<vault>/.codebus/wiki/` tree. The `vault` argument is OPTIONAL and is resolved per the `Vault selection across startup modes` requirement; when omitted in registry mode with more than one present vault, `wiki_list` SHALL aggregate pages across ALL present registered vaults. In registry mode each returned entry SHALL additionally carry its source `vault` (the normalized absolute path) and `name` (the display name) alongside `slug` and `title`, so a caller can pass the correct `vault` to `wiki_read`. It SHALL tolerate pages whose frontmatter is missing or malformed by using the filename stem as both slug and title fallback.

#### Scenario: Lists pages including those without frontmatter

- **WHEN** a client calls `wiki_list` against a single resolved vault whose wiki tree contains pages with and without YAML frontmatter
- **THEN** the result SHALL contain one entry per `.md` file, where slug is the filename stem and title is the frontmatter `title` when present, otherwise the slug

#### Scenario: Empty vault returns an empty list

- **WHEN** a client calls `wiki_list` AND the resolved wiki tree contains no `.md` files
- **THEN** the result SHALL be an empty list returned as success, not an error

#### Scenario: Aggregates pages across all present vaults on omission

- **WHEN** a client calls `wiki_list` in registry mode with no `vault` argument AND two or more present vaults are registered
- **THEN** the result SHALL contain pages from every present vault, and each entry SHALL carry its source `vault` and `name`

### Requirement: wiki_read returns the paginated page body

`wiki_read(vault?, slug, offset, limit)` SHALL return the page body with the leading frontmatter block stripped, paginated by Unicode character (`char`) count over the stripped body. `offset` SHALL default to 0; `limit` SHALL default to 12000 and SHALL be clamped to a maximum of 20000. Slicing SHALL occur on character boundaries so multi-byte UTF-8 / CJK characters are never split. The result SHALL include the returned `content`, the `offset` used, a `next_offset` (null when fully read), a `has_more` boolean, and `total_chars`. Because the same slug can occur in more than one vault, `wiki_read` SHALL locate a single page unambiguously: the `vault` argument is resolved per the `Vault selection across startup modes` requirement, EXCEPT that in registry mode with more than one present vault and `vault` omitted, `wiki_read` SHALL return an MCP error instructing the caller to specify a vault and SHALL NOT aggregate. The normal caller path supplies the `vault` carried on a prior `wiki_list` / `wiki_search` result.

#### Scenario: Reads a page and strips frontmatter

- **WHEN** a client calls `wiki_read` with a resolved vault and a slug that resolves to a page beginning with a `---` frontmatter block
- **THEN** the returned `content` SHALL start with the body after the closing frontmatter delimiter, never with the frontmatter

#### Scenario: Paginates a body larger than the limit

- **WHEN** a client calls `wiki_read` with an offset and limit against a body longer than `offset + limit` characters
- **THEN** the returned `content` SHALL contain exactly `limit` characters starting at `offset`, `has_more` SHALL be true, and `next_offset` SHALL equal `offset + limit`

##### Example: character pagination boundaries

| total_chars | offset | limit | returned content len | has_more | next_offset |
| ----------- | ------ | ----- | -------------------- | -------- | ----------- |
| 30000       | 0      | 12000 | 12000                | true     | 12000       |
| 30000       | 12000  | 12000 | 12000                | true     | 24000       |
| 30000       | 24000  | 12000 | 6000                 | false    | null        |
| 30000       | 0      | 99999 | 20000 (clamped)      | true     | 20000       |
| 500         | 0      | 12000 | 500                  | false    | null        |

#### Scenario: Unknown slug is an error, not empty content

- **WHEN** a client calls `wiki_read` with a slug that matches no page in the resolved vault
- **THEN** the tool SHALL return an MCP error result, not a success with empty content

#### Scenario: Read requires an explicit vault when multiple are present

- **WHEN** a client calls `wiki_read` in registry mode with no `vault` argument AND two or more present vaults are registered
- **THEN** the tool SHALL return an MCP error instructing the caller to specify a `vault` (the one carried on the `wiki_list` / `wiki_search` result), and SHALL NOT read any page

### Requirement: wiki_search performs keyword substring search

`wiki_search(query, vault?)` SHALL perform case-insensitive substring matching of `query` against each page's title and body across the resolved vault(s), treating `query` as a single needle (no tokenization). The `vault` argument is OPTIONAL and resolved per the `Vault selection across startup modes` requirement; when omitted in registry mode with more than one present vault, `wiki_search` SHALL search across ALL present registered vaults. For each matched page it SHALL return its source `vault` (normalized absolute path) and `name` (display name) alongside slug, title, and a snippet of up to 100 characters on each side of the first match (on character boundaries). Results SHALL be capped at 20 results IN TOTAL across all searched vaults (a global cap, one snippet per page); when more pages match than are returned, the result SHALL set `truncated` to true. The tool description SHALL instruct callers to pass a keyword rather than a full sentence. RAG / semantic search is out of scope; this grep fallback is the implemented behavior.

#### Scenario: Matches a keyword and returns a snippet tagged with its vault

- **WHEN** a client calls `wiki_search` with a keyword that occurs in one or more pages
- **THEN** the result SHALL list each matched page with its source `vault`, `name`, `slug`, `title`, and a context snippet around the first occurrence

#### Scenario: No match returns an empty result, not an error

- **WHEN** a client calls `wiki_search` with a keyword that occurs in no page of the resolved vault(s)
- **THEN** the result SHALL be an empty result list returned as success

#### Scenario: Blank query is rejected

- **WHEN** a client calls `wiki_search` with an empty or whitespace-only `query`
- **THEN** the tool SHALL return an MCP error result

#### Scenario: Aggregates matches across all present vaults with a global cap

- **WHEN** a client calls `wiki_search` in registry mode with no `vault` argument AND two or more present vaults are registered AND the combined matches exceed 20 pages
- **THEN** the result SHALL contain at most 20 hits drawn from across the present vaults, each tagged with its source `vault` and `name`, AND `truncated` SHALL be true

### Requirement: Read-only security boundary

The server SHALL read only from the resolved vault's `<vault>/.codebus/wiki/` subtree and SHALL NOT expose `<vault>/.codebus/raw/` (the PII-redacted code mirror) or any path outside the wiki subtree. In registry mode the resolved vault SHALL be a member of the app-state registry: a supplied `vault` argument SHALL be accepted only when its canonicalized path equals the canonicalized path of a registered, present (non-missing) vault entry; any other `vault` — including a path outside the registry such as a home-directory secret path — SHALL be rejected with an MCP error. When `vault` is omitted and a tool aggregates across vaults, the server SHALL iterate ONLY the registered present vaults, so aggregation never reaches a path outside the registry. The server SHALL treat the registry as READ-ONLY and SHALL NOT write `app-state.json`. Slug resolution SHALL match pages by filename stem rather than by path joining, so a slug containing `../` or path separators cannot escape the wiki subtree. The resolved page path SHALL be verified to remain within the wiki subtree before reading.

#### Scenario: The raw code mirror is unreachable

- **WHEN** a client supplies a slug crafted to resemble a path into `<vault>/.codebus/raw/code/` or any location outside the wiki subtree
- **THEN** the server SHALL NOT return any file outside the wiki subtree; resolution by filename stem SHALL only ever match pages already within the wiki subtree

#### Scenario: Traversal slug cannot escape

- **WHEN** a client calls `wiki_read` with a slug containing `..` path components or separators
- **THEN** the tool SHALL either match a same-named wiki page by stem or return an error, and SHALL NOT read a file outside the wiki subtree

#### Scenario: Vault outside the registry is rejected

- **WHEN** a client calls a wiki tool with a `vault` whose canonicalized path matches no registered, present vault (for example a home-directory path like `~/.ssh`)
- **THEN** the tool SHALL return an MCP error AND SHALL NOT read any file under that path

#### Scenario: Aggregation stays within the registry

- **WHEN** a client calls `wiki_search` or `wiki_list` with no `vault` argument in registry mode
- **THEN** the server SHALL search or list only the registered present vaults AND SHALL NOT read any path outside the registry

#### Scenario: The registry is read-only to the server

- **WHEN** the server resolves vaults from `~/.codebus/app-state.json` over its lifetime
- **THEN** it SHALL only read the file AND SHALL NOT create, modify, or delete `app-state.json`

## ADDED Requirements

### Requirement: vault_list enumerates registry vaults

`vault_list` SHALL return one entry per vault that is both registered in `~/.codebus/app-state.json` and present on disk. Each entry SHALL carry `vault` — the normalized absolute path, which is the stable identifier the wiki tools accept — and `name` — the display name, for human readability only and NOT used for addressing. Entries whose path is missing or unreadable SHALL be omitted. In pinned mode (`--vault`), `vault_list` SHALL return exactly the single pinned vault. The registry SHALL be re-read on each resolution so a vault added while the server runs becomes visible without restarting the server. An empty or absent registry SHALL yield an empty list returned as success, not an error. `vault_list` is a discovery aid, not a required first call — a caller can also omit `vault` on `wiki_list` / `wiki_search` to explore across all present vaults.

#### Scenario: Lists registered present vaults

- **WHEN** a client calls `vault_list` in registry mode AND two registered vault paths exist on disk
- **THEN** the result SHALL contain one entry per present vault, each with a `vault` absolute-path identifier and a `name` display label

#### Scenario: Missing vault entries are omitted

- **WHEN** a registered vault's path no longer exists on disk AND a client calls `vault_list`
- **THEN** that vault SHALL be omitted from the result

#### Scenario: Empty registry returns an empty list

- **WHEN** a client calls `vault_list` AND `~/.codebus/app-state.json` is absent or contains no vault entries
- **THEN** the result SHALL be an empty list returned as success, not an error

#### Scenario: Newly added vault becomes visible without restart

- **WHEN** the registry-mode server is already running AND a new vault entry is appended to `~/.codebus/app-state.json` by the app AND a client then calls `vault_list`
- **THEN** the result SHALL include the newly added vault

#### Scenario: Pinned mode returns the single vault

- **WHEN** the server was started with `--vault <path>` AND a client calls `vault_list`
- **THEN** the result SHALL contain exactly one entry for the pinned vault

### Requirement: Vault selection across startup modes

When a wiki tool (`wiki_list` / `wiki_read` / `wiki_search`) is invoked, the served vault(s) SHALL be resolved deterministically from the startup mode, the supplied `vault` argument, the registry contents, and the tool's nature: `wiki_list` and `wiki_search` are exploratory and aggregate across vaults on omission, while `wiki_read` locates a single page and requires an unambiguous vault. In pinned mode the pinned vault SHALL be used; a supplied `vault` that differs from the pinned path SHALL be rejected with an MCP error (fail-loud, NOT silently ignored). In registry mode: when `vault` is supplied it SHALL be resolved against the registry whitelist (per the `Read-only security boundary` requirement); when `vault` is omitted AND exactly one present vault is registered, that vault SHALL be used; when `vault` is omitted AND more than one present vault is registered, `wiki_list` and `wiki_search` SHALL query ALL present registered vaults and tag each result with its source `vault`, while `wiki_read` SHALL return an MCP error instructing the caller to specify a vault; when no present vault is registered, the tool SHALL return an MCP error stating that no vault is registered.

#### Scenario: Multi-vault registry mode searches and lists across all present vaults on omission

- **WHEN** a client calls `wiki_search` or `wiki_list` in registry mode with no `vault` argument AND two or more present vaults are registered
- **THEN** the tool SHALL query every present registered vault AND SHALL tag each result with its source `vault` and `name`

#### Scenario: Read requires an explicit vault when multiple are present

- **WHEN** a client calls `wiki_read` in registry mode with no `vault` argument AND two or more present vaults are registered
- **THEN** the tool SHALL return an MCP error instructing the caller to specify a `vault`, and SHALL NOT aggregate or read any page

#### Scenario: Single-vault registry mode defaults on omission

- **WHEN** a client calls any wiki tool in registry mode with no `vault` argument AND exactly one present vault is registered
- **THEN** the tool SHALL resolve to that single registered vault and serve its wiki

#### Scenario: Pinned mode rejects a mismatched vault

- **WHEN** the server was started with `--vault <path>` AND a client calls a wiki tool with a `vault` argument that differs from the pinned path
- **THEN** the tool SHALL return an MCP error (fail-loud) AND SHALL NOT serve the supplied path; an omitted `vault` SHALL serve the pinned vault

##### Example: vault resolution matrix

| startup mode | present vaults in registry | `vault` argument | `wiki_list` / `wiki_search` | `wiki_read` |
| ------------ | -------------------------- | ---------------- | --------------------------- | ----------- |
| registry     | 2 or more                  | omitted          | query all present vaults (tagged) | MCP error: specify vault |
| registry     | exactly 1                  | omitted          | use that one vault | use that one vault |
| registry     | 1 or more                  | supplied, in registry | use the supplied vault | use the supplied vault |
| registry     | any                        | supplied, not in registry | MCP error: vault not in registry | MCP error: vault not in registry |
| registry     | 0                          | omitted or supplied | MCP error: no vault registered | MCP error: no vault registered |
| pinned       | n/a                        | omitted          | use the pinned vault | use the pinned vault |
| pinned       | n/a                        | supplied ≠ pinned | MCP error: vault mismatch | MCP error: vault mismatch |
