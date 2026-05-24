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

#[test]
fn neutral_rules_contains_language_policy_section() {
    // Per spec ADDED Requirement "NEUTRAL_RULES Language Policy" in change
    // prompt-surface-layer-1-batch: §0 Language Policy must exist in
    // NEUTRAL_RULES and must precede §1 Workspace Layout. Five SKILL workflow
    // body references (skill_bundle/mod.rs goal Step 5, query Step 4, fix
    // Step 5, and quiz test assertions) cite this section as the authority
    // for agent output language selection.
    let section_0 = "## 0. Language Policy";
    let section_1 = "## 1. Workspace Layout";

    let pos_0 = NEUTRAL_RULES.find(section_0).unwrap_or_else(|| {
        panic!(
            "neutral.md must contain `{section_0}` section; SKILL workflows reference it as the language contract"
        )
    });
    let pos_1 = NEUTRAL_RULES.find(section_1).unwrap_or_else(|| {
        panic!("neutral.md must contain `{section_1}` section")
    });

    assert!(
        pos_0 < pos_1,
        "`## 0. Language Policy` (byte offset {pos_0}) must precede `## 1. Workspace Layout` (byte offset {pos_1})"
    );

    let section_0_body_lower = NEUTRAL_RULES[pos_0..pos_1].to_lowercase();
    assert!(
        section_0_body_lower.contains("agent output"),
        "§0 Language Policy section must mention `agent output` (the entity whose language is constrained)"
    );
    assert!(
        section_0_body_lower.contains("structural tokens"),
        "§0 Language Policy section must mention `structural tokens` (the entity that stays literal English)"
    );
}
