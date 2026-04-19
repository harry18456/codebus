## ADDED Requirements

### Requirement: LLMProvider protocol

The sidecar SHALL define an `LLMProvider` Protocol exposing `chat` and `embed` methods, per `docs/decisions.md` D-012 and `docs/llm-provider.md`.

#### Scenario: Protocol methods present

- **WHEN** `LLMProvider` is imported
- **THEN** it MUST declare a `chat(messages, response_model)` method and an `embed(texts)` method

#### Scenario: Protocol is checkable at type level

- **WHEN** a concrete class that implements both methods is checked against `LLMProvider`
- **THEN** static type analysis MUST accept it as an `LLMProvider` subtype

### Requirement: Mock provider returns Instructor-compatible output

The sidecar SHALL ship a `MockProvider` implementation whose `chat` method produces values parsed by Instructor and Pydantic through the real code path, per design decision D-local-4.

#### Scenario: Mock chat satisfies response_model

- **WHEN** `MockProvider.chat(messages=[...], response_model=SomeBaseModel)` is called and no script is provided
- **THEN** the return value MUST be an instance of `SomeBaseModel` and MUST pass Pydantic validation

#### Scenario: Mock script controls output

- **WHEN** `MockProvider` is constructed with a `MockScript` that pins the next `chat` output to a specific payload
- **THEN** the subsequent `chat` call MUST return the pinned payload and MUST consume one script entry

#### Scenario: Mock embed returns deterministic vector

- **WHEN** `MockProvider.embed(texts=["hello"])` is called twice with the same input
- **THEN** both calls MUST return the same vector, enabling cache-key tests downstream

### Requirement: No outbound LLM traffic during M1

While this change is active, the sidecar SHALL NOT send any network request to an external LLM provider. This enforces the M1 invariant recorded in the proposal.

#### Scenario: Only MockProvider registered

- **WHEN** the sidecar process starts for M1 development
- **THEN** the provider registry MUST expose only `MockProvider` and MUST NOT instantiate any provider that performs outbound HTTP to OpenAI, Anthropic, Gemini, or Ollama

#### Scenario: Integration test asserts no outbound calls

- **WHEN** the M1 integration test suite runs
- **THEN** it MUST assert that no outbound HTTP request leaves the sidecar process during any test, using a network-interception fixture
