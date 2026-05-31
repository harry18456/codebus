## ADDED Requirements

### Requirement: Codex Body Translation Drift Guard

The codex SKILL body is derived from the Claude SKILL body by a fixed set of literal source-to-target string replacements (the Claude body is the source of truth per the `Codex Instruction Materialization` requirement). Because a literal replacement whose source string is absent from the body is a silent no-op, the derivation SHALL be drift-guarded by an automated test invariant with two parts:

- **(a) Every translation source-literal SHALL match the current Claude body.** For each source-literal used to derive a codex body from a Claude body, that literal SHALL be present in at least one of the rendered Claude SKILL bodies (`goal`, `query`, `fix`, `chat`, `quiz`). A source-literal absent from every rendered Claude body SHALL fail the guard, because its replacement can no longer fire and the corresponding codex body segment would silently retain Claude-only content. The failure SHALL name the unmatched source-literal.

- **(b) No rendered codex SKILL body SHALL contain a Claude-only mechanism token.** For each verb, the rendered codex SKILL body SHALL NOT contain any of the Claude-only mechanism tokens `--tools`, `PreToolUse`, `mcp_`, `CLAUDE.md`, or the Claude bash heredoc self-validation delimiter `<<'CBQZ'`. A codex body containing any such token SHALL fail the guard, naming the offending verb and token. (The denylist intentionally excludes the bare command `codebus quiz validate`, which the codex quiz body legitimately references in its no-validate paragraph; the Claude-only signal is the heredoc delimiter, not the command.)

The set of source-literals checked by part (a) SHALL be the same set the derivation applies (a single shared source of truth), so that adding or changing a replacement automatically extends the guard rather than requiring a separately maintained copy. The guard's detection logic SHALL itself be covered by a meta-test that confirms it reports a deliberately unmatched source-literal and a deliberately leaked Claude-only token as failures (and reports the real, current inputs as passing).

This requirement constrains test coverage of the derivation mechanism; it SHALL NOT change the materialized SKILL body content, which continues to follow the `Codex Instruction Materialization` requirement.

#### Scenario: Every codex translation source-literal matches a current Claude body

- **WHEN** the codex body derivation's source-literals are checked against the rendered Claude SKILL bodies for `goal`, `query`, `fix`, `chat`, and `quiz`
- **THEN** each source-literal SHALL be present in at least one of those Claude bodies
- **AND** if any source-literal is absent from every Claude body, the guard SHALL fail and name that source-literal

#### Scenario: Codex SKILL body contains no Claude-only mechanism token

- **WHEN** the codex SKILL body is rendered for each of `goal`, `query`, `fix`, `chat`, and `quiz`
- **THEN** none of those codex bodies SHALL contain the tokens `--tools`, `PreToolUse`, `mcp_`, `CLAUDE.md`, or `<<'CBQZ'`
- **AND** if any codex body contains such a token, the guard SHALL fail and name the offending verb and token

#### Scenario: Drift guard detects an unmatched source-literal and a leaked token

- **WHEN** the guard's detection logic is given a source-literal that is absent from every Claude body, and separately a synthetic codex body containing a Claude-only token
- **THEN** the detection logic SHALL report the unmatched source-literal as a failure AND report the leaked token as a failure
- **AND** when given the real current source-literals and the real rendered codex bodies, the detection logic SHALL report no failures
