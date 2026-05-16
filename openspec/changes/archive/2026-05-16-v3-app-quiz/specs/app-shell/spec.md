## MODIFIED Requirements

### Requirement: AppConfig Namespace Isolation

The system SHALL maintain an `app.*` namespace inside `~/.codebus/config.yaml`. After this change the namespace SHALL contain only `app.quiz.pass_threshold` (integer, 50–100, default 80). The `app.quiz.default_length` key SHALL NO LONGER live in `app.*`; the default quiz length is relocated to the shared `quiz.default_length` key defined by the `quiz` capability's Shared Quiz Config Namespace requirement. The codebus CLI binaries (`init`, `goal`, `query`, `lint`, `fix`, `quiz`) SHALL NOT read, write, or otherwise depend on the `app.*` namespace; the `codebus quiz` subcommand obtains its question count from the shared `quiz.*` namespace, never from `app.*`.

#### Scenario: CLI ignores app namespace

- **WHEN** any codebus CLI verb (including `quiz`) runs against a `~/.codebus/config.yaml` containing the `app.*` namespace
- **THEN** the CLI executes normally with no warnings about `app.*` and no modification to `app.*` values

#### Scenario: App reads pass_threshold default

- **WHEN** the app loads global config and `app.quiz.pass_threshold` is absent from the YAML
- **THEN** the loaded `GlobalConfig` returns `app.quiz.pass_threshold = 80` (default)

#### Scenario: default_length no longer read from app namespace

- **GIVEN** a `~/.codebus/config.yaml` that still contains a stale `app.quiz.default_length: 7` from a prior version
- **WHEN** the app resolves the default quiz length
- **THEN** the value SHALL be sourced from the shared `quiz.default_length` key (or its default of 5 when that shared key is absent) AND the stale `app.quiz.default_length` SHALL NOT be the source of truth
