//! Built-in `CLAUDE.md` schema written by `codebus init` into the user's
//! vault. The string content lives in `CLAUDE.md` next to this file so the
//! same source-of-truth feeds both the Rust port (`include_str!` below) and
//! the legacy TS reference impl (which `readFileSync`s the same path).
//!
//! Editing the schema = editing `CLAUDE.md`. The lock-in tests below assert
//! load-bearing phrases the agent depends on; updating them is a deliberate
//! act, not an accident.

pub const CODEBUS_SCHEMA: &str = include_str!("./CLAUDE.md");

#[cfg(test)]
mod tests {
    use super::CODEBUS_SCHEMA;

    #[test]
    fn contains_spdx_license_header() {
        assert!(CODEBUS_SCHEMA.contains("SPDX-License-Identifier: MIT"));
    }

    #[test]
    fn contains_all_twelve_schema_sections() {
        let sections = [
            "Your Role",
            "Workspace Layout",
            "Wiki Structure",
            "Workflow per Goal",
            "Page Conflict",
            "Frontmatter Schema",
            "WikiLinks",
            "Source",
            "Stopping Criteria",
            "Failure Modes",
            "Output Format",
            "Workflow per Query",
        ];
        for s in sections {
            assert!(CODEBUS_SCHEMA.contains(s), "schema missing section: {s}");
        }
    }

    #[test]
    fn warns_about_wikilink_yaml_quoting() {
        assert!(CODEBUS_SCHEMA.contains("\"[["));
        let lower = CODEBUS_SCHEMA.to_lowercase();
        assert!(
            lower.contains("quote")
                || CODEBUS_SCHEMA.contains("引號")
                || CODEBUS_SCHEMA.contains("MUST quote"),
            "schema must instruct YAML quoting for wikilinks"
        );
    }

    #[test]
    fn instructs_only_path_in_sources() {
        let lower = CODEBUS_SCHEMA.to_lowercase();
        assert!(
            lower.contains("only fill"),
            "schema must say only fill path"
        );
        assert!(lower.contains("path"));
        assert!(lower.contains("sha256"));
        assert!(lower.contains("auto-fill"));
    }

    #[test]
    fn specifies_utc_date_convention() {
        assert!(CODEBUS_SCHEMA.contains("UTC YYYY-MM-DD"));
    }

    #[test]
    fn contains_out_of_scope_detection_subsection() {
        assert!(CODEBUS_SCHEMA.contains("Out-of-scope detection"));
        assert!(CODEBUS_SCHEMA.contains("In-scope** if ANY of:"));
        assert!(CODEBUS_SCHEMA.contains("Out-of-scope** otherwise"));
    }

    #[test]
    fn contains_stop_rules_forbidding_noop_record() {
        assert!(CODEBUS_SCHEMA.contains("If out-of-scope: STOP"));
        assert!(CODEBUS_SCHEMA.contains("No `wiki/log.md` append"));
        assert!(CODEBUS_SCHEMA.contains("No `wiki/index.md` modification"));
        // Retired forms — these MUST NOT come back without an explicit
        // schema review (they were removed in wiki-taxonomy-realign).
        assert!(!CODEBUS_SCHEMA.contains("No `wiki/overview.md` update"));
        assert!(!CODEBUS_SCHEMA.contains("no \"no-op record\" goal-guide"));
    }

    #[test]
    fn schema_is_non_trivial_size() {
        // Guards against an empty / partially copied .md file at build time.
        // Current schema is ~12K bytes; threshold catches catastrophic
        // truncation without being brittle to edits.
        assert!(
            CODEBUS_SCHEMA.len() > 8_000,
            "schema unexpectedly short: {} bytes",
            CODEBUS_SCHEMA.len()
        );
    }
}
