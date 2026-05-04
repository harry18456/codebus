## 1. Repo bootstrap (TypeScript skeleton + license)

- [x] [P] 1.1 Create package.json (name=codebus, bin entry pointing dist/cli.js, deps: commander chalk ora gray-matter simple-git js-yaml; devDeps: typescript tsx vitest @types/node @types/js-yaml), tsconfig.json (ES2022/module ES2022/Bundler/strict), vitest.config.ts (80% coverage threshold) — sets the foundation for the Hexagonal 三層架構（core/infra/ui） used throughout
- [x] [P] 1.2 Add LICENSE (MIT), .gitignore (node_modules/dist/coverage), skeleton README.md placeholder
- [x] 1.3 Verify `npm install` + `npm run build` produce `dist/cli.js` with no tsc errors (npm install verified; build verify deferred to Task 16.2 since src/cli.ts is created in Task 14)

## 2. Core vault layout and lock

- [x] [P] 2.1 Implement vaultPaths(repoRoot) in src/core/vault/layout.ts returning all .codebus/ subdirectory and file paths (root, git, gitignore, goalsJsonl, schemaMd, raw, rawCode, wiki, wikiOverview, wikiIndex, wikiLog, wikiPages, wikiGoals, output, lock) — backbone of Initialize .codebus/ vault structure under user repo requirement
- [x] [P] 2.2 Implement file-based lock in src/core/vault/lock.ts using `wx` flag for atomic creation; idempotent releaseLock — fulfills Acquire file-based lock for vault operations requirement
- [x] 2.3 Write unit tests for both modules (paths shape; acquire→release happy path; double-acquire throws; double-release no-op)

## 3. Core wiki page model

- [x] [P] 3.1 Define types in src/core/wiki/types.ts (PageType, SourceRef with optional sha256/at_commit fields, PageFrontmatter, ParsedPage)
- [x] 3.2 Implement parsePage / serializePage in src/core/wiki/frontmatter.ts using gray-matter; validate REQUIRED_FIELDS (throw on missing); round-trip test
- [x] [P] 3.3 Implement repairWikilinkList in src/core/wiki/frontmatter-repair.ts — regex util converting unquoted `related: [[a]], [[b]]` → `related: ["[[a]]", "[[b]]"]`; preserves already-quoted lists; ignores non-wikilink arrays
- [x] 3.4 Implement mergePage in src/core/wiki/page-merge.ts — Append-merge resolves page conflicts requirement (and Append-merge page conflict design decision): preserve title/type/created locked fields, union sources/goals/related arrays (MUST union BOTH `[...existing.goals, goalText]` AND `incoming.frontmatter.goals` — bug caught in review iter-8 where impl ignored incoming.goals), append `## from goal: <X> (UTC YYYY-MM-DD)` body section
- [x] 3.5 Implement detectStaleSources pure function in src/core/wiki/stale-detect.ts (frontmatter + Map<path, currentHash> → StaleResult) — supports Stale-detect: detect-and-flag only（phase 1） design decision
- [x] 3.6 Add utcTodayISO helper (Date fields use UTC YYYY-MM-DD requirement) and write unit tests for all four wiki modules

## 4. Infra fs (file-ops + raw-sync)

- [x] [P] 4.1 Implement sha256File (stream hashing) and listFilesRecursive (relative-path listing) in src/infra/fs/file-ops.ts
- [x] [P] 4.2 Implement syncRepoToRaw in src/infra/fs/raw-sync.ts — clear rawDir → walk repo → skip .git/.codebus/.env at root + gitignored entries + files larger than 5 MiB, then copyFile each — fulfills Sync source repo to raw/code/ excluding ignored content requirement
- [x] 4.3 Write unit tests covering: include src/, exclude node_modules (via gitignore), exclude .git/.codebus/.env, preserve sibling raw/docs/ untouched on re-sync

## 5. Infra git (source-version + nested-repo)

- [x] [P] 5.1 Implement getSourceVersion(repoRoot) in src/infra/git/source-version.ts using simple-git: returns {commit, uncommitted}; commit=null when not a git repo — supports Record source version per goal in goals.jsonl flow
- [x] 5.2 Implement initNestedRepo(vaultRoot) in src/infra/git/nested-repo.ts (idempotent: skip if .git exists; configure codebus identity) — Initialize nested git repository at .codebus/.git requirement
- [x] 5.3 Implement autoCommit(vaultRoot, message) in src/infra/git/nested-repo.ts (no-op when working tree clean; returns commit sha) — Auto-commit nested git on goal completion requirement
- [x] 5.4 Write unit tests with real git repos under tmpdir (clean repo → commit hash + uncommitted=false; dirty repo → uncommitted=true; non-git path → commit=null)

## 6. Infra LLM provider (interface + claude-cli adapter)

- [x] [P] 6.1 Define LLMProvider interface in src/infra/llm/types.ts: StreamEvent union (kind: 'thought' | 'tool_use' | 'tool_result' | 'done'), LLMMode, InvokeOptions ({systemPrompt, userMessage, mode, cwd, vaultRoot}) — implements LLMProvider interface + ClaudeCliProvider single adapter（phase 1） design decision
- [x] 6.2 Implement ClaudeCliProvider.buildArgv in src/infra/llm/claude-cli.ts — argv MUST include `-p`, `--output-format stream-json`, `--input-format stream-json`, `--verbose`, `--permission-mode acceptEdits`, `--disallowedTools` (Bash,WebFetch,WebSearch in ingest mode; Bash,WebFetch,WebSearch,Write,Edit in query mode); MUST NOT include `--add-dir` — implements 三條 must flag design decision and Sandbox 三層（spike-verified） design decision
- [x] 6.3 Implement ClaudeCliProvider.invoke: spawn child with cwd=opts.cwd (= .codebus/ for phase 1; system-level user-source-repo isolation), write stream-json input, parse stdout line-by-line via parseClaudeStreamLine (yields zero-or-more StreamEvent per line), await exit code, classifyExit branching — fulfills Spawn LLM agent with sandbox flags and cwd isolation requirement
- [x] 6.4 Implement classifyExit (regex on stderr keywords: unauthen|auth|token|login → oauth-needed, else generic-error; code 0 → success) — Detect OAuth failure from subprocess exit requirement
- [x] 6.5 Implement cancel() — SIGTERM child if alive
- [x] 6.6 Write unit tests for ClaudeCliProvider: buildArgv (assert no --add-dir, assert --permission-mode acceptEdits present, mode-specific disallowedTools); classifyExit cases; AND a critical test that mocks child_process.spawn and verifies opts.cwd is forwarded as spawn cwd (covers iter-5 cwd=.codebus/ regression risk)

## 7. UI stream parser (extracted, real schema verified)

- [x] [P] 7.1 Implement parseClaudeStreamLine in src/ui/stream-parser.ts using SPIKE-VERIFIED schema (Stream-json schema（spike-verified, NOT plan-imagined） design decision): parses `{type:"assistant", message:{content:[{type:"text"|"tool_use"|"thinking"}]}}` → text becomes thought / tool_use becomes tool_use / thinking skipped; parses `{type:"user", message:{content:[{type:"tool_result"}]}}` → tool_result with isError flag; system / result / rate_limit_event / unknown types skipped (return empty array); malformed JSON returns empty array — returns StreamEvent[] (zero-or-more per line) since assistant.content[] can have text + tool_use together — fulfills Stream agent events to terminal and emit them via callback requirement
- [x] 7.2 Refactor src/infra/llm/claude-cli.ts to import parseClaudeStreamLine; for-await loop iterates `for (const event of parseClaudeStreamLine(line)) yield event`
- [x] 7.3 Write unit tests using REAL schema payloads (assistant.text → thought, assistant.tool_use → tool_use, user.tool_result success, user.tool_result error, multi-content → multi-events with thinking skipped, malformed JSON → empty array, system / result / rate_limit_event / unknown → empty array)

## 8. UI emoji-mode + render (Hybrid emoji/symbol 終端輸出（5-level priority） decision)

- [x] [P] 8.1 Implement EmojiMode type ('auto'|'on'|'off') and resolveEmojiMode(flag, runtime) in src/ui/emoji-mode.ts honoring TTY/CI/NO_EMOJI/TERM=dumb auto-detect — supports Resolve emoji mode via 5-level priority requirement
- [x] 8.2 Implement detectRuntime() reading process.stdout.isTTY and process.env
- [x] [P] 8.3 Implement renderEvent in src/ui/render.ts (Render per-event stream output with emoji or symbol prefix requirement) — emoji map (thought/tool/write/result) + symbol fallback map; chalk colors (cyan/green/dim/red); Write/Edit special-cased to write glyph; tool_result error uses red color (not separate emoji)
- [x] 8.4 Implement renderBanner in src/ui/render.ts (Render lifecycle banners requirement) — start/goal/done/hint with emoji glyph or symbol fallback
- [x] 8.5 Apply chalk color when stdout is a TTY requirement — render functions accept useColor flag; cli.ts gates by `process.stdout.isTTY && !process.env.NO_COLOR`
- [x] 8.6 Write unit tests covering all 7 resolveEmojiMode branches (on/off/auto×TTY×CI×NO_EMOJI×TERM), emoji vs symbol output for each event/banner kind

## 9. Infra global-config (~/.codebus/config.yaml)

- [x] 9.1 Implement loadGlobalConfig() in src/infra/global-config.ts reading ~/.codebus/config.yaml using js-yaml; returns typed GlobalConfig (emoji?: 'auto'|'on'|'off') — Load global config tolerantly requirement
- [x] 9.2 pickKnownFields validates emoji enum, warns on unknown values, silently ignores phase-2 forward-compat fields (default_provider, api_keys, token_usage_log) without warning
- [x] 9.3 Handle missing file → empty config (no warn); parse error → warn + empty
- [x] 9.4 Write unit tests with vi.stubEnv('HOME', tmpDir) covering all 5 scenarios (missing → empty / valid emoji / invalid yaml warns / unknown fields silent / unknown emoji value warns)

## 10. Schema (built-in CLAUDE.md content)

- [x] 10.1 Implement CODEBUS_SCHEMA_MARKDOWN in src/schema/claude-md.ts exporting the 12-section schema content with `SPDX-License-Identifier: MIT` header — implements MIT license + Clean-room 對 LLM Wiki GPL v3 design decision and supports Install built-in CLAUDE.md schema requirement
- [x] 10.2 Schema sections must include: Your Role, Workspace Layout (READ raw/code+wiki, WRITE wiki only, do not touch raw/CLAUDE.md/.git), Wiki Structure (overview/index/log/goals + pages), Workflow per Goal (7 steps + re-run discipline using `wiki/goals/<this-slug>.md` as signal + INDEPENDENT source-dedup signal explanation), Page Conflict, Frontmatter Schema (path-only convention; sha256/at_commit codebus auto-fills; UTC YYYY-MM-DD date convention), WikiLinks (YAML quoting requirement), Source Code References, Stopping Criteria, Failure Modes, Output Format, Workflow per Query — schema teaches agent the 採用 LLM Wiki pattern（incremental persistent wiki） on which the whole system rests
- [x] 10.3 Write unit tests asserting SPDX header present, all 12 section names found as substrings, UTC YYYY-MM-DD convention mentioned, sources path-only instruction with sha256 auto-fill note present, wikilink YAML quoting requirement mentioned

## 11. Commands: init

- [x] 11.1 Implement runInit(repoRoot) in src/commands/init.ts: mkdir all vault subdirs (root/raw/rawCode/wiki/wikiPages/wikiGoals/output), write CLAUDE.md only if missing (preserves user customization), touch goals.jsonl if missing, write internal .codebus/.gitignore (`.lock\nraw/code/\n`), call initNestedRepo, call autoCommit for the initial state — fulfills the init-only invocation behavior of Initialize .codebus/ vault structure under user repo requirement
- [x] 11.2 Add .codebus to source repo .gitignore when source is a git repo requirement — only mutate when `.git/` exists; create `.gitignore` when missing; check existing lines and avoid duplicate entries; ensure trailing newline before append
- [x] 11.3 Init is idempotent requirement — verify by writing a test that runs runInit twice and asserts final state matches once
- [x] 11.4 Write unit tests covering all 5 init scenarios (creates all paths / .gitignore mutation creates file / .gitignore dedup / idempotent two-runs / non-git source skips .gitignore but still creates vault)

## 12. Commands: goal (full ingest sequence)

- [x] 12.1 Implement runGoal({repoRoot, goal, provider, onEvent}) in src/commands/goal.ts — Run ingest flow on --goal invocation requirement: if .codebus/ missing → call runInit first; acquireLock (release in finally); syncRepoToRaw + getSourceVersion + append goals.jsonl line {goal, source_commit, uncommitted, timestamp ISO}; compose system prompt from schema + wiki/index.md (or `(empty)`) + goal text; call provider.invoke with cwd=p.root, vaultRoot=p.root, mode='ingest'; iterate StreamEvent → onEvent callback; then enrichSourceMetadata + flagStalePages + autoCommit
- [x] 12.2 Implement enrichSourceMetadata in src/commands/goal.ts — Enrich newly-written page sources with sha256 and at_commit requirement and Sha256 + at_commit 由 codebus 後處理（agent 不算） design decision: walk wiki/pages/*.md; SKIP pages where every source already has sha256+at_commit (carry-over from prior runs — leave their fingerprints alone so flagStalePages can detect drift); for pages with at-least-one missing source, ENRICH only the missing entries (preserve already-filled per-source) — bug caught review iter-8: unconditional overwrite reset every page's sha256 to current raw hash → flagStalePages compared same-hash-vs-same-hash → never stale, breaking the entire §10 mechanism
- [x] 12.3 Implement flagStalePages in src/commands/goal.ts — Stale-detect compares frontmatter sha256 to current raw and flags drift requirement: walk wiki/pages → compute Map<path, currentSha256> from raw/code/<path> → call detectStaleSources → if isStale changed from prior value, rewrite page with `stale: true`
- [x] 12.4 Write unit tests with FakeProvider (clean repo → goals.jsonl line has uncommitted=false; dirty repo → uncommitted=true; both scenarios assert raw/code/app.ts exists post-sync)

## 13. Commands: query (read-only)

- [x] 13.1 Implement runQuery({repoRoot, query, provider, onEvent}) in src/commands/query.ts — Run query flow on --query invocation requirement: validate .codebus/wiki/pages/ existence and non-empty (Reject query when wiki is empty requirement: throw error containing the `--goal` hint when missing or empty); compose system prompt from schema + wiki/index.md (Compose system prompt from schema and wiki index requirement) + query-mode instruction asking agent to cite via `[[wikilink]]` and not write any files; call provider.invoke with mode='query' and cwd=p.root (Spawn agent in query mode with Write/Edit hard-disabled requirement — disallowedTools includes Write/Edit per Task 6.2); MUST NOT sync raw, MUST NOT append to goals.jsonl, MUST NOT call autoCommit (Query flow does not mutate the vault requirement)
- [x] 13.2 Write unit tests: empty wiki/pages dir → throws with "請先用 --goal" hint; populated wiki → invokes provider with mode='query' and provider observes mode correctly

## 14. CLI entry (commander dispatch + emoji + SIGINT)

- [x] 14.1 Implement src/cli.ts using commander: --repo (default cwd), --goal, --query, --debug, --emoji <auto|on|off>, --no-emoji (sugar for --emoji off), --version 0.1.0, --help mentioning all flags
- [x] 14.2 In main(): declare `const repo = opts.repo` BEFORE registering the SIGINT handler — bug caught review iter-8: if SIGINT fires (or buffered ^C) between handler registration and var initialization, handler accesses repo in TDZ → ReferenceError; SIGINT handler MUST cancel active provider then best-effort unlinkSync(vaultPaths(repo).lock) using `try { unlinkSync } catch (e) { if e.code !== 'ENOENT' swallow }` (race-free, no existsSync TOCTOU window)
- [x] 14.3 Resolve emoji mode via the 5-level priority chain (CLI flag --emoji enum → --no-emoji sugar → NO_EMOJI env → loadGlobalConfig().emoji → 'auto') passed to resolveEmojiMode(detectRuntime())
- [x] 14.4 Dispatch: no goal/query → print start banner + runInit + done/hint banners; --goal → start + goal banners + runGoal({repoRoot, goal, provider: new ClaudeCliProvider(), onEvent: render}) + done/hint; --query → start banner + runQuery
- [x] 14.5 Write smoke tests for cli argv parsing: --version prints 0.1.0; --help mentions --repo / --goal / --query

## 15. E2E smoke (init flow without real LLM)

- [x] 15.1 Write tests/e2e/init-smoke.test.ts: spawn `npx tsx src/cli.ts --repo $tmpDir`; assert .codebus/, .codebus/.git/, .codebus/CLAUDE.md exist; source .gitignore contains .codebus
- [x] 15.2 Verify full vitest suite passes with coverage ≥ 80% via `npx vitest run --coverage`

## 16. Distribution prep (npm publish ready)

- [x] 16.1 Expand README.md with badges (npm version, MIT license), Install section (prerequisite: install @anthropic-ai/claude-code first via npm; then npm install -g codebus), Usage section (3 examples covering init / --goal / --query), Flags table, Settings priority note, mandatory Security warning that goal/query text is fed directly to the LLM and users must not paste content from untrusted sources (phase 2 will add sanitization), License section
- [x] [P] 16.2 Verify `npm run build` produces dist/cli.js + .d.ts files with no tsc errors and `node dist/cli.js --version` prints 0.1.0
- [x] [P] 16.3 Verify `npm pack --dry-run` lists only dist/, LICENSE, README.md, package.json (no src/ tests/ node_modules/)
- [x] [P] 16.4 Final full test suite passes with coverage ≥ 80%
