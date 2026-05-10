//! `pii.*` config loader for v3-config PII Configuration Schema.
//!
//! Schema:
//! ```yaml
//! # ~/.codebus/config.yaml
//! pii:
//!   scanner: regex_basic   # or "none" to disable scanning entirely
//!   patterns_extra: []     # optional regex strings appended to built-in 4 patterns
//!   on_hit: warn           # warn | skip | mask — controls Warn-severity only;
//!                          # Critical-severity matches are unconditionally
//!                          # masked by raw_sync (security floor)
//! ```
//!
//! Why `"none"` not `"null"`: YAML treats `null` as the null literal, which
//! parses as an absent value — defeating "explicit opt-out". Using `none`
//! keeps the value as a string discriminator and avoids the foot-gun.
//!
//! All fields are optional. Defaults: `scanner: regex_basic`,
//! `patterns_extra: []`, `on_hit: warn`. Missing file / missing section /
//! missing field all fall through to defaults without stderr output.
//! Unknown keys inside `pii` are silently ignored (forward-compat). Unknown
//! enum discriminators (e.g. `on_hit: hyperflood`) cause `serde_yaml` to
//! return a parse error, which the caller is expected to translate into a
//! stderr warning + default fallback.
//!
//! Default policy history: v3-pii hardcoded `OnHit::Warn`; v3-config flipped
//! the default to `OnHit::Mask`; v3-pii-severity-dispatch reverts the default
//! back to `OnHit::Warn` while making raw_sync route Critical-severity matches
//! through Mask unconditionally (the prior `Mask` default mass-redacted
//! benign Warn-severity matches like example emails / `127.0.0.1` in docs).

use crate::pii::provider::OnHit;
use serde::Deserialize;
use std::fs;
use std::path::Path;

/// Active PII scanner selection. Maps to a concrete impl at the call site.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PiiScannerKind {
    /// `RegexBasicScanner` with built-in 4 patterns plus any `patterns_extra`.
    RegexBasic,
    /// `NullScanner` — produces no matches; used to opt-out entirely.
    Null,
}

impl Default for PiiScannerKind {
    fn default() -> Self {
        PiiScannerKind::RegexBasic
    }
}

/// Effective PII configuration after merging file + defaults.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PiiConfig {
    pub scanner: PiiScannerKind,
    pub patterns_extra: Vec<String>,
    pub on_hit: OnHit,
}

impl Default for PiiConfig {
    fn default() -> Self {
        Self {
            scanner: PiiScannerKind::RegexBasic,
            patterns_extra: Vec::new(),
            // v3-pii-severity-dispatch: default Warn-severity policy is Warn
            // (not Mask). Critical-severity matches are unconditionally masked
            // by raw_sync per the security floor — this default only governs
            // Warn-severity (email / ipv4) handling, where the prior `Mask`
            // default produced too many false-positive redactions in
            // docs / test fixtures.
            on_hit: OnHit::Warn,
        }
    }
}

#[derive(Debug, Default, Deserialize)]
struct ConfigFile {
    #[serde(default)]
    pii: Option<PiiSection>,
}

#[derive(Debug, Default, Deserialize)]
struct PiiSection {
    scanner: Option<PiiScannerKindWire>,
    #[serde(default)]
    patterns_extra: Option<Vec<String>>,
    on_hit: Option<OnHitWire>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
enum PiiScannerKindWire {
    RegexBasic,
    /// User-facing wire form is `"none"` — the YAML literal `null` would
    /// collapse to an absent field, which is indistinguishable from "key not
    /// present" and would silently fall back to default `RegexBasic`.
    #[serde(rename = "none")]
    Null,
}

impl From<PiiScannerKindWire> for PiiScannerKind {
    fn from(w: PiiScannerKindWire) -> Self {
        match w {
            PiiScannerKindWire::RegexBasic => PiiScannerKind::RegexBasic,
            PiiScannerKindWire::Null => PiiScannerKind::Null,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
enum OnHitWire {
    Warn,
    Skip,
    Mask,
}

impl From<OnHitWire> for OnHit {
    fn from(w: OnHitWire) -> Self {
        match w {
            OnHitWire::Warn => OnHit::Warn,
            OnHitWire::Skip => OnHit::Skip,
            OnHitWire::Mask => OnHit::Mask,
        }
    }
}

/// Load `pii.*` config from `path`. Returns defaults when the file does not
/// exist OR the `pii` section is absent. Returns `Err` only when the file
/// exists but cannot be read (IO error) or is structurally invalid YAML —
/// callers SHALL fall back to defaults on `Err` after printing a stderr
/// warning, mirroring the `lint.fix.*` loader contract.
pub fn load_pii_config(path: &Path) -> Result<PiiConfig, super::ConfigLoadError> {
    let body = match fs::read_to_string(path) {
        Ok(s) => s,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            return Ok(PiiConfig::default());
        }
        Err(err) => return Err(super::ConfigLoadError::Io(err)),
    };
    let file: ConfigFile = serde_yaml::from_str(&body).map_err(super::ConfigLoadError::YamlParse)?;
    let mut cfg = PiiConfig::default();
    if let Some(pii) = file.pii {
        if let Some(scanner) = pii.scanner {
            cfg.scanner = scanner.into();
        }
        if let Some(extras) = pii.patterns_extra {
            cfg.patterns_extra = extras;
        }
        if let Some(on_hit) = pii.on_hit {
            cfg.on_hit = on_hit.into();
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
        let cfg = load_pii_config(&tmp.path().join("nonexistent.yaml")).unwrap();
        assert_eq!(cfg, PiiConfig::default());
        assert_eq!(cfg.scanner, PiiScannerKind::RegexBasic);
        // v3-pii-severity-dispatch: default is Warn (governs Warn-severity
        // matches only; Critical-severity always masked by raw_sync).
        assert_eq!(cfg.on_hit, OnHit::Warn);
        assert!(cfg.patterns_extra.is_empty());
    }

    /// Spec: "Default config when pii section absent"
    #[test]
    fn default_when_pii_section_absent() {
        let tmp = TempDir::new().unwrap();
        let p = write_yaml(tmp.path(), "lint:\n  fix:\n    enabled: true\n");
        let cfg = load_pii_config(&p).unwrap();
        assert_eq!(cfg, PiiConfig::default());
    }

    /// Spec: "Partial config fills missing fields with defaults"
    #[test]
    fn partial_config_fills_missing_fields_with_defaults() {
        let tmp = TempDir::new().unwrap();
        let p = write_yaml(tmp.path(), "pii:\n  scanner: none\n");
        let cfg = load_pii_config(&p).unwrap();
        assert_eq!(cfg.scanner, PiiScannerKind::Null);
        // v3-pii-severity-dispatch: default Warn-severity policy is now Warn
        // (was Mask in v3-config — see PiiConfig::default doc comment).
        assert_eq!(cfg.on_hit, OnHit::Warn);
        assert!(cfg.patterns_extra.is_empty());
    }

    /// YAML `scanner: null` is the YAML null literal — explicitly NOT
    /// interpreted as "use NullScanner". Per the foot-gun-avoidance rationale,
    /// it falls through to the default `RegexBasic`. Users opt out via
    /// `scanner: none` instead.
    #[test]
    fn yaml_null_literal_falls_through_to_default_scanner() {
        let tmp = TempDir::new().unwrap();
        let p = write_yaml(tmp.path(), "pii:\n  scanner: null\n");
        let cfg = load_pii_config(&p).unwrap();
        assert_eq!(cfg.scanner, PiiScannerKind::RegexBasic);
    }

    /// Spec: "Unknown pii subkey silently ignored"
    #[test]
    fn unknown_pii_subkey_silently_ignored() {
        let tmp = TempDir::new().unwrap();
        let p = write_yaml(
            tmp.path(),
            "pii:\n  future_field: hello\n  scanner: regex_basic\n",
        );
        let cfg = load_pii_config(&p).unwrap();
        assert_eq!(cfg.scanner, PiiScannerKind::RegexBasic);
    }

    /// Spec: "Unknown on-hit value falls back to default" — loader returns
    /// parse Err; caller is responsible for the stderr+default fallback.
    #[test]
    fn unknown_on_hit_value_returns_parse_err() {
        let tmp = TempDir::new().unwrap();
        let p = write_yaml(tmp.path(), "pii:\n  on_hit: hyperflood\n");
        let result = load_pii_config(&p);
        assert!(matches!(
            result,
            Err(super::super::ConfigLoadError::YamlParse(_))
        ));
    }

    /// Patterns_extra round-trips a list of strings.
    #[test]
    fn patterns_extra_list_is_preserved() {
        let tmp = TempDir::new().unwrap();
        let p = write_yaml(
            tmp.path(),
            "pii:\n  patterns_extra:\n    - 'sk-foo-[a-z]+'\n    - 'token-\\d+'\n",
        );
        let cfg = load_pii_config(&p).unwrap();
        assert_eq!(
            cfg.patterns_extra,
            vec!["sk-foo-[a-z]+".to_string(), "token-\\d+".to_string()]
        );
    }

    /// All three on_hit variants parse correctly.
    #[test]
    fn on_hit_variants_parse() {
        let tmp = TempDir::new().unwrap();
        for (yaml_val, expected) in [
            ("warn", OnHit::Warn),
            ("skip", OnHit::Skip),
            ("mask", OnHit::Mask),
        ] {
            let p = write_yaml(tmp.path(), &format!("pii:\n  on_hit: {yaml_val}\n"));
            let cfg = load_pii_config(&p).unwrap();
            assert_eq!(cfg.on_hit, expected, "on_hit: {yaml_val}");
        }
    }

    /// Invalid YAML returns Err so caller can warn-and-default.
    #[test]
    fn invalid_yaml_returns_err() {
        let tmp = TempDir::new().unwrap();
        let p = write_yaml(tmp.path(), "pii:\n  : :: not yaml\n");
        let result = load_pii_config(&p);
        assert!(matches!(
            result,
            Err(super::super::ConfigLoadError::YamlParse(_))
        ));
    }
}
