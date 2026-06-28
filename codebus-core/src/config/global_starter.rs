//! Global config starter file writer for v3-config.
//!
//! `~/.codebus/config.yaml` is never created automatically by the loaders
//! (they fall through to defaults on `NotFound`), so users have no way to
//! discover what knobs exist. The starter writer fixes that: `codebus init`
//! invokes [`write_starter_config_if_missing`] to drop a starter file — a
//! short shared header comment pointing at the field reference doc, followed
//! by every key with its default value. The body is pure values: per-field
//! teaching lives in `docs/config-reference.md`, not in inline comments,
//! because `serde_yaml` strips comments the moment the app re-serializes the
//! config on save (so inline teaching would silently vanish on first save).
//!
//! The starter content is hardcoded — it is the source of truth for what
//! defaults look like in YAML form. Round-tripping it through the per-section
//! loaders MUST yield exactly `Default::default()` for each section (covered
//! by the `starter_round_trips_to_defaults` test below).
//!
//! `if-missing` semantics: if `path.exists()`, return [`StarterOutcome::AlreadyPresent`]
//! without reading or writing — user customizations are sacred. If the parent
//! directory does not exist, it is `create_dir_all`'d.

use std::fs;
use std::io;
use std::path::Path;

/// Outcome of a starter write attempt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StarterOutcome {
    /// Wrote a new file (the parent directory was created if necessary).
    Written,
    /// File already existed; no action taken.
    AlreadyPresent,
}

/// Shared config-file header text. A `macro_rules!` rather than a `const` so
/// it can be fed to `concat!` at compile time — a `const &str` cannot — which
/// lets [`CONFIG_HEADER`] and [`STARTER_CONFIG`] share one source of header
/// text with no duplicated literal. Kept deliberately short: it points at the
/// field reference doc instead of embedding per-field teaching.
macro_rules! config_header {
    () => {
        "# codebus config (~/.codebus/config.yaml)\n# Managed by the codebus app Settings, or hand-edit. Every key is optional;\n# omit it to use the default. Full field reference: docs/config-reference.md\n"
    };
}

/// The shared config-file header (see [`config_header`]). The app's
/// `save_global_config` path prepends this exact text after serializing, so a
/// CLI-written starter and an app-saved config share one header-plus-values
/// shape. Single source of truth — do not duplicate the literal elsewhere.
pub const CONFIG_HEADER: &str = config_header!();

/// Hardcoded starter config content: [`CONFIG_HEADER`] followed by every
/// section populated with its default value as pure YAML (no inline per-field
/// comments — see `docs/config-reference.md` for what each knob does). The
/// body MUST round-trip through every section loader to yield
/// `Default::default()` — verified by `starter_round_trips_to_defaults` below.
pub const STARTER_CONFIG: &str = concat!(
    config_header!(),
    r#"
pii:
  scanner: regex_basic
  patterns_extra: []
  on_hit: warn

agent:
  active_provider: claude
  providers:
    claude:
      active: system
      system:
        goal:
          model: opus-4-6
          effort: high
        query:
          model: haiku-4-5
          effort: low
        fix:
          model: sonnet-4-6
          effort: medium
        verify:
          model: opus-4-6
          effort: high

hooks:
  read_image_block: true
  read_path_containment: true

lint:
  fix:
    enabled: true

log:
  sink: jsonl
"#
);

/// Write [`STARTER_CONFIG`] to `path` if the file does not already exist.
/// Creates the parent directory if necessary. Returns [`StarterOutcome::AlreadyPresent`]
/// when `path.exists()` — the caller is responsible for surfacing that as a
/// user-facing message; this primitive does NOT print anything itself.
pub fn write_starter_config_if_missing(path: &Path) -> io::Result<StarterOutcome> {
    if path.exists() {
        return Ok(StarterOutcome::AlreadyPresent);
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, STARTER_CONFIG)?;
    Ok(StarterOutcome::Written)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{
        ClaudeCodeConfig, LogConfig, PiiConfig, load_claude_code_config, load_lint_fix_config,
        load_log_config, load_pii_config,
    };
    use crate::pii::provider::OnHit;
    use tempfile::TempDir;

    /// Spec: writes the starter file when missing.
    #[test]
    fn writes_when_missing() {
        let tmp = TempDir::new().unwrap();
        let target = tmp.path().join("config.yaml");
        let outcome = write_starter_config_if_missing(&target).unwrap();
        assert_eq!(outcome, StarterOutcome::Written);
        assert!(target.exists());
        let body = fs::read_to_string(&target).unwrap();
        assert_eq!(body, STARTER_CONFIG);
    }

    /// Spec: noop when the file already exists. Existing content is NOT read
    /// nor compared — the file is sacrosanct.
    #[test]
    fn noop_when_present() {
        let tmp = TempDir::new().unwrap();
        let target = tmp.path().join("config.yaml");
        fs::write(&target, "user-custom-content\n").unwrap();
        let outcome = write_starter_config_if_missing(&target).unwrap();
        assert_eq!(outcome, StarterOutcome::AlreadyPresent);
        let body = fs::read_to_string(&target).unwrap();
        assert_eq!(body, "user-custom-content\n");
    }

    /// Spec: parent directory created when absent.
    #[test]
    fn creates_parent_dir() {
        let tmp = TempDir::new().unwrap();
        let target = tmp.path().join("nested").join("dir").join("config.yaml");
        assert!(!target.parent().unwrap().exists());
        let outcome = write_starter_config_if_missing(&target).unwrap();
        assert_eq!(outcome, StarterOutcome::Written);
        assert!(target.exists());
    }

    /// The starter uses the unified `agent.providers.*` schema, not the
    /// removed legacy top-level `claude_code` block.
    #[test]
    fn starter_config_uses_agent_schema() {
        assert!(STARTER_CONFIG.contains("\nagent:\n"));
        assert!(STARTER_CONFIG.contains("active_provider: claude"));
        assert!(STARTER_CONFIG.contains("  providers:\n"));
        assert!(STARTER_CONFIG.contains("    claude:\n"));
        // No legacy top-level claude_code block.
        assert!(!STARTER_CONFIG.contains("\nclaude_code:\n"));
    }

    /// Spec: starter content round-trips through every loader to defaults.
    /// This is the contract that keeps STARTER_CONFIG honest — if defaults
    /// change in any sub-module, this test will catch the drift.
    #[test]
    fn starter_round_trips_to_defaults() {
        let tmp = TempDir::new().unwrap();
        let target = tmp.path().join("config.yaml");
        write_starter_config_if_missing(&target).unwrap();

        let pii = load_pii_config(&target).unwrap();
        assert_eq!(pii, PiiConfig::default());
        // v3-pii-severity-dispatch: starter `on_hit: warn` matches the new
        // PiiConfig::default() value.
        assert_eq!(pii.on_hit, OnHit::Warn);

        let cc = load_claude_code_config(&target).unwrap();
        assert_eq!(cc, ClaudeCodeConfig::default());
        // Strong check: the starter is actually PARSED as the new schema (not
        // ignored → default). A bogus marker model would fail the round-trip,
        // so confirm the resolved model came from the starter's system block.
        assert_eq!(
            cc.resolve(crate::config::Verb::Goal).model.as_deref(),
            Some("claude-opus-4-6")
        );

        let lf = load_lint_fix_config(&target).unwrap();
        assert!(lf.enabled);

        // v3-run-log: log section also round-trips to default.
        let lg = load_log_config(&target).unwrap();
        assert_eq!(lg, LogConfig::default());

        // check-read-vault-containment: hooks section round-trips, both
        // gates default on (independent).
        let hooks = crate::config::load_hooks_config(&target).unwrap();
        assert_eq!(hooks, crate::config::HooksConfig::default());
        assert!(hooks.read_image_block);
        assert!(hooks.read_path_containment);
    }

    /// check-read-vault-containment: the starter documents AND enables the
    /// containment gate (body contains the key set to true).
    #[test]
    fn starter_config_includes_read_path_containment() {
        assert!(STARTER_CONFIG.contains("read_path_containment: true"));
    }

    /// config-save-robustness: the starter begins with the shared CONFIG_HEADER
    /// (single source of truth reused by the app's save path).
    #[test]
    fn starter_starts_with_shared_header() {
        assert!(STARTER_CONFIG.starts_with(CONFIG_HEADER));
        assert!(CONFIG_HEADER.starts_with("# codebus config"));
        assert!(CONFIG_HEADER.contains("docs/config-reference.md"));
    }

    /// config-save-robustness: past the shared header the starter body carries
    /// NO inline per-field teaching comments — pure values only, so the app's
    /// comment-stripping save round-trip cannot diverge from the starter shape.
    #[test]
    fn starter_body_has_no_inline_comments_beyond_header() {
        let body = STARTER_CONFIG
            .strip_prefix(CONFIG_HEADER)
            .expect("starter must start with CONFIG_HEADER");
        for line in body.lines() {
            assert!(
                !line.trim_start().starts_with('#'),
                "starter body must carry no inline comments, found: {line:?}"
            );
        }
    }
}
