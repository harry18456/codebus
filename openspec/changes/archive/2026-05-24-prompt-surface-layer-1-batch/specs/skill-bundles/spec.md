## ADDED Requirements

### Requirement: NEUTRAL_RULES Language Policy

The `NEUTRAL_RULES` schema document (materialized as `<vault>/CLAUDE.md` on the Claude provider path and as `<vault>/AGENTS.md` on the codex provider path) SHALL contain a `§0 Language Policy` section preceding `§1 Workspace Layout` that defines two normative rules:

1. The natural language of agent output — page bodies, stdout summary lines, and answer text — SHALL follow the prompt context language (the language of the user's goal/query/chat text), and SHALL NOT default to the language of any existing wiki page or raw source content read along the way.
2. Structural tokens and YAML keys (`type:`, `sources:`, `created:`, `updated:`, marker lines such as `[CODEBUS_*]`) SHALL always be literal English regardless of the prompt context language.

This requirement makes the contract real for the SKILL workflow body references (in `codebus-core/src/skill_bundle/mod.rs`, including goal Step 5, query Step 4, fix Step 5, and quiz mode validation paths) that cite "the §0 Language Policy in cwd CLAUDE.md" as the authority for output-language selection. Without `§0` the contract is dangling: agent behavior on multi-language prompts falls back to the underlying model's heuristic, producing inconsistent output language across providers and across model versions.

#### Scenario: Multi-language goal produces same-language summary

- **WHEN** a user runs `codebus goal "把支付模組的時序圖整理出來"` (Traditional Chinese goal text) against a vault whose existing wiki pages are written in English
- **THEN** the agent's stdout summary line and any newly written or updated wiki page body content SHALL be in Traditional Chinese
- **AND** structural frontmatter keys (`type:`, `sources:`, `created:`, `updated:`, `[CODEBUS_*]` markers) SHALL remain literal English

##### Example: Mixed-language vault, Japanese goal

- **GIVEN** a vault containing `wiki/modules/payment-gateway.md` authored in English and `wiki/concepts/checkout-flow.md` authored in Traditional Chinese
- **WHEN** a user runs `codebus goal "決済処理の主要なコンポーネントを把握したい"` (Japanese goal text)
- **THEN** the stdout summary line SHALL be in Japanese
- **AND** any new `## from goal: ... (YYYY-MM-DD)` section appended to existing pages SHALL have its body in Japanese (per Language Override) while the `## from goal:` heading literal and date stay English/numeric
- **AND** `type:`, `sources:`, `goals:`, `created:`, `updated:` keys in frontmatter SHALL remain literal English

#### Scenario: Schema document materialized with Language Policy preceding workspace layout

- **WHEN** `codebus init` materializes `NEUTRAL_RULES` to the vault's `CLAUDE.md` (Claude provider path) or `AGENTS.md` (codex provider path)
- **THEN** the materialized file SHALL contain `## 0. Language Policy` as a section ordered before `## 1. Workspace Layout`
- **AND** the section body SHALL define both the agent-output-language rule and the structural-tokens-stay-English rule
