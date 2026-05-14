//! App-private persistent state.
//!
//! The CLI never touches files in this module — see spec
//! `AppConfig Namespace Isolation` for the strict CLI / app boundary.

pub mod active_runs;
pub mod app_state;
