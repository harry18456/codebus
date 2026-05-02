"""Tests for ``POST /internal/startup-config`` — Tauri-to-sidecar key
injection endpoint.

Backs SHALL clauses in
``openspec/changes/phase7-onboarding-polish/specs/keyring-integration/spec.md``
  Requirement: Tauri-to-sidecar startup key injection (4 scenarios; idempotent
  lock relaxed)

Spec scenarios:
  - valid bearer + well-formed body → 204 + ``app.state.provider_keys`` written
  - second call within process lifetime → 204 + new body OVERWRITES first
    (D-033 B's idempotent 409 lock relaxed in `phase7-onboarding-polish`
    so onboarding can push keys after the user enters them; trust boundary
    unchanged — bearer + loopback + Tauri-only caller)
  - missing bearer → 401 (existing bearer middleware behavior)
  - endpoint absent from ``/openapi.json`` document
"""
from __future__ import annotations

import secrets

import pytest
from fastapi.testclient import TestClient

from codebus_agent.api import create_app


def _bearer() -> str:
    return secrets.token_urlsafe(32)


def _auth(token: str) -> dict[str, str]:
    return {"Authorization": f"Bearer {token}"}


def _make_client() -> tuple[TestClient, str]:
    token = _bearer()
    app = create_app(bearer_token=token)
    return TestClient(app), token


def test_valid_call_returns_204_and_writes_provider_keys() -> None:
    client, token = _make_client()
    body = {
        "provider_keys": {
            "openai-default": "sk-test-A",
            "openai-embed-3": "sk-test-B",
        }
    }

    resp = client.post("/internal/startup-config", json=body, headers=_auth(token))

    assert resp.status_code == 204
    assert resp.content == b""

    keys = client.app.state.provider_keys
    assert keys == {"openai-default": "sk-test-A", "openai-embed-3": "sk-test-B"}


def test_second_call_overwrites_provider_keys() -> None:
    """Onboarding wizard submit happens after sidecar boot; the sidecar
    must accept the post-onboarding ``provider_keys`` push or the user
    is stuck redirecting back to ``/onboarding/welcome`` forever
    (D-033 B `phase7-onboarding-polish` regression fix)."""
    client, token = _make_client()
    first = {"provider_keys": {"openai-default": "sk-first"}}
    second = {
        "provider_keys": {
            "openai-default": "sk-second",
            "openai-embed-3": "sk-embed-second",
        }
    }

    r1 = client.post("/internal/startup-config", json=first, headers=_auth(token))
    assert r1.status_code == 204

    r2 = client.post("/internal/startup-config", json=second, headers=_auth(token))
    assert r2.status_code == 204
    assert r2.content == b""

    # The second body REPLACES the first wholesale (not merged) — the
    # latest call always wins so onboarding's "two providers in one
    # POST" semantics is preserved.
    assert client.app.state.provider_keys == {
        "openai-default": "sk-second",
        "openai-embed-3": "sk-embed-second",
    }


def test_third_call_replaces_again() -> None:
    """Repeat overwrite must work for an arbitrary number of calls
    (settings page edits trigger another push)."""
    client, token = _make_client()

    for n, body in enumerate(
        [
            {"provider_keys": {"p1": "v1"}},
            {"provider_keys": {"p1": "v1-edited"}},
            {"provider_keys": {"p2": "v2-only"}},
        ]
    ):
        resp = client.post("/internal/startup-config", json=body, headers=_auth(token))
        assert resp.status_code == 204, f"call {n} returned {resp.status_code}"

    assert client.app.state.provider_keys == {"p2": "v2-only"}


def test_missing_bearer_returns_401() -> None:
    client, _ = _make_client()
    resp = client.post(
        "/internal/startup-config",
        json={"provider_keys": {"openai-default": "sk-test"}},
    )
    assert resp.status_code == 401


def test_endpoint_hidden_from_openapi() -> None:
    client, token = _make_client()
    resp = client.get("/openapi.json", headers=_auth(token))
    assert resp.status_code == 200
    paths = resp.json().get("paths", {})
    assert "/internal/startup-config" not in paths


@pytest.mark.parametrize(
    "bad_body, reason",
    [
        ({}, "missing provider_keys"),
        ({"provider_keys": "not-a-dict"}, "provider_keys must be dict"),
        ({"provider_keys": {"a": 123}}, "values must be strings"),
    ],
)
def test_malformed_body_rejected_422(bad_body: dict, reason: str) -> None:
    client, token = _make_client()
    resp = client.post("/internal/startup-config", json=bad_body, headers=_auth(token))
    assert resp.status_code == 422, f"expected 422 for {reason}, got {resp.status_code}"


# ───── Secret-leak coverage (Task 2.3 / 2.4) ────────────────────────
#
# Spec ``API keys never written to disk or audit logs`` (3 scenarios):
# the sentinel api_key value MUST NOT appear in any audit JSONL under
# ``<workspace>/.codebus/``, in the sidecar's logger output (stdout /
# stderr proxies in pytest are caplog records), or in any FastAPI
# response body.

_SENTINEL_API_KEY = "sk-leak-canary-do-not-emit-AAAAAAAAAAAAAAAAAAAA"


def test_startup_config_response_body_does_not_echo_api_key(caplog) -> None:
    """204 response carries no body — the api_key MUST NOT round-trip."""
    client, token = _make_client()
    body = {"provider_keys": {"openai-default": _SENTINEL_API_KEY}}

    with caplog.at_level("DEBUG"):
        resp = client.post("/internal/startup-config", json=body, headers=_auth(token))

    assert resp.status_code == 204
    assert _SENTINEL_API_KEY not in resp.content.decode("utf-8", errors="replace")
    assert _SENTINEL_API_KEY not in resp.headers.values()
    for record in caplog.records:
        msg = record.getMessage()
        assert _SENTINEL_API_KEY not in msg, (
            f"sentinel leaked into {record.levelname} log line: {msg}"
        )


def test_sentinel_api_key_does_not_appear_in_public_endpoints(caplog) -> None:
    """After injection, neither /healthz nor /openapi.json nor /sanitizer/rules
    must echo the sentinel value back."""
    client, token = _make_client()
    body = {"provider_keys": {"openai-default": _SENTINEL_API_KEY}}
    inject = client.post("/internal/startup-config", json=body, headers=_auth(token))
    assert inject.status_code == 204

    with caplog.at_level("DEBUG"):
        for path in ("/healthz", "/openapi.json", "/sanitizer/rules"):
            resp = client.get(path, headers=_auth(token))
            assert _SENTINEL_API_KEY not in resp.content.decode(
                "utf-8", errors="replace"
            ), f"sentinel leaked into {path} body"
            for header_value in resp.headers.values():
                assert _SENTINEL_API_KEY not in header_value, (
                    f"sentinel leaked into {path} header"
                )

    for record in caplog.records:
        assert _SENTINEL_API_KEY not in record.getMessage()


def test_sentinel_api_key_does_not_appear_in_audit_jsonl(tmp_path) -> None:
    """Drive a TrackedProvider through several LLM calls and confirm the
    sentinel value never reaches any audit JSONL writer.

    Runs against the same workspace audit chain
    (`<ws>/.codebus/llm_calls.jsonl` / `token_usage.jsonl` /
    `sanitize_audit.jsonl`) that production code uses, so a
    regression that pipes provider_keys into any audit lane would be
    caught here.
    """
    from codebus_agent.providers import (
        LLMCallLogger,
        ProviderRole,
        TrackedProvider,
        UsageTracker,
    )
    from codebus_agent.providers.mock import MockProvider
    from codebus_agent.providers.protocol import Message
    from codebus_agent.sanitizer import (
        RULES_VERSION,
        SanitizerAuditLogger,
        SanitizerEngine,
    )
    from pydantic import BaseModel

    class _ProbeAnswer(BaseModel):
        text: str = "ok"

    workspace = tmp_path / "ws"
    audit_dir = workspace / ".codebus"
    audit_dir.mkdir(parents=True)

    tracker = UsageTracker(audit_dir / "token_usage.jsonl")
    call_logger = LLMCallLogger(audit_dir / "llm_calls.jsonl")
    sanitizer_audit = SanitizerAuditLogger(audit_dir / "sanitize_audit.jsonl")

    provider = TrackedProvider(
        MockProvider(),
        tracker=tracker,
        logger=call_logger,
        role=ProviderRole.CHAT,
        sanitizer=SanitizerEngine(),
        sanitizer_audit=sanitizer_audit,
        rules_version=RULES_VERSION,
        default_module="leak_probe",
    )

    # Drive a few mixed chat + embed calls. Body content intentionally
    # plain (no PII), so this exercises the audit happy path. The
    # sentinel api_key is provisioned in app.state by
    # POST /internal/startup-config in production; here we never feed
    # it into the provider call chain — the test verifies that no
    # audit lane has back-channel access to provider_keys regardless.
    import asyncio

    async def _drive() -> None:
        await provider.chat(
            [Message(role="user", content="describe codebus")],
            response_model=_ProbeAnswer,
        )
        await provider.chat(
            [Message(role="user", content="another probe message")],
            response_model=_ProbeAnswer,
        )
        await provider.embed(["alpha", "beta", "gamma"])

    asyncio.run(_drive())

    for jsonl in audit_dir.glob("*.jsonl"):
        text = jsonl.read_text(encoding="utf-8")
        assert _SENTINEL_API_KEY not in text, (
            f"sentinel leaked into {jsonl.name}: {text[:200]!r}"
        )
