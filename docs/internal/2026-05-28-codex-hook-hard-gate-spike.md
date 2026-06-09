# 2026-05-28 Codex hook hard-gate spike

## Goal

Determine whether codebus can improve its Codex provider from coarse sandbox + prompt-layer discipline to a hard gate comparable to Claude Code `PreToolUse(Read)`, especially for blocking image / PDF / binary reads that bypass text PII filtering.

## Questions

1. Where does Codex 0.134.0 load hooks from: user config, project config, repo-local `.codex/hooks.json`, plugin manifests, plugin-local `hooks.json`, or some combination?
2. Which flags in codebus's current Codex spawn recipe suppress hooks: `--ignore-user-config`, `--ignore-rules`, `--disable apps`, `project_root_markers`, `--ephemeral`?
3. Does Codex `PreToolUse` fire before file reads or only before shell / patch / MCP-like tools?
4. If hooks fire, can a hook block a tool call by returning a decision JSON or non-zero status?
5. Can codebus enable a codebus-owned hard gate without reopening user-global MCP/plugin/config injection?

## Baseline

- Date: 2026-05-28 Asia/Taipei
- Workspace: `D:\side_project\codebus`
- Codex CLI version: `codex-cli 0.134.0` (`codex doctor` reports npm install, Windows x86_64).
- Upstream source inspected from `openai/codex` cloned into `target/codex-hook-spike-src`.
- Local execution fixture: isolated `CODEX_HOME` directories under `target/codex-hook-spike/*`, copied `auth.json` only where needed, local hook scripts, local workspaces, and no writes to the real user Codex config.
- codebus `CodexBackend` recipe:
  - Always emits `codex exec`.
  - Isolation / hygiene flags: `--json`, `--ignore-user-config`, `--disable apps`, `--ignore-rules`, `--skip-git-repo-check`, `-c project_root_markers=['.codebus-vault']`, `-c windows.sandbox=unelevated`.
  - Single-shot verbs add `--ephemeral`; chat omits `--ephemeral` so resume works.
  - Fresh spawns use `-s read-only` or `-s workspace-write`; `resume` uses `-c sandbox_mode=<mode>` because `codex exec resume` rejects `-s`.
  - Model/effort: `-m <model>`, `-c model_reasoning_effort=<effort>`.
  - Azure mode adds `-c model_provider=azure` and the `model_providers.azure.*` overrides; key comes from `CODEBUS_CODEX_AZURE_KEY`.

## External research notes

- Codex has a real hook system now. `codex exec --help` exposes `--dangerously-bypass-hook-trust`, and upstream source defines lifecycle events: `PreToolUse`, `PermissionRequest`, `PostToolUse`, `PreCompact`, `PostCompact`, `SessionStart`, `UserPromptSubmit`, `SubagentStart`, `SubagentStop`, and `Stop`.
- Official Codex Hooks docs say `PreToolUse` can intercept `Bash`, file edits performed through `apply_patch`, and MCP tool calls. The same docs explicitly call it a guardrail rather than a complete enforcement boundary, and say it does not intercept `WebSearch` or other non-shell, non-MCP tool calls.
- Official Codex CLI docs define `--image/-i` as attaching image files to the initial prompt. That path is user input, not a tool invocation, so `PreToolUse` is not the right interception point.
- Official app-server docs describe turn input as supporting text and image/local-image content. Those image inputs are also request content, not tool calls.
- Hook config formats seen in upstream tests:
  - User/project TOML: `[hooks]`, `[[hooks.PreToolUse]]`, `[[hooks.PreToolUse.hooks]]`.
  - JSON: `hooks.json` with `{ "hooks": { "PreToolUse": [...] } }`.
  - Plugin hooks exist in source/test paths as plugin-local `hooks/hooks.json`, but they are still gated by plugin enablement and trust.
- Upstream `PreToolUse` output can block in two ways:
  - JSON: `{"decision":"block","reason":"..."}`.
  - JSON hook-specific deny: `{"hookSpecificOutput":{"hookEventName":"PreToolUse","permissionDecision":"deny","permissionDecisionReason":"..."}}`.
  - Exit code `2` with a reason on stderr also blocks.
- Hook trust is separate from hook discovery. Enabled unmanaged hooks only run when trusted, or when the invocation sets `--dangerously-bypass-hook-trust`.
- `--ignore-user-config` skips `${CODEX_HOME}/config.toml`, but project config layers are still searched from `cwd` to project root. Project-local hooks, however, require project trust.
- `--ignore-rules` maps to `ignore_user_and_project_exec_policy_rules`; it is not the same thing as disabling hooks.
- `hooks` is a stable feature and default-enabled in current upstream source.
- Relevant upstream/community signals:
  - openai/codex issue #20204 says hook dispatch is generic but only handlers that return `pre_tool_use_payload` emit events; many tools still have no payload coverage.
  - openai/codex issue #20616 reports built-in image generation did not emit `PreToolUse` in tested Windows/Desktop sessions.
  - openai/codex issue #18491 describes historical `PreToolUse` coverage as shell-first and asks for read/list/grep coverage.
  - Current upstream `main` source appears to have broader generic `pre_tool_use_payload` coverage than the installed `codex-cli 0.134.0` binary tested here. Treat source-main observations as future/version-dependent until verified against the installed release.

Source links:

- https://github.com/openai/codex/issues/20204
- https://github.com/openai/codex/issues/20616
- https://github.com/openai/codex/issues/18491
- https://github.com/openai/codex/issues/19385
- https://github.com/openai/codex/releases
- https://developers.openai.com/codex/hooks
- https://developers.openai.com/codex/cli/reference
- https://developers.openai.com/api/docs/guides/tools-image-generation

## Experiments

### E1: Capture current CLI help and codebus argv baseline

Status: done

Result:

- `codex exec --help` confirms `--dangerously-bypass-hook-trust`, `--ignore-user-config`, `--ignore-rules`, `--disable`, `-i/--image`, `-s`, `--ephemeral`, and `--json`.
- codebus currently does not pass `--dangerously-bypass-hook-trust`.
- codebus currently does not pass `-i/--image`.
- The existing codebus flags are coherent for isolation: they remove user global config/plugins/apps/rules and pin project-root detection to the vault marker.

### E2: User-level hooks.json fires under plain `codex exec`

Status: tested, did not fire in local CLI

Fixture:

- `CODEX_HOME=target/codex-hook-spike/run1/home`
- `home/hooks.json` configured `PreToolUse` matcher `.*` and `UserPromptSubmit`.
- `home/config.toml` also configured TOML `PreToolUse`.
- Hook command logged stdin/env/cwd to `target/codex-hook-spike/run1/hook-log.jsonl`.
- Invocation used `--dangerously-bypass-hook-trust` and later also `--enable hooks`.

Observed:

- `codex exec` showed the warning that hook trust bypass was enabled.
- The model ran `shell_command` successfully.
- No hook log was written.
- The same PowerShell hook script works when invoked manually.

Interpretation:

- In this local `codex exec 0.134.0` environment on Windows, configured hooks were not executed even though the flag and config are accepted.
- This is an empirical result for codebus's target execution path; upstream source/test support alone is not sufficient proof of deployability.

### E3: User-level hooks.json under codebus isolation flags

Status: tested, did not fire

Invocation added the codebus-like flags:

`--ignore-user-config --disable apps --ignore-rules --skip-git-repo-check --ephemeral -s workspace-write -c project_root_markers=['.codebus-vault'] -c windows.sandbox=unelevated`

Observed:

- Command ran.
- Hook did not log.

Interpretation:

- With current codebus flags, user-level hook config is intentionally unavailable because `--ignore-user-config` removes user config. Even adding `--dangerously-bypass-hook-trust` did not produce a runnable hook in the local CLI fixture.

### E4: Project-level hooks.json / config hook path behavior

Status: source-confirmed, local CLI execution not confirmed

Source says:

- Project config layers are loaded from `.codex/config.toml` between project root and cwd.
- Linked worktrees use root-checkout hook declarations.
- Project hooks only load from trusted project layers.
- Project-local hook declarations can be ignored when the project is untrusted.

Local execution:

- A trusted project config was attempted in the isolated `CODEX_HOME`.
- `codex exec` still did not run hooks in this fixture.

### E5: Plugin-local hook behavior

Status: source-confirmed only

Source/tests show plugin hooks via installed plugin cache path like `plugins/cache/<marketplace>/<plugin>/local/hooks/hooks.json`, surfaced as `HookSource::Plugin`.

For codebus, enabling plugin hook hard gates would conflict with the current `--disable apps` / plugin-surface reduction unless codebus owns and pins the plugin source. This path is not the first choice for a vault hard gate.

### E6: PreToolUse payload coverage for shell vs file read vs apply_patch vs image-related tools

Status: source-researched, local hook execution blocked by E2 result

Source says:

- Generic function tools can expose `PreToolUse` by implementing/returning `pre_tool_use_payload`.
- Shell/unified exec use hook-facing tool name `Bash`.
- `apply_patch` uses hook-facing tool name `apply_patch` plus matcher aliases `Write` and `Edit`.
- `view_image` is implemented as a normal function tool in source, but current upstream issue reports suggest coverage has been inconsistent across tool handlers/builds.
- CLI prompt image attachment `-i/--image` is not a tool call; it is initial user input. A `PreToolUse` hook cannot block an image already attached via CLI argument.

Implication:

- Even if hooks run, `PreToolUse` is not a general "before every file read" primitive unless every relevant read/image path emits payloads.

### E7: Can hook block execution?

Status: source-confirmed, local CLI not confirmed

Source tests confirm block semantics for `PreToolUse`:

- `permissionDecision: "deny"` blocks.
- Deprecated `decision: "block"` blocks.
- Exit code `2` with stderr reason blocks.

Local CLI block test:

- Set `CODEX_HOOK_SPIKE_BLOCK=1` so the hook would emit `{"decision":"block","reason":"blocked by CODEX_HOOK_SPIKE_BLOCK"}`.
- `codex exec` still ran `echo SHOULD_HAVE_BEEN_BLOCKED`.
- No hook log was written.

Interpretation:

- Blocking works in upstream engine tests, but was not reachable in the local `codex exec` path tested here.

### E8: App-server `PreToolUse` fire path

Status: confirmed locally with Codex 0.134.0

Fixture:

- Isolated `CODEX_HOME=target/codex-hook-spike/run6/home`.
- `codex app-server --listen stdio://`.
- Local mock Responses API provider, so the model deterministically returned a `shell_command` function call.
- User config contained:
  - `[features] hooks = true`
  - `[[hooks.PreToolUse]] matcher = "^Bash$"`
  - command hook script that logs stdin and returns `{"decision":"block","reason":"run6 pre-tool hook blocked"}`.

Required protocol details:

- `hooks/list` returns hooks under `result.data[0].hooks`, not `result.hooks`.
- Unmanaged user hooks are discovered as `enabled: true` but `trustStatus: "untrusted"` until trusted.
- Trust is written through `config/batchWrite` at `keyPath: "hooks.state"` with:

```json
{
  "<hook key>": {
    "trusted_hash": "<currentHash>"
  }
}
```

- `thread/start` legacy `sandbox` accepts `read-only`, `workspace-write`, or `danger-full-access`.
- `turn/start.sandboxPolicy.type` uses the newer camel-case union such as `workspaceWrite`; omitting `sandboxPolicy` and using the thread config also works.

Observed:

- After trust write, `hooks/list` changed `trustStatus` from `untrusted` to `trusted`.
- `turn/start` streamed `hook/started`, then `hook/completed` with `status: "blocked"`.
- The hook stdin payload included:

```json
{
  "hook_event_name": "PreToolUse",
  "tool_name": "Bash",
  "tool_input": {
    "command": "Write-Output SHOULD_NOT_RUN"
  },
  "tool_use_id": "call_1"
}
```

- The shell command was not executed.
- Codex sent the next model request with a `function_call_output`:

```text
Command blocked by PreToolUse hook: run6 pre-tool hook blocked. Command: Write-Output SHOULD_NOT_RUN
```

Interpretation:

- Codex hooks are real and can hard-block tool calls in the app-server execution path.
- The earlier `codex exec` failure is not a hook syntax issue; app-server discovered, trusted, executed, and blocked the same class of hook.
- `hooks/list` discovery alone is not enough. For unmanaged hooks to run, they must be trusted through `hooks.state` or launched with an explicit trust bypass path.

### E9: `codex exec` with mock provider and exact TOML hook

Status: tested, did not fire

Fixture:

- Isolated `CODEX_HOME=target/codex-hook-spike/run8-exec/home`.
- Same TOML hook shape as E8: `[features] hooks = true`, `[[hooks.PreToolUse]] matcher = "^Bash$"`, command handler that logs stdin and returns `decision:block`.
- Same local mock Responses API provider. The mock model deterministically returned a `shell_command` function call.
- Invocation:

```text
codex exec --dangerously-bypass-hook-trust --skip-git-repo-check --json run_shell_command_now
```

Observed:

- `codex exec` printed the expected warning that `--dangerously-bypass-hook-trust` was enabled.
- The mock model request included the `shell_command` tool.
- The model returned `shell_command` with `Write-Output SHOULD_NOT_RUN_EXEC`.
- Codex emitted a `command_execution` item and attempted to run the command.
- No hook log was written.
- No `hook/started` or hook-related event appeared in `--json` output.

Interpretation:

- This confirms the earlier real-model `codex exec` result without relying on model behavior.
- In Codex 0.134.0 on this Windows install, `codex exec` accepts hook config and the trust-bypass flag, but this tested path still does not dispatch `PreToolUse` before shell execution.
- For codebus, which currently spawns `codex exec`, app-server hook success is not directly deployable without changing the provider architecture.

### E10: App-server `view_image` tool coverage

Status: tested, did not fire

Fixture:

- Isolated `CODEX_HOME=target/codex-hook-spike/run9-view-image/home`.
- `codex app-server --listen stdio://`.
- Same trust flow as E8, but hook matcher was `".*"` to catch any hook-facing tool name.
- Mock model returned a `view_image` function call against a local `tiny.png`.

Observed:

- `hooks/list` showed the hook as `trusted`.
- The mock model request exposed `view_image` in the available tool list.
- The model returned a `view_image` function call.
- No hook log was written.
- No `hook/started` or `hook/completed` notification appeared.
- Codex attempted to process the image and returned an image decode error as the tool output.

Interpretation:

- In this Codex 0.134.0 app-server fixture, `PreToolUse` covers `Bash` but does not cover `view_image`.
- A Codex hook cannot currently be treated as a universal image-read gate. It may block image reads that happen through shell commands, but not this direct image tool path.

### E11: Current codebus flags, hosted web search, and shell HTTP

Status: tested

Fixture:

- Codex CLI 0.134.0 on Windows.
- Same default codebus-style isolation flags:
  - `--ignore-user-config`
  - `--disable apps`
  - `--ignore-rules`
  - `--skip-git-repo-check`
  - `-c project_root_markers=['.codebus-vault']`
  - `-c windows.sandbox=unelevated`
  - `--ephemeral`

Hosted web search test:

- Ran with `-s read-only`.
- Prompt explicitly said not to use shell commands and to use built-in web search if available.
- Codex emitted web search activity and reported a current OpenAI Newsroom headline.

Shell network tests:

- `-s read-only`: Codex attempted a PowerShell HTTP fetch, but the shell command was rejected before execution with `blocked by policy`.
- `-s workspace-write`: Codex executed `curl.exe -I https://example.com`; the request failed with `curl: (7) Failed to connect to example.com port 443 via 127.0.0.1 after 2048 ms`.

Disable candidate:

- Adding `-c web_search=disabled` to the same hosted-web-search prompt made Codex answer `Web search is unavailable.`

Interpretation:

- Current codebus flags do not disable hosted web search. `--disable apps` removes app/plugin tools, not Codex/provider-hosted web search.
- Current sandbox behavior prevented a successful shell HTTP fetch in this environment, but only `read-only` rejected the network command before execution. Under `workspace-write`, the command executed and then failed at connection time, so this should not be treated as a deliberate codebus hard network-off policy.
- `web_search=disabled` should not be described as "network fully off". It disables the hosted web-search capability. Shell/network egress is a separate concern and needs sandbox or OS/container/network-level enforcement if codebus must hard-block all outbound traffic.
- For codebus default behavior, add `-c web_search=disabled` unless a future verb explicitly needs web research, and keep a regression test for it.

### E12: Hosted image generation when web search is disabled

Status: tested

Fixture:

- Codex CLI 0.134.0 on Windows.
- Same default codebus-style isolation flags as E11.
- Added `-c web_search=disabled`.
- Used `-s workspace-write`.
- Prompt explicitly said not to use shell commands.

Observed:

- With only `-c web_search=disabled`, Codex reported that hosted image generation succeeded and returned an image directly in chat.
- Re-running the same test with both `-c web_search=disabled` and `--disable image_generation` made Codex report that no hosted image generation tool was exposed and image generation did not succeed.
- A follow-up `--json` run with `-c web_search=disabled` again reported successful hosted image generation. The JSONL stream did not expose a detailed image-generation event in this run; it only included the final agent message, so this evidence is behavioral rather than a low-level tool-event capture.

Interpretation:

- `web_search=disabled` disables hosted web search, not hosted image generation.
- If codebus later wants both "no web research" and "no image generation", it should pass both `-c web_search=disabled` and `--disable image_generation`. For now, these should remain separate policy decisions.
- Add a regression test that runs the disabled-image-generation prompt and asserts that Codex reports no exposed image generation tool. If a future CLI exposes image-generation events in JSONL, upgrade the test to assert the tool is absent at the event layer instead of relying only on final text.

## Findings

1. The old statement "Codex has no hooks" is now wrong. Codex has lifecycle hooks, including `PreToolUse`, and the block contract works locally through app-server.
2. The useful codebus statement is narrower: current codebus integration uses `codex exec`, and this spike found that `PreToolUse` does not fire on that entrypoint in the tested Codex 0.134.0 Windows environment, even with a mock model and trust bypass.
3. Claude still offers finer practical control for codebus's current need: codebus can install a vault-local Claude `PreToolUse` hook for `Read` and hard-block image/PDF/binary reads by file path before tool execution.
4. Codex's control surface is coarser in the current codebus path because:
   - codebus intentionally disables user config, apps/plugins, and rules;
   - unmanaged hooks require trust state unless bypassed;
   - hook execution is confirmed through app-server, but failed under the current `codex exec` path;
   - CLI `-i/--image` is outside `PreToolUse`;
   - hook payload coverage is not universal across read/image paths; `view_image` did not fire `PreToolUse` in the app-server fixture.
   - official Codex docs currently describe `PreToolUse` support as `Bash`, `apply_patch`, and MCP tool calls, not direct image input, `view_image`, web search, or hosted image generation.
5. The current codebus flags are reasonable for isolation. They are conservative, not accidental. They do not, however, disable hosted web search by default.
6. `web_search=disabled` and `--disable image_generation` are independent controls; disabling web search alone still allowed hosted image generation in E12.
7. "No web research" and "no network egress" are different policies. `-c web_search=disabled` addresses the former; full outbound network denial would need a separate enforcement layer beyond the Codex hosted-tool flags tested here.
8. There is optimization room, but it should be opt-in and codebus-owned, not by re-enabling arbitrary user config/plugins.

## Recommendation for codebus

Short term:

- Keep the current Codex prompt-level soft constraint for "do not read image/PDF/binary unless explicitly allowed".
- Keep `--ignore-user-config`, `--disable apps`, `--ignore-rules`, and vault-root pinning as the default safe mode.
- Add `-c web_search=disabled` to the default Codex argv so codebus Codex runs do not perform hosted web research unless a future mode explicitly opts in.
- Keep hosted image generation enabled for now. It is a separate creative-output capability, and E12 shows it can remain available while hosted web search is disabled.
- Add explicit docs explaining that Codex image/read blocking is soft today, while Claude has a hard read hook.
- Do not claim `web_search=disabled` fully disables network egress. It disables hosted web search; shell-level outbound traffic remains a separate policy surface.

Medium term:

- Add a `codebus codex-spike-hooks` or hidden dev check that can re-run both isolated fixtures against the installed Codex version:
  - `codex app-server` mock-provider fixture, expected to fire and block `Bash`;
  - `codex exec` mock-provider fixture, currently expected to demonstrate the gap unless a future Codex version changes behavior.
- Consider a future app-server-backed Codex provider only if codebus wants hook-grade control. App-server has a real hook review/trust API and emits `hook/started` / `hook/completed` events, but it is a larger integration change than spawning `codex exec`.
- Investigate a codebus-owned project hook path only if all of these are true:
  - project trust can be supplied without loading arbitrary user global config;
  - hooks execute in the actual codebus Codex entrypoint on Windows and the target CI/dev environments;
  - `PreToolUse` fires for `Bash`, `apply_patch`, `view_image`, and any file-read path codebus cares about;
  - `--ignore-user-config` can remain on, or an equivalent allowlisted config loader can be used.
- If a Codex hard gate becomes viable, prefer a codebus-generated hook command that calls `codebus hook check-read` or a new `codebus hook check-codex-tool`, using a deny-by-default policy for image/PDF/binary paths.

Do not do yet:

- Do not simply remove `--ignore-user-config` to get hooks. That would re-open user-global MCP/config/plugin injection, which is worse than the current soft constraint.
- Do not rely on plugin hooks for the vault hard gate until plugin provenance, enablement, and trust are fully codebus-controlled.
