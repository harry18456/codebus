## ADDED Requirements

### Requirement: AuthorizationAuditLogger is the sole writer for the App-level audit log

The CodeBus sidecar SHALL persist authorization events to a single App-level append-only JSONL file at `~/.codebus/authorization_audit.jsonl`. The file path SHALL be sourced exclusively from `sidecar/src/codebus_agent/auth/paths.py` constants (`_APP_AUDIT_HOME_SUBDIR = ".codebus"`, `_AUTHORIZATION_AUDIT_FILENAME = "authorization_audit.jsonl"`); no other module SHALL contain the literal string `"authorization_audit.jsonl"` in source code.

The single writer class SHALL be `AuthorizationAuditLogger`, exposing exactly three typed methods (`write_grant_issued`, `write_grant_denied`, `write_grant_revoked`) corresponding to the three event kinds. The logger constructor SHALL `mkdir(parents=True, exist_ok=True)` the parent directory and SHALL accept only an absolute path (relative paths MUST raise `ValueError` at construction). All sidecar code paths that need to record an authorization event SHALL go through this logger; direct `open(..., "a")` writes against the audit path are forbidden.

The audit log MUST NOT contain any bearer token, OpenAI API key, raw file contents, or sanitizer pre-replacement text. Each line MUST be a single self-contained JSON object terminated by `\n`.

#### Scenario: Filename literal is single-sourced in canonical leaf module

- **WHEN** `sidecar/src/codebus_agent/` is grepped for the literal string `authorization_audit.jsonl` (case-sensitive)
- **THEN** exactly one match MUST be returned, located in `auth/paths.py`
- **AND** all other sidecar modules referring to the path MUST import `_AUTHORIZATION_AUDIT_FILENAME` from that module

#### Scenario: Logger constructor auto-creates parent directory

- **WHEN** `AuthorizationAuditLogger(path=Path("/nonexistent/.codebus/authorization_audit.jsonl"))` is instantiated and the parent `.codebus/` directory does not yet exist
- **THEN** the directory MUST be created with `mkdir(parents=True, exist_ok=True)` before the first write
- **AND** the constructor MUST NOT raise when the directory already exists

#### Scenario: Relative path raises at construction

- **WHEN** `AuthorizationAuditLogger(path=Path("authorization_audit.jsonl"))` (relative) is instantiated
- **THEN** the constructor MUST raise `ValueError` with a message naming the path
- **AND** no file system side effect MUST occur (the parent directory MUST NOT be created if it did not already exist)

#### Scenario: Each event method writes exactly one JSONL line

- **WHEN** any of `write_grant_issued`, `write_grant_denied`, or `write_grant_revoked` is invoked
- **THEN** the file MUST gain exactly one new line with valid JSON parseable by `json.loads`
- **AND** the line MUST be terminated by exactly one `\n` (no `\r\n`, no double newline)

#### Scenario: Direct file open against the audit path is rejected by review

- **WHEN** any sidecar source file opens the path returned by `auth.paths.authorization_audit_path()` directly (via `open()`, `Path.open()`, `aiofiles.open()`, etc.) for writing
- **THEN** code review MUST flag it as an invariant violation
- **AND** the only legitimate writer MUST be `AuthorizationAuditLogger` instance methods

### Requirement: Three-event audit schema with workspace_type discriminator

The audit log SHALL record exactly three event kinds: `grant_issued`, `grant_denied`, and `grant_revoked`. Each event line MUST include the discriminator field `event` with the matching string value. The discriminator field `workspace_type` MUST be present from day 1 in every `grant_issued` and `grant_denied` event with values restricted to the closed set `{"folder", "topic"}`; MVP supports `"folder"` only, but the schema slot MUST exist day 1 to avoid breaking the audit log when Phase 2 adds Topic mode.

**`grant_issued` MUST include**: `ts` (ISO 8601 UTC), `event` (`"grant_issued"`), `session_id` (sidecar-generated UUIDv4 string), `workspace_id` (path-derived stable id; see scenario), `workspace_type`, `workspace_source` (object whose shape depends on `workspace_type`: for `"folder"` it MUST be `{"path": "<absolute path>"}`), `scenario` (one of `"first_run"` / `"scope_reconfirm"` / `"scope_upgrade_new_kind"`), `scope` (object with `llm_provider: str`, `llm_model: str`, `outbound_endpoint: str`), `sanitizer_rules_version` (verbatim copy of `codebus_agent.sanitizer.RULES_VERSION` at grant time, opaque string format), and `user_ack` (list of acknowledgement flag strings).

**`grant_denied` MUST include**: `ts`, `event` (`"grant_denied"`), `session_id`, `workspace_type`, `workspace_source`, `scenario`, and `reason` (closed set `{"user_cancelled", "app_closed"}`; `"dialog_dismissed"` deferred to a later change).

**`grant_revoked` MUST include**: `ts`, `event` (`"grant_revoked"`), `session_id`, `workspace_id`, `grant_ts` (ISO 8601 UTC of the revoked grant), and `trigger` (P0 closed set `{"settings_revoke"}`; the values `"rules_version_bump"`, `"provider_change"`, and `"workspace_deleted"` are deferred to P1).

The `workspace_id` field MUST be a sidecar-derived stable identifier: SHA-256 of the resolved canonical path as POSIX-form lowercase string, prefix `"ws_"`, take first 12 hex characters. Same path MUST yield same `workspace_id` across sidecar restarts.

The `session_id` field MUST be a sidecar-generated UUIDv4 string (decimal canonical form, e.g. `"01928b1f-7c3a-4f9e-9c8b-a1b2c3d4e5f6"`). The sidecar MUST NOT reuse bearer token text or any portion thereof as session_id.

The `user_ack` list MUST contain at minimum the three base flags `"raw_stays_local"`, `"no_kb_persist"`, and `"outbound_to_<provider>"` (where `<provider>` is the `scope.llm_provider` value). The `scope_upgrade_new_kind` scenario MUST additionally include one `"new_kind:<kind>"` flag per newly-introduced sanitizer kind that the user explicitly acknowledged.

#### Scenario: workspace_id is path-derived and stable

- **WHEN** `auth.service.workspace_id_for_path(Path("/Users/x/projects/timeline"))` is invoked twice with the same canonical path
- **THEN** both calls MUST return the identical 15-character string starting with `"ws_"` followed by 12 lowercase hex characters
- **AND** invoking with a path that differs only in case on Windows (e.g., `C:\Projects\Timeline` vs `C:\projects\timeline`) MUST still produce the same `workspace_id`

#### Scenario: session_id is fresh UUIDv4 per grant

- **WHEN** two successive `grant_issued` events are recorded for the same `workspace_id`
- **THEN** the two `session_id` values MUST differ
- **AND** each session_id MUST be a valid UUIDv4 (parseable by `uuid.UUID(value, version=4)`)

#### Scenario: workspace_type folder requires path in workspace_source

- **WHEN** a `grant_issued` event is written with `workspace_type="folder"`
- **THEN** the `workspace_source` field MUST be an object containing exactly one key `"path"` whose value is an absolute path string
- **AND** the path MUST exist and be a directory at the moment of writing (verified by the `POST /auth/grant` handler)

#### Scenario: workspace_type topic returns 501 in MVP but schema slot exists

- **WHEN** a `POST /auth/grant` request arrives with `workspace_type="topic"`
- **THEN** the handler MUST return HTTP 501 with `{"detail": {"code": "AUTH_INVALID_REQUEST", "message": "topic mode reserved for Phase 2"}}`
- **AND** no audit event MUST be recorded
- **AND** the Pydantic request model MUST still accept `workspace_type="topic"` at validation time (the 501 is a handler-level decision, not a schema rejection)

#### Scenario: sanitizer_rules_version is verbatim from sanitizer module

- **WHEN** a `grant_issued` event is written
- **THEN** the `sanitizer_rules_version` field MUST equal exactly the value of `codebus_agent.sanitizer.RULES_VERSION` at the time of the call
- **AND** the sidecar MUST NOT parse, transform, or compare this version string in P0 — it is opaque metadata recorded for audit trail and future P1 trigger logic

#### Scenario: user_ack list contains the three base flags plus per-new-kind flags

- **WHEN** a `grant_issued` event is written for scenario `"first_run"` with provider `"anthropic"`
- **THEN** `user_ack` MUST contain exactly `["raw_stays_local", "no_kb_persist", "outbound_to_anthropic"]` in any order
- **WHEN** the same event is written for scenario `"scope_upgrade_new_kind"` with two newly-acked kinds `secret` and `email`
- **THEN** `user_ack` MUST additionally contain `"new_kind:secret"` and `"new_kind:email"` (giving five total flags)

### Requirement: Four sync sidecar endpoints under bearer middleware

The sidecar SHALL expose exactly four authorization endpoints, all mounted under the bearer authentication middleware and all returning JSON synchronously without using the SSE task channel: `POST /auth/grant`, `POST /auth/deny`, `POST /auth/revoke`, and `GET /auth/status`. None of these endpoints SHALL spawn a background task; the `task_id` regex MUST NOT be extended to include an `"auth"` prefix.

**`POST /auth/grant`** SHALL accept a Pydantic `GrantRequest` body containing `workspace_type` (Literal `"folder"` | `"topic"`), `workspace_source` (discriminated union by `workspace_type`), `scenario` (Literal of three values), `scope` (object with provider/model/endpoint), `sanitizer_rules_version` (string, MUST equal current `RULES_VERSION` or the handler returns 400), and `user_ack` (list of flag strings). On success: validate `workspace_root` is an existing directory (else 400 `AUTH_WORKSPACE_INVALID`), generate `session_id`, derive `workspace_id`, write `grant_issued` event, register session in in-memory dict, return `200 {"session_id": str, "workspace_id": str, "granted_at": str}`. On invalid request: return 400 with `auth/errors.py` code constants in the `detail.code` field.

**`POST /auth/deny`** SHALL accept a Pydantic `DenyRequest` containing `workspace_type`, `workspace_source`, `scenario`, and `reason` (Literal `"user_cancelled"` | `"app_closed"`). The handler writes a `grant_denied` event and returns `204 No Content`. No session is created.

**`POST /auth/revoke`** SHALL accept `RevokeRequest` containing `session_id` and `trigger` (P0 Literal restricted to `"settings_revoke"`). On success: look up session in in-memory dict (else 404 `AUTH_NO_ACTIVE_GRANT`), find original `grant_issued` event in audit log to get `grant_ts`, write `grant_revoked` event, remove session from in-memory dict, return `204 No Content`.

**`GET /auth/status`** SHALL accept `workspace_id` as a query parameter. The handler MUST return `200 AuthStatusResponse` containing `has_active_grant: bool` (whether the session corresponding to last `grant_issued` for this `workspace_id` is still in the in-memory dict), `session_id: str | None`, `last_grant: GrantSnapshot | None` (the most recent `grant_issued` payload for this workspace_id from audit log, or `None` if never granted), and `current_rules_version: str` (the value of `RULES_VERSION` at request time). Reading the audit log on every request is the canonical implementation; no in-memory cache MUST be introduced in P0.

The HTTP error code constants `AUTH_WORKSPACE_INVALID`, `AUTH_NO_ACTIVE_GRANT`, and `AUTH_INVALID_REQUEST` SHALL live in `sidecar/src/codebus_agent/auth/errors.py` as module-level string constants. They MUST NOT be added to `sidecar/src/codebus_agent/api/tasks.py::ERROR_CODES` (which is the closed SSE wire-error frozenset). A defensive test SHALL assert the disjoint property between auth HTTP error codes and SSE wire-error codes.

#### Scenario: Bearer middleware enforced on all four endpoints

- **WHEN** any of `POST /auth/grant`, `POST /auth/deny`, `POST /auth/revoke`, or `GET /auth/status` is called without a valid `Authorization: Bearer <token>` header
- **THEN** the response MUST be HTTP 401 with no payload differing from other bearer-protected endpoints
- **AND** no audit event MUST be written

#### Scenario: POST /auth/grant rejects invalid workspace path

- **WHEN** `POST /auth/grant` is called with `workspace_type="folder"` and `workspace_source.path` pointing to a path that does not exist or is a regular file
- **THEN** the response MUST be HTTP 400 with body `{"detail": {"code": "AUTH_WORKSPACE_INVALID", "message": "<safe>"}}`
- **AND** no audit event MUST be written
- **AND** no in-memory session MUST be created

#### Scenario: POST /auth/grant on success returns session_id and writes audit

- **WHEN** `POST /auth/grant` is called with a valid request body and an existing workspace directory
- **THEN** the response MUST be HTTP 200 with body containing keys `session_id`, `workspace_id`, and `granted_at`
- **AND** exactly one new line MUST be appended to `~/.codebus/authorization_audit.jsonl`
- **AND** the new line MUST be a `grant_issued` event matching the request, with the response's `session_id` and `workspace_id` echoed in the audit line

#### Scenario: POST /auth/deny writes audit and creates no session

- **WHEN** `POST /auth/deny` is called with a valid request body
- **THEN** the response MUST be HTTP 204 No Content
- **AND** exactly one new line MUST be appended to the audit log as a `grant_denied` event
- **AND** the in-memory session dict MUST NOT gain any entry

#### Scenario: POST /auth/revoke without active session returns 404

- **WHEN** `POST /auth/revoke` is called with a `session_id` that is not present in the in-memory session dict
- **THEN** the response MUST be HTTP 404 with body `{"detail": {"code": "AUTH_NO_ACTIVE_GRANT", "message": "<safe>"}}`
- **AND** no audit event MUST be written

#### Scenario: GET /auth/status reads audit log fresh on each call

- **WHEN** `GET /auth/status?workspace_id=ws_abc123def456` is called
- **THEN** the handler MUST scan `~/.codebus/authorization_audit.jsonl` to find the latest `grant_issued` event matching `workspace_id`
- **AND** the response MUST include `last_grant` populated from that audit line, or `None` if no matching event exists
- **AND** no in-memory cache of audit log entries MUST be introduced

#### Scenario: Auth HTTP error codes disjoint from SSE wire-error codes

- **WHEN** the test suite imports both `auth.errors.AUTH_WORKSPACE_INVALID` / `AUTH_NO_ACTIVE_GRANT` / `AUTH_INVALID_REQUEST` and `api.tasks.ERROR_CODES`
- **THEN** the intersection of the auth code set and the SSE frozenset MUST be empty
- **AND** the SSE `ERROR_CODES` frozenset MUST remain the closed set of ten codes defined by `sidecar-runtime` `Background task error containment`

### Requirement: O-01 Authorization Modal supports three P0 scenarios with a shared Vue component

The O-01 Authorization Modal SHALL be implemented as a single reusable Vue component `web/app/components/auth/AuthorizationModal.vue` whose visual variant is selected by an `activeScenario` prop matching the closed set `{"first_run", "scope_reconfirm", "scope_upgrade_new_kind"}`. The component MUST NOT render content outside this closed set; passing an unrecognised scenario MUST be a TypeScript compile-time error.

The modal SHALL expose the following props (TypeScript interface):

```typescript
interface AuthorizationModalProps {
  activeScenario: 'first_run' | 'scope_reconfirm' | 'scope_upgrade_new_kind'
  workspacePath: string
  fileCount: number
  dominantLanguages: string[]
  sanitizeKindCounts: Record<string, number>  // e.g. {secret: 12, email: 47}
  llmProvider: string
  llmModel: string
  newKinds?: string[]  // required when activeScenario == 'scope_upgrade_new_kind'
}
```

The submit button (CTA `"授權並開始"`) MUST be disabled until all three base ack checkboxes (`raw_stays_local`, `no_kb_persist`, `outbound_to_<provider>`) are checked. For `scope_upgrade_new_kind`, every new kind in `newKinds` MUST also have a corresponding ack checkbox checked before the submit button enables.

The cancel button (`"先不啟用此 workspace"`) MUST trigger a `POST /auth/deny` call via `useSidecar().deny(...)` and emit a `denied` event so the parent route can navigate back to the workspace selection page. The modal MUST NOT remain mounted on the page after deny — it is the route's responsibility to dismount.

The modal MUST NOT bypass `useSidecar()` to call any `/auth/*` endpoint directly. The four typed wrappers (`grant`, `deny`, `revoke`, `status`) on `useSidecar()` are the only legitimate IPC entry points for this component.

`web/app/composables/useAuthorization.ts` SHALL be a separate composable that owns the modal **flow state** (current scenario, ack flags, submit-enabled boolean, deferred error). It SHALL NOT duplicate `useSidecar()`'s bearer/baseUrl exposure; modal flow state and IPC concerns are separated.

#### Scenario: TypeScript rejects unrecognised scenario value

- **WHEN** a parent component instantiates `<AuthorizationModal :active-scenario="'rules_version_bump'" .../>`
- **THEN** TypeScript MUST report a type error at compile time (the literal type does not include `"rules_version_bump"`)
- **AND** `npm run typecheck` MUST fail with the offending file path

#### Scenario: Submit button disabled until all base acks checked

- **WHEN** `<AuthorizationModal :active-scenario="'first_run'" :llm-provider="'anthropic'" .../>` is mounted with no ack checkboxes ticked
- **THEN** the submit button MUST be rendered with the `disabled` attribute set to `true`
- **WHEN** all three base ack checkboxes (`raw_stays_local`, `no_kb_persist`, `outbound_to_anthropic`) are ticked
- **THEN** the submit button MUST become enabled
- **AND** unticking any single base ack MUST disable it again

#### Scenario: scope_upgrade_new_kind requires per-kind acks

- **WHEN** `<AuthorizationModal :active-scenario="'scope_upgrade_new_kind'" :new-kinds="['secret']" .../>` is mounted with all three base acks ticked but the `new_kind:secret` ack unticked
- **THEN** the submit button MUST remain disabled
- **WHEN** the `new_kind:secret` ack checkbox is also ticked
- **THEN** the submit button MUST become enabled

#### Scenario: Modal calls useSidecar().deny on cancel

- **WHEN** the user clicks the cancel button in `<AuthorizationModal />`
- **THEN** the component MUST call `useSidecar().deny({...})` exactly once with a request body matching the current scenario
- **AND** the component MUST emit a `denied` Vue event after the deny call resolves (or rejects)
- **AND** the component MUST NOT itself navigate; navigation is the route's concern

### Requirement: scope upgrade detection reads the latest grant from audit log

When `POST /auth/grant` is called with `scenario="scope_upgrade_new_kind"`, the sidecar handler SHALL verify the claim by reading `~/.codebus/authorization_audit.jsonl`, finding the most recent `grant_issued` line matching the same `workspace_id`, extracting all flags from that entry's `user_ack` list that start with the prefix `"new_kind:"`, computing `acked_kinds = {flag.removeprefix("new_kind:") for flag in user_ack if flag.startswith("new_kind:")}`, and confirming that the current request's claimed `new_kinds` (the kinds whose `new_kind:<kind>` flags are present in this request's `user_ack` but NOT in `acked_kinds`) is non-empty. If no prior `grant_issued` exists for this workspace_id and the scenario is `scope_upgrade_new_kind`, the handler MUST return 400 `AUTH_INVALID_REQUEST` with a message indicating no prior grant was found.

For the `scenario="first_run"` value, the handler MUST verify no prior `grant_issued` for this workspace_id exists in the audit log. For `scenario="scope_reconfirm"`, the handler MUST verify a prior `grant_issued` exists AND the current request's `user_ack` does not introduce any new `new_kind:*` flags relative to the prior grant.

The audit log scan SHALL be a linear read (no in-memory index in P0). On a 1000-line audit log on local disk, this completes in well under 100 ms; performance is acceptable for P0.

#### Scenario: first_run rejected when prior grant exists for same workspace

- **WHEN** `POST /auth/grant` is called with `scenario="first_run"` for a `workspace_id` that has at least one prior `grant_issued` entry in the audit log
- **THEN** the response MUST be HTTP 400 with `{"detail": {"code": "AUTH_INVALID_REQUEST", "message": "..."}}`
- **AND** no new audit event MUST be written

#### Scenario: scope_upgrade_new_kind requires non-empty diff

- **WHEN** `POST /auth/grant` is called with `scenario="scope_upgrade_new_kind"` and a `user_ack` whose `new_kind:*` flags are all already present in the latest prior `grant_issued.user_ack` for this workspace_id
- **THEN** the response MUST be HTTP 400 with `{"detail": {"code": "AUTH_INVALID_REQUEST", "message": "..."}}`
- **AND** no new audit event MUST be written

#### Scenario: scope_upgrade_new_kind on workspace with no prior grant rejected

- **WHEN** `POST /auth/grant` is called with `scenario="scope_upgrade_new_kind"` for a `workspace_id` that has no prior `grant_issued` in the audit log
- **THEN** the response MUST be HTTP 400 with `{"detail": {"code": "AUTH_INVALID_REQUEST", "message": "..."}}`

#### Scenario: scope_reconfirm allowed when no new kinds introduced

- **WHEN** `POST /auth/grant` is called with `scenario="scope_reconfirm"` and a `user_ack` whose `new_kind:*` flag set is a subset of the latest prior `grant_issued.user_ack` for this workspace_id
- **THEN** the response MUST be HTTP 200 and a new `grant_issued` event MUST be appended to the audit log
- **AND** the new event's `scenario` field MUST equal `"scope_reconfirm"` (not silently rewritten to `"first_run"`)
