## ADDED Requirements

### Requirement: Source-Signal Detection on Verb Invocation

The system SHALL provide a source-signal drift detection operation that determines whether the raw mirror needs to be re-synced before a verb invocation proceeds. The operation SHALL compare the manifest's persisted `source_signal` (written by init's manifest step) against a freshly-recomputed signal from the same source repository: the `git_head` field (`<repo>/.git/HEAD` verbatim contents or YAML null), the `file_count` field (count of files that would be mirrored under current mirror rules), and the `total_bytes` field (aggregate byte total of the same set). When any of the three fields differs between the persisted signal and the recomputed signal, the operation SHALL report the source as drifted. When all three fields are equal, the operation SHALL report the source as unchanged.

When detection itself cannot complete successfully (manifest file is missing, malformed YAML, or unreadable; git HEAD I/O error; source repository walk failure), the operation SHALL fail-safe and report the source as drifted, ensuring the caller proceeds with a re-sync rather than skipping it.

The detection operation SHALL be invoked by verbs that read or write the raw mirror (currently `goal`); after a re-sync triggered by drift, the system SHALL update the manifest's `source_signal` to reflect the new state.

#### Scenario: Detection reports unchanged when all three signal fields match

- **WHEN** the manifest's `source_signal.git_head`, `source_signal.file_count`, and `source_signal.total_bytes` all equal their respective recomputed values from the current source state
- **THEN** the detection operation SHALL report unchanged AND the caller SHALL skip the raw mirror re-sync

#### Scenario: Detection reports drifted when git_head differs

- **WHEN** the manifest's `source_signal.git_head` is `ref: refs/heads/main\n` AND the current `<repo>/.git/HEAD` content is `ref: refs/heads/feature\n`
- **THEN** the detection operation SHALL report drifted regardless of file_count and total_bytes

#### Scenario: Detection reports drifted when file_count differs

- **WHEN** the manifest's `source_signal.file_count` is 142 AND the recomputed file_count is 143
- **THEN** the detection operation SHALL report drifted

#### Scenario: Detection reports drifted when total_bytes differs

- **WHEN** the manifest's `source_signal.total_bytes` is 89234 AND the recomputed total_bytes is 89890
- **THEN** the detection operation SHALL report drifted

#### Scenario: Detection fail-safe when manifest is missing

- **WHEN** the detection operation is invoked but `<repo>/.codebus/manifest.yaml` does not exist
- **THEN** the operation SHALL report drifted (fail-safe) AND the caller SHALL proceed with a re-sync

#### Scenario: Detection fail-safe when manifest is malformed

- **WHEN** the detection operation is invoked and `<repo>/.codebus/manifest.yaml` cannot be parsed as valid YAML
- **THEN** the operation SHALL report drifted (fail-safe) rather than propagating a parse error

#### Scenario: Re-sync after drift updates the manifest signal

- **WHEN** detection reports drifted, the caller re-runs the raw mirror, and the new mirror state has `file_count=N` and `total_bytes=B`
- **THEN** the manifest's `source_signal.file_count` SHALL equal N AND `source_signal.total_bytes` SHALL equal B AND the manifest's `last_sync_at` SHALL be updated to the current UTC timestamp
