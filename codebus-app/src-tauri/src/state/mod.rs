//! App-private persistent state.
//!
//! The app is the sole *writer* of these files. The CLI does not write them;
//! the `mcp` subcommand reads `app-state.json` read-only (via
//! `codebus_core::app_state`) as the multi-vault registry. See spec
//! `AppConfig Namespace Isolation` and the `mcp-server` capability for the
//! CLI / app boundary.

pub mod active_runs;
pub mod app_state;
