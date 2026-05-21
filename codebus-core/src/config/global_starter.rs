//! Global config starter file writer for v3-config.
//!
//! `~/.codebus/config.yaml` is never created automatically by the loaders
//! (they fall through to defaults on `NotFound`), so users have no way to
//! discover what knobs exist. The starter writer fixes that: `codebus init`
//! invokes [`write_starter_config_if_missing`] to drop a fully-commented
//! template containing every key with its default value.
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

/// Hardcoded starter config content. Every section is fully populated with
/// inline-comment defaults so a user reading the file knows what each knob
/// does without consulting docs. The string MUST round-trip through every
/// section loader to yield `Default::default()` — verified by
/// `starter_round_trips_to_defaults` below.
pub const STARTER_CONFIG: &str = r#"# codebus global config — ~/.codebus/config.yaml
#
# Edit this file to customize codebus behavior. Every key below is optional;
# omitting a key applies its default. Unknown keys are silently ignored
# (forward-compat) so future codebus versions can extend this schema without
# breaking your config.

# PII scanner behavior during raw mirror sync.
pii:
  # Scanner implementation: "regex_basic" runs the built-in 4-pattern regex
  # set (AWS access key, Anthropic API key, email, IPv4); "none" disables
  # PII scanning entirely. (Note: do NOT use the bare YAML literal `null`
  # here — that parses as the YAML null literal and falls through to the
  # default `regex_basic`. Use the string `none` instead.)
  scanner: regex_basic

  # Extra regex patterns appended to the built-in set. Each entry is a regex
  # source string; compile failures fall back to the built-in set with a
  # stderr warning.
  patterns_extra: []

  # Action on Warn-severity PII match (email, ipv4):
  #   warn — copy file to mirror as-is, emit stderr warning per match (default)
  #   skip — do NOT copy the file to the mirror; emit stderr warning per match
  #   mask — copy file with each Warn match replaced by [REDACTED:<pattern_name>]
  #
  # NOTE: this setting only governs Warn-severity matches. Critical-severity
  # matches (AWS access keys, Anthropic API keys) are ALWAYS masked
  # regardless of this value — the security floor that prevents real
  # credentials from entering the raw mirror in a recoverable form is
  # non-negotiable. Set to `mask` for the legacy v3-config behavior of
  # masking everything (Warn matches included).
  on_hit: warn

# Claude Code endpoint + per-verb agent config. Two profiles are supported:
#   system — use the user's globally configured Claude CLI endpoint (no env
#            injection). `model` is a closed enum: opus-4-7 / opus-4-6 /
#            haiku-4-5 / sonnet-4-6 (codebus translates to the right --model
#            flag).
#   azure  — talk to an Azure AI Foundry Anthropic-compatible endpoint.
#            `model` is the Azure deployment name (a free string, passed
#            verbatim). API key is read from the OS keyring; codebus
#            injects ANTHROPIC_BASE_URL / ANTHROPIC_API_KEY /
#            CLAUDE_CODE_DISABLE_ADVISOR_TOOL into the child process only —
#            never modifies the parent shell environment.
#
# The `active` selector picks which profile drives the spawn. The other
# profile is cold storage: codebus does NOT validate its fields, so you
# can park half-edited config there while iterating.
claude_code:
  active: system

  system:
    goal:
      # Reasoning-heavy ingest into the wiki — v2-verified default.
      model: opus-4-6
      effort: high
    query:
      # Read-only retrieval — fast turnaround.
      model: haiku-4-5
      effort: low
    fix:
      # Lint-and-edit loop — balanced choice.
      model: sonnet-4-6
      effort: medium
    verify:
      # Content-verify spawn for quiz / goal verbs — judges whether the
      # generated content is grounded in the source mirror / planned pages.
      # Defaults to opus-4-6 + high effort: the "expensive verification"
      # half of the "cheap generation + expensive verification" pattern.
      # Override to haiku-4-5 + low if you want verify to share the cheap
      # profile of your main spawn (defeats the cost design but valid).
      model: opus-4-6
      effort: high

  # Uncomment + fill in to use Azure endpoints. Run
  #   codebus config set-key azure
  # to store the API key in your OS keyring.
  # azure:
  #   base_url: https://<your-resource>.cognitiveservices.azure.com/anthropic
  #   keyring_service: codebus-azure
  #   goal:   { model: <your-opus-deployment-name>,   effort: high   }
  #   query:  { model: <your-haiku-deployment-name>,  effort: low    }
  #   fix:    { model: <your-sonnet-deployment-name>, effort: medium }
  #   verify: { model: <your-opus-deployment-name>,   effort: high   }

# PreToolUse hook gates. Default behaviors are safe (block image / binary
# reads to keep the regex_basic PII filter effective); flip individual
# knobs to false at your own risk.
hooks:
  # Controls `codebus hook check-read` — the PreToolUse hook that blocks
  # the agent from reading image / PDF / binary files (extensions like
  # png / jpg / pdf / gif / webp / bmp / tiff / ico / heic / heif / avif).
  # Default true (block). Set false to let the agent ingest these files;
  # doing so bypasses the regex_basic PII filter (which only scans text).
  read_image_block: true

# Lint subsystem.
lint:
  fix:
    # Whether the post-goal lint-and-fix phase runs (and whether `codebus fix`
    # is allowed to run when invoked directly). Set false to disable both.
    enabled: true

# Run-log persistence (per-verb-invocation jsonl history).
log:
  # Sink implementation:
  #   jsonl  — append one JSON line per run to <dir>/runs-YYYY-MM-DD.jsonl
  #   none   — opt out, no log written. (Use the literal `none`; a bare YAML
  #            null literal returns a parse error and falls back to default.
  #            Same foot-gun avoidance as `pii.scanner: none`.)
  sink: jsonl

  # Output directory. Omit (or comment out) to use the per-vault default
  # <vault>/.codebus/log/. Tilde-prefixed paths expand to the home directory.
  # dir: ~/codebus-history
"#;

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

        let lf = load_lint_fix_config(&target).unwrap();
        assert!(lf.enabled);

        // v3-run-log: log section also round-trips to default.
        let lg = load_log_config(&target).unwrap();
        assert_eq!(lg, LogConfig::default());
    }
}
