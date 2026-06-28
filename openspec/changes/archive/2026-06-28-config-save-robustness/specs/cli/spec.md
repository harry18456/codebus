## ADDED Requirements

### Requirement: Global Config Starter Content Shape

The global config starter content written by the `write_starter_config_if_missing` primitive SHALL consist of a single shared header comment block followed by pure default key/value pairs. The starter SHALL NOT carry inline per-field teaching comments. The header SHALL be a short comment block (at most a few lines) that points the reader to the field reference documentation instead of embedding field documentation in the config file itself.

The header SHALL be defined as a single source-of-truth constant named `CONFIG_HEADER`, exported from codebus-core, so the exact same header text is reused by the app's config save path (see the `app-shell` capability's Config Save Hygiene requirement). Field-level teaching that previously lived in inline comments SHALL be migrated to a dedicated documentation file at `docs/config-reference.md`.

Removing inline comments SHALL NOT change any default value: the starter body SHALL continue to round-trip through every section loader to that section's `Default::default()`.

#### Scenario: Starter begins with the shared header and omits inline field comments

- **WHEN** `write_starter_config_if_missing` writes a new starter file because none exists
- **THEN** the written content SHALL begin with the shared `CONFIG_HEADER` block AND SHALL NOT contain inline per-field teaching comments beyond that header

#### Scenario: Starter still round-trips to defaults

- **WHEN** the starter content is loaded back through the PII, agent, hooks, lint, and log section loaders
- **THEN** each loaded section SHALL equal its `Default::default()` value
