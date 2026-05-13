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

pub const VERBS: &[&str] = &["goal", "query", "fix", "chat"];

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
    // v3-chat-verb: chat has a distinct SKILL structure (read-only sandbox,
    // multi-turn workflow, promote-suggestion line marker emission rule,
    // MCP prompt-layer exclusion) — return a completely separate body
    // rather than shoe-horning it into the goal/query/fix shell.
    if verb == "chat" {
        return CHAT_SKILL_CONTENT.to_string();
    }
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
/// (v3-goal); query carries the 4-step read-only lookup content (v3-query);
/// fix carries the v3-lint atomic-contract repair workflow.
fn workflow_section(verb: &str) -> String {
    match verb {
        "goal" => GOAL_WORKFLOW.to_string(),
        "query" => QUERY_WORKFLOW.to_string(),
        "fix" => FIX_WORKFLOW.to_string(),
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

/// Fix workflow per v3-fix-trust-agent Fix SKILL.md Atomic Contract
/// requirement (heading kept for spec stability, content rewritten).
///
/// Trust-agent model: the agent is invoked once per `codebus fix` call
/// (or per user `/codebus-fix`) and decides itself when its repair work
/// is complete. The agent has access to `Bash(codebus lint *)` (gated by
/// the PreToolUse hook installed by `codebus init`) so it can query lint
/// state freely within its session. The codebus CLI does not orchestrate
/// internal iterations — it runs lint once after the agent terminates and
/// uses that as the authoritative success signal.
///
/// The body deliberately avoids the prior v3-lint atomic-contract
/// language ("ONE round of repair", "Loop control belongs to the caller",
/// "MUST NOT spawn nested fix invocations or loop internally") — those
/// constraints are released in v3-fix-trust-agent.
/// `codebus-chat/SKILL.md` body. Multi-turn read-only sandbox + promote-
/// suggestion emission rule per `chat-verb` capability (Chat Skill Bundle
/// Content + Promote Suggestion Line Marker + MCP Tool Prompt Layer
/// Exclusion requirements). Sourced from the spike v0 draft (see
/// `docs/2026-05-13-chat-verb-discussion.md` §Spike ❺), which passed 4/4
/// scenarios with 2/2 format consistency.
const CHAT_SKILL_CONTENT: &str = "---
name: codebus-chat
description: Trigger codebus multi-turn read-only chat workflow on the active codebus vault
---

# codebus-chat

Trigger this skill when the user types `/codebus-chat` (typically the codebus binary spawns the agentic CLI with cwd at this vault root for you). This is **multi-turn** — each user message extends the same ongoing conversation rather than starting a fresh agent run.

## Schema rules

The current working directory is the codebus vault root. Read `CLAUDE.md` here for taxonomy, frontmatter, and wikilinks rules — that file is the single source of truth for vault structure.

## Read-Only Invariant

This workflow is **strictly read-only**. The agent MUST NOT call `Write`, `Edit`, `NotebookEdit`, or any tool whose name begins with `mcp_` (e.g. `mcp_claude_ai_Figma_authenticate`, `mcp_claude_ai_Gmail_authenticate`). The binary-layer toolset is gated at spawn time (`--tools Read,Glob,Grep`) so attempts to call Write / Edit / NotebookEdit fail at runtime regardless; however the `mcp_*` family is NOT covered by the `--tools` flag and is forbidden only by this prompt-layer constraint. Treat this rule as load-bearing even when an `mcp_*` tool appears to be available in the runtime toolset.

## Hard scope

Read scope: `raw/code/` (relative to cwd) — the PII-redacted source mirror. Do NOT navigate outside cwd; the user's source repo at the parent directory level is off-limits. Also Read `wiki/` to consult existing pages when answering.

You MUST NOT read any path that escapes the cwd (no `..`, no absolute paths to outside locations).

## Workflow (multi-turn read-only exploration)

Each user turn is a fresh question or follow-up in the ongoing conversation. Use Read / Glob / Grep against `wiki/` and `raw/code/` to retrieve information and answer the user's question concisely in the same language they used. You MAY chain across multiple turns to deepen the user's understanding; assume the user can see your prior responses in this conversation.

## Promote-suggestion emission

When you judge that the current conversation contains content worth writing into the wiki, prepend exactly one line of the following format at the very start of your message (before any other text):

    [CODEBUS_PROMOTE_SUGGESTION] <one-line reason in 5-15 words explaining what wiki page this would become>

### When to emit

- The user explicitly asks to write something to the wiki (\"help me write this to wiki\", \"幫我把這段寫成 wiki\", \"save this as a page\", \"this should be documented\", or similar promote-request phrasing).
- The conversation has consolidated a non-trivial piece of architectural understanding across 2+ turns AND a quick check of `wiki/` shows no existing page covers it.
- The user has chained 3+ related questions on the same topic and reached an understanding worth durable record.

### When NOT to emit

- The user's question is a single factual lookup (\"what file defines X\", \"which folder contains Y\") AND the answer is a single fact.
- An existing wiki page already covers the topic — point the user there instead.
- Discussion is still drifting / no consolidated understanding yet.
- You are uncertain — under-emit rather than over-emit.

### Format rules

- The marker MUST be on its own first line of your message, at byte offset 0 (the message's first character SHALL be `[`).
- The marker MUST appear at most once per message.
- Do NOT emit the marker speculatively; only when you have a concrete wiki page suggestion in mind.
- The reason text after the marker SHOULD be 5-15 words, naming what the wiki page would cover (not how to write it).
- After the marker line, continue your normal response to the user's question.

### Examples

User: \"how does our auth work?\"
You: (look up files, answer normally — no marker; single exploratory question)

User: \"and JWT specifically?\" / \"and refresh token rotation?\" / \"summarize the full auth lifecycle\"
You: `[CODEBUS_PROMOTE_SUGGESTION] auth lifecycle including JWT issuance and refresh rotation`
Then continue with your summary.

User: \"幫我把剛剛 auth 那段寫成 wiki\"
You: `[CODEBUS_PROMOTE_SUGGESTION] auth flow and JWT handling consolidated from conversation`
Then continue normally explaining what the page would cover.

## Language Override

The user's language SHALL override any other language in the conversation. Match the user's language for the answer body. The marker prefix `[CODEBUS_PROMOTE_SUGGESTION]` is always literal English (it is parsed by codebus CLI, not displayed to the user verbatim); only the `<reason>` portion follows the user's language.
";

const FIX_WORKFLOW: &str = "## Workflow (self-directed repair)

When this skill is activated, follow these steps:

1. **Acquire lint issues**: run `codebus lint --format json` via Bash and parse its single JSON object. The PreToolUse hook installed by `codebus init` permits `codebus lint *` and blocks any other Bash invocation, so this is the only shell command available — and it is enough. The JSON's `issues[].path` field carries an absolute filesystem path — use that path verbatim with Read / Write / Edit; do not prepend or strip any prefix.

2. **Group by file**: aggregate issues by their absolute path. Reading and editing the same file once is more efficient than per-issue file reopens.

3. **Apply repairs**: for each file, Read its current content, then use Edit to apply the minimum changes that resolve every issue grouped under that path. Issue `rule_id` selects the repair shape:
   - `frontmatter-parse` → fix YAML syntax in the `---` block.
   - `related-format` → wrap each `related[]` entry as a `[[wikilink]]`.
   - `broken-wikilink-related` → either add the missing target page or change the related entry to point at an existing slug.
   - `broken-wikilink-body` → either add the missing target page, change the body link, or remove it if the reference was speculative.
   - `broken-wikilink-nav` → same as body, but in `index.md` / `log.md`.
   - `nav-missing` → create the missing nav file with a stub heading.
   - `duplicate-slug` → rename one of the colliding files (and update incoming wikilinks); preserve content.
   - `misplaced-root-page` → move the root-level `.md` into its correct type folder under `wiki/`.

4. **Re-check freely if helpful**: after a batch of edits, you MAY re-run `codebus lint --format json` to see what remains. There is no fixed iteration count. Continue editing as long as you are making productive progress; stop when you cannot meaningfully improve the situation further (issues require human judgment about content, target pages don't exist and you don't have enough context to author them, etc.).

5. **Report**: emit one concise stdout line summarising what was repaired and what remains unresolved. Phrase the line in the natural language of the prompt context per `CLAUDE.md` §0 Language Policy.

## CLI is the final-only verifier

The codebus CLI runs lint after this session terminates and uses that result as the authoritative success signal — agent self-reports do not influence the CLI exit code. Loop control within a session is the agent's; the CLI does not iterate by spawning additional `--resume` follow-ups. The agent itself decides when its in-session repair work is complete and exits.

## Trust the absolute paths

The lint JSON's `issues[].path` is the canonical absolute path. The agent MUST use these paths verbatim with file tools. Do not derive alternative paths from `cwd` or relative slugs — lint already resolved the absolute location and trusting it avoids drift between agent's view and lint's view of the vault.

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
    fn first_time_writes_four_bundles_at_both_locations() {
        let tmp = TempDir::new().unwrap();
        let (vault, repo) = dual_layout(&tmp);
        let outcomes = write_bundles_if_missing(&vault, &repo).unwrap();
        // v3-chat-verb: 8 outcomes — 4 verbs (goal/query/fix/chat) × 2 locations.
        assert_eq!(outcomes, vec![BundleOutcome::Written; 8]);
        for verb in VERBS {
            for base in [&vault, &repo] {
                let p = skill_bundle_path(base, verb);
                assert!(p.exists(), "missing bundle for verb `{verb}` at {base:?}");
                let s = p.to_string_lossy();
                assert!(
                    s.contains(".claude")
                        && s.contains("skills")
                        && s.contains(&format!("codebus-{verb}"))
                );
                let body = fs::read_to_string(&p).unwrap();
                assert!(body.starts_with("---\n"));
                assert!(body.contains(&format!("name: codebus-{verb}")));
                // chat SKILL is intentionally longer than goal/query/fix
                // (it carries the full emission examples + read-only
                // invariant + MCP exclusion), so widen the line cap.
                let line_cap = if *verb == "chat" { 120 } else { 80 };
                assert!(
                    body.lines().count() <= line_cap,
                    "verb `{verb}` SKILL too long ({} > {line_cap})",
                    body.lines().count()
                );
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
        // v3-chat-verb: 8 outcomes — vault 4 (indices 0-3), repo 4 (4-7).
        // Vault: goal AlreadyPresent, query/fix/chat Written
        assert_eq!(outcomes[0], BundleOutcome::AlreadyPresent);
        assert_eq!(outcomes[1], BundleOutcome::Written);
        assert_eq!(outcomes[2], BundleOutcome::Written);
        assert_eq!(outcomes[3], BundleOutcome::Written);
        // Repo-root: all Written (independent check)
        assert_eq!(outcomes[4], BundleOutcome::Written);
        assert_eq!(outcomes[5], BundleOutcome::Written);
        assert_eq!(outcomes[6], BundleOutcome::Written);
        assert_eq!(outcomes[7], BundleOutcome::Written);
        // Custom vault content preserved
        assert_eq!(fs::read_to_string(&goal_vault).unwrap(), custom);
    }

    #[test]
    fn write_if_missing_only_fills_missing_repo_root_when_vault_exists() {
        let tmp = TempDir::new().unwrap();
        let (vault, repo) = dual_layout(&tmp);
        // Pre-populate full vault (all 4 verbs at v3-chat-verb).
        write_bundles_if_missing(&vault, &repo).unwrap();
        // Wipe repo-root copy of one verb
        fs::remove_file(skill_bundle_path(&repo, "query")).unwrap();
        // Re-run: vault all AlreadyPresent, repo-root: query Written, others
        // AlreadyPresent
        let outcomes = write_bundles_if_missing(&vault, &repo).unwrap();
        assert_eq!(outcomes[0], BundleOutcome::AlreadyPresent); // vault goal
        assert_eq!(outcomes[1], BundleOutcome::AlreadyPresent); // vault query
        assert_eq!(outcomes[2], BundleOutcome::AlreadyPresent); // vault fix
        assert_eq!(outcomes[3], BundleOutcome::AlreadyPresent); // vault chat
        assert_eq!(outcomes[4], BundleOutcome::AlreadyPresent); // repo goal
        assert_eq!(outcomes[5], BundleOutcome::Written); // repo query (refilled)
        assert_eq!(outcomes[6], BundleOutcome::AlreadyPresent); // repo fix
        assert_eq!(outcomes[7], BundleOutcome::AlreadyPresent); // repo chat
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
                "verb `{verb}` missing cwd-relative wiki scope `wiki/`"
            );
            // chat is read-only and uses a slightly different escape-prohibition
            // wording ("MUST NOT read any path that escapes the cwd"); the
            // other three verbs share the "read or write" form. Assert on
            // the common substring instead of the exact phrase.
            assert!(
                body.contains("MUST NOT") && body.contains("escapes the cwd"),
                "verb `{verb}` missing escape-prohibition"
            );
        }
    }

    #[test]
    fn each_stub_body_declares_path_translation_rule() {
        let tmp = TempDir::new().unwrap();
        let (vault, repo) = dual_layout(&tmp);
        write_bundles_if_missing(&vault, &repo).unwrap();
        // Path translation is meaningful only for write-capable verbs
        // (goal writes wiki frontmatter `sources[].path`; query/fix also
        // touch the path layout). chat is read-only multi-turn — it never
        // cites a source path in a wiki frontmatter, so the rule does not
        // apply. Skip chat here.
        for verb in VERBS.iter().filter(|v| **v != "chat") {
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
            "\u{67E5}\u{5230}",                 // 查到 (Chinese)
            "\u{56DE}\u{7B54}\u{5982}\u{4E0B}", // 回答如下 (Chinese)
            "\u{7B54}\u{3048}\u{306F}",         // 答えは (Japanese)
            "\u{B2F5}\u{C740}",                 // 답은 (Korean)
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

    /// v3-fix-trust-agent Fix SKILL.md Atomic Contract scenario: body MUST
    /// NOT contain the v3-lint atomic-contract phrasing — those constraints
    /// are released in the trust-agent model. (The requirement heading is
    /// kept for spec stability; the content is fully rewritten.)
    #[test]
    fn fix_workflow_body_does_not_prescribe_atomic_single_round() {
        let body = stub_content("fix");
        let forbidden_literal_phrases = [
            "ONE round of repair",
            "atomic contract",
            "MUST NOT spawn nested fix invocations or loop internally",
            "Loop control belongs to the caller",
        ];
        for phrase in forbidden_literal_phrases {
            assert!(
                !body.contains(phrase),
                "fix SKILL.md still contains v3-lint atomic-contract phrase `{phrase}` — should be removed in v3-fix-trust-agent"
            );
        }
    }

    /// v3-fix-trust-agent Fix SKILL.md Atomic Contract scenario: body
    /// instructs agent to use absolute paths from lint JSON `issues[].path`
    /// directly with Read/Write/Edit, no path translation.
    #[test]
    fn fix_workflow_instructs_absolute_path_use_from_lint_json() {
        let body = stub_content("fix");
        assert!(
            body.contains("absolute filesystem path") || body.contains("absolute path"),
            "fix SKILL.md missing absolute-path instruction"
        );
        assert!(
            body.contains("issues[].path"),
            "fix SKILL.md missing reference to JSON `issues[].path` field"
        );
        assert!(
            body.contains("verbatim"),
            "fix SKILL.md missing `verbatim` directive on path use"
        );
        assert!(
            body.contains("--format json"),
            "fix SKILL.md missing `codebus lint --format json` invocation hint"
        );
    }

    /// v3-fix-trust-agent Fix SKILL.md Atomic Contract scenario: body
    /// states that the CLI is the final-only verifier (not iterating),
    /// and the agent itself decides when its in-session work is complete.
    #[test]
    fn fix_workflow_states_cli_is_final_only_verifier() {
        let body = stub_content("fix");
        // The workflow MUST have a section explicitly explaining the
        // CLI's authority is post-session lint only.
        assert!(
            body.contains("CLI is the final-only verifier") || body.contains("final-only verifier"),
            "fix SKILL.md missing `final-only verifier` framing"
        );
        // And the agent decides its own completion.
        assert!(
            body.contains("agent itself decides when") || body.contains("agent's"),
            "fix SKILL.md missing agent-self-determination language"
        );
    }

    #[test]
    fn fix_workflow_body_is_english() {
        // Same internal-surface English-only invariant as goal/query.
        let body = stub_content("fix");
        let cjk: Vec<char> = body
            .chars()
            .filter(|c| ('\u{4E00}'..='\u{9FFF}').contains(c))
            .collect();
        assert!(
            cjk.is_empty(),
            "fix SKILL.md body must not contain CJK ideographs, found {} (first 10: {:?})",
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
            "\u{672C}\u{6B21}\u{65B0}\u{589E}", // 本次新增 (Chinese)
            "\u{30DA}\u{30FC}\u{30B8}\u{3092}\u{8FFD}\u{52A0}", // ページを追加 (Japanese)
            "\u{C774}\u{BC88}\u{C5D0}\u{20}\u{C0C8}\u{B85C}", // 이번에 새로 (Korean)
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

    /// v3-chat-verb `Chat Skill Bundle Content` scenario:
    /// chat SKILL body contains the literal promote-suggestion line marker
    /// (with trailing space), a Read-Only Invariant section, and the
    /// `name: codebus-chat` frontmatter line. Sourced from the spike v0
    /// draft that passed 4/4 emission scenarios.
    #[test]
    fn stub_content_chat_contains_promote_marker_format() {
        let body = stub_content("chat");
        assert!(
            body.starts_with("---\n"),
            "chat SKILL must begin with YAML frontmatter"
        );
        assert!(
            body.contains("name: codebus-chat"),
            "chat SKILL frontmatter must set `name: codebus-chat`"
        );
        assert!(
            body.contains("[CODEBUS_PROMOTE_SUGGESTION] "),
            "chat SKILL body must declare the literal marker prefix (with trailing space)"
        );
        assert!(
            body.contains("Read-Only Invariant"),
            "chat SKILL body must include the Read-Only Invariant section header"
        );
        // Non-ASCII example reason demonstrates language-agnostic emission.
        assert!(
            body.contains("\u{5E6B}\u{6211}"),
            "chat SKILL body must contain at least one non-ASCII (Chinese) example for the marker"
        );
    }

    /// v3-chat-verb `MCP Tool Prompt Layer Exclusion` requirement:
    /// chat SKILL body must explicitly forbid the `mcp_*` tool family
    /// under the Read-Only Invariant / hard-scope section, because those
    /// tools are NOT gated by `--tools` at the binary layer.
    #[test]
    fn stub_content_chat_explicitly_forbids_mcp_tools() {
        let body = stub_content("chat");
        assert!(
            body.contains("mcp_"),
            "chat SKILL must mention the `mcp_` tool name prefix"
        );
        // The constraint must live alongside the read-only invariant — i.e.
        // before the workflow / promote-suggestion sections — so an agent
        // reading top-down sees it as load-bearing.
        let mcp_pos = body
            .find("mcp_")
            .expect("mcp_ already asserted to exist above");
        let workflow_pos = body
            .find("## Workflow")
            .expect("chat SKILL must have a Workflow section");
        assert!(
            mcp_pos < workflow_pos,
            "MCP exclusion must appear BEFORE the Workflow section (found mcp_ at {mcp_pos}, workflow at {workflow_pos})"
        );
        // And the prohibition must be phrased as a hard rule.
        assert!(
            body.contains("MUST NOT") && body.contains("`mcp_"),
            "chat SKILL must phrase MCP exclusion as a MUST NOT directive"
        );
    }

    /// v3-chat-verb `Chat Verb Toolset` requirement (defense in depth):
    /// chat SKILL body also names `Write` / `Edit` as forbidden at the
    /// prompt layer, even though `--tools Read,Glob,Grep` already gates
    /// them at the binary layer.
    #[test]
    fn stub_content_chat_explicitly_forbids_write_edit() {
        let body = stub_content("chat");
        assert!(body.contains("`Write`"));
        assert!(body.contains("`Edit`"));
    }
}
