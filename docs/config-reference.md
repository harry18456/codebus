# codebus config reference

Field reference for the global config file `~/.codebus/config.yaml`.

The file is written for you on first `codebus init` (a starter with every key at
its default), and you can edit it by hand or through the codebus app's Settings
UI. Every key is **optional** — omit a key to use its default. Unknown keys are
silently ignored (forward-compat), so a newer codebus can extend the schema
without breaking an older config.

The file itself carries only a short header pointing here; the per-field
teaching lives in this document rather than as inline comments, because the app
re-serializes the config on save and a YAML serializer does not preserve
comments (so inline teaching would vanish the first time you save from the app).

---

## `pii` — PII scanner for the raw mirror

codebus copies a PII-redacted **mirror** of your repo into the vault for the
agent to read. This section controls that scan.

```yaml
pii:
  scanner: regex_basic
  patterns_extra: []
  on_hit: warn
```

### `pii.scanner` — default `regex_basic`

- `regex_basic` runs the built-in regex set covering common secret shapes (AWS /
  Anthropic / GitHub / Slack / Google / OpenAI / Stripe keys, PEM private keys,
  JWTs, DB connection strings) plus email and IPv4.
- `none` disables PII scanning entirely.

> Do **not** use the bare YAML literal `null` here — YAML parses `null` as the
> null literal, which falls through to the default `regex_basic`. Use the string
> `none` instead.

### `pii.patterns_extra` — default `[]`

A list of extra regex source strings appended to the built-in set. Each entry is
a regex source; every match is treated as Critical severity.

- A malformed regex (one that fails to compile) is rejected at construction with
  a stderr warning, falling back to the built-in set — fix the typo to re-enable
  your custom patterns.
- An **empty or whitespace-only** entry is skipped (it is not a typo to fail on,
  just an unfinished rule) and never becomes a pattern. The app's Settings UI
  also drops blank rows when you save, and the scanner ignores any zero-width
  match, so a stray blank pattern can never flood the scan with one match per
  character.

### `pii.on_hit` — default `warn`

Action on a **Warn-severity** match (email, ipv4):

- `warn` — copy the file to the mirror as-is, emit one stderr warning per match.
- `skip` — do **not** copy the file to the mirror; emit one stderr warning per
  match.
- `mask` — copy the file with each Warn match replaced by
  `[REDACTED:<pattern_name>]`.

> This setting only governs Warn-severity matches. **Critical**-severity matches
> (real API keys, private keys, DB credentials, …) are **always masked**
> regardless of this value — the security floor that keeps real credentials out
> of the mirror in a recoverable form is non-negotiable. Set `mask` for the
> legacy behavior of masking everything (Warn matches included).

---

## `agent` — provider, endpoint, and per-verb model/effort

```yaml
agent:
  active_provider: claude
  providers:
    claude:
      active: system
      system:
        goal:   { model: opus-4-6,   effort: high }
        query:  { model: haiku-4-5,  effort: low }
        fix:    { model: sonnet-4-6, effort: medium }
        verify: { model: opus-4-6,   effort: high }
```

### `agent.active_provider`

Selects which agent CLI drives spawns. Each provider lives under
`providers.<name>` with its own endpoint profiles. Supported providers are
`claude` and `codex`.

### `agent.providers.claude`

Two endpoint profiles are supported; `active` picks which one drives spawns. The
other profile is **cold storage** — codebus does not validate its fields, so you
can park half-edited config there while iterating.

- **`system`** — use your globally configured Claude CLI endpoint (no env
  injection). `model` is a free-string alias such as `opus-4-6`, `opus-4-7`,
  `haiku-4-5`, or `sonnet-4-6` (codebus translates it to the right `--model`
  flag).
- **`azure`** — talk to an Azure AI Foundry Anthropic-compatible endpoint.
  `model` is the Azure deployment name (a free string, passed verbatim). The API
  key is read from the OS keyring; codebus injects `ANTHROPIC_BASE_URL` /
  `ANTHROPIC_API_KEY` / `CLAUDE_CODE_DISABLE_ADVISOR_TOOL` into the child process
  only — it never modifies your parent shell environment.

Each profile carries per-verb `{ model, effort }` sub-blocks for `goal`,
`query`, `fix`, and `verify`. `effort` is one of `low` / `medium` / `high` /
`xhigh` / `max`. The `goal` verb is the reasoning-heavy ingest; `query` is the
fast read-only retrieval; `fix` is the lint-and-edit loop; `verify` is the
content-verification spawn for the quiz / goal verbs (the expensive half of the
"cheap generation + expensive verification" pattern).

An azure profile, when used, looks like:

```yaml
      azure:
        base_url: https://<your-resource>.cognitiveservices.azure.com/anthropic
        keyring_service: codebus-azure
        goal:   { model: <your-opus-deployment-name>,   effort: high }
        query:  { model: <your-haiku-deployment-name>,  effort: low }
        fix:    { model: <your-sonnet-deployment-name>, effort: medium }
        verify: { model: <your-opus-deployment-name>,   effort: high }
```

Store the API key in your OS keyring with `codebus config set-key azure`. The
`codex` provider is configured the same way (system / azure profiles) and is
easiest to set up through the app's Settings UI.

---

## `hooks` — PreToolUse hook gates

Default behaviors are safe; flip individual knobs to `false` at your own risk.

```yaml
hooks:
  read_image_block: true
  read_path_containment: true
```

### `hooks.read_image_block` — default `true`

Controls `codebus hook check-read`, the gate that blocks the agent from reading
image / PDF / binary files (extensions like png / jpg / pdf / gif / webp / bmp /
tiff / ico / heic / heif / avif). Default `true` (block). Set `false` to let the
agent ingest these files — doing so **bypasses the `regex_basic` PII filter**,
which only scans text.

### `hooks.read_path_containment` — default `true`

Confines the agent's Read / Glob / Grep to inside the vault (`raw/code`, `wiki`).
A read whose path canonicalizes outside the vault root is blocked. Default `true`
(contain). Set `false` **only** as an emergency escape hatch — disabling it
re-opens reads of the parent repo and your home-directory files.

---

## `lint` — lint-and-fix loop

```yaml
lint:
  fix:
    enabled: true
```

### `lint.fix.enabled` — default `true`

Whether the post-`goal` lint-and-fix phase runs (and whether `codebus fix` is
allowed when invoked directly). Set `false` to disable both.

---

## `log` — per-run log persistence

```yaml
log:
  sink: jsonl
  # dir: ~/codebus-history
```

### `log.sink` — default `jsonl`

- `jsonl` — append one JSON line per run to `<dir>/runs-YYYY-MM-DD.jsonl`.
- `none` — opt out, no log written. (Use the literal `none`; a bare YAML `null`
  returns a parse error and falls back to the default — the same foot-gun
  avoidance as `pii.scanner: none`.)

### `log.dir` — optional

Output directory. Omit (or comment out) to use the per-vault default
`<vault>/.codebus/log/`. A tilde-prefixed path expands to your home directory.
