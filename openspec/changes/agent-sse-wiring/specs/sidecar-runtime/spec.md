## MODIFIED Requirements

### Requirement: task_id format

Task identifiers SHALL follow the format `{kind}_{rand}` where `kind` is one of the lowercase strings `"scan"`, `"kb"`, or `"explore"` (extensible by future capabilities) and `rand` is exactly eight lowercase hexadecimal characters generated from a cryptographic random source (e.g. `secrets.token_hex(4)`). Identifiers MUST match the regular expression `^(scan|kb|explore)_[0-9a-f]{8}$` for tasks created within scope of the `sse-progress-skeleton` change AND the `agent-sse-wiring` change that introduces the `explore` kind.

#### Scenario: Generated id matches required regex

- **WHEN** the sidecar creates a scan task identifier
- **THEN** the resulting `task_id` MUST match `^scan_[0-9a-f]{8}$`

#### Scenario: Explore kind follows same shape

- **WHEN** the sidecar creates an explore task identifier
- **THEN** the resulting `task_id` MUST match `^explore_[0-9a-f]{8}$`
- **AND** the `TaskRegistry` single-slot enforcement MUST apply equally — an in-flight `explore` task MUST block subsequent `scan` / `kb` / `explore` creations with `409 TASK_IN_FLIGHT`
