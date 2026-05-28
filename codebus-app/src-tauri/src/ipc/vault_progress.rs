//! Tauri-layer mapping from `codebus_core::vault::init::InitEvent` to
//! the LoadingOverlay's normalized 6-phase progress stream.
//!
//! Lives in the app crate (not `codebus-core`) because the 6-phase
//! grouping is a UI-presentation concern — a CLI consumer would likely
//! want a different mapping. See design.md "Phase mapping 邏輯放 Tauri
//! layer 而非 codebus-core".

use codebus_core::vault::init::InitEvent;
use serde::{Deserialize, Serialize};

/// Tauri event name emitted by `add_vault_at` while `run_init` progresses.
pub const VAULT_INIT_PROGRESS_EVENT: &str = "vault-init-progress";

/// Payload of the `vault-init-progress` Tauri event. Field names are
/// snake_case to match existing IPC convention. Frontend MUST NOT branch
/// layout on `init_event_kind` — only on `phase`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub struct VaultInitProgress {
    /// LoadingOverlay phase (1..=6).
    pub phase: u8,
    /// InitEvent variant debug label (e.g. "Start", "LayoutCreated").
    pub init_event_kind: String,
    /// Milliseconds since `add_vault_at` started.
    pub elapsed_ms: u64,
}

/// Map an `InitEvent` to its LoadingOverlay phase number (1..=6).
///
/// Exhaustive `match` (no catch-all `_`) so that adding a new variant
/// to `codebus_core::vault::init::InitEvent` produces a compile-time
/// `non-exhaustive patterns` error here until this mapping is updated.
pub(crate) fn init_event_to_phase(event: &InitEvent<'_>) -> u8 {
    match event {
        InitEvent::Start { .. }
        | InitEvent::LayoutCreated { .. }
        | InitEvent::SourceGitignore { .. } => 1,
        InitEvent::PiiConfigLoadWarn { .. }
        | InitEvent::PiiPatternsExtraWarn { .. }
        | InitEvent::RawSyncDone { .. } => 2,
        InitEvent::InternalGitignoreDone { .. } | InitEvent::NestedRepoDone { .. } => 3,
        InitEvent::SchemaDone { .. }
        | InitEvent::ManifestSignal { .. }
        | InitEvent::ManifestDone { .. }
        | InitEvent::SkillBundlesDone { .. }
        | InitEvent::NavStubsDone { .. }
        | InitEvent::SettingsDone { .. } => 4,
        InitEvent::ObsidianResult { .. } | InitEvent::ObsidianSkipped => 5,
        InitEvent::StarterConfigUnavailable
        | InitEvent::StarterConfigDone { .. }
        | InitEvent::StarterConfigError { .. }
        | InitEvent::CommitDone { .. }
        | InitEvent::Finished { .. } => 6,
    }
}

/// Stable label string for an `InitEvent` variant. Embedded in the
/// `init_event_kind` field of `VaultInitProgress` so log captures retain
/// which backend step triggered each phase advance.
///
/// Exhaustive `match` for the same compile-time guarantee as
/// [`init_event_to_phase`].
pub(crate) fn init_event_label(event: &InitEvent<'_>) -> &'static str {
    match event {
        InitEvent::Start { .. } => "Start",
        InitEvent::LayoutCreated { .. } => "LayoutCreated",
        InitEvent::SourceGitignore { .. } => "SourceGitignore",
        InitEvent::PiiConfigLoadWarn { .. } => "PiiConfigLoadWarn",
        InitEvent::PiiPatternsExtraWarn { .. } => "PiiPatternsExtraWarn",
        InitEvent::RawSyncDone { .. } => "RawSyncDone",
        InitEvent::InternalGitignoreDone { .. } => "InternalGitignoreDone",
        InitEvent::NestedRepoDone { .. } => "NestedRepoDone",
        InitEvent::SchemaDone { .. } => "SchemaDone",
        InitEvent::ManifestSignal { .. } => "ManifestSignal",
        InitEvent::ManifestDone { .. } => "ManifestDone",
        InitEvent::SkillBundlesDone { .. } => "SkillBundlesDone",
        InitEvent::NavStubsDone { .. } => "NavStubsDone",
        InitEvent::SettingsDone { .. } => "SettingsDone",
        InitEvent::ObsidianResult { .. } => "ObsidianResult",
        InitEvent::ObsidianSkipped => "ObsidianSkipped",
        InitEvent::CommitDone { .. } => "CommitDone",
        InitEvent::StarterConfigUnavailable => "StarterConfigUnavailable",
        InitEvent::StarterConfigDone { .. } => "StarterConfigDone",
        InitEvent::StarterConfigError { .. } => "StarterConfigError",
        InitEvent::Finished { .. } => "Finished",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use codebus_core::vault::init::InitEvent;
    use codebus_core::vault::source_gitignore::GitignoreOutcome;
    use std::path::{Path, PathBuf};

    /// Construct one representative variant per phase using only
    /// cheap-to-build payloads. The full 22-variant mapping is enforced
    /// at compile time by the exhaustive `match` in
    /// [`init_event_to_phase`] / [`init_event_label`] — removing any
    /// arm produces `non-exhaustive patterns`.
    #[test]
    fn phase_mapping_covers_one_variant_per_phase() {
        let p = Path::new("/tmp/test-vault");

        let phase1 = InitEvent::SourceGitignore {
            outcome: GitignoreOutcome::AlreadyPresent,
        };
        assert_eq!(init_event_to_phase(&phase1), 1, "SourceGitignore → 1");
        assert_eq!(init_event_label(&phase1), "SourceGitignore");

        let phase2 = InitEvent::PiiConfigLoadWarn {
            message: "load warning".into(),
        };
        assert_eq!(init_event_to_phase(&phase2), 2, "PiiConfigLoadWarn → 2");
        assert_eq!(init_event_label(&phase2), "PiiConfigLoadWarn");

        let phase3 = InitEvent::InternalGitignoreDone {
            path: PathBuf::new(),
            required_count: 0,
        };
        assert_eq!(init_event_to_phase(&phase3), 3, "InternalGitignoreDone → 3");
        assert_eq!(init_event_label(&phase3), "InternalGitignoreDone");

        let phase4 = InitEvent::NavStubsDone {
            vault_root: p,
            written: 0,
            preserved: 0,
        };
        assert_eq!(init_event_to_phase(&phase4), 4, "NavStubsDone → 4");
        assert_eq!(init_event_label(&phase4), "NavStubsDone");

        let phase5 = InitEvent::ObsidianSkipped;
        assert_eq!(init_event_to_phase(&phase5), 5, "ObsidianSkipped → 5");
        assert_eq!(init_event_label(&phase5), "ObsidianSkipped");

        let phase6 = InitEvent::StarterConfigUnavailable;
        assert_eq!(
            init_event_to_phase(&phase6),
            6,
            "StarterConfigUnavailable → 6"
        );
        assert_eq!(init_event_label(&phase6), "StarterConfigUnavailable");
    }

    /// Variants that share a phase MUST all map to the same number.
    /// Spot-check the multi-event phases (2, 3, 6).
    #[test]
    fn phase_mapping_groups_variants_correctly() {
        let p = Path::new("/tmp/test-vault");

        assert_eq!(
            init_event_to_phase(&InitEvent::PiiPatternsExtraWarn {
                message: "extra".into()
            }),
            2,
        );
        assert_eq!(
            init_event_to_phase(&InitEvent::NestedRepoDone {
                vault_root: p,
                already_initialized: true,
            }),
            3,
        );
        assert_eq!(
            init_event_to_phase(&InitEvent::StarterConfigError {
                path: PathBuf::new(),
                message: "starter".into(),
            }),
            6,
        );
        assert_eq!(
            init_event_to_phase(&InitEvent::CommitDone {
                head_sha: "abc".into(),
                sha7: "abc".into(),
            }),
            6,
        );
    }

    /// Spec scenario "phase mapping table" example — JSON shape stable.
    #[test]
    fn payload_serde_round_trip_matches_spec_example() {
        let p = VaultInitProgress {
            phase: 3,
            init_event_kind: "NestedRepoDone".into(),
            elapsed_ms: 1200,
        };
        let json = serde_json::to_string(&p).expect("serialize ok");
        assert_eq!(
            json,
            r#"{"phase":3,"init_event_kind":"NestedRepoDone","elapsed_ms":1200}"#
        );

        let back: VaultInitProgress = serde_json::from_str(&json).expect("deserialize ok");
        assert_eq!(back, p);
    }

    #[test]
    fn event_name_constant_matches_spec() {
        assert_eq!(VAULT_INIT_PROGRESS_EVENT, "vault-init-progress");
    }
}
