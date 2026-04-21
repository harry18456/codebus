"""Tests for built-in Sanitizer rules — covers Requirement
"Built-in rule set covers Secret, PII, internal-identifier kinds".
"""
from __future__ import annotations

import pytest

from codebus_agent.sanitizer.rules import default_rules


def _find_all(text: str):
    """Collect every `(rule_id, kind, value)` hit from the built-in rules."""
    matches = []
    for rule in default_rules():
        for m in rule.find(text):
            matches.append((m.rule_id, m.kind, m.value))
    return matches


def _kinds(text: str) -> set[str]:
    return {kind for _, kind, _ in _find_all(text)}


@pytest.mark.parametrize(
    "phone",
    ["0912-345-678", "0912345678", "+886-912-345-678", "+886912345678"],
)
def test_rule_taiwan_mobile(phone):
    assert "phone" in _kinds(f"contact the team at {phone} please")


def test_rule_taiwan_national_id():
    assert "id" in _kinds("citizen: A123456789 is valid")


def test_rule_taiwan_national_id_rejects_invalid_prefix():
    # "9" is not a valid area letter and "3" is not a valid gender digit.
    assert "id" not in _kinds("not an id: 9A12345678")


def test_rule_email_basic():
    assert "email" in _kinds("say hi to alice@example.com today")


@pytest.mark.parametrize(
    "ip",
    [
        "10.0.3.42",
        "10.255.255.255",
        "172.16.3.42",
        "172.31.0.1",
        "192.168.1.1",
        "169.254.1.1",
    ],
)
def test_rule_rfc1918_and_link_local_ipv4(ip):
    kinds = _kinds(f"server lives at {ip} internally")
    assert "ip" in kinds


def test_rule_rfc4193_ipv6():
    assert "ip" in _kinds("v6 host fd00:dead:beef::1 reachable")


def test_rule_ignores_public_ipv4():
    # Public IP MUST NOT trigger the RFC1918/link-local rule.
    assert "ip" not in _kinds("public dns 8.8.8.8 is not internal")


@pytest.mark.parametrize(
    "host",
    ["db01.local", "app.internal", "filer.lan", "vault.corp"],
)
def test_rule_internal_tld(host):
    assert "internal-domain" in _kinds(f"resolve {host} via split-horizon")


def test_rule_detect_secrets_aws_key_flagged_as_secret():
    text = "AWS_KEY=AKIAIOSFODNN7EXAMPLE"
    kinds = _kinds(text)
    assert "secret" in kinds


def test_rule_detect_secrets_jwt_flagged_as_jwt():
    jwt = (
        "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9."
        "eyJzdWIiOiIxMjM0NTY3ODkwIiwibmFtZSI6IkpvaG4gRG9lIn0."
        "SflKxwRJSMeKKF2QT4fwpMeJf36POk6yJV_adQssw5c"
    )
    assert "jwt" in _kinds(f"token: {jwt}")


def test_rule_detect_secrets_pem_flagged_as_private_key():
    pem = "-----BEGIN RSA PRIVATE KEY-----"
    assert "private-key" in _kinds(pem)


def test_rule_match_records_exact_span():
    """RuleMatch.start/end MUST bracket the original substring."""
    text = "email is alice@example.com ok"
    for rule in default_rules():
        for m in rule.find(text):
            assert text[m.start : m.end] == m.value
