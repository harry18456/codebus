## MODIFIED Requirements

### Requirement: Verb Library Module Surface

The system SHALL expose a public module `codebus_core::verb` containing five sub-modules `goal`, `query`, `fix`, `chat`, and `quiz`. The `goal`, `query`, `fix`, and `chat` sub-modules SHALL each export exactly one public orchestration function (`run_goal`, `run_query`, `run_fix`, `run_chat_turn`) plus the verb-specific options and report structs. The `quiz` sub-module SHALL export exactly **two** public orchestration functions — `run_quiz_plan` and `run_quiz_generate` — plus its option/report structs; this is the documented exception to the one-function rule, required because the GUI confirm gate (`app-workspace` Quiz Tab Plan-Confirm-Generate Flow, design D1) demands the plan and generate spawns be separately invokable and a single connected call cannot pause mid-flight for an asynchronous confirmation. The `codebus_core::verb` parent module SHALL also export the cross-verb types `VerbEvent`, `VerbLifecycleEvent`, and `VerbError`. No other public surface SHALL be exposed under `codebus_core::verb` by this change. The `codebus_core::vault::init::run_init` function defined by foundation SHALL remain in its existing location and SHALL NOT be moved into `codebus_core::verb`.

#### Scenario: Verb library module path exists

- **WHEN** a downstream crate (codebus-cli or codebus-app) writes `use codebus_core::verb::{goal, query, fix, chat, quiz};`
- **THEN** the compilation SHALL succeed AND the five sub-modules SHALL resolve to public modules (goal/query/fix/chat each exporting one orchestration function; quiz exporting `run_quiz_plan` + `run_quiz_generate`)

#### Scenario: Init verb is not moved

- **WHEN** a downstream crate writes `use codebus_core::verb::init;`
- **THEN** the compilation SHALL fail (no such module) AND init orchestration SHALL remain accessible only via `codebus_core::vault::init::run_init`

#### Scenario: Chat sub-module exports run_chat_turn

- **WHEN** a downstream crate writes `use codebus_core::verb::chat::{run_chat_turn, ChatTurnOptions, ChatTurnReport, CHAT_TOOLSET};`
- **THEN** the compilation SHALL succeed AND `run_chat_turn` SHALL resolve to a function with the signature defined by the `chat-verb` capability

#### Scenario: Quiz sub-module exports plan and generate functions

- **WHEN** a downstream crate writes `use codebus_core::verb::quiz::{run_quiz_plan, run_quiz_generate, QuizPlanOptions, QuizGenerateOptions, QuizPlanOutcome, QuizReport};`
- **THEN** the compilation SHALL succeed AND `run_quiz_plan` / `run_quiz_generate` SHALL resolve to functions with the signatures defined by the `quiz` capability
