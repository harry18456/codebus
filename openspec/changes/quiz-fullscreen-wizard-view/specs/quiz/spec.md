## ADDED Requirements

### Requirement: Quiz Scope Plan Bucket Taxonomy

The quiz scope-planning step SHALL present its planned scope to the user as a checklist grouped by the Karpathy five-bucket taxonomy. The bucket identifiers SHALL be exactly the five literal strings `concepts`, `entities`, `modules`, `processes`, and `synthesis` (no more, no fewer). These identifiers SHALL NOT be translated; they remain in English in every locale, including in URL state, store payloads, IPC payloads, source code constants, and i18n bundle values for any keys whose value IS the identifier itself. Human-readable bucket header labels (the surrounding UI prose, for example the section heading "Modules" or its locale-specific casing) SHALL be sourced from the application i18n system and MAY be localized; the underlying identifier-value-typed strings SHALL remain English. The user SHALL be able to deselect any individual bucket before confirming the scope; deselecting a bucket SHALL exclude that bucket from the generate spawn payload and SHALL NOT spawn an agent. The user SHALL also be able to return from the scope-confirm step to the topic-input step, which SHALL discard the planned buckets and SHALL NOT spawn an agent.

The bucket display order in the scope-confirm step SHALL be `modules`, `processes`, `synthesis`, `concepts`, `entities` (matching the order established by the wiki tree taxonomy source — `codebus-app/design-handoff/walkthrough-decisions.html` § Wiki tree); deviating display orders are not permitted without a follow-up change.

#### Scenario: Scope confirm shows five-bucket checklist

- **WHEN** the plan spawn emits a planned scope and the wizard transitions to the scope-confirm step
- **THEN** the UI SHALL render a checklist grouped by exactly the five bucket identifiers `modules`, `processes`, `synthesis`, `concepts`, `entities` in that order AND each bucket whose plan produced no entries SHALL be rendered with an empty state (it SHALL NOT be hidden) AND each bucket header label SHALL be sourced from the application i18n system

#### Scenario: Bucket identifiers are not translated

- **GIVEN** the application locale is set to `zh-tw`
- **WHEN** the scope-confirm step renders the five-bucket checklist
- **THEN** the bucket identifiers used in URL state, store payloads, and any i18n value typed as an identifier SHALL be the English strings `concepts`, `entities`, `modules`, `processes`, `synthesis` AND the surrounding human-readable bucket header prose MAY be localized but the identifier-typed strings SHALL remain English

#### Scenario: Deselecting a bucket excludes it from the generate payload

- **GIVEN** the scope-confirm step is shown with buckets `concepts`, `entities`, `modules`, `processes`, `synthesis` selected
- **WHEN** the user deselects the `processes` bucket and confirms
- **THEN** the generate spawn payload SHALL exclude any pages that were planned only under `processes` AND no extra agent spawn SHALL be issued by the deselection itself

#### Scenario: Back from scope-confirm to topic discards buckets

- **GIVEN** the scope-confirm step holds a planned five-bucket set
- **WHEN** the user activates the back-to-topic control
- **THEN** the wizard SHALL return to the topic-input step AND the planned buckets SHALL be discarded AND no agent spawn SHALL be issued by the navigation itself
