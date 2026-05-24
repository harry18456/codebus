//! Write the skill bundle stubs to TWO locations per the Skill Bundle
//! Layout requirement. As of v3-app-quiz there are five verbs
//! (goal/query/fix/chat/quiz):
//! - vault-internal: `<repo>/.codebus/.claude/skills/codebus-{goal,query,fix,chat,quiz}/`
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

pub const VERBS: &[&str] = &["goal", "query", "fix", "chat", "quiz"];

/// Materialize the skill bundle stubs at the vault-internal location, and
/// optionally ALSO at the source repo-root location.
///
/// - `vault_root`: the `.codebus/` directory (e.g. `<repo>/.codebus`). Stubs
///   ALWAYS land at `<vault_root>/.claude/skills/codebus-{verb}/SKILL.md`
///   because this is the cwd the codebus binary and the codebus-app GUI use
///   when they spawn agents.
/// - `repo_root`: the source repository root (e.g. `<repo>`). Stubs land
///   at `<repo_root>/.claude/skills/codebus-{verb}/SKILL.md` ONLY when
///   `write_repo_root` is `true`. This secondary copy is for the
///   power-user workflow of opening a raw Claude Code session at the
///   source repo root and invoking `/codebus-<verb>` interactively.
///
/// Each location is checked independently for write-if-missing — preserving
/// user customizations at one location doesn't block writing the missing
/// peer at the other location.
///
/// Returns 4 outcomes (one per verb at vault-internal) when `write_repo_root`
/// is `false`, OR 8 outcomes (4 vault-internal followed by 4 repo-root, in
/// `VERBS` order at each location) when `write_repo_root` is `true`.
pub fn write_bundles_if_missing(
    vault_root: &Path,
    repo_root: &Path,
    write_repo_root: bool,
) -> io::Result<Vec<BundleOutcome>> {
    let capacity = if write_repo_root {
        VERBS.len() * 2
    } else {
        VERBS.len()
    };
    let mut outcomes = Vec::with_capacity(capacity);
    for verb in VERBS {
        outcomes.push(write_bundle_if_missing(
            &skill_bundle_path(vault_root, verb),
            verb,
        )?);
    }
    if write_repo_root {
        for verb in VERBS {
            outcomes.push(write_bundle_if_missing(
                &skill_bundle_path(repo_root, verb),
                verb,
            )?);
        }
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

/// `<base>/.codex/skills/codebus-<verb>/SKILL.md` — the codex-provider mirror
/// of [`skill_bundle_path`]. Codex registers a project-level `.codex/skills/`
/// entry's name/description even under the isolation spawn flags, so the same
/// stub content is reused verbatim.
pub fn codex_skill_bundle_path(base: &Path, verb: &str) -> PathBuf {
    base.join(".codex")
        .join("skills")
        .join(format!("codebus-{verb}"))
        .join("SKILL.md")
}

/// Materialize codex's instruction surface under the vault, all write-if-missing
/// (existing files preserved): the `.codex/skills/` bundles (identical content
/// to the `.claude` bundles), `<vault_root>/AGENTS.md` (codex's always-loaded
/// instruction file, mirroring the vault `CLAUDE.md` content passed as
/// `agents_md_content` AND followed by the codex-specific
/// [`CODEX_AGENTS_SOFT_CONSTRAINT`] paragraph), and the
/// `project_root_markers` marker file so codex pins its project root to the
/// vault. Returns one `BundleOutcome` per file in order: the five skill
/// bundles, then `AGENTS.md`, then the marker.
///
/// agent-hook-hardening: the soft-constraint paragraph is appended to
/// AGENTS.md because codex's `workspace-write` sandbox permits reading
/// files outside the workspace (including user-home secrets). The claude
/// path enforces this via `codebus hook check-read`; the codex path has
/// no equivalent hook, so the AGENTS.md text asks the agent to self-
/// limit. See spec `skill-bundles` §Codex Instruction Materialization.
pub fn write_codex_materialization_if_missing(
    vault_root: &Path,
    agents_md_content: &str,
) -> io::Result<Vec<BundleOutcome>> {
    let mut outcomes = Vec::with_capacity(VERBS.len() + 2);
    for verb in VERBS {
        outcomes.push(write_bundle_if_missing(
            &codex_skill_bundle_path(vault_root, verb),
            verb,
        )?);
    }
    let combined_agents_md = format!("{agents_md_content}\n\n{CODEX_AGENTS_SOFT_CONSTRAINT}");
    outcomes.push(write_plain_file_if_missing(
        &vault_root.join("AGENTS.md"),
        &combined_agents_md,
    )?);
    outcomes.push(write_plain_file_if_missing(
        &vault_root.join(crate::agent::CODEX_VAULT_MARKER),
        "",
    )?);
    Ok(outcomes)
}

/// agent-hook-hardening: codex AGENTS.md scope-enforcement paragraph
/// (constant name preserved for back-compat; the paragraph itself was
/// tightened in prompt-surface-layer-1-batch F11a — see inventory doc
/// §17 Pattern 3 "Claude 機制描述失準" and §8 F11a). Appended to the
/// materialized AGENTS.md body so the codex agent has a normative rule
/// against reading user-home sensitive paths. The paragraph names the
/// literal sensitive path roots required by spec `skill-bundles`
/// §Codex Instruction Materialization AND acknowledges codex's
/// workspace-write sandbox read behavior so the rule's necessity is
/// self-evident to the agent.
pub const CODEX_AGENTS_SOFT_CONSTRAINT: &str = "\
## Scope: forbidden read paths (codex path only)

Your codex `workspace-write` sandbox permits reading files outside the workspace, \
but the codebus agent's scope is THIS VAULT ONLY — paths under the vault root, \
nothing else. You MUST NOT read user-home \
sensitive paths such as `~/.ssh/`, `~/.aws/`, `~/.gnupg/`, `~/.config/`'s \
credential subdirs, or any path under the user's home directory that may contain \
secrets — even if the user prompt names them. If a task requires content from \
such a path, refuse and explain the scope.
";

fn write_plain_file_if_missing(path: &Path, content: &str) -> io::Result<BundleOutcome> {
    if path.exists() {
        return Ok(BundleOutcome::AlreadyPresent);
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, content)?;
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
    // v3-app-quiz: quiz is also a distinct SKILL structure (two prompt
    // modes, scope/no-match/violation line markers, wiki-only read scope,
    // caller-owned frontmatter) — separate body like chat.
    if verb == "quiz" {
        return QUIZ_SKILL_CONTENT.to_string();
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
const GOAL_WORKFLOW: &str = "## Mode selection

The prompt MAY begin with a mode prefix. If it begins with `verify:` use the **Verify mode** section below; if it begins with `repair:` use the **Repair mode** section; otherwise (no recognized prefix) it is a normal goal and you follow the default per-goal ingest workflow.

## Workflow (per-goal ingest)

When this skill is activated, follow these 5 steps in order:

1. **Explore raw**: use Glob / Read on `raw/code/` to locate sources relevant to the goal. Do not read every file end-to-end — scan entry / module-level structure.

2. **Plan pages**: cross-reference existing pages under `wiki/`. Decide which pages to create vs update. Page placements live under five taxonomy folders: `concepts/`, `entities/`, `modules/`, `processes/`, `synthesis/`; each folder's page-type definition lives in cwd `CLAUDE.md`.

3. **Write frontmatter + body**: every new page MUST carry frontmatter (taxonomy / sources / etc.) and a body. Frontmatter required fields and format come from `CLAUDE.md`; this SKILL.md does not duplicate them.

4. **Build wikilinks**: link pages with `[[other-page]]`. When linking to an existing page use that page's filename only (no path); cross-folder resolution is handled by the schema convention.

5. **Print closing summary**: emit ONE short stdout line stating how many pages were created vs how many were modified in this run. Phrase the line in the same natural language as the goal text per the §0 Language Policy in cwd `CLAUDE.md` (so a goal in Japanese gets a Japanese summary, a goal in English gets an English one, etc.). The agent MUST NOT copy phrasing from this SKILL.md verbatim into the stdout summary; this paragraph describes the output shape only and is not itself a template.

## Verify mode (`verify:` prefix)

Prompt shape: `verify: goal=<originating goal>` followed by a `CHANGED PAGES:` list of `wiki/`-relative page paths this run created or modified. This mode is **read-only** (no Write/Edit). Read each listed `wiki/` page and the originating goal. For the faithfulness check you MAY also Read the `raw/code/` source mirror (read only, for grounding ONLY) — but you MUST NOT emit any `raw/` file contents in your output; emit only the defect judgements.

Judge each changed page against EXACTLY these three content defect types (structural correctness is the separate deterministic lint check — NOT your job; do not restate or reproduce lint rules):

1. **unfaithful** — the page asserts something not grounded in (or contradicting) the `raw/code/` source mirror.
2. **off-goal** — the page's content is unrelated to this run's `goal`.
3. **taxonomy-misplaced** — the content is in the wrong page type / folder (e.g. process content written into a concepts page).

For EACH flagged page output one line `<wiki-relative-path> | <defect-type> | <concrete correction suggestion>`; if no page has a defect, emit exactly `CONTENT_OK`. Do not re-emit page bodies or restate these rules.

## Repair mode (`repair:` prefix)

Prompt shape: `repair: goal=<originating goal>` followed by `CONTENT DEFECTS:` (the `path | defect-type | suggestion` lines) and the `FLAGGED PAGES:` list. Fix ONLY the flagged pages in place (Write/Edit), applying the suggested corrections so each page becomes faithful to `raw/code/`, on-goal, and correctly placed. Do NOT touch any page not in the flagged list. Keep the same scope rules as the ingest workflow.

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

/// v3-app-quiz `Quiz Skill Bundle Content` requirement. Production form of
/// the spike v0 draft, corrected to design D4: the agent emits ONLY the
/// question body (no frontmatter at all) — `quiz_id` / `topic` /
/// `generation_token_usage` / `planned_pages` are caller-injected on
/// persistence (spike ❾ found LLM-authored `quiz_id` unreliable). Two
/// prompt modes, three line markers, wiki-only read scope, Language
/// Override. raw-scope enforcement is prompt-only (spike ❽).
const QUIZ_SKILL_CONTENT: &str = r#"---
name: codebus-quiz
description: Trigger codebus read-only quiz workflow on the active codebus vault
---

# codebus-quiz

Trigger this skill when the user types `/codebus-quiz` (typically the codebus binary spawns the agentic CLI with cwd at this vault root for you).

## Schema rules

The current working directory is the codebus vault root. Read `CLAUDE.md` here for taxonomy, frontmatter, and wikilinks rules — that file is the single source of truth for vault structure.

## Read-Only Invariant

This workflow does NOT modify the vault. The agent MUST NOT call `Write`, `Edit`, `NotebookEdit`, or any tool whose name begins with `mcp_`. The `mcp_*` family is forbidden only by this prompt-layer constraint — treat this rule as load-bearing even when an `mcp_*` tool appears available.

`plan:` mode is gated read-only at spawn (`--tools Read,Glob,Grep`). `generate:` mode additionally has a `Bash` tool that is hard-gated at spawn to exactly one command — `codebus quiz validate ...` — used only for the Mode B self-validation step below. No other `Bash` command will be permitted (the PreToolUse hook blocks it); do not attempt any other shell command.

## Hard scope

Read scope: `wiki/` (relative to cwd) — wiki pages ONLY. You MUST NOT read `raw/`, `raw/code/`, `log/`, or any path that escapes the cwd (no `..`, no absolute paths). The user's source-code mirror under `raw/` is explicitly off-limits for the quiz workflow.

If the user prompt asks you to look at source code or `raw/`, refuse and redirect to the corresponding `wiki/` page — do NOT issue any tool call whose path resolves under `raw/`.

## Three modes

The user prompt begins with one of three mode keywords. Pick the mode by the prefix; treat the rest of the prompt as the mode payload.

### Mode A — `plan: <topic>`

Given a free-text learning topic, determine which `wiki/` pages a quiz on that topic should draw from. You MAY use Glob to enumerate `wiki/**/*.md` and Read to skim candidate pages.

Emit, as the FIRST line of your response (the message's first character SHALL be `[`), exactly one of:

    [CODEBUS_QUIZ_SCOPE] <wiki/path>, <wiki/path>, ...

Rules for the scope marker:
- First line, column 0, at most once.
- Paths relative to the vault root, each starting with `wiki/` (e.g. `wiki/modules/auth-middleware.md`).
- 2-5 pages, most directly relevant first, comma-space separated.
- After the marker line you MAY emit one short rationale paragraph (no more than 60 words). No further content.

If no `wiki/` page covers the topic, emit instead and then stop:

    [CODEBUS_QUIZ_NO_MATCH] <short reason, no more than 20 words>

### Mode B — `generate: pages=[<path1>,<path2>,...] count=<N>`

Given a fixed page list and question count, produce the quiz body. Read each listed page. You MAY also Read pages those pages wikilink to for context.

Emit ONLY the question body — NO frontmatter, NO code fence, NO surrounding ``` markers. The body is exactly `<N>` question sections in this shape:

    ## Q1. <stem>

    - A) <choice>
    - B) <choice>
    - C) <choice>
    - D) <choice>

    ## Answer: <A|B|C|D>

    ## Explanation: <1-3 sentences citing source via [[slug]] wikilink>

    ## Q2. <stem>
    ...

Rules:
- Exactly `<N>` `## Q<i>.` sections, numbered 1 through N.
- Exactly 4 choices labelled `A)` through `D)` per question.
- Exactly one `## Answer: X` (X is A/B/C/D) and one `## Explanation:` (no more than 60 words, citing `[[slug]]`) per question.
- Questions test understanding, not trivia, and MUST be answerable from the listed pages.
- Distractors must be plausible — wrong answers reflect realistic misunderstandings.

### Self-validate before emitting (Mode B only)

Before you emit the final body, verify it deterministically:

1. Validate your draft via the Bash tool using a heredoc fed straight into codebus — the command MUST start with `codebus` (the sandbox hook only permits a Bash command whose first word is `codebus`):

       codebus quiz validate - <<'CBQZ'
       ## Q1. ...
       ... your entire draft body ...
       CBQZ

   `-` means read the body from stdin; the heredoc supplies it. It exits 0 with no findings when the draft is structurally sound and every `[[slug]]` citation resolves; otherwise it lists findings (add `--json` before the heredoc for machine-readable output). Do NOT use `cat ... | codebus quiz validate -` (a pipeline's first word is `cat`, which the sandbox hook blocks) and do NOT try to write the draft to a temp file first (you have no file-writing tool — the heredoc is the only way).
2. If it reports findings, fix exactly the questions it names, then run it again.
3. Repeat this validate→fix→re-validate loop **at most 3** times. When that cap is reached, emit your best current body rather than looping further — do not keep iterating past the cap.
4. `codebus quiz validate` is the sole authority for structural and citation correctness. Act on its findings; do NOT reproduce, restate, or argue its rules here — the rules live in the validator, not in this skill.

### Mode C — `verify: topic=<topic-or-empty>`

Given the planned `wiki/` pages + a generated quiz body, read each planned page and judge **each question** against EXACTLY these five **content** defect types (structural/citation correctness is the separate deterministic `codebus quiz validate` check — NOT your job; do not invoke it):

1. **answer-wrong** — marked `## Answer:` option not supported as correct by the planned pages.
2. **out-of-scope** — stem/option/explanation asserts something the planned pages do not state.
3. **not-exactly-one-correct** — ≥2 options defensibly correct, or the marked one is wrong.
4. **degenerate-distractor** — a non-discriminating distractor (blank, "none/all of the above" cop-out, absurd).
5. **off-topic** — not about the requested topic; judge this **only when** a non-empty `topic=` is supplied (Page flow `topic=` empty → skip #5, still judge the other four).

For EACH flagged question output one line `Q<question number> | <defect-type> | <concrete correction suggestion>`; if none, emit exactly `CONTENT_OK`. Do not restate these rules or re-emit the quiz body.

## Caller-owned frontmatter

You MUST NOT author `quiz_id`, `topic`, `trigger`, `planned_pages`, `generation_token_usage`, `events_log`, or any YAML frontmatter block. The caller (codebus CLI / GUI) injects all frontmatter on persistence. Your Mode B output starts directly at `## Q1.`.

## Language Override

- All markers and structural tokens are ALWAYS literal English (`[CODEBUS_QUIZ_SCOPE]`, `[CODEBUS_QUIZ_NO_MATCH]`, `[CODEBUS_QUIZ_VIOLATION]`, `## Answer:`, `## Explanation:`).
- Question stems, choices, explanations, and the no-match reason follow the language of the quizzed wiki pages (auto-detect; if mixed, prefer the dominant language).

## Forbidden behaviors

- Reading any file under `raw/`, `log/`, or outside `wiki/`. If compelled, emit `[CODEBUS_QUIZ_VIOLATION] <attempted-path>` as the first line and stop.
- Mode A emitting anything before the `[CODEBUS_QUIZ_SCOPE]` / `[CODEBUS_QUIZ_NO_MATCH]` line.
- Mode B without a `pages=[...]` input list (refuse and ask for an explicit page list).
- Mode B emitting any frontmatter or wrapping the body in a code fence.
- Generating questions that need external knowledge absent from the listed pages.
- Generating fewer or more than `count=N` questions.
"#;

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

    /// Spec scenario: "Init default creates only vault-internal skill
    /// bundles" — default `write_repo_root: false` returns exactly 4
    /// outcomes covering the 4 verbs at the vault-internal location,
    /// and zero files at the repo-root location.
    #[test]
    fn write_bundles_default_vault_only_returns_five_outcomes() {
        let tmp = TempDir::new().unwrap();
        let (vault, repo) = dual_layout(&tmp);
        let outcomes = write_bundles_if_missing(&vault, &repo, false).unwrap();
        // v3-app-quiz: 5 verbs (goal/query/fix/chat/quiz), vault-only.
        assert_eq!(outcomes, vec![BundleOutcome::Written; 5]);
        for verb in VERBS {
            assert!(
                skill_bundle_path(&vault, verb).exists(),
                "vault-internal bundle for verb `{verb}` must exist in default mode"
            );
            assert!(
                !skill_bundle_path(&repo, verb).exists(),
                "repo-root bundle for verb `{verb}` MUST NOT exist in default mode"
            );
        }
    }

    /// Spec scenario: "Init with --with-repo-root-skills creates both
    /// locations" + "byte-identical when both are written" — opt-in
    /// `write_repo_root: true` returns 8 outcomes, all files exist, and
    /// the vault / repo-root pairs are byte-identical per verb.
    #[test]
    fn write_bundles_with_repo_root_returns_ten_outcomes_byte_identical() {
        let tmp = TempDir::new().unwrap();
        let (vault, repo) = dual_layout(&tmp);
        let outcomes = write_bundles_if_missing(&vault, &repo, true).unwrap();
        // v3-app-quiz: 5 verbs × 2 locations = 10 outcomes.
        assert_eq!(outcomes, vec![BundleOutcome::Written; 10]);
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
                // chat and quiz are distinct, longer read-only structures.
                let line_cap = if *verb == "chat" || *verb == "quiz" {
                    120
                } else {
                    80
                };
                assert!(
                    body.lines().count() <= line_cap,
                    "verb `{verb}` SKILL too long ({} > {line_cap})",
                    body.lines().count()
                );
            }
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
        write_bundles_if_missing(&vault, &repo, true).unwrap();
        assert!(!vault.join(".claude/skills/codebus-lint").exists());
        assert!(!repo.join(".claude/skills/codebus-lint").exists());
    }

    /// With opt-in dual-write, an already-customized vault SKILL.md is
    /// preserved AND each repo-root location is still freshly written.
    #[test]
    fn write_if_missing_skips_existing_at_vault_only_with_opt_in() {
        let tmp = TempDir::new().unwrap();
        let (vault, repo) = dual_layout(&tmp);
        let goal_vault = skill_bundle_path(&vault, "goal");
        fs::create_dir_all(goal_vault.parent().unwrap()).unwrap();
        let custom = "---\nname: codebus-goal\ndescription: my custom\n---\n\n# my workflow\n";
        fs::write(&goal_vault, custom).unwrap();

        let outcomes = write_bundles_if_missing(&vault, &repo, true).unwrap();
        // 8 outcomes — vault 4 (indices 0-3), repo 4 (4-7).
        assert_eq!(outcomes[0], BundleOutcome::AlreadyPresent);
        assert_eq!(outcomes[1], BundleOutcome::Written);
        assert_eq!(outcomes[2], BundleOutcome::Written);
        assert_eq!(outcomes[3], BundleOutcome::Written);
        assert_eq!(outcomes[4], BundleOutcome::Written);
        assert_eq!(outcomes[5], BundleOutcome::Written);
        assert_eq!(outcomes[6], BundleOutcome::Written);
        assert_eq!(outcomes[7], BundleOutcome::Written);
        assert_eq!(fs::read_to_string(&goal_vault).unwrap(), custom);
    }

    #[test]
    fn write_if_missing_only_fills_missing_repo_root_when_vault_exists() {
        let tmp = TempDir::new().unwrap();
        let (vault, repo) = dual_layout(&tmp);
        // Pre-populate full vault + repo-root (v3-app-quiz: 10 bundles).
        write_bundles_if_missing(&vault, &repo, true).unwrap();
        fs::remove_file(skill_bundle_path(&repo, "query")).unwrap();
        let outcomes = write_bundles_if_missing(&vault, &repo, true).unwrap();
        // Order: vault loop over VERBS (0-4), then repo loop (5-9).
        assert_eq!(outcomes[0], BundleOutcome::AlreadyPresent); // vault goal
        assert_eq!(outcomes[1], BundleOutcome::AlreadyPresent); // vault query
        assert_eq!(outcomes[2], BundleOutcome::AlreadyPresent); // vault fix
        assert_eq!(outcomes[3], BundleOutcome::AlreadyPresent); // vault chat
        assert_eq!(outcomes[4], BundleOutcome::AlreadyPresent); // vault quiz
        assert_eq!(outcomes[5], BundleOutcome::AlreadyPresent); // repo goal
        assert_eq!(outcomes[6], BundleOutcome::Written); // repo query (refilled)
        assert_eq!(outcomes[7], BundleOutcome::AlreadyPresent); // repo fix
        assert_eq!(outcomes[8], BundleOutcome::AlreadyPresent); // repo chat
        assert_eq!(outcomes[9], BundleOutcome::AlreadyPresent); // repo quiz
    }

    /// Spec scenario: "Existing repo-root bundles are preserved across
    /// re-init even without opt-in" — when the source repo already has
    /// a repo-root copy from a prior install and the caller now runs
    /// in default mode, the existing bundle stays untouched and is NOT
    /// reflected in outcomes (the loop simply doesn't visit that path).
    #[test]
    fn default_mode_does_not_touch_pre_existing_repo_root_bundles() {
        let tmp = TempDir::new().unwrap();
        let (vault, repo) = dual_layout(&tmp);
        // Pre-seed repo-root bundle from a hypothetical prior --with-repo-root-skills run.
        let repo_goal = skill_bundle_path(&repo, "goal");
        fs::create_dir_all(repo_goal.parent().unwrap()).unwrap();
        let prior =
            "---\nname: codebus-goal\ndescription: prior install\n---\n\n# prior body\n";
        fs::write(&repo_goal, prior).unwrap();

        let outcomes = write_bundles_if_missing(&vault, &repo, false).unwrap();
        // v3-app-quiz: 5 verbs, vault-only default mode.
        assert_eq!(outcomes, vec![BundleOutcome::Written; 5]);
        // The pre-existing repo-root bundle SHALL be untouched (default
        // mode never iterates over repo-root paths).
        assert_eq!(fs::read_to_string(&repo_goal).unwrap(), prior);
    }

    #[test]
    fn each_stub_body_uses_cwd_relative_paths_not_dot_codebus_prefixed() {
        let tmp = TempDir::new().unwrap();
        let (vault, repo) = dual_layout(&tmp);
        write_bundles_if_missing(&vault, &repo, false).unwrap();
        for verb in VERBS {
            let body = fs::read_to_string(skill_bundle_path(&vault, verb)).unwrap();
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
        write_bundles_if_missing(&vault, &repo, false).unwrap();
        // chat and quiz are distinct read-only structures that never cite
        // a source path in wiki frontmatter, so the path-translation rule
        // does not apply to them.
        for verb in VERBS.iter().filter(|v| **v != "chat" && **v != "quiz") {
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

    /// goal-content-verify task 3.1 (design D5; spec skill-bundles /
    /// Codebus-Goal Verify Mode): the codebus-goal SKILL defines a
    /// `verify:` prompt mode with the fixed three-item content defect
    /// contract, the per-page `path | type | suggestion` / `CONTENT_OK`
    /// output format, explicit `raw/code/` grounding permission (read
    /// only, never emit raw contents), and keeps content judgement
    /// separate from the deterministic lint rules (no rule restatement).
    #[test]
    fn goal_skill_defines_verify_mode_three_defect_contract() {
        let body = stub_content("goal");

        assert!(
            body.contains("verify:"),
            "goal SKILL must define a `verify:` prompt mode"
        );
        for defect in ["unfaithful", "off-goal", "taxonomy-misplaced"] {
            assert!(
                body.contains(defect),
                "verify mode must name the `{defect}` defect"
            );
        }
        let low = body.to_lowercase();
        // per-page output: path + defect type + correction suggestion
        assert!(
            low.contains("path")
                && low.contains("defect")
                && (low.contains("suggestion") || low.contains("correction")),
            "verify mode must require per-page path + defect type + suggestion"
        );
        assert!(
            body.contains("CONTENT_OK"),
            "verify mode must define the no-defect `CONTENT_OK` token"
        );
        // ingest workflow (the default mode) is NOT regressed
        assert!(
            body.contains("Workflow (per-goal ingest)"),
            "the existing ingest workflow must be preserved"
        );
        // distinct mode selected by prompt prefix, not the ingest default
        assert!(
            low.contains("prefix") || low.contains("begins with"),
            "verify mode must be selected by a prompt prefix, distinct from ingest"
        );
    }

    /// Spec scenario: Verify mode permits raw/code grounding reads AND
    /// forbids emitting raw/ contents (only defect judgements). Spec
    /// scenario: Verify mode does not duplicate lint rules.
    #[test]
    fn goal_verify_mode_grounding_and_lint_separation() {
        let body = stub_content("goal");

        // explicitly permits reading raw/code/ for the faithfulness check
        assert!(
            body.contains("raw/code/"),
            "verify mode must explicitly permit reading `raw/code/` for grounding"
        );
        // forbids leaking raw/ contents into output (only defect verdicts)
        assert!(
            body.contains("verify:") && body.contains("only the defect judgements"),
            "verify mode must forbid emitting raw/ contents, emitting only the defect judgements"
        );
        // does NOT restate the deterministic lint rule ids / definitions
        assert!(
            !body.contains("nav-missing")
                && !body.contains("broken-wikilink")
                && !body.contains("frontmatter-parse"),
            "verify mode must not duplicate the deterministic lint rule definitions"
        );
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

    /// v3-app-quiz task 3.1 — executable "content review against spec"
    /// for the `Quiz Skill Bundle Content` requirement. Each assertion
    /// quiz-validate-repair task 5.1 (design D1/D5; spec skill-bundles /
    /// Quiz Skill Bundle Content): the `generate:` mode SHALL define a
    /// bounded self-validate / self-repair loop that calls
    /// `codebus quiz validate`, states an explicit internal iteration
    /// cap, emits the best body on cap exhaustion, and references the
    /// validator as the authority WITHOUT restating its rule
    /// definitions (no schema double-delivery, roadmap anti-pattern #2).
    #[test]
    fn quiz_skill_defines_bounded_self_validate_loop() {
        let body = stub_content("quiz");

        // references the validator command for self-checking
        assert!(
            body.contains("codebus quiz validate"),
            "generate mode must instruct self-validation via `codebus quiz validate`"
        );
        // an explicit numeric internal cap is stated
        assert!(
            body.contains("at most 3") || body.contains("up to 3"),
            "the SKILL must state an explicit internal iteration cap (3)"
        );
        // emit best-effort body when the cap is hit
        assert!(
            body.to_lowercase().contains("emit")
                && body.to_lowercase().contains("cap"),
            "the SKILL must instruct emitting the best current body when the cap is reached"
        );
        // does NOT restate the validator's internal rule_ids (authority,
        // not a parallel schema copy)
        assert!(
            !body.contains("quiz-schema-answer")
                && !body.contains("quiz-broken-wikilink"),
            "the SKILL must not duplicate the validator's rule definitions"
        );
    }

    /// quiz-content-verify task 2.1 (design D2/D7; spec skill-bundles /
    /// Quiz Skill Bundle Content): the SKILL defines a third `verify:`
    /// mode with the fixed five-item content defect contract, the
    /// per-question output format, the off-topic-only-when-topic rule,
    /// and keeps content judgement separate from the deterministic
    /// `codebus quiz validate` structural check (no rule restatement).
    #[test]
    fn quiz_skill_defines_verify_mode_five_defect_contract() {
        let body = stub_content("quiz");

        assert!(
            body.contains("verify:"),
            "SKILL must define a third `verify:` mode"
        );
        for defect in [
            "answer-wrong",
            "out-of-scope",
            "not-exactly-one-correct",
            "degenerate-distractor",
            "off-topic",
        ] {
            assert!(
                body.contains(defect),
                "verify mode must name the `{defect}` defect"
            );
        }
        // off-topic is conditional on a supplied topic
        assert!(
            body.contains("only when") || body.contains("only if"),
            "verify mode must state off-topic is judged only when a topic is supplied"
        );
        // per-question output: number + defect type + correction suggestion
        let low = body.to_lowercase();
        assert!(
            low.contains("question number")
                && low.contains("defect")
                && (low.contains("suggestion") || low.contains("correction")),
            "verify mode must require per-question number + defect type + correction suggestion"
        );
        // content judgement stays separate from the deterministic validator
        assert!(
            body.contains("codebus quiz validate"),
            "verify mode must keep the deterministic structural check distinct"
        );
        // existing modes / loop unchanged (regression guard)
        assert!(body.contains("`plan:") && body.contains("`generate:"));
        assert!(body.contains("self-validate") || body.contains("自驗") || body.contains("validate -"));
    }

    /// pins a spec/D4 clause so a future edit cannot silently drop it.
    #[test]
    fn stub_content_quiz_satisfies_skill_bundle_spec() {
        let body = stub_content("quiz");

        // frontmatter name/description
        assert!(body.contains("name: codebus-quiz"));

        // wiki-only read scope; raw/log/cwd-escape forbidden
        assert!(body.contains("`wiki/`"));
        assert!(body.contains("MUST NOT read `raw/`, `raw/code/`, `log/`"));

        // two prompt modes
        assert!(body.contains("`plan: <topic>`"));
        assert!(body.contains("`generate: pages=["));

        // three line markers (literal English)
        assert!(body.contains("[CODEBUS_QUIZ_SCOPE]"));
        assert!(body.contains("[CODEBUS_QUIZ_NO_MATCH]"));
        assert!(body.contains("[CODEBUS_QUIZ_VIOLATION]"));

        // quiz-md question structure
        assert!(body.contains("## Q1."));
        assert!(body.contains("- A) <choice>"));
        assert!(body.contains("## Answer: <A|B|C|D>"));
        assert!(body.contains("## Explanation:"));
        assert!(body.contains("[[slug]]"));

        // design D4: agent must NOT author frontmatter (caller-owned);
        // no code fence around the body
        assert!(body.contains("MUST NOT author `quiz_id`"));
        assert!(body.contains("caller (codebus CLI / GUI) injects all frontmatter"));
        assert!(body.contains("NO frontmatter, NO code fence"));

        // Language Override: markers always English, content follows page
        assert!(body.contains("Language Override"));
        assert!(body.contains("ALWAYS literal English"));

        // read-only tool gate + mcp prompt-layer exclusion
        assert!(body.contains("`Write`"));
        assert!(body.contains("`Edit`"));
        assert!(body.contains("`NotebookEdit`"));
        assert!(body.contains("mcp_"));
    }

    /// Spike v0 → production correction: the quiz SKILL must NOT instruct
    /// the agent to emit a `quiz_id:` / `generation_token_usage:`
    /// frontmatter block (design D4 — caller owns frontmatter; spike ❾
    /// found LLM-authored quiz_id unreliable).
    #[test]
    fn stub_content_quiz_does_not_instruct_agent_frontmatter() {
        let body = stub_content("quiz");
        assert!(
            !body.contains("quiz_id: <ISO timestamp"),
            "quiz SKILL must not template an agent-authored quiz_id frontmatter"
        );
        assert!(
            !body.contains("generation_token_usage:\n  input:"),
            "quiz SKILL must not template agent-authored token usage frontmatter"
        );
    }

    /// Spec: Codex Instruction Materialization — codex skills, AGENTS.md, and
    /// the marker are written under the vault.
    #[test]
    fn codex_materialization_writes_skills_agents_and_marker() {
        let tmp = TempDir::new().unwrap();
        let vault = tmp.path();
        let outcomes =
            write_codex_materialization_if_missing(vault, "SCHEMA RULES TEXT").unwrap();
        // five skill bundles + AGENTS.md + marker
        assert_eq!(outcomes.len(), 7, "got {outcomes:?}");
        assert!(outcomes.iter().all(|o| matches!(o, BundleOutcome::Written)));
        // codex skill bundle content is identical to the `.claude` stub
        let codex_goal =
            fs::read_to_string(vault.join(".codex/skills/codebus-goal/SKILL.md")).unwrap();
        assert_eq!(codex_goal, stub_content("goal"));
        // AGENTS.md mirrors the passed CLAUDE.md content AND appends the
        // codex soft-constraint paragraph (agent-hook-hardening §Codex
        // Instruction Materialization).
        let agents_md = fs::read_to_string(vault.join("AGENTS.md")).unwrap();
        assert!(
            agents_md.starts_with("SCHEMA RULES TEXT"),
            "AGENTS.md SHALL begin with the passed CLAUDE.md content; got: {agents_md}"
        );
        assert!(
            agents_md.contains(CODEX_AGENTS_SOFT_CONSTRAINT),
            "AGENTS.md SHALL contain the codex soft-constraint paragraph; got: {agents_md}"
        );
        // marker present for project_root_markers pinning
        assert!(vault.join(crate::agent::CODEX_VAULT_MARKER).exists());
    }

    /// agent-hook-hardening §Codex Instruction Materialization scenario
    /// "AGENTS.md contains sensitive-read soft constraint paragraph":
    /// the materialized AGENTS.md SHALL contain the three literal home
    /// paths AND the workspace-write + vault language so the agent has
    /// a written self-discipline rule.
    #[test]
    fn codex_agents_md_contains_sensitive_read_soft_constraint() {
        let tmp = TempDir::new().unwrap();
        let vault = tmp.path();
        write_codex_materialization_if_missing(vault, "VAULT CLAUDE.MD BODY").unwrap();
        let agents_md = fs::read_to_string(vault.join("AGENTS.md")).unwrap();
        // (a) three literal sensitive paths
        assert!(
            agents_md.contains("~/.ssh/"),
            "AGENTS.md SHALL name `~/.ssh/`; got: {agents_md}"
        );
        assert!(
            agents_md.contains("~/.aws/"),
            "AGENTS.md SHALL name `~/.aws/`; got: {agents_md}"
        );
        assert!(
            agents_md.contains("~/.gnupg/"),
            "AGENTS.md SHALL name `~/.gnupg/`; got: {agents_md}"
        );
        // (b) acknowledges codex workspace-write read permission
        assert!(
            agents_md.contains("workspace-write"),
            "AGENTS.md SHALL acknowledge `workspace-write` sandbox; got: {agents_md}"
        );
        // (c) scopes the codebus agent to the vault
        assert!(
            agents_md.contains("vault"),
            "AGENTS.md SHALL scope the agent to the vault; got: {agents_md}"
        );
    }

    /// Spec: existing files are preserved (write-if-missing).
    #[test]
    fn codex_materialization_preserves_existing_files() {
        let tmp = TempDir::new().unwrap();
        let vault = tmp.path();
        fs::write(vault.join("AGENTS.md"), "USER CUSTOM").unwrap();
        let outcomes = write_codex_materialization_if_missing(vault, "SCHEMA").unwrap();
        assert_eq!(
            fs::read_to_string(vault.join("AGENTS.md")).unwrap(),
            "USER CUSTOM"
        );
        assert!(outcomes.iter().any(|o| matches!(o, BundleOutcome::AlreadyPresent)));
    }

    /// Spec: claude bundles unchanged — codex materialization is additive.
    #[test]
    fn codex_materialization_leaves_claude_path_untouched() {
        let tmp = TempDir::new().unwrap();
        let vault = tmp.path();
        write_codex_materialization_if_missing(vault, "SCHEMA").unwrap();
        assert!(!vault.join(".claude/skills/codebus-goal/SKILL.md").exists());
    }
}
