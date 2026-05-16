---
title: Login Flow
type: process
sources:
  - path: src/auth.py
  - path: src/user_store.py
goals:
  - explain web auth components
related:
  - auth-middleware
  - jwt-token-lifecycle
  - user-store
created: '2026-05-15'
updated: '2026-05-15'
stale: false
---

# Login Flow

The sequence of steps that turns an email + password submission into a
usable JWT.

## Sequence

1. Client submits `POST /login` with `{email, password}`.
2. The login handler calls `get_by_email(email)` on the [[user-store]].
   If no record matches, respond `401 Unauthorized` (do not reveal
   whether the email exists).
3. The handler calls `verify_password(record, password)`. On mismatch,
   respond `401 Unauthorized`.
4. On success, the handler calls `issue_token(record.id)`. See
   [[jwt-token-lifecycle]] for the payload shape.
5. The token is returned to the client either:
   - In the JSON response body (`{"token": "..."}`) for API clients, or
   - As a `Set-Cookie` header for browser clients.

## Why login does not pass through auth-middleware

The [[auth-middleware]] requires a valid JWT in the `Authorization`
header. The login route is the entry point that ISSUES the first token,
so it cannot require a token as input. The router excludes `/login`
from the middleware chain.

## Subsequent requests

Once the client has the token, every subsequent request travels through
[[auth-middleware]] which verifies the token before any handler runs.
