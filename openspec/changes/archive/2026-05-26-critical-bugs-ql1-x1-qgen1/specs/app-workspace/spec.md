## ADDED Requirements

### Requirement: Activity Stream Shell Command Wrapper Extraction

The codebus-app activity stream renderer SHALL display Shell tool invocations using the inner command the user authored, not the OS-specific wrapper the agent runtime wraps it in. When the raw `command` field of a Shell tool invocation matches a recognized wrapper shape, the renderer SHALL extract and display the inner command verbatim; the wrapper prefix and any surrounding quotes SHALL NOT count against the display character budget.

The three recognized wrapper shapes SHALL be:

1. **PowerShell wrapper** — a path ending in `powershell.exe` (case-insensitive, with or without surrounding quotes), optionally followed by zero or more leading PowerShell switch flags (each shaped `-<word>`, e.g. `-NoProfile`, `-NoLogo`, `-NonInteractive`), then `-Command` (case-insensitive), then the inner command (optionally enclosed in single or double quotes). The path MAY be a Windows absolute path containing spaces (e.g., `C:\Windows\System32\WindowsPowerShell\v1.0\powershell.exe`). Real-world Codex sandbox invocations have been observed using both the bare `-Command` and the `-NoProfile -Command` forms; both SHALL be stripped.
2. **POSIX shell -c wrapper** — `sh` or `bash` (with or without a leading absolute path such as `/bin/`), followed by `-c`, followed by the inner command (optionally enclosed in single or double quotes).
3. **No wrapper recognized** — the raw command is passed through unchanged.

After extraction, the renderer SHALL truncate the displayed inner command to a maximum of 80 visible characters (matching the existing `summarizeToolInput` truncation cap), appending an ellipsis when truncation occurs. The truncation cap SHALL be applied to the extracted inner command, not to the raw wrapped command.

The extraction SHALL NOT mutate the underlying tool-use event payload (the raw wrapped command remains available in the per-run events.jsonl and in any debug / verbose surface).

#### Scenario: PowerShell wrapper is stripped before truncation

- **WHEN** the renderer receives a Shell tool invocation whose `command` is `"C:\Windows\System32\WindowsPowerShell\v1.0\powershell.exe" -Command "Get-Content package.json | Select-Object -First 50"`
- **THEN** the displayed command SHALL begin with `Get-Content package.json` AND SHALL NOT contain `powershell.exe` or `-Command` AND SHALL NOT have been truncated within the wrapper prefix

#### Scenario: PowerShell wrapper with leading switch flags is stripped

- **WHEN** the renderer receives a Shell tool invocation whose `command` is `"C:\Windows\System32\WindowsPowerShell\v1.0\powershell.exe" -NoProfile -Command "Get-ChildItem -Recurse -File wiki"`
- **THEN** the displayed command SHALL begin with `Get-ChildItem -Recurse -File wiki` AND SHALL NOT contain `powershell.exe` or `-NoProfile` or `-Command`

#### Scenario: PowerShell wrapper with multiple leading switch flags is stripped

- **WHEN** the renderer receives a Shell tool invocation whose `command` is `powershell.exe -NoLogo -NonInteractive -NoProfile -Command "Get-Date"`
- **THEN** the displayed command SHALL begin with `Get-Date` AND SHALL NOT contain any of `powershell.exe`, `-NoLogo`, `-NonInteractive`, `-NoProfile`, or `-Command`

#### Scenario: PowerShell wrapper around a multi-line here-string inner command is stripped

- **WHEN** the renderer receives a Shell tool invocation whose `command` is a PowerShell wrapper whose inner command is a PowerShell here-string (begins with `@'`, ends with `'@`, contains newlines), e.g. `"…\powershell.exe" -Command "@'<NL>line 1<NL>line 2<NL>'@"`
- **THEN** the inner command SHALL still be extracted (the renderer SHALL tolerate newlines inside the inner command, not stop at the first line break)

#### Scenario: POSIX sh -c wrapper is stripped before truncation

- **WHEN** the renderer receives a Shell tool invocation whose `command` is `/bin/sh -c "git log --oneline -n 20"`
- **THEN** the displayed command SHALL begin with `git log --oneline -n 20` AND SHALL NOT contain `/bin/sh` or `-c`

#### Scenario: bash -c wrapper is stripped

- **WHEN** the renderer receives a Shell tool invocation whose `command` is `bash -c 'grep -r "AppShell" src/'`
- **THEN** the displayed command SHALL begin with `grep -r` AND SHALL NOT contain `bash -c`

#### Scenario: Unrecognized command passes through unchanged

- **WHEN** the renderer receives a Shell tool invocation whose `command` is `git status --short` (no wrapper)
- **THEN** the displayed command SHALL be `git status --short`

#### Scenario: Inner command exceeding 80 chars is truncated after extraction

- **WHEN** the renderer receives a Shell tool invocation whose `command` wraps a 200-character inner command with a PowerShell wrapper
- **THEN** the displayed command SHALL contain the first 80 characters of the extracted inner command followed by an ellipsis AND SHALL NOT contain any portion of the wrapper prefix

##### Example: wrapper-detection table

| Raw `command` | Displayed (post-extraction, pre-truncation) |
|---|---|
| `"C:\Windows\System32\WindowsPowerShell\v1.0\powershell.exe" -Command "Get-Date"` | `Get-Date` |
| `"C:\Windows\System32\WindowsPowerShell\v1.0\powershell.exe" -NoProfile -Command "Get-ChildItem"` | `Get-ChildItem` |
| `powershell.exe -NoLogo -NonInteractive -NoProfile -Command "Get-Date"` | `Get-Date` |
| `powershell.exe -Command "ls D:\"` | `ls D:\` |
| `/bin/sh -c "echo hi"` | `echo hi` |
| `bash -c 'cat foo.txt'` | `cat foo.txt` |
| `sh -c "ls -la"` | `ls -la` |
| `git status` | `git status` |

---

### Requirement: Activity Stream Internal Sentinel Marker Filter

The codebus-app activity stream renderer SHALL NOT render internal `[CODEBUS_*]` sentinel markers as raw user-facing text. These markers are an agent ↔ codebus-core wire protocol (e.g., `[CODEBUS_QUIZ_SCOPE]`, `[CODEBUS_QUIZ_NO_MATCH]`, `[CODEBUS_QUIZ_NO_VALIDATE]`, `[CODEBUS_QUIZ_VIOLATION]`) and exposing them raw produces text that reads as a defect to end users.

When a thought block's text begins with a `[CODEBUS_*]` token (an opening `[`, the literal `CODEBUS_`, an uppercase ASCII / underscore identifier, a closing `]`), the renderer SHALL apply the following display rules:

1. When the marker has a registered user-facing translation (sourced from `codebus-app/src/i18n/messages.ts` under a key namespaced by the marker name), the renderer SHALL render the translated text in the active locale (zh-tw / en) in place of the raw marker-prefixed line. The remainder of the marker's payload MAY be appended after the translation when it carries information meaningful to the user (e.g., a reason string).
2. When the marker is `[CODEBUS_*]` but has no registered translation, the renderer SHALL suppress the marker-prefixed line entirely (render nothing for that thought block). The renderer SHALL NOT render the literal `[CODEBUS_…]` substring as user-facing text under any fallback path.

The first registered translation SHALL be `[CODEBUS_QUIZ_NO_VALIDATE]` with zh-tw value `codex 沙箱無法跑 quiz 結構驗證，跳過此步` and a matching en value. Future markers MAY be added to the same registry without renderer changes.

The filter SHALL apply only when the marker begins the thought block's text (after optional leading whitespace). A marker appearing mid-sentence inside a longer thought block SHALL NOT trigger suppression (such occurrences are out of scope for this requirement; they have not been observed in practice and conservative non-suppression preserves user-visible content).

The filter SHALL NOT mutate the underlying stream event payload (the raw marker text remains available in the per-run events.jsonl).

#### Scenario: Known marker is replaced by translated user-facing text

- **GIVEN** the active locale is zh-tw
- **WHEN** the renderer receives a thought block whose text is `[CODEBUS_QUIZ_NO_VALIDATE] codex sandbox cannot run quiz structure validation`
- **THEN** the rendered output SHALL contain `codex 沙箱無法跑 quiz 結構驗證，跳過此步` AND SHALL NOT contain the literal substring `[CODEBUS_QUIZ_NO_VALIDATE]`

#### Scenario: Unknown marker is suppressed entirely

- **WHEN** the renderer receives a thought block whose text is `[CODEBUS_FUTURE_MARKER] some payload codebus-app has never seen`
- **THEN** the rendered output for this thought block SHALL be empty AND SHALL NOT contain the literal substring `[CODEBUS_FUTURE_MARKER]`

#### Scenario: Thought block without a leading marker is unaffected

- **WHEN** the renderer receives a thought block whose text is `I will start by reading README.md to understand the project structure.`
- **THEN** the rendered output SHALL be the thought text verbatim AND the filter SHALL NOT alter it

#### Scenario: Mid-sentence marker is not suppressed

- **WHEN** the renderer receives a thought block whose text is `The agent emitted [CODEBUS_QUIZ_SCOPE] wiki/a.md as its first line.`
- **THEN** the rendered output SHALL contain the thought verbatim including the literal `[CODEBUS_QUIZ_SCOPE]` substring (the filter only triggers when the marker begins the block)

##### Example: marker-handling table

| Locale | Raw thought text | Rendered output |
|---|---|---|
| zh-tw | `[CODEBUS_QUIZ_NO_VALIDATE] codex sandbox cannot run quiz structure validation` | `codex 沙箱無法跑 quiz 結構驗證，跳過此步` |
| en | `[CODEBUS_QUIZ_NO_VALIDATE] codex sandbox cannot run quiz structure validation` | (en translation registered under the same i18n key) |
| zh-tw | `[CODEBUS_UNKNOWN_MARKER] payload` | (empty — suppressed) |
| zh-tw | `Reading README.md first.` | `Reading README.md first.` |
