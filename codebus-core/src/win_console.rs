//! Console-window suppression for short-lived child processes on Windows.
//!
//! A release GUI build (`windows_subsystem = "windows"`, no console of its
//! own) still flashes a fresh console window whenever it spawns an external
//! command — `cmd /C <bin> --version` to probe a CLI, `git` to compute
//! changed paths or a wiki diff, etc. [`hide_console`] sets
//! `CREATE_NO_WINDOW` on the command so no window pops up.
//!
//! This is deliberately narrower than [`crate::agent::process_kill`]'s
//! `pre_spawn`: it sets *only* `CREATE_NO_WINDOW`. These are short-lived
//! `output()`/`status()` commands that don't need the Job Object /
//! process-group machinery for process-tree kill — that stays exclusive to
//! the agent invoke path. On non-Windows targets this is a no-op.

/// Suppress the console window for a child process on Windows.
///
/// Sets `CREATE_NO_WINDOW` so spawning `cmd` / `git` from a windowless GUI
/// build does not flash a console. Does not touch stdio piping, so any
/// `output()` captured stdout/stderr is unaffected. No-op on non-Windows.
pub fn hide_console(cmd: &mut std::process::Command) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        use windows_sys::Win32::System::Threading::CREATE_NO_WINDOW;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    #[cfg(not(windows))]
    let _ = cmd;
}
