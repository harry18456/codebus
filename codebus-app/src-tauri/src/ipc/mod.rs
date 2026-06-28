//! IPC command registry.
//!
//! The frontend MAY invoke only the thirty-two commands listed in
//! [`REGISTERED_COMMANDS`]. The spec ("IPC Command Registry", modified
//! by `v3-app-workspace-goal` for goal lifecycle, by `v3-app-chat-cmdk`
//! for chat-turn lifecycle, by `v3-app-quiz` for quiz plan/generate
//! lifecycle, by `quiz-attempt-progress` for the two progress-sidecar
//! commands, by `wiki-open-in-obsidian` for the Obsidian probe +
//! open commands, and by `mcp-multi-vault-and-client-install` for the
//! three MCP-integration commands) forbids any other command from being
//! registered by this change. The constant is the source of truth
//! checked by the unit test below, and is consumed by `lib::run` via
//! [`generate_handler`] so the registration and the asserted list
//! cannot drift in isolation.

use crate::error::AppError;

pub mod chats;
pub mod cli_status;
pub mod config;
pub mod global_md;
pub mod goals;
pub mod keyring;
pub mod mcp_install;
pub mod quiz;
pub mod vault_list;
pub mod vault_progress;
pub mod wiki;

pub use chats::{cancel_chat_turn, spawn_chat_turn};
pub use quiz::{
    cancel_quiz, list_quiz_attempts, read_quiz_attempt, read_quiz_events,
    read_quiz_progress, spawn_quiz_generate, spawn_quiz_plan, write_quiz_progress,
};
pub use cli_status::check_cli_installed;
pub use mcp_install::{mcp_client_install, mcp_client_remove, mcp_client_status};
pub use config::{load_global_config, save_global_config};
pub use goals::{cancel_goal, get_run_detail, list_runs, spawn_goal};
pub use keyring::{delete_endpoint_key, get_endpoint_key, set_endpoint_key};
pub use vault_list::{add_vault, list_vaults, remove_vault};
pub use wiki::{get_obsidian_vault_id, list_wiki_pages, open_wiki_in_obsidian, read_wiki_page};

/// Exactly the thirty-two commands exposed by this Tauri app. Used by
/// the `exactly_thirty_two_commands_are_registered` test and consumed
/// by `lib::run`.
///
/// Foundation 9 + workspace 8 (`spawn_goal`, `cancel_goal`, `list_runs`,
/// `get_run_detail`, `list_wiki_pages`, `read_wiki_page`,
/// `get_obsidian_vault_id`, `open_wiki_in_obsidian`) + chat 2
/// (`spawn_chat_turn`, `cancel_chat_turn`) + quiz 8 (`spawn_quiz_plan`,
/// `spawn_quiz_generate`, `cancel_quiz`, `list_quiz_attempts`,
/// `read_quiz_attempt`, `read_quiz_events`, `read_quiz_progress`,
/// `write_quiz_progress`) + watcher 2 (`start_vault_watcher`,
/// `stop_vault_watcher`) + mcp-install 3 (`mcp_client_status`,
/// `mcp_client_install`, `mcp_client_remove`).
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
    "spawn_goal",
    "cancel_goal",
    "list_runs",
    "get_run_detail",
    "list_wiki_pages",
    "read_wiki_page",
    "get_obsidian_vault_id",
    "open_wiki_in_obsidian",
    "spawn_chat_turn",
    "cancel_chat_turn",
    "spawn_quiz_plan",
    "spawn_quiz_generate",
    "cancel_quiz",
    "list_quiz_attempts",
    "read_quiz_attempt",
    "read_quiz_events",
    "read_quiz_progress",
    "write_quiz_progress",
    "start_vault_watcher",
    "stop_vault_watcher",
    "mcp_client_status",
    "mcp_client_install",
    "mcp_client_remove",
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
            $crate::ipc::goals::spawn_goal,
            $crate::ipc::goals::cancel_goal,
            $crate::ipc::goals::list_runs,
            $crate::ipc::goals::get_run_detail,
            $crate::ipc::wiki::list_wiki_pages,
            $crate::ipc::wiki::read_wiki_page,
            $crate::ipc::wiki::get_obsidian_vault_id,
            $crate::ipc::wiki::open_wiki_in_obsidian,
            $crate::ipc::chats::spawn_chat_turn,
            $crate::ipc::chats::cancel_chat_turn,
            $crate::ipc::quiz::spawn_quiz_plan,
            $crate::ipc::quiz::spawn_quiz_generate,
            $crate::ipc::quiz::cancel_quiz,
            $crate::ipc::quiz::list_quiz_attempts,
            $crate::ipc::quiz::read_quiz_attempt,
            $crate::ipc::quiz::read_quiz_events,
            $crate::ipc::quiz::read_quiz_progress,
            $crate::ipc::quiz::write_quiz_progress,
            $crate::watcher::start_vault_watcher,
            $crate::watcher::stop_vault_watcher,
            $crate::ipc::mcp_install::mcp_client_status,
            $crate::ipc::mcp_install::mcp_client_install,
            $crate::ipc::mcp_install::mcp_client_remove,
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
    fn exactly_thirty_two_commands_are_registered() {
        assert_eq!(
            REGISTERED_COMMANDS.len(),
            32,
            "IPC Command Registry requires exactly 32 commands (9 foundation + 8 workspace + 2 chat + 8 quiz + 2 watcher + 3 mcp-install)"
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
            "spawn_goal",
            "cancel_goal",
            "list_runs",
            "get_run_detail",
            "list_wiki_pages",
            "read_wiki_page",
            "get_obsidian_vault_id",
            "open_wiki_in_obsidian",
            "spawn_chat_turn",
            "cancel_chat_turn",
            "spawn_quiz_plan",
            "spawn_quiz_generate",
            "cancel_quiz",
            "list_quiz_attempts",
            "read_quiz_attempt",
            "read_quiz_events",
            "read_quiz_progress",
            "write_quiz_progress",
            "start_vault_watcher",
            "stop_vault_watcher",
            "mcp_client_status",
            "mcp_client_install",
            "mcp_client_remove",
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
