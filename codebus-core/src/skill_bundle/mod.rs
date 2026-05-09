//! Write 3 skill bundle stubs to `<vault_root>/.claude/skills/codebus-{goal,query,fix}/`.
//! Vault-internal: skills live UNDER the `.codebus/` directory so that an
//! agentic CLI invoked with cwd=`<vault_root>` discovers them via standard
//! `<cwd>/.claude/skills/` lookup. This keeps agent's read scope naturally
//! constrained to the vault (cwd-bounded) — paths inside SKILL.md are
//! cwd-relative (`raw/code/`, `wiki/`, `CLAUDE.md`), NOT `.codebus/`-prefixed.
//!
//! Full per-verb workflow lands in #4 / #5 / #7. Write-if-missing semantics
//! preserve user customization across re-inits.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BundleOutcome {
    Written,
    AlreadyPresent,
}

pub const VERBS: &[&str] = &["goal", "query", "fix"];

/// Materialize the three skill bundle stubs under `<vault_root>/.claude/skills/`.
/// `vault_root` is the `.codebus/` directory (NOT the source repo root).
/// Returns one outcome per verb in `VERBS` order.
pub fn write_bundles_if_missing(vault_root: &Path) -> io::Result<Vec<BundleOutcome>> {
    let mut outcomes = Vec::with_capacity(VERBS.len());
    for verb in VERBS {
        outcomes.push(write_bundle_if_missing(vault_root, verb)?);
    }
    Ok(outcomes)
}

pub fn skill_bundle_path(vault_root: &Path, verb: &str) -> PathBuf {
    vault_root
        .join(".claude")
        .join("skills")
        .join(format!("codebus-{verb}"))
        .join("SKILL.md")
}

fn write_bundle_if_missing(vault_root: &Path, verb: &str) -> io::Result<BundleOutcome> {
    let path = skill_bundle_path(vault_root, verb);
    if path.exists() {
        return Ok(BundleOutcome::AlreadyPresent);
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&path, stub_content(verb))?;
    Ok(BundleOutcome::Written)
}

fn stub_content(verb: &str) -> String {
    let description = match verb {
        "goal" => "Trigger codebus goal-ingest workflow on the active codebus vault",
        "query" => "Trigger codebus read-only wiki query workflow on the active codebus vault",
        "fix" => "Trigger codebus lint-feedback fix loop on the active codebus vault",
        _ => "codebus skill",
    };
    let workflow = workflow_section(verb);
    format!(
        "---\n\
         name: codebus-{verb}\n\
         description: {description}\n\
         ---\n\
         \n\
         # codebus-{verb}\n\
         \n\
         Trigger this skill when the user types `/codebus-{verb}` (typically the codebus binary spawns the agentic CLI with cwd at this vault root for you).\n\
         \n\
         ## Schema rules\n\
         \n\
         The current working directory is the codebus vault root. Read `CLAUDE.md` here for taxonomy, frontmatter, and wikilinks rules — that file is the single source of truth for vault structure.\n\
         \n\
         ## Hard scope\n\
         \n\
         Read scope: `raw/code/` (relative to cwd) — the PII-redacted source mirror. Do NOT navigate outside cwd; the user's source repo at the parent directory level is off-limits.\n\
         \n\
         Write scope: `wiki/` (relative to cwd) — wiki pages, `wiki/index.md`, `wiki/log.md`.\n\
         \n\
         You MUST NOT read or write any path that escapes the cwd (no `..`, no absolute paths to outside locations).\n\
         \n\
         ## Path translation\n\
         \n\
         When citing source files in wiki page frontmatter `sources[].path`, use the **repo-relative logical path** (e.g. `src/services/payment.py`), NOT the mirrored path (e.g. `raw/code/src/services/payment.py`). Wikilinks resolve by filename across folders, so the path naming has to be logical/source-relative for cross-vault link conventions to hold.\n\
         \n\
         {workflow}"
    )
}

/// `## Workflow` section per verb. Goal carries the 5-step ingest content
/// landed by v3-goal #5; query / fix retain their stub placeholder until
/// #6 / #8 land.
fn workflow_section(verb: &str) -> String {
    match verb {
        "goal" => GOAL_WORKFLOW.to_string(),
        _ => format!(
            "## Workflow\n\
             \n\
             Detailed workflow content lands in a subsequent codebus release. For now, follow the schema rules in `CLAUDE.md` and apply common sense for the `{verb}` action while respecting the hard scope above.\n"
        ),
    }
}

/// 5-step ingest workflow for the goal verb. SKILL.md is an "internal surface"
/// (consumed by the agent, not by the user) per the vault `CLAUDE.md` §0
/// Language Policy → workflow body stays in English to keep token cost
/// compact and prevent literal phrases from leaking into user-facing
/// surfaces. Step 5 deliberately avoids any literal sample summary string
/// the agent could copy verbatim; it describes the desired output shape and
/// defers the output language to `CLAUDE.md` §0.
///
/// Schema rules (taxonomy definitions, frontmatter format, wikilink
/// resolution) stay in `CLAUDE.md` (cwd-relative); this section
/// orchestrates the ingest dance only. Step 2 enumerates the five taxonomy
/// folder names so the agent knows where pages go, but type definitions
/// are not duplicated here.
const GOAL_WORKFLOW: &str = "## Workflow (per-goal ingest)

When this skill is activated, follow these 5 steps in order:

1. **Explore raw**: use Glob / Read on `raw/code/` to locate sources relevant to the goal. Do not read every file end-to-end — scan entry / module-level structure.

2. **Plan pages**: cross-reference existing pages under `wiki/`. Decide which pages to create vs update. Page placements live under five taxonomy folders: `concepts/`, `entities/`, `modules/`, `processes/`, `synthesis/`; each folder's page-type definition lives in cwd `CLAUDE.md`.

3. **Write frontmatter + body**: every new page MUST carry frontmatter (taxonomy / sources / etc.) and a body. Frontmatter required fields and format come from `CLAUDE.md`; this SKILL.md does not duplicate them.

4. **Build wikilinks**: link pages with `[[other-page]]`. When linking to an existing page use that page's filename only (no path); cross-folder resolution is handled by the schema convention.

5. **Print closing summary**: emit ONE short stdout line stating how many pages were created vs how many were modified in this run. Phrase the line in the same natural language as the goal text per the §0 Language Policy in cwd `CLAUDE.md` (so a goal in Japanese gets a Japanese summary, a goal in English gets an English one, etc.). The agent MUST NOT copy phrasing from this SKILL.md verbatim into the stdout summary; this paragraph describes the output shape only and is not itself a template.
";

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn first_time_writes_three_bundles_under_vault_dot_claude() {
        let tmp = TempDir::new().unwrap();
        let outcomes = write_bundles_if_missing(tmp.path()).unwrap();
        assert_eq!(outcomes, vec![BundleOutcome::Written; 3]);
        for verb in VERBS {
            let p = skill_bundle_path(tmp.path(), verb);
            assert!(p.exists(), "missing bundle for verb `{verb}`");
            // Path is `<vault_root>/.claude/skills/codebus-{verb}/SKILL.md`
            let s = p.to_string_lossy();
            assert!(s.contains(".claude") && s.contains("skills") && s.contains(&format!("codebus-{verb}")));
            let body = fs::read_to_string(&p).unwrap();
            assert!(body.starts_with("---\n"));
            assert!(body.contains(&format!("name: codebus-{verb}")));
            assert!(body.lines().count() <= 80);
        }
    }

    #[test]
    fn does_not_create_codebus_lint_bundle() {
        let tmp = TempDir::new().unwrap();
        write_bundles_if_missing(tmp.path()).unwrap();
        let lint_dir = tmp.path().join(".claude/skills/codebus-lint");
        assert!(!lint_dir.exists());
    }

    #[test]
    fn preserves_user_modified_bundle() {
        let tmp = TempDir::new().unwrap();
        let goal_path = skill_bundle_path(tmp.path(), "goal");
        fs::create_dir_all(goal_path.parent().unwrap()).unwrap();
        let custom = "---\nname: codebus-goal\ndescription: my custom\n---\n\n# my workflow\n";
        fs::write(&goal_path, custom).unwrap();

        let outcomes = write_bundles_if_missing(tmp.path()).unwrap();
        assert_eq!(outcomes[0], BundleOutcome::AlreadyPresent);
        assert_eq!(outcomes[1], BundleOutcome::Written);
        assert_eq!(outcomes[2], BundleOutcome::Written);
        assert_eq!(fs::read_to_string(&goal_path).unwrap(), custom);
    }

    #[test]
    fn mixed_state_writes_only_missing() {
        let tmp = TempDir::new().unwrap();
        let goal_path = skill_bundle_path(tmp.path(), "goal");
        fs::create_dir_all(goal_path.parent().unwrap()).unwrap();
        fs::write(&goal_path, "preserved").unwrap();

        let outcomes = write_bundles_if_missing(tmp.path()).unwrap();
        assert_eq!(outcomes[0], BundleOutcome::AlreadyPresent);
        assert_eq!(outcomes[1], BundleOutcome::Written);
        assert_eq!(outcomes[2], BundleOutcome::Written);
        assert_eq!(fs::read_to_string(&goal_path).unwrap(), "preserved");
    }

    #[test]
    fn each_stub_body_uses_cwd_relative_paths_not_dot_codebus_prefixed() {
        let tmp = TempDir::new().unwrap();
        write_bundles_if_missing(tmp.path()).unwrap();
        for verb in VERBS {
            let body = fs::read_to_string(skill_bundle_path(tmp.path(), verb)).unwrap();
            // Hard-scope must reference cwd-relative paths, NOT `.codebus/`-prefixed
            // (the agent's cwd IS the vault root, so `.codebus/` prefix is wrong)
            assert!(
                !body.contains(".codebus/raw/code/") && !body.contains(".codebus/wiki/"),
                "verb `{verb}` body uses .codebus/-prefixed paths but should be cwd-relative: {body}"
            );
            assert!(
                body.contains("`raw/code/`"),
                "verb `{verb}` missing cwd-relative read scope `raw/code/`"
            );
            assert!(
                body.contains("`wiki/`"),
                "verb `{verb}` missing cwd-relative write scope `wiki/`"
            );
            assert!(
                body.contains("MUST NOT read or write any path that escapes the cwd"),
                "verb `{verb}` missing escape-prohibition"
            );
        }
    }

    #[test]
    fn each_stub_body_declares_path_translation_rule() {
        let tmp = TempDir::new().unwrap();
        write_bundles_if_missing(tmp.path()).unwrap();
        for verb in VERBS {
            let body = fs::read_to_string(skill_bundle_path(tmp.path(), verb)).unwrap();
            assert!(body.contains("repo-relative logical path"));
            assert!(body.contains("NOT the mirrored path"));
        }
    }

    #[test]
    fn skill_bundle_path_resolves_under_vault_dot_claude_skills() {
        let p = skill_bundle_path(Path::new("/some/repo/.codebus"), "goal");
        let s = p.to_string_lossy();
        assert!(s.contains("/some/repo/.codebus") || s.contains("\\some\\repo\\.codebus"));
        assert!(s.contains(".claude"));
        assert!(s.contains("skills"));
        assert!(s.contains("codebus-goal"));
        assert!(s.ends_with("SKILL.md"));
    }

    #[test]
    fn goal_workflow_body_is_english() {
        // Spec scenario: codebus-goal workflow body is written in English.
        // Internal surface per CLAUDE.md §0 Language Policy → no CJK
        // Unified Ideographs (U+4E00..U+9FFF) anywhere in the body.
        let body = stub_content("goal");
        let cjk: Vec<char> = body
            .chars()
            .filter(|c| ('\u{4E00}'..='\u{9FFF}').contains(c))
            .collect();
        assert!(
            cjk.is_empty(),
            "goal SKILL.md body must not contain CJK ideographs, found {} (first 10: {:?})",
            cjk.len(),
            cjk.iter().take(10).collect::<Vec<_>>()
        );
    }

    #[test]
    fn goal_step_5_has_no_literal_summary_template() {
        // Spec scenario: step 5 instruction is abstract, not a literal
        // output template. The body MUST NOT contain canned summary
        // phrases the agent could copy verbatim into stdout, MUST
        // reference CLAUDE.md as the language source-of-truth, AND MUST
        // include an explicit "do not copy verbatim" directive.
        let body = stub_content("goal");

        // Forbidden literal sample phrases that the agent could parrot
        // into the user-facing stdout summary. v3-goal smoke (2026-05-09)
        // showed that any such literal — Chinese, Japanese, Korean, or
        // English — leaks to the wrong audience for some goal language.
        let forbidden_literals = [
            "Added N pages",
            "Added 4 pages",
            "modified 0 pages",
            "created N pages",
            "\u{672C}\u{6B21}\u{65B0}\u{589E}",       // 本次新增 (Chinese)
            "\u{30DA}\u{30FC}\u{30B8}\u{3092}\u{8FFD}\u{52A0}", // ページを追加 (Japanese)
            "\u{C774}\u{BC88}\u{C5D0}\u{20}\u{C0C8}\u{B85C}",  // 이번에 새로 (Korean)
        ];
        for phrase in forbidden_literals {
            assert!(
                !body.contains(phrase),
                "goal SKILL.md body contains literal summary template `{phrase}` — step 5 must be abstract"
            );
        }

        // Required references.
        assert!(
            body.contains("CLAUDE.md"),
            "step 5 must reference CLAUDE.md as the language source-of-truth"
        );
        assert!(
            body.contains("verbatim"),
            "step 5 must include an explicit `verbatim` warning that agents must not copy from this SKILL.md"
        );
    }
}
