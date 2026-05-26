## MODIFIED Requirements

### Requirement: i18n Bundle Coverage Policy

All user-facing strings rendered by `codebus-app` components SHALL be defined as keys in `codebus-app/src/i18n/messages.ts` and consumed through the `useT` hook. "User-facing" SHALL include: visible button labels, link text, headings, body copy, DialogTitle / DialogDescription content, form labels, input placeholders, status badges, error and success messages, toast content, AND assistive-technology attributes (`aria-label`, `aria-description`, `title` attr surfaced as tooltip / accessible name).

The bundle SHALL maintain key parity between `en` and `zh` locales — TypeScript MUST fail to compile if a key exists in `en` but is missing in `zh`, or vice versa.

The following identifier categories SHALL be treated as jargon and SHALL remain English in BOTH locales (i.e., still defined in the bundle for centralization, but `en` and `zh` values are identical English strings):

1. Workspace tab labels: `Goals`, `Wiki`, `Quiz`.
2. Verb names visible in settings UI: `goal`, `query`, `fix`, `verify`, `chat`.
3. Codex effort enum values: `low`, `medium`, `high`, `xhigh`.
4. PII action enum values: `warn`, `mask`, `block`.
5. Config YAML key names rendered as field labels: `base_url`, `api_version`, `keyring_service`.

The following SHALL NOT be required to go through the i18n bundle, because they are program identifiers and not UI labels:

1. Claude API tool name identifiers used in `case` match statements or stream-event discriminants (e.g. `Read`, `Write`, `Glob`, `Grep`, `Edit`, `Bash`).
2. Internal log messages, comments, JSDoc, and developer-facing console output.

Where the same accessibility concept appears in more than one component, the bundle SHALL expose ONE shared key consumed by all sites, rather than per-component duplicate keys.

When a label is composed of an emoji or symbol prefix immediately followed by translatable text (e.g. `🎯 Goal target`, `🚌 Here comes the CodeBus...`), the entire string including the emoji SHALL be stored as a single bundle value. The emoji and text MUST NOT be split into separate keys, because the emoji is part of the label's semantic meaning and translation MUST preserve them as one unit per locale.

#### Scenario: Component renders a user-facing string

- **WHEN** a `codebus-app` component renders any visible label, placeholder, DialogTitle, error message, status badge, button text, or sets an `aria-label` / `title` attribute used as an accessible name
- **THEN** the string MUST be sourced from `t("<key>")` where `<key>` is defined in both `en` and `zh` maps of `codebus-app/src/i18n/messages.ts`

#### Scenario: Adding a jargon term to the bundle

- **WHEN** a jargon identifier from the allow-list (workspace tab label, verb name, codex effort value, PII action value, or config YAML key name) is rendered in UI
- **THEN** the bundle SHALL define a key for it AND the `en` and `zh` values SHALL be identical English strings

##### Example: jargon allow-list bundle entries

| Key | en value | zh value | Allow-list category |
| --- | -------- | -------- | ------------------- |
| `workspace.tab.goals` | `Goals` | `Goals` | tab label |
| `settings.codex.effort.value.high` | `high` | `high` | codex effort enum |
| `settings.pii.action.block` | `block` | `block` | PII action enum |
| `settings.endpoint.field.baseUrl` | `base_url` | `base_url` | config YAML key |

#### Scenario: Shared accessibility key reused across components

- **WHEN** multiple components surface the SAME accessibility concept (such as the "Page not found" tooltip rendered on broken wiki links inside `ChatTranscript`, `ExplanationText`, and `WikiPreview`)
- **THEN** the bundle SHALL define one shared key (e.g. `a11y.pageNotFound`) AND all such components SHALL consume that single key rather than each defining its own

#### Scenario: Emoji-prefixed label stored as one bundle value

- **WHEN** a component renders a label whose visible form is an emoji or symbol prefix followed by translatable text (e.g. activity banner labels such as `🎯 Goal target`, status banners such as `🚌 Here comes the CodeBus...`)
- **THEN** the bundle SHALL define ONE key whose value contains both the emoji and the text in each locale, AND the component MUST NOT concatenate two separate keys for the emoji and the text

##### Example: bannerLabel emoji-text bundle entries

| Key | en value | zh value |
| --- | -------- | -------- |
| `workspace.activity.banner.goal` | `🎯 Goal target: {goalText}` | `🎯 任務目標：{goalText}` |
| `workspace.activity.banner.done` | `🎉 Complete` | `🎉 完成` |

#### Scenario: 6-pattern sweep finds no policy violations

- **WHEN** a reviewer or maintainer runs the canonical 6-pattern grep sweep (Patterns 1a, 1b, 2, 3, 4 against `codebus-app/src/components/**/*.tsx`; Pattern 5 against `codebus-app/src/**/*.{ts,tsx}` for template-literal interpolation with Latin neighbours; Pattern 6 against `codebus-app/src/**/*.ts` outside `components/` for helper / lib files)
- **THEN** every reported line MUST resolve to one of: (a) a `t("...")` call, (b) an entry from the Cat D jargon allow-list, (c) a Claude API tool name identifier from the non-UI exclusion list, or (d) a documented runtime-keyword identifier such as the re-init confirmation literal `delete` in `NewVaultFlow.tsx`. Any unaccounted line constitutes a policy violation requiring a follow-up change.

##### Example: canonical 6-pattern sweep commands

| # | Purpose | Command (run from `codebus-app/`) |
| - | ------- | --------------------------------- |
| 1a | JSX text content with Latin (single-line) | `grep -rPn '>([^<{]*[A-Za-z]+[^<{]*)<' src/components/ --include='*.tsx' \| grep -v '.test.tsx' \| grep -vE 't\("\|\{t\(\|className=\|data-testid='` |
| 1b | Indented JSX text (multi-line) | `grep -rPn "^[[:space:]]+[A-Z][a-zA-Z][a-zA-Z' ]*[a-zA-Z][\.…!\?]?[[:space:]]*$" src/components/ --include='*.tsx' \| grep -v '.test.tsx'` |
| 2 | Emoji / arrow-prefixed Latin string | `grep -rPn '[←→↻⏹⚠✓✕▸▿⤢⤡⏺] [A-Za-z]' src/components/ --include='*.tsx' \| grep -v '.test.tsx'` |
| 3 | Untranslated `aria-label` / `title` / `placeholder` attrs | `grep -rPn '(aria-label\|title\|placeholder)="[^"]*[A-Za-z]' src/components/ --include='*.tsx' \| grep -v '.test.tsx' \| grep -v 't("'` |
| 4 | String literals with placeholder syntax | `grep -rPn "'[A-Za-z][^']*\{\w+\}" src/components/ --include='*.tsx' \| grep -v '.test.tsx'` |
| 5 | Template-literal interpolation with Latin neighbour | `grep -rPn "\`[^\`]*\$\{[^}]+\}[^\`]*[A-Za-z]" src/ --include='*.ts' --include='*.tsx' \| grep -v '.test.' \| grep -v 't("'` |
| 6 | Helper / lib hard-codes outside `components/` | Re-run Patterns 1a / 1b / 2 / 3 / 4 with `src/` as the search root and `--include='*.ts' --include='*.tsx'`, then exclude paths under `src/components/` (already covered by Patterns 1-4) and any `.test.` files |

Pattern 1 was originally written as `>(?![\s{<])([^<{]*[A-Za-z]){2,}[^<{]*<` requiring two or more Latin chunks; that excluded single-word JSX text such as `Checking…` and `Loading…` (Phase 3A residual sweep 2026-05-26). Pattern 1 is now split into 1a (single-line JSX node) and 1b (multi-line JSX text whose Latin word lives on its own indented line) to cover both shapes.

Patterns 5 and 6 were added in the Phase 3A follow-up sweep (2026-05-26) after CDP en-locale smoke surfaced template-literal hard-codes such as `` `${diffHr}h ago` `` in `RunListItem.tsx` and helper-produced strings such as the pass / fail verdict in `src/lib/quiz-parse.ts` that the original 4-pattern sweep could not match. Pattern 5 catches strings inside backtick literals (no `<>` brackets, no symbol prefix, no attribute prefix, double quotes only). Pattern 6 widens the search root from `src/components/` to all of `src/` so that helper modules, formatters, and other `.ts` files outside `components/` are covered.

JSX text starting with non-Latin punctuation (e.g. `+ New goal`, `+ New chat`) remains a known gap of Patterns 1a and 1b — those button labels lose the `[A-Z]` anchor and Latin-chunk count requirement, so they SHALL be discovered by manual CDP smoke or by reviewer pattern-matching rather than by automated sweep. Future follow-up changes MAY introduce Pattern 7 for that shape if recurring instances justify it.
