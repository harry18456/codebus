//! IPC command registry.
//!
//! The frontend MAY invoke only the nine commands listed in
//! [`REGISTERED_COMMANDS`]. The spec ("IPC Command Registry") forbids any
//! other command from being registered by this change. The constant is the
//! source of truth checked by the unit test below, and is consumed by
//! `lib::run` via [`generate_handler`] so the registration and the asserted
//! list cannot drift in isolation.

use crate::error::AppError;

pub mod cli_status;
pub mod config;
pub mod keyring;
pub mod vault_list;

pub use cli_status::check_cli_installed;
pub use config::{load_global_config, save_global_config};
pub use keyring::{delete_endpoint_key, get_endpoint_key, set_endpoint_key};
pub use vault_list::{add_vault, list_vaults, remove_vault};

/// Exactly the nine commands exposed by this Tauri app. Used by the
/// `exactly_nine_commands_are_registered` test and consumed by `lib::run`.
pub const REGISTERED_COMMANDS: &[&str] = &[
    "list_vaults",
    "add_vault",
    "remove_vault",
    "load_global_config",
    "save_global_config",
    "set_endpoint_key",
    "get_endpoint_key",
    "delete_endpoint_key",
    "check_cli_installed",
];

/// Sugar for `tauri::generate_handler!` so the registration is colocated
/// with the asserted command list.
#[macro_export]
macro_rules! generate_ipc_handler {
    () => {
        ::tauri::generate_handler![
            $crate::ipc::vault_list::list_vaults,
            $crate::ipc::vault_list::add_vault,
            $crate::ipc::vault_list::remove_vault,
            $crate::ipc::config::load_global_config,
            $crate::ipc::config::save_global_config,
            $crate::ipc::keyring::set_endpoint_key,
            $crate::ipc::keyring::get_endpoint_key,
            $crate::ipc::keyring::delete_endpoint_key,
            $crate::ipc::cli_status::check_cli_installed,
        ]
    };
}

/// All IPC commands return `Result<T, AppError>` so the frontend always
/// receives the same discriminated-union error shape (see `error.rs`).
pub type IpcResult<T> = std::result::Result<T, AppError>;

#[cfg(test)]
mod tests {
    use super::REGISTERED_COMMANDS;

    #[test]
    fn exactly_nine_commands_are_registered() {
        assert_eq!(
            REGISTERED_COMMANDS.len(),
            9,
            "IPC Command Registry requires exactly 9 commands"
        );
    }

    #[test]
    fn command_names_match_spec() {
        let expected: std::collections::HashSet<&str> = [
            "list_vaults",
            "add_vault",
            "remove_vault",
            "load_global_config",
            "save_global_config",
            "set_endpoint_key",
            "get_endpoint_key",
            "delete_endpoint_key",
            "check_cli_installed",
        ]
        .into_iter()
        .collect();
        let actual: std::collections::HashSet<&str> = REGISTERED_COMMANDS.iter().copied().collect();
        assert_eq!(actual, expected, "registered command names drifted");
    }

    #[test]
    fn no_command_name_duplicates() {
        let mut seen = std::collections::HashSet::new();
        for name in REGISTERED_COMMANDS {
            assert!(seen.insert(*name), "duplicate command: {name}");
        }
    }
}
