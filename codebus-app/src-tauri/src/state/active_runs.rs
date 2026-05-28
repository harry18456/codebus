//! `ActiveRuns` — in-memory map of currently running goal/chat/quiz verb
//! invocations.
//!
//! Spec: `app-workspace § One Active Goal Run At A Time` + `app-workspace §
//! Cross-Vault Goal Spawn Permitted`. The map keys on `RunId` (the
//! started_at slug, optionally with `chat-` / `quiz-` prefix) and each
//! entry stores `(vault_path, cancel_flag)`. Storing vault inside the
//! value (rather than promoting it into the key) lets the cancel path
//! continue to look up entries by `run_id` alone — preserving the IPC
//! contract for `cancel_goal` / `cancel_chat_turn` / `cancel_quiz_*` —
//! while still letting pre-spawn guards filter by vault via
//! `has_*_for_vault` predicates.
//!
//! Vault key shape: callers pass `vault: &str` matching the `vault_path`
//! used by the corresponding IPC entry point. The store performs NO
//! canonicalization (no FS normalization, no symlink resolution). Treating
//! the caller-supplied string as ground truth keeps this layer free of
//! async / fallible work; ensuring different call sites pass consistent
//! representations of the same vault is the caller's responsibility.
//!
//! Lock granularity: a single `Mutex` wraps the whole map. The v1
//! invariant of at most one active run per (vault, mode) means
//! contention is effectively zero; a lock-free map would be premature
//! optimization.

use std::collections::HashMap;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};

/// Per-entry value stored in `ActiveRuns`. Holds the vault the run was
/// started under (for `has_*_for_vault` filtering) plus the cancel flag
/// the spawn thread polls.
#[derive(Debug, Clone)]
pub struct ActiveRunEntry {
    pub vault_path: String,
    pub cancel: Arc<AtomicBool>,
}

#[derive(Debug, Default)]
pub struct ActiveRuns(pub Mutex<HashMap<String, ActiveRunEntry>>);

impl ActiveRuns {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a cancel flag for `run_id` under `vault`. RunId remains
    /// the sole HashMap key; vault is stored in the value so cancel
    /// lookups (which only know `run_id`) keep working.
    pub fn insert(&self, vault: &str, run_id: String, cancel: Arc<AtomicBool>) {
        let mut map = self.0.lock().expect("active_runs mutex poisoned");
        map.insert(
            run_id,
            ActiveRunEntry {
                vault_path: vault.to_string(),
                cancel,
            },
        );
    }

    /// Remove the entry for `run_id`. Idempotent. Signature stays
    /// `run_id`-only so the cancel path's terminal cleanup
    /// (`active_runs_thread.remove(&run_id)` in spawn threads) and the
    /// IPC cancel commands need no vault propagation.
    pub fn remove(&self, run_id: &str) {
        let mut map = self.0.lock().expect("active_runs mutex poisoned");
        map.remove(run_id);
    }

    /// Return a clone of the cancel flag for `run_id`, or `None` when
    /// the run is not currently active. The clone shares ownership of
    /// the same `AtomicBool` so a `store(true)` is observed by the
    /// background thread polling its own clone. Signature stays
    /// `run_id`-only — the IPC cancel commands do not carry vault_path.
    pub fn get(&self, run_id: &str) -> Option<Arc<AtomicBool>> {
        let map = self.0.lock().expect("active_runs mutex poisoned");
        map.get(run_id).map(|entry| entry.cancel.clone())
    }

    /// Cross-vault aggregate: whether the map contains no entries under
    /// any vault. Test scaffolding only; production code SHOULD prefer
    /// the `has_*_for_vault` predicates which scope correctly to spec
    /// `One Active Goal Run At A Time` (per-vault enforcement).
    pub fn is_empty(&self) -> bool {
        let map = self.0.lock().expect("active_runs mutex poisoned");
        map.is_empty()
    }

    /// Whether any chat turn (RunId keyed with the `chat-` prefix) is
    /// currently active **under the given vault**. Used by
    /// `spawn_chat_turn`'s pre-spawn check. Entries under other vaults
    /// SHALL NOT cause this predicate to report active.
    pub fn has_chat_turn_for_vault(&self, vault: &str) -> bool {
        let map = self.0.lock().expect("active_runs mutex poisoned");
        map.iter()
            .any(|(run_id, entry)| entry.vault_path == vault && run_id.starts_with("chat-"))
    }

    /// Whether any goal run (RunId NOT prefixed `chat-`) is currently
    /// active **under the given vault**. Used by `spawn_goal`'s pre-spawn
    /// check — chat turns SHALL NOT block goal spawn per the `One Active
    /// Goal Run At A Time` requirement (chat is read-only and cannot
    /// conflict with goal's writes), AND entries under other vaults
    /// SHALL NOT block goal spawn per the `Cross-Vault Goal Spawn
    /// Permitted` requirement. Note: predicate inherits the original
    /// `has_goal_run` semantics — only `chat-` prefix is excluded from
    /// "goal"; `quiz-` prefixed entries would also be counted as goal
    /// here, but production UI flow never overlaps quiz and goal under
    /// the same vault simultaneously so this remains the conservative
    /// non-scope-creep behavior preserved from the pre-vault-scope impl.
    pub fn has_goal_run_for_vault(&self, vault: &str) -> bool {
        let map = self.0.lock().expect("active_runs mutex poisoned");
        map.iter()
            .any(|(run_id, entry)| entry.vault_path == vault && !run_id.starts_with("chat-"))
    }

    /// Whether any quiz run (RunId keyed with the `quiz-` prefix, covering
    /// both `quiz-plan-*` and `quiz-generate-*`) is currently active
    /// **under the given vault**. Used by `spawn_quiz_plan` /
    /// `spawn_quiz_generate`'s pre-spawn check.
    pub fn has_quiz_run_for_vault(&self, vault: &str) -> bool {
        let map = self.0.lock().expect("active_runs mutex poisoned");
        map.iter()
            .any(|(run_id, entry)| entry.vault_path == vault && run_id.starts_with("quiz-"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::Ordering;

    #[test]
    fn active_runs_insert_then_remove() {
        let runs = ActiveRuns::new();
        assert!(runs.is_empty());

        let flag = Arc::new(AtomicBool::new(false));
        runs.insert("/vault/a", "2026-05-13T14-56-21Z".into(), flag.clone());
        assert!(!runs.is_empty());

        let fetched = runs
            .get("2026-05-13T14-56-21Z")
            .expect("entry should exist");
        // The fetched Arc shares the same AtomicBool as the inserted one.
        fetched.store(true, Ordering::Relaxed);
        assert!(flag.load(Ordering::Relaxed));

        runs.remove("2026-05-13T14-56-21Z");
        assert!(runs.get("2026-05-13T14-56-21Z").is_none());
        assert!(runs.is_empty());
    }

    #[test]
    fn active_runs_remove_unknown_id_is_noop() {
        let runs = ActiveRuns::new();
        runs.remove("nonexistent");
        assert!(runs.is_empty());
    }

    #[test]
    fn active_runs_get_unknown_returns_none() {
        let runs = ActiveRuns::new();
        assert!(runs.get("nothing").is_none());
    }

    #[test]
    fn has_chat_turn_detects_chat_prefix() {
        let runs = ActiveRuns::new();
        assert!(!runs.has_chat_turn_for_vault("/vault/a"));
        runs.insert(
            "/vault/a",
            "chat-2026-05-14T10-20-30Z".into(),
            Arc::new(AtomicBool::new(false)),
        );
        assert!(runs.has_chat_turn_for_vault("/vault/a"));
        assert!(
            !runs.has_goal_run_for_vault("/vault/a"),
            "chat entry SHALL NOT register as goal"
        );
    }

    #[test]
    fn has_goal_run_ignores_chat_prefix() {
        let runs = ActiveRuns::new();
        assert!(!runs.has_goal_run_for_vault("/vault/a"));
        // Goal RunId is the started_at slug without prefix.
        runs.insert(
            "/vault/a",
            "2026-05-14T10-20-30Z".into(),
            Arc::new(AtomicBool::new(false)),
        );
        assert!(runs.has_goal_run_for_vault("/vault/a"));
        assert!(
            !runs.has_chat_turn_for_vault("/vault/a"),
            "goal entry SHALL NOT register as chat"
        );
    }

    /// quiz-double-spawn-guard: has_quiz_run_for_vault detects the `quiz-`
    /// prefix and distinguishes quiz ids from chat / goal ids.
    #[test]
    fn has_quiz_run_detects_quiz_prefix() {
        let runs = ActiveRuns::new();
        assert!(!runs.has_quiz_run_for_vault("/vault/a"));
        runs.insert(
            "/vault/a",
            "quiz-generate-2026-05-21T05-37-14Z".into(),
            Arc::new(AtomicBool::new(false)),
        );
        assert!(runs.has_quiz_run_for_vault("/vault/a"));
    }

    #[test]
    fn has_quiz_run_false_for_chat_and_goal_ids() {
        let runs = ActiveRuns::new();
        runs.insert(
            "/vault/a",
            "chat-2026-05-21T05-37-14Z".into(),
            Arc::new(AtomicBool::new(false)),
        );
        runs.insert(
            "/vault/a",
            "2026-05-21T05-37-14Z".into(),
            Arc::new(AtomicBool::new(false)),
        );
        assert!(
            !runs.has_quiz_run_for_vault("/vault/a"),
            "chat / goal ids SHALL NOT register as quiz"
        );
    }

    #[test]
    fn chat_and_goal_can_coexist() {
        let runs = ActiveRuns::new();
        runs.insert(
            "/vault/a",
            "chat-2026-05-14T10-20-30Z".into(),
            Arc::new(AtomicBool::new(false)),
        );
        runs.insert(
            "/vault/a",
            "2026-05-14T10-25-00Z".into(),
            Arc::new(AtomicBool::new(false)),
        );
        assert!(runs.has_chat_turn_for_vault("/vault/a"));
        assert!(runs.has_goal_run_for_vault("/vault/a"));
        assert!(!runs.is_empty());
    }

    // ----- Per-vault scope tests (vault-switch-goal-regression Decision 1/2/5) -----

    /// Spec § Cross-Vault Goal Spawn Permitted scenario 1: goal in vault A
    /// SHALL NOT cause the per-vault guard for vault B to report active.
    #[test]
    fn active_runs_cross_vault_goal_allowed() {
        let runs = ActiveRuns::new();
        runs.insert(
            "/vault/a",
            "2026-05-14T10-20-30Z".into(),
            Arc::new(AtomicBool::new(false)),
        );
        assert!(runs.has_goal_run_for_vault("/vault/a"));
        assert!(
            !runs.has_goal_run_for_vault("/vault/b"),
            "goal under vault A SHALL NOT block vault B"
        );
    }

    /// Spec § One Active Goal Run At A Time scenario: same vault same mode
    /// remains mutually exclusive.
    #[test]
    fn active_runs_same_vault_same_mode_blocks() {
        let runs = ActiveRuns::new();
        runs.insert(
            "/vault/a",
            "2026-05-14T10-20-30Z".into(),
            Arc::new(AtomicBool::new(false)),
        );
        assert!(runs.has_goal_run_for_vault("/vault/a"));
        runs.insert(
            "/vault/a",
            "2026-05-14T10-21-00Z".into(),
            Arc::new(AtomicBool::new(false)),
        );
        assert!(
            runs.has_goal_run_for_vault("/vault/a"),
            "second goal under same vault SHALL still be detected"
        );
    }

    /// Spec § Cross-Vault Goal Spawn Permitted scenarios 3/4: chat in vault A
    /// SHALL NOT block chat in vault B; quiz in vault A SHALL NOT block quiz
    /// in vault B.
    #[test]
    fn active_runs_per_vault_chat_and_quiz_isolation() {
        let runs = ActiveRuns::new();
        runs.insert(
            "/vault/a",
            "chat-2026-05-14T10-20-30Z".into(),
            Arc::new(AtomicBool::new(false)),
        );
        runs.insert(
            "/vault/a",
            "quiz-generate-2026-05-14T10-21-00Z".into(),
            Arc::new(AtomicBool::new(false)),
        );
        assert!(runs.has_chat_turn_for_vault("/vault/a"));
        assert!(runs.has_quiz_run_for_vault("/vault/a"));
        assert!(
            !runs.has_chat_turn_for_vault("/vault/b"),
            "chat under vault A SHALL NOT register as active under vault B"
        );
        assert!(
            !runs.has_quiz_run_for_vault("/vault/b"),
            "quiz under vault A SHALL NOT register as active under vault B"
        );
    }

    /// `get(run_id)` returns the cancel flag for whichever vault the run
    /// was inserted under — the value-side vault storage means cancel
    /// lookups (which know only `run_id`) keep working.
    #[test]
    fn active_runs_get_returns_cancel_regardless_of_vault() {
        let runs = ActiveRuns::new();
        let flag = Arc::new(AtomicBool::new(false));
        runs.insert("/vault/a", "2026-05-14T10-20-30Z".into(), flag.clone());
        let fetched = runs
            .get("2026-05-14T10-20-30Z")
            .expect("entry should exist");
        fetched.store(true, Ordering::Relaxed);
        assert!(flag.load(Ordering::Relaxed));
    }

    /// `is_empty` retains cross-vault aggregate semantics (test scaffolding
    /// only; production code SHOULD prefer `has_*_for_vault`).
    #[test]
    fn active_runs_is_empty_aggregates_across_vaults() {
        let runs = ActiveRuns::new();
        assert!(runs.is_empty());
        runs.insert(
            "/vault/a",
            "2026-05-14T10-20-30Z".into(),
            Arc::new(AtomicBool::new(false)),
        );
        assert!(!runs.is_empty());
        runs.insert(
            "/vault/b",
            "2026-05-14T11-30-00Z".into(),
            Arc::new(AtomicBool::new(false)),
        );
        assert!(!runs.is_empty());
        runs.remove("2026-05-14T10-20-30Z");
        assert!(!runs.is_empty(), "vault B entry still present");
        runs.remove("2026-05-14T11-30-00Z");
        assert!(runs.is_empty());
    }
}
