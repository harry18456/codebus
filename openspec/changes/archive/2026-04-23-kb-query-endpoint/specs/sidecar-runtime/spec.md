## ADDED Requirements

### Requirement: KB query endpoint registration

The sidecar's `create_app` SHALL register the `POST /kb/query` route on the same `kb_router` already used for `POST /kb/build`, behind the same bearer-token middleware. The endpoint MUST resolve its dependencies from `app.state` exactly like `POST /kb/build` does, but SHALL read `app.state.kb_query_provider` (the query-flavored TrackedProvider factory tagged with `default_module="kb_query"`) instead of `app.state.kb_provider` (the build-flavored factory tagged with `default_module="kb_build"`). This separation lets `token_usage.jsonl` distinguish embedding cost spent on building the KB versus querying it, without per-call `module=` plumbing in the endpoint.

#### Scenario: Both KB build and KB query slots present after wiring

- **WHEN** `create_app(...)` returns with `CODEBUS_OPENAI_API_KEY` set
- **THEN** `app.state.kb_provider` MUST be a callable factory and `app.state.kb_query_provider` MUST be a separate callable factory; invoking each with the same `workspace_root` MUST return distinct `TrackedProvider` instances whose `_default_module` values are `"kb_build"` and `"kb_query"` respectively

#### Scenario: Missing OpenAI API key leaves both provider slots None

- **WHEN** the sidecar starts without `CODEBUS_OPENAI_API_KEY`
- **THEN** both `app.state.kb_provider` and `app.state.kb_query_provider` MUST be `None`, and the sidecar MUST start successfully (the existing graceful-degrade contract)

#### Scenario: Bearer middleware blocks unauthenticated KB query

- **WHEN** a `POST /kb/query` request arrives without an `Authorization` header
- **THEN** the bearer middleware MUST short-circuit with `401` before the endpoint handler runs, mirroring the behavior verified for `POST /kb/build`
