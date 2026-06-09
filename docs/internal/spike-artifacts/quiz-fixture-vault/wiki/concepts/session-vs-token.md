---
title: Session vs Token
type: concept
sources: []
goals:
  - explain web auth components
related:
  - jwt-token-lifecycle
created: '2026-05-15'
updated: '2026-05-15'
stale: false
---

# Session vs Token

Two common authentication strategies for web apps: server-side sessions
(stateful) and self-contained tokens (stateless).

## Server-side sessions

The server creates a record in its session store (memory, Redis, database)
keyed by an opaque session id. The id is sent to the client as a cookie.
On each request, the server looks up the session id to recover user state.

- **Pros**: revocation is instant (delete the row); session data can be large.
- **Cons**: server must maintain shared state across instances; horizontal
  scaling requires a shared store.

## Self-contained tokens (JWT)

The server signs user claims directly into the token (see
[[jwt-token-lifecycle]]). The token IS the state — no server lookup
needed.

- **Pros**: stateless; trivially scales horizontally; works across services
  sharing the secret.
- **Cons**: revocation is hard (token stays valid until `exp`); claims
  cannot be updated mid-session without re-issuance.

## When to pick which

Pick sessions when you need fine-grained revocation (banking, sensitive
ops). Pick tokens when you need horizontal scale and cross-service
authentication (microservices, mobile APIs).

This fixture uses JWT; see [[jwt-token-lifecycle]] for the issue/verify
flow.
