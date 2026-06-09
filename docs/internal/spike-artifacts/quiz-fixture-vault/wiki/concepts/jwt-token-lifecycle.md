---
title: JWT Token Lifecycle
type: concept
sources:
  - path: src/auth.py
goals:
  - explain web auth components
related:
  - session-vs-token
  - auth-middleware
created: '2026-05-15'
updated: '2026-05-15'
stale: false
---

# JWT Token Lifecycle

A JSON Web Token (JWT) is a signed, self-contained credential that encodes
user identity claims. Unlike a server-side session, the server stores no
state for a JWT — verification happens by checking the signature.

## Issuance

When a user authenticates successfully (see [[login-flow]]), the server
constructs a payload containing the `sub` (subject — usually the user id),
`iat` (issued-at timestamp), and `exp` (expiry timestamp). The payload is
signed with a secret key using HS256, producing a compact base64-encoded
token.

In this fixture, tokens expire 24 hours after issuance.

## Verification

The [[auth-middleware]] extracts the JWT from the `Authorization: Bearer
<token>` header on every request. It calls `verify_token`, which decodes
the signature against the secret and rejects expired or malformed tokens.

A verified token's claims are attached to the request (as `request.user_id`)
for downstream handlers.

## Expiry

Once `exp` is past, verification returns `None` and the middleware responds
with 401 Unauthorized. There is no revocation list — a stolen token remains
valid until expiry. This is the core trade-off of stateless tokens
(see [[session-vs-token]]).
