//! Pre-write `wiki/index.md` and `wiki/log.md` nav stubs during init.
//!
//! Spec: `vault § Vault Layout` requires init to materialize both nav
//! files with minimal frontmatter so the existing `nav-missing` lint
//! rule does not fire on a freshly-inited vault — which previously
//! caused every first `codebus goal` run to spawn a fix-loop just to
//! create these stubs.
//!
//! Each file is checked independently for write-if-missing — user
//! customizations to either nav file are preserved across re-init.
//! Body text contains NO `[[…]]` wikilink syntax so the
//! `broken-wikilink` lint rule cannot misfire on the placeholder.

use std::fs;
use std::io;
use std::path::Path;

/// Materialize the two nav files at the wiki root when missing.
///
/// - `vault_root`: the `.codebus/` directory (e.g. `<repo>/.codebus`).
///   Nav files land at `<vault_root>/wiki/index.md` and
///   `<vault_root>/wiki/log.md`.
/// - `today_utc`: UTC date in `YYYY-MM-DD` format, used as both
///   `created` and `updated` frontmatter values.
///
/// Returns `(written, preserved)` where `written` is the number of
/// nav files newly created in this call (0, 1, or 2) and `preserved`
/// is the number that already existed and were left untouched.
pub fn write_nav_stubs_if_missing(
    vault_root: &Path,
    today_utc: &str,
) -> io::Result<(usize, usize)> {
    let wiki_root = vault_root.join("wiki");
    if !wiki_root.exists() {
        fs::create_dir_all(&wiki_root)?;
    }
    let mut written = 0usize;
    let mut preserved = 0usize;
    for (name, title) in NAV_FILES {
        let path = wiki_root.join(format!("{name}.md"));
        if path.exists() {
            preserved += 1;
            continue;
        }
        let body = nav_stub_content(name, title, today_utc);
        fs::write(&path, body)?;
        written += 1;
    }
    Ok((written, preserved))
}

/// `(file_basename, frontmatter_title)` pairs for the two nav files.
const NAV_FILES: &[(&str, &str)] = &[
    ("index", "Wiki Index"),
    ("log", "Goal Log"),
];

/// Build the minimal stub body for one nav file. Frontmatter satisfies
/// the wiki schema (`title`, `type`, `sources`, `goals`, `created`,
/// `updated`, `related`, `stale`); body is one short line containing
/// NO `[[…]]` syntax so `broken-wikilink` lint cannot misfire.
pub(crate) fn nav_stub_content(name: &str, title: &str, today_utc: &str) -> String {
    let body_hint = match name {
        "index" => "No wiki pages yet — run a goal to start documenting.",
        "log" => "No goals run yet.",
        _ => "Placeholder body.",
    };
    format!(
        "---\n\
         title: {title}\n\
         type: synthesis\n\
         sources: []\n\
         goals: []\n\
         created: '{today_utc}'\n\
         updated: '{today_utc}'\n\
         related: []\n\
         stale: false\n\
         ---\n\
         \n\
         {body_hint}\n"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    const TODAY: &str = "2026-05-14";

    /// Spec scenario: "Init materializes both nav files at the wiki
    /// root" — stub frontmatter carries the 8 required schema keys.
    #[test]
    fn nav_stub_content_index_has_required_frontmatter_keys() {
        let body = nav_stub_content("index", "Wiki Index", TODAY);
        assert!(body.starts_with("---\n"));
        for key in [
            "title:",
            "type: synthesis",
            "sources:",
            "goals:",
            "created:",
            "updated:",
            "related:",
            "stale:",
        ] {
            assert!(body.contains(key), "stub missing key `{key}`:\n{body}");
        }
        // Frontmatter delimiters present.
        assert_eq!(body.matches("---\n").count(), 2);
        // Body contains a non-empty placeholder line after the second `---`.
        let after_close = body.split_once("---\n").and_then(|(_, rest)| rest.split_once("---\n")).map(|(_, b)| b).unwrap_or("");
        assert!(
            after_close.trim().len() > 0,
            "stub body must contain a non-empty placeholder line"
        );
    }

    /// Spec scenario: "Nav placeholder body contains no wikilink syntax".
    #[test]
    fn nav_stub_content_body_has_no_wikilink_syntax() {
        for name in ["index", "log"] {
            let body = nav_stub_content(name, "Some Title", TODAY);
            assert!(
                !body.contains("[["),
                "{name} stub must not contain `[[` token:\n{body}"
            );
            assert!(
                !body.contains("]]"),
                "{name} stub must not contain `]]` token:\n{body}"
            );
        }
    }

    /// Spec scenario: "Init materializes both nav files at the wiki root"
    /// — fresh vault → both files written, outcomes == (2, 0).
    #[test]
    fn write_nav_stubs_first_run_writes_both() {
        let tmp = TempDir::new().unwrap();
        let vault_root = tmp.path().to_path_buf();
        let (written, preserved) =
            write_nav_stubs_if_missing(&vault_root, TODAY).unwrap();
        assert_eq!((written, preserved), (2, 0));
        assert!(vault_root.join("wiki/index.md").exists());
        assert!(vault_root.join("wiki/log.md").exists());
    }

    /// Spec scenario: "Nav write-if-missing preserves existing files".
    #[test]
    fn write_nav_stubs_preserves_existing_index() {
        let tmp = TempDir::new().unwrap();
        let vault_root = tmp.path().to_path_buf();
        let wiki = vault_root.join("wiki");
        fs::create_dir_all(&wiki).unwrap();
        let index_path = wiki.join("index.md");
        let custom = "---\ntitle: My Custom Index\n---\n\ncustom body\n";
        fs::write(&index_path, custom).unwrap();

        let (written, preserved) =
            write_nav_stubs_if_missing(&vault_root, TODAY).unwrap();
        assert_eq!((written, preserved), (1, 1));
        // Custom index untouched.
        assert_eq!(fs::read_to_string(&index_path).unwrap(), custom);
        // Log freshly created.
        assert!(wiki.join("log.md").exists());
    }

    /// Re-running init twice with the same caller leaves both nav files
    /// at their first-write contents (no drift).
    #[test]
    fn re_run_idempotent_for_nav_stubs() {
        let tmp = TempDir::new().unwrap();
        let vault_root = tmp.path().to_path_buf();
        write_nav_stubs_if_missing(&vault_root, TODAY).unwrap();
        let snapshot_index = fs::read(vault_root.join("wiki/index.md")).unwrap();
        let snapshot_log = fs::read(vault_root.join("wiki/log.md")).unwrap();

        let (written, preserved) =
            write_nav_stubs_if_missing(&vault_root, "2099-12-31").unwrap();
        assert_eq!((written, preserved), (0, 2));
        assert_eq!(fs::read(vault_root.join("wiki/index.md")).unwrap(), snapshot_index);
        assert_eq!(fs::read(vault_root.join("wiki/log.md")).unwrap(), snapshot_log);
    }
}
