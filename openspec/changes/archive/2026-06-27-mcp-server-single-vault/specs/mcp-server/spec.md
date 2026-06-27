## ADDED Requirements

### Requirement: Single-vault stdio MCP server lifecycle

The system SHALL provide a `codebus mcp --vault <path>` subcommand that starts a Model Context Protocol server over stdio transport, bound to exactly one vault pinned at startup. The server SHALL advertise the `tools` capability and SHALL NOT advertise resources or prompts.

#### Scenario: Server starts for a valid vault

- **WHEN** `codebus mcp --vault <path>` is invoked AND `<path>/.codebus/wiki/` exists and is a directory
- **THEN** the process SHALL start an MCP server that exchanges JSON-RPC over stdin/stdout, advertises only the `tools` capability, and SHALL keep serving until stdin closes or the process is terminated

#### Scenario: Missing wiki directory aborts startup

- **WHEN** `codebus mcp --vault <path>` is invoked AND `<path>/.codebus/wiki/` does not exist or is not a directory
- **THEN** the process SHALL exit with a non-zero status and write an explanatory message to stderr, and SHALL NOT start the server

#### Scenario: Diagnostics never pollute the protocol channel

- **WHEN** the server emits any log, warning, or diagnostic output
- **THEN** it SHALL write that output to stderr only, leaving stdout exclusively for JSON-RPC frames

### Requirement: Tools-only query surface without path parameters

The server SHALL expose exactly three query-only tools — `wiki_list`, `wiki_read`, and `wiki_search`. No tool SHALL accept a vault path, filesystem path, or any argument that selects a directory; the served vault is fixed at startup. No tool SHALL mutate the vault.

#### Scenario: tools/list enumerates the three query tools

- **WHEN** a client sends a `tools/list` request
- **THEN** the response SHALL contain `wiki_list`, `wiki_read`, and `wiki_search`, each with an auto-generated input schema, and SHALL NOT contain any write tool or any tool that accepts a path argument

### Requirement: wiki_list returns the page index

`wiki_list` SHALL return the slug and title of every Markdown page under the pinned `<vault>/.codebus/wiki/` tree. It SHALL tolerate pages whose frontmatter is missing or malformed by using the filename stem as both slug and title fallback.

#### Scenario: Lists pages including those without frontmatter

- **WHEN** a client calls `wiki_list` against a wiki tree containing pages with and without YAML frontmatter
- **THEN** the result SHALL contain one entry per `.md` file, where slug is the filename stem and title is the frontmatter `title` when present, otherwise the slug

#### Scenario: Empty vault returns an empty list

- **WHEN** a client calls `wiki_list` AND the wiki tree contains no `.md` files
- **THEN** the result SHALL be an empty list returned as success, not an error

### Requirement: wiki_read returns the paginated page body

`wiki_read(slug, offset, limit)` SHALL return the page body with the leading frontmatter block stripped, paginated by Unicode character (`char`) count over the stripped body. `offset` SHALL default to 0; `limit` SHALL default to 12000 and SHALL be clamped to a maximum of 20000. Slicing SHALL occur on character boundaries so multi-byte UTF-8 / CJK characters are never split. The result SHALL include the returned `content`, the `offset` used, a `next_offset` (null when fully read), a `has_more` boolean, and `total_chars`.

#### Scenario: Reads a page and strips frontmatter

- **WHEN** a client calls `wiki_read` with a slug that resolves to a page beginning with a `---` frontmatter block
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

- **WHEN** a client calls `wiki_read` with a slug that matches no page
- **THEN** the tool SHALL return an MCP error result, not a success with empty content

### Requirement: wiki_search performs keyword substring search

`wiki_search(query)` SHALL perform case-insensitive substring matching of `query` against each page's title and body, treating `query` as a single needle (no tokenization). For each matched page it SHALL return slug, title, and a snippet of up to 100 characters on each side of the first match (on character boundaries). Results SHALL be capped at 20 pages with one snippet per page; when more pages match, the result SHALL set `truncated` to true. The tool description SHALL instruct callers to pass a keyword rather than a full sentence. RAG / semantic search is out of scope; this grep fallback is the implemented behavior.

#### Scenario: Matches a keyword and returns a snippet

- **WHEN** a client calls `wiki_search` with a keyword that occurs in one or more pages
- **THEN** the result SHALL list each matched page with its slug, title, and a context snippet around the first occurrence

#### Scenario: No match returns an empty result, not an error

- **WHEN** a client calls `wiki_search` with a keyword that occurs in no page
- **THEN** the result SHALL be an empty result list returned as success

#### Scenario: Blank query is rejected

- **WHEN** a client calls `wiki_search` with an empty or whitespace-only `query`
- **THEN** the tool SHALL return an MCP error result

##### Example: query handling

| query        | result                                  | notes                       |
| ------------ | --------------------------------------- | --------------------------- |
| "auth"       | pages containing "auth" (any case)      | normal keyword match        |
| "AUTH"       | same pages as "auth"                    | case-insensitive            |
| "zzznomatch" | empty list, success                     | legitimate no-match         |
| ""           | MCP error                               | blank query rejected        |
| "   "        | MCP error                               | whitespace-only rejected    |

### Requirement: Read-only security boundary

The server SHALL read only from the pinned `<vault>/.codebus/wiki/` subtree and SHALL NOT expose `<vault>/.codebus/raw/` (the PII-redacted code mirror) or any path outside the wiki subtree. Slug resolution SHALL match pages by filename stem rather than by path joining, so a slug containing `../` or path separators cannot escape the wiki subtree. The resolved page path SHALL be verified to remain within the wiki subtree before reading.

#### Scenario: The raw code mirror is unreachable

- **WHEN** a client supplies a slug crafted to resemble a path into `<vault>/.codebus/raw/code/` or any location outside the wiki subtree
- **THEN** the server SHALL NOT return any file outside the wiki subtree; resolution by filename stem SHALL only ever match pages already within the wiki subtree

#### Scenario: Traversal slug cannot escape

- **WHEN** a client calls `wiki_read` with a slug containing `..` path components or separators
- **THEN** the tool SHALL either match a same-named wiki page by stem or return an error, and SHALL NOT read a file outside the wiki subtree

### Requirement: Error-versus-empty semantics and non-blocking filesystem access

Real failures SHALL surface as MCP `ErrorData` and SHALL NOT be silently coerced into empty successful results; legitimately empty outcomes SHALL return success. Blocking filesystem work (directory recursion, file reads) SHALL run off the async runtime's main threads so a slow read cannot stall the server.

#### Scenario: Filesystem read failure surfaces as an error

- **WHEN** a tool's underlying filesystem read fails (for example, a permission error on a resolved page)
- **THEN** the tool SHALL return an MCP error result rather than an empty success

#### Scenario: Empty outcomes are successes

- **WHEN** `wiki_list` finds no pages OR `wiki_search` finds no matches
- **THEN** the tool SHALL return an empty result as success, distinguishable by the client from an error
