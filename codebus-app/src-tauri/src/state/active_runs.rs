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

    /// Whether any goal run is currently active. Used by `spawn_goal`'s
    /// pre-spawn invariant check (`AppError::Invalid { field: "active_runs", ... }`).
    pub fn is_empty(&self) -> bool {
        let map = self.0.lock().expect("active_runs mutex poisoned");
        map.is_empty()
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
}
