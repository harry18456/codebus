---
title: User Store
type: module
sources:
  - path: src/user_store.py
goals:
  - explain web auth components
related:
  - auth-middleware
  - login-flow
created: '2026-05-15'
updated: '2026-05-15'
stale: false
---

# User Store

Persistent storage of user records and the lookup helpers used by
authentication flows.

## Record shape

Each user record has:

- `id` (integer, primary key)
- `email` (unique string)
- `password_hash` (bcrypt-style hash; never the plaintext)
- `created_at`

## Lookup helpers

Two helpers are exposed:

- `get_by_id(user_id)` — used by downstream handlers after the
  [[auth-middleware]] has attached `request.user_id`.
- `get_by_email(email)` — used during [[login-flow]] to locate the
  record by the login form's email field.

## Password verification

The store exposes `verify_password(record, plaintext)` which hashes
the plaintext input and compares against the stored `password_hash`
in constant time. This is the only place plaintext passwords appear
in the request path; it is invoked exclusively from [[login-flow]].

The store never returns the `password_hash` to callers — handlers
treat the user record as opaque after the verification step.
