## MODIFIED Requirements

### Requirement: ToolContext carries workspace type discriminator

The sidecar SHALL define a `ToolContext` Pydantic model that includes a `workspace_type` field typed as `Literal["folder", "topic"]`, per `docs/decisions.md` D-002 and D-023.

The model SHALL additionally expose two optional dependency slots — `kb: KnowledgeBase | None = None` and `usage_tracker: UsageTracker | None = None` — so Explorer-mode tools (`explorer-tools-p0`) can reach the KB client for `search` dispatch and wire `UsageTracker` into any future tool that itself consumes LLM budget. Both fields default to `None`, preserving backward compatibility with every prior construction site (M1 red-team fixtures, scanner Pass 1 orchestration, sanitizer safety-chain tests).

`ToolContext` SHALL keep `frozen=True` so tools cannot silently relocate the workspace or swap dependencies mid-run by mutating the context.

#### Scenario: Folder workspace accepted

- **WHEN** a `ToolContext` is constructed with `workspace_type="folder"`
- **THEN** the model MUST validate without raising

#### Scenario: Topic workspace accepted at schema level

- **WHEN** a `ToolContext` is constructed with `workspace_type="topic"`
- **THEN** the model MUST validate without raising, even though topic-mode tool behavior is not yet implemented

#### Scenario: Invalid workspace type rejected

- **WHEN** a `ToolContext` is constructed with any string other than `"folder"` or `"topic"`
- **THEN** Pydantic MUST raise a validation error

#### Scenario: Optional kb and usage_tracker fields default to None

- **WHEN** a `ToolContext` is constructed without supplying `kb` or `usage_tracker`
- **THEN** `ctx.kb` MUST be `None` and `ctx.usage_tracker` MUST be `None`
- **AND** the model MUST validate without raising
- **AND** existing red-team fixtures and scanner constructors MUST continue to compile without modification

#### Scenario: kb and usage_tracker fields accept their typed values

- **WHEN** a `ToolContext` is constructed with `kb=<KnowledgeBase instance>` and `usage_tracker=<UsageTracker instance>`
- **THEN** the model MUST validate without raising
- **AND** `ctx.kb` and `ctx.usage_tracker` MUST expose the supplied instances
