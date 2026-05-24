//! (Phase 3 / prompt-surface-layer-3-spawnspec-restructure)
//!
//! This module previously held `initial_prompt() -> "/codebus-fix"` —
//! the pre-composed slash-form prompt that the verb layer passed into
//! `SpawnSpec.prompt`. Phase 3 removed pre-composition: the backend now
//! assembles the `/codebus-fix` (claude) or `$codebus-fix` (codex) form
//! from `SpawnSpec { verb: Verb::Fix, sub_mode: None, input: "" }`.
//!
//! The module is kept (empty) so external dependents don't break on a
//! missing path import; the helper function it once exported is no
//! longer needed.
