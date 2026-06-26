//! Scoped environment for the agent sub-process.
//!
//! The spawn path does NOT let the agent child inherit the parent shell
//! environment verbatim. Both backends call [`Command::env_clear`] right
//! after constructing the command, then re-populate ONLY the cross-platform
//! system-essential allowlist via [`passthrough_env`] (so secrets like
//! `GITHUB_TOKEN` / `AWS_*` / `KUBECONFIG` and codebus's own `CODEBUS_*`
//! keys never reach the agent), and finally layer the profile-specific
//! provider injection (`EnvOverrides`) on top. Injection order is therefore
//! `env_clear` → allowlist passthrough → provider overrides. The parent
//! process environment is never modified — codebus never calls
//! `std::env::set_var` in the spawn path, and `env_clear` acts only on the
//! child `Command`. See spec `claude-code-config / Scoped Environment
//! Injection At Spawn` and `codex-backend / Spawn Environment Scrub`.
//!
//! `EnvOverrides` carries the deterministic `(name, value)` provider map
//! layered last. Two builders cover the active-profile cases:
//!
//! - [`EnvOverrides::for_system`] — empty map. The system profile adds no
//!   provider env (the child still receives the allowlist passthrough).
//! - [`EnvOverrides::for_azure`] — exactly three keys: `ANTHROPIC_BASE_URL`
//!   pointing at the Azure-compatible endpoint, `ANTHROPIC_API_KEY` from
//!   the keyring / env fallback chain, and `CLAUDE_CODE_DISABLE_ADVISOR_TOOL`
//!   set to the literal string `"1"` (v2 strategy memo §8 verified this
//!   undocumented env is required — Azure rejects the `anthropic-beta:
//!   advisor-tool-2026-03-01` header with HTTP 400).

use std::collections::BTreeMap;
use std::ffi::OsString;

/// `ANTHROPIC_BASE_URL` — points Claude CLI at the configured endpoint.
pub const ENV_ANTHROPIC_BASE_URL: &str = "ANTHROPIC_BASE_URL";

/// `ANTHROPIC_API_KEY` — auth key. Forwarded from the keyring fallback
/// chain (see `config::keyring::read_azure_key`).
pub const ENV_ANTHROPIC_API_KEY: &str = "ANTHROPIC_API_KEY";

/// `CLAUDE_CODE_DISABLE_ADVISOR_TOOL` — undocumented Claude Code env that
/// suppresses the `anthropic-beta: advisor-tool-2026-03-01` header. Azure
/// Anthropic-compatible endpoints reject that header with HTTP 400, so
/// codebus forces this flag on whenever it injects Azure routing.
pub const ENV_DISABLE_ADVISOR_TOOL: &str = "CLAUDE_CODE_DISABLE_ADVISOR_TOOL";

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct EnvOverrides {
    entries: BTreeMap<String, String>,
}

impl EnvOverrides {
    /// Empty map. The system profile adds no provider env vars; the spawned
    /// child still receives the [`passthrough_env`] allowlist (the spawn path
    /// `env_clear`s first, so it does NOT inherit the parent shell verbatim).
    pub fn for_system() -> Self {
        Self::default()
    }

    /// Azure profile injection: `ANTHROPIC_BASE_URL`, `ANTHROPIC_API_KEY`,
    /// `CLAUDE_CODE_DISABLE_ADVISOR_TOOL=1`. The advisor-tool override is
    /// non-negotiable; v2 strategy memo §8 proved Azure 400s without it.
    pub fn for_azure(base_url: &str, api_key: &str) -> Self {
        let mut entries = BTreeMap::new();
        entries.insert(ENV_ANTHROPIC_BASE_URL.to_string(), base_url.to_string());
        entries.insert(ENV_ANTHROPIC_API_KEY.to_string(), api_key.to_string());
        entries.insert(ENV_DISABLE_ADVISOR_TOOL.to_string(), "1".to_string());
        Self { entries }
    }

    pub fn iter(&self) -> impl Iterator<Item = (&String, &String)> {
        self.entries.iter()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Look up a key. Used by tests + by `claude_cli::invoke` debug
    /// paths.
    pub fn get(&self, key: &str) -> Option<&str> {
        self.entries.get(key).map(String::as_str)
    }
}

// ---------------------------------------------------------------------------
// Spawn environment scrub: cross-platform passthrough allowlist
// ---------------------------------------------------------------------------
//
// Both backends `env_clear()` the child command and then re-inject ONLY the
// names below (read from the parent). Everything else — parent-shell secrets
// (`GITHUB_TOKEN` / `AWS_*` / `KUBECONFIG` / any `*_TOKEN` / `*_KEY` /
// `*_SECRET`) and codebus's own `CODEBUS_*` control/key vars — is dropped.
// The single shared list keeps the claude and codex spawn paths from
// drifting. Full per-item necessity justification lives in the change design
// doc (`agent-spawn-env-scrub`); each member is required for the agent CLI
// child (and the tools it shells out to) to spawn and run.

/// Universal system-essential names (all platforms).
const ALLOWLIST_UNIVERSAL: &[&str] = &["PATH", "HOME", "LANG", "LANGUAGE", "TZ"];

/// Windows system-essential names. The codex chain on Windows is
/// `codex.cmd` → `node.exe` → `codex.exe`, which relies on these for
/// executable resolution (`PATH` / `PATHEXT` / `ComSpec`), system dirs
/// (`SystemRoot` / `windir` / `SystemDrive`), home/profile resolution, and
/// scratch (`TEMP` / `TMP`). Dropping any of them can break the spawn.
#[cfg(windows)]
const ALLOWLIST_OS: &[&str] = &[
    "SystemRoot",
    "SystemDrive",
    "windir",
    "USERPROFILE",
    "HOMEDRIVE",
    "HOMEPATH",
    "APPDATA",
    "LOCALAPPDATA",
    "PROGRAMDATA",
    "ProgramFiles",
    "ProgramFiles(x86)",
    "PATHEXT",
    "ComSpec",
    "TEMP",
    "TMP",
    "NUMBER_OF_PROCESSORS",
    "OS",
    "COMPUTERNAME",
];

/// Unix system-essential names (identity, shell, temp dir).
#[cfg(unix)]
const ALLOWLIST_OS: &[&str] = &["USER", "LOGNAME", "SHELL", "TMPDIR"];

/// Fallback for exotic targets that are neither Windows nor Unix: only the
/// universal set passes through.
#[cfg(not(any(windows, unix)))]
const ALLOWLIST_OS: &[&str] = &[];

/// Name PREFIXES passed through as families.
///
/// - `LC_` — POSIX locale categories (`LC_ALL` / `LC_CTYPE` / ...). Always
///   locale, never secrets.
/// - `CODEBUS_MOCK_` — integration-test control vars (behavior selector, log
///   sink, session id) consumed by `codebus-cli/tests/bins/mock_claude.rs`.
///   That mock is spawned THROUGH this very scrub path, so it must receive
///   its control out-of-band; this prefix is that seam. It provably never
///   carries a secret and is never set in a production deployment. codebus's
///   real secret/control vars (`CODEBUS_AZURE_KEY`, `CODEBUS_CODEX_AZURE_KEY`,
///   `CODEBUS_HOME`, ...) do NOT match this prefix and remain scrubbed.
const ALLOWLIST_PREFIXES: &[&str] = &["LC_", "CODEBUS_MOCK_"];

/// Case rule for exact name comparison: Windows env names are
/// case-insensitive; Unix names are case-sensitive.
#[cfg(windows)]
fn name_eq(a: &str, b: &str) -> bool {
    a.eq_ignore_ascii_case(b)
}
#[cfg(not(windows))]
fn name_eq(a: &str, b: &str) -> bool {
    a == b
}

/// Prefix match honoring the same case rule as [`name_eq`]. Byte-wise on
/// Windows to stay panic-free on non-char-boundary inputs (env names are
/// ASCII in practice, but the allowlist must never panic the spawn path).
#[cfg(windows)]
fn name_has_prefix(name: &str, prefix: &str) -> bool {
    name.len() >= prefix.len()
        && name.as_bytes()[..prefix.len()].eq_ignore_ascii_case(prefix.as_bytes())
}
#[cfg(not(windows))]
fn name_has_prefix(name: &str, prefix: &str) -> bool {
    name.starts_with(prefix)
}

/// Whether `name` is a passthrough allowlist member (exact name or prefix
/// family).
fn is_passthrough_name(name: &str) -> bool {
    ALLOWLIST_UNIVERSAL
        .iter()
        .chain(ALLOWLIST_OS.iter())
        .any(|allowed| name_eq(name, allowed))
        || ALLOWLIST_PREFIXES
            .iter()
            .any(|prefix| name_has_prefix(name, prefix))
}

/// Pure allowlist filter over an arbitrary env iterator. Returns the subset
/// of `(name, value)` pairs whose name is a passthrough allowlist member,
/// preserving the parent's original name casing and value bytes. Split out
/// from [`passthrough_env`] so it can be unit-tested with synthetic input
/// (deterministic, no process-wide env mutation / races).
///
/// Names are matched on their lossy UTF-8 view. The allowlist members are
/// all ASCII, so a name that is not valid UTF-8 cannot match and is dropped
/// (fail-closed) — and using `OsString` throughout means an unrelated
/// non-UTF-8 env var can never panic the spawn path the way `env::vars()`
/// would.
fn filter_passthrough<I>(vars: I) -> Vec<(OsString, OsString)>
where
    I: IntoIterator<Item = (OsString, OsString)>,
{
    vars.into_iter()
        .filter(|(name, _)| is_passthrough_name(name.to_string_lossy().as_ref()))
        .collect()
}

/// Read the parent process environment and return only the passthrough
/// allowlist members. Both spawn backends call this immediately after
/// `Command::env_clear()` to re-populate the child with system-essential
/// vars while dropping inherited secrets. Uses `env::vars_os` (not
/// `env::vars`) so a non-UTF-8 env var cannot panic the spawn.
pub(crate) fn passthrough_env() -> Vec<(OsString, OsString)> {
    filter_passthrough(std::env::vars_os())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsStr;

    fn ospairs(pairs: &[(&str, &str)]) -> Vec<(OsString, OsString)> {
        pairs
            .iter()
            .map(|(k, v)| (OsString::from(*k), OsString::from(*v)))
            .collect()
    }

    fn as_map(v: Vec<(OsString, OsString)>) -> BTreeMap<OsString, OsString> {
        v.into_iter().collect()
    }

    fn got<'a>(m: &'a BTreeMap<OsString, OsString>, k: &str) -> Option<&'a str> {
        m.get(OsStr::new(k)).and_then(|v| v.to_str())
    }

    /// Scrub: system-essential names + the `LC_` locale family pass through;
    /// parent-shell secrets and arbitrary vars are dropped. The Windows-only
    /// `ProgramFiles(x86)` member is asserted platform-aware.
    #[test]
    fn filter_passthrough_keeps_essentials_drops_secrets() {
        let m = as_map(filter_passthrough(ospairs(&[
            ("PATH", "/usr/bin"),
            ("GITHUB_TOKEN", "ghp_secret"),
            ("AWS_SECRET_ACCESS_KEY", "aws_secret"),
            ("KUBECONFIG", "/home/u/.kube/config"),
            ("CODEBUS_AZURE_KEY", "sk-secret"),
            ("LC_CTYPE", "UTF-8"),
            ("ProgramFiles(x86)", "C:\\Program Files (x86)"),
            ("SOME_RANDOM_VAR", "x"),
        ])));

        // System-essential + locale family pass through.
        assert_eq!(got(&m, "PATH"), Some("/usr/bin"));
        assert_eq!(got(&m, "LC_CTYPE"), Some("UTF-8"));
        // Secrets and arbitrary vars are scrubbed.
        assert!(!m.contains_key(OsStr::new("GITHUB_TOKEN")));
        assert!(!m.contains_key(OsStr::new("AWS_SECRET_ACCESS_KEY")));
        assert!(!m.contains_key(OsStr::new("KUBECONFIG")));
        assert!(!m.contains_key(OsStr::new("CODEBUS_AZURE_KEY")));
        assert!(!m.contains_key(OsStr::new("SOME_RANDOM_VAR")));
        // Windows-only allowlist member: present on Windows, dropped elsewhere.
        #[cfg(windows)]
        assert_eq!(got(&m, "ProgramFiles(x86)"), Some("C:\\Program Files (x86)"));
        #[cfg(not(windows))]
        assert!(!m.contains_key(OsStr::new("ProgramFiles(x86)")));
    }

    /// Test-harness seam: the `CODEBUS_MOCK_` prefix family passes through (the
    /// mock is spawned through this scrub path and needs its control vars),
    /// while codebus's real secret / control vars stay scrubbed.
    #[test]
    fn filter_passthrough_allows_mock_prefix_but_scrubs_codebus_secrets() {
        let m = as_map(filter_passthrough(ospairs(&[
            ("CODEBUS_MOCK_LOG", "/tmp/mock.log"),
            ("CODEBUS_MOCK_BEHAVIOR", "success-noop"),
            ("CODEBUS_AZURE_KEY", "sk-secret"),
            ("CODEBUS_CODEX_AZURE_KEY", "sk-codex-secret"),
            ("CODEBUS_HOME", "/home/u/.codebus"),
        ])));

        assert_eq!(got(&m, "CODEBUS_MOCK_LOG"), Some("/tmp/mock.log"));
        assert!(m.contains_key(OsStr::new("CODEBUS_MOCK_BEHAVIOR")));
        // Prefix mismatch → still scrubbed.
        assert!(!m.contains_key(OsStr::new("CODEBUS_AZURE_KEY")));
        assert!(!m.contains_key(OsStr::new("CODEBUS_CODEX_AZURE_KEY")));
        assert!(!m.contains_key(OsStr::new("CODEBUS_HOME")));
    }

    /// Windows env names are case-insensitive: `Path` / `Tmp` match the
    /// `PATH` / `TMP` allowlist members and pass through under the parent's
    /// original casing.
    #[cfg(windows)]
    #[test]
    fn filter_passthrough_windows_name_match_is_case_insensitive() {
        let m = as_map(filter_passthrough(ospairs(&[
            ("Path", "C:\\Windows\\System32"),
            ("Tmp", "C:\\Temp"),
        ])));
        assert_eq!(got(&m, "Path"), Some("C:\\Windows\\System32"));
        assert_eq!(got(&m, "Tmp"), Some("C:\\Temp"));
    }

    /// Spec: System profile injects no env.
    #[test]
    fn for_system_returns_empty_map() {
        let env = EnvOverrides::for_system();
        assert!(env.is_empty());
        assert_eq!(env.len(), 0);
        assert!(env.iter().next().is_none());
    }

    /// Spec: Azure profile injects exactly three env vars.
    #[test]
    fn for_azure_has_exactly_three_keys() {
        let env = EnvOverrides::for_azure(
            "https://example.cognitiveservices.azure.com/anthropic",
            "sk-test",
        );
        assert_eq!(env.len(), 3);
        let keys: Vec<&String> = env.iter().map(|(k, _)| k).collect();
        assert!(keys.iter().any(|k| *k == ENV_ANTHROPIC_BASE_URL));
        assert!(keys.iter().any(|k| *k == ENV_ANTHROPIC_API_KEY));
        assert!(keys.iter().any(|k| *k == ENV_DISABLE_ADVISOR_TOOL));
    }

    /// Spec: `CLAUDE_CODE_DISABLE_ADVISOR_TOOL` value SHALL be the literal
    /// string `"1"` (not "true", not "on"). Claude Code only honours `1`.
    #[test]
    fn for_azure_disable_advisor_tool_value_is_literal_one() {
        let env = EnvOverrides::for_azure("https://x.example.com/anthropic", "sk");
        assert_eq!(env.get(ENV_DISABLE_ADVISOR_TOOL), Some("1"));
    }

    /// Values flow verbatim — codebus does not normalise URL or key.
    #[test]
    fn for_azure_values_flow_verbatim() {
        let env = EnvOverrides::for_azure(
            "https://example.cognitiveservices.azure.com/anthropic",
            "sk-secret-12345",
        );
        assert_eq!(
            env.get(ENV_ANTHROPIC_BASE_URL),
            Some("https://example.cognitiveservices.azure.com/anthropic"),
        );
        assert_eq!(env.get(ENV_ANTHROPIC_API_KEY), Some("sk-secret-12345"));
    }

    /// Iteration order is deterministic (BTreeMap by name) — relied on
    /// by spawn-path snapshot tests that capture the env list as a Vec.
    #[test]
    fn for_azure_iteration_order_is_sorted_by_key() {
        let env = EnvOverrides::for_azure("u", "k");
        let keys: Vec<&str> = env.iter().map(|(k, _)| k.as_str()).collect();
        assert_eq!(
            keys,
            vec![
                ENV_ANTHROPIC_API_KEY,
                ENV_ANTHROPIC_BASE_URL,
                ENV_DISABLE_ADVISOR_TOOL,
            ]
        );
    }
}
