//! `claude_code.*` config loader for v3-config Claude Code Configuration Schema.
//!
//! Schema:
//! ```yaml
//! # ~/.codebus/config.yaml
//! claude_code:
//!   goal:
//!     model: opus
//!     effort: high
//!   query:
//!     model: haiku
//!     effort: low
//!   fix:
//!     model: sonnet
//!     effort: medium
//! ```
//!
//! All fields are optional. Per-verb defaults are baked in (goal=opus/high,
//! query=haiku/low, fix=sonnet/medium) reflecting the verbs' compute profile
//! differences. `model` and `effort` are pass-through `Option<String>` —
//! codebus does not validate the values; they flow through to the Claude
//! CLI's `--model` / `--effort` flags which validate themselves. This keeps
//! codebus model-version-agnostic.
//!
//! Missing file / missing section / missing field / missing verb subsection
//! all fall through to the corresponding default.

use serde::Deserialize;
use std::fs;
use std::path::Path;

/// Per-verb agent configuration. `model` / `effort` are optional pass-through
/// strings forwarded as `--model <X>` / `--effort <Y>` to the Claude CLI when
/// `Some`; omitted from argv when `None`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerbAgentConfig {
    pub model: Option<String>,
    pub effort: Option<String>,
}

impl VerbAgentConfig {
    fn new(model: &str, effort: &str) -> Self {
        Self {
            model: Some(model.to_string()),
            effort: Some(effort.to_string()),
        }
    }
}

/// Three-verb agent configuration. Each verb has its own default reflecting
/// its compute profile.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClaudeCodeConfig {
    pub goal: VerbAgentConfig,
    pub query: VerbAgentConfig,
    pub fix: VerbAgentConfig,
}

impl Default for ClaudeCodeConfig {
    fn default() -> Self {
        Self {
            goal: VerbAgentConfig::new("opus", "high"),
            query: VerbAgentConfig::new("haiku", "low"),
            fix: VerbAgentConfig::new("sonnet", "medium"),
        }
    }
}

#[derive(Debug, Default, Deserialize)]
struct ConfigFile {
    #[serde(default)]
    claude_code: Option<ClaudeCodeSection>,
}

#[derive(Debug, Default, Deserialize)]
struct ClaudeCodeSection {
    goal: Option<VerbSection>,
    query: Option<VerbSection>,
    fix: Option<VerbSection>,
}

#[derive(Debug, Default, Deserialize)]
struct VerbSection {
    model: Option<String>,
    effort: Option<String>,
}

impl VerbSection {
    fn merge_into(self, target: &mut VerbAgentConfig) {
        if let Some(m) = self.model {
            target.model = Some(m);
        }
        if let Some(e) = self.effort {
            target.effort = Some(e);
        }
    }
}

/// Load `claude_code.*` config from `path`. Returns defaults when the file
/// does not exist OR the `claude_code` section is absent. Returns `Err` only
/// when the file exists but cannot be read or is structurally invalid YAML.
/// Callers SHALL fall back to defaults on `Err` after a stderr warning.
pub fn load_claude_code_config(
    path: &Path,
) -> Result<ClaudeCodeConfig, super::ConfigLoadError> {
    let body = match fs::read_to_string(path) {
        Ok(s) => s,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            return Ok(ClaudeCodeConfig::default());
        }
        Err(err) => return Err(super::ConfigLoadError::Io(err)),
    };
    let file: ConfigFile = serde_yaml::from_str(&body).map_err(super::ConfigLoadError::YamlParse)?;
    let mut cfg = ClaudeCodeConfig::default();
    if let Some(cc) = file.claude_code {
        if let Some(g) = cc.goal {
            g.merge_into(&mut cfg.goal);
        }
        if let Some(q) = cc.query {
            q.merge_into(&mut cfg.query);
        }
        if let Some(f) = cc.fix {
            f.merge_into(&mut cfg.fix);
        }
    }
    Ok(cfg)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use tempfile::TempDir;

    fn write_yaml(dir: &Path, body: &str) -> PathBuf {
        let p = dir.join("config.yaml");
        fs::write(&p, body).unwrap();
        p
    }

    /// Spec: "Default config when file missing"
    #[test]
    fn default_when_file_missing() {
        let tmp = TempDir::new().unwrap();
        let cfg = load_claude_code_config(&tmp.path().join("nope.yaml")).unwrap();
        assert_eq!(cfg, ClaudeCodeConfig::default());
        assert_eq!(cfg.goal.model.as_deref(), Some("opus"));
        assert_eq!(cfg.goal.effort.as_deref(), Some("high"));
        assert_eq!(cfg.query.model.as_deref(), Some("haiku"));
        assert_eq!(cfg.query.effort.as_deref(), Some("low"));
        assert_eq!(cfg.fix.model.as_deref(), Some("sonnet"));
        assert_eq!(cfg.fix.effort.as_deref(), Some("medium"));
    }

    /// Default when claude_code section absent.
    #[test]
    fn default_when_section_absent() {
        let tmp = TempDir::new().unwrap();
        let p = write_yaml(tmp.path(), "lint:\n  fix:\n    enabled: true\n");
        let cfg = load_claude_code_config(&p).unwrap();
        assert_eq!(cfg, ClaudeCodeConfig::default());
    }

    /// Spec: "Per-verb override applies only to that verb"
    #[test]
    fn per_verb_override_applies_only_to_that_verb() {
        let tmp = TempDir::new().unwrap();
        let p = write_yaml(
            tmp.path(),
            "claude_code:\n  goal:\n    model: sonnet\n",
        );
        let cfg = load_claude_code_config(&p).unwrap();
        assert_eq!(cfg.goal.model.as_deref(), Some("sonnet"));
        // goal.effort untouched → default "high"
        assert_eq!(cfg.goal.effort.as_deref(), Some("high"));
        // other verbs unchanged
        assert_eq!(cfg.query, VerbAgentConfig::new("haiku", "low"));
        assert_eq!(cfg.fix, VerbAgentConfig::new("sonnet", "medium"));
    }

    /// Spec: "Arbitrary model string is accepted" — no enum validation;
    /// arbitrary strings (full names with version digits) flow through
    /// verbatim so codebus does not need updates when models bump.
    #[test]
    fn arbitrary_model_string_is_accepted() {
        let tmp = TempDir::new().unwrap();
        let p = write_yaml(
            tmp.path(),
            "claude_code:\n  goal:\n    model: claude-opus-4-7\n",
        );
        let cfg = load_claude_code_config(&p).unwrap();
        assert_eq!(cfg.goal.model.as_deref(), Some("claude-opus-4-7"));
    }

    /// Spec: "Unknown subkey silently ignored"
    #[test]
    fn unknown_subkey_silently_ignored() {
        let tmp = TempDir::new().unwrap();
        let p = write_yaml(
            tmp.path(),
            "claude_code:\n  goal:\n    model: opus\n    future_field: hello\n",
        );
        let cfg = load_claude_code_config(&p).unwrap();
        assert_eq!(cfg.goal.model.as_deref(), Some("opus"));
        // future_field has no observable effect
    }

    /// All three verbs accept full overrides simultaneously.
    #[test]
    fn full_override_replaces_all_three_verbs() {
        let tmp = TempDir::new().unwrap();
        let p = write_yaml(
            tmp.path(),
            "claude_code:\n  \
             goal:\n    model: m1\n    effort: e1\n  \
             query:\n    model: m2\n    effort: e2\n  \
             fix:\n    model: m3\n    effort: e3\n",
        );
        let cfg = load_claude_code_config(&p).unwrap();
        assert_eq!(cfg.goal, VerbAgentConfig::new("m1", "e1"));
        assert_eq!(cfg.query, VerbAgentConfig::new("m2", "e2"));
        assert_eq!(cfg.fix, VerbAgentConfig::new("m3", "e3"));
    }

    /// Top-level unknown key is silently ignored (forward-compat).
    #[test]
    fn unknown_top_level_key_silently_ignored() {
        let tmp = TempDir::new().unwrap();
        let p = write_yaml(tmp.path(), "future_section: hi\nclaude_code:\n  goal:\n    model: opus\n");
        let cfg = load_claude_code_config(&p).unwrap();
        assert_eq!(cfg.goal.model.as_deref(), Some("opus"));
    }
}
