//! `~/.codebus/config.yaml` loader.
//!
//! Tolerance contract (see `terminal-output` spec, "Load global config
//! tolerantly" requirement):
//!
//! 1. Missing file → return [`GlobalConfig::default()`], no warning.
//! 2. Parse failure → stderr warning, return default.
//! 3. Unknown top-level key → silently ignored (forward-compat for future
//!    schema growth).
//! 4. Unknown discriminator value (e.g. `provider: gibberish`) → warning +
//!    treat that section as unset (factory falls through to default).
//! 5. Unknown sub-field within a known section → silently ignored.
//! 6. Type-mismatched sub-field (e.g. `timeout_secs: "thirty"`) → warning,
//!    that sub-field is treated as unset, the rest of the section is honored.
//!
//! Each plugin section's `parse_*` function walks `serde_yaml::Value`
//! manually so field-level tolerance (rule 6) is preserved, and constructs
//! the factory-domain tagged enum directly as the output type. Warnings
//! are written to stderr via `eprintln!`. Tests can assert on the parsed
//! [`GlobalConfig`] without needing to capture stderr.

use crate::config::schema::{AutoFixConfig, EmojiMode, GlobalConfig, LintConfig};
use crate::llm::ProviderConfig;
use crate::log::SinkConfig;
use crate::pii::{OnHit, ScannerConfig};
use crate::render::{RenderOptions, RendererConfig};
use serde_yaml::Value;
use std::fs;
use std::path::{Path, PathBuf};

/// Read `~/.codebus/config.yaml` (resolved via the `dirs` crate). Always
/// returns a [`GlobalConfig`]; failures are warned and folded into a
/// default.
pub fn load_config() -> GlobalConfig {
    match config_path() {
        Some(p) => load_config_from_path(&p),
        None => GlobalConfig::default(),
    }
}

/// Resolve the canonical config file path: `<home>/.codebus/config.yaml`.
/// Returns `None` if the home directory cannot be determined.
///
/// `CODEBUS_HOME` env var takes precedence over the resolved home dir —
/// useful for relocating the config in CI / containers, and as a clean
/// test hook on Windows where `dirs::home_dir()` ignores `HOME` /
/// `USERPROFILE` env overrides.
pub fn config_path() -> Option<PathBuf> {
    if let Ok(custom) = std::env::var("CODEBUS_HOME") {
        if !custom.is_empty() {
            return Some(PathBuf::from(custom).join(".codebus").join("config.yaml"));
        }
    }
    dirs::home_dir().map(|h| h.join(".codebus").join("config.yaml"))
}

/// Test hook — load from an explicit path. Public so integration tests
/// (and a future `--config` flag) can target a non-default location.
pub fn load_config_from_path(path: &Path) -> GlobalConfig {
    let raw = match fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return GlobalConfig::default();
        }
        Err(e) => {
            eprintln!(
                "warning: codebus config at {} could not be read ({e}); using defaults",
                path.display()
            );
            return GlobalConfig::default();
        }
    };

    let value: Value = match serde_yaml::from_str(&raw) {
        Ok(v) => v,
        Err(e) => {
            eprintln!(
                "warning: codebus config at {} is not valid YAML ({e}); using defaults",
                path.display()
            );
            return GlobalConfig::default();
        }
    };

    let Value::Mapping(map) = value else {
        if !matches!(value, Value::Null) {
            eprintln!(
                "warning: codebus config at {} is not a mapping; using defaults",
                path.display()
            );
        }
        return GlobalConfig::default();
    };

    let mut cfg = GlobalConfig::default();
    for (k, v) in map {
        let Some(key) = k.as_str() else { continue };
        match key {
            "emoji" => cfg.emoji = parse_emoji(&v),
            "llm" => cfg.llm = parse_llm(&v),
            "pii" => cfg.pii = parse_pii(&v),
            "lint" => cfg.lint = parse_lint(&v),
            "render" => cfg.render = parse_render(&v),
            "log" => cfg.log = parse_log(&v),
            // Forward-compat: unknown top-level fields are silently ignored.
            _ => {}
        }
    }
    cfg
}

fn parse_emoji(v: &Value) -> Option<EmojiMode> {
    let Some(s) = v.as_str() else {
        warn_type_mismatch("emoji", "auto | on | off", v);
        return None;
    };
    match s {
        "auto" => Some(EmojiMode::Auto),
        "on" => Some(EmojiMode::On),
        "off" => Some(EmojiMode::Off),
        other => {
            eprintln!(
                "warning: codebus config `emoji: {other}` is not one of auto|on|off; ignoring"
            );
            None
        }
    }
}

fn parse_llm(v: &Value) -> Option<ProviderConfig> {
    let Value::Mapping(map) = v else {
        if !matches!(v, Value::Null) {
            warn_type_mismatch("llm", "mapping", v);
        }
        return Some(ProviderConfig::default());
    };

    let mut provider_str: Option<String> = None;
    let mut binary_path: Option<String> = None;
    let mut timeout_secs: Option<u64> = None;
    let mut api_key: Option<String> = None;
    let mut provider_was_explicitly_invalid = false;

    for (k, val) in map {
        let Some(key) = k.as_str() else { continue };
        match key {
            "provider" => match val.as_str() {
                Some(s @ ("claude_cli" | "anthropic_api" | "openai" | "ollama_local")) => {
                    provider_str = Some(s.to_string());
                }
                Some(other) => {
                    eprintln!(
                        "warning: codebus config `llm.provider: {other}` is unknown; treating llm section as unset"
                    );
                    provider_was_explicitly_invalid = true;
                }
                None => warn_type_mismatch("llm.provider", "string", val),
            },
            "binary_path" => match val.as_str() {
                Some(s) => binary_path = Some(s.to_string()),
                None => warn_type_mismatch("llm.binary_path", "string", val),
            },
            "timeout_secs" => match val.as_u64() {
                Some(n) => timeout_secs = Some(n),
                None => warn_type_mismatch("llm.timeout_secs", "non-negative integer", val),
            },
            "api_key" => match val.as_str() {
                Some(s) => api_key = Some(s.to_string()),
                None => warn_type_mismatch("llm.api_key", "string", val),
            },
            // Forward-compat: unknown sub-fields silently ignored.
            // Sub-fields valid in a sibling variant (e.g. `api_key` under
            // `provider: claude_cli`) fall into this arm too — silently
            // dropped, matching the spec scenario "Sub-field valid in a
            // sibling variant is silently ignored".
            _ => {}
        }
    }

    if provider_was_explicitly_invalid {
        return None;
    }

    // Construct the variant. Missing provider field → default variant.
    let variant = match provider_str.as_deref() {
        None | Some("claude_cli") => ProviderConfig::ClaudeCli { binary_path },
        Some("anthropic_api") => ProviderConfig::AnthropicApi {
            api_key,
            timeout_secs,
        },
        Some("openai") => ProviderConfig::Openai {
            api_key,
            timeout_secs,
        },
        Some("ollama_local") => ProviderConfig::OllamaLocal {},
        _ => unreachable!("provider_str validated above"),
    };

    Some(variant)
}

fn parse_pii(v: &Value) -> Option<ScannerConfig> {
    let Value::Mapping(map) = v else {
        if !matches!(v, Value::Null) {
            warn_type_mismatch("pii", "mapping", v);
        }
        return Some(ScannerConfig::default());
    };

    let mut scanner_str: Option<String> = None;
    let mut on_hit: OnHit = OnHit::Warn;
    let mut patterns_extra: Vec<String> = Vec::new();
    let mut scanner_was_explicitly_invalid = false;

    for (k, val) in map {
        let Some(key) = k.as_str() else { continue };
        match key {
            "scanner" => match val.as_str() {
                Some(s @ ("null" | "regex_basic" | "presidio" | "aws")) => {
                    scanner_str = Some(s.to_string());
                }
                Some(other) => {
                    eprintln!(
                        "warning: codebus config `pii.scanner: {other}` is unknown; treating pii section as unset"
                    );
                    scanner_was_explicitly_invalid = true;
                }
                None => warn_type_mismatch("pii.scanner", "string", val),
            },
            "on_hit" => match val.as_str() {
                Some("warn") => on_hit = OnHit::Warn,
                Some("skip") => on_hit = OnHit::Skip,
                Some("mask") => on_hit = OnHit::Mask,
                Some(other) => {
                    eprintln!(
                        "warning: codebus config `pii.on_hit: {other}` is not one of warn|skip|mask; ignoring"
                    );
                }
                None => warn_type_mismatch("pii.on_hit", "string", val),
            },
            "patterns_extra" => match val {
                Value::Sequence(seq) => {
                    patterns_extra = seq
                        .iter()
                        .filter_map(|x| x.as_str().map(|s| s.to_string()))
                        .collect();
                }
                _ => warn_type_mismatch("pii.patterns_extra", "list of strings", val),
            },
            _ => {}
        }
    }

    if scanner_was_explicitly_invalid {
        return None;
    }

    let variant = match scanner_str.as_deref() {
        None | Some("null") => ScannerConfig::Null { on_hit },
        Some("regex_basic") => ScannerConfig::RegexBasic {
            on_hit,
            patterns_extra,
        },
        Some("presidio") => ScannerConfig::Presidio { on_hit },
        Some("aws") => ScannerConfig::Aws { on_hit },
        _ => unreachable!("scanner_str validated above"),
    };

    Some(variant)
}

fn parse_lint(v: &Value) -> Option<LintConfig> {
    let Value::Mapping(map) = v else {
        if !matches!(v, Value::Null) {
            warn_type_mismatch("lint", "mapping", v);
        }
        return Some(LintConfig::default());
    };
    let mut out = LintConfig::default();

    for (k, val) in map {
        let Some(key) = k.as_str() else { continue };
        match key {
            "disabled_rules" => match val {
                Value::Sequence(seq) => {
                    out.disabled_rules = seq
                        .iter()
                        .filter_map(|x| x.as_str().map(|s| s.to_string()))
                        .collect();
                }
                _ => warn_type_mismatch("lint.disabled_rules", "list of strings", val),
            },
            "custom_rules_dir" => match val.as_str() {
                Some(s) => out.custom_rules_dir = Some(s.to_string()),
                None if matches!(val, Value::Null) => {} // explicit null = unset
                None => warn_type_mismatch("lint.custom_rules_dir", "string", val),
            },
            "auto_fix" => out.auto_fix = parse_auto_fix(val),
            _ => {}
        }
    }
    Some(out)
}

fn parse_auto_fix(v: &Value) -> AutoFixConfig {
    let Value::Mapping(map) = v else {
        if !matches!(v, Value::Null) {
            warn_type_mismatch("lint.auto_fix", "mapping", v);
        }
        return AutoFixConfig::default();
    };
    let mut out = AutoFixConfig::default();
    for (k, val) in map {
        let Some(key) = k.as_str() else { continue };
        match key {
            "enabled" => match val.as_bool() {
                Some(b) => out.enabled = b,
                None => warn_type_mismatch("lint.auto_fix.enabled", "bool", val),
            },
            "max_iterations" => match val.as_u64() {
                Some(n) => out.max_iterations = n as u32,
                None => {
                    warn_type_mismatch("lint.auto_fix.max_iterations", "non-negative integer", val)
                }
            },
            // Forward-compat: unknown sub-fields silently ignored.
            _ => {}
        }
    }
    out
}

fn parse_render(v: &Value) -> Option<RendererConfig> {
    let Value::Mapping(map) = v else {
        if !matches!(v, Value::Null) {
            warn_type_mismatch("render", "mapping", v);
        }
        return Some(RendererConfig::default());
    };

    let mut format_str: Option<String> = None;
    let mut options: RenderOptions = RenderOptions::default();
    let mut format_was_explicitly_invalid = false;

    for (k, val) in map {
        let Some(key) = k.as_str() else { continue };
        match key {
            "format" => match val.as_str() {
                Some(s @ ("terminal" | "json_lines" | "tauri")) => {
                    format_str = Some(s.to_string());
                }
                Some(other) => {
                    eprintln!(
                        "warning: codebus config `render.format: {other}` is unknown; treating render section as unset"
                    );
                    format_was_explicitly_invalid = true;
                }
                None => warn_type_mismatch("render.format", "string", val),
            },
            "options" => match val {
                Value::Mapping(_) => match serde_yaml::from_value::<RenderOptions>(val.clone()) {
                    Ok(o) => options = o,
                    Err(_) => warn_type_mismatch(
                        "render.options",
                        "mapping with use_emoji/use_color bools",
                        val,
                    ),
                },
                _ => warn_type_mismatch("render.options", "mapping", val),
            },
            _ => {}
        }
    }

    if format_was_explicitly_invalid {
        return None;
    }

    let variant = match format_str.as_deref() {
        None | Some("terminal") => RendererConfig::Terminal { options },
        Some("json_lines") => RendererConfig::JsonLines {},
        Some("tauri") => RendererConfig::Tauri {},
        _ => unreachable!("format_str validated above"),
    };

    Some(variant)
}

fn parse_log(v: &Value) -> Option<SinkConfig> {
    let Value::Mapping(map) = v else {
        if !matches!(v, Value::Null) {
            warn_type_mismatch("log", "mapping", v);
        }
        return Some(SinkConfig::default());
    };

    let mut sink_str: Option<String> = None;
    let mut dir: Option<PathBuf> = None;
    let mut retention_days: Option<u32> = None;
    let mut sink_was_explicitly_invalid = false;

    for (k, val) in map {
        let Some(key) = k.as_str() else { continue };
        match key {
            "sink" => match val.as_str() {
                Some(s @ ("null" | "jsonl" | "otel")) => sink_str = Some(s.to_string()),
                Some(other) => {
                    eprintln!(
                        "warning: codebus config `log.sink: {other}` is unknown; treating log section as unset"
                    );
                    sink_was_explicitly_invalid = true;
                }
                None => warn_type_mismatch("log.sink", "string", val),
            },
            "dir" => match val.as_str() {
                Some(s) => dir = Some(PathBuf::from(s)),
                None => warn_type_mismatch("log.dir", "string", val),
            },
            "retention_days" => match val.as_u64() {
                Some(n) => retention_days = Some(n as u32),
                None => warn_type_mismatch("log.retention_days", "non-negative integer", val),
            },
            _ => {}
        }
    }

    if sink_was_explicitly_invalid {
        return None;
    }

    let variant = match sink_str.as_deref() {
        None | Some("null") => SinkConfig::Null {},
        Some("jsonl") => SinkConfig::Jsonl {
            dir,
            retention_days,
        },
        Some("otel") => SinkConfig::Otel {},
        _ => unreachable!("sink_str validated above"),
    };

    Some(variant)
}

fn warn_type_mismatch(field: &str, expected: &str, actual: &Value) {
    eprintln!(
        "warning: codebus config `{field}` expected {expected}, got {}; ignoring",
        type_name(actual)
    );
}

fn type_name(v: &Value) -> &'static str {
    match v {
        Value::Null => "null",
        Value::Bool(_) => "bool",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Sequence(_) => "list",
        Value::Mapping(_) => "mapping",
        Value::Tagged(_) => "tagged",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn nanos() -> u32 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .subsec_nanos()
    }

    fn write_tmp(name: &str, body: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "codebus-cfg-{name}-{}-{}",
            std::process::id(),
            nanos()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("config.yaml");
        fs::write(&path, body).unwrap();
        path
    }

    fn cleanup(p: &Path) {
        let _ = fs::remove_dir_all(p.parent().unwrap());
    }

    // --- Spec scenarios from `terminal-output/spec.md` "Load global config tolerantly" ---

    #[test]
    fn missing_config_returns_default() {
        let path = std::env::temp_dir().join(format!("codebus-cfg-missing-{}", nanos()));
        let cfg = load_config_from_path(&path);
        assert_eq!(cfg, GlobalConfig::default());
    }

    #[test]
    fn invalid_yaml_returns_default() {
        let p = write_tmp("badyaml", ":\n: : : not valid yaml :::");
        let cfg = load_config_from_path(&p);
        assert_eq!(cfg, GlobalConfig::default());
        cleanup(&p);
    }

    #[test]
    fn unknown_emoji_value_is_treated_as_unset() {
        let p = write_tmp("badmoji", "emoji: maybe\n");
        let cfg = load_config_from_path(&p);
        assert!(cfg.emoji.is_none());
        cleanup(&p);
    }

    #[test]
    fn future_top_level_field_is_silently_ignored() {
        let p = write_tmp(
            "futurekey",
            "emoji: on\nfuture_unknown_top_level: 'something'\n",
        );
        let cfg = load_config_from_path(&p);
        assert_eq!(cfg.emoji, Some(EmojiMode::On));
        cleanup(&p);
    }

    #[test]
    fn llm_section_selects_provider_via_discriminator() {
        let p = write_tmp(
            "llmclaude",
            "llm:\n  provider: claude_cli\n  binary_path: /usr/local/bin/claude\n",
        );
        let cfg = load_config_from_path(&p);
        let llm = cfg.llm.expect("llm section parsed");
        match llm {
            ProviderConfig::ClaudeCli { binary_path } => {
                assert_eq!(binary_path.as_deref(), Some("/usr/local/bin/claude"));
            }
            other => panic!("expected ClaudeCli, got {other:?}"),
        }
        cleanup(&p);
    }

    #[test]
    fn unknown_llm_provider_treats_section_as_unset() {
        let p = write_tmp("llmgib", "llm:\n  provider: gibberish\n  api_key: x\n");
        let cfg = load_config_from_path(&p);
        // Per spec: section treated as unset, factory falls through to default.
        assert!(cfg.llm.is_none());
        cleanup(&p);
    }

    #[test]
    fn unknown_sub_field_in_known_section_silently_ignored() {
        let p = write_tmp(
            "futsub",
            "llm:\n  provider: claude_cli\n  future_unknown_field: 1\n",
        );
        let cfg = load_config_from_path(&p);
        let llm = cfg.llm.expect("llm parsed");
        assert!(matches!(llm, ProviderConfig::ClaudeCli { .. }));
        cleanup(&p);
    }

    #[test]
    fn sub_field_valid_in_sibling_variant_is_silently_ignored() {
        // Spec scenario: api_key is valid for anthropic_api / openai variants
        // but NOT for claude_cli. Loader silently drops it (no warning, no
        // error) — matches "any unknown sub-field" treatment.
        let p = write_tmp(
            "siblingfield",
            "llm:\n  provider: claude_cli\n  api_key: secret\n",
        );
        let cfg = load_config_from_path(&p);
        let llm = cfg.llm.expect("llm parsed");
        match llm {
            ProviderConfig::ClaudeCli { binary_path } => {
                assert!(binary_path.is_none(), "claude_cli has no api_key field");
            }
            other => panic!("expected ClaudeCli, got {other:?}"),
        }
        cleanup(&p);
    }

    #[test]
    fn pii_section_selects_scanner_via_discriminator() {
        let p = write_tmp(
            "pii",
            "pii:\n  scanner: regex_basic\n  on_hit: warn\n  patterns_extra:\n    - 'INTERNAL-\\d{6}'\n",
        );
        let cfg = load_config_from_path(&p);
        let pii = cfg.pii.expect("pii parsed");
        match pii {
            ScannerConfig::RegexBasic {
                on_hit,
                patterns_extra,
            } => {
                assert_eq!(on_hit, OnHit::Warn);
                assert_eq!(patterns_extra.len(), 1);
                assert_eq!(patterns_extra[0], r"INTERNAL-\d{6}");
            }
            other => panic!("expected RegexBasic, got {other:?}"),
        }
        cleanup(&p);
    }

    #[test]
    fn lint_section_overrides_recognized() {
        let p = write_tmp("lintdis", "lint:\n  disabled_rules:\n    - oversize-page\n");
        let cfg = load_config_from_path(&p);
        let lint = cfg.lint.expect("lint parsed");
        assert_eq!(lint.disabled_rules, vec!["oversize-page"]);
        cleanup(&p);
    }

    // === lint-feedback-loop: lint.auto_fix parsing ===

    #[test]
    fn lint_section_without_auto_fix_falls_through_to_default() {
        let p = write_tmp(
            "noautofix",
            "lint:\n  disabled_rules:\n    - oversize-page\n",
        );
        let cfg = load_config_from_path(&p);
        let lint = cfg.lint.expect("lint parsed");
        assert!(lint.auto_fix.enabled);
        assert_eq!(lint.auto_fix.max_iterations, 5);
        cleanup(&p);
    }

    #[test]
    fn lint_auto_fix_explicit_values_parse() {
        let p = write_tmp(
            "autofixexplicit",
            "lint:\n  auto_fix:\n    enabled: false\n    max_iterations: 10\n",
        );
        let cfg = load_config_from_path(&p);
        let lint = cfg.lint.expect("lint parsed");
        assert!(!lint.auto_fix.enabled);
        assert_eq!(lint.auto_fix.max_iterations, 10);
        cleanup(&p);
    }

    #[test]
    fn lint_auto_fix_type_mismatch_falls_back_to_default_field() {
        let p = write_tmp(
            "autofixbad",
            "lint:\n  auto_fix:\n    enabled: true\n    max_iterations: 'twenty'\n",
        );
        let cfg = load_config_from_path(&p);
        let lint = cfg.lint.expect("lint parsed");
        assert!(lint.auto_fix.enabled);
        assert_eq!(lint.auto_fix.max_iterations, 5);
        cleanup(&p);
    }

    #[test]
    fn lint_auto_fix_unknown_subfield_silently_ignored() {
        let p = write_tmp(
            "autofixfut",
            "lint:\n  auto_fix:\n    enabled: false\n    future_unknown: 'x'\n",
        );
        let cfg = load_config_from_path(&p);
        let lint = cfg.lint.expect("lint parsed");
        assert!(!lint.auto_fix.enabled);
        cleanup(&p);
    }

    #[test]
    fn render_section_selects_renderer() {
        let p = write_tmp("render", "render:\n  format: terminal\n");
        let cfg = load_config_from_path(&p);
        let render = cfg.render.expect("render parsed");
        assert!(matches!(render, RendererConfig::Terminal { .. }));
        cleanup(&p);
    }

    #[test]
    fn log_section_selects_sink() {
        let p = write_tmp(
            "log",
            "log:\n  sink: jsonl\n  dir: /var/log/codebus\n  retention_days: 30\n",
        );
        let cfg = load_config_from_path(&p);
        let log = cfg.log.expect("log parsed");
        match log {
            SinkConfig::Jsonl {
                dir,
                retention_days,
            } => {
                assert_eq!(dir.as_deref(), Some(Path::new("/var/log/codebus")));
                assert_eq!(retention_days, Some(30));
            }
            other => panic!("expected Jsonl, got {other:?}"),
        }
        cleanup(&p);
    }

    #[test]
    fn empty_plugin_section_parses_as_defaults() {
        // Spec scenario: empty pii section parses to default variant
        // (Null with on_hit=Warn).
        let p = write_tmp("emptypii", "pii: {}\n");
        let cfg = load_config_from_path(&p);
        let pii = cfg.pii.expect("pii parsed as defaults");
        match pii {
            ScannerConfig::Null { on_hit } => {
                assert_eq!(on_hit, OnHit::Warn);
            }
            other => panic!("expected default Null variant, got {other:?}"),
        }
        cleanup(&p);
    }

    #[test]
    fn type_mismatched_sub_field_is_treated_as_unset() {
        // Spec scenario: bad timeout_secs only nukes that field, provider
        // is preserved. This pins **field-level** tolerance — switching to
        // section-level fallback would erase provider too.
        let p = write_tmp(
            "typemis",
            "llm:\n  provider: anthropic_api\n  api_key: ok\n  timeout_secs: 'thirty'\n",
        );
        let cfg = load_config_from_path(&p);
        let llm = cfg.llm.expect("llm parsed despite bad timeout");
        match llm {
            ProviderConfig::AnthropicApi {
                api_key,
                timeout_secs,
            } => {
                assert_eq!(api_key.as_deref(), Some("ok"));
                assert!(timeout_secs.is_none(), "bad timeout_secs should be unset");
            }
            other => panic!("expected AnthropicApi, got {other:?}"),
        }
        cleanup(&p);
    }

    // --- Extra robustness ---

    #[test]
    fn fully_specified_config_round_trips() {
        // `scanner` / `sink` use quoted strings to disambiguate from YAML's
        // bare `null` literal — both kinds happen to be named "null", so a
        // bare `null:` would parse as `Value::Null` instead of `"null"`.
        let body = r#"
emoji: off
llm:
  provider: claude_cli
  binary_path: claude
pii:
  scanner: "null"
  on_hit: warn
lint:
  disabled_rules: []
render:
  format: terminal
log:
  sink: "null"
"#;
        let p = write_tmp("fullspec", body);
        let cfg = load_config_from_path(&p);
        assert_eq!(cfg.emoji, Some(EmojiMode::Off));
        assert!(matches!(cfg.llm, Some(ProviderConfig::ClaudeCli { .. })));
        assert!(matches!(cfg.pii, Some(ScannerConfig::Null { .. })));
        assert!(matches!(cfg.render, Some(RendererConfig::Terminal { .. })));
        assert!(matches!(cfg.log, Some(SinkConfig::Null {})));
        cleanup(&p);
    }
}
