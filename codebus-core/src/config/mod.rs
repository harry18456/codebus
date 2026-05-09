//! Global config loaded from `~/.codebus/config.yaml`.
//!
//! v3 config surface is intentionally minimal — only `lint.fix.*` fields
//! are read today. New config sections SHALL be added as their own
//! submodule and exposed via `pub use` here.

pub mod lint_fix;

pub use lint_fix::{LintFixConfig, default_config_path, load_lint_fix_config};
