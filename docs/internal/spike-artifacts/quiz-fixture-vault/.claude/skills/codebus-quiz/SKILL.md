---
name: codebus-quiz
description: Trigger codebus read-only quiz workflow on the active codebus vault
---

# codebus-quiz

Trigger this skill when the user types `/codebus-quiz` (typically the codebus binary spawns the agentic CLI with cwd at this vault root for you).

## Schema rules

The current working directory is the codebus vault root. Read `CLAUDE.md` here for taxonomy, frontmatter, and wikilinks rules — that file is the single source of truth for vault structure.

## Hard scope

Read scope: `wiki/` (relative to cwd) — wiki pages only. **You MUST NOT read `raw/`, `raw/code/`, `log/`, or any path outside `wiki/`.** The user's source code mirror under `raw/` is explicitly off-limits for the quiz workflow.

Write scope: NONE. The quiz workflow is strictly read-only inside the agent process. Quiz files are written by the caller (CLI / GUI) based on your structured output, not by you using Write/Edit/NotebookEdit.

You MUST NOT use Write, Edit, NotebookEdit. You MUST NOT use `mcp__*` tools or LSP. The allowed tools are exactly `Read`, `Glob`, `Grep`, restricted to paths starting with `wiki/`.

If the user prompt asks you to look at source code or `raw/`, refuse and redirect them to the corresponding `wiki/` page.

## Two modes

The user prompt begins with one of two mode keywords. Pick the mode by the prefix; treat the rest of the prompt as the mode payload.

### Mode A — `plan: <topic>`

Given a free-text learning topic, determine which `wiki/` pages a quiz on that topic should draw from. You MAY use Glob to enumerate `wiki/**/*.md` and Read to skim candidate pages.

Emit a single line at the start of your response:

```
[CODEBUS_QUIZ_SCOPE] <relative wiki path>, <relative wiki path>, ...
```

Rules:
- Marker MUST be the first line of your response, on column 0.
- Paths MUST be relative to the vault root and start with `wiki/` (e.g. `wiki/modules/auth-middleware.md`).
- 2-5 pages per scope. Include the most directly relevant page first.
- Comma-space separator between paths.
- After the marker line, you MAY emit one short paragraph of rationale (≤ 60 words). No further content.

If no `wiki/` page covers the topic, emit instead:

```
[CODEBUS_QUIZ_NO_MATCH] <short reason ≤ 20 words>
```

and then stop.

### Mode B — `generate: pages=[<path1>,<path2>,...] count=<N>`

Given a fixed page list and question count, produce a quiz markdown document. Read each listed page. You MAY also Read pages that the listed pages wikilink to, if needed for context — but do NOT add them to `planned_pages` in the frontmatter; the input list is authoritative.

Emit a complete markdown document with this structure:

```markdown
---
quiz_id: <ISO timestamp like 2026-05-15T14-30-22>
trigger: ai_planned
topic: ""
planned_pages:
  - wiki/path1.md
  - wiki/path2.md
generation_token_usage:
  input: 0
  output: 0
---

## Q1. <stem>

- A) <choice>
- B) <choice>
- C) <choice>
- D) <choice>

## Answer: <A|B|C|D>

## Explanation: <1-3 sentence explanation citing wiki via [[slug]]>

## Q2. <stem>
...
```

Rules:
- Exactly `<N>` `## Q<i>.` sections, numbered 1 through N.
- Each question has exactly 4 choices labeled `A)` through `D)`.
- Exactly one `## Answer: X` line per question, where X is A/B/C/D.
- Exactly one `## Explanation: ...` per question, ≤ 60 words, citing source via `[[slug]]` wikilink syntax.
- Questions test understanding, not trivia. Each question MUST be answerable from the listed pages.
- Choices should be plausible distractors, not nonsense. Wrong answers should reflect realistic misunderstandings.

## Language Override

- All markers, frontmatter keys, and YAML structure are ALWAYS English (`[CODEBUS_QUIZ_SCOPE]`, `quiz_id`, `## Answer:`, etc.).
- Question stems, choices, and explanations follow the language of the wiki pages being quizzed on (auto-detect; if mixed, prefer the dominant language).

## Forbidden behaviors

- Reading any file under `raw/`, `log/`, or outside `wiki/`. If asked, emit `[CODEBUS_QUIZ_VIOLATION] attempted to access <path>` and stop.
- Mode A emitting anything before the `[CODEBUS_QUIZ_SCOPE]` or `[CODEBUS_QUIZ_NO_MATCH]` line.
- Mode B without a `pages=[...]` input list (refuse and ask for explicit page list).
- Generating questions that require external knowledge not present in the listed wiki pages.
- Generating less than `count=N` or more than `count=N` questions.
