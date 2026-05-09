//! Git operations on the nested vault repository at `.codebus/`.
//!
//! v3-vault-history #4 ships a single submodule [`nested_repo`] exposing two
//! operations:
//!
//! - [`init_nested_repo`]: idempotent `git init -b main` + local
//!   `user.email=codebus@local` / `user.name=codebus` config (decoupled from
//!   the user's global git config so codebus auto-commits work on fresh boxes
//!   / CI / containers without setup).
//! - [`auto_commit`]: `git add -A` + `git commit -m`; clean working tree is a
//!   no-op that returns the existing HEAD sha. Used by init for the first
//!   commit and (in later changes #5 / #8) by goal / fix verbs at spawn tail.

pub mod nested_repo;

pub use nested_repo::{auto_commit, init_nested_repo};
