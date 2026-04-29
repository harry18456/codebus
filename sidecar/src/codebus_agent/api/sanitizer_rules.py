"""``GET /sanitizer/rules`` — read-only registry snapshot endpoint.

Backs SHALL clauses in
``openspec/changes/sanitizer-audit-inspector-p0/specs/sanitizer-audit-inspector/spec.md``
  Requirement: `GET /sanitizer/rules` sidecar endpoint exposes rules registry snapshot

Per design Decision 2 (rule explainer pulls from registry, not from
audit log) and Decision 5 (composable boundary): the endpoint is the
single source of truth for human-readable rule descriptions consumed
by the SanitizerAuditInspector overlay.

P0 strict invariants enforced here:
  * Endpoint MUST NOT touch ``SanitizerAuditLogger`` / ``AuditEntry``
    (Decision 1 — P0 metadata-only inspector)
  * ``rules_version`` MUST equal the value written into
    ``sanitize_audit.jsonl`` rows (constant ``RULES_VERSION``)
  * ``pattern_summary`` MUST be human-readable, not raw executable
    regex source (no ``(?P<`` / ``(?:``, length <= 80)
"""
from __future__ import annotations

from typing import Any

from fastapi import APIRouter

from codebus_agent.sanitizer import RULES_VERSION
from codebus_agent.sanitizer.config import SanitizerConfig, _global_config_path
from codebus_agent.sanitizer.rules import (
    _DETECT_SECRETS_KIND_MAP,
    DetectSecretsRule,
    RegexRule,
    default_rules,
)

router = APIRouter()


# Curated description + pattern_summary for every built-in rule_id. Keeping
# this hand-maintained avoids leaking executable regex source (named-group
# syntax `(?P<...>`, non-capturing `(?:...)`, lookahead `(?!...)` and
# lookbehind `(?<!...)`) into the wire response. Each entry's summary stays
# under 80 chars per spec.
_BUILTIN_DESCRIPTIONS: dict[str, tuple[str, str]] = {
    "pii_email_v1": (
        "Email address (RFC 5322 form)",
        "<email RFC 5322>",
    ),
    "pii_tw_mobile_v1": (
        "Taiwan mobile phone number",
        "+886-9XX-XXX-XXX or 09XX-XXX-XXX",
    ),
    "pii_tw_national_id_v1": (
        "Taiwan national ID",
        "[A-Z][1-2] followed by 8 digits",
    ),
    "net_rfc1918_a_v1": (
        "RFC 1918 private IPv4 (Class A range 10.0.0.0/8)",
        "10.x.x.x",
    ),
    "net_rfc1918_b_v1": (
        "RFC 1918 private IPv4 (Class B range 172.16.0.0/12)",
        "172.16-31.x.x",
    ),
    "net_rfc1918_c_v1": (
        "RFC 1918 private IPv4 (Class C range 192.168.0.0/16)",
        "192.168.x.x",
    ),
    "net_link_local_v4_v1": (
        "Link-local IPv4 (169.254.0.0/16)",
        "169.254.x.x",
    ),
    "net_rfc4193_v6_v1": (
        "RFC 4193 unique-local IPv6 (fd00::/8)",
        "fdXX::xxxx",
    ),
    "net_internal_tld_v1": (
        "Internal-use TLD (.local / .internal / .corp / .lan)",
        "*.local / *.internal / *.corp / *.lan",
    ),
}


_DETECT_SECRETS_DESCRIPTIONS: dict[str, str] = {
    "detect_secrets_aws_v1": "AWS access key (static credential)",
    "detect_secrets_jwt_v1": "JSON Web Token",
    "detect_secrets_privkey_v1": "PEM-formatted private key",
    "detect_secrets_github_v1": "GitHub personal access token",
    "detect_secrets_slack_v1": "Slack bot or user token",
    "detect_secrets_stripe_v1": "Stripe API access key",
    "detect_secrets_azure_v1": "Azure storage account access key",
    "detect_secrets_twilio_v1": "Twilio API key",
    "detect_secrets_npm_v1": "NPM access token",
    "detect_secrets_sendgrid_v1": "SendGrid API key",
    "detect_secrets_mailchimp_v1": "Mailchimp access key",
    "detect_secrets_square_v1": "Square OAuth secret",
}


_DETECT_SECRETS_PATTERN_SUMMARIES: dict[str, str] = {
    "detect_secrets_aws_v1": "AKIA[0-9A-Z]{16}",
    "detect_secrets_jwt_v1": "eyJ... (header.payload.signature)",
    "detect_secrets_privkey_v1": "-----BEGIN PRIVATE KEY-----",
    "detect_secrets_github_v1": "gh[pousr]_[A-Za-z0-9]{36,}",
    "detect_secrets_slack_v1": "xox[abprs]-[0-9A-Za-z\\-]+",
    "detect_secrets_stripe_v1": "sk_live_[0-9a-zA-Z]{24}",
    "detect_secrets_azure_v1": "<base64 storage key>",
    "detect_secrets_twilio_v1": "SK[0-9a-fA-F]{32}",
    "detect_secrets_npm_v1": "npm_[A-Za-z0-9]{36}",
    "detect_secrets_sendgrid_v1": "SG.[A-Za-z0-9_-]{22}.[A-Za-z0-9_-]{43}",
    "detect_secrets_mailchimp_v1": "<32-hex>-us[0-9]+",
    "detect_secrets_square_v1": "sq0csp-[A-Za-z0-9_-]{43}",
}


def _builtin_entries() -> list[dict[str, str]]:
    """Snapshot of the built-in rule table as wire-shaped dicts."""
    out: list[dict[str, str]] = []
    seen_ids: set[str] = set()
    for rule in default_rules():
        if isinstance(rule, RegexRule):
            description, pattern_summary = _BUILTIN_DESCRIPTIONS.get(
                rule.rule_id, (rule.rule_id, "<unsummarized>")
            )
            entry = {
                "rule_id": rule.rule_id,
                "kind": rule.kind,
                "description": description,
                "pattern_summary": pattern_summary,
                "source": "builtin",
            }
            if rule.rule_id not in seen_ids:
                out.append(entry)
                seen_ids.add(rule.rule_id)
        elif isinstance(rule, DetectSecretsRule):
            for _ds_type, (kind, rule_id) in _DETECT_SECRETS_KIND_MAP.items():
                if rule_id in seen_ids:
                    continue
                description = _DETECT_SECRETS_DESCRIPTIONS.get(rule_id, rule_id)
                pattern_summary = _DETECT_SECRETS_PATTERN_SUMMARIES.get(
                    rule_id, "<detect-secrets plugin>"
                )
                out.append(
                    {
                        "rule_id": rule_id,
                        "kind": kind,
                        "description": description,
                        "pattern_summary": pattern_summary,
                        "source": "builtin",
                    }
                )
                seen_ids.add(rule_id)
    return out


def _user_yaml_entries() -> list[dict[str, str]]:
    """Snapshot of user-yaml allowlist entries from the global config file.

    Workspace-scoped yaml is intentionally skipped at this endpoint —
    P0 has no workspace context here, and the global file is the
    single user-controlled source documented at
    ``~/.codebus/sanitizer.local.yaml``. Workspace-scoped exposure
    moves to a future change if needed.
    """
    global_path = _global_config_path()
    if not global_path.is_file():
        return []
    try:
        config = SanitizerConfig._from_yaml_file(global_path)
    except Exception:
        # Malformed yaml is the user's problem; surfacing details
        # could leak file content. Stay silent here; the SanitizerEngine
        # raises loudly when it tries to load the same file.
        return []
    out: list[dict[str, str]] = []
    for index, entry in enumerate(config.pattern_allowlist, start=1):
        summary = entry.pattern
        if len(summary) > 80:
            summary = summary[:77] + "..."
        out.append(
            {
                "rule_id": f"user_allowlist_{index}",
                "kind": "allowlist",
                "description": entry.reason,
                "pattern_summary": summary,
                "source": "user_yaml",
            }
        )
    return out


def build_rules_snapshot() -> dict[str, Any]:
    """Build the registry snapshot dict served by ``GET /sanitizer/rules``.

    Single function so tests can monkeypatch it cleanly. Production code
    paths MUST go through this entry point — direct callers of
    ``_builtin_entries`` / ``_user_yaml_entries`` bypass test injection.
    """
    rules = _builtin_entries() + _user_yaml_entries()
    return {"rules_version": RULES_VERSION, "rules": rules}


@router.get("/sanitizer/rules")
async def get_sanitizer_rules() -> dict[str, Any]:
    """Return the current effective rules registry as JSON.

    Read-only: no query parameters mutate state, no request body is
    accepted, and the SanitizerEngine / SanitizerAuditLogger are never
    touched (build_rules_snapshot only reads ``default_rules()`` and
    the global yaml).
    """
    return build_rules_snapshot()


__all__ = ["build_rules_snapshot", "router"]
