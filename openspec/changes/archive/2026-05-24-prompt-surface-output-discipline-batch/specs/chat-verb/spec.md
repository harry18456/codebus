## ADDED Requirements

### Requirement: Chat No-Match Discipline Prompt Layer

The `codebus-chat/SKILL.md` body SHALL include a normative clause in the Workflow section (or an explicit sibling section) instructing the agent to handle the no-retrieval-match case without fabricating hypothetical implementation suggestions. When the agent`s `Read` / `Glob` / `Grep` traversal across `wiki/` and `raw/code/` returns nothing relevant to the user`s question, the agent SHALL acknowledge the gap explicitly (for example by stating that the vault does not currently cover the topic) AND SHALL NOT emit a generic walkthrough of how the missing feature might be implemented, structured implementation checklists, or hypothetical architecture suggestions sourced from the agent`s background knowledge rather than from the vault.

The clause SHALL be normative (use SHALL / MUST language) and SHALL be visible to the agent under the Workflow section or an immediately adjacent section so it is read alongside the in-vault retrieval rules. The clause SHALL distinguish this no-match handling from the existing Scope Guard refusal: a no-match question is in-scope (the user is asking about this codebase) but the vault does not contain the answer, so the agent SHALL say so plainly without refusing the whole turn and without inventing content. The agent MAY suggest concrete in-vault next steps the user could take (for example pointing to the closest folder or naming a related page that does exist) when such a step is grounded in retrieved content.

This requirement closes the prompt-surface-review F70 finding (an empirical 2026-05-24 run asked `how does dark mode and theme switching work` against a backend-only vault; the chat agent correctly stated the codebase did not implement the feature but then emitted a five-point hypothetical implementation walkthrough — frontend state management, CSS/theme variables, persistence, provider/context, system preference detection — drawn entirely from the agent`s background knowledge rather than the vault).

#### Scenario: No-match question receives explicit no-match acknowledgement

- **WHEN** a user asks the chat agent about a topic AND `Read` / `Glob` / `Grep` against `wiki/` and `raw/code/` returns nothing covering it (the topic is absent from the vault)
- **THEN** the chat agent SHALL respond with an explicit statement that the vault does not currently cover the topic AND SHALL NOT continue the response with a generic implementation walkthrough or hypothetical architecture suggestion drawn from outside the vault

#### Scenario: No-match question SHALL NOT trigger generic implementation walkthrough

- **WHEN** a backend-only vault is asked `how does dark mode and theme switching work` AND no `wiki/` or `raw/code/` content covers dark mode or theming
- **THEN** the chat agent response SHALL state the vault does not cover dark mode AND SHALL NOT contain a numbered or bulleted implementation walkthrough listing items like `frontend state management`, `CSS or theme variables`, `persistence layer`, `provider or context`, or `system preference detection` when those concepts are not present in the vault

##### Example: No-match answer shape

- **GIVEN** a chat session against a vault whose `raw/code/` contains only `src/auth.py` and `src/db.py` and whose `wiki/` does not mention dark mode or theming
- **WHEN** the user sends `how does dark mode and theme switching work`
- **THEN** the response SHALL contain a short statement that the vault does not currently cover dark mode (or equivalent wording in the user language) AND the response SHALL NOT include a list or paragraph describing how dark mode could be implemented in general (no frontend-state, CSS-variables, persistence, or provider-context discussion)
- **AND** the response MAY suggest pointing the user at the vault folders that do exist (for example by naming the `wiki/modules/` folder) only if those suggestions are grounded in retrieved content

#### Scenario: Partial-match question is distinct from no-match

- **WHEN** the user asks about a topic AND retrieval returns at least one relevant `wiki/` page or `raw/code/` file
- **THEN** the chat agent SHALL answer normally using the retrieved content AND the no-match clause SHALL NOT trigger AND the agent MAY include further detail grounded in the retrieved content

<!-- @trace
source: prompt-surface-output-discipline-batch
updated: 2026-05-24
code:
  - codebus-core/src/skill_bundle/mod.rs
tests:
  - codebus-core/tests/vault_init.rs
-->
