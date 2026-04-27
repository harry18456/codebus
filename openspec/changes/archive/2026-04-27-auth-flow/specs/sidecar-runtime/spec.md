## ADDED Requirements

### Requirement: Authorization endpoints registration

The sidecar SHALL register four synchronous HTTP endpoints under the `/auth/` route prefix from a dedicated router module `sidecar/src/codebus_agent/api/auth.py`. The router SHALL be included in the FastAPI app inside `create_app` after the bearer authentication middleware is installed, mirroring the pattern used by `/scan`, `/kb/*`, `/explore`, `/generate`, and `/qa` routers. The four endpoints (`POST /auth/grant`, `POST /auth/deny`, `POST /auth/revoke`, `GET /auth/status`) MUST be subject to the same bearer enforcement as all other sidecar endpoints; no auth endpoint MAY bypass bearer middleware.

The auth router SHALL NOT use the `TaskRegistry`, SHALL NOT spawn background tasks, and SHALL NOT extend the `task_id` regex. All auth endpoints return their result synchronously in the HTTP response body. Error responses SHALL use HTTP 4xx status codes with body shape `{"detail": {"code": "AUTH_*", "message": "<safe>"}}` where the `code` value comes from `sidecar/src/codebus_agent/auth/errors.py` module-level string constants. The auth error code constants MUST NOT be added to the SSE-channel `ERROR_CODES` frozenset in `api/tasks.py`; the two error code spaces are intentionally disjoint.

The app factory `create_app` SHALL accept a new optional `auth_audit_logger_factory: Callable[[], AuthorizationAuditLogger] | None = None` parameter and SHALL store it on `app.state.auth_audit_logger_factory`. The startup path (`main.py`) SHALL pass a default factory that constructs an `AuthorizationAuditLogger` pointing at the App-level audit log path (`~/.codebus/authorization_audit.jsonl`). When the factory is `None`, the four `/auth/*` endpoints MUST return HTTP 503 with `{"detail": {"code": "AUTH_NOT_CONFIGURED", "message": "..."}}` (the constant `AUTH_NOT_CONFIGURED` is added to `auth/errors.py` alongside the four primary codes); this stays parallel to the `*_NOT_CONFIGURED` pattern used by the KB/Explorer/Generator/QA endpoints.

#### Scenario: Auth router included in app factory

- **WHEN** `create_app(auth_audit_logger_factory=lambda: AuthorizationAuditLogger(...))` is called
- **THEN** the returned FastAPI app MUST have a route registered for each of the four paths: `POST /auth/grant`, `POST /auth/deny`, `POST /auth/revoke`, `GET /auth/status`
- **AND** each route MUST be subject to the bearer authentication dependency
- **AND** `app.state.auth_audit_logger_factory` MUST equal the factory passed in

#### Scenario: Auth endpoints return 503 when factory is None

- **WHEN** `create_app(auth_audit_logger_factory=None)` is called and `POST /auth/grant` is invoked with a valid bearer
- **THEN** the response MUST be HTTP 503 with body `{"detail": {"code": "AUTH_NOT_CONFIGURED", "message": "<safe>"}}`
- **AND** no audit log MUST be written
- **AND** no in-memory session MUST be created

#### Scenario: Auth endpoints reject missing bearer

- **WHEN** `POST /auth/grant` is called without an `Authorization` header
- **THEN** the response MUST be HTTP 401 (matching the bearer middleware behavior for all other endpoints)
- **AND** the response body MUST NOT include any auth-specific code (the response is the generic bearer-missing response, not an auth-flow-specific error)

#### Scenario: task_id regex unchanged

- **WHEN** the sidecar codebase is grepped for the regex literal in `tasks.py` defining the task_id format
- **THEN** the pattern MUST remain `^(scan|kb|explore|generate|qa)_[0-9a-f]{8}$` (no `auth` prefix added)
- **AND** any test asserting the regex MUST NOT be modified by this change

#### Scenario: Auth error codes disjoint from SSE ERROR_CODES frozenset

- **WHEN** the test suite imports `auth.errors.AUTH_WORKSPACE_INVALID`, `auth.errors.AUTH_NO_ACTIVE_GRANT`, `auth.errors.AUTH_INVALID_REQUEST`, `auth.errors.AUTH_NOT_CONFIGURED`, and `api.tasks.ERROR_CODES`
- **THEN** the intersection of the auth code set and the SSE frozenset MUST be empty
- **AND** the SSE `ERROR_CODES` frozenset MUST remain exactly the closed set of ten codes defined by the `Background task error containment` Requirement
