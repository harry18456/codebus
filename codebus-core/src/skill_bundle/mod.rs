//! Write 3 skill bundle stubs to TWO locations per v3-lint Skill Bundle Layout:
//! - vault-internal: `<repo>/.codebus/.claude/skills/codebus-{goal,query,fix}/`
//!   (used when CLI spawns agent with cwd=vault root)
//! - repo-root: `<repo>/.claude/skills/codebus-{goal,query,fix}/`
//!   (used when user opens Claude Code at source repo root and invokes
//!    `/codebus-{verb}` directly)
//!
//! Both copies have byte-identical SKILL.md content. SKILL.md paths are
//! cwd-relative (`raw/code/`, `wiki/`, `CLAUDE.md`) — vault auto-detection
//! in lint/fix handles cwd disambiguation rather than pre-baking absolute
//! paths into SKILL.md.
//!
//! Write-if-missing semantics preserve user customization across re-inits;
//! each location is checked independently.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BundleOutcome {
    Written,
    AlreadyPresent,
}

pub const VERBS: &[&str] = &["goal", "query", "fix"];

/// Materialize the three skill bundle stubs at BOTH the vault-internal and
/// the source repo-root locations.
///
/// - `vault_root`: the `.codebus/` directory (e.g. `<repo>/.codebus`). Stubs
///   land at `<vault_root>/.claude/skills/codebus-{verb}/SKILL.md`.
/// - `repo_root`: the source repository root (e.g. `<repo>`). Stubs ALSO land
///   at `<repo_root>/.claude/skills/codebus-{verb}/SKILL.md` so users opening
///   Claude Code at repo root discover the skill via `<cwd>/.claude/skills/`.
///
/// Each location is checked independently for write-if-missing — preserving
/// user customizations at one location doesn't block writing the missing
/// peer at the other location.
///
/// Returns 6 outcomes: 3 for vault-internal followed by 3 for repo-root, in
/// `VERBS` order at each location.
pub fn write_bundles_if_missing(
    vault_root: &Path,
    repo_root: &Path,
) -> io::Result<Vec<BundleOutcome>> {
    let mut outcomes = Vec::with_capacity(VERBS.len() * 2);
    for verb in VERBS {
        outcomes.push(write_bundle_if_missing(
            &skill_bundle_path(vault_root, verb),
            verb,
        )?);
    }
    for verb in VERBS {
        outcomes.push(write_bundle_if_missing(
            &skill_bundle_path(repo_root, verb),
            verb,
        )?);
    }
    Ok(outcomes)
}

/// `<base>/.claude/skills/codebus-<verb>/SKILL.md`. `base` is the location
/// root — vault-internal callers pass the `.codebus/` path; repo-root
/// callers pass the source repository root.
pub fn skill_bundle_path(base: &Path, verb: &str) -> PathBuf {
    base.join(".claude")
        .join("skills")
        .join(format!("codebus-{verb}"))
        .join("SKILL.md")
}

fn write_bundle_if_missing(path: &Path, verb: &str) -> io::Result<BundleOutcome> {
    if path.exists() {
        return Ok(BundleOutcome::AlreadyPresent);
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, stub_content(verb))?;
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
/// (v3-goal #5); query carries the 4-step read-only lookup content
/// (v3-query #6); fix retains its stub placeholder until v3-fix #8 lands.
fn workflow_section(verb: &str) -> String {
    match verb {
        "goal" => GOAL_WORKFLOW.to_string(),
        "query" => QUERY_WORKFLOW.to_string(),
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

## Language Override

The goal text's language SHALL override the natural language of any existing wiki page or raw source content read in steps 1-2. When appending a `## from goal: ...` section to an existing page authored in a different language, the new section's body language follows the goal text, not the existing page's language. The agent reads existing pages to know what already exists, not to imitate their writing language.

";


/// 4-step read-only lookup workflow for the query verb. SKILL.md is an
/// "internal surface" per cwd `CLAUDE.md` §0 Language Policy → workflow
/// body stays in English. Step 4 deliberately avoids any literal sample
/// answer phrase the agent could copy verbatim; it describes the output
/// shape and defers the answer language to `CLAUDE.md` §0.
///
/// The workflow restates the read-only invariant (no Write/Edit) for
/// defense-in-depth even though the binary layer's `--tools Read,Glob,Grep`
/// already gates Write/Edit out of the toolset at runtime.
const QUERY_WORKFLOW: &str = "## Workflow (per-query lookup)

When this skill is activated, follow these 4 steps in order:

1. **Parse the query**: parse the user's question text. Identify which taxonomy folders under `wiki/` (`concepts/`, `entities/`, `modules/`, `processes/`, `synthesis/`) are most likely relevant given the question's subject.

2. **Find candidate pages**: use Glob and Read to scan `wiki/` for pages whose frontmatter (title, sources, related) matches the query. Read frontmatter first as a lightweight relevance filter; only read body when the frontmatter signals a match.

3. **Follow wikilinks**: from matched pages, follow `[[other-page]]` references to assemble cross-page context. Bound the traversal to 1-2 hops so the lookup does not drift across the whole vault.

4. **Print the answer**: emit ONE coherent answer to stdout. Phrase the answer in the same natural language as the query text per the §0 Language Policy in cwd `CLAUDE.md` (so a Japanese query gets a Japanese answer, an English query gets an English one, etc.). The agent MUST NOT copy phrasing from this SKILL.md verbatim into the stdout answer; this paragraph describes the output shape only and is not itself a template.

## Read-Only Invariant

This workflow is strictly read-only. The agent MUST NOT use Write or Edit to mutate any file inside `wiki/`, `raw/`, or anywhere else inside the vault. Note that the toolset is also gated at the binary layer (`--tools Read,Glob,Grep` was passed when this agent was spawned, so Write and Edit attempts will fail at runtime), but this SKILL.md restates the invariant for defense-in-depth.

## Language Override

The query text's language SHALL override the natural language of any wiki content read in steps 2-3. When matched pages are authored in a different language than the query, the answer in step 4 SHALL match the query's language regardless. The agent reads `wiki/` to retrieve information, not to imitate the wiki's writing language.

";

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    /// Helper: build a (vault_root, repo_root) pair under a single TempDir
    /// so tests assert dual-location behavior (vault-internal under
    /// `<tmp>/.codebus`, repo-root at `<tmp>` itself).
    fn dual_layout(tmp: &TempDir) -> (PathBuf, PathBuf) {
        let repo_root = tmp.path().to_path_buf();
        let vault_root = repo_root.join(".codebus");
        (vault_root, repo_root)
    }

    #[test]
    fn first_time_writes_three_bundles_at_both_locations() {
        let tmp = TempDir::new().unwrap();
        let (vault, repo) = dual_layout(&tmp);
        let outcomes = write_bundles_if_missing(&vault, &repo).unwrap();
        // 6 outcomes: 3 vault-internal + 3 repo-root, all Written
        assert_eq!(outcomes, vec![BundleOutcome::Written; 6]);
        for verb in VERBS {
            for base in [&vault, &repo] {
                let p = skill_bundle_path(base, verb);
                assert!(p.exists(), "missing bundle for verb `{verb}` at {base:?}");
                let s = p.to_string_lossy();
                assert!(s.contains(".claude") && s.contains("skills") && s.contains(&format!("codebus-{verb}")));
                let body = fs::read_to_string(&p).unwrap();
                assert!(body.starts_with("---\n"));
                assert!(body.contains(&format!("name: codebus-{verb}")));
                assert!(body.lines().count() <= 80);
            }
        }
    }

    #[test]
    fn vault_and_repo_root_skill_md_byte_identical() {
        let tmp = TempDir::new().unwrap();
        let (vault, repo) = dual_layout(&tmp);
        write_bundles_if_missing(&vault, &repo).unwrap();
        for verb in VERBS {
            let vault_body = fs::read(skill_bundle_path(&vault, verb)).unwrap();
            let repo_body = fs::read(skill_bundle_path(&repo, verb)).unwrap();
            assert_eq!(
                vault_body, repo_body,
                "verb `{verb}` SKILL.md must be byte-identical at both locations"
            );
        }
    }

    #[test]
    fn does_not_create_codebus_lint_bundle_at_either_location() {
        let tmp = TempDir::new().unwrap();
        let (vault, repo) = dual_layout(&tmp);
        write_bundles_if_missing(&vault, &repo).unwrap();
        assert!(!vault.join(".claude/skills/codebus-lint").exists());
        assert!(!repo.join(".claude/skills/codebus-lint").exists());
    }

    #[test]
    fn write_if_missing_skips_existing_at_vault_only() {
        let tmp = TempDir::new().unwrap();
        let (vault, repo) = dual_layout(&tmp);
        let goal_vault = skill_bundle_path(&vault, "goal");
        fs::create_dir_all(goal_vault.parent().unwrap()).unwrap();
        let custom = "---\nname: codebus-goal\ndescription: my custom\n---\n\n# my workflow\n";
        fs::write(&goal_vault, custom).unwrap();

        let outcomes = write_bundles_if_missing(&vault, &repo).unwrap();
        // Vault: goal AlreadyPresent, query/fix Written
        assert_eq!(outcomes[0], BundleOutcome::AlreadyPresent);
        assert_eq!(outcomes[1], BundleOutcome::Written);
        assert_eq!(outcomes[2], BundleOutcome::Written);
        // Repo-root: all Written (independent check)
        assert_eq!(outcomes[3], BundleOutcome::Written);
        assert_eq!(outcomes[4], BundleOutcome::Written);
        assert_eq!(outcomes[5], BundleOutcome::Written);
        // Custom vault content preserved
        assert_eq!(fs::read_to_string(&goal_vault).unwrap(), custom);
    }

    #[test]
    fn write_if_missing_only_fills_missing_repo_root_when_vault_exists() {
        let tmp = TempDir::new().unwrap();
        let (vault, repo) = dual_layout(&tmp);
        // Pre-populate full vault (all 3 verbs)
        write_bundles_if_missing(&vault, &repo).unwrap();
        // Wipe repo-root copy of one verb
        fs::remove_file(skill_bundle_path(&repo, "query")).unwrap();
        // Re-run: vault all AlreadyPresent, repo-root: query Written, others
        // AlreadyPresent
        let outcomes = write_bundles_if_missing(&vault, &repo).unwrap();
        assert_eq!(outcomes[0], BundleOutcome::AlreadyPresent); // vault goal
        assert_eq!(outcomes[1], BundleOutcome::AlreadyPresent); // vault query
        assert_eq!(outcomes[2], BundleOutcome::AlreadyPresent); // vault fix
        assert_eq!(outcomes[3], BundleOutcome::AlreadyPresent); // repo goal
        assert_eq!(outcomes[4], BundleOutcome::Written);        // repo query (refilled)
        assert_eq!(outcomes[5], BundleOutcome::AlreadyPresent); // repo fix
    }

    #[test]
    fn each_stub_body_uses_cwd_relative_paths_not_dot_codebus_prefixed() {
        let tmp = TempDir::new().unwrap();
        let (vault, repo) = dual_layout(&tmp);
        write_bundles_if_missing(&vault, &repo).unwrap();
        for verb in VERBS {
            let body = fs::read_to_string(skill_bundle_path(&vault, verb)).unwrap();
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
        let (vault, repo) = dual_layout(&tmp);
        write_bundles_if_missing(&vault, &repo).unwrap();
        for verb in VERBS {
            let body = fs::read_to_string(skill_bundle_path(&vault, verb)).unwrap();
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
    fn query_workflow_body_is_english() {
        // Spec scenario: codebus-query workflow body is written in English.
        // Internal surface per CLAUDE.md §0 Language Policy → no CJK
        // Unified Ideographs (U+4E00..U+9FFF) anywhere in the body.
        let body = stub_content("query");
        let cjk: Vec<char> = body
            .chars()
            .filter(|c| ('\u{4E00}'..='\u{9FFF}').contains(c))
            .collect();
        assert!(
            cjk.is_empty(),
            "query SKILL.md body must not contain CJK ideographs, found {} (first 10: {:?})",
            cjk.len(),
            cjk.iter().take(10).collect::<Vec<_>>()
        );
    }

    #[test]
    fn query_step_4_has_no_literal_template() {
        // Spec scenario: step 4 is abstract, not a literal output template.
        // Body MUST NOT contain canned answer phrases, MUST reference
        // CLAUDE.md, AND MUST include explicit "do not copy verbatim"
        // directive.
        let body = stub_content("query");

        let forbidden_literals = [
            "Here is the answer",
            "The answer is",
            "Found N pages",
            "\u{67E5}\u{5230}",                  // 查到 (Chinese)
            "\u{56DE}\u{7B54}\u{5982}\u{4E0B}",  // 回答如下 (Chinese)
            "\u{7B54}\u{3048}\u{306F}",          // 答えは (Japanese)
            "\u{B2F5}\u{C740}",                  // 답은 (Korean)
        ];
        for phrase in forbidden_literals {
            assert!(
                !body.contains(phrase),
                "query SKILL.md body contains literal answer template `{phrase}` — step 4 must be abstract"
            );
        }

        assert!(
            body.contains("CLAUDE.md"),
            "step 4 must reference CLAUDE.md as the language source-of-truth"
        );
        assert!(
            body.contains("verbatim"),
            "step 4 must include an explicit `verbatim` warning that agents must not copy from this SKILL.md"
        );
    }

    #[test]
    fn query_workflow_declares_read_only_invariant() {
        // Spec scenario: codebus-query workflow declares read-only invariant.
        // Defense-in-depth — the binary layer already gates Write/Edit via
        // `--tools Read,Glob,Grep`, but SKILL.md restates the invariant so
        // a hypothetical future toolset-mechanism change does not silently
        // unlock writes.
        let body = stub_content("query");
        assert!(
            body.contains("MUST NOT use Write"),
            "query SKILL.md body must explicitly forbid Write/Edit"
        );
        assert!(
            body.contains("gated at the binary layer"),
            "query SKILL.md body must note that toolset gating is a binary-layer mechanism (defense-in-depth context)"
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
