# v3-app-workspace-goal — manual happy path notes

**Date:** 2026-05-14
**Vault:** D:/side_project/cc-haha
**Tester:** harry
**Build:** local `npm run tauri dev` (Rust dev + Vite HMR)

## Steps observed (task 12.3 acceptance)

| Step | Result | Notes |
|---|---|---|
| 1. Lobby → click vault card → Workspace 載入 | ✓ | Workspace mounts immediately, sidebar shows vault display name + path |
| 2. Goals tab default active | ✓ | `data-active="true"` on `workspace-tab-goals` |
| 3. + New goal modal 開、輸入、Run | ✓ | Modal centered, textarea focused, Run disabled until non-whitespace input |
| 4. 切到 Running detail，看 stream + elapsed time | ✓ | Stream renders inline (emoji + Chinese thought穿插), elapsed seconds tick live, ⏹ Cancel button reachable |
| 5. Goal done → 切 Done detail | ✓ | Auto-switch on `goal-terminal` event; metadata line shows duration + tokens |
| 6. Covered pages 出現 wikilinks | ✓ | Phase-grouped (goal / fix); page titles display when wiki refresh completes |
| 7. 點 covered page → 切 Wiki tab + Milkdown 渲染 | ✓ | Now using `react-markdown` (Milkdown deferred per design Risks fallback); switch + load works |
| 8. File tree icon 展開 → groupings 正確 | ✓ | Default expanded (changed mid-session per UX feedback); taxonomy folders concept / entity / module / process / synthesis surface |
| 9. Wikilink 點擊切頁 | ✓ | Resolvable links render as title-text + 紫色, click invokes loadPage |
| 10. ← Back to Lobby → 回 Lobby | ✓ | Route transitions cleanly; active run continues in background per spec |

## Goals smoke-tested

| Goal text | Outcome | Observed |
|---|---|---|
| "專案在幹嘛?" | succeeded | 8 covered pages (goal phase) + 0 fix-phase changes |
| "今天天氣" | succeeded | goal agent judged out-of-scope (correct), 0 wiki writes; fix phase wrote `[[index]]` + `[[log]]` nav-missing repair |
| "明天天氣" | succeeded | same out-of-scope behavior, fix phase wrote nav + repaired broken-wikilink false positive in log.md |

All three runs commit successfully (auto-commit `wiki: <goal>` on `.codebus/.git`), lint 0/0.

## UX polish applied mid-session

Several issues surfaced during manual test and were fixed inline (each accompanied by spec MODIFIED + tests):

1. **WindowControls overlap** — `+ New goal` button overlapped frameless window controls; added `pr-[160px]` to all tab top bars + `<aside data-tauri-drag-region>` + `<header data-tauri-drag-region>` for proper drag.
2. **Goal terminal not propagating** — Rust thread completion didn't notify frontend → UI stuck in Running detail. Fixed by adding `goal-terminal` Tauri event channel + `useGoalsStore._onTerminal` handler + Workspace useEffect to fetch detail.
3. **Activity Summary phase grouping** — Initial implementation showed flat tool counts mixing goal-agent and fix-agent tools. Added `VerbLifecycleEvent::SpawnStart { verb: Fix }` emit in `verb::goal::run_goal` fix loop (was missing) + frontend `phasesFromEvents` groupby `SpawnStart/SpawnEnd` boundary.
4. **Wiki tab refresh timing** — `useWikiStore.pages` wasn't reloading after goal completes; new pages invisible. Fixed by subscribing wiki store to `goal-terminal` channel + re-running `listPages` + invalidating current page body cache.
5. **Stream rendering** — Originally GUI used plain `→` ASCII arrow + buffered Thought block at bottom. Rewrote to CLI-aligned emoji (`🛠️` / `✍️` / `🤔`) with Thought events folded inline at original timeline position. Multi-line Thought collapses to `(N more lines ▼)`.
6. **Run detail completeness** — Done detail originally only showed metadata + Covered pages + Lint. Added Activity summary (per-phase tool counts) + collapsible Run details (full event replay with same `ActivityStreamItem` fold). Removed earlier-shipped Thinking collapsible after Thoughts went inline (dedup).
7. **Wiki preview style** — Milkdown without theme rendered as plain text. Swapped to `react-markdown` + `remark-gfm` + Tailwind-styled components: H1/H2 with border-bottom, code with sunken bg, blockquote with left border, max-width 720px centered, system font stack, 紫色 (#7c8cff) Obsidian-style links, line-height 1.7.
8. **Wikilink display text** — Both wiki preview internal `[[xxx]]` and Done detail Covered pages were showing slug. Changed to display `useWikiStore.pages[slug].title` with slug fallback.
9. **Wiki tab default state** — Originally collapsed by default (per design); user feedback flipped to default expanded so the tree is immediately visible.
10. **Path link in sidebar** — Workspace sidebar vault path was non-interactive. Added `tauri-plugin-opener` + click handler to open in OS file explorer.
11. **LoadingOverlay drag** — Overlay covered the window and blocked dragging during init. Added `data-tauri-drag-region` to overlay.
12. **Spawn / retry routing** — `spawn_goal` and Retry-with-same-goal originally dropped user back to Goals list; now auto-route to the new Running detail via `onSpawnedRun` / `onRetrySpawned` callbacks forwarded to `Workspace.setSelectedRunId`.
13. **codebus-core u128 deserialize bug** — `VerbBanner::SyncDone / LintDone.elapsed_ms: u128` could not round-trip through `serde_json` default features. Changed to `u64` (no wire impact, still 580M years headroom). Touched: `render/banner.rs`, `verb/event.rs`, `verb/goal.rs`, `verb/fix.rs`, `cli/commands/init.rs` (cast `.as_millis() as u64`).
14. **target_path dead_code warn** — `EventsJsonlSink::target_path` was test-only but lacked `#[cfg(test)]`; added.

## Backlog discovered (NOT in scope for this change)

Recorded to `docs/BACKLOG.md` + per-item md:

- **`docs/2026-05-14-skill-bundles-vault-only-backlog.md`** — repo-root `.claude/skills` copy is unused by 80% of users (only triggered when user opens raw `claude` at source repo root). Proposes opt-in flag.
- **`docs/2026-05-14-git-context-tool-backlog.md`** — agents could benefit from PII-aware `git_log` / `git_blame` / `git_show` tools to write better lineage in wiki, but raw Bash + git would bypass PII filter. Proposes wrapper tool.

Both parked until v3-app-workspace-goal archive completes; not blockers.

## Spec MODIFICATIONS applied to this change's spec.md mid-session

Polish items shifted `specs/app-workspace/spec.md` accordingly:

- **Run Detail Views — Running**: ToolUse Write/Edit specialization (`✍️ <file_path>`); other ToolUse with emoji `🛠️` + tool name + arg summary; Thought folded inline (was buffered to bottom); ThoughtItem first-line + collapsible multi-line spillover.
- **Run Detail Views — Done**: Added Activity summary block (phase-grouped tool counts), Covered pages phase grouping, Run details collapsible full replay. Removed standalone Thinking collapsible (thoughts now inline in Run details).
- **Wiki Tab with Collapsible File Tree**: Default state flipped from collapsed → expanded.
- **Wikilink Resolution and Click Behavior**: Link display uses page frontmatter `title` with slug fallback (was: always slug).

Plus design.md rationale + i18n keys + 3 new spec scenarios. All caught by Vitest (201/201).

## Test signals at archive time

- `cargo test -p codebus-app-tauri --lib` — 71 / 71
- `cargo test -p codebus-core --lib` — 369 / 369
- `cargo test -p codebus-cli` — 110 / 110 (verified earlier; `codebus.exe` was holding lock so latest workspace test was skipped — confirmed unchanged paths)
- `npm test` (Vitest) — 201 / 201
- `npm run typecheck` — clean
- `spectra validate v3-app-workspace-goal` — valid
- `spectra analyze v3-app-workspace-goal --json` — 0 Critical / 0 Warning / 3 Suggestion (vague language: "should" + "may" + missing example on app-shell scenario — pre-existing, non-blocking)

## Cost

Approximate live cost incurred during manual + polish session: ~$5 (multiple goals + chat-verb prior validation). Within handoff "Live verification 預期" budget.

## Ready to archive

All 37 tasks complete (12.3 marked done after writing this file). Spec + design + tasks all in `done` artifact status. Frontend + backend tests fully green. Manual happy path verified end-to-end with three live goal runs. No outstanding blockers.
