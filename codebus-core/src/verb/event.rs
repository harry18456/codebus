//! `VerbEvent` — the unified event type emitted by `verb::{goal,query,fix}::run_*`
//! through the caller-supplied `on_event` closure.
//!
//! Three variants cover all observable progress during a verb run:
//! - `Banner(VerbBanner)` — lifecycle milestones (Start / SyncDone / Done / ...)
//! - `Stream(StreamEvent)` — agent stream-json events (Thought / ToolUse / ...)
//! - `Lifecycle(VerbLifecycleEvent)` — verb-specific lifecycle hooks
//!   (SpawnStart / SpawnEnd / FixIterationStart / LintFinal)
//!
//! `VerbBanner` mirrors `crate::render::Banner` with owned fields (PathBuf /
//! String) so the event can cross thread boundaries (GUI emits over Tauri
//! event bus needs `'static + Send`). `VerbBanner::as_banner` borrows back
//! into the existing `Banner<'_>` for the CLI render path so `print_banner`
//! is reused without duplicating formatting logic.
//!
//! `VerbLifecycleEvent` MAY be extended with new variants by future changes
//! following minor-version semantics — downstream match arms SHALL use a
//! non-exhaustive marker or wildcard arm to remain forward-compatible.

use crate::config::Verb;
use crate::render::Banner;
use crate::stream::StreamEvent;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Top-level event emitted by `verb::*::run_*` orchestration functions.
///
/// Serialized with `kind` discriminator + nested `data` payload
/// (`{"kind":"banner|stream|lifecycle","data":{...}}`) so events.jsonl
/// consumers (GUI, analytics) dispatch on `kind` without colliding with
/// the inner enums' own `kind` tags (`VerbBanner` / `StreamEvent` /
/// `VerbLifecycleEvent` each carry their own variant `kind`).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", content = "data", rename_all = "snake_case")]
pub enum VerbEvent {
    Banner(VerbBanner),
    Stream(StreamEvent),
    Lifecycle(VerbLifecycleEvent),
}

/// Owning mirror of [`crate::render::Banner`]. Banner is borrowed (`'a`)
/// because it's designed for the direct print path; VerbBanner is owning so
/// it can be sent across thread boundaries (GUI Tauri event emit).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum VerbBanner {
    Start {
        repo_path: PathBuf,
    },
    Goal {
        goal_text: String,
    },
    SyncStart,
    SyncDone {
        files: usize,
        mib: f64,
        elapsed_ms: u128,
    },
    PiiSummary {
        scanner: String,
        scanned: usize,
        hits: usize,
        action: String,
    },
    LintStart,
    LintDone {
        errors: usize,
        warns: usize,
        elapsed_ms: u128,
    },
    CommitDone {
        sha7: String,
    },
    Done {
        wiki_path: PathBuf,
    },
    Hint {
        wiki_path: PathBuf,
    },
}

impl VerbBanner {
    /// Borrow as a `Banner<'_>` for use with `crate::render::print_banner`.
    /// CLI thin wrappers call this in their `on_event` closure to reuse the
    /// existing terminal renderer.
    pub fn as_banner(&self) -> Banner<'_> {
        match self {
            VerbBanner::Start { repo_path } => Banner::Start { repo_path },
            VerbBanner::Goal { goal_text } => Banner::Goal { goal_text },
            VerbBanner::SyncStart => Banner::SyncStart,
            VerbBanner::SyncDone {
                files,
                mib,
                elapsed_ms,
            } => Banner::SyncDone {
                files: *files,
                mib: *mib,
                elapsed_ms: *elapsed_ms,
            },
            VerbBanner::PiiSummary {
                scanner,
                scanned,
                hits,
                action,
            } => Banner::PiiSummary {
                scanner,
                scanned: *scanned,
                hits: *hits,
                action,
            },
            VerbBanner::LintStart => Banner::LintStart,
            VerbBanner::LintDone {
                errors,
                warns,
                elapsed_ms,
            } => Banner::LintDone {
                errors: *errors,
                warns: *warns,
                elapsed_ms: *elapsed_ms,
            },
            VerbBanner::CommitDone { sha7 } => Banner::CommitDone { sha7 },
            VerbBanner::Done { wiki_path } => Banner::Done { wiki_path },
            VerbBanner::Hint { wiki_path } => Banner::Hint { wiki_path },
        }
    }
}

/// Lifecycle events specific to verb orchestration (not present in `Banner`
/// because they're not user-facing terminal lines — they're for GUI progress
/// UI). The CLI thin wrappers for goal/query/fix SHALL no-op on these
/// variants. The chat CLI command observes `PromoteSuggestion` to drive
/// its interactive `(y/n)` prompt.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum VerbLifecycleEvent {
    SpawnStart {
        verb: Verb,
    },
    SpawnEnd {
        verb: Verb,
        exit_code: Option<i32>,
    },
    FixIterationStart {
        iteration: u8,
    },
    LintFinal {
        error_count: usize,
        warn_count: usize,
    },
    /// v3-chat-verb: chat agent emitted a promote-to-wiki suggestion via
    /// the `[CODEBUS_PROMOTE_SUGGESTION] <reason>` line marker convention.
    /// Emitted exclusively by `verb::chat::run_chat_turn` when its stream
    /// parser detects the marker at the start of an assistant message.
    /// CLI / GUI consumers use this to render an interactive confirmation
    /// (e.g., `[suggest] promote to wiki? (y/n)`). The `reason` payload is
    /// the literal substring after `[CODEBUS_PROMOTE_SUGGESTION] ` up to
    /// the end of that line.
    PromoteSuggestion {
        reason: String,
    },
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::{Banner, RenderOptions, format_banner};
    use std::path::Path;

    #[test]
    fn verb_banner_round_trips_through_as_banner_for_start() {
        let owned = VerbBanner::Start {
            repo_path: PathBuf::from("/tmp/repo"),
        };
        let borrowed: Banner<'_> = owned.as_banner();
        let opts = RenderOptions::explicit(true, false, false, None);
        let rendered = format_banner(borrowed, &opts);
        // Reference rendering via Banner directly should match.
        let reference = format_banner(
            Banner::Start {
                repo_path: Path::new("/tmp/repo"),
            },
            &opts,
        );
        assert_eq!(rendered, reference);
    }

    #[test]
    fn verb_banner_round_trips_through_as_banner_for_pii_summary() {
        let owned = VerbBanner::PiiSummary {
            scanner: "regex_basic".to_string(),
            scanned: 100,
            hits: 5,
            action: "warn".to_string(),
        };
        let borrowed = owned.as_banner();
        match borrowed {
            Banner::PiiSummary {
                scanner,
                scanned,
                hits,
                action,
            } => {
                assert_eq!(scanner, "regex_basic");
                assert_eq!(scanned, 100);
                assert_eq!(hits, 5);
                assert_eq!(action, "warn");
            }
            _ => panic!("expected PiiSummary"),
        }
    }

    #[test]
    fn verb_event_variants_constructible() {
        let _ = VerbEvent::Banner(VerbBanner::SyncStart);
        let _ = VerbEvent::Stream(StreamEvent::Thought {
            text: "hello".to_string(),
        });
        let _ = VerbEvent::Lifecycle(VerbLifecycleEvent::SpawnStart { verb: Verb::Goal });
        let _ = VerbEvent::Lifecycle(VerbLifecycleEvent::SpawnEnd {
            verb: Verb::Query,
            exit_code: Some(0),
        });
        let _ = VerbEvent::Lifecycle(VerbLifecycleEvent::FixIterationStart { iteration: 2 });
        let _ = VerbEvent::Lifecycle(VerbLifecycleEvent::LintFinal {
            error_count: 0,
            warn_count: 1,
        });
        let _ = VerbEvent::Lifecycle(VerbLifecycleEvent::PromoteSuggestion {
            reason: "auth lifecycle including JWT issuance".into(),
        });
    }

    /// v3-chat-verb: pin the `PromoteSuggestion` payload shape so the chat
    /// stream parser and CLI confirmation prompt rely on a stable contract.
    #[test]
    fn verb_lifecycle_event_promote_suggestion_constructible() {
        let event = VerbLifecycleEvent::PromoteSuggestion {
            reason: "uv-lib 與 uv-child 的關係".into(),
        };
        match event {
            VerbLifecycleEvent::PromoteSuggestion { reason } => {
                assert_eq!(reason, "uv-lib 與 uv-child 的關係");
            }
            other => panic!("expected PromoteSuggestion, got {other:?}"),
        }
    }

    /// v3-chat-verb: ensure `PromoteSuggestion` round-trips through serde
    /// so events.jsonl rows for chat turns parse back cleanly (GUI / analytics
    /// consumers can rely on this).
    #[test]
    fn verb_lifecycle_event_promote_suggestion_serde_round_trip() {
        let event = VerbLifecycleEvent::PromoteSuggestion {
            reason: "auth flow".into(),
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("promote_suggestion"));
        assert!(json.contains("\"reason\":\"auth flow\""));
        let parsed: VerbLifecycleEvent = serde_json::from_str(&json).unwrap();
        match parsed {
            VerbLifecycleEvent::PromoteSuggestion { reason } => {
                assert_eq!(reason, "auth flow");
            }
            other => panic!("expected PromoteSuggestion after round-trip, got {other:?}"),
        }
    }
}
