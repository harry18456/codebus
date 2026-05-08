pub mod config_path;
pub mod process_detect;
pub mod registry;

pub use registry::{lookup_vault_id, register_vault, RegisterOutcome};
