//! `ActiveRuns` — in-memory map of currently running goal verb invocations.
//!
//! Spec: `app-workspace § One Active Goal Run At A Time` + design `Active
//! runs 狀態存 AppState.active_runs`. The map keys on the RunId string
//! (= run `started_at` slug) and values are an `Arc<AtomicBool>` shared
//! between the spawn thread and the cancel-button handler — flipping the
//! flag triggers cooperative cancellation inside `run_goal`'s polling
//! loop.
//!
//! Lock granularity: a single `Mutex` wraps the whole map. The v1
//! invariant of at most one active run means contention is effectively
//! zero; a lock-free map would be premature optimization.

use std::collections::HashMap;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};

#[derive(Debug, Default)]
pub struct ActiveRuns(pub Mutex<HashMap<String, Arc<AtomicBool>>>);

impl ActiveRuns {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a cancel flag for the given run id.
    pub fn insert(&self, run_id: String, cancel: Arc<AtomicBool>) {
        let mut map = self.0.lock().expect("active_runs mutex poisoned");
        map.insert(run_id, cancel);
    }

    /// Remove the entry for `run_id`. Idempotent — removing a non-existent
    /// key is a no-op.
    pub fn remove(&self, run_id: &str) {
        let mut map = self.0.lock().expect("active_runs mutex poisoned");
        map.remove(run_id);
    }

    /// Return a clone of the cancel flag for `run_id`, or `None` when
    /// the run is not currently active. The clone shares ownership of
    /// the same `AtomicBool` so a `store(true)` is observed by the
    /// background thread polling its own clone.
    pub fn get(&self, run_id: &str) -> Option<Arc<AtomicBool>> {
        let map = self.0.lock().expect("active_runs mutex poisoned");
        map.get(run_id).cloned()
    }

    /// Whether the map is empty (no active goal AND no active chat turn).
    /// Kept for backward compat; new code SHOULD prefer the prefix-specific
    /// helpers below.
    pub fn is_empty(&self) -> bool {
        let map = self.0.lock().expect("active_runs mutex poisoned");
        map.is_empty()
    }

    /// Whether any chat turn (RunId keyed with the `chat-` prefix) is
    /// currently active. Used by `spawn_chat_turn`'s pre-spawn check —
    /// chat session semantics disallow two concurrent turns even though
    /// chat and goal can coexist (see `Chat Turn Lifecycle` requirement).
    pub fn has_chat_turn(&self) -> bool {
        let map = self.0.lock().expect("active_runs mutex poisoned");
        map.keys().any(|k| k.starts_with("chat-"))
    }

    /// Whether any goal run (RunId NOT prefixed `chat-`) is currently
    /// active. Used by `spawn_goal`'s pre-spawn check — chat turns SHALL
    /// NOT block goal spawn per the `One Active Goal Run At A Time`
    /// requirement (chat is read-only and cannot conflict with goal's
    /// writes).
    pub fn has_goal_run(&self) -> bool {
        let map = self.0.lock().expect("active_runs mutex poisoned");
        map.keys().any(|k| !k.starts_with("chat-"))
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
        runs.insert("2026-05-13T14-56-21Z".into(), flag.clone());
        assert!(!runs.is_empty());

        let fetched = runs.get("2026-05-13T14-56-21Z").expect("entry should exist");
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
        assert!(!runs.has_chat_turn());
        runs.insert("chat-2026-05-14T10-20-30Z".into(), Arc::new(AtomicBool::new(false)));
        assert!(runs.has_chat_turn());
        assert!(!runs.has_goal_run(), "chat entry SHALL NOT register as goal");
    }

    #[test]
    fn has_goal_run_ignores_chat_prefix() {
        let runs = ActiveRuns::new();
        assert!(!runs.has_goal_run());
        // Goal RunId is the started_at slug without prefix.
        runs.insert("2026-05-14T10-20-30Z".into(), Arc::new(AtomicBool::new(false)));
        assert!(runs.has_goal_run());
        assert!(!runs.has_chat_turn(), "goal entry SHALL NOT register as chat");
    }

    #[test]
    fn chat_and_goal_can_coexist() {
        let runs = ActiveRuns::new();
        runs.insert("chat-2026-05-14T10-20-30Z".into(), Arc::new(AtomicBool::new(false)));
        runs.insert("2026-05-14T10-25-00Z".into(), Arc::new(AtomicBool::new(false)));
        assert!(runs.has_chat_turn());
        assert!(runs.has_goal_run());
        assert!(!runs.is_empty());
    }
}
