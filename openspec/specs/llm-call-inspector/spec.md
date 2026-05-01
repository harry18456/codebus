# llm-call-inspector Specification

## Purpose

TBD - created by archiving change 'llm-call-inspector-p0'. Update Purpose after archive.

## Requirements

### Requirement: `read_audit_jsonl` Tauri command exposes seven workspace audit JSONLs by enum

The Tauri host SHALL expose a `read_audit_jsonl(workspace_root: String, audit_kind: String) -> Result<Vec<serde_json::Value>, String>` command. The `audit_kind` parameter MUST be one of the literal strings `"sanitize"`, `"tool"`, `"reasoning"`, `"token"`, `"llm"`, `"kb_growth"`, `"generator"`, mirroring the seven workspace-level audit JSONL files declared by CLAUDE.md `七層 Audit JSONL`. Any other value MUST return `Err("E_AUDIT_KIND_INVALID")` without touching the filesystem.

The command MUST resolve the target file as `<workspace_root>/.codebus/<filename>.jsonl` where `<filename>` is the canonical name per `_AUDIT_KIND_TO_FILENAME` mapping (e.g., `"llm"` → `"llm_calls.jsonl"`, `"kb_growth"` → `"kb_growth.jsonl"`, `"reasoning"` → `"reasoning_log.jsonl"`, etc.). The mapping MUST live in a single Rust constant whose values are the canonical filenames listed in `sidecar/src/codebus_agent/_audit_paths.py` `_<NAME>_FILENAME` constants — drift between Rust and Python is a defensive-test failure (see `Requirement: Audit kind filename mapping defensive parity`).

The command MUST validate the resolved path through `validate_audit_path(workspace_root, audit_kind)` BEFORE opening the file. The validator MUST enforce all of:

1. `workspace_root` is absolute, exists, and is a directory; otherwise `Err("E_WORKSPACE_INVALID")`.
2. The resolved path's relative segment starts with `.codebus/` (forward-slash normalised).
3. No segment equals `..` or `.`; no segment ends with `.` or space; no segment contains `:` (Windows ADS / drive); no segment matches Windows reserved names (`con`, `prn`, `aux`, `nul`, `com1`–`com9`, `lpt1`–`lpt9`) at the case-folded stem level.
4. The file extension is exactly `.jsonl` (case-insensitive); otherwise `Err("E_INVALID_PATH")`.
5. Canonicalised resolved path starts with the canonicalised workspace root (post-symlink resolution); otherwise `Err("E_DENIED")`.

When the file does not exist on disk, the command MUST return `Ok(vec![])` (not `Err`), so empty audit logs surface as empty lists rather than errors. When the file exists but is not a regular file (e.g., directory or symlink to outside workspace), the command MUST return `Err("E_NOT_REGULAR_FILE")`. When `std::fs::metadata().len()` returns more than `5 * 1024 * 1024` bytes, the command MUST return `Err("E_AUDIT_TOO_LARGE")` without parsing.

For sizes ≤ 5 MB, the command MUST read the file as UTF-8, split on `\n`, skip empty lines, and parse each line as JSON. A line that fails JSON parsing MUST be SKIPPED with a `log::warn!` — not propagated as an error — so a single corrupt line does not poison the whole list.

#### Scenario: Valid llm kind returns parsed entries

- **WHEN** the command is invoked with a valid `workspace_root` containing a 3-line `llm_calls.jsonl` and `audit_kind = "llm"`
- **THEN** the result MUST be `Ok(entries)` where `entries.len() == 3`
- **AND** each entry MUST be a JSON object containing at least `timestamp`, `role`, `model`, `prompt_tokens`, `completion_tokens` keys

#### Scenario: Unknown audit_kind rejected without filesystem access

- **WHEN** the command is invoked with `audit_kind = "secrets"` (not in the seven-tab enum)
- **THEN** the result MUST be `Err("E_AUDIT_KIND_INVALID")`
- **AND** no `<workspace_root>/.codebus/secrets.jsonl` resolution MUST be attempted (verifiable via no `metadata()` call on that path)

#### Scenario: Missing audit file returns empty vec, not error

- **WHEN** the command is invoked with `audit_kind = "llm"` and `<workspace_root>/.codebus/llm_calls.jsonl` does not exist
- **THEN** the result MUST be `Ok(vec![])`
- **AND** the result MUST NOT be any `Err` variant

#### Scenario: Path escape via `..` rejected

- **WHEN** any path resolution somehow produces a relative segment containing `..` (defensive — current call shape cannot, but a future mutation might)
- **THEN** `validate_audit_path` MUST reject with `Err("E_INVALID_PATH")`
- **AND** the file MUST NOT be opened

#### Scenario: Symlink escape rejected

- **WHEN** `<workspace_root>/.codebus/llm_calls.jsonl` is a symlink whose canonicalised target lives outside `<workspace_root>`
- **THEN** the command MUST return `Err("E_DENIED")`
- **AND** the file MUST NOT be read

#### Scenario: File over 5 MB rejected pre-parse

- **WHEN** the target file's `metadata().len()` reports `5 * 1024 * 1024 + 1` or more
- **THEN** the command MUST return `Err("E_AUDIT_TOO_LARGE")`
- **AND** no parse work MUST occur (verifiable via timing or a single `metadata()` call)

#### Scenario: Corrupt JSONL line skipped without poisoning result

- **WHEN** the file contains 3 lines where line 2 is `{not valid json` and lines 1 and 3 are valid JSON objects
- **THEN** the result MUST be `Ok(entries)` where `entries.len() == 2` (line 2 dropped)
- **AND** a `log::warn!` MUST have been invoked for line 2

---
### Requirement: Audit kind filename mapping defensive parity

The Rust `_AUDIT_KIND_TO_FILENAME` constant in `tauri/src-tauri/src/audit_files.rs` SHALL contain exactly seven `(audit_kind, filename)` pairs whose filenames equal the values of `_<NAME>_FILENAME` constants in `sidecar/src/codebus_agent/_audit_paths.py`. A defensive test MUST grep both files and assert pair equality so the two language sides cannot drift independently.

The Rust mapping MUST list pairs in this exact order to mirror the seven-tab UI declaration in `frontend-shell` Requirement `AuditPanel surfaces seven workspace-level audit JSONL tabs`:

```
("sanitize",  "sanitize_audit.jsonl")
("tool",      "tool_audit.jsonl")
("reasoning", "reasoning_log.jsonl")
("token",     "token_usage.jsonl")
("llm",       "llm_calls.jsonl")
("kb_growth", "kb_growth.jsonl")
("generator", "generator_log.jsonl")
```

#### Scenario: All seven kinds map to the canonical filename

- **WHEN** a defensive Rust integration test iterates `_AUDIT_KIND_TO_FILENAME`
- **THEN** all seven kinds listed above MUST appear with the exact filename strings
- **AND** the order MUST match the seven-tab UI order

#### Scenario: Defensive parity test catches Python-side drift

- **WHEN** a hypothetical mutation renames `_LLM_FILENAME = "llm_calls.jsonl"` to `_LLM_FILENAME = "llm.jsonl"` in `_audit_paths.py`
- **THEN** the cross-language defensive test MUST FAIL with a clear message naming both the Rust and Python sides

---
### Requirement: `useAuditJsonl` composable wraps the Tauri command with optional live-tail

The frontend SHALL expose a composable `web/app/composables/useAuditJsonl.ts` exporting `useAuditJsonl(workspaceRoot: string, kind: AuditKind, opts?: { liveTailFromExplorerStream?: UseExplorerStreamApi }): UseAuditJsonlApi`. The `AuditKind` TypeScript type MUST equal the literal union `"sanitize" | "tool" | "reasoning" | "token" | "llm" | "kb_growth" | "generator"`; passing any other string MUST be a TypeScript compile-time error.

The composable MUST:

1. Invoke `Tauri::invoke('read_audit_jsonl', { workspaceRoot, auditKind: kind })` once on construction; populate `entries: Ref<unknown[]>` with the parsed JSON entries (timestamp ascending — the disk file is append-only by write order).
2. Expose `loading: Ref<boolean>` (true during the initial invoke), `error: Ref<Error | null>` (set on `E_*` rejections), and a `reload(): Promise<void>` for explicit re-fetch.
3. When `opts?.liveTailFromExplorerStream` is provided AND `kind === "llm"`, watch the supplied `useExplorerStream` instance's underlying SSE event stream and append every `llm_call` event payload into `entries` (keeping timestamp-ascending order). For other kinds, the live-tail option MUST be ignored at runtime (no error, no-op).
4. Dedup live-tail appends by `request_id` — if an entry with the same `request_id` already exists in `entries`, the live-tail event MUST NOT push a duplicate.
5. NOT open a second EventSource. Live-tail piggybacks on the caller-provided `useExplorerStream` instance; the composable itself MUST NOT call `useSseTask` or instantiate `EventSource` directly.

The composable MUST surface `error.value` as a non-null `Error` whose `message` includes the raw `E_*` code on Tauri command rejection (e.g., `Error("E_AUDIT_TOO_LARGE")`); UI consumers branch on the message to render appropriate fallback (e.g., "audit too large to inline").

#### Scenario: Initial load populates entries from Tauri command

- **WHEN** `useAuditJsonl('/abs/ws', 'llm')` is constructed and the Tauri command resolves with three entries
- **THEN** `entries.value` MUST equal those three entries in the order returned by the command
- **AND** `loading.value` MUST flip to `false` after the promise resolves

#### Scenario: Live-tail appends llm_call SSE events while explorer stream emits them

- **WHEN** `useAuditJsonl('/abs/ws', 'llm', { liveTailFromExplorerStream: stream })` is constructed and `stream` later receives two `llm_call` SSE events with distinct `request_id` values
- **THEN** `entries.value.length` MUST equal `<initial disk count> + 2` after the second event is dispatched
- **AND** the two new entries MUST be appended at the end of `entries.value` (timestamp-ascending preserved)

#### Scenario: Live-tail ignores non-llm kinds

- **WHEN** `useAuditJsonl('/abs/ws', 'sanitize', { liveTailFromExplorerStream: stream })` is constructed and `stream` receives `llm_call` events
- **THEN** `entries.value` MUST NOT receive any new entries from those events
- **AND** no error MUST be raised

#### Scenario: Dedup by request_id prevents disk + SSE double-push

- **WHEN** the disk load contains an entry with `request_id: "req_abc"` AND a subsequent live-tail `llm_call` event arrives carrying the same `request_id: "req_abc"`
- **THEN** `entries.value.length` MUST NOT increase
- **AND** the existing entry MUST remain in place (no replacement)

#### Scenario: E_AUDIT_TOO_LARGE surfaces as Error with code in message

- **WHEN** the Tauri command rejects with `E_AUDIT_TOO_LARGE`
- **THEN** `error.value` MUST be a non-null `Error` instance whose `.message` contains the literal substring `"E_AUDIT_TOO_LARGE"`
- **AND** `entries.value` MUST remain `[]`

---
### Requirement: `LlmCallInspector` overlay renders four tabs and prev/next navigation

The frontend SHALL ship `web/app/components/audit/LlmCallInspector.vue` as a drawer overlay component accepting these props:

```ts
defineProps<{
  rows: LlmCallEntry[]
  activeIndex: number | null  // null = closed
}>()

defineEmits<{
  (e: 'close'): void
  (e: 'select-index', index: number): void
}>()
```

When `activeIndex === null`, the component MUST render nothing (return empty fragment / `v-if="activeIndex !== null"` at root). When `activeIndex` is a valid index into `rows`, the component MUST render an `<aside>` overlay with these regions in order:

1. **Header**: title `LLM Call Inspector`, request_id, step_id (or `—` when null), timestamp; prev/next buttons emitting `select-index` with `activeIndex - 1` / `activeIndex + 1` (clamped to `[0, rows.length - 1]`); close button emitting `close`. Display `{activeIndex + 1} / {rows.length}` between prev/next.
2. **Status strip**: badges for `role`, `module`, `model`, `sanitizer_pass2_applied: true → "Pass 2 sanitize ON"` (purple), HTTP status code (green when present, neutral when null); right-aligned `latency {n} ms · cost ${n}` (em-dash when either field null).
3. **Tab switcher** with four tabs in fixed order: `Wire payload` / `Response` / `Tokens & cost` / `Timeline`. The first tab MUST be active by default when `activeIndex` changes.
4. **Tab body** rendering per active tab.

For `Wire payload` tab body: render `request.messages` as a syntax-highlighted JSON pre-block. When `sanitizer_pass2_applied === true`, render a small banner above the pre-block reading "Pass 2 sanitize ON · pre-sanitize values are not stored (D-015)". The component MUST NOT render any pre-sanitize column / diff view; pre-sanitize disclosure is out of scope per `agent-console-p0` archive.

For `Response` tab body: render `response` as a pretty JSON pre-block. When `response === null`, render "(no response — call may have failed; see error field if present)" along with `entry.error?.message` when defined.

For `Tokens & cost` tab body: render a small key-value table with `prompt_tokens` / `completion_tokens` / `total = prompt + completion` / `cost_usd` (em-dash when null) / `latency_ms` (em-dash when null).

For `Timeline` tab body: render a small structured list `module: <module>` / `role: <role>` / `step: <step_id ?? "—">` / `provider: <provider_id>` / `call_type: <call_type>`. P0 does NOT render a multi-call timeline visualization; the tab is a single-call summary.

Pressing the Escape key MUST emit `close` (component listens via `@keydown.esc.window` directive on the root).

#### Scenario: activeIndex null hides the overlay

- **WHEN** `<LlmCallInspector :rows="someRows" :active-index="null" />` mounts
- **THEN** the rendered DOM MUST contain zero `<aside>` elements

#### Scenario: All four tabs render in canonical order

- **WHEN** the inspector is open with a valid entry
- **THEN** the rendered tab strip MUST contain exactly four buttons whose `data-tab` attribute values equal `wire`, `response`, `tokens`, `timeline` in that left-to-right order

#### Scenario: Prev/next emit clamped index

- **WHEN** the inspector is open with `activeIndex: 0` and the user clicks "prev"
- **THEN** the `select-index` emit MUST receive `0` (clamped, NOT `-1`)

- **WHEN** the inspector is open with `activeIndex: rows.length - 1` and the user clicks "next"
- **THEN** the `select-index` emit MUST receive `rows.length - 1` (clamped)

#### Scenario: Sanitize banner appears when pass 2 applied

- **WHEN** the active entry has `sanitizer_pass2_applied: true`
- **THEN** the Wire payload tab body MUST contain a banner element whose text contains the literal substring `"Pass 2 sanitize ON"`
- **AND** the banner MUST contain the literal substring `"D-015"`

#### Scenario: tokens_used 0 renders em-dash for cost when null

- **WHEN** the active entry has `cost_usd: null`
- **THEN** the Tokens & cost tab MUST render `—` (U+2014) in the cost cell
- **AND** MUST NOT render `$0` or `$null`

#### Scenario: Escape key closes the overlay

- **WHEN** the inspector is open and a `keydown` event with `key === "Escape"` fires on `window`
- **THEN** the component MUST emit `close`

---
### Requirement: `/audit/llm` page surfaces the inspector standalone

The frontend SHALL ship a Nuxt page at `web/app/pages/audit/llm.vue`. The page accepts a `?ws_path=` query parameter (absolute workspace path); when missing, render an error message identical in shape to the `/explorer/[task_id]` invalid-path page (refusing to call any IPC).

The page MUST construct one `useAuditJsonl(ws_path, 'llm')` instance and render:

- A left-rail list of entries (timestamp descending — server returns ascending, the page reverses for display) with timestamp / role / module / model / prompt+completion tokens / cost. Click selects the row and opens `<LlmCallInspector>` overlay over the right portion of the layout.
- Filter chips above the list for `role` (`reasoning` / `judge` / `chat` / `embed` / `pii_detection`) and `module` (`kb_build` / `kb_query` / `reasoning` / `judge` / `chat` / `coverage` / `generate` / `qa_agent`); chips are toggle-able multi-select; empty selection means "show all"; chip selection state lives in `ref<Set<string>>` (no URL persistence in P0).
- Empty state when `entries.value.length === 0 && !loading.value`: "no LLM calls in this workspace yet — run an Explorer or Q&A task to populate."
- Loading state when `loading.value === true`: a subtle spinner / "loading audit log…" label.
- Error state when `error.value !== null`: render `error.value.message`; if message contains `"E_AUDIT_TOO_LARGE"`, render the dedicated copy from the Tokens & cost tab description ("audit too large for inline view").

#### Scenario: Missing ws_path renders error without IPC call

- **WHEN** the user navigates to `/audit/llm` (no `?ws_path=`)
- **THEN** no `Tauri::invoke('read_audit_jsonl', ...)` MUST occur
- **AND** the page MUST display the missing-ws_path error message

#### Scenario: Row click opens inspector with the clicked index

- **WHEN** the user clicks the third row in the list
- **THEN** the `LlmCallInspector` MUST mount with `activeIndex` equal to the third row's index in the underlying entries array (post-reverse-for-display: page MUST translate display index back to underlying index when emitting select)

#### Scenario: Filter chip narrows the visible list

- **WHEN** the user toggles ON the `role: reasoning` chip while entries contain mixed roles
- **THEN** only entries with `role === "reasoning"` MUST be visible in the list
- **AND** the inspector's `:rows` prop MUST receive the same filtered subset (so prev/next iterates only filtered entries)

#### Scenario: Empty entries shows the empty state

- **WHEN** `useAuditJsonl` resolves with `entries.value === []`
- **THEN** the rendered DOM MUST contain the literal substring "no LLM calls in this workspace yet"
- **AND** the inspector MUST NOT be rendered

---
### Requirement: Explorer console page reuses the same inspector overlay

The page `web/app/pages/explorer/[task_id].vue` SHALL bind the existing `AuditPanel` `llm` tab to live `useAuditJsonl(ws_path, 'llm', { liveTailFromExplorerStream: stream })` rows. When the active tab is `"llm"`, the AuditPanel `:rows` MUST be the audit list; when other tabs are active, `:rows` MUST be either the appropriate other source (already wired for `reasoning` per `agent-console-p0`) or `[]`.

When the user clicks a row in the AuditPanel `llm` tab list, the page MUST open `<LlmCallInspector>` overlay (mounted in the page template, not inside AuditPanel) with the clicked entry's index. The overlay MUST share the same prev/next + close behavior as the standalone `/audit/llm` page; the two contexts use the same component, only the host differs.

The page MUST acquire `ws_path` from the same source it already uses for `read_tutorial_file` calls (the `?ws_path=` query parameter mirrored from R-01 routes, OR — if absent — render an error indicating ws_path is required for live audit binding).

#### Scenario: Explorer page binds llm tab to live audit rows

- **WHEN** the user is on `/explorer/explore_4f2a8b91?ws_path=/abs/ws` and switches AuditPanel active tab to `"llm"`
- **THEN** `<AuditPanel :rows="..." />` MUST receive the entries from `useAuditJsonl(ws_path, 'llm', { liveTailFromExplorerStream: explorerStream })`
- **AND** as new `llm_call` SSE events arrive, the AuditPanel rows MUST live-update

#### Scenario: Row click in explorer page opens the same inspector overlay

- **WHEN** the user clicks a row in the AuditPanel `llm` tab while on the explorer page
- **THEN** the page MUST mount `<LlmCallInspector>` with `:active-index` equal to the clicked row's index in the `useAuditJsonl` entries

#### Scenario: Missing ws_path on explorer page renders fallback for the llm tab

- **WHEN** the user navigates to `/explorer/explore_4f2a8b91` without `?ws_path=`
- **THEN** the AuditPanel `llm` tab MUST render an empty / error state explaining ws_path is required for audit binding
- **AND** the explorer SSE stream MUST still open (the existing `agent-console-p0` route validation does not require ws_path)

---
### Requirement: LlmCallInspector renders provider id and filters PII detection role

The `<LlmCallInspector>` component SHALL render the `provider_id` field of the active `LlmCallEntry` in the status strip alongside the existing `role` / `module` / `model` badges. The provider id MUST appear as a chip with `data-testid="llm-inspector-provider-id"` and MUST display the literal id (e.g., `openai-default`) — not the resolved `base_url` or any derived nickname.

The component SHALL accept a new prop `hidePiiDetection: boolean` (default `true`) which, when true, causes the inspector to skip rows whose `role === "pii_detection"` from the prev/next navigation chain. When `hidePiiDetection` is `false`, all rows in the input array participate in navigation regardless of role.

The component SHALL display, in the header region below the prev/next buttons, a small banner of the form `"+ N PII detection call(s) hidden"` whenever `hidePiiDetection === true` and at least one `pii_detection` row exists in the input. The banner MUST be a `<button>` element with `data-testid="llm-inspector-toggle-pii"`; clicking it MUST emit a new event `(e: 'toggle-pii-visible'): void` so the parent page can flip the prop.

#### Scenario: Provider id chip rendered

- **WHEN** the inspector is open with an entry whose `provider_id == "openai-default"`
- **THEN** the rendered DOM MUST contain an element matching `[data-testid="llm-inspector-provider-id"]`
- **AND** that element's text content MUST equal `openai-default`

#### Scenario: PII rows excluded from navigation by default

- **WHEN** the inspector is mounted with rows containing 3 chat entries and 2 pii_detection entries (default `hidePiiDetection: true`)
- **THEN** clicking next from the last chat entry MUST clamp at the last chat entry, not advance into a pii_detection entry
- **AND** the rendered count display MUST read `3 / 3` (chat rows only), not `5 / 5`

#### Scenario: Toggle button surfaces hidden count

- **WHEN** rows include 2 pii_detection entries and `hidePiiDetection === true`
- **THEN** the inspector MUST render a button with `data-testid="llm-inspector-toggle-pii"`
- **AND** the button text content MUST contain the literal `"2"` (the hidden count)

#### Scenario: Toggle emits event

- **WHEN** the user clicks the toggle button
- **THEN** the inspector MUST emit `toggle-pii-visible` with no payload

---
### Requirement: AuditPanel filters llm tab rows by role for PII separation

The `<AuditPanel>` component SHALL accept a new optional prop `hidePiiDetection: boolean` (default `true`). When the active tab is `llm` and `hidePiiDetection === true`, the rows passed to the panel MUST be filtered to exclude `role === "pii_detection"` entries before count display, row rendering, and selection events. The panel MUST display the same toggle banner (mirroring the inspector) at the top of the body region when at least one pii_detection row exists; clicking the banner MUST emit a new event `(e: 'toggle-pii-visible'): void`.

The other six audit tabs (`sanitize`, `tool`, `reasoning`, `token`, `kb_growth`, `generator`) MUST NOT be affected by this prop — they do not carry a `role: "pii_detection"` concept.

#### Scenario: llm tab count excludes pii rows by default

- **WHEN** `<AuditPanel :active-tab="'llm'" :rows="[...]" :counts="{ ...counts, llm: 5 }" />` is rendered with 3 chat rows and 2 pii_detection rows
- **THEN** the displayed row count for the `llm` tab MUST read `3` (not `5`)
- **AND** the rendered list MUST contain exactly 3 row elements

#### Scenario: Sanitize tab unaffected

- **WHEN** `<AuditPanel :active-tab="'sanitize'" :hide-pii-detection="true" />` is rendered
- **THEN** the panel MUST behave identically to the case where `hide-pii-detection` is omitted
- **AND** no PII-related toggle banner MUST appear on the sanitize tab
