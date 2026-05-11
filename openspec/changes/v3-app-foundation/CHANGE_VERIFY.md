# v3-app-foundation — Manual Verification Log

The automated test suite covers tasks 1.x – 12.1 (~408 tests across Rust
workspace + frontend vitest). Two manual tasks remain in this change; a
third (cross-platform regression) is formally deferred per the roadmap's
Cross-platform policy.

## Task 12.2 — Zero outbound network in Lobby / Settings flow

Open Tauri devtools (F12) → Network panel → enable Preserve log →
Clear log. Walk through:

1. App boot (Lobby empty state visible)
2. `+ Board a new bus` → file picker → pick a fresh repo folder →
   loading overlay → vault card appears
3. Drag-drop a folder with an existing `.codebus/` onto the Lobby window
   → detection dialog → Just-Bind → vault appears
4. Open Settings (bottom-left gear) → change Quiz pass threshold →
   Save → "Saved" toast

Expected: **zero** outbound HTTP requests in the network panel for the
entire flow. Allowed: `localhost:1420` / `localhost:1421` (Vite dev +
HMR) and `data:` URLs only.

- Outcome (2026-05-11, Windows MSVC): **PASS** — verified during the
  session that walked steps 1–4 with devtools network panel open. No
  non-localhost requests observed.

## Task 13.1 — Windows MSVC acceptance checklist

Run `cargo tauri dev` on the Windows MSVC dev machine. Walked items:

- [x] First launch shows empty state with 🚌 emoji + locale-appropriate
      title (zh-tw → "來搭第一台公車吧").
- [x] `+ Board a new bus` → fresh repo → loading overlay → vault card
      in list.
- [x] Quit app, relaunch → vault card persists (proves
      `~/.codebus/app-state.json` round-trip).
- [x] Drag a repo folder with an existing `.codebus/` onto Lobby window
      → detection dialog → Just-Bind → vault added.
- [x] Settings → Quiz pass threshold → Save → reopen → slider reads
      saved value → `~/.codebus/config.yaml` contains
      `app.quiz.pass_threshold: 75` and the enriched
      `app.quiz.default_length: 5`.
- [x] Click vault card → Workspace stub renders → `← 回到 Lobby` returns
      to Lobby (state preserved).

- Outcome (2026-05-11, Windows MSVC dev machine): **PASS**.

## Task 13.2 — Cross-platform verification deferral

Per `docs/v3-app-roadmap.md` "Cross-platform policy", macOS / Linux
acceptance verification for **all v3-app-* changes** is consolidated
into `v3-app-polish-ship`. This change does not run a mac/linux pass.

Rationale (full text in roadmap):

1. Primary dev machine is Windows; three-platform gate per change costs
   velocity
2. Cross-platform builds + installer already scoped to polish-ship, so
   bundling manual verification there avoids verifying twice
3. polish-ship will introduce E2E infra that may partially automate the
   regression

The task's work IS done in this change — the deferral decision was
captured in three places (roadmap policy section, tasks.md 13.2
rewrite, this note).

## Polish / quality fixes added during the verification session

Beyond the 34 spec'd tasks, the verification session surfaced UX gaps
and edge-case bugs. The fixes shipped in this change too (each
discussed-then-implemented inline):

- App icon: real amber bus SVG + full Tauri icon set
  (`icon.ico` / `icon.icns` / multi-res PNG / Windows Store square /
  Android / iOS) generated via `cargo tauri icon`
- Favicon for the Vite dev webview
- Frameless window: `decorations: false` + custom
  `WindowControls` (min / restore-or-maximize / close, locale-aware aria
  labels, z-60 stays above modal overlay)
- `LoadingOverlay` with bus-roll CSS animation during init-heavy
  `addVault` modes (detect / re_init)
- `Toast` for vault errors (vault_already_exists, vault_not_found, etc.)
- Path normalize: strip `\\?\` Windows verbatim prefix on both write
  (add_vault) and read (list_vaults migrates legacy entries)
- `DropTargetOverlay` shown on `tauri://drag-enter`, hides on leave/drop
- Global `cursor: pointer` for interactive elements
  (buttons / `[role="button"]` / menuitems / anchors)
- Full i18n: `src/i18n/messages.ts` (zh + en flat keys) +
  `useT(localeOverride?)` hook + `tStatic` for non-React + shared
  `LocalizedError` helper for stores
- PII scanner dropdown `onValueChange` wired (previously a dead select)
- Log sink "Change folder" wired with Tauri folder picker; new "reset
  to per-vault default" affordance
- Per-field reset buttons for 5 of 7 Settings fields (Default model
  goal/query/fix, PII scanner, Quiz pass threshold, Quiz length, Log
  dir); always visible + disabled when at default + tooltip
- `AppQuizConfig` Rust struct now `#[serde(default)]` so partial
  frontend patches (e.g. only `pass_threshold`) deserialize cleanly;
  `save_global_config_at` enriches `app.*` so on-disk YAML always
  carries both fields
