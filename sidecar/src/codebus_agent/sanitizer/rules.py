"""Built-in Sanitizer rules — Secret / PII / internal-identifier kinds.

Backs SHALL clauses in
openspec/changes/sanitizer-safety-chain/specs/sanitizer/spec.md
  Requirement: Built-in rule set covers Secret, PII, internal-identifier kinds

Two rule shapes are provided:
- `RegexRule` for deterministic patterns (email, TW mobile, TW national
  ID, RFC1918 / RFC4193 / link-local, internal TLDs).
- `DetectSecretsRule` wraps `detect-secrets` and classifies a curated
  subset of its plugin types (AWS / JWT / Private Key) into our
  stable `kind` vocabulary. High-entropy suspects (Base64 / Hex) are
  intentionally dropped per `docs/sanitizer.md §十` — MVP does not
  surface suspect-level signals.
"""
from __future__ import annotations

import re
from dataclasses import dataclass
from typing import Protocol

from detect_secrets import settings as detect_secrets_settings
from detect_secrets.core import scan as detect_secrets_scan


@dataclass(frozen=True)
class RuleMatch:
    rule_id: str
    kind: str
    start: int
    end: int
    value: str


class Rule(Protocol):
    rule_id: str
    kind: str

    def find(self, text: str) -> list[RuleMatch]:
        ...


@dataclass(frozen=True)
class RegexRule:
    rule_id: str
    kind: str
    pattern: re.Pattern[str]

    def find(self, text: str) -> list[RuleMatch]:
        out: list[RuleMatch] = []
        for m in self.pattern.finditer(text):
            value = m.group(0)
            out.append(
                RuleMatch(
                    rule_id=self.rule_id,
                    kind=self.kind,
                    start=m.start(),
                    end=m.end(),
                    value=value,
                )
            )
        return out


_EMAIL_RE = re.compile(
    r"\b[A-Za-z0-9._%+\-]+@[A-Za-z0-9.\-]+\.[A-Za-z]{2,}\b",
)

# Taiwan mobile: either domestic `09XX-XXX-XXX` or international
# `+886-9XX-XXX-XXX` (the leading `0` drops when the country code is
# explicit). Hyphens are optional throughout.
_TW_MOBILE_RE = re.compile(
    r"(?:\+886-?9|09)\d{2}-?\d{3}-?\d{3}(?!\d)",
)

# Taiwan national ID: [A-Z][12]\d{8}. Guard with word-boundary-like
# surroundings so `9A12345678` does not match (leading digit before the
# letter must NOT be an alphanumeric).
_TW_NATIONAL_ID_RE = re.compile(
    r"(?<![A-Za-z0-9])[A-Z][12]\d{8}(?![A-Za-z0-9])",
)

_RFC1918_A = re.compile(r"\b10\.(?:\d{1,3})\.(?:\d{1,3})\.(?:\d{1,3})\b")
_RFC1918_B = re.compile(
    r"\b172\.(?:1[6-9]|2\d|3[01])\.(?:\d{1,3})\.(?:\d{1,3})\b",
)
_RFC1918_C = re.compile(r"\b192\.168\.(?:\d{1,3})\.(?:\d{1,3})\b")
_LINK_LOCAL_V4 = re.compile(r"\b169\.254\.(?:\d{1,3})\.(?:\d{1,3})\b")

# RFC4193 IPv6 unique local (fc00::/7, practically fd00::/8 for locally
# assigned). Match the first two hextets plus at least one more group.
_RFC4193_V6 = re.compile(
    r"\bfd[0-9a-fA-F]{2}(?::[0-9a-fA-F]{1,4}){1,7}(?:::[0-9a-fA-F]{1,4})?\b",
    re.IGNORECASE,
)

_INTERNAL_TLD_RE = re.compile(
    r"\b[A-Za-z][A-Za-z0-9\-]*(?:\.[A-Za-z][A-Za-z0-9\-]*)*\.(?:local|internal|corp|lan)\b",
)


_DETECT_SECRETS_KIND_MAP: dict[str, tuple[str, str]] = {
    "AWS Access Key": ("secret", "detect_secrets_aws_v1"),
    "JSON Web Token": ("jwt", "detect_secrets_jwt_v1"),
    "Private Key": ("private-key", "detect_secrets_privkey_v1"),
    "GitHub Token": ("secret", "detect_secrets_github_v1"),
    "Slack Token": ("secret", "detect_secrets_slack_v1"),
    "Stripe Access Key": ("secret", "detect_secrets_stripe_v1"),
    "Azure Storage Account access key": ("secret", "detect_secrets_azure_v1"),
    "Twilio API Key": ("secret", "detect_secrets_twilio_v1"),
    "NPM tokens": ("secret", "detect_secrets_npm_v1"),
    "SendGrid API Key": ("secret", "detect_secrets_sendgrid_v1"),
    "Mailchimp Access Key": ("secret", "detect_secrets_mailchimp_v1"),
    "Square OAuth Secret": ("secret", "detect_secrets_square_v1"),
}


@dataclass(frozen=True)
class DetectSecretsRule:
    """Wraps detect-secrets plugin output into our RuleMatch vocabulary."""

    rule_id: str = "detect_secrets_dispatch_v1"
    kind: str = "secret"  # default — real kind resolved per hit

    def find(self, text: str) -> list[RuleMatch]:
        out: list[RuleMatch] = []
        with detect_secrets_settings.default_settings():
            offset = 0
            for line in text.splitlines(keepends=True):
                for secret in detect_secrets_scan.scan_line(line):
                    mapping = _DETECT_SECRETS_KIND_MAP.get(secret.type)
                    if mapping is None:
                        continue
                    value = secret.secret_value
                    if not value:
                        continue
                    idx = line.find(value)
                    if idx < 0:
                        continue
                    start = offset + idx
                    end = start + len(value)
                    kind, rule_id = mapping
                    out.append(
                        RuleMatch(
                            rule_id=rule_id,
                            kind=kind,
                            start=start,
                            end=end,
                            value=value,
                        )
                    )
                offset += len(line)
        return out


def default_rules() -> list[Rule]:
    """Built-in rule table (stable ordering)."""
    return [
        RegexRule("pii_email_v1", "email", _EMAIL_RE),
        RegexRule("pii_tw_mobile_v1", "phone", _TW_MOBILE_RE),
        RegexRule("pii_tw_national_id_v1", "id", _TW_NATIONAL_ID_RE),
        RegexRule("net_rfc1918_a_v1", "ip", _RFC1918_A),
        RegexRule("net_rfc1918_b_v1", "ip", _RFC1918_B),
        RegexRule("net_rfc1918_c_v1", "ip", _RFC1918_C),
        RegexRule("net_link_local_v4_v1", "ip", _LINK_LOCAL_V4),
        RegexRule("net_rfc4193_v6_v1", "ip", _RFC4193_V6),
        RegexRule("net_internal_tld_v1", "internal-domain", _INTERNAL_TLD_RE),
        DetectSecretsRule(),
    ]
