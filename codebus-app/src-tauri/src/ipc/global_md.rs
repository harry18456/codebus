//! Manage the codebus MCP guidance block inside a client's GLOBAL instruction
//! file (claude `~/.claude/CLAUDE.md`, codex `~/.codex/AGENTS.md`).
//!
//! This is the first place codebus writes the user's *global* instruction file
//! (everything else lives inside a vault's `.codebus/`), so the contract is
//! deliberately narrow and safe:
//!   - The guidance lives between two literal markers; only the bytes between
//!     (and including) them are ever touched. Hand-written content outside the
//!     markers is preserved.
//!   - Enable is an idempotent upsert: a second enable replaces the block in
//!     place, never appends a duplicate.
//!   - Disable removes exactly the marked block (and the blank line that
//!     separated it), and is a no-op when the block — or the file — is absent.
//!   - Writes are atomic (temp file + rename) so a failure never leaves a
//!     half-written instruction file.
//!
//! Path resolution honors each client's own home-relocation env var
//! (`CLAUDE_CONFIG_DIR` / `CODEX_HOME`) so the integration respects a relocated
//! config dir and so tests can redirect the write to a tempdir.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// Opening marker of the codebus managed block.
const MARK_START: &str = "<!-- codebus:mcp:start -->";
/// Closing marker of the codebus managed block.
const MARK_END: &str = "<!-- codebus:mcp:end -->";

/// Guidance text written between the markers. Identical for both clients — the
/// MCP tools they see are the same — framing the wikis as a cross-project
/// reference library and saying when to reach for them.
const GUIDANCE_BODY: &str = "## codebus wiki library (MCP)\n\
You have a library of codebus-generated codebase wikis available via MCP \
(vault_list, wiki_list, wiki_search, wiki_read). When applying a pattern from a \
codebase you've indexed, when a cross-project reference would help, or when \
asked, run vault_list to see what's available and wiki_search across them. \
Treat the wiki as reference — verify load-bearing details against current source.";

/// The full managed block (markers + guidance) an enable writes.
fn block() -> String {
    format!("{MARK_START}\n{GUIDANCE_BODY}\n{MARK_END}")
}

/// Resolve a global instruction file path from an optional relocation env value
/// and the home directory. Pure (no env / fs access) so it is unit-testable;
/// the public resolvers feed it `std::env::var(..)` + `dirs::home_dir()`.
fn resolve(env_val: Option<String>, home: Option<PathBuf>, subdir: &str, file: &str) -> Option<PathBuf> {
    let base = match env_val {
        Some(v) if !v.trim().is_empty() => PathBuf::from(v),
        _ => home?.join(subdir),
    };
    Some(base.join(file))
}

/// claude's global memory file: `$CLAUDE_CONFIG_DIR/CLAUDE.md` when set,
/// otherwise `~/.claude/CLAUDE.md`.
pub fn claude_md_path() -> Option<PathBuf> {
    resolve(
        std::env::var("CLAUDE_CONFIG_DIR").ok(),
        dirs::home_dir(),
        ".claude",
        "CLAUDE.md",
    )
}

/// codex's global instructions file: `$CODEX_HOME/AGENTS.md` when set,
/// otherwise `~/.codex/AGENTS.md`.
pub fn codex_md_path() -> Option<PathBuf> {
    resolve(
        std::env::var("CODEX_HOME").ok(),
        dirs::home_dir(),
        ".codex",
        "AGENTS.md",
    )
}

/// Insert or replace the codebus managed block in the file at `path`.
/// Creates the file (and parent dirs) when absent. Idempotent: when the markers
/// are already present and well-ordered, the block is replaced in place;
/// otherwise it is appended after the existing content, separated by a blank
/// line. Every byte outside the markers is preserved.
pub fn upsert_block_at(path: &Path) -> io::Result<()> {
    let existing = read_or_empty(path)?;
    let blk = block();
    let updated = match marker_span(&existing) {
        Some((start, end)) => format!("{}{}{}", &existing[..start], blk, &existing[end..]),
        None => {
            let sep = if existing.is_empty() || existing.ends_with("\n\n") {
                ""
            } else if existing.ends_with('\n') {
                "\n"
            } else {
                "\n\n"
            };
            format!("{existing}{sep}{blk}\n")
        }
    };
    atomic_write(path, &updated)
}

/// Remove the codebus managed block from the file at `path`, collapsing the
/// blank line its removal would leave. No-op (Ok) when the block is absent or
/// the file does not exist. Content outside the markers is preserved.
pub fn remove_block_at(path: &Path) -> io::Result<()> {
    let existing = match fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(e),
    };
    let Some((start, end)) = marker_span(&existing) else {
        return Ok(());
    };
    // Trim the separator whitespace codebus itself added around the block; the
    // hand-written content on either side is preserved.
    let before = existing[..start].trim_end_matches('\n');
    let after = existing[end..].trim_start_matches('\n');
    let joined = match (before.is_empty(), after.is_empty()) {
        (true, true) => String::new(),
        (false, true) => format!("{before}\n"),
        (true, false) => format!("{after}\n"),
        (false, false) => format!("{before}\n\n{after}\n"),
    };
    atomic_write(path, &joined)
}

/// Byte span `[start, end)` of the managed block (markers inclusive) when both
/// markers are present and well-ordered; `None` otherwise.
fn marker_span(content: &str) -> Option<(usize, usize)> {
    let start = content.find(MARK_START)?;
    let end_marker = content[start..].find(MARK_END)? + start;
    Some((start, end_marker + MARK_END.len()))
}

fn read_or_empty(path: &Path) -> io::Result<String> {
    match fs::read_to_string(path) {
        Ok(s) => Ok(s),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(String::new()),
        Err(e) => Err(e),
    }
}

/// Write `content` to `path` atomically (temp file then rename), creating parent
/// directories as needed.
fn atomic_write(path: &Path, content: &str) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension("codebus-tmp");
    fs::write(&tmp, content)?;
    fs::rename(&tmp, path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn count(haystack: &str, needle: &str) -> usize {
        haystack.matches(needle).count()
    }

    #[test]
    fn upsert_into_missing_file_creates_one_block() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("CLAUDE.md");
        upsert_block_at(&path).unwrap();
        let body = fs::read_to_string(&path).unwrap();
        assert_eq!(count(&body, MARK_START), 1);
        assert_eq!(count(&body, MARK_END), 1);
        assert!(body.contains("codebus wiki library"));
    }

    #[test]
    fn upsert_twice_keeps_exactly_one_block() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("CLAUDE.md");
        upsert_block_at(&path).unwrap();
        upsert_block_at(&path).unwrap();
        let body = fs::read_to_string(&path).unwrap();
        assert_eq!(count(&body, MARK_START), 1, "two enables must not duplicate: {body}");
        assert_eq!(count(&body, MARK_END), 1);
    }

    #[test]
    fn upsert_preserves_hand_written_content() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("CLAUDE.md");
        fs::write(&path, "# my rules\nalways use tabs\n").unwrap();
        upsert_block_at(&path).unwrap();
        let body = fs::read_to_string(&path).unwrap();
        assert!(body.starts_with("# my rules\nalways use tabs\n"), "content clobbered: {body}");
        assert!(body.contains(MARK_START) && body.contains(MARK_END));
    }

    #[test]
    fn upsert_replaces_stale_block_in_place() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("CLAUDE.md");
        // A pre-existing codebus block carrying stale guidance text.
        fs::write(
            &path,
            format!("# rules\n\n{MARK_START}\nSTALE GUIDANCE\n{MARK_END}\n"),
        )
        .unwrap();
        upsert_block_at(&path).unwrap();
        let body = fs::read_to_string(&path).unwrap();
        assert_eq!(count(&body, MARK_START), 1, "replaced in place, not duplicated: {body}");
        assert!(!body.contains("STALE GUIDANCE"), "stale content must be replaced: {body}");
        assert!(body.contains("codebus wiki library"));
        assert!(body.starts_with("# rules\n"), "outside content preserved: {body}");
    }

    #[test]
    fn remove_deletes_block_and_restores_original() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("CLAUDE.md");
        let original = "# my rules\nalways use tabs\n";
        fs::write(&path, original).unwrap();
        upsert_block_at(&path).unwrap();
        remove_block_at(&path).unwrap();
        let body = fs::read_to_string(&path).unwrap();
        assert!(!body.contains(MARK_START) && !body.contains(MARK_END), "block not removed: {body}");
        assert_eq!(body, original, "removal must restore the original content");
    }

    #[test]
    fn remove_preserves_content_outside_markers() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("AGENTS.md");
        fs::write(
            &path,
            format!("top\n\n{MARK_START}\nblk\n{MARK_END}\n\nbottom\n"),
        )
        .unwrap();
        remove_block_at(&path).unwrap();
        let body = fs::read_to_string(&path).unwrap();
        assert!(body.contains("top"), "{body}");
        assert!(body.contains("bottom"), "{body}");
        assert!(!body.contains(MARK_START));
    }

    #[test]
    fn remove_is_noop_when_block_absent() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("CLAUDE.md");
        fs::write(&path, "# just my rules\n").unwrap();
        remove_block_at(&path).unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), "# just my rules\n");
    }

    #[test]
    fn remove_is_noop_when_file_missing() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("nope.md");
        remove_block_at(&path).expect("missing file is a no-op, not an error");
        assert!(!path.exists());
    }

    #[test]
    fn resolve_uses_env_override_when_set() {
        let p = resolve(Some("/relocated/.claude".to_string()), None, ".claude", "CLAUDE.md").unwrap();
        assert!(p.ends_with("CLAUDE.md"));
        assert!(p.to_string_lossy().contains("relocated"));
    }

    #[test]
    fn resolve_falls_back_to_home_subdir_when_env_unset() {
        let home = PathBuf::from("/home/u");
        let claude = resolve(None, Some(home.clone()), ".claude", "CLAUDE.md").unwrap();
        let codex = resolve(Some(String::new()), Some(home), ".codex", "AGENTS.md").unwrap();
        assert!(claude.ends_with("CLAUDE.md") && claude.to_string_lossy().contains(".claude"));
        // Whitespace-only env value is treated as unset → home fallback.
        assert!(codex.ends_with("AGENTS.md") && codex.to_string_lossy().contains(".codex"));
    }
}
