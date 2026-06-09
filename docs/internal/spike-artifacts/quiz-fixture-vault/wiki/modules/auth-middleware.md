---
title: Auth Middleware
type: module
sources:
  - path: src/auth.py
goals:
  - explain web auth components
related:
  - jwt-token-lifecycle
  - login-flow
  - user-store
created: '2026-05-15'
updated: '2026-05-15'
stale: false
---

# Auth Middleware

HTTP middleware that gates every protected route by verifying the JWT
in the `Authorization` header.

## Behavior

The middleware wraps a downstream handler. On each incoming request:

1. Extract the `Authorization` header. Strip the `Bearer ` prefix to
   isolate the raw token.
2. Call `verify_token` (see [[jwt-token-lifecycle]]) to decode and
   validate the signature + expiry.
3. If verification returns `None`, short-circuit with `401 Unauthorized`.
4. Otherwise, attach `request.user_id = claims["sub"]` and call the
   wrapped handler.

## Position in the request flow

```
incoming HTTP request
        │
        ▼
auth_middleware (this module)
        │
        ▼
downstream handler (e.g. /api/profile)
```

Login itself does NOT pass through this middleware — the client has no
token yet. See [[login-flow]] for how the initial token is issued.

## Relationship to user lookup

The middleware only validates the token; it does not load the full user
record. Downstream handlers fetch the user via [[user-store]] using the
attached `user_id` if they need additional fields.
