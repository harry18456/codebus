## ADDED Requirements

### Requirement: Query Bundle Workflow Content

The `codebus-query/SKILL.md` file written by init SHALL contain a `## Workflow` section (or equivalently-named heading containing the substring `Workflow`) that documents a four-step read-only lookup procedure for the query verb. The four steps SHALL be presented in order and SHALL describe: (1) parsing the query intent and identifying which taxonomy types under `wiki/` are likely relevant; (2) globbing `wiki/` for candidate pages, reading frontmatter first as a relevance filter, and only reading body when frontmatter matches; (3) following `[[wikilink]]` references in matched pages to assemble cross-page context, with bounded depth to avoid drift; (4) emitting the answer to stdout in the same natural language as the query text per the cwd `CLAUDE.md` Language Policy, without copying phrasing from the SKILL.md verbatim.

The workflow body SHALL explicitly declare the read-only invariant: the agent MUST NOT use Write or Edit, and MUST NOT mutate any file inside `wiki/`, `raw/`, or anywhere else. The body SHALL note that the toolset is also gated at the binary layer (so a Write attempt will fail at the runtime), but the SKILL.md statement of the invariant is required for defense-in-depth.

The workflow body SHALL be written in English (no characters in the CJK Unified Ideographs block U+4E00..U+9FFF, except inside ASCII-only path or wikilink slug fragments). Step 4's instruction SHALL be abstract — describing the desired output shape rather than providing a literal sample answer phrase that the agent could copy verbatim — and SHALL reference cwd `CLAUDE.md` as the source of truth for output language.

The workflow body SHALL NOT inline schema rules (taxonomy definitions, frontmatter field formats, wikilink resolution rules); these rules belong in cwd `CLAUDE.md` only. References to `CLAUDE.md` from workflow steps are permitted.

#### Scenario: codebus-query SKILL.md contains four-step workflow markers

- **WHEN** init runs against a repository with no existing `<repo>/.codebus/.claude/skills/codebus-query/SKILL.md`
- **THEN** the resulting file SHALL contain a `## Workflow` heading AND the body under that heading SHALL contain at least four distinct numbered list items (lines beginning with `1.`, `2.`, `3.`, `4.`)

#### Scenario: codebus-query workflow declares read-only invariant

- **WHEN** init writes the query bundle SKILL.md
- **THEN** the workflow body SHALL contain a substring stating that the agent MUST NOT use Write or Edit (case-insensitive match for the phrase `MUST NOT use Write` or equivalent canonical wording)
- **AND** the workflow body SHALL contain a substring stating that the toolset is gated at the binary layer

#### Scenario: codebus-query workflow body is written in English

- **WHEN** init writes the query bundle SKILL.md
- **THEN** the workflow body (everything under the `## Workflow` heading through end of file) SHALL NOT contain any character in the CJK Unified Ideographs block (Unicode range U+4E00 through U+9FFF)

#### Scenario: codebus-query step 4 is abstract, not a literal output template

- **WHEN** init writes the query bundle SKILL.md
- **THEN** the workflow body's step 4 SHALL describe the desired stdout answer's shape (language matching the query text, no literal sample) without including any literal sample answer phrase the agent could copy verbatim
- **AND** the step 4 instruction SHALL reference cwd `CLAUDE.md` as the source of truth for output language
- **AND** the step 4 instruction SHALL include an explicit directive that the agent MUST NOT copy phrasing from this SKILL.md verbatim

#### Scenario: codebus-query workflow defers schema rules to CLAUDE.md

- **WHEN** init writes the query bundle SKILL.md
- **THEN** the workflow body SHALL contain the substring `CLAUDE.md` (the schema reference)
- **AND** the workflow body SHALL NOT contain inline taxonomy definitions enumerating concepts, entities, modules, processes, and synthesis as the five page types in a single sentence

## MODIFIED Requirements

### Requirement: Goal Bundle Workflow Content

The `codebus-goal/SKILL.md` file written by init SHALL contain a `## Workflow` section (or equivalently-named heading containing the substring `Workflow`) that documents a five-step ingest procedure for the goal verb. The five steps SHALL be presented in order and SHALL describe: (1) exploring the raw mirror under `raw/code/` to find sources relevant to the goal; (2) planning which wiki pages to create or update across the five taxonomy folders (`concepts/`, `entities/`, `modules/`, `processes/`, `synthesis/`); (3) writing each page's frontmatter and body; (4) establishing wikilinks between pages using filename references; (5) emitting a brief summary line (counts of new and modified pages) to stdout at the end.

The workflow content SHALL NOT inline the schema rules (taxonomy definitions, frontmatter format details, wikilink resolution rules, stop criteria). Schema rules SHALL be referenced by pointing the agent to the cwd-relative `CLAUDE.md` file; the workflow body SHALL NOT duplicate the schema content. References to `CLAUDE.md` as the schema source-of-truth are permitted in workflow steps.

The remaining bundle (`codebus-fix/SKILL.md`) SHALL retain its stub workflow content from v3-init until subsequent change #8 v3-fix replaces it. Its existing requirements (Skill Bundle Layout, Stub Bundle Content Format, Write-If-Missing Semantics) SHALL continue to apply.

#### Scenario: codebus-goal SKILL.md contains five-step workflow markers

- **WHEN** init runs against a repository with no existing `<repo>/.codebus/.claude/skills/codebus-goal/SKILL.md`
- **THEN** the resulting file SHALL contain a `## Workflow` heading AND the body under that heading SHALL contain at least five distinct numbered list items (lines beginning with `1.`, `2.`, `3.`, `4.`, `5.`)

#### Scenario: codebus-goal workflow references raw/code and wiki cwd-relatively

- **WHEN** init writes the goal bundle SKILL.md
- **THEN** the workflow body SHALL contain the substring `raw/code/` AND SHALL contain the substring `wiki/` AND SHALL NOT contain the substring `.codebus/raw/code/` AND SHALL NOT contain the substring `.codebus/wiki/`

#### Scenario: codebus-goal workflow defers schema rules to CLAUDE.md

- **WHEN** init writes the goal bundle SKILL.md
- **THEN** the workflow body SHALL contain the substring `CLAUDE.md` (the schema reference) AND SHALL NOT contain inline taxonomy definitions enumerating concepts, entities, modules, processes, and synthesis as the five page types in a single sentence (the schema's authoritative enumeration belongs in `CLAUDE.md` only)

#### Scenario: codebus-goal workflow mentions all five taxonomy folder names

- **WHEN** init writes the goal bundle SKILL.md
- **THEN** the workflow body SHALL mention each of `concepts`, `entities`, `modules`, `processes`, `synthesis` at least once (so the agent knows which folders are valid page locations) but the mention SHALL be brief enumeration only (e.g., as a parenthetical list within a single step), not a definition of each type

#### Scenario: codebus-fix bundle retains stub workflow

- **WHEN** init runs against a repository with no existing skill bundles
- **THEN** the resulting `<repo>/.codebus/.claude/skills/codebus-fix/SKILL.md` SHALL retain stub workflow content (no expanded workflow), pending replacement by v3-fix

#### Scenario: codebus-goal workflow body is written in English

- **WHEN** init writes the goal bundle SKILL.md
- **THEN** the workflow body (everything under the `## Workflow` heading through end of file) SHALL be written in English: it SHALL NOT contain any character in the CJK Unified Ideographs block (Unicode range U+4E00 through U+9FFF), with the exception of file path components or wikilink slugs that remain ASCII anyway

#### Scenario: Step 5 instruction is abstract, not a literal output template

- **WHEN** init writes the goal bundle SKILL.md
- **THEN** the workflow body's step 5 SHALL describe the desired stdout summary's shape (count of created vs modified pages, language matching the goal text) without including any literal sample summary phrase that the agent could copy verbatim into stdout
- **AND** the step 5 instruction SHALL reference the cwd `CLAUDE.md` Language Policy as the source of truth for the output language
- **AND** the step 5 instruction SHALL include an explicit directive that the agent MUST NOT copy phrasing from this SKILL.md verbatim into the stdout summary
