## MODIFIED Requirements

### Requirement: SpawnSpec Provider-Neutral Intent

The `SpawnSpec` type SHALL carry provider-neutral spawn intent and SHALL NOT embed provider-specific encodings (no slash-command strings, no CLI flag glob syntax, no provider-specific trigger prefix). `SpawnSpec` SHALL contain:

- `verb` (one of the five SKILL bundle verbs: `Goal`, `Query`, `Fix`, `Chat`, `Quiz`) — the bundle name used by both providers to address the SKILL workflow. The `verb` field is the **bundle identity**, NOT the model-resolution key.
- `resolve_as` (`Option<Verb>`) — optional model-resolution override. When `None`, the backend SHALL resolve model/effort via `resolve(verb)` (i.e. the bundle's own config sub-block). When `Some(other_verb)`, the backend SHALL resolve via `resolve(other_verb)` instead. The override exists for **cross-flow content-verify spawns**: goal verify and quiz verify spawns set `verb: Goal` / `verb: Quiz` (the SKILL bundle they invoke) but `resolve_as: Some(Verb::Verify)` (so model/effort come from the dedicated `verify` config sub-block per the verify-stage-independent-model pattern).
- `sub_mode` (`Option<String>`) — when present, names a verb sub-mode such as `verify`, `repair`, `plan`, `generate`; when absent, the spawn is a free-text invocation.
- `input` (`String`) — user text or structured body.
- `permission` (an enum with variants `ReadOnly` and `Workspace`).
- `command_allowance` (an optional `CommandPrefix` holding a neutral command token sequence).
- `resume_session_id` (optional).

The `permission`, `command_allowance`, `sub_mode`, `resolve_as`, and `resume_session_id` fields SHALL be per-spawn values, NOT derived from `verb`, because a single verb can issue multiple spawns with differing permission, sub-mode, and model-resolution context. The codebus core SHALL NOT introduce a separate `SpawnRole` enum; model/effort resolution SHALL reuse the existing `Verb` enum and its resolution function (via `resolve_as.unwrap_or(verb)` for the lookup key).

**Backend assembly responsibility**: each concrete `AgentBackend` implementation SHALL synthesize the provider-specific invocation string from `verb` + `sub_mode` + `input`. The verb layer SHALL NOT pre-compose any slash-command or dollar-prefix string into `SpawnSpec`; passing such a pre-composed string would violate the provider-neutral intent of `SpawnSpec`.

**Provider-specific assembly forms**:
- The Claude backend SHALL assemble `/codebus-{verb} {sub_mode}: {input}` when `sub_mode` is `Some`, OR `/codebus-{verb} "{input}"` (with double-quote wrapping) when `sub_mode` is `None`. The `-p` CLI flag SHALL carry the assembled string.
- The codex backend SHALL assemble `$codebus-{verb} {sub_mode}: {input}` when `sub_mode` is `Some`, OR `$codebus-{verb} {input}` (no quote wrapping) when `sub_mode` is `None`. The first positional argument SHALL carry the assembled string. The `$`-prefix invokes the codex CLI's native skill explicit-invocation mechanism (verified 2026-05-23 against codex-cli 0.133.0: `$`-prefix saves approximately 24.8% input tokens versus the claude `/`-prefix because codex routes `/`-prefix through description-match implicit invocation, which adds a separate Read of the SKILL body).

#### Scenario: A single verb issues multiple spawns with differing permission

- **WHEN** the quiz flow runs
- **THEN** it SHALL issue a plan spawn with `verb: Quiz, sub_mode: Some("plan"), resolve_as: None, permission: ReadOnly`, a generate spawn with `verb: Quiz, sub_mode: Some("generate"), resolve_as: None, permission: ReadOnly, command_allowance: Some(["codebus","quiz","validate"])`, and a content-verify spawn with `verb: Quiz, sub_mode: Some("verify"), resolve_as: Some(Verb::Verify), permission: ReadOnly` (the verify spawn invokes the quiz SKILL bundle but resolves model/effort from the dedicated `verify` config sub-block)

#### Scenario: command_allowance is a neutral token sequence

- **WHEN** a `SpawnSpec` restricts the agent to a single command family
- **THEN** `command_allowance` SHALL hold a `CommandPrefix` of plain tokens (e.g. `["codebus","quiz","validate"]`) AND SHALL NOT hold a Claude `--allowedTools` glob string such as `Bash(codebus quiz validate *)`

#### Scenario: Claude backend assembles slash-prefix invocation from SpawnSpec fields

- **WHEN** the Claude backend receives a `SpawnSpec { verb: Goal, sub_mode: None, input: "draft payments overview" }`
- **THEN** the assembled `-p` argument SHALL equal the literal string `/codebus-Goal "draft payments overview"` (quote-wrapped free-text form)
- **WHEN** the Claude backend receives a `SpawnSpec { verb: Goal, sub_mode: Some("verify"), input: "goal=X\n\nCHANGED PAGES:\n..." }`
- **THEN** the assembled `-p` argument SHALL equal the literal string `/codebus-Goal verify: goal=X\n\nCHANGED PAGES:\n...` (sub-mode prefix form, no quote wrapping)

##### Example: Claude assembly for chat verb free-text

- **GIVEN** `SpawnSpec { verb: Chat, sub_mode: None, input: "explain the auth flow" }`
- **WHEN** the Claude backend builds the `claude` CLI command
- **THEN** the `-p` argument SHALL be the literal string `/codebus-Chat "explain the auth flow"`

##### Example: Claude assembly for quiz verb plan sub-mode

- **GIVEN** `SpawnSpec { verb: Quiz, sub_mode: Some("plan"), input: "auth middleware" }`
- **WHEN** the Claude backend builds the `claude` CLI command
- **THEN** the `-p` argument SHALL be the literal string `/codebus-Quiz plan: auth middleware`

#### Scenario: codex backend assembles dollar-prefix invocation from SpawnSpec fields

- **WHEN** the codex backend receives a `SpawnSpec { verb: Goal, sub_mode: None, input: "draft payments overview" }`
- **THEN** the assembled first positional argument SHALL equal the literal string `$codebus-Goal draft payments overview` (no quote wrapping)
- **WHEN** the codex backend receives a `SpawnSpec { verb: Goal, sub_mode: Some("verify"), input: "goal=X\n\nCHANGED PAGES:\n..." }`
- **THEN** the assembled first positional argument SHALL equal the literal string `$codebus-Goal verify: goal=X\n\nCHANGED PAGES:\n...` (sub-mode prefix form)

##### Example: codex assembly for chat verb free-text

- **GIVEN** `SpawnSpec { verb: Chat, sub_mode: None, input: "explain the auth flow" }`
- **WHEN** the codex backend builds the `codex` CLI command
- **THEN** the first positional argument SHALL be the literal string `$codebus-Chat explain the auth flow`

##### Example: codex assembly for quiz verb plan sub-mode

- **GIVEN** `SpawnSpec { verb: Quiz, sub_mode: Some("plan"), input: "auth middleware" }`
- **WHEN** the codex backend builds the `codex` CLI command
- **THEN** the first positional argument SHALL be the literal string `$codebus-Quiz plan: auth middleware`

#### Scenario: SpawnSpec does not embed provider-specific trigger prefix

- **WHEN** a verb layer constructs a `SpawnSpec`
- **THEN** the `input` field SHALL NOT begin with `/codebus-` or `$codebus-` (those prefixes are backend-assembly territory)
- **AND** the `input` field SHALL NOT contain `\"` (double-quote) escaping around free text (claude backend adds quote wrapping on free-text spawns; codex backend never adds quotes — verb layer is unaware of either)
