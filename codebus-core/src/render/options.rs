//! Terminal output styling decisions captured once at process start.
//!
//! [`RenderOptions`] is a plain struct (no trait, no factory) carrying three
//! independent capability flags plus an optional Obsidian vault id. The
//! capability flags are derived from the environment by [`RenderOptions::detect`]
//! per the `Environment-Aware Output Styling` requirement of the `cli`
//! capability:
//!
//! - `use_emoji = std::io::stdout().is_terminal()` — non-TTY (pipe/file)
//!   forces ASCII fallback so logs / scripts don't see `🚌` glyphs
//! - `use_color = use_emoji && env::var_os("NO_COLOR").is_none()` — TTY +
//!   `NO_COLOR` env unset; community-standard color disable knob
//! - `use_hyperlinks = use_color && supports_hyperlinks::on(stdout)` —
//!   color enabled + terminal advertises OSC 8 capability via the
//!   `supports-hyperlinks` crate
//!
//! No `~/.codebus/config.yaml` field, no `--emoji`/`--no-emoji` flag, no
//! `NO_EMOJI` env variable governs these flags. This is a deliberate
//! simplification away from v2's 5-level priority chain — the discuss
//! session resolved that user-tunable styling adds complexity without a
//! real demand.

use std::io::IsTerminal;

/// Process-wide output styling capabilities.
#[derive(Debug, Clone)]
pub struct RenderOptions {
    pub use_emoji: bool,
    pub use_color: bool,
    pub use_hyperlinks: bool,
    /// Obsidian vault id used to build OSC 8 hyperlink URLs of the form
    /// `obsidian://open?vault=<id>&file=<rel>`. `None` disables OSC 8
    /// emission for the lint output even when `use_hyperlinks` is true.
    pub vault_id: Option<String>,
    /// When true, the agent-stream renderer surfaces complete tool input and
    /// complete tool result without summarization / truncation / suppression
    /// (the CLI sets this from the `--debug` flag). Defaults to false in every
    /// constructor, preserving the compact rendering as the default mode.
    pub verbose: bool,
}

impl RenderOptions {
    /// Detect capabilities from the current process environment. Call once
    /// at the verb command's entry; do NOT re-detect per banner — the
    /// `Detection runs once per process` scenario forbids it.
    pub fn detect() -> Self {
        Self::detect_with_vault_id(None)
    }

    /// Same as [`RenderOptions::detect`] but seeds the `vault_id` field.
    /// Used by `codebus lint` after calling `obsidian_register::lookup_vault_id`
    /// so the lint output can emit OSC 8 hyperlinks pointing at the
    /// Obsidian vault opener.
    pub fn detect_with_vault_id(vault_id: Option<String>) -> Self {
        let use_emoji = std::io::stdout().is_terminal();
        let use_color = use_emoji && std::env::var_os("NO_COLOR").is_none();
        let use_hyperlinks =
            use_color && supports_hyperlinks::on(supports_hyperlinks::Stream::Stdout);
        Self {
            use_emoji,
            use_color,
            use_hyperlinks,
            vault_id,
            verbose: false,
        }
    }

    /// All-off variant. Used for tests, fixture comparisons, and the
    /// pre-existing `format_text` byte-equal compatibility wrapper inside
    /// `wiki::lint::output`.
    pub fn no_styling() -> Self {
        Self {
            use_emoji: false,
            use_color: false,
            use_hyperlinks: false,
            vault_id: None,
            verbose: false,
        }
    }

    /// Test seam: explicit construction for unit tests that need to drive
    /// every flag combination without depending on the live environment.
    /// Production code should use [`detect`](Self::detect) or
    /// [`no_styling`](Self::no_styling).
    pub fn explicit(
        use_emoji: bool,
        use_color: bool,
        use_hyperlinks: bool,
        vault_id: Option<String>,
    ) -> Self {
        Self {
            use_emoji,
            use_color,
            use_hyperlinks,
            vault_id,
            verbose: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Spec scenario: "no_styling returns all false"
    #[test]
    fn no_styling_returns_all_false() {
        let opts = RenderOptions::no_styling();
        assert!(!opts.use_emoji);
        assert!(!opts.use_color);
        assert!(!opts.use_hyperlinks);
        assert!(opts.vault_id.is_none());
    }

    /// Spec scenario: "Detection runs once per process" — `detect()` returns
    /// a `Clone`able struct so callers can pass shared snapshots through
    /// the run without re-detection. We assert the struct supports `Clone`
    /// at compile time via the `Clone` trait bound.
    #[test]
    fn detect_returns_clonable_snapshot() {
        // detect() reads stdin/env at call time; we don't assert specific
        // values (CI may run with redirected stdout), only that the call
        // succeeds and the result clones cleanly.
        let opts = RenderOptions::detect();
        let cloned = opts.clone();
        assert_eq!(opts.use_emoji, cloned.use_emoji);
        assert_eq!(opts.use_color, cloned.use_color);
        assert_eq!(opts.use_hyperlinks, cloned.use_hyperlinks);
        assert_eq!(opts.vault_id, cloned.vault_id);
    }

    /// Spec scenario: "NO_COLOR disables ANSI color but keeps emoji" —
    /// indirectly. Direct env-mutation tests are flaky in parallel runs;
    /// here we test the explicit constructor instead, which composes the
    /// same shape as the env-driven detect path.
    #[test]
    fn explicit_constructor_carries_all_four_fields() {
        let opts = RenderOptions::explicit(true, false, false, Some("vid".into()));
        assert!(opts.use_emoji);
        assert!(!opts.use_color);
        assert!(!opts.use_hyperlinks);
        assert_eq!(opts.vault_id.as_deref(), Some("vid"));
    }

    /// Coherence: hyperlinks SHALL NOT be true when color is false (per
    /// detect()'s derivation rule). Explicit constructor allows incoherent
    /// combos for test purposes — production code goes through detect().
    #[test]
    fn detect_invariant_hyperlinks_implies_color() {
        let opts = RenderOptions::detect();
        if opts.use_hyperlinks {
            assert!(
                opts.use_color,
                "hyperlinks=true requires color=true in detect()"
            );
        }
    }

    #[test]
    fn detect_invariant_color_implies_emoji() {
        let opts = RenderOptions::detect();
        if opts.use_color {
            assert!(
                opts.use_emoji,
                "color=true requires emoji=true (TTY) in detect()"
            );
        }
    }

    #[test]
    fn detect_with_vault_id_carries_through() {
        let opts = RenderOptions::detect_with_vault_id(Some("my-vault".into()));
        assert_eq!(opts.vault_id.as_deref(), Some("my-vault"));
    }

    /// cli-debug-stream-detail: verbose defaults to false in every
    /// constructor so compact rendering stays the default mode.
    #[test]
    fn verbose_defaults_to_false() {
        assert!(!RenderOptions::detect().verbose);
        assert!(!RenderOptions::no_styling().verbose);
        assert!(!RenderOptions::detect_with_vault_id(Some("v".into())).verbose);
        assert!(!RenderOptions::explicit(true, true, false, None).verbose);
    }
}
