use codebus_core::schema::NEUTRAL_RULES;

const FORBIDDEN_TOKENS: &[&str] = &[
    "claude",
    "anthropic",
    "stream-json",
    "--tools",
    "codex",
    "gemini",
    "cursor",
];

#[test]
fn neutral_rules_contains_no_vendor_specific_tokens() {
    let lower = NEUTRAL_RULES.to_lowercase();
    let mut violations: Vec<(usize, String, &str)> = Vec::new();
    for (idx, line) in lower.lines().enumerate() {
        for token in FORBIDDEN_TOKENS {
            if line.contains(token) {
                violations.push((idx + 1, line.to_string(), token));
            }
        }
    }
    assert!(
        violations.is_empty(),
        "neutral.md must not reference vendor-specific tokens. Violations:\n{}",
        violations
            .iter()
            .map(|(line, content, token)| format!(
                "  line {line}: contains '{token}' — `{content}`"
            ))
            .collect::<Vec<_>>()
            .join("\n")
    );
}

#[test]
fn neutral_rules_contains_five_taxonomy_folder_names() {
    for folder in ["concepts", "entities", "modules", "processes", "synthesis"] {
        assert!(
            NEUTRAL_RULES.contains(folder),
            "neutral.md must mention `{folder}` folder"
        );
    }
}

#[test]
fn neutral_rules_is_substantive() {
    assert!(NEUTRAL_RULES.len() > 1000);
}
