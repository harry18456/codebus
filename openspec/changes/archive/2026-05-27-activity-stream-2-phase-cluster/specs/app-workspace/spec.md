## ADDED Requirements

### Requirement: Activity Stream Two-Phase Cluster Rendering

The Activity stream block in the Run Detail Views — Running, Done, Interrupted, and Failed states SHALL wrap consecutive tool-use events into semantic phase clusters according to the design v1.5 specification. Two cluster phases SHALL exist:

- **READING CODEBASE** — tool calls that intake information without mutating state. The phase SHALL contain `Read`, `Glob`, `Grep`, `ToolUse` events whose `tool_kind` is `read` or `inspect`, AND any future tool whose `tool_kind` is `other_read`.
- **WRITING WIKI** — tool calls that mutate state. The phase SHALL contain `Write`, `Edit`, `ToolUse` events whose `tool_kind` is `mutation`, AND any future tool whose `tool_kind` is `other_write`.

Each cluster SHALL render as a single collapsible row containing a heading, a count of contained tool-use events, and (when expanded) the child rows in arrival order using the existing per-event renderer.

Cluster boundary rules SHALL be:

- Cluster boundary opens when the first qualifying tool-use event of a phase arrives after either the start of the stream or a non-tool-use event (banner or thought).
- Cluster boundary closes when any of the following arrives: a banner event, a thought block, OR a tool-use event whose phase classification differs from the open cluster.
- Clusters SHALL be allowed to repeat across the timeline (e.g. `thought → reading → thought → reading → writing` is a legal sequence where two distinct READING CODEBASE clusters are rendered with a thought block between them).
- The cluster count SHALL only include rendered tool-use rows; thought blocks and banners SHALL NOT be counted.
- Tool-use events whose `tool_kind` is missing (`None` / `undefined`) SHALL be classified as `Inspect` for cluster purposes and grouped under READING CODEBASE.

Cluster collapsible default state SHALL track the run status: a cluster SHALL render expanded by default when the surrounding run is still `running`, AND SHALL render collapsed by default once the run reaches any terminal state (`done`, `interrupted`, `failed`). The user SHALL be able to toggle a cluster open or closed by activating its heading; the heading SHALL be a `<button>` carrying `aria-expanded` and `aria-controls` attributes.

Cluster heading icon prefixes SHALL use mono ASCII / single glyph form (NOT brand emoji) per design v1.5 lock:

| Tool / kind | Icon |
| --- | --- |
| `Read` | `📄` |
| `Glob` | `🗂` |
| `Grep` | `🔍` |
| `Shell` with `tool_kind: read` | `$_` |
| `Shell` with `tool_kind: inspect` | `$?` |
| `Write` / `Edit` | `✎` |
| `Shell` with `tool_kind: mutation` | `$!` |

Cluster heading label SHALL be localized:

- READING CODEBASE heading (en): `Reading codebase`; (zh): `讀檔案`
- WRITING WIKI heading (en): `Writing wiki`; (zh): `寫 wiki`

Cluster collapsed summary, rendered only after the run reaches a terminal state, SHALL include counts in the localized form:

- READING (en): `Reading codebase · {reads} reads · {shell} shell · {elapsedSeconds}s`
- READING (zh): `讀檔案 {reads} 次 · shell {shell} 次 · {elapsedSeconds} 秒`
- WRITING (en): `Writing wiki · {new} new · {updated} updated · {elapsedSeconds}s`
- WRITING (zh): `新增 {new} · 更新 {updated} · {elapsedSeconds} 秒`

Cluster wrapping SHALL be implemented as a pure timeline projection function (`projectClusters`) consumed by a dedicated `ActivityCluster` React component. The function SHALL NOT mutate input arrays AND SHALL be unit-testable independently of React.

#### Scenario: Consecutive read tools fold into one READING CODEBASE cluster

- **WHEN** the Activity stream receives, in order, `ToolUse(Read, file_path=a.md)`, `ToolUse(Glob, pattern=wiki/**.md)`, `ToolUse(Grep, pattern=foo)`
- **THEN** the stream SHALL render exactly one `ActivityCluster` element with phase `reading_codebase`, count `3`, AND its three child rows in arrival order

##### Example: cluster count excludes thought

- **GIVEN** event sequence `Read`, `Thought("checking ...")`, `Read`, `Read`
- **WHEN** projected through `projectClusters`
- **THEN** the result SHALL contain two `ActivityCluster` entries with counts `1` and `2` and a `thought_block` between them

#### Scenario: Phase change breaks cluster and opens a new one

- **WHEN** the Activity stream receives, in order, `ToolUse(Read)`, `ToolUse(Write, file_path=wiki/a.md)`, `ToolUse(Edit, file_path=wiki/b.md)`
- **THEN** the stream SHALL render exactly one READING CODEBASE cluster (count `1`) followed by exactly one WRITING WIKI cluster (count `2`)

#### Scenario: Banner inside cluster sequence ends the cluster

- **WHEN** the Activity stream receives, in order, `ToolUse(Read)`, `VerbBanner::commit_done { sha7 }`, `ToolUse(Read)`
- **THEN** the stream SHALL render in order: a READING CODEBASE cluster with count `1`, a `stream-banner` row for `commit_done`, then a second READING CODEBASE cluster with count `1`

#### Scenario: Cluster default open during running, collapsed when terminal

- **WHEN** the user views the Run Detail Views — Running for an in-flight run AND a READING CODEBASE cluster of three Read events is present
- **THEN** the cluster's heading SHALL carry `aria-expanded="true"` AND the three child rows SHALL be visible in the DOM
- **AND WHEN** the run transitions to the `done` terminal state and the user re-mounts the cluster element
- **THEN** the cluster's heading SHALL carry `aria-expanded="false"` AND the three child rows SHALL NOT be visible

#### Scenario: User toggles cluster open and closed

- **WHEN** the user activates the cluster heading button while it carries `aria-expanded="false"`
- **THEN** the heading attribute SHALL flip to `aria-expanded="true"` AND the child rows SHALL become visible
- **AND WHEN** the user activates the same button a second time
- **THEN** the attribute SHALL flip back to `aria-expanded="false"` AND the child rows SHALL no longer be visible

#### Scenario: Cluster heading uses mono icon and localized label

- **WHEN** a READING CODEBASE cluster is rendered in the `en` locale
- **THEN** the heading SHALL contain the string `Reading codebase` AND SHALL contain at least one of the icons `📄`, `🗂`, `🔍`, `$_`, or `$?` AND SHALL NOT contain the emoji `🛠️`

##### Example: cluster terminal summary string

| Locale | Phase | Counts | Expected summary fragment |
| --- | --- | --- | --- |
| `en` | reading | `reads=12`, `shell=195`, `elapsedSeconds=6.2` | `Reading codebase · 12 reads · 195 shell · 6.2s` |
| `zh` | reading | `reads=12`, `shell=195`, `elapsedSeconds=6.2` | `讀檔案 12 次 · shell 195 次 · 6.2 秒` |
| `en` | writing | `new=3`, `updated=2`, `elapsedSeconds=4.5` | `Writing wiki · 3 new · 2 updated · 4.5s` |
| `zh` | writing | `new=3`, `updated=2`, `elapsedSeconds=4.5` | `新增 3 · 更新 2 · 4.5 秒` |

#### Scenario: Missing tool_kind defaults to Inspect

- **WHEN** the Activity stream receives a `ToolUse(Bash, command="git status")` event whose `tool_kind` field is `undefined`
- **THEN** the event SHALL be grouped under a READING CODEBASE cluster (treated as `Inspect`) AND the frontend SHALL NOT log a console warning or error
